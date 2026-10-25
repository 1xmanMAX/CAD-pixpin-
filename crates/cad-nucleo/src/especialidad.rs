//! **Las herramientas del oficio**: lo que se dibuja a diario en un plano de
//! arquitectura y que en un CAD generico son diez pasos (un muro con su
//! espesor, una malla de ejes con sus burbujas, una puerta con su giro, una
//! cota, el nivel de un piso, la lamina con su membrete…). Cada funcion da
//! las entidades ya hechas; la orden las anade en su capa.

use std::f64::consts::{FRAC_PI_2, PI};

use opencadcodec::entities::{Solid, TextHorizontalAlignment, TextVerticalAlignment};
use opencadcodec::{Arc, Circle, EntityType, Line, LwPolyline, Text, Vector2};

use crate::formas::v3;
use crate::geo::{self, P};

/// Una capa de la plantilla.
pub struct CapaDePlantilla {
    pub nombre: &'static str,
    /// Color ACI (1 rojo, 2 amarillo, 3 verde, 4 cian, 5 azul, 6 magenta, 7 blanco/negro…).
    pub color: i16,
    pub descripcion: &'static str,
}

/// **Las capas de un plano nuevo**, por especialidad (A arquitectura, E
/// estructuras, IE electricas, IS sanitarias), como se suelen nombrar en
/// los expedientes.
pub const CAPAS: &[CapaDePlantilla] = &[
    CapaDePlantilla {
        nombre: "A-MUROS",
        color: 7,
        descripcion: "Muros y tabiques",
    },
    CapaDePlantilla {
        nombre: "A-PUERTAS",
        color: 3,
        descripcion: "Puertas y su giro",
    },
    CapaDePlantilla {
        nombre: "A-VENTANAS",
        color: 4,
        descripcion: "Ventanas y mamparas",
    },
    CapaDePlantilla {
        nombre: "A-MOBILIARIO",
        color: 8,
        descripcion: "Mobiliario y aparatos",
    },
    CapaDePlantilla {
        nombre: "A-COTAS",
        color: 1,
        descripcion: "Cotas",
    },
    CapaDePlantilla {
        nombre: "A-TEXTOS",
        color: 2,
        descripcion: "Textos y rotulos",
    },
    CapaDePlantilla {
        nombre: "A-EJES",
        color: 1,
        descripcion: "Ejes y sus burbujas",
    },
    CapaDePlantilla {
        nombre: "A-NIVELES",
        color: 6,
        descripcion: "Niveles de piso",
    },
    CapaDePlantilla {
        nombre: "E-ESTRUCTURAS",
        color: 5,
        descripcion: "Columnas, vigas y placas",
    },
    CapaDePlantilla {
        nombre: "IE-ELECTRICAS",
        color: 30,
        descripcion: "Instalaciones electricas",
    },
    CapaDePlantilla {
        nombre: "IS-SANITARIAS",
        color: 140,
        descripcion: "Instalaciones sanitarias",
    },
    CapaDePlantilla {
        nombre: "L-LAMINA",
        color: 7,
        descripcion: "Marco y membrete",
    },
];

pub const CAPA_MUROS: &str = "A-MUROS";
pub const CAPA_PUERTAS: &str = "A-PUERTAS";
pub const CAPA_VENTANAS: &str = "A-VENTANAS";
pub const CAPA_COTAS: &str = "A-COTAS";
pub const CAPA_TEXTOS: &str = "A-TEXTOS";
pub const CAPA_EJES: &str = "A-EJES";
pub const CAPA_NIVELES: &str = "A-NIVELES";
pub const CAPA_LAMINA: &str = "L-LAMINA";

/// El color ACI de una capa de la plantilla (7 si no es de la plantilla).
pub fn color_de_capa(nombre: &str) -> i16 {
    CAPAS
        .iter()
        .find(|c| c.nombre == nombre)
        .map_or(7, |c| c.color)
}

pub fn linea(a: P, b: P) -> EntityType {
    EntityType::Line(Line::from_points(v3(a), v3(b)))
}

pub fn polilinea(puntos: &[P], cerrada: bool) -> EntityType {
    let mut p = LwPolyline::from_points(puntos.iter().map(|q| Vector2::new(q[0], q[1])).collect());
    p.is_closed = cerrada;
    EntityType::LwPolyline(p)
}

pub fn circulo(c: P, r: f64) -> EntityType {
    EntityType::Circle(Circle::from_center_radius(v3(c), r))
}

/// Un arco antihorario de `inicio` a `fin` (radianes).
pub fn arco(c: P, r: f64, inicio: f64, fin: f64) -> EntityType {
    EntityType::Arc(Arc {
        center: v3(c),
        radius: r,
        start_angle: inicio,
        end_angle: fin,
        ..Default::default()
    })
}

/// Un texto de una linea, con su punto de alineacion.
pub fn texto(valor: &str, p: P, alto: f64, rot: f64, centrado: bool) -> EntityType {
    let mut t = Text::with_value(valor, v3(p))
        .with_height(alto)
        .with_rotation(rot);
    if centrado {
        t.horizontal_alignment = TextHorizontalAlignment::Center;
        t.vertical_alignment = TextVerticalAlignment::Middle;
        t.alignment_point = Some(v3(p));
    }
    EntityType::Text(t)
}

/// Un triangulo relleno (las flechas de las cotas, la del norte).
pub fn relleno(a: P, b: P, c: P) -> EntityType {
    EntityType::Solid(Solid::triangle(v3(a), v3(b), v3(c)))
}

/// Desplaza una polilinea abierta (o cerrada) `d` a su izquierda, con las
/// esquinas a inglete. `None` si tiene menos de dos puntos.
pub fn desplazar_polilinea(pts: &[P], d: f64, cerrada: bool) -> Option<Vec<P>> {
    let n = pts.len();
    if n < 2 {
        return None;
    }
    let tramos = if cerrada { n } else { n - 1 };
    let lineas: Vec<(P, P)> = (0..tramos)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            let o = geo::por(geo::perp(geo::unitario(geo::restar(b, a))), d);
            (geo::sumar(a, o), geo::sumar(b, o))
        })
        .collect();
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
        let (anterior, siguiente) = if cerrada {
            (
                Some(lineas[(i + tramos - 1) % tramos]),
                Some(lineas[i % tramos]),
            )
        } else {
            (i.checked_sub(1).map(|k| lineas[k]), lineas.get(i).copied())
        };
        let p = match (anterior, siguiente) {
            (Some(a), Some(s)) => geo::corte_rectas(a.0, a.1, s.0, s.1).unwrap_or(a.1),
            (Some(a), None) => a.1,
            (None, Some(s)) => s.0,
            (None, None) => return None,
        };
        v.push(p);
    }
    Some(v)
}

/// **Un muro** por su eje: dos lineas paralelas a medio espesor de cada lado
/// y, si no es cerrado, las dos tapas.
pub fn muro(eje: &[P], espesor: f64, cerrado: bool) -> Vec<EntityType> {
    let e = espesor.abs() / 2.0;
    let (Some(izq), Some(der)) = (
        desplazar_polilinea(eje, e, cerrado),
        desplazar_polilinea(eje, -e, cerrado),
    ) else {
        return Vec::new();
    };
    let mut v = vec![polilinea(&izq, cerrado), polilinea(&der, cerrado)];
    if !cerrado {
        v.push(linea(izq[0], der[0]));
        v.push(linea(*izq.last().unwrap(), *der.last().unwrap()));
    }
    v
}

/// El nombre del eje `i` en letras: A..Z, AA, AB… (la I y la O se saltan,
/// para no confundirlas con 1 y 0).
pub fn letra_de_eje(i: usize) -> String {
    const LETRAS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ";
    let mut i = i;
    let mut s = Vec::new();
    loop {
        s.push(LETRAS[i % LETRAS.len()]);
        if i < LETRAS.len() {
            break;
        }
        i = i / LETRAS.len() - 1;
    }
    s.reverse();
    String::from_utf8(s).unwrap_or_default()
}

/// **Una malla de ejes**: verticales numerados (1, 2, 3…) separados por
/// `sep_x` y horizontales con letras (A, B, C…) separados por `sep_y`, desde
/// `origen`, con su burbuja de radio `r` al final de cada uno.
pub fn ejes(origen: P, sep_x: &[f64], sep_y: &[f64], r: f64) -> Vec<EntityType> {
    let xs: Vec<f64> = std::iter::once(0.0)
        .chain(sep_x.iter().scan(0.0, |s, d| {
            *s += d;
            Some(*s)
        }))
        .collect();
    let ys: Vec<f64> = std::iter::once(0.0)
        .chain(sep_y.iter().scan(0.0, |s, d| {
            *s += d;
            Some(*s)
        }))
        .collect();
    let (ancho, alto) = (*xs.last().unwrap(), *ys.last().unwrap());
    // Lo que sobresalen los ejes de la malla, mas la burbuja.
    let sale = r * 3.0;
    let mut v = Vec::new();
    for (i, x) in xs.iter().enumerate() {
        let x = origen[0] + x;
        let (a, b) = ([x, origen[1] - sale], [x, origen[1] + alto + sale]);
        v.push(linea(a, b));
        let c = [x, b[1] + r];
        v.push(circulo(c, r));
        v.push(texto(&(i + 1).to_string(), c, r, 0.0, true));
    }
    for (j, y) in ys.iter().enumerate() {
        let y = origen[1] + y;
        let (a, b) = ([origen[0] - sale, y], [origen[0] + ancho + sale, y]);
        v.push(linea(a, b));
        let c = [a[0] - r, y];
        v.push(circulo(c, r));
        v.push(texto(&letra_de_eje(j), c, r, 0.0, true));
    }
    v
}

/// **Una puerta**: la hoja desde la bisagra hacia `hacia` (con el ancho del
/// vano) y el arco de su giro de 90 grados. `izquierda` dice hacia que lado
/// abre (mirando de la bisagra a la hoja cerrada a lo largo del muro).
pub fn puerta(bisagra: P, a_lo_largo: P, ancho: f64, izquierda: bool) -> Vec<EntityType> {
    let dir = geo::unitario(geo::restar(a_lo_largo, bisagra));
    if geo::norma(dir) < geo::EPS || ancho <= 0.0 {
        return Vec::new();
    }
    let cerrada = geo::sumar(bisagra, geo::por(dir, ancho));
    let giro = if izquierda { FRAC_PI_2 } else { -FRAC_PI_2 };
    let abierta = geo::rotar(cerrada, bisagra, giro);
    let (a0, a1) = (geo::angulo(bisagra, cerrada), geo::angulo(bisagra, abierta));
    let (inicio, fin) = if izquierda { (a0, a1) } else { (a1, a0) };
    vec![linea(bisagra, abierta), arco(bisagra, ancho, inicio, fin)]
}

/// **Una ventana** en un muro de `espesor`, entre `a` y `b` (sobre el eje
/// del muro): el marco y dos lineas de vidrio.
pub fn ventana(a: P, b: P, espesor: f64) -> Vec<EntityType> {
    let dir = geo::unitario(geo::restar(b, a));
    if geo::norma(dir) < geo::EPS {
        return Vec::new();
    }
    let n = geo::perp(dir);
    let e = espesor.abs() / 2.0;
    let q = |p: P, k: f64| geo::sumar(p, geo::por(n, k));
    let mut v = vec![polilinea(&[q(a, e), q(b, e), q(b, -e), q(a, -e)], true)];
    let vidrio = e / 4.0;
    v.push(linea(q(a, vidrio), q(b, vidrio)));
    v.push(linea(q(a, -vidrio), q(b, -vidrio)));
    v
}

/// Como se escribe una medida (en las unidades del dibujo).
pub fn texto_de_medida(d: f64, decimales: usize) -> String {
    format!("{d:.decimales$}")
}

/// **Una cota alineada** de `a` a `b`, con la linea de cota pasando por
/// `por` (lineas de extension, linea de cota, marcas a 45 grados como en
/// los planos de arquitectura y el texto encima). Va desmontada (lineas y
/// texto): se ve igual en cualquier programa.
pub fn cota(a: P, b: P, por: P, alto_texto: f64, decimales: usize) -> Vec<EntityType> {
    let d = geo::dist(a, b);
    if d < geo::EPS {
        return Vec::new();
    }
    let dir = geo::unitario(geo::restar(b, a));
    let n = geo::perp(dir);
    // Lo que se separa la linea de cota de lo medido (con signo).
    let sep = geo::punto(geo::restar(por, a), n);
    let (ca, cb) = (
        geo::sumar(a, geo::por(n, sep)),
        geo::sumar(b, geo::por(n, sep)),
    );
    let s = sep.signum();
    let hueco = alto_texto * 0.5;
    let pasa = alto_texto * 0.6;
    let mut v = Vec::new();
    // Lineas de extension: dejan un hueco junto al objeto y pasan un poco la cota.
    for (p, c) in [(a, ca), (b, cb)] {
        if sep.abs() > hueco {
            v.push(linea(
                geo::sumar(p, geo::por(n, hueco * s)),
                geo::sumar(c, geo::por(n, pasa * s)),
            ));
        }
    }
    // La linea de cota, pasando un poco de las extensiones.
    v.push(linea(
        geo::sumar(ca, geo::por(dir, -pasa)),
        geo::sumar(cb, geo::por(dir, pasa)),
    ));
    // Las marcas (trazos a 45 grados).
    let m = geo::por(geo::unitario(geo::sumar(dir, n)), alto_texto * 0.5);
    for c in [ca, cb] {
        v.push(linea(geo::restar(c, m), geo::sumar(c, m)));
    }
    // El texto encima de la linea, siempre legible (sin quedar boca abajo).
    let mut rot = geo::angulo(a, b);
    if rot > FRAC_PI_2 + 1e-9 || rot <= -FRAC_PI_2 + 1e-9 {
        rot += PI;
    }
    let arriba = geo::perp([rot.cos(), rot.sin()]);
    let t = geo::sumar(geo::medio(ca, cb), geo::por(arriba, alto_texto * 0.9));
    v.push(texto(
        &texto_de_medida(d, decimales),
        t,
        alto_texto,
        rot,
        true,
    ));
    v
}

/// **Un nivel de piso** (NPT): el triangulo con la punta en `p`, su raya y
/// el valor (`+0.15`).
pub fn nivel(p: P, valor: f64, alto_texto: f64) -> Vec<EntityType> {
    let h = alto_texto * 1.2;
    let tri = [p, [p[0] - h * 0.6, p[1] + h], [p[0] + h * 0.6, p[1] + h]];
    let signo = if valor >= 0.0 { "+" } else { "-" };
    vec![
        polilinea(&tri, true),
        linea([p[0] - h * 0.6, p[1] + h], [p[0] + h * 4.0, p[1] + h]),
        texto(
            &format!("NPT {signo}{:.2}", valor.abs()),
            [p[0] + h * 0.8, p[1] + h * 1.3],
            alto_texto,
            0.0,
            false,
        ),
    ]
}

/// **La flecha del norte** en `c`, de radio `r`, girada `ang` desde arriba.
pub fn norte(c: P, r: f64, ang: f64) -> Vec<EntityType> {
    let gira = |p: P| geo::rotar(p, c, ang);
    let punta = gira([c[0], c[1] + r]);
    let izq = gira([c[0] - r * 0.4, c[1] - r * 0.6]);
    let der = gira([c[0] + r * 0.4, c[1] - r * 0.6]);
    vec![
        circulo(c, r),
        relleno(punta, c, der),
        polilinea(&[punta, izq, c], false),
        texto("N", gira([c[0], c[1] + r * 1.45]), r * 0.4, ang, true),
    ]
}

/// Los formatos ISO de papel, en milimetros (apaisados).
pub const FORMATOS: &[(&str, f64, f64)] = &[
    ("A4", 297.0, 210.0),
    ("A3", 420.0, 297.0),
    ("A2", 594.0, 420.0),
    ("A1", 841.0, 594.0),
    ("A0", 1189.0, 841.0),
];

pub fn formato(nombre: &str) -> Option<(f64, f64)> {
    FORMATOS
        .iter()
        .find(|f| f.0.eq_ignore_ascii_case(nombre.trim()))
        .map(|f| (f.1, f.2))
}

/// Los datos del membrete.
#[derive(Debug, Clone, Default)]
pub struct Membrete {
    pub proyecto: String,
    pub plano: String,
    pub autor: String,
    pub fecha: String,
    pub lamina: String,
}

/// Cuantas unidades del dibujo es un milimetro de papel a escala 1:`escala`,
/// segun las unidades del dibujo (`$INSUNITS`: 4 mm, 5 cm, 6 m).
pub fn unidades_por_mm(escala: f64, insunits: i16) -> f64 {
    let mm_por_unidad = match insunits {
        5 => 10.0,
        6 => 1000.0,
        1 => 25.4,
        _ => 1.0,
    };
    escala / mm_por_unidad
}

/// **Una lamina**: el borde del papel, el margen (25 mm a la izquierda para
/// anillar, 10 mm el resto) y el membrete abajo a la derecha, todo a
/// escala con la esquina de abajo a la izquierda en `origen`.
pub fn lamina(
    origen: P,
    ancho_mm: f64,
    alto_mm: f64,
    k: f64,
    escala: f64,
    datos: &Membrete,
) -> Vec<EntityType> {
    let p = |x: f64, y: f64| [origen[0] + x * k, origen[1] + y * k];
    let mut v = vec![
        polilinea(
            &[
                p(0.0, 0.0),
                p(ancho_mm, 0.0),
                p(ancho_mm, alto_mm),
                p(0.0, alto_mm),
            ],
            true,
        ),
        polilinea(
            &[
                p(25.0, 10.0),
                p(ancho_mm - 10.0, 10.0),
                p(ancho_mm - 10.0, alto_mm - 10.0),
                p(25.0, alto_mm - 10.0),
            ],
            true,
        ),
    ];
    // El membrete: 180 x 50 mm, en filas.
    let (mw, mh) = (180.0_f64.min(ancho_mm - 35.0), 50.0);
    let (x0, y0, x1) = (ancho_mm - 10.0 - mw, 10.0, ancho_mm - 10.0);
    v.push(polilinea(
        &[p(x0, y0), p(x1, y0), p(x1, y0 + mh), p(x0, y0 + mh)],
        true,
    ));
    let filas = [
        ("PROYECTO", datos.proyecto.as_str()),
        ("PLANO", datos.plano.as_str()),
        ("ESCALA", &*format!("1/{}", escala.round() as i64)),
        ("AUTOR", datos.autor.as_str()),
        ("FECHA", datos.fecha.as_str()),
    ];
    let alto_fila = mh / filas.len() as f64;
    let col = x0 + 32.0;
    v.push(linea(p(col, y0), p(col, y0 + mh)));
    // La casilla del numero de lamina, a la derecha.
    let caja_num = x1 - 30.0;
    v.push(linea(p(caja_num, y0), p(caja_num, y0 + mh)));
    for (i, (etiqueta, valor)) in filas.iter().enumerate() {
        let y = y0 + mh - alto_fila * (i as f64 + 1.0);
        if i + 1 < filas.len() {
            v.push(linea(p(x0, y), p(caja_num, y)));
        }
        v.push(texto(
            etiqueta,
            p(x0 + 2.0, y + alto_fila * 0.3),
            2.5 * k,
            0.0,
            false,
        ));
        v.push(texto(
            valor,
            p(col + 2.0, y + alto_fila * 0.3),
            3.0 * k,
            0.0,
            false,
        ));
    }
    v.push(texto(
        "LAMINA",
        p(caja_num + 15.0, y0 + mh - 6.0),
        2.5 * k,
        0.0,
        true,
    ));
    let num = if datos.lamina.is_empty() {
        "A-01"
    } else {
        datos.lamina.as_str()
    };
    v.push(texto(
        num,
        p(caja_num + 15.0, y0 + mh / 2.0 - 4.0),
        7.0 * k,
        0.0,
        true,
    ));
    v
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::formas;

    #[test]
    fn letras_de_ejes_sin_i_ni_o() {
        assert_eq!(letra_de_eje(0), "A");
        assert_eq!(letra_de_eje(7), "H");
        assert_eq!(letra_de_eje(8), "J");
        assert_eq!(letra_de_eje(23), "Z");
        assert_eq!(letra_de_eje(24), "AA");
        assert_eq!(letra_de_eje(25), "AB");
    }

    #[test]
    fn un_muro_recto_da_dos_caras_y_dos_tapas() {
        let v = muro(&[[0.0, 0.0], [1000.0, 0.0]], 150.0, false);
        assert_eq!(v.len(), 4);
        let EntityType::LwPolyline(izq) = &v[0] else {
            panic!()
        };
        assert!((izq.vertices[0].location.y - 75.0).abs() < 1e-9);
    }

    #[test]
    fn el_inglete_de_un_muro_en_l() {
        let v = desplazar_polilinea(&[[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]], 1.0, false).unwrap();
        // A la izquierda del avance: por dentro de la esquina.
        assert!(geo::dist(v[1], [9.0, 1.0]) < 1e-9, "{v:?}");
        let fuera =
            desplazar_polilinea(&[[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]], -1.0, false).unwrap();
        assert!(geo::dist(fuera[1], [11.0, -1.0]) < 1e-9);
    }

    #[test]
    fn malla_de_tres_por_dos() {
        let v = ejes([0.0, 0.0], &[5000.0, 4000.0], &[6000.0], 400.0);
        // 3 verticales y 2 horizontales, cada uno con raya, burbuja y texto.
        assert_eq!(v.len(), 15);
        let textos: Vec<String> = v
            .iter()
            .filter_map(|e| match e {
                EntityType::Text(t) => Some(t.value.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(textos, ["1", "2", "3", "A", "B"]);
    }

    #[test]
    fn la_puerta_gira_un_cuarto_de_vuelta() {
        let v = puerta([0.0, 0.0], [1.0, 0.0], 900.0, true);
        let EntityType::Arc(a) = &v[1] else { panic!() };
        assert!((a.radius - 900.0).abs() < 1e-9);
        assert!((a.sweep_angle() - FRAC_PI_2).abs() < 1e-9);
        let EntityType::Line(hoja) = &v[0] else {
            panic!()
        };
        assert!((hoja.end.y - 900.0).abs() < 1e-9);
    }

    #[test]
    fn la_cota_mide_y_se_lee() {
        let v = cota([0.0, 0.0], [3000.0, 0.0], [1500.0, -500.0], 100.0, 0);
        let t = v
            .iter()
            .find_map(|e| match e {
                EntityType::Text(t) => Some(t.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(t.value, "3000");
        assert!(t.rotation.abs() < 1e-9);
        // De derecha a izquierda, el texto no queda boca abajo.
        let w = cota([3000.0, 0.0], [0.0, 0.0], [1500.0, 500.0], 100.0, 0);
        let EntityType::Text(t2) = w.last().unwrap() else {
            panic!()
        };
        assert!(t2.rotation.cos() > 0.0);
    }

    #[test]
    fn la_lamina_a1_a_escala_50() {
        let (w, h) = formato("a1").unwrap();
        let k = unidades_por_mm(50.0, 4);
        let v = lamina([0.0, 0.0], w, h, k, 50.0, &Membrete::default());
        let caja = formas::caja(&v[0], &formas::formas(&v[0])).unwrap();
        assert!((caja[2] - 841.0 * 50.0).abs() < 1e-6);
        // En metros, la misma lamina mide 42,05 m.
        assert!((unidades_por_mm(50.0, 6) * 841.0 - 42.05).abs() < 1e-9);
        assert!(formato("A7").is_none());
    }
}
