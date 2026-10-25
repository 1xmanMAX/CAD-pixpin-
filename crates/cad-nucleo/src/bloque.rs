//! **Los bloques del dibujo**: definir uno (su registro y sus entidades,
//! relativas al punto base), saber cuales hay, ponerlo con un INSERT y sus
//! formas para la vista previa.
//!
//! La definicion no entra en el deshacer (como un estilo o una capa): lo que
//! se deshace es lo del espacio modelo (los originales y el INSERT).

use opencadcodec::entities::Insert;
use opencadcodec::tables::BlockRecord;
use opencadcodec::{CadDocument, EntityType, Handle, Vector3};

use crate::formas::{self, p2, v3};
use crate::geo::{self, Forma, P};
use crate::operaciones as op;

/// Hasta donde se entra en un bloque dentro de otro (para la previa).
const PROFUNDIDAD: u32 = 4;

/// Las letras que AutoCAD no admite en un nombre.
const PROHIBIDAS: &[char] = &[
    '<', '>', '/', '\\', '"', ':', ';', '?', '*', '|', ',', '=', '`',
];

/// **Revisa un nombre de bloque**: sin espacios a los lados, no vacio, sin
/// las letras prohibidas ni el `*` de los anonimos. Da el nombre limpio.
pub fn validar_nombre(nombre: &str) -> Result<String, String> {
    let n = nombre.trim();
    if n.is_empty() {
        return Err("El nombre no puede estar vacio".into());
    }
    if n.chars().count() > 255 {
        return Err("El nombre es demasiado largo (255 letras como mucho)".into());
    }
    if let Some(c) = n.chars().find(|c| PROHIBIDAS.contains(c) || c.is_control()) {
        return Err(format!("El nombre no puede llevar «{c}»"));
    }
    Ok(n.to_string())
}

/// Si el dibujo ya tiene un bloque con ese nombre (sin mirar mayusculas).
pub fn existe(doc: &CadDocument, nombre: &str) -> bool {
    doc.block_records.contains(nombre)
}

/// **Los bloques con nombre** del dibujo (sin los espacios, los anonimos,
/// las referencias externas ni lo que estas traen, `REF|BLOQUE`), en orden
/// alfabetico.
pub fn nombres(doc: &CadDocument) -> Vec<String> {
    let mut v: Vec<String> = doc
        .block_records
        .iter()
        .filter(|b| {
            !b.name.starts_with('*')
                && !b.flags.anonymous
                && !crate::xref::es_referencia(b)
                && !b.name.contains(crate::xref::SEPARADOR)
        })
        .map(|b| b.name.clone())
        .collect();
    v.sort_by_key(|n| n.to_uppercase());
    v
}

/// **Crea la definicion** `nombre` con `entidades` (en coordenadas del
/// dibujo), llevadas para que `base` quede en el origen del bloque. Las
/// entidades reciben handles nuevos. Da el handle del registro.
pub fn definir(
    doc: &mut CadDocument,
    nombre: &str,
    entidades: Vec<EntityType>,
    base: P,
) -> Result<Handle, String> {
    let nombre = validar_nombre(nombre)?;
    if existe(doc, &nombre) {
        return Err(format!("Ya hay un bloque «{nombre}»"));
    }
    let mut br = BlockRecord::new(nombre.clone());
    br.handle = doc.allocate_handle();
    br.block_entity_handle = doc.allocate_handle();
    br.block_end_handle = doc.allocate_handle();
    // El punto base queda en 0,0: las entidades ya van corridas. Asi lo ve
    // igual quien lea la base del registro o la del marcador BLOCK.
    br.base_point = Vector3::ZERO;
    let h = br.handle;
    doc.block_records.add(br)?;
    let corrimiento = Vector3::new(-base[0], -base[1], 0.0);
    for mut e in entidades {
        if !matches!(e, EntityType::Block(_) | EntityType::BlockEnd(_)) {
            e.translate(corrimiento);
            e.as_entity_mut().set_handle(Handle::NULL);
            e.common_mut().owner_handle = h;
            doc.add_entity(e).map_err(|e| e.to_string())?;
        }
    }
    Ok(h)
}

/// El punto base de un bloque (el del marcador BLOCK si lo hay, que es el
/// que usan los visores; si no, el del registro).
pub fn base_de(doc: &CadDocument, nombre: &str) -> P {
    let Some(br) = doc.block_records.get(nombre) else {
        return [0.0, 0.0];
    };
    match doc.get_entity(br.block_entity_handle) {
        Some(EntityType::Block(b)) => p2(&b.base_point),
        _ => p2(&br.base_point),
    }
}

/// **Un INSERT** de `nombre` en `punto`, a `escala` (uniforme) y girado `giro` radianes.
pub fn insercion(nombre: &str, punto: P, escala: f64, giro: f64) -> EntityType {
    EntityType::Insert(
        Insert::new(nombre, v3(punto))
            .with_uniform_scale(escala)
            .with_rotation(giro),
    )
}

/// **Las formas de un bloque** en su propio sistema (con el punto base en el
/// origen), entrando en los bloques que tenga dentro. Se calcula una vez y
/// luego se lleva al cursor con [`transformar`].
pub fn formas_de(doc: &CadDocument, nombre: &str) -> Vec<Forma> {
    let mut v = Vec::new();
    formas_de_bloque(doc, nombre, 0, &mut v);
    v
}

fn formas_de_bloque(doc: &CadDocument, nombre: &str, prof: u32, v: &mut Vec<Forma>) {
    if prof > PROFUNDIDAD {
        return;
    }
    let base = base_de(doc, nombre);
    let desde = v.len();
    for e in doc.entities_in_block(nombre) {
        formas_de_entidad(Some(doc), e, prof, v);
    }
    if base != [0.0, 0.0] {
        for f in &mut v[desde..] {
            *f = transformar(f, geo::por(base, -1.0), 1.0, 0.0);
        }
    }
}

/// Las formas de unas entidades sueltas (las de la biblioteca, antes de
/// ser bloque).
pub fn formas_de_lista(v: &[EntityType]) -> Vec<Forma> {
    let mut out = Vec::new();
    for e in v {
        formas_de_entidad(None, e, 0, &mut out);
    }
    out
}

fn formas_de_entidad(doc: Option<&CadDocument>, e: &EntityType, prof: u32, v: &mut Vec<Forma>) {
    match (e, doc) {
        (EntityType::Insert(i), Some(doc)) => {
            let desde = v.len();
            formas_de_bloque(doc, &i.block_name, prof + 1, v);
            let s = i.x_scale();
            for f in &mut v[desde..] {
                *f = transformar(f, p2(&i.insert_point), s, i.rotation);
            }
        }
        // Los bordes de un sombreado: con ellos basta para la previa.
        _ => v.extend(formas::formas(e)),
    }
}

/// **Las piezas de un INSERT**, ya en su sitio, con handles nulos. Con
/// escala uniforme cada pieza se mueve, escala y gira tal cual (un circulo
/// sigue siendo circulo); si no, lo hace opencadcodec (los circulos pasan a
/// elipses). Lo que esta en la capa 0 toma la del INSERT y lo que es "por
/// bloque", su color, como en AutoCAD.
pub fn descomponer(doc: &CadDocument, i: &Insert) -> Vec<EntityType> {
    let s = i.x_scale();
    let uniforme = (s - i.y_scale()).abs() < 1e-12 && s > 0.0 && !i.is_minsert();
    if !uniforme || i.normal.z < 0.0 {
        return i.explode_from_document(doc);
    }
    let base = base_de(doc, &i.block_name);
    let ins = p2(&i.insert_point);
    doc.entities_in_block(&i.block_name)
        .filter(|e| {
            !matches!(
                e,
                EntityType::Block(_) | EntityType::BlockEnd(_) | EntityType::AttributeDefinition(_)
            )
        })
        .map(|e| {
            let mut e = e.clone();
            e.as_entity_mut().set_handle(Handle::NULL);
            op::trasladar(&mut e, geo::por(base, -1.0));
            if (s - 1.0).abs() > 1e-12 {
                op::escalar(&mut e, [0.0, 0.0], s);
            }
            if i.rotation.abs() > 1e-15 {
                op::girar(&mut e, [0.0, 0.0], i.rotation);
            }
            op::trasladar(&mut e, ins);
            let c = e.common_mut();
            if c.layer == "0" {
                c.layer = i.common.layer.clone();
                c.layer_handle = i.common.layer_handle;
            }
            if matches!(c.color, opencadcodec::Color::ByBlock) {
                c.color = i.common.color;
            }
            e
        })
        .collect()
}

/// **Lleva una forma** a `ins`, escalada por `s` (mayor que cero) y girada
/// `giro` radianes alrededor del origen.
pub fn transformar(f: &Forma, ins: P, s: f64, giro: f64) -> Forma {
    let t = |p: P| geo::sumar(ins, geo::rotar(geo::por(p, s), [0.0, 0.0], giro));
    match *f {
        Forma::Segmento { a, b } => Forma::Segmento { a: t(a), b: t(b) },
        Forma::Circulo { c, r } => Forma::Circulo {
            c: t(c),
            r: r * s.abs(),
        },
        Forma::Arco {
            c,
            r,
            inicio,
            barrido,
        } => Forma::Arco {
            c: t(c),
            r: r * s.abs(),
            inicio: geo::normalizar(inicio + giro),
            barrido,
        },
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::especialidad as esp;

    #[test]
    fn nombres_validos_y_no() {
        assert_eq!(validar_nombre("  Mesa 1 ").unwrap(), "Mesa 1");
        assert!(validar_nombre("").is_err());
        assert!(validar_nombre("a/b").is_err());
        assert!(validar_nombre("*U1").is_err());
        assert!(validar_nombre(&"x".repeat(300)).is_err());
    }

    #[test]
    fn definir_corre_al_punto_base_y_no_repite_nombres() {
        let mut doc = CadDocument::new();
        definir(
            &mut doc,
            "Caja",
            vec![esp::linea([100.0, 100.0], [200.0, 100.0])],
            [100.0, 100.0],
        )
        .unwrap();
        let v: Vec<_> = doc.entities_in_block("CAJA").collect();
        assert_eq!(v.len(), 1);
        let EntityType::Line(l) = v[0] else { panic!() };
        assert_eq!((l.start.x, l.end.x), (0.0, 100.0));
        // No esta en el espacio modelo.
        assert_eq!(doc.model_space_entities().count(), 0);
        assert!(definir(&mut doc, "caja", vec![], [0.0, 0.0]).is_err());
        assert_eq!(nombres(&doc), ["Caja"]);
        let f = formas_de(&doc, "Caja");
        let g = transformar(&f[0], [10.0, 0.0], 2.0, std::f64::consts::FRAC_PI_2);
        let Forma::Segmento { a, b } = g else {
            panic!()
        };
        assert!(geo::dist(a, [10.0, 0.0]) < 1e-9 && geo::dist(b, [10.0, 200.0]) < 1e-9);
    }
}
