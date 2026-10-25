//! **Dibujar, lo que faltaba**: POLIGONO, ELIPSE, DIVIDIR y GRADUAR, como
//! en AutoCAD.

use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use opencadcodec::entities::{Ellipse, Point};
use opencadcodec::{EntityType, Handle, Vector3};

use super::modificar::{Tramo, cadena_de, invertir, largo, opcion, punto_a};
use crate::especialidad as esp;
use crate::formas::v3;
use crate::geo::{self, P};
use crate::herramienta::{Contexto, Flujo, Herramienta, Lectura, Previa, Registro};
use crate::ordenes::{Grupo, Id, Orden};

macro_rules! registro {
    ($n:literal, [$($a:literal),*], $et:literal, $ay:literal, $crear:expr) => {
        Registro {
            orden: Orden {
                id: Id::Otra($n),
                nombre: $n,
                alias: &[$($a),*],
                grupo: Grupo::Dibujo,
                ayuda: $ay,
                con_objetos: false,
            },
            etiqueta: $et,
            crear: $crear,
        }
    };
}

pub const REGISTROS: &[Registro] = &[
    registro!(
        "POLIGONO",
        ["POL", "POLYGON"],
        "Polígono",
        "Poligono regular inscrito, circunscrito o por un lado",
        || Box::new(Poligono::default())
    ),
    registro!(
        "ELIPSE",
        ["EL", "ELLIPSE"],
        "Elipse",
        "Elipse por su centro (o un eje) y la distancia al otro eje",
        || Box::new(Elipse::default())
    ),
    registro!(
        "DIVIDIR",
        ["DIV", "DIVIDE"],
        "Dividir",
        "Puntos que parten un objeto en partes iguales",
        || Box::new(Repartir::default())
    ),
    registro!(
        "GRADUAR",
        ["ME", "GRA", "MEASURE"],
        "Graduar",
        "Puntos a una distancia fija a lo largo de un objeto",
        || Box::new(Repartir {
            medir: true,
            ..Default::default()
        })
    ),
];

// ================================================================ POLIGONO

/// Lo ultimo que se uso (como en AutoCAD, se ofrece la vez siguiente).
static LADOS: AtomicU32 = AtomicU32::new(4);
static INSCRITO: AtomicBool = AtomicBool::new(true);

/// Los lados que se admiten.
const LADOS_MAX: u32 = 1024;

/// **POLIGONO**: regular, por su centro y el radio del circulo (inscrito o
/// circunscrito) o por un lado.
#[derive(Default)]
struct Poligono {
    lados: Option<u32>,
    centro: Option<P>,
    por_lado: bool,
    lado_a: Option<P>,
    inscrito: Option<bool>,
}

/// Lo que da el radio: un punto (que marca el giro) o un numero (con el
/// lado de abajo horizontal).
#[derive(Clone, Copy)]
enum Radio {
    Hasta(P),
    Vale(f64),
}

/// Los vertices de un poligono de `n` lados con centro `c`.
fn vertices_centro(c: P, n: u32, radio: Radio, inscrito: bool) -> Option<Vec<P>> {
    let medio = PI / n as f64;
    let (r, giro) = match radio {
        Radio::Hasta(q) => {
            let a = geo::angulo(c, q);
            // Inscrito: un vertice en el punto; circunscrito: el medio de un lado.
            (geo::dist(c, q), if inscrito { a } else { a + medio })
        }
        Radio::Vale(r) => (r, -FRAC_PI_2 + medio),
    };
    if r <= geo::EPS {
        return None;
    }
    let rv = if inscrito { r } else { r / medio.cos() };
    Some(
        (0..n)
            .map(|k| geo::polar(c, rv, giro + TAU * k as f64 / n as f64))
            .collect(),
    )
}

/// Los vertices de un poligono de `n` lados con el lado `a`-`b` (antihorario).
fn vertices_lado(a: P, b: P, n: u32) -> Option<Vec<P>> {
    if geo::dist(a, b) <= geo::EPS {
        return None;
    }
    let mut v = vec![a, b];
    let mut dir = geo::restar(b, a);
    for _ in 2..n {
        dir = geo::rotar(dir, [0.0, 0.0], TAU / n as f64);
        let u = v[v.len() - 1];
        v.push(geo::sumar(u, dir));
    }
    Some(v)
}

impl Poligono {
    fn n(&self) -> u32 {
        self.lados.unwrap_or_else(|| LADOS.load(Ordering::Relaxed))
    }

    fn crear(&self, cx: &mut Contexto, v: Option<Vec<P>>) -> Flujo {
        match v {
            Some(v) => {
                cx.crear("POLIGONO", vec![esp::polilinea(&v, true)], None);
                Flujo::Termina
            }
            None => {
                cx.decir("El poligono no puede tener tamano cero");
                Flujo::Sigue
            }
        }
    }

    fn radio(&mut self, cx: &mut Contexto, r: Radio) -> Flujo {
        let (Some(c), Some(i)) = (self.centro, self.inscrito) else {
            return Flujo::NoEntiende;
        };
        self.crear(cx, vertices_centro(c, self.n(), r, i))
    }
}

impl Herramienta for Poligono {
    fn indicacion(&self, _cx: &Lectura) -> String {
        if self.lados.is_none() {
            format!("Numero de lados <{}>:", LADOS.load(Ordering::Relaxed))
        } else if self.por_lado {
            if self.lado_a.is_none() {
                "Primer extremo del lado:".into()
            } else {
                "Segundo extremo del lado:".into()
            }
        } else if self.centro.is_none() {
            "Centro del poligono o [Lado]:".into()
        } else if self.inscrito.is_none() {
            let d = if INSCRITO.load(Ordering::Relaxed) {
                "I"
            } else {
                "C"
            };
            format!("Opcion [Inscrito en el circulo/Circunscrito al circulo] <{d}>:")
        } else {
            "Radio del circulo:".into()
        }
    }

    fn base(&self) -> Option<P> {
        if self.por_lado {
            self.lado_a
        } else {
            self.centro.filter(|_| self.inscrito.is_some())
        }
    }

    fn numero(&mut self, cx: &mut Contexto, v: f64) -> Flujo {
        if self.lados.is_none() {
            if v.fract() != 0.0 || !(3.0..=LADOS_MAX as f64).contains(&v) {
                cx.decir(format!("Los lados van de 3 a {LADOS_MAX}"));
                return Flujo::Sigue;
            }
            self.lados = Some(v as u32);
            LADOS.store(v as u32, Ordering::Relaxed);
            return Flujo::Sigue;
        }
        if self.centro.is_some() && self.inscrito.is_some() && !self.por_lado {
            return self.radio(cx, Radio::Vale(v));
        }
        Flujo::NoEntiende
    }

    fn texto(&mut self, _cx: &mut Contexto, t: &str) -> Flujo {
        let o = opcion(t);
        if self.lados.is_none() {
            return Flujo::NoEntiende;
        }
        if self.centro.is_none() && !self.por_lado {
            return match o.as_str() {
                "L" | "E" | "LADO" | "EDGE" => {
                    self.por_lado = true;
                    Flujo::Sigue
                }
                _ => Flujo::NoEntiende,
            };
        }
        if self.centro.is_some() && self.inscrito.is_none() {
            let i = match o.as_str() {
                "I" | "INSCRITO" | "INSCRIBED" => true,
                "C" | "CIRCUNSCRITO" | "CIRCUMSCRIBED" => false,
                _ => return Flujo::NoEntiende,
            };
            self.inscrito = Some(i);
            INSCRITO.store(i, Ordering::Relaxed);
            return Flujo::Sigue;
        }
        Flujo::NoEntiende
    }

    fn punto(&mut self, cx: &mut Contexto, p: P) -> Flujo {
        if self.lados.is_none() {
            return Flujo::NoEntiende;
        }
        if self.por_lado {
            return match self.lado_a {
                None => {
                    self.lado_a = Some(p);
                    Flujo::Sigue
                }
                Some(a) => self.crear(cx, vertices_lado(a, p, self.n())),
            };
        }
        if self.centro.is_none() {
            self.centro = Some(p);
            return Flujo::Sigue;
        }
        if self.inscrito.is_none() {
            return Flujo::NoEntiende;
        }
        self.radio(cx, Radio::Hasta(p))
    }

    fn enter(&mut self, _cx: &mut Contexto) -> Flujo {
        if self.lados.is_none() {
            self.lados = Some(LADOS.load(Ordering::Relaxed));
            return Flujo::Sigue;
        }
        if self.centro.is_some() && self.inscrito.is_none() && !self.por_lado {
            self.inscrito = Some(INSCRITO.load(Ordering::Relaxed));
            return Flujo::Sigue;
        }
        Flujo::Termina
    }

    fn previa(&self, _cx: &Lectura, cursor: P) -> Previa {
        let v = match (self.por_lado, self.lado_a, self.centro, self.inscrito) {
            (true, Some(a), _, _) => vertices_lado(a, cursor, self.n()),
            (false, _, Some(c), Some(i)) => vertices_centro(c, self.n(), Radio::Hasta(cursor), i),
            _ => None,
        };
        match v {
            Some(v) => Previa::default().con(&[esp::polilinea(&v, true)]),
            None => Previa::default(),
        }
    }
}

// ================================================================== ELIPSE

/// **ELIPSE**: por el centro y el extremo de un eje (o por los dos extremos
/// de un eje), y la distancia al otro.
#[derive(Default)]
struct Elipse {
    por_eje: bool,
    puntos: Vec<P>,
}

/// La elipse de centro `c`, semieje `eje` (vector) y otro semieje `b`.
fn elipse(c: P, eje: P, b: f64) -> Option<EntityType> {
    let a = geo::norma(eje);
    if a <= geo::EPS || b <= geo::EPS {
        return None;
    }
    // El eje mayor es el mas largo: la proporcion va de 0 a 1.
    let (mayor, prop) = if b <= a {
        (eje, b / a)
    } else {
        (geo::por(geo::perp(geo::unitario(eje)), b), a / b)
    };
    Some(EntityType::Ellipse(Ellipse::from_center_axes(
        v3(c),
        Vector3::new(mayor[0], mayor[1], 0.0),
        prop,
    )))
}

impl Elipse {
    /// El centro y el semieje, si ya hay eje.
    fn eje(&self) -> Option<(P, P)> {
        let (a, b) = (*self.puntos.first()?, *self.puntos.get(1)?);
        Some(if self.por_eje {
            let c = geo::medio(a, b);
            (c, geo::restar(b, c))
        } else {
            (a, geo::restar(b, a))
        })
    }

    fn crear(&self, cx: &mut Contexto, d: f64) -> Flujo {
        let Some((c, eje)) = self.eje() else {
            return Flujo::NoEntiende;
        };
        match elipse(c, eje, d) {
            Some(e) => {
                cx.crear("ELIPSE", vec![e], None);
                Flujo::Termina
            }
            None => {
                cx.decir("La distancia tiene que ser mayor que cero");
                Flujo::Sigue
            }
        }
    }
}

impl Herramienta for Elipse {
    fn indicacion(&self, _cx: &Lectura) -> String {
        match (self.puntos.len(), self.por_eje) {
            (0, false) => "Centro de la elipse o [Eje]:".into(),
            (0, true) => "Primer extremo del eje o [Centro]:".into(),
            (1, false) => "Extremo del eje:".into(),
            (1, true) => "Otro extremo del eje:".into(),
            _ => "Distancia al otro eje:".into(),
        }
    }

    fn base(&self) -> Option<P> {
        match self.puntos.len() {
            0 => None,
            1 => Some(self.puntos[0]),
            _ => self.eje().map(|(c, _)| c),
        }
    }

    fn texto(&mut self, _cx: &mut Contexto, t: &str) -> Flujo {
        if !self.puntos.is_empty() {
            return Flujo::NoEntiende;
        }
        match opcion(t).as_str() {
            "E" | "EJE" | "A" | "AXIS" => self.por_eje = true,
            "C" | "CENTRO" | "CENTER" => self.por_eje = false,
            _ => return Flujo::NoEntiende,
        }
        Flujo::Sigue
    }

    fn punto(&mut self, cx: &mut Contexto, p: P) -> Flujo {
        match self.puntos.len() {
            0 => {
                self.puntos.push(p);
                Flujo::Sigue
            }
            1 => {
                if geo::dist(self.puntos[0], p) <= geo::EPS {
                    cx.decir("El eje no puede tener largo cero");
                } else {
                    self.puntos.push(p);
                }
                Flujo::Sigue
            }
            _ => {
                let c = self.eje().map_or(p, |(c, _)| c);
                self.crear(cx, geo::dist(c, p))
            }
        }
    }

    fn numero(&mut self, cx: &mut Contexto, v: f64) -> Flujo {
        if self.puntos.len() == 2 {
            self.crear(cx, v)
        } else {
            Flujo::NoEntiende
        }
    }

    fn previa(&self, _cx: &Lectura, cursor: P) -> Previa {
        let Some((c, eje)) = self.eje() else {
            return Previa::default();
        };
        match elipse(c, eje, geo::dist(c, cursor)) {
            Some(e) => Previa::default().con(&[e]),
            None => Previa::default(),
        }
    }
}

// ======================================================= DIVIDIR y GRADUAR

/// Cuantos puntos pone, como mucho (lo de AutoCAD).
const PUNTOS_MAX: usize = 32767;

/// **DIVIDIR** (en partes iguales) y **GRADUAR** (cada tanto): puntos a lo
/// largo de una raya, un arco, un circulo o una polilinea.
#[derive(Default)]
struct Repartir {
    medir: bool,
    objeto: Option<(Handle, P)>,
}

/// Los puntos que parten la cadena en `n` partes (en una cerrada, tambien
/// el del principio).
fn puntos_divididos(c: &[Tramo], cerrada: bool, n: usize) -> Vec<P> {
    let total = largo(c);
    let desde = if cerrada { 0 } else { 1 };
    (desde..n)
        .filter_map(|i| punto_a(c, total * i as f64 / n as f64))
        .collect()
}

/// Los puntos cada `d` desde el principio de la cadena (sin el final).
fn puntos_medidos(c: &[Tramo], d: f64) -> Option<Vec<P>> {
    let total = largo(c);
    let n = (total / d * (1.0 + 1e-12)).floor() as usize;
    if n > PUNTOS_MAX {
        return None;
    }
    Some(
        (1..=n)
            .map(|i| i as f64 * d)
            .filter(|s| *s < total - 1e-9 * total.max(1.0))
            .filter_map(|s| punto_a(c, s))
            .collect(),
    )
}

impl Repartir {
    fn nombre(&self) -> &'static str {
        if self.medir { "GRADUAR" } else { "DIVIDIR" }
    }
}

impl Herramienta for Repartir {
    fn indicacion(&self, _cx: &Lectura) -> String {
        match (self.objeto.is_some(), self.medir) {
            (false, false) => "Designe el objeto a dividir:".into(),
            (false, true) => "Designe el objeto a graduar:".into(),
            (true, false) => format!("Numero de segmentos (2-{PUNTOS_MAX}):"),
            (true, true) => "Longitud de segmento:".into(),
        }
    }

    fn pide_objeto(&self) -> bool {
        self.objeto.is_none()
    }

    fn objeto(&mut self, cx: &mut Contexto, h: Handle, p: P) -> Flujo {
        if cx.dibujo.entidad(h).and_then(cadena_de).is_none() {
            cx.decir("Solo rayas, arcos, circulos y polilineas");
        } else {
            self.objeto = Some((h, p));
        }
        Flujo::Sigue
    }

    fn numero(&mut self, cx: &mut Contexto, v: f64) -> Flujo {
        let Some((h, pick)) = self.objeto else {
            return Flujo::NoEntiende;
        };
        let Some((c, cerrada)) = cx.dibujo.entidad(h).and_then(cadena_de) else {
            cx.decir("El objeto ya no esta");
            return Flujo::Termina;
        };
        let puntos = if self.medir {
            if v <= geo::EPS {
                cx.decir("La longitud tiene que ser mayor que cero");
                return Flujo::Sigue;
            }
            // Desde el extremo mas cerca de donde se toco.
            let c = match (c.first(), c.last()) {
                (Some(a), Some(b)) if !cerrada && geo::dist(pick, b.b) < geo::dist(pick, a.a) => {
                    invertir(&c)
                }
                _ => c,
            };
            match puntos_medidos(&c, v) {
                Some(p) => p,
                None => {
                    cx.decir(format!("Saldrian mas de {PUNTOS_MAX} puntos"));
                    return Flujo::Sigue;
                }
            }
        } else {
            if v.fract() != 0.0 || !(2.0..=PUNTOS_MAX as f64).contains(&v) {
                cx.decir(format!("Los segmentos van de 2 a {PUNTOS_MAX}"));
                return Flujo::Sigue;
            }
            puntos_divididos(&c, cerrada, v as usize)
        };
        let n = puntos.len();
        let v: Vec<EntityType> = puntos
            .into_iter()
            .map(|p| {
                let mut k = Point::new();
                k.location = v3(p);
                EntityType::Point(k)
            })
            .collect();
        cx.crear(self.nombre(), v, None);
        cx.decir(format!("{n} puntos"));
        Flujo::Termina
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::{Dibujo, Sesion};

    fn con(v: Vec<EntityType>) -> Sesion {
        let mut d = Dibujo::nuevo();
        d.editar("PREPARAR", |t| {
            for e in v {
                t.anadir(e);
            }
        });
        Sesion::nueva(d)
    }

    fn polilineas(s: &Sesion) -> Vec<opencadcodec::LwPolyline> {
        s.dibujo
            .doc
            .model_space_entities()
            .filter_map(|e| match e {
                EntityType::LwPolyline(p) => Some(p.clone()),
                _ => None,
            })
            .collect()
    }

    fn puntos(s: &Sesion) -> Vec<P> {
        let mut v: Vec<P> = s
            .dibujo
            .doc
            .model_space_entities()
            .filter_map(|e| match e {
                EntityType::Point(p) => Some([p.location.x, p.location.y]),
                _ => None,
            })
            .collect();
        v.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
        v
    }

    fn cerca(a: P, b: P) -> bool {
        geo::dist(a, b) < 1e-6
    }

    #[test]
    fn poligono_inscrito_circunscrito_y_por_lado() {
        let mut s = con(vec![]);
        s.escribir("pol");
        // Casos negativos: muy pocos lados o con decimales.
        s.escribir("2");
        assert!(s.registro.last().unwrap().contains("de 3 a"));
        s.escribir("4.5");
        assert!(s.indicacion().contains("lados"));
        s.escribir("6");
        s.escribir("0,0");
        s.escribir("i");
        assert!(!s.previa([100.0, 0.0]).formas.is_empty());
        s.escribir("100");
        let p = polilineas(&s);
        assert_eq!(p.len(), 1);
        assert!(p[0].is_closed && p[0].vertices.len() == 6);
        for v in &p[0].vertices {
            assert!(((v.location.x).hypot(v.location.y) - 100.0).abs() < 1e-9);
        }
        // Circunscrito, con el punto en el medio de un lado.
        s.escribir("polygon");
        s.escribir("");
        s.escribir("0,0");
        s.escribir("c");
        s.escribir("0,100");
        let p = &polilineas(&s)[1];
        let r = 100.0 / (PI / 6.0).cos();
        for v in &p.vertices {
            assert!(((v.location.x).hypot(v.location.y) - r).abs() < 1e-9);
        }
        // Por un lado: un cuadrado.
        s.escribir("pol");
        s.escribir("4");
        s.escribir("l");
        s.escribir("0,0");
        s.escribir("100,0");
        let p = &polilineas(&s)[2];
        let area = crate::formas::area_lw(p);
        assert!((area - 10000.0).abs() < 1e-6, "{area}");
        // Caso negativo: radio cero.
        s.escribir("pol");
        s.escribir("");
        s.escribir("0,0");
        s.escribir("");
        s.escribir("0");
        assert!(s.registro.last().unwrap().contains("cero"));
        assert_eq!(polilineas(&s).len(), 3);
    }

    #[test]
    fn elipse_por_centro_y_por_eje() {
        let mut s = con(vec![]);
        s.escribir("el");
        s.escribir("0,0");
        s.escribir("100,0");
        assert!(!s.previa([0.0, 50.0]).formas.is_empty());
        s.escribir("50");
        // El otro eje mas largo: pasa a ser el mayor.
        s.escribir("ellipse");
        s.escribir("0,0");
        s.escribir("50,0");
        s.escribir("0,100");
        s.escribir("el");
        s.escribir("e");
        s.escribir("0,0");
        s.escribir("200,0");
        s.escribir("50");
        let v: Vec<Ellipse> = s
            .dibujo
            .doc
            .model_space_entities()
            .filter_map(|e| match e {
                EntityType::Ellipse(x) => Some(x.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(v.len(), 3);
        assert!(
            (v[0].major_axis.x - 100.0).abs() < 1e-9 && (v[0].minor_axis_ratio - 0.5).abs() < 1e-9
        );
        assert!(v[1].major_axis.x.abs() < 1e-9 && (v[1].major_axis.y - 100.0).abs() < 1e-9);
        assert!((v[1].minor_axis_ratio - 0.5).abs() < 1e-9);
        assert!((v[2].center.x - 100.0).abs() < 1e-9 && (v[2].minor_axis_ratio - 0.5).abs() < 1e-9);
    }

    #[test]
    fn elipse_rechaza_ejes_nulos() {
        let mut s = con(vec![]);
        s.escribir("el");
        s.escribir("10,10");
        s.escribir("10,10");
        assert!(s.registro.last().unwrap().contains("largo cero"));
        s.escribir("100,10");
        s.escribir("0");
        assert!(s.registro.last().unwrap().contains("mayor que cero"));
        s.escribir("zz");
        assert!(s.registro.last().unwrap().contains("no valida"));
        assert_eq!(s.dibujo.doc.model_space_entities().count(), 0);
    }

    #[test]
    fn dividir_una_raya_y_un_circulo() {
        let mut s = con(vec![
            esp::linea([0.0, 0.0], [100.0, 0.0]),
            esp::circulo([0.0, 500.0], 10.0),
        ]);
        s.escribir("div");
        s.clic([50.0, 0.0], 1.0);
        // Casos negativos.
        s.escribir("1");
        assert!(s.registro.last().unwrap().contains("de 2 a"));
        s.escribir("2.5");
        assert!(s.activa().is_some());
        s.escribir("4");
        assert_eq!(puntos(&s), vec![[25.0, 0.0], [50.0, 0.0], [75.0, 0.0]]);
        s.escribir("divide");
        s.clic([10.0, 500.0], 1.0);
        s.escribir("4");
        let p = puntos(&s);
        assert_eq!(p.len(), 7);
        assert!(p.iter().any(|q| cerca(*q, [10.0, 500.0])));
        assert!(p.iter().any(|q| cerca(*q, [0.0, 510.0])));
        // Un solo paso de deshacer.
        s.escribir("u");
        assert_eq!(puntos(&s).len(), 3);
    }

    #[test]
    fn graduar_desde_el_extremo_tocado() {
        let mut s = con(vec![
            esp::polilinea(&[[0.0, 0.0], [50.0, 0.0], [100.0, 0.0]], false),
            esp::texto("A", [500.0, 0.0], 10.0, 0.0, false),
        ]);
        s.escribir("me");
        // Un texto no se gradua.
        s.clic([502.0, 2.0], 1.0);
        assert!(s.registro.last().unwrap().contains("Solo"));
        s.clic([10.0, 0.0], 1.0);
        s.escribir("0");
        assert!(s.registro.last().unwrap().contains("mayor que cero"));
        s.escribir("30");
        assert_eq!(puntos(&s), vec![[30.0, 0.0], [60.0, 0.0], [90.0, 0.0]]);
        s.escribir("measure");
        s.clic([95.0, 0.0], 1.0);
        s.escribir("40");
        let p = puntos(&s);
        assert_eq!(p.len(), 5);
        assert!(p.iter().any(|q| cerca(*q, [20.0, 0.0])));
        assert!(p.iter().any(|q| cerca(*q, [60.0, 0.0])));
    }
}
