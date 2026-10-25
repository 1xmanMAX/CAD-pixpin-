//! **Las propiedades de lo elegido**, para verlas y cambiarlas desde el
//! panel (como la paleta de Propiedades de AutoCAD): capa, color, tipo de
//! linea y grosor, lo que hay de cada tipo, y lo que mide todo junto. Cada
//! cambio es una orden que se deshace de una vez.

use opencadcodec::{Color, EntityType, Handle, LineType, LineWeight};

use crate::dibujo::Dibujo;
use crate::formas;

/// **Lo que tienen en comun** varias entidades: un valor o «varios».
#[derive(Debug, Clone, PartialEq)]
pub enum Comun<T> {
    Uno(T),
    Varios,
}

impl<T: PartialEq + Clone> Comun<T> {
    /// Junta `v` con lo de antes (`None`: aun no habia nada).
    fn juntar(antes: Option<Comun<T>>, v: &T) -> Comun<T> {
        match antes {
            None => Comun::Uno(v.clone()),
            Some(Comun::Uno(a)) if a == *v => Comun::Uno(a),
            Some(_) => Comun::Varios,
        }
    }
}

/// **El resumen de lo elegido.**
#[derive(Debug, Clone, PartialEq)]
pub struct Resumen {
    /// Cuantas entidades (de las elegidas que siguen en el dibujo).
    pub cuantos: usize,
    /// Cuantas hay de cada tipo, de la mas comun a la menos.
    pub por_tipo: Vec<(&'static str, usize)>,
    pub capa: Option<Comun<String>>,
    pub color: Option<Comun<Color>>,
    /// El nombre del tipo de linea (vacio: «PorCapa»).
    pub tipo_linea: Option<Comun<String>>,
    pub grosor: Option<Comun<LineWeight>>,
    /// La longitud de todo junto y el area de lo cerrado.
    pub largo: f64,
    pub area: f64,
}

/// **Resume** las entidades `sel` del dibujo.
pub fn resumir(d: &Dibujo, sel: &[Handle]) -> Resumen {
    let mut r = Resumen {
        cuantos: 0,
        por_tipo: Vec::new(),
        capa: None,
        color: None,
        tipo_linea: None,
        grosor: None,
        largo: 0.0,
        area: 0.0,
    };
    let mut tipos: Vec<(&'static str, usize)> = Vec::new();
    for h in sel {
        let Some(e) = d.entidad(*h) else { continue };
        r.cuantos += 1;
        let c = e.common();
        r.capa = Some(Comun::juntar(r.capa.take(), &c.layer));
        r.color = Some(Comun::juntar(r.color.take(), &c.color));
        let lt = if c.linetype.eq_ignore_ascii_case("BYLAYER") {
            String::new()
        } else {
            c.linetype.clone()
        };
        r.tipo_linea = Some(Comun::juntar(r.tipo_linea.take(), &lt));
        r.grosor = Some(Comun::juntar(r.grosor.take(), &c.line_weight));
        let t = nombre_de_tipo(e);
        match tipos.iter_mut().find(|(n, _)| *n == t) {
            Some((_, k)) => *k += 1,
            None => tipos.push((t, 1)),
        }
        let (l, a) = formas::largo_y_area(e);
        r.largo += l;
        r.area += a.unwrap_or(0.0);
    }
    tipos.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    r.por_tipo = tipos;
    r
}

/// **El nombre en castellano** del tipo de una entidad.
pub fn nombre_de_tipo(e: &EntityType) -> &'static str {
    match e {
        EntityType::Line(_) => "Línea",
        EntityType::Circle(_) => "Círculo",
        EntityType::Arc(_) => "Arco",
        EntityType::LwPolyline(_) => "Polilínea",
        EntityType::Text(_) => "Texto",
        EntityType::MText(_) => "Texto de párrafo",
        EntityType::Insert(_) => "Bloque",
        EntityType::Dimension(_) => "Cota",
        EntityType::Hatch(_) => "Sombreado",
        EntityType::Point(_) => "Punto",
        EntityType::Solid(_) => "Sólido 2D",
        EntityType::Spline(_) => "Spline",
        EntityType::Ellipse(_) => "Elipse",
        EntityType::RasterImage(_) => "Imagen",
        EntityType::Mesh(_) | EntityType::PolyfaceMesh(_) => "Malla 3D",
        EntityType::Face3D(_) => "Cara 3D",
        _ => "Objeto",
    }
}

/// **Un cambio** de propiedad para todo lo elegido.
#[derive(Debug, Clone, PartialEq)]
pub enum Cambio {
    Capa(String),
    Color(Color),
    /// El nombre del tipo de linea (vacio o «PorCapa»: el de la capa).
    TipoLinea(String),
    Grosor(LineWeight),
}

/// **Cambia** la propiedad de las entidades `sel`, en un solo paso de
/// deshacer. Crea la capa o el tipo de linea si no estan. Dice cuantas
/// cambiaron.
pub fn aplicar(d: &mut Dibujo, sel: &[Handle], cambio: &Cambio) -> usize {
    match cambio {
        Cambio::Capa(c) => d.asegurar_capa(c, crate::especialidad::color_de_capa(c)),
        Cambio::TipoLinea(n) => asegurar_tipo_linea(d, n),
        _ => {}
    }
    let capa = match cambio {
        Cambio::Capa(c) => d.doc.layers.get(c).map(|l| l.handle),
        _ => None,
    };
    let (tipo, tipo_handle) = match cambio {
        Cambio::TipoLinea(n) if es_por_capa(n) => ("BYLAYER".to_string(), None),
        Cambio::TipoLinea(n) => (
            n.clone(),
            d.doc
                .line_types
                .iter()
                .find(|t| t.name.eq_ignore_ascii_case(n))
                .map(|t| t.handle),
        ),
        _ => (String::new(), None),
    };
    d.editar("PROPIEDADES", |t| {
        let mut n = 0;
        for h in sel {
            let hecho = t.cambiar(*h, |e| {
                let c = e.common_mut();
                match cambio {
                    Cambio::Capa(nombre) => {
                        c.layer = nombre.clone();
                        c.layer_handle = capa;
                    }
                    Cambio::Color(col) => c.color = *col,
                    Cambio::TipoLinea(_) => {
                        c.linetype = tipo.clone();
                        c.linetype_handle = tipo_handle;
                    }
                    Cambio::Grosor(g) => c.line_weight = *g,
                }
            });
            n += usize::from(hecho);
        }
        n
    })
}

fn es_por_capa(n: &str) -> bool {
    n.is_empty() || n.eq_ignore_ascii_case("BYLAYER") || n.eq_ignore_ascii_case("PORCAPA")
}

/// **Los tipos de linea de siempre** (nombre, descripcion y trazos en
/// unidades del dibujo a escala 1: positivo trazo, negativo hueco).
pub const TIPOS_DE_LINEA: &[(&str, &str, &[f64])] = &[
    ("CONTINUOUS", "Continua", &[]),
    ("DASHED", "Trazos __ __ __", &[12.7, -6.35]),
    ("HIDDEN", "Oculta _ _ _ _", &[6.35, -3.175]),
    ("CENTER", "Eje ____ _ ____", &[31.75, -6.35, 6.35, -6.35]),
    ("DOT", "Puntos . . . .", &[0.0, -6.35]),
    (
        "PHANTOM",
        "Fantasma ___ _ _ ___",
        &[31.75, -6.35, 6.35, -6.35, 6.35, -6.35],
    ),
];

/// Los tipos de linea que se ofrecen: los del dibujo y los de siempre, sin
/// repetir (sin «ByLayer» ni «ByBlock», que van aparte).
pub fn tipos_de_linea(d: &Dibujo) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    let nombres = TIPOS_DE_LINEA
        .iter()
        .map(|(n, _, _)| n.to_string())
        .chain(d.doc.line_types.iter().map(|t| t.name.clone()));
    for n in nombres {
        let up = n.to_uppercase();
        if up == "BYLAYER" || up == "BYBLOCK" || n.is_empty() {
            continue;
        }
        if !v.iter().any(|x| x.eq_ignore_ascii_case(&n)) {
            v.push(n);
        }
    }
    v
}

/// Pone el tipo de linea `n` en la tabla del dibujo si no esta (de los de
/// siempre; uno desconocido se crea continuo).
pub fn asegurar_tipo_linea(d: &mut Dibujo, n: &str) {
    if es_por_capa(n)
        || d.doc
            .line_types
            .iter()
            .any(|t| t.name.eq_ignore_ascii_case(n))
    {
        return;
    }
    let mut lt = LineType::new(n.to_uppercase());
    if let Some((_, desc, trazos)) = TIPOS_DE_LINEA
        .iter()
        .find(|(m, _, _)| m.eq_ignore_ascii_case(n))
    {
        lt.description = desc.to_string();
        for t in *trazos {
            lt.add_element(if *t > 0.0 {
                opencadcodec::tables::LineTypeElement::dash(*t)
            } else if *t < 0.0 {
                opencadcodec::tables::LineTypeElement::space(-*t)
            } else {
                opencadcodec::tables::LineTypeElement::dot()
            });
        }
    }
    let _ = d.doc.line_types.add(lt);
    d.tocar();
}

/// **Los grosores de linea** que se ofrecen, en centesimas de milimetro.
pub const GROSORES: &[i16] = &[
    0, 5, 9, 13, 15, 18, 20, 25, 30, 35, 40, 50, 53, 60, 70, 80, 90, 100, 106, 120, 140, 158, 200,
    211,
];

/// Como se dice un grosor.
pub fn texto_de_grosor(g: &LineWeight) -> String {
    match g {
        LineWeight::ByLayer => "PorCapa".into(),
        LineWeight::ByBlock => "PorBloque".into(),
        LineWeight::Default => "Por defecto".into(),
        LineWeight::Value(v) => format!("{:.2} mm", *v as f64 / 100.0),
    }
}

/// Como se dice un color (el indice con su nombre, si lo tiene).
pub fn texto_de_color(c: &Color) -> String {
    match c {
        Color::ByLayer => "PorCapa".into(),
        Color::ByBlock => "PorBloque".into(),
        Color::None => "Ninguno".into(),
        Color::Index(i) => match nombre_aci(*i) {
            Some(n) => format!("{i} {n}"),
            None => format!("Color {i}"),
        },
        Color::Rgb { r, g, b } => format!("{r},{g},{b}"),
    }
}

/// Como se dice un tipo de linea (vacio: «PorCapa»).
pub fn texto_de_tipo_linea(n: &str) -> String {
    if es_por_capa(n) {
        "PorCapa".into()
    } else if n.eq_ignore_ascii_case("BYBLOCK") {
        "PorBloque".into()
    } else {
        n.to_string()
    }
}

fn nombre_aci(i: u8) -> Option<&'static str> {
    Some(match i {
        1 => "Rojo",
        2 => "Amarillo",
        3 => "Verde",
        4 => "Cian",
        5 => "Azul",
        6 => "Magenta",
        7 => "Blanco",
        8 => "Gris",
        9 => "Gris claro",
        _ => return None,
    })
}

/// **El color de un indice ACI** (1 a 255) en RGB, como la paleta de
/// AutoCAD: los nueve primeros, 24 tonos cada 15° en cinco brillos (los
/// impares, pastel) y seis grises.
pub fn aci_rgb(i: u8) -> (u8, u8, u8) {
    match i {
        0 => (0, 0, 0),
        1 => (255, 0, 0),
        2 => (255, 255, 0),
        3 => (0, 255, 0),
        4 => (0, 255, 255),
        5 => (0, 0, 255),
        6 => (255, 0, 255),
        7 => (255, 255, 255),
        8 => (128, 128, 128),
        9 => (192, 192, 192),
        10..=249 => {
            let tono = ((i - 10) / 10) as f64 * 15.0;
            let k = (i % 10) as usize;
            let v = [255.0, 204.0, 153.0, 127.0, 76.0][k / 2];
            let s = if k % 2 == 0 { 1.0 } else { 0.5 };
            hsv(tono, s, v)
        }
        _ => {
            let g = [51, 91, 132, 173, 214, 255][(i - 250) as usize];
            (g, g, g)
        }
    }
}

/// De tono (grados), saturacion (0 a 1) y brillo (0 a 255) a RGB.
fn hsv(tono: f64, s: f64, v: f64) -> (u8, u8, u8) {
    let canal = |n: f64| {
        let k = (n + tono / 60.0) % 6.0;
        let f = (k.min(4.0 - k)).clamp(0.0, 1.0);
        (v * (1.0 - s * f)).round() as u8
    };
    (canal(5.0), canal(3.0), canal(1.0))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::especialidad as esp;

    fn dibujo() -> (Dibujo, Vec<Handle>) {
        let mut d = Dibujo::nuevo();
        let hs = d.editar("x", |t| {
            vec![
                t.anadir_en(esp::linea([0.0, 0.0], [10.0, 0.0]), "0")
                    .unwrap(),
                t.anadir_en(esp::linea([0.0, 5.0], [10.0, 5.0]), "0")
                    .unwrap(),
                t.anadir_en(esp::circulo([0.0, 0.0], 1.0), "A-MUROS")
                    .unwrap(),
            ]
        });
        (d, hs)
    }

    #[test]
    fn el_resumen_junta_lo_comun_y_cuenta_por_tipo() {
        let (d, hs) = dibujo();
        let r = resumir(&d, &hs);
        assert_eq!(r.cuantos, 3);
        assert_eq!(r.por_tipo, vec![("Línea", 2), ("Círculo", 1)]);
        assert_eq!(r.capa, Some(Comun::Varios));
        assert_eq!(r.color, Some(Comun::Uno(Color::ByLayer)));
        assert!((r.largo - (20.0 + std::f64::consts::TAU)).abs() < 1e-9);
        let r = resumir(&d, &hs[..2]);
        assert_eq!(r.capa, Some(Comun::Uno("0".to_string())));
        // Nada elegido (o ya borrado): sin valores.
        let r = resumir(&d, &[]);
        assert_eq!((r.cuantos, r.capa), (0, None));
    }

    #[test]
    fn cambiar_color_tipo_grosor_y_capa_se_deshace_de_una_vez() {
        let (mut d, hs) = dibujo();
        assert_eq!(aplicar(&mut d, &hs, &Cambio::Color(Color::Index(1))), 3);
        assert_eq!(aplicar(&mut d, &hs, &Cambio::TipoLinea("DASHED".into())), 3);
        assert!(d.doc.line_types.iter().any(|t| t.name == "DASHED"));
        aplicar(&mut d, &hs, &Cambio::Grosor(LineWeight::Value(50)));
        aplicar(&mut d, &hs[..1], &Cambio::Capa("NUEVA".into()));
        let r = resumir(&d, &hs);
        assert_eq!(r.color, Some(Comun::Uno(Color::Index(1))));
        assert_eq!(r.tipo_linea, Some(Comun::Uno("DASHED".into())));
        assert_eq!(r.grosor, Some(Comun::Uno(LineWeight::Value(50))));
        assert!(d.doc.layers.contains("NUEVA"));
        assert_eq!(d.entidad(hs[0]).unwrap().common().layer, "NUEVA");
        // Volver a «PorCapa».
        aplicar(&mut d, &hs, &Cambio::TipoLinea("PorCapa".into()));
        assert_eq!(resumir(&d, &hs).tipo_linea, Some(Comun::Uno(String::new())));
        // Cada cambio, un paso.
        d.deshacer();
        d.deshacer();
        d.deshacer();
        assert_eq!(
            resumir(&d, &hs).grosor,
            Some(Comun::Uno(LineWeight::ByLayer))
        );
        // Lo mismo otra vez no deja paso.
        let antes = d.version();
        aplicar(&mut d, &hs, &Cambio::Color(Color::Index(1)));
        assert_eq!(d.version(), antes);
    }

    #[test]
    fn la_paleta_aci_como_la_de_autocad() {
        assert_eq!(aci_rgb(1), (255, 0, 0));
        assert_eq!(aci_rgb(10), (255, 0, 0));
        assert_eq!(aci_rgb(11), (255, 128, 128));
        assert_eq!(aci_rgb(30), (255, 128, 0));
        assert_eq!(aci_rgb(130), (0, 255, 255));
        assert_eq!(aci_rgb(12), (204, 0, 0));
        assert_eq!(aci_rgb(250), (51, 51, 51));
        assert_eq!(aci_rgb(255), (255, 255, 255));
        assert_eq!(texto_de_color(&Color::Index(3)), "3 Verde");
        assert_eq!(texto_de_grosor(&LineWeight::Value(25)), "0.25 mm");
    }

    #[test]
    fn los_tipos_de_linea_sin_repetir() {
        let (mut d, _) = dibujo();
        asegurar_tipo_linea(&mut d, "hidden");
        let v = tipos_de_linea(&d);
        assert_eq!(
            v.iter()
                .filter(|n| n.eq_ignore_ascii_case("HIDDEN"))
                .count(),
            1
        );
        assert!(v.iter().any(|n| n == "CONTINUOUS"));
        assert!(!v.iter().any(|n| n.eq_ignore_ascii_case("BYLAYER")));
    }
}
