//! **Los formatos 2D de intercambio**: importar y exportar PDF y SVG,
//! adjuntar imagenes, guardar en otra version de DWG/DXF y escribir solo lo
//! elegido a otro plano. Rust puro (se prueba en Linux); las ordenes estan
//! en [`crate::herramientas::formatos`].
//!
//! Lo importado (PDF o SVG) pasa primero por un modelo comun ([`Importado`],
//! en milimetros de papel con la y hacia arriba) y de ahi a entidades con
//! [`a_entidades`]; lo exportado sale de una [`escena::Escena`] (lo visible
//! del dibujo ya resuelto en rayas, rellenos y textos con su color y grosor).

use opencadcodec::entities::LwPolyline;
use opencadcodec::{Color, EntityType, LineWeight, Vector2};

use crate::especialidad as esp;
use crate::geo::{self, P};
use crate::patrones::{self, Anillo, Patron};

pub mod escena;
pub mod imagen;
pub mod pdf;
pub mod shx;
pub mod svg;
pub mod version;
pub mod wbloque;

/// Un color de 8 bits por canal.
pub type Rgb = [u8; 3];

/// **Una pieza de geometria importada**, en mm de papel.
#[derive(Debug, Clone, PartialEq)]
pub enum Geometria {
    Polilinea { puntos: Vec<P>, cerrada: bool },
    Circulo { c: P, r: f64 },
}

/// **Un camino importado**: sus piezas y como se pinta.
#[derive(Debug, Clone, PartialEq)]
pub struct Figura {
    pub partes: Vec<Geometria>,
    /// El color y el grosor (mm) del contorno, si se traza.
    pub trazo: Option<(Rgb, f64)>,
    /// El color del relleno, si se rellena.
    pub relleno: Option<Rgb>,
    /// La capa de origen (un grupo de contenido opcional del PDF, una capa
    /// de Inkscape), si la hay.
    pub grupo: Option<String>,
}

/// **Un texto importado** (posicion de la linea base, en mm de papel).
#[derive(Debug, Clone, PartialEq)]
pub struct Rotulo {
    pub texto: String,
    pub p: P,
    /// La altura de las mayusculas.
    pub alto: f64,
    /// En radianes.
    pub giro: f64,
    /// El factor de ancho (1 normal).
    pub ancho: f64,
    pub color: Rgb,
    pub grupo: Option<String>,
}

/// **Lo leido de un PDF o un SVG**, listo para pasar a entidades.
#[derive(Debug, Clone, Default)]
pub struct Importado {
    pub figuras: Vec<Figura>,
    pub rotulos: Vec<Rotulo>,
    /// Lo que no se pudo traer (imagenes, textos sin decodificar…).
    pub avisos: Vec<String>,
    /// Los grupos que el fichero tiene apagados (sus capas quedan apagadas).
    pub apagados: Vec<String>,
    /// El tamano de la pagina en mm (ancho, alto).
    pub pagina: (f64, f64),
}

/// Las capas que crea una importacion: nombre, color y si queda apagada.
#[derive(Debug, Clone, PartialEq)]
pub struct CapaNueva {
    pub nombre: String,
    pub color: Color,
    pub apagada: bool,
}

/// Lo que sale de [`a_entidades`]: cada entidad con su capa, y las capas.
#[derive(Debug, Clone, Default)]
pub struct Entidades {
    pub entidades: Vec<(EntityType, String)>,
    pub capas: Vec<CapaNueva>,
}

/// **Un nombre valido de capa** (sin `<>/\":;?*|=,` ni de mas de 200 letras).
pub fn sanear_capa(nombre: &str) -> String {
    let limpio: String = nombre
        .trim()
        .chars()
        .map(|c| {
            if "<>/\\\":;?*|=,`".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .take(200)
        .collect();
    if limpio.is_empty() {
        "SIN_NOMBRE".into()
    } else {
        limpio
    }
}

/// El color de un RGB para el dibujo: el negro pasa al 7 (blanco o negro
/// segun el fondo, como en todo CAD), un color exacto del indice al indice.
pub fn color_de(c: Rgb) -> Color {
    if c == [0, 0, 0] {
        return Color::Index(7);
    }
    for i in 1..=255u8 {
        if i != 7 && opencadcodec::types::aci_table::aci_to_rgb(i) == Some((c[0], c[1], c[2])) {
            return Color::Index(i);
        }
    }
    Color::Rgb {
        r: c[0],
        g: c[1],
        b: c[2],
    }
}

/// Los grosores de linea de AutoCAD (centesimas de mm).
const GROSORES: [i16; 24] = [
    0, 5, 9, 13, 15, 18, 20, 25, 30, 35, 40, 50, 53, 60, 70, 80, 90, 100, 106, 120, 140, 158, 200,
    211,
];

/// **El grosor estandar mas cercano** a `mm` (de papel).
pub fn grosor_de(mm: f64) -> LineWeight {
    if !mm.is_finite() || mm <= 0.0 {
        return LineWeight::ByLayer;
    }
    let c = (mm * 100.0).round();
    let g = GROSORES
        .iter()
        .copied()
        .min_by(|a, b| {
            (*a as f64 - c)
                .abs()
                .partial_cmp(&(*b as f64 - c).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(25);
    LineWeight::Value(g)
}

/// Quita los puntos repetidos seguidos.
pub fn sin_repetidos(v: &mut Vec<P>, tol: f64) {
    v.dedup_by(|b, a| geo::dist(*a, *b) <= tol);
}

/// **Si una polilinea cerrada es un circulo** (como los cuatro Bezier de un
/// PDF): su centro y radio.
pub fn es_circulo(puntos: &[P]) -> Option<(P, f64)> {
    let mut v: Vec<P> = puntos.to_vec();
    if v.len() > 2 && geo::dist(v[0], v[v.len() - 1]) < 1e-9 {
        v.pop();
    }
    if v.len() < 8 {
        return None;
    }
    let n = v.len() as f64;
    let c = [
        v.iter().map(|p| p[0]).sum::<f64>() / n,
        v.iter().map(|p| p[1]).sum::<f64>() / n,
    ];
    let r = v.iter().map(|p| geo::dist(*p, c)).sum::<f64>() / n;
    if r < 1e-9
        || v.iter()
            .any(|p| (geo::dist(*p, c) - r).abs() > r * 0.005 + 1e-9)
    {
        return None;
    }
    // Que de la vuelta entera, sin saltos grandes y en un solo sentido.
    let mut total = 0.0;
    for i in 0..v.len() {
        let (a, b) = (v[i], v[(i + 1) % v.len()]);
        let mut d = geo::angulo(c, b) - geo::angulo(c, a);
        while d > std::f64::consts::PI {
            d -= std::f64::consts::TAU;
        }
        while d < -std::f64::consts::PI {
            d += std::f64::consts::TAU;
        }
        if d.abs() > 100f64.to_radians() {
            return None;
        }
        total += d;
    }
    ((total.abs() - std::f64::consts::TAU).abs() < 0.05).then_some((c, r))
}

/// **Los puntos de un Bezier cubico** de `a` a `d` (sin `a`), con un error
/// de cuerda menor que `tol`.
pub fn bezier(a: P, b: P, c: P, d: P, tol: f64) -> Vec<P> {
    let dd = |p: P, q: P, r: P| geo::norma([p[0] - 2.0 * q[0] + r[0], p[1] - 2.0 * q[1] + r[1]]);
    let m = dd(a, b, c).max(dd(b, c, d));
    let n = ((0.75 * m / tol.max(1e-9)).sqrt().ceil() as usize).clamp(1, 64);
    (1..=n)
        .map(|i| {
            let t = i as f64 / n as f64;
            let u = 1.0 - t;
            let (k0, k1, k2, k3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            [
                k0 * a[0] + k1 * b[0] + k2 * c[0] + k3 * d[0],
                k0 * a[1] + k1 * b[1] + k2 * c[1] + k3 * d[1],
            ]
        })
        .collect()
}

/// El texto de una entidad sin los codigos de AutoCAD (`%%c`, `%%d`, `%%p`,
/// `\U+00F1`).
pub fn texto_limpio(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < c.len() {
        if c[i] == '%' && c.get(i + 1) == Some(&'%') && i + 2 < c.len() {
            match c[i + 2].to_ascii_lowercase() {
                'c' => out.push('Ø'),
                'd' => out.push('°'),
                'p' => out.push('±'),
                '%' => out.push('%'),
                'u' | 'o' => {}
                otro => {
                    out.push_str("%%");
                    out.push(otro);
                }
            }
            i += 3;
            continue;
        }
        if c[i] == '\\'
            && matches!(c.get(i + 1), Some('U') | Some('u'))
            && c.get(i + 2) == Some(&'+')
            && i + 7 <= c.len()
        {
            let hex: String = c[i + 3..i + 7].iter().collect();
            if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                out.push(ch);
                i += 7;
                continue;
            }
        }
        out.push(c[i]);
        i += 1;
    }
    out
}

/// **Pasa lo importado a entidades** con la esquina de la pagina en
/// `origen` y `k` unidades del dibujo por mm de papel. Las capas: `prefijo`
/// mas el grupo de origen o, si no hay, mas el color (`PDF_FF0000`).
pub fn a_entidades(imp: &Importado, prefijo: &str, origen: P, k: f64) -> Entidades {
    let mut out = Entidades::default();
    let lleva = |p: P| [origen[0] + p[0] * k, origen[1] + p[1] * k];
    let capa = |grupo: &Option<String>, color: Rgb, out: &mut Entidades| -> (String, Color) {
        let (nombre, color_capa, color_ent, apagada) = match grupo {
            Some(g) => (
                sanear_capa(&format!("{prefijo}{g}")),
                Color::Index(7),
                color_de(color),
                imp.apagados.contains(g),
            ),
            None => (
                format!("{prefijo}{:02X}{:02X}{:02X}", color[0], color[1], color[2]),
                color_de(color),
                Color::ByLayer,
                false,
            ),
        };
        if !out.capas.iter().any(|c| c.nombre == nombre) {
            out.capas.push(CapaNueva {
                nombre: nombre.clone(),
                color: color_capa,
                apagada,
            });
        }
        (nombre, color_ent)
    };
    for f in &imp.figuras {
        if let Some((color, mm)) = f.trazo {
            let (nombre, c) = capa(&f.grupo, color, &mut out);
            for g in &f.partes {
                let Some(mut e) = geometria_a_entidad(g, &lleva, k) else {
                    continue;
                };
                e.common_mut().color = c;
                e.common_mut().line_weight = grosor_de(mm);
                out.entidades.push((e, nombre.clone()));
            }
        }
        if let Some(color) = f.relleno {
            let anillos: Vec<Anillo> = f
                .partes
                .iter()
                .filter_map(|g| match g {
                    Geometria::Circulo { c, r } => Some(patrones::anillo_circulo(lleva(*c), r * k)),
                    Geometria::Polilinea { puntos, .. } if puntos.len() >= 3 => {
                        Some(puntos.iter().map(|p| (lleva(*p), 0.0)).collect())
                    }
                    _ => None,
                })
                .collect();
            if anillos.is_empty() {
                continue;
            }
            let (nombre, c) = capa(&f.grupo, color, &mut out);
            let mut h = patrones::sombreado(&anillos, &[], Patron::Solido, 1.0, 0.0, None);
            h.common_mut().color = c;
            out.entidades.push((h, nombre));
        }
    }
    for r in &imp.rotulos {
        let (nombre, c) = capa(&r.grupo, r.color, &mut out);
        let mut e = esp::texto(&r.texto, lleva(r.p), r.alto * k, r.giro, false);
        if let EntityType::Text(t) = &mut e
            && (r.ancho - 1.0).abs() > 0.01
            && r.ancho.is_finite()
            && r.ancho > 0.0
        {
            t.width_factor = r.ancho;
        }
        e.common_mut().color = c;
        out.entidades.push((e, nombre));
    }
    out
}

fn geometria_a_entidad(g: &Geometria, lleva: &impl Fn(P) -> P, k: f64) -> Option<EntityType> {
    match g {
        Geometria::Circulo { c, r } => Some(esp::circulo(lleva(*c), r * k)),
        Geometria::Polilinea { puntos, cerrada } => {
            let mut v: Vec<P> = puntos.iter().map(|p| lleva(*p)).collect();
            sin_repetidos(&mut v, 1e-9 * k.max(1.0));
            let cerrada = *cerrada || (v.len() > 3 && geo::dist(v[0], v[v.len() - 1]) < 1e-9);
            if cerrada && v.len() > 3 && geo::dist(v[0], v[v.len() - 1]) < 1e-9 {
                v.pop();
            }
            if cerrada && let Some((c, r)) = es_circulo(&v) {
                return Some(esp::circulo(c, r));
            }
            match v.len() {
                0 | 1 => None,
                2 if !cerrada => Some(esp::linea(v[0], v[1])),
                _ => {
                    let mut p = LwPolyline::from_points(
                        v.iter().map(|q| Vector2::new(q[0], q[1])).collect(),
                    );
                    p.is_closed = cerrada && v.len() > 2;
                    Some(EntityType::LwPolyline(p))
                }
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn capas_validas_colores_y_grosores() {
        assert_eq!(sanear_capa(" a/b:c "), "a_b_c");
        assert_eq!(sanear_capa(""), "SIN_NOMBRE");
        assert_eq!(color_de([0, 0, 0]), Color::Index(7));
        assert_eq!(color_de([255, 0, 0]), Color::Index(1));
        assert!(color_de([12, 34, 56]).is_true_color());
        assert_eq!(grosor_de(0.26), LineWeight::Value(25));
        assert_eq!(grosor_de(0.0), LineWeight::ByLayer);
        assert_eq!(grosor_de(5.0), LineWeight::Value(211));
    }

    #[test]
    fn un_circulo_aproximado_se_reconoce() {
        let v: Vec<P> = (0..36)
            .map(|i| geo::polar([5.0, 5.0], 2.0, i as f64 * 10f64.to_radians()))
            .collect();
        let (c, r) = es_circulo(&v).unwrap();
        assert!(geo::dist(c, [5.0, 5.0]) < 1e-6 && (r - 2.0).abs() < 1e-6);
        // Medio circulo no.
        assert!(es_circulo(&v[..18]).is_none());
        // Un cuadrado no.
        assert!(es_circulo(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]).is_none());
    }

    #[test]
    fn el_bezier_acaba_en_su_punto_y_es_fino() {
        let v = bezier([0.0, 0.0], [0.0, 10.0], [10.0, 10.0], [10.0, 0.0], 0.01);
        assert!(v.len() > 8);
        assert_eq!(*v.last().unwrap(), [10.0, 0.0]);
        // Una recta da un solo tramo.
        assert_eq!(
            bezier([0.0, 0.0], [1.0, 0.0], [2.0, 0.0], [3.0, 0.0], 0.01).len(),
            1
        );
    }

    #[test]
    fn codigos_de_texto() {
        assert_eq!(texto_limpio("%%c50 %%d %%p1"), "Ø50 ° ±1");
        assert_eq!(texto_limpio("a\\U+00F1o"), "año");
        assert_eq!(texto_limpio("100%"), "100%");
    }

    #[test]
    fn lo_importado_pasa_a_capas_por_color_y_por_grupo() {
        let imp = Importado {
            figuras: vec![
                Figura {
                    partes: vec![Geometria::Polilinea {
                        puntos: vec![[0.0, 0.0], [10.0, 0.0]],
                        cerrada: false,
                    }],
                    trazo: Some(([255, 0, 0], 0.35)),
                    relleno: None,
                    grupo: None,
                },
                Figura {
                    partes: vec![Geometria::Polilinea {
                        puntos: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]],
                        cerrada: true,
                    }],
                    trazo: None,
                    relleno: Some([0, 0, 255]),
                    grupo: Some("Muros".into()),
                },
            ],
            rotulos: vec![Rotulo {
                texto: "Hola".into(),
                p: [1.0, 1.0],
                alto: 2.5,
                giro: 0.0,
                ancho: 1.0,
                color: [0, 0, 0],
                grupo: None,
            }],
            apagados: vec!["Muros".into()],
            ..Default::default()
        };
        let r = a_entidades(&imp, "PDF_", [100.0, 0.0], 2.0);
        assert_eq!(r.entidades.len(), 3);
        let EntityType::Line(l) = &r.entidades[0].0 else {
            panic!()
        };
        assert_eq!((l.start.x, l.end.x), (100.0, 120.0));
        assert_eq!(r.entidades[0].1, "PDF_FF0000");
        assert_eq!(r.entidades[0].0.common().line_weight, LineWeight::Value(35));
        assert!(matches!(r.entidades[1].0, EntityType::Hatch(_)));
        assert_eq!(r.entidades[1].1, "PDF_Muros");
        assert!(r.capas.iter().any(|c| c.nombre == "PDF_Muros" && c.apagada));
        let EntityType::Text(t) = &r.entidades[2].0 else {
            panic!()
        };
        assert_eq!(t.height, 5.0);
        assert_eq!(r.entidades[2].1, "PDF_000000");
    }
}
