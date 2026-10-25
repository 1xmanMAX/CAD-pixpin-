//! **El portapapeles de CAD-pixpin**: copias de entidades con su punto base
//! y sus capas, para pegarlas en el mismo dibujo o en otro abierto despues
//! en la misma sesion de la app (Ctrl+C, Ctrl+X, Ctrl+Mayus+C y Ctrl+V).
//!
//! Vive en el hilo de la interfaz (`thread_local`): la app tiene un solo
//! hilo que edita, y asi cada prueba tiene el suyo.

use std::cell::RefCell;
use std::rc::Rc;

use opencadcodec::{EntityType, Handle};

use crate::dibujo::Dibujo;
use crate::geo::{self, Caja, Forma, P};
use crate::{formas, operaciones as op};

/// Hasta cuantas formas se ensenan en la vista previa al pegar; con mas, solo
/// el recuadro de lo copiado (cada movimiento del raton las vuelve a pintar).
pub const FORMAS_EN_PREVIA: usize = 4000;

/// **Lo copiado.**
#[derive(Debug, Clone)]
pub struct Contenido {
    pub entidades: Vec<EntityType>,
    /// El punto que va al cursor al pegar.
    pub base: P,
    /// Las capas que usan, con su color ACI (se crean al pegar en otro dibujo).
    pub capas: Vec<(String, i16)>,
    /// Lo que se ensena al pegar, ya calculado (relativo a nada: en las
    /// coordenadas de donde se copio).
    pub formas: Vec<Forma>,
    pub caja: Caja,
}

thread_local! {
    static ACTUAL: RefCell<Option<Rc<Contenido>>> = const { RefCell::new(None) };
}

/// **Copia** las entidades `sel` del dibujo. `base`: el punto base (sin el,
/// la esquina inferior izquierda de lo copiado, como AutoCAD). `None` si no
/// hay nada que copiar.
pub fn copiar(d: &Dibujo, sel: &[Handle], base: Option<P>) -> Option<Contenido> {
    let mut entidades = Vec::new();
    let mut capas: Vec<(String, i16)> = Vec::new();
    let mut todas = Vec::new();
    let mut caja = geo::CAJA_VACIA;
    for h in sel {
        let Some(e) = d.entidad(*h) else { continue };
        let f = formas::formas(e);
        if let Some(c) = formas::caja(e, &f) {
            caja = geo::unir_cajas(caja, c);
        }
        if todas.len() <= FORMAS_EN_PREVIA {
            todas.extend(f);
        }
        let capa = &e.common().layer;
        if !capas.iter().any(|(n, _)| n == capa) {
            let color = d
                .doc
                .layers
                .get(capa)
                .and_then(|l| l.color.index())
                .map_or(7, |i| i as i16);
            capas.push((capa.clone(), color));
        }
        entidades.push(e.clone());
    }
    if entidades.is_empty() {
        return None;
    }
    if !geo::caja_valida(&caja) {
        caja = [0.0, 0.0, 0.0, 0.0];
    }
    if todas.len() > FORMAS_EN_PREVIA {
        todas = recuadro(caja);
    }
    Some(Contenido {
        entidades,
        base: base.unwrap_or([caja[0], caja[1]]),
        capas,
        formas: todas,
        caja,
    })
}

/// Las cuatro rayas de una caja.
fn recuadro(c: Caja) -> Vec<Forma> {
    let e = [[c[0], c[1]], [c[2], c[1]], [c[2], c[3]], [c[0], c[3]]];
    (0..4)
        .map(|i| Forma::Segmento {
            a: e[i],
            b: e[(i + 1) % 4],
        })
        .collect()
}

/// Lo deja en el portapapeles (lo que hubiera se pierde).
pub fn poner(c: Contenido) {
    ACTUAL.with(|a| *a.borrow_mut() = Some(Rc::new(c)));
}

/// Lo que hay en el portapapeles.
pub fn actual() -> Option<Rc<Contenido>> {
    ACTUAL.with(|a| a.borrow().clone())
}

pub fn vaciar() {
    ACTUAL.with(|a| *a.borrow_mut() = None);
}

/// Una forma movida `d`.
pub fn trasladar_forma(f: &Forma, d: P) -> Forma {
    match *f {
        Forma::Segmento { a, b } => Forma::Segmento {
            a: geo::sumar(a, d),
            b: geo::sumar(b, d),
        },
        Forma::Arco {
            c,
            r,
            inicio,
            barrido,
        } => Forma::Arco {
            c: geo::sumar(c, d),
            r,
            inicio,
            barrido,
        },
        Forma::Circulo { c, r } => Forma::Circulo {
            c: geo::sumar(c, d),
            r,
        },
    }
}

/// Lo que se ve al pegar con el punto base en `en`.
pub fn formas_en(c: &Contenido, en: P) -> Vec<Forma> {
    let d = geo::restar(en, c.base);
    c.formas.iter().map(|f| trasladar_forma(f, d)).collect()
}

/// El bloque que usa una entidad (inserciones y cotas), si usa alguno.
fn bloque_de(e: &EntityType) -> Option<&str> {
    match e {
        EntityType::Insert(i) => Some(&i.block_name),
        EntityType::Dimension(d) if !d.base().block_name.is_empty() => Some(&d.base().block_name),
        _ => None,
    }
}

/// **Pega** `c` con su punto base en `en`, de una vez (un paso de deshacer).
/// Crea las capas que falten. Lo que usa un bloque que este dibujo no tiene
/// (de otro plano) no se pega. Da lo nuevo y cuantos se dejaron fuera.
pub fn pegar(d: &mut Dibujo, c: &Contenido, en: P) -> (Vec<Handle>, usize) {
    for (capa, color) in &c.capas {
        d.asegurar_capa(capa, *color);
    }
    let mover = geo::restar(en, c.base);
    let mut fuera = 0;
    let listas: Vec<EntityType> = c
        .entidades
        .iter()
        .filter(|e| {
            let falta = bloque_de(e).is_some_and(|b| !d.doc.block_records.contains(b));
            fuera += falta as usize;
            !falta
        })
        .cloned()
        .map(|mut e| {
            op::trasladar(&mut e, mover);
            e
        })
        .collect();
    let nuevas = d.editar("PEGAR", |t| {
        listas
            .into_iter()
            .filter_map(|e| {
                let capa = e.common().layer.clone();
                t.anadir_en(e, &capa)
            })
            .collect()
    });
    (nuevas, fuera)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::especialidad::{circulo, linea};

    fn con_dos() -> (Dibujo, Vec<Handle>) {
        let mut d = Dibujo::nuevo();
        let hs = d.editar("x", |t| {
            vec![
                t.anadir_en(linea([10.0, 20.0], [110.0, 20.0]), "A-MUROS")
                    .unwrap(),
                t.anadir_en(circulo([60.0, 70.0], 10.0), "MIA").unwrap(),
            ]
        });
        (d, hs)
    }

    #[test]
    fn copiar_con_la_esquina_como_base() {
        let (d, hs) = con_dos();
        let c = copiar(&d, &hs, None).unwrap();
        assert_eq!(c.entidades.len(), 2);
        assert_eq!(c.base, [10.0, 20.0]);
        assert_eq!(c.capas.len(), 2);
        let c = copiar(&d, &hs, Some([1.0, 2.0])).unwrap();
        assert_eq!(c.base, [1.0, 2.0]);
    }

    #[test]
    fn nada_que_copiar() {
        let (d, _) = con_dos();
        assert!(copiar(&d, &[], None).is_none());
        assert!(copiar(&d, &[Handle::new(0xFF_FFFF)], None).is_none());
    }

    #[test]
    fn pegar_en_otro_dibujo_crea_las_capas() {
        let (d, hs) = con_dos();
        let c = copiar(&d, &hs, None).unwrap();
        let mut otro = Dibujo::nuevo();
        assert!(!otro.doc.layers.contains("MIA"));
        let (nuevas, fuera) = pegar(&mut otro, &c, [0.0, 0.0]);
        assert_eq!((nuevas.len(), fuera), (2, 0));
        assert!(otro.doc.layers.contains("MIA"));
        let EntityType::Line(l) = otro.entidad(nuevas[0]).unwrap() else {
            panic!()
        };
        assert_eq!((l.start.x, l.start.y), (0.0, 0.0));
        assert_eq!(otro.entidad(nuevas[1]).unwrap().common().layer, "MIA");
        // Un solo paso de deshacer.
        otro.deshacer();
        assert_eq!(otro.doc.model_space_entities().count(), 0);
    }

    #[test]
    fn un_bloque_que_no_esta_no_se_pega() {
        let mut ins = opencadcodec::entities::Insert::new("NO-EXISTE", Default::default());
        ins.common.layer = "0".into();
        let c = Contenido {
            entidades: vec![EntityType::Insert(ins)],
            base: [0.0, 0.0],
            capas: vec![("0".into(), 7)],
            formas: Vec::new(),
            caja: [0.0, 0.0, 0.0, 0.0],
        };
        let mut d = Dibujo::nuevo();
        let (nuevas, fuera) = pegar(&mut d, &c, [5.0, 5.0]);
        assert!(nuevas.is_empty());
        assert_eq!(fuera, 1);
        assert!(!d.se_puede_deshacer());
    }

    #[test]
    fn la_previa_va_con_el_cursor_y_se_resume_si_es_mucho() {
        let (d, hs) = con_dos();
        let c = copiar(&d, &hs, None).unwrap();
        let f = formas_en(&c, [0.0, 0.0]);
        let Forma::Segmento { a, .. } = f[0] else {
            panic!()
        };
        assert_eq!(a, [0.0, 0.0]);
        let mut grande = Dibujo::nuevo();
        let hs = grande.editar("x", |t| {
            (0..FORMAS_EN_PREVIA + 10)
                .filter_map(|i| t.anadir(linea([i as f64, 0.0], [i as f64, 1.0])))
                .collect::<Vec<_>>()
        });
        let c = copiar(&grande, &hs, None).unwrap();
        assert_eq!(c.formas.len(), 4);
    }

    #[test]
    fn el_portapapeles_se_llena_y_se_vacia() {
        vaciar();
        assert!(actual().is_none());
        let (d, hs) = con_dos();
        poner(copiar(&d, &hs, None).unwrap());
        assert_eq!(actual().unwrap().entidades.len(), 2);
        vaciar();
        assert!(actual().is_none());
    }
}
