//! **Del DWG a rayas y triangulos** ([`Modelo`]): se recorre el espacio
//! modelo, se despliegan los bloques (con su punto base, escala, giro y
//! filas y columnas de MINSERT), se trocean las curvas y se resuelven los
//! colores (por capa, por bloque, el 7 que cambia con el fondo).
//!
//! Hecho a partir de como lo cuenta OpenCADStudio (su arquitectura, no su
//! codigo) y de la especificacion DXF: el sistema de coordenadas del objeto
//! (OCS) con su eje arbitrario, la capa «0» que hereda la del bloque, los
//! colores *ByBlock*. Donde la libreria tiene un fallo conocido (la matriz
//! del INSERT sin el punto base, el MINSERT sin girar) se hace aqui.

use std::collections::HashSet;

use opencadcodec::CadDocument;
use opencadcodec::entities::{BoundaryEdge, EntityType};
use opencadcodec::types::{Color, LineWeight, Matrix3, Vector3};

use crate::modelo::{Constructor, Modelo};
use crate::teselar;
use crate::texto::Textos;

/// Una matriz afin 3D (3x4, por filas) en `f64`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Afin {
    pub m: [[f64; 4]; 3],
}

impl Afin {
    pub const IDENTIDAD: Afin = Afin {
        m: [[1., 0., 0., 0.], [0., 1., 0., 0.], [0., 0., 1., 0.]],
    };

    pub fn traslacion(x: f64, y: f64, z: f64) -> Afin {
        let mut a = Afin::IDENTIDAD;
        a.m[0][3] = x;
        a.m[1][3] = y;
        a.m[2][3] = z;
        a
    }

    pub fn de_matriz3(r: &Matrix3) -> Afin {
        let mut a = Afin::IDENTIDAD;
        for i in 0..3 {
            for j in 0..3 {
                a.m[i][j] = r.m[i][j];
            }
        }
        a
    }

    pub fn giro_z(ang: f64) -> Afin {
        let (s, c) = ang.sin_cos();
        Afin {
            m: [[c, -s, 0., 0.], [s, c, 0., 0.], [0., 0., 1., 0.]],
        }
    }

    pub fn escala(x: f64, y: f64, z: f64) -> Afin {
        Afin {
            m: [[x, 0., 0., 0.], [0., y, 0., 0.], [0., 0., z, 0.]],
        }
    }

    /// `self · o` (primero `o`).
    pub fn por(&self, o: &Afin) -> Afin {
        let mut r = Afin::IDENTIDAD;
        for i in 0..3 {
            for j in 0..4 {
                let mut v = if j == 3 { self.m[i][3] } else { 0.0 };
                for k in 0..3 {
                    v += self.m[i][k] * o.m[k][j];
                }
                r.m[i][j] = v;
            }
        }
        r
    }

    pub fn punto(&self, x: f64, y: f64, z: f64) -> [f64; 2] {
        [
            self.m[0][0] * x + self.m[0][1] * y + self.m[0][2] * z + self.m[0][3],
            self.m[1][0] * x + self.m[1][1] * y + self.m[1][2] * z + self.m[1][3],
        ]
    }

    pub fn p3(&self, v: &Vector3) -> [f64; 2] {
        self.punto(v.x, v.y, v.z)
    }

    /// Cuanto agranda (para el tamano de lo dibujado y el de los textos).
    pub fn escala_media(&self) -> f64 {
        let a = (self.m[0][0] * self.m[0][0] + self.m[1][0] * self.m[1][0]).sqrt();
        let b = (self.m[0][1] * self.m[0][1] + self.m[1][1] * self.m[1][1]).sqrt();
        ((a * b).sqrt()).max(1e-300)
    }
}

/// **El volteo de un TEXT** (DXF 71): 2 al reves (espejo en x), 4 cabeza
/// abajo (espejo en y), alrededor de su punto de justificacion y en su giro,
/// como AutoCAD. Va en el sistema del texto (OCS). Como PixPin Android v0.111.
pub fn volteo_de_texto(
    flags: i16,
    ins: [f64; 3],
    alineado: Option<[f64; 3]>,
    h: u8,
    v: u8,
    giro: f64,
) -> Afin {
    let fx = if flags & 2 != 0 { -1.0 } else { 1.0 };
    let fy = if flags & 4 != 0 { -1.0 } else { 1.0 };
    if fx > 0.0 && fy > 0.0 {
        return Afin::IDENTIDAD;
    }
    let a = match alineado {
        Some(a) if h != 0 || v != 0 => a,
        _ => ins,
    };
    Afin::traslacion(a[0], a[1], a[2])
        .por(&Afin::giro_z(giro))
        .por(&Afin::escala(fx, fy, 1.0))
        .por(&Afin::giro_z(-giro))
        .por(&Afin::traslacion(-a[0], -a[1], -a[2]))
}

/// El OCS de una entidad con su `normal` (eje arbitrario de AutoCAD).
pub fn ocs(normal: &Vector3) -> Afin {
    let n = normal;
    let largo = (n.x * n.x + n.y * n.y + n.z * n.z).sqrt();
    if !largo.is_finite() || largo < 1e-12 || (n.x.abs() < 1e-12 && n.y.abs() < 1e-12 && n.z > 0.0)
    {
        return Afin::IDENTIDAD;
    }
    Afin::de_matriz3(&Matrix3::arbitrary_axis(*normal))
}

// --------------------------------------------------------------- colores

/// RGBA empaquetado como lo lee `R8G8B8A8_UNORM`.
pub fn rgba(r: u8, g: u8, b: u8) -> u32 {
    r as u32 | (g as u32) << 8 | (b as u32) << 16 | 0xff00_0000
}

/// El color 7 (blanco en fondo oscuro, negro en claro): alfa 0.
pub const COLOR_7: u32 = 0x00ff_ffff;

/// Como se apunta en `sin_dibujar` un objeto guardado sin su dibujo de
/// reserva (solo la caja): «sin dibujo: AeccDbSurfaceTin (AeccLand130)».
pub const PROXY_SIN_DIBUJO: &str = "sin dibujo: ";

pub(crate) fn de_color(c: &Color) -> Option<u32> {
    match c {
        Color::Index(7) => Some(COLOR_7),
        Color::Rgb { r, g, b } => Some(rgba(*r, *g, *b)),
        Color::Index(_) => c.rgb().map(|(r, g, b)| rgba(r, g, b)),
        _ => None,
    }
}

// ---------------------------------------------------------------- recorrer

/// Lo heredado al entrar en un bloque.
#[derive(Clone)]
struct Contexto<'a> {
    m: Afin,
    /// La capa del INSERT, para lo que esta en la capa «0».
    capa: Option<&'a str>,
    /// El color del INSERT ya resuelto, para lo *ByBlock*.
    color: u32,
    /// El grosor del INSERT ya resuelto (centesimas de mm; [`GROSOR_NORMAL`]),
    /// para lo *ByBlock*. Solo lo mira la conversion por capas.
    grosor: i16,
    profundidad: u32,
}

/// El grosor «por defecto» de AutoCAD (una raya fina).
pub const GROSOR_NORMAL: i16 = -3;

/// **La conversion por capas** (para exportar a HTML, 13-oct-2026): un
/// [`Constructor`] por cada capa y grosor, para poder encender y apagar las
/// capas en la pagina y pintar cada raya con su grosor. El visor no la usa:
/// a la tarjeta le da igual de que capa es cada raya.
struct PorCapas {
    actual: (String, i16),
    aparcados: std::collections::HashMap<(String, i16), Constructor>,
    imagenes: Vec<ImagenDelPlano>,
}

/// Un trozo del plano: lo de una capa con un grosor.
#[derive(Debug, Clone, PartialEq)]
pub struct Grupo {
    pub capa: String,
    /// Centesimas de milimetro ([`GROSOR_NORMAL`]: el de por defecto).
    pub grosor: i16,
    /// Si la capa esta encendida en el plano (apagada o congelada: no).
    pub encendida: bool,
    pub modelo: Modelo,
}

/// Una imagen puesta en el plano (IMAGE): el fichero que dice y donde cae.
/// Las rayas se convierten; la imagen la lee quien exporta.
#[derive(Debug, Clone, PartialEq)]
pub struct ImagenDelPlano {
    pub capa: String,
    /// La ruta tal como la guarda el plano (puede ser relativa a el).
    pub ruta: String,
    /// La esquina de abajo a la izquierda y los lados (ancho y alto
    /// enteros), en el plano.
    pub esquina: [f64; 2],
    pub u: [f64; 2],
    pub v: [f64; 2],
}

/// El plano partido por capas, con sus imagenes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlanoPorCapas {
    pub grupos: Vec<Grupo>,
    pub imagenes: Vec<ImagenDelPlano>,
}

/// Lo que da la conversion, ademas del modelo.
#[derive(Debug, Default, Clone)]
pub struct Cuentas {
    pub entidades: usize,
    pub letras: usize,
    /// Vertices por tipo de entidad (para medir).
    pub vertices_por_tipo: std::collections::BTreeMap<String, usize>,
}

pub struct Convertidor<'d> {
    doc: &'d CadDocument,
    /// `Some`: la conversion por capas (ver [`PorCapas`]).
    por_capas: Option<PorCapas>,
    c: Constructor,
    pub cuentas: Cuentas,
    textos: Textos,
    /// Los bloques que se estan desplegando (para no entrar en bucle).
    abiertos: HashSet<String>,
    copias_minsert: usize,
    /// `Some`: ademas de dibujar, apunta cada texto con su sitio (ver
    /// [`textos_del_fichero`]).
    recogidos: Option<Vec<TextoDelPlano>>,
}

/// **Un texto del plano y donde esta**, en coordenadas del dibujo (las de
/// AutoCAD, las mismas que `PIXPIN_CAD_VISTA`). Para que el asistente
/// encuentre «JR. ANTISUYO» y lleve ahi el visor (14-oct-2026).
#[derive(Debug, Clone, PartialEq)]
pub struct TextoDelPlano {
    pub texto: String,
    /// El punto de insercion.
    pub x: f64,
    pub y: f64,
    /// El alto de la letra, ya escalado por los bloques.
    pub alto: f64,
}

const PROFUNDIDAD_MAX: u32 = 32;
const MINSERT_MAX: usize = 20_000;

impl<'d> Convertidor<'d> {
    pub fn nuevo(doc: &'d CadDocument) -> Self {
        Self {
            doc,
            c: Constructor::nuevo(),
            cuentas: Cuentas::default(),
            textos: Textos::nuevo(),
            abiertos: HashSet::new(),
            copias_minsert: 0,
            por_capas: None,
            recogidos: None,
        }
    }

    /// Para [`Convertidor::convertir_por_capas`]: tambien lo de las capas
    /// apagadas, que en la pagina se pueden encender.
    pub fn por_capas(doc: &'d CadDocument) -> Self {
        let mut c = Self::nuevo(doc);
        c.por_capas = Some(PorCapas {
            actual: ("0".into(), GROSOR_NORMAL),
            aparcados: Default::default(),
            imagenes: Vec::new(),
        });
        c
    }

    /// **El plano partido por capas y grosores**, cada trozo con su modelo
    /// (cada uno con su origen). Ordenados por capa y grosor.
    pub fn convertir_por_capas(mut self) -> PlanoPorCapas {
        if self.por_capas.is_none() {
            self.por_capas = Some(PorCapas {
                actual: ("0".into(), GROSOR_NORMAL),
                aparcados: Default::default(),
                imagenes: Vec::new(),
            });
        }
        self.recorrer();
        let Some(pc) = self.por_capas.take() else {
            return PlanoPorCapas::default();
        };
        let unidades = self.c.unidades;
        let doc = self.doc;
        let mut todos: Vec<((String, i16), Constructor)> = pc.aparcados.into_iter().collect();
        todos.push((pc.actual, self.c));
        let mut grupos: Vec<Grupo> = todos
            .into_iter()
            .map(|((capa, grosor), mut c)| {
                c.unidades = unidades;
                let encendida = doc.layers.get(&capa).is_none_or(|l| l.is_visible());
                Grupo {
                    capa,
                    grosor,
                    encendida,
                    modelo: c.terminar(),
                }
            })
            .filter(|g| !g.modelo.vacio())
            .collect();
        grupos.sort_by(|a, b| {
            a.capa
                .to_lowercase()
                .cmp(&b.capa.to_lowercase())
                .then(a.grosor.cmp(&b.grosor))
        });
        PlanoPorCapas {
            grupos,
            imagenes: pc.imagenes,
        }
    }

    /// Pasa a dibujar en el constructor de `clave` (la conversion por capas)
    /// y dice cual habia.
    fn cambiar_a(&mut self, clave: (String, i16)) -> Option<(String, i16)> {
        let pc = self.por_capas.as_mut()?;
        if pc.actual == clave {
            return None;
        }
        let nuevo = pc
            .aparcados
            .remove(&clave)
            .unwrap_or_else(Constructor::nuevo);
        let viejo = std::mem::replace(&mut self.c, nuevo);
        let antes = std::mem::replace(&mut pc.actual, clave);
        pc.aparcados.insert(antes.clone(), viejo);
        Some(antes)
    }

    /// El grosor de una entidad, en centesimas de mm.
    fn grosor_de(&self, lw: &LineWeight, capa: &str, del_bloque: i16) -> i16 {
        match lw {
            LineWeight::ByBlock => del_bloque,
            LineWeight::Default => GROSOR_NORMAL,
            LineWeight::Value(v) if *v >= 0 => *v,
            LineWeight::Value(_) | LineWeight::ByLayer => {
                match self.doc.layers.get(capa).map(|l| &l.line_weight) {
                    Some(LineWeight::Value(v)) if *v >= 0 => *v,
                    _ => GROSOR_NORMAL,
                }
            }
        }
    }

    /// El espacio modelo; si esta vacio, el papel.
    pub fn convertir(mut self) -> (Modelo, Cuentas) {
        self.recorrer();
        let cuentas = self.cuentas.clone();
        (self.c.terminar(), cuentas)
    }

    fn recorrer(&mut self) {
        let raiz = Contexto {
            m: Afin::IDENTIDAD,
            capa: None,
            color: COLOR_7,
            grosor: GROSOR_NORMAL,
            profundidad: 0,
        };
        // Lo que mide el plano (de su cabecera, si es creible): para trocear
        // las curvas solo lo que se vea.
        let (a, b) = (
            &self.doc.header.model_space_extents_min,
            &self.doc.header.model_space_extents_max,
        );
        let lado = (b.x - a.x).max(b.y - a.y);
        teselar::poner_tamano_del_plano(if lado.is_finite() && lado > 0.0 && lado < 1e9 {
            lado
        } else {
            0.0
        });
        self.c.unidades = self.doc.header.insertion_units.max(0) as u32;
        let entidades: Vec<&EntityType> = self.doc.model_space_entities().collect();
        let espacio: Vec<&EntityType> = if entidades.iter().any(|e| dibujable(e)) {
            entidades
        } else {
            self.doc.entities_in_block("*Paper_Space").collect()
        };
        for e in espacio {
            if self.c.lleno() {
                break;
            }
            self.entidad(e, &raiz);
        }
    }

    fn capa_visible(&self, nombre: &str) -> bool {
        // Por capas va todo: en la pagina las apagadas se pueden encender.
        self.por_capas.is_some() || self.doc.layers.get(nombre).is_none_or(|l| l.is_visible())
    }

    /// La capa efectiva y el color de una entidad.
    fn estilo<'a>(&self, e: &'a EntityType, cx: &Contexto<'a>) -> Option<(&'a str, u32)>
    where
        'd: 'a,
    {
        let comun = e.common();
        if comun.invisible {
            return None;
        }
        let capa: &'a str = match (comun.layer.as_str(), cx.capa) {
            ("0", Some(c)) => c,
            (c, _) => c,
        };
        if !self.capa_visible(capa) {
            return None;
        }
        let color = match &comun.color {
            Color::ByBlock => cx.color,
            Color::ByLayer | Color::None => self
                .doc
                .layers
                .get(capa)
                .and_then(|l| de_color(&l.color))
                .unwrap_or(COLOR_7),
            c => de_color(c).unwrap_or(COLOR_7),
        };
        Some((capa, color))
    }

    fn linea(&mut self, pts: &[[f64; 2]], color: u32) {
        self.c.polilinea(pts, color, None);
    }

    fn entidad<'a>(&mut self, e: &'a EntityType, cx: &Contexto<'a>)
    where
        'd: 'a,
    {
        let Some((capa, color)) = self.estilo(e, cx) else {
            return;
        };
        self.cuentas.entidades += 1;
        if self.por_capas.is_some() {
            // Cada cosa a lo de su capa y su grosor, y lo de despues (los
            // atributos de un INSERT, la entidad siguiente) a lo de antes.
            let grosor = self.grosor_de(&e.common().line_weight, capa, cx.grosor);
            let cx = &Contexto {
                grosor,
                ..cx.clone()
            };
            let antes = self.cambiar_a((capa.to_string(), grosor));
            self.entidad_sin_medir(e, cx, capa, color);
            if let Some(a) = antes {
                self.cambiar_a(a);
            }
            return;
        }
        let antes = self.c.vertices();
        self.entidad_sin_medir(e, cx, capa, color);
        if !matches!(e, EntityType::Insert(_) | EntityType::Dimension(_)) {
            *self
                .cuentas
                .vertices_por_tipo
                .entry(e.as_entity().entity_type().to_string())
                .or_default() += self.c.vertices() - antes;
        }
    }

    fn entidad_sin_medir<'a>(
        &mut self,
        e: &'a EntityType,
        cx: &Contexto<'a>,
        capa: &'a str,
        color: u32,
    ) where
        'd: 'a,
    {
        let m = cx.m;
        match e {
            EntityType::Line(l) => {
                let pts = [m.p3(&l.start), m.p3(&l.end)];
                self.linea(&pts, color);
            }
            EntityType::Point(p) => {
                // Un punto: una cruz diminuta, que de cerca se ve.
                let c = m.p3(&p.location);
                let r = 1e-3 * m.escala_media();
                self.c
                    .polilinea(&[[c[0] - r, c[1]], [c[0] + r, c[1]]], color, Some(r));
                self.c
                    .polilinea(&[[c[0], c[1] - r], [c[0], c[1] + r]], color, Some(r));
            }
            EntityType::Circle(k) => {
                let t = m.por(&ocs(&k.normal));
                let z = k.center.z;
                if let Some((c, r, _)) = como_circulo(
                    &t,
                    [k.center.x, k.center.y, z],
                    k.radius,
                    0.0,
                    std::f64::consts::TAU,
                ) {
                    self.c.arco(c, r, 0.0, std::f64::consts::TAU, color);
                    return;
                }
                let pts: Vec<_> = teselar::circulo([k.center.x, k.center.y], k.radius)
                    .into_iter()
                    .map(|p| t.punto(p[0], p[1], z))
                    .collect();
                self.linea(&pts, color);
            }
            EntityType::Arc(a) => {
                let t = m.por(&ocs(&a.normal));
                let z = a.center.z;
                let mut barrido = a.end_angle - a.start_angle;
                while barrido <= 0.0 {
                    barrido += std::f64::consts::TAU;
                }
                if let Some((c, r, inicio)) = como_circulo(
                    &t,
                    [a.center.x, a.center.y, z],
                    a.radius,
                    a.start_angle,
                    barrido.min(std::f64::consts::TAU),
                ) {
                    self.c
                        .arco(c, r, inicio, barrido.min(std::f64::consts::TAU), color);
                    return;
                }
                let pts: Vec<_> = teselar::arco(
                    [a.center.x, a.center.y],
                    a.radius,
                    a.start_angle,
                    a.end_angle,
                )
                .into_iter()
                .map(|p| t.punto(p[0], p[1], z))
                .collect();
                self.linea(&pts, color);
            }
            EntityType::Ellipse(el) => {
                // En WCS: el eje menor sale de la normal por el mayor.
                let n = &el.normal;
                let a = &el.major_axis;
                let menor = Vector3::new(
                    n.y * a.z - n.z * a.y,
                    n.z * a.x - n.x * a.z,
                    n.x * a.y - n.y * a.x,
                );
                let r = el.minor_axis_ratio;
                let pts: Vec<_> = teselar::elipse(
                    [0.0, 0.0],
                    [1.0, 0.0],
                    1.0,
                    el.start_parameter,
                    el.end_parameter,
                )
                .into_iter()
                .map(|p| {
                    let (k, s) = (p[0], p[1]);
                    m.punto(
                        el.center.x + a.x * k + menor.x * r * s,
                        el.center.y + a.y * k + menor.y * r * s,
                        el.center.z + a.z * k + menor.z * r * s,
                    )
                })
                .collect();
                self.linea(&pts, color);
            }
            EntityType::LwPolyline(p) => {
                let t = m.por(&ocs(&p.normal));
                let v: Vec<([f64; 2], f64)> = p
                    .vertices
                    .iter()
                    .map(|v| ([v.location.x, v.location.y], v.bulge))
                    .collect();
                let pts = polilinea_2d(&v, p.is_closed);
                let z = p.elevation;
                let pts: Vec<_> = pts.into_iter().map(|q| t.punto(q[0], q[1], z)).collect();
                if p.constant_width > 0.0 && pts.len() >= 2 {
                    self.ancha(&pts, p.constant_width * t.escala_media(), color);
                } else {
                    self.linea(&pts, color);
                }
            }
            EntityType::Polyline2D(p) => {
                let t = m.por(&ocs(&p.normal));
                let v: Vec<([f64; 2], f64)> = p
                    .vertices
                    .iter()
                    .filter(|v| v.flags.bits() & 16 == 0)
                    .map(|v| ([v.location.x, v.location.y], v.bulge))
                    .collect();
                let pts = polilinea_2d(&v, p.flags.is_closed());
                let z = p.elevation;
                let pts: Vec<_> = pts.into_iter().map(|q| t.punto(q[0], q[1], z)).collect();
                self.linea(&pts, color);
            }
            EntityType::Polyline3D(p) => {
                let mut pts: Vec<_> = p.vertices.iter().map(|v| m.p3(&v.position)).collect();
                if p.flags.closed && pts.len() > 2 {
                    pts.push(pts[0]);
                }
                self.linea(&pts, color);
            }
            EntityType::Spline(s) => {
                let pts = if s.control_points.len() >= 2 {
                    let cp: Vec<[f64; 3]> =
                        s.control_points.iter().map(|c| [c.x, c.y, c.z]).collect();
                    // De Boor da x,y; la z se pierde: el plano se ve desde arriba.
                    teselar::spline(s.degree.max(1) as usize, &s.knots, &cp, &s.weights)
                } else {
                    let fp: Vec<[f64; 2]> = s.fit_points.iter().map(|c| [c.x, c.y]).collect();
                    teselar::por_puntos(&fp)
                };
                let pts: Vec<_> = pts.into_iter().map(|q| m.punto(q[0], q[1], 0.0)).collect();
                self.linea(&pts, color);
            }
            EntityType::Solid(s) => {
                let t = m.por(&ocs(&s.normal));
                let p: Vec<[f64; 2]> = [
                    &s.first_corner,
                    &s.second_corner,
                    &s.third_corner,
                    &s.fourth_corner,
                ]
                .iter()
                .map(|v| t.p3(v))
                .collect();
                // El orden de un SOLID es 1-2-4-3.
                self.c
                    .triangulos(&[p[0], p[1], p[2], p[1], p[3], p[2]], color, None);
            }
            EntityType::Face3D(f) => {
                let p: Vec<[f64; 2]> = [
                    &f.first_corner,
                    &f.second_corner,
                    &f.third_corner,
                    &f.fourth_corner,
                    &f.first_corner,
                ]
                .iter()
                .map(|v| m.p3(v))
                .collect();
                self.linea(&p, color);
            }
            EntityType::Leader(l) => {
                let pts: Vec<_> = l.vertices.iter().map(|v| m.p3(v)).collect();
                self.linea(&pts, color);
                if l.arrow_enabled && pts.len() >= 2 {
                    self.flecha(
                        pts[1],
                        pts[0],
                        l.arrow_size.max(0.0) * m.escala_media(),
                        color,
                    );
                }
            }
            EntityType::MultiLeader(ml) => {
                for raiz in &ml.context.leader_roots {
                    for linea in &raiz.lines {
                        let pts: Vec<_> = linea.points.iter().map(|v| m.p3(v)).collect();
                        self.linea(&pts, color);
                    }
                }
                let t = &ml.context;
                if !t.text_string.is_empty() {
                    self.texto_mtext(
                        &t.text_string,
                        "",
                        &t.text_location,
                        t.text_height,
                        0.0,
                        None,
                        1,
                        0.0,
                        &Vector3::new(0., 0., 1.),
                        color,
                        &m,
                    );
                }
            }
            EntityType::Insert(ins) => self.insertar(ins, capa, color, cx),
            EntityType::Dimension(d) => {
                let b = d.base();
                if !b.block_name.is_empty() {
                    let hijo = Contexto {
                        m,
                        capa: Some(capa),
                        color,
                        grosor: cx.grosor,
                        profundidad: cx.profundidad + 1,
                    };
                    self.bloque(&b.block_name, &hijo);
                }
            }
            EntityType::Hatch(h) => self.sombreado(h, color, &m),
            EntityType::Text(t) => {
                let alt = t.alignment_point.as_ref();
                self.texto_simple(
                    &t.value,
                    &t.insertion_point,
                    alt,
                    t.height,
                    t.rotation,
                    t.width_factor,
                    t.oblique_angle,
                    &t.style,
                    t.horizontal_alignment as u8,
                    t.vertical_alignment as u8,
                    &t.normal,
                    color,
                    &m,
                    t.generation_flags,
                );
            }
            EntityType::MText(t) => {
                let ap = t.attachment_point as u8;
                self.texto_mtext(
                    &t.value,
                    &t.style,
                    &t.insertion_point,
                    t.height,
                    t.rotation,
                    t.dwg_x_direction.as_ref(),
                    ap,
                    t.rectangle_width,
                    &t.normal,
                    color,
                    &m,
                );
            }
            EntityType::AttributeEntity(a) => self.atributo(a, color, &m),
            EntityType::Block(_)
            | EntityType::BlockEnd(_)
            | EntityType::Seqend(_)
            | EntityType::AttributeDefinition(_)
            | EntityType::Viewport(_) => {}
            // Los objetos de otras aplicaciones (Civil 3D...): su dibujo de reserva.
            EntityType::Unknown(u) => match u
                .common
                .graphic_data
                .as_deref()
                .and_then(crate::proxy::leer)
            {
                Some(crate::proxy::Reserva::Dibujo(prims)) => self.reserva(&prims, color, &m),
                Some(crate::proxy::Reserva::SoloCaja(clase)) => {
                    self.c.no_dibujado(&format!("{PROXY_SIN_DIBUJO}{clase}"))
                }
                None => self.c.no_dibujado(&u.dxf_name),
            },
            EntityType::PolyfaceMesh(p) => {
                let v: Vec<[f64; 2]> = p.vertices.iter().map(|v| m.p3(&v.location)).collect();
                for f in &p.faces {
                    let idx: Vec<i16> = f.vertex_indices();
                    // Indices desde 1; negativo: arista invisible.
                    for k in 0..idx.len() {
                        let (a, b) = (idx[k], idx[(k + 1) % idx.len()]);
                        if a <= 0 || b == 0 {
                            continue;
                        }
                        if let (Some(pa), Some(pb)) =
                            (v.get(a as usize - 1), v.get(b.unsigned_abs() as usize - 1))
                        {
                            self.linea(&[*pa, *pb], color);
                        }
                    }
                }
            }
            EntityType::PolygonMesh(p) => {
                let (fm, cn) = (
                    p.m_vertex_count.max(0) as usize,
                    p.n_vertex_count.max(0) as usize,
                );
                let v: Vec<[f64; 2]> = p.vertices.iter().map(|v| m.p3(&v.location)).collect();
                if fm * cn == v.len() {
                    for i in 0..fm {
                        self.linea(&v[i * cn..(i + 1) * cn], color);
                    }
                    for j in 0..cn {
                        let col: Vec<[f64; 2]> = (0..fm).map(|i| v[i * cn + j]).collect();
                        self.linea(&col, color);
                    }
                }
            }
            EntityType::Mesh(me) => {
                let v: Vec<[f64; 2]> = me.vertices.iter().map(|q| m.p3(q)).collect();
                for f in &me.faces {
                    let mut pts: Vec<[f64; 2]> = f
                        .vertices
                        .iter()
                        .filter_map(|&i| v.get(i).copied())
                        .collect();
                    if pts.len() > 2 {
                        pts.push(pts[0]);
                        self.linea(&pts, color);
                    }
                }
            }
            // Una imagen: por capas se apunta, para que quien exporta la lea.
            EntityType::RasterImage(im) if self.por_capas.is_some() => {
                let o = m.p3(&im.insertion_point);
                let (w, h) = (im.size.x.max(0.0), im.size.y.max(0.0));
                let lado = |v: &Vector3, n: f64| {
                    let a = m.punto(v.x * n, v.y * n, v.z * n);
                    let z = m.punto(0.0, 0.0, 0.0);
                    [a[0] - z[0], a[1] - z[1]]
                };
                let (u, v) = (lado(&im.u_vector, w), lado(&im.v_vector, h));
                if let Some(pc) = self.por_capas.as_mut()
                    && w > 0.0
                    && h > 0.0
                    && !im.file_path.trim().is_empty()
                {
                    pc.imagenes.push(ImagenDelPlano {
                        capa: capa.to_string(),
                        ruta: im.file_path.clone(),
                        esquina: o,
                        u,
                        v,
                    });
                }
            }
            otro => self.c.no_dibujado(otro.as_entity().entity_type()),
        }
    }

    /// El dibujo de reserva de un objeto desconocido, visto desde arriba.
    fn reserva(&mut self, prims: &[crate::proxy::Primitiva], color: u32, m: &Afin) {
        use crate::proxy::Primitiva;
        for p in prims {
            match p {
                Primitiva::Raya { puntos, color: c } => {
                    let pts: Vec<[f64; 2]> =
                        puntos.iter().map(|q| m.punto(q[0], q[1], q[2])).collect();
                    self.linea(&pts, c.unwrap_or(color));
                }
                Primitiva::Caras { puntos, color: c } => {
                    let pts: Vec<[f64; 2]> =
                        puntos.iter().map(|q| m.punto(q[0], q[1], q[2])).collect();
                    self.c.triangulos(&pts, c.unwrap_or(color), None);
                }
                Primitiva::Texto {
                    pos,
                    dir,
                    alto,
                    ancho,
                    texto,
                    color: c,
                } => {
                    let giro = dir[1].atan2(dir[0]);
                    let ancho = if *ancho > 0.0 { *ancho } else { 1.0 };
                    let z = Vector3::new(0.0, 0.0, 1.0);
                    self.texto_simple(
                        texto,
                        &Vector3::new(pos[0], pos[1], pos[2]),
                        None,
                        *alto,
                        giro,
                        ancho,
                        0.0,
                        "",
                        0,
                        0,
                        &z,
                        c.unwrap_or(color),
                        m,
                        0,
                    );
                }
            }
        }
    }

    fn atributo(&mut self, a: &opencadcodec::entities::AttributeEntity, color: u32, m: &Afin) {
        if a.flags.invisible || a.value.is_empty() {
            return;
        }
        if let Some(mt) = &a.embedded_mtext {
            self.texto_mtext(
                &mt.value,
                &mt.style,
                &mt.insertion_point,
                mt.height,
                mt.rotation,
                mt.dwg_x_direction.as_ref(),
                mt.attachment_point as u8,
                mt.rectangle_width,
                &mt.normal,
                color,
                m,
            );
            return;
        }
        let alt = Some(&a.alignment_point);
        self.texto_simple(
            &a.value,
            &a.insertion_point,
            alt,
            a.height,
            a.rotation,
            a.width_factor,
            a.oblique_angle,
            &a.text_style,
            a.horizontal_alignment as u8,
            a.vertical_alignment as u8,
            &a.normal,
            color,
            m,
            0,
        );
    }

    fn insertar<'a>(
        &mut self,
        ins: &'a opencadcodec::entities::Insert,
        capa: &'a str,
        color: u32,
        cx: &Contexto<'a>,
    ) where
        'd: 'a,
    {
        // Los atributos van en el sistema de quien contiene al INSERT.
        for a in &ins.attributes {
            if self.estilo_attr(a, cx).is_some() {
                self.atributo(
                    a,
                    if matches!(a.common.color, Color::ByBlock) {
                        color
                    } else {
                        self.color_de(&a.common.color, capa, color)
                    },
                    &cx.m,
                );
            }
        }
        if cx.profundidad >= PROFUNDIDAD_MAX || self.abiertos.contains(&ins.block_name) {
            return;
        }
        let Some(br) = self.doc.block_records.get(&ins.block_name) else {
            return;
        };
        let base = &self
            .bloque_base(&ins.block_name)
            .unwrap_or(Vector3::new(0., 0., 0.));
        let (filas, cols) = (
            ins.row_count.max(1) as usize,
            ins.column_count.max(1) as usize,
        );
        let _ = br;
        let mut copias = filas * cols;
        if copias > 1 {
            self.copias_minsert += copias;
            if self.copias_minsert > MINSERT_MAX {
                copias = 1;
            }
        }
        let comun =
            cx.m.por(&ocs(&ins.normal))
                .por(&Afin::traslacion(
                    ins.insert_point.x,
                    ins.insert_point.y,
                    ins.insert_point.z,
                ))
                .por(&Afin::giro_z(ins.rotation));
        let escala = Afin::escala(ins.x_scale(), ins.y_scale(), ins.z_scale())
            .por(&Afin::traslacion(-base.x, -base.y, -base.z));
        self.abiertos.insert(ins.block_name.clone());
        for i in 0..copias {
            let (f, c) = (i / cols, i % cols);
            let m = comun
                .por(&Afin::traslacion(
                    c as f64 * ins.column_spacing,
                    f as f64 * ins.row_spacing,
                    0.0,
                ))
                .por(&escala);
            let hijo = Contexto {
                m,
                capa: Some(capa),
                color,
                grosor: cx.grosor,
                profundidad: cx.profundidad + 1,
            };
            self.bloque(&ins.block_name, &hijo);
        }
        self.abiertos.remove(&ins.block_name);
    }

    fn estilo_attr(
        &self,
        a: &opencadcodec::entities::AttributeEntity,
        cx: &Contexto,
    ) -> Option<()> {
        if a.common.invisible {
            return None;
        }
        let capa = match (a.common.layer.as_str(), cx.capa) {
            ("0", Some(c)) => c,
            (c, _) => c,
        };
        self.capa_visible(capa).then_some(())
    }

    fn color_de(&self, c: &Color, capa: &str, del_bloque: u32) -> u32 {
        match c {
            Color::ByBlock => del_bloque,
            Color::ByLayer | Color::None => self
                .doc
                .layers
                .get(capa)
                .and_then(|l| de_color(&l.color))
                .unwrap_or(COLOR_7),
            c => de_color(c).unwrap_or(COLOR_7),
        }
    }

    fn bloque_base(&self, nombre: &str) -> Option<Vector3> {
        let br = self.doc.block_records.get(nombre)?;
        match self.doc.get_entity(br.block_entity_handle) {
            Some(EntityType::Block(b)) => Some(b.base_point),
            _ => None,
        }
    }

    fn bloque<'a>(&mut self, nombre: &str, cx: &Contexto<'a>)
    where
        'd: 'a,
    {
        let doc = self.doc;
        self.abiertos.insert(nombre.to_string());
        for e in doc.entities_in_block(nombre) {
            if self.c.lleno() {
                break;
            }
            self.entidad(e, cx);
        }
        self.abiertos.remove(nombre);
    }

    /// Una polilinea con ancho: una cinta de triangulos.
    fn ancha(&mut self, pts: &[[f64; 2]], ancho: f64, color: u32) {
        let h = ancho / 2.0;
        let mut tri = Vec::with_capacity(pts.len() * 6);
        for w in pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let l = (dx * dx + dy * dy).sqrt();
            if l < 1e-12 {
                continue;
            }
            let (nx, ny) = (-dy / l * h, dx / l * h);
            let (a1, a2, b1, b2) = (
                [a[0] + nx, a[1] + ny],
                [a[0] - nx, a[1] - ny],
                [b[0] + nx, b[1] + ny],
                [b[0] - nx, b[1] - ny],
            );
            tri.extend_from_slice(&[a1, a2, b1, a2, b2, b1]);
        }
        self.c.triangulos(&tri, color, None);
    }

    fn flecha(&mut self, desde: [f64; 2], punta: [f64; 2], largo: f64, color: u32) {
        let (dx, dy) = (punta[0] - desde[0], punta[1] - desde[1]);
        let l = (dx * dx + dy * dy).sqrt();
        if l < 1e-12 || largo <= 0.0 {
            return;
        }
        let (ux, uy) = (dx / l, dy / l);
        let base = [punta[0] - ux * largo, punta[1] - uy * largo];
        let a = largo / 6.0;
        self.c.triangulos(
            &[
                punta,
                [base[0] - uy * a, base[1] + ux * a],
                [base[0] + uy * a, base[1] - ux * a],
            ],
            color,
            None,
        );
    }

    // ----------------------------------------------------------- sombreados

    fn sombreado(&mut self, h: &opencadcodec::entities::Hatch, color: u32, m: &Afin) {
        let t = m.por(&ocs(&h.normal));
        let z = h.elevation;
        // Los contornos, en el plano del sombreado.
        let mut anillos: Vec<Vec<[f64; 2]>> = Vec::new();
        for camino in &h.paths {
            if camino.flags.bits() & 8 != 0 {
                continue; // el hueco de un texto
            }
            let mut a: Vec<[f64; 2]> = Vec::new();
            for borde in &camino.edges {
                match borde {
                    BoundaryEdge::Line(l) => {
                        a.push([l.start.x, l.start.y]);
                        a.push([l.end.x, l.end.y]);
                    }
                    BoundaryEdge::CircularArc(c) => {
                        let (a0, a1) = if c.counter_clockwise {
                            (c.start_angle, c.end_angle)
                        } else {
                            (-c.start_angle, -c.end_angle)
                        };
                        let mut p = if c.counter_clockwise {
                            teselar::arco([c.center.x, c.center.y], c.radius, a0, a1)
                        } else {
                            let mut v = teselar::arco([c.center.x, c.center.y], c.radius, a1, a0);
                            v.reverse();
                            v
                        };
                        a.append(&mut p);
                    }
                    BoundaryEdge::EllipticArc(e) => {
                        let ccw = e.counter_clockwise;
                        let (t0, t1) = if ccw {
                            (e.start_angle, e.end_angle)
                        } else {
                            (-e.start_angle, -e.end_angle)
                        };
                        let mayor = [e.major_axis_endpoint.x, e.major_axis_endpoint.y];
                        let mut p = if ccw {
                            teselar::elipse(
                                [e.center.x, e.center.y],
                                mayor,
                                e.minor_axis_ratio,
                                t0,
                                t1,
                            )
                        } else {
                            let mut v = teselar::elipse(
                                [e.center.x, e.center.y],
                                mayor,
                                e.minor_axis_ratio,
                                t1,
                                t0,
                            );
                            v.reverse();
                            v
                        };
                        a.append(&mut p);
                    }
                    BoundaryEdge::Spline(s) => {
                        let cp: Vec<[f64; 3]> =
                            s.control_points.iter().map(|c| [c.x, c.y, 0.0]).collect();
                        let pesos: Vec<f64> = if s.rational {
                            s.control_points.iter().map(|c| c.z).collect()
                        } else {
                            Vec::new()
                        };
                        let mut p = if cp.len() >= 2 {
                            teselar::spline(s.degree.max(1) as usize, &s.knots, &cp, &pesos)
                        } else {
                            teselar::por_puntos(
                                &s.fit_points.iter().map(|f| [f.x, f.y]).collect::<Vec<_>>(),
                            )
                        };
                        a.append(&mut p);
                    }
                    BoundaryEdge::Polyline(p) => {
                        let v: Vec<([f64; 2], f64)> =
                            p.vertices.iter().map(|v| ([v.x, v.y], v.z)).collect();
                        a.extend(polilinea_2d(&v, p.is_closed));
                    }
                }
            }
            a.dedup();
            if a.len() >= 3 && a.len() < 200_000 {
                anillos.push(a);
            }
        }
        if anillos.is_empty() {
            return;
        }
        let a_mundo = |p: &[f64; 2]| t.punto(p[0], p[1], z);
        if h.is_solid || h.pattern.lines.is_empty() {
            let (p, i) = crate::relleno::rellenar(&anillos);
            let p: Vec<[f64; 2]> = p.iter().map(a_mundo).collect();
            self.c.malla(&p, &i, color, None);
            return;
        }
        // Con patron: el contorno relleno; las rayas las pinta la tarjeta.
        let (p, i) = crate::relleno::rellenar(&anillos);
        let p: Vec<[f64; 2]> = p.iter().map(a_mundo).collect();
        let al_plano = [
            t.m[0][0],
            t.m[0][1],
            t.m[1][0],
            t.m[1][1],
            t.m[0][2] * z + t.m[0][3],
            t.m[1][2] * z + t.m[1][3],
        ];
        let familias: Vec<crate::modelo::FamiliaPlano> = h
            .pattern
            .lines
            .iter()
            .map(|f| crate::modelo::FamiliaPlano {
                angulo: f.angle,
                base: [f.base_point.x, f.base_point.y],
                desplazamiento: [f.offset.x, f.offset.y],
                trazos: f.dash_lengths.clone(),
            })
            .collect();
        self.c.trama(&p, &i, color, al_plano, &familias);
    }

    // ---------------------------------------------------------------- textos

    #[allow(clippy::too_many_arguments)]
    fn texto_simple(
        &mut self,
        valor: &str,
        ins: &Vector3,
        alineado: Option<&Vector3>,
        alto: f64,
        giro: f64,
        ancho: f64,
        oblicuo: f64,
        estilo: &str,
        h: u8,
        v: u8,
        normal: &Vector3,
        color: u32,
        m: &Afin,
        volteo: i16,
    ) {
        let texto = crate::texto::texto_plano(valor);
        self.cuentas.letras += texto.chars().count();
        let t = m.por(&ocs(normal)).por(&volteo_de_texto(
            volteo,
            [ins.x, ins.y, ins.z],
            alineado.map(|a| [a.x, a.y, a.z]),
            h,
            v,
            giro,
        ));
        if let Some(r) = self.recogidos.as_mut()
            && !texto.trim().is_empty()
        {
            let p = t.punto(ins.x, ins.y, ins.z);
            r.push(TextoDelPlano {
                texto: texto.trim().to_string(),
                x: p[0],
                y: p[1],
                alto: alto * t.escala_media(),
            });
        }
        let fuente = self.fuente_de(estilo);
        self.textos.simple(
            &mut self.c,
            &texto,
            [ins.x, ins.y, ins.z],
            alineado.map(|a| [a.x, a.y, a.z]),
            alto,
            giro,
            ancho,
            oblicuo,
            h,
            v,
            &fuente,
            color,
            &t,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn texto_mtext(
        &mut self,
        valor: &str,
        estilo: &str,
        ins: &Vector3,
        alto: f64,
        giro: f64,
        dir_x: Option<&Vector3>,
        ap: u8,
        caja: f64,
        normal: &Vector3,
        color: u32,
        m: &Afin,
    ) {
        let mut lineas = crate::texto::parrafos_de_mtext(valor);
        // El ancho de letra del estilo vale para todo el MTEXT salvo un \W.
        let ancho_estilo = self
            .doc
            .text_styles
            .get(if estilo.is_empty() {
                "Standard"
            } else {
                estilo
            })
            .map_or(1.0, |s| s.width_factor);
        if ancho_estilo > 0.0 && (ancho_estilo - 1.0).abs() > 1e-6 {
            for t in lineas.iter_mut().flatten() {
                t.ancho.get_or_insert(ancho_estilo);
            }
        }
        self.cuentas.letras += lineas
            .iter()
            .flatten()
            .map(|l| l.texto.chars().count())
            .sum::<usize>();
        let (t, giro) = match dir_x {
            // `dwg_x_direction` va en WCS: el giro sale de ella.
            Some(d) if d.x.abs() + d.y.abs() > 1e-12 => (*m, d.y.atan2(d.x)),
            _ => (m.por(&ocs(normal)), giro),
        };
        if let Some(r) = self.recogidos.as_mut() {
            let junto: Vec<String> = lineas
                .iter()
                .map(|l| l.iter().map(|t| t.texto.as_str()).collect::<String>())
                .filter(|l| !l.trim().is_empty())
                .collect();
            if !junto.is_empty() {
                let p = t.punto(ins.x, ins.y, ins.z);
                r.push(TextoDelPlano {
                    texto: junto.join(" ").trim().to_string(),
                    x: p[0],
                    y: p[1],
                    alto: alto * t.escala_media(),
                });
            }
        }
        let fuente = self.fuente_de(estilo);
        self.textos.multilinea(
            &mut self.c,
            &lineas,
            [ins.x, ins.y, ins.z],
            alto,
            giro,
            ap,
            caja,
            &fuente,
            color,
            &t,
        );
    }

    /// La fuente de un estilo: su SHX, o su TrueType por el nombre de la
    /// familia (el registro de Windows dice que fichero es), o su fichero.
    fn fuente_de(&self, estilo: &str) -> String {
        let Some(s) = self.doc.text_styles.get(if estilo.is_empty() {
            "Standard"
        } else {
            estilo
        }) else {
            return String::new();
        };
        let f = s.font_file.trim().to_lowercase();
        if f.ends_with(".shx") {
            return f;
        }
        if !s.true_type_font.trim().is_empty()
            && let Some(k) = crate::texto::clave_de_familia(&s.true_type_font, false)
        {
            return k;
        }
        if !f.is_empty()
            && !f.contains('.')
            && let Some(k) = crate::texto::clave_de_familia(&f, false)
        {
            return k;
        }
        s.font_file.clone()
    }
}

/// Si `t` lleva un circulo a un circulo (giro, escala igual en los dos ejes,
/// espejo), su centro, su radio y donde empieza el arco en el plano. Si no
/// (escala distinta en x e y, o inclinado en 3D), `None`: se trocea.
fn como_circulo(
    t: &Afin,
    centro: [f64; 3],
    radio: f64,
    inicio: f64,
    barrido: f64,
) -> Option<([f64; 2], f64, f64)> {
    let (a, b, c, d) = (t.m[0][0], t.m[0][1], t.m[1][0], t.m[1][1]);
    let s = (a * a + c * c).sqrt();
    if !(s > 0.0) || !s.is_finite() {
        return None;
    }
    let tol = s * 1e-6;
    let giro = (a - d).abs() < tol && (b + c).abs() < tol;
    let espejo = (a + d).abs() < tol && (b - c).abs() < tol;
    if !giro && !espejo {
        return None;
    }
    let psi = c.atan2(a);
    let inicio_plano = if giro {
        inicio + psi
    } else {
        psi - (inicio + barrido)
    };
    let cen = t.punto(centro[0], centro[1], centro[2]);
    Some((cen, radio * s, inicio_plano))
}

fn dibujable(e: &EntityType) -> bool {
    !matches!(
        e,
        EntityType::Viewport(_)
            | EntityType::Block(_)
            | EntityType::BlockEnd(_)
            | EntityType::Seqend(_)
    )
}

/// Los puntos de una polilinea 2D con sus *bulges*.
pub fn polilinea_2d(v: &[([f64; 2], f64)], cerrada: bool) -> Vec<[f64; 2]> {
    if v.is_empty() {
        return Vec::new();
    }
    let mut out = vec![v[0].0];
    let n = v.len();
    let tramos = if cerrada { n } else { n - 1 };
    for i in 0..tramos {
        let (p, b) = v[i];
        let q = v[(i + 1) % n].0;
        teselar::tramo_bulge(p, q, b, &mut out);
    }
    out
}

/// Abre y convierte por capas (para exportar). Bloquea: proceso aparte.
pub fn convertir_fichero_por_capas(ruta: &std::path::Path) -> Result<PlanoPorCapas, String> {
    let doc = crate::leer::leer(ruta)?;
    Ok(Convertidor::por_capas(&doc).convertir_por_capas())
}

/// **Los textos del plano con su sitio** (TEXT, MTEXT y atributos, tambien
/// los de dentro de los bloques, ya colocados). Recorre el plano como al
/// dibujarlo, asi que cae donde se ve. Bloquea: proceso aparte, como leer.
pub fn textos_del_fichero(ruta: &std::path::Path) -> Result<Vec<TextoDelPlano>, String> {
    let doc = crate::leer::leer(ruta)?;
    let mut c = Convertidor::nuevo(&doc);
    c.recogidos = Some(Vec::new());
    c.recorrer();
    Ok(c.recogidos.take().unwrap_or_default())
}

/// Abre y convierte. Bloquea: hilo o proceso aparte.
pub fn convertir_fichero(ruta: &std::path::Path) -> Result<(Modelo, Cuentas), String> {
    let doc = crate::leer::leer(ruta)?;
    Ok(Convertidor::nuevo(&doc).convertir())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_matriz_del_insert_resta_el_punto_base_y_gira() {
        // Bloque con base (1,0), insertado en (10,0) girado 90 y escala 2.
        let m = Afin::traslacion(10.0, 0.0, 0.0)
            .por(&Afin::giro_z(std::f64::consts::FRAC_PI_2))
            .por(&Afin::escala(2.0, 2.0, 1.0))
            .por(&Afin::traslacion(-1.0, 0.0, 0.0));
        let p = m.punto(1.0, 0.0, 0.0);
        assert!((p[0] - 10.0).abs() < 1e-9 && p[1].abs() < 1e-9);
        let q = m.punto(2.0, 0.0, 0.0);
        assert!((q[0] - 10.0).abs() < 1e-9 && (q[1] - 2.0).abs() < 1e-9);
    }

    #[test]
    fn la_normal_hacia_abajo_da_la_vuelta_a_x() {
        let t = ocs(&Vector3::new(0.0, 0.0, -1.0));
        let p = t.punto(1.0, 0.0, 0.0);
        assert!((p[0] + 1.0).abs() < 1e-9);
        // Caso negativo: la normal de siempre no cambia nada.
        assert_eq!(ocs(&Vector3::new(0.0, 0.0, 1.0)), Afin::IDENTIDAD);
    }

    #[test]
    fn un_circulo_girado_o_en_espejo_sigue_siendo_circulo_y_estirado_no() {
        let (c, r, a) = como_circulo(
            &Afin::giro_z(1.0).por(&Afin::escala(2.0, 2.0, 1.0)),
            [1.0, 0.0, 0.0],
            3.0,
            0.5,
            1.0,
        )
        .unwrap();
        assert!((r - 6.0).abs() < 1e-9 && (a - 1.5).abs() < 1e-9);
        assert!((c[0] - 2.0 * 1f64.cos()).abs() < 1e-9);
        // En espejo (x -> -x): un arco de 0 a 90 queda de 90 a 180.
        let (_, _, a) = como_circulo(
            &Afin::escala(-1.0, 1.0, 1.0),
            [0.0; 3],
            1.0,
            0.0,
            std::f64::consts::FRAC_PI_2,
        )
        .unwrap();
        assert!((a - std::f64::consts::FRAC_PI_2).abs() < 1e-9, "{a}");
        // Caso negativo: estirado no es un circulo.
        assert!(como_circulo(&Afin::escala(2.0, 1.0, 1.0), [0.0; 3], 1.0, 0.0, 1.0).is_none());
    }

    #[test]
    fn el_color_7_cambia_con_el_fondo_y_los_demas_no() {
        assert_eq!(de_color(&Color::Index(7)), Some(COLOR_7));
        assert_eq!(de_color(&Color::Index(1)), Some(rgba(255, 0, 0)));
        assert_eq!(de_color(&Color::ByLayer), None);
    }

    #[test]
    #[ignore = "convierte los planos de muestra de E:/pixpin-cad/muestras"]
    fn convierte_las_muestras() {
        for e in std::fs::read_dir("E:/pixpin-cad/muestras")
            .unwrap()
            .flatten()
        {
            let t = std::time::Instant::now();
            match convertir_fichero(&e.path()) {
                Ok((m, c)) => println!(
                    "{:?}: {:?} entidades={} letras={} vertices={} lineas={} tri={} tramos={}+{} sin_dibujar={:?}
   {:?}",
                    e.file_name(),
                    t.elapsed(),
                    c.entidades,
                    c.letras,
                    m.bytes_gpu() / 1_000_000,
                    m.lineas.len(),
                    m.triangulos.len() / 3,
                    m.tramos_lineas.len(),
                    m.tramos_triangulos.len(),
                    m.sin_dibujar,
                    c.vertices_por_tipo
                ),
                Err(err) => println!("{:?}: ERROR {err}", e.file_name()),
            }
        }
    }
}
