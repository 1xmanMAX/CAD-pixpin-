//! **Las cotas de verdad** (DIMENSION con su bloque, ver
//! [`crate::cotas_bloque`]): lineal, alineada, continua, linea base, radio,
//! diametro y angular, y la escala de las cotas. La COTA de siempre
//! ([`crate::ordenes::Id::Cota`]) sigue dejandola desmontada.

use std::cell::Cell;
use std::f64::consts::FRAC_PI_2;

use opencadcodec::entities::Dimension;
use opencadcodec::{EntityType, Handle};

use crate::cotas_bloque as cb;
use crate::dibujo::Dibujo;
use crate::formas::p2;
use crate::geo::{self, P};
use crate::herramienta::{Contexto, Flujo, Herramienta, Lectura, Previa, Registro};
use crate::ordenes::{Grupo, Id, Orden};

const fn orden(nombre: &'static str, alias: &'static [&'static str], ayuda: &'static str) -> Orden {
    Orden {
        id: Id::Otra(nombre),
        nombre,
        alias,
        grupo: Grupo::Anotar,
        ayuda,
        con_objetos: false,
    }
}

pub const REGISTROS: &[Registro] = &[
    Registro {
        orden: orden(
            "COTALINEAL",
            &["DLI", "DIMLINEAR"],
            "Cota horizontal o vertical (segun el cursor)",
        ),
        etiqueta: "Cota lineal",
        crear: || Box::new(Lineal::nueva(false)),
    },
    Registro {
        orden: orden(
            "COTAALINEADA",
            &["DAL", "DIMALIGNED"],
            "Cota paralela a lo medido",
        ),
        etiqueta: "Cota alineada",
        crear: || Box::new(Lineal::nueva(true)),
    },
    Registro {
        orden: orden(
            "CONTINUA",
            &["DCO", "DIMCONTINUE"],
            "Cotas seguidas de la ultima",
        ),
        etiqueta: "Continua",
        crear: || Box::new(Encadenada::nueva(false)),
    },
    Registro {
        orden: orden(
            "LINEABASE",
            &["DBA", "DIMBASELINE"],
            "Cotas desde el mismo origen que la ultima",
        ),
        etiqueta: "Línea base",
        crear: || Box::new(Encadenada::nueva(true)),
    },
    Registro {
        orden: orden(
            "RADIO",
            &["DRA", "DIMRADIUS"],
            "Cota del radio de un arco o circulo",
        ),
        etiqueta: "Radio",
        crear: || Box::new(Radial::nueva(false)),
    },
    Registro {
        orden: orden(
            "DIAMETRO",
            &["DDI", "DIMDIAMETER"],
            "Cota del diametro de un arco o circulo",
        ),
        etiqueta: "Diámetro",
        crear: || Box::new(Radial::nueva(true)),
    },
    Registro {
        orden: orden(
            "ANGULAR",
            &["DAN", "DIMANGULAR"],
            "Cota del angulo entre dos lineas",
        ),
        etiqueta: "Angular",
        crear: || Box::new(Angular::default()),
    },
    Registro {
        orden: orden(
            "ESCALACOTA",
            &["EC", "DIMSCALE"],
            "La escala de las cotas nuevas (DIMSCALE)",
        ),
        etiqueta: "Escala de cotas",
        crear: || Box::new(EscalaCota),
    },
];

// ------------------------------------------------------ la ultima cota

thread_local! {
    /// La ultima cota lineal o alineada hecha, con el dibujo en que se hizo
    /// (su direccion: la sesion no lo mueve mientras se dibuja).
    static ULTIMA: Cell<Option<(usize, Handle)>> = const { Cell::new(None) };
}

fn clave(d: &Dibujo) -> usize {
    std::ptr::from_ref(d) as usize
}

/// Apunta la ultima cota hecha en el dibujo `d`.
fn apuntar(d: &Dibujo, h: Handle) {
    ULTIMA.with(|u| u.set(Some((clave(d), h))));
}

/// Si `h` es una cota de la que se puede seguir (lineal o alineada).
fn encadenable(d: &Dibujo, h: Handle) -> bool {
    matches!(
        d.entidad(h),
        Some(EntityType::Dimension(
            Dimension::Linear(_) | Dimension::Aligned(_)
        ))
    )
}

/// **La ultima cota lineal o alineada** hecha en este dibujo (si sigue ahi).
pub fn ultima(d: &Dibujo) -> Option<Handle> {
    let (k, h) = ULTIMA.with(Cell::get)?;
    (k == clave(d) && encadenable(d, h)).then_some(h)
}

/// La tolerancia para tomar un texto o una cota con un clic, sin saber el
/// zoom: lo que mide un texto.
fn tolerancia(alto_texto: f64) -> f64 {
    alto_texto.max(geo::EPS)
}

/// Crea la cota, la apunta como ultima (si es encadenable) y lo dice.
fn hacer(cx: &mut Contexto, nombre: &str, dim: Dimension) -> Option<Handle> {
    let (alto, dec) = (cx.ajustes.alto_texto, cx.ajustes.decimales);
    let h = cb::crear(cx.dibujo, nombre, dim, alto, dec)?;
    if encadenable(cx.dibujo, h) {
        apuntar(cx.dibujo, h);
    }
    if let Some(EntityType::Dimension(d)) = cx.dibujo.entidad(h) {
        let m = cb::medidas_de(&cx.dibujo.doc, d, cb::Medidas::para(alto, dec));
        let t = cb::texto_de_cota(d, &m)
            .replace("%%d", "°")
            .replace("%%c", "Ø");
        cx.decir(format!("Cota: {t}"));
    }
    Some(h)
}

/// La previa de una cota: sus rayas y su texto junto al cursor.
fn previa_de(cx: &Lectura, dim: Option<Dimension>) -> Previa {
    let Some(dim) = dim else {
        return Previa::default();
    };
    let m = cb::medidas_del_dibujo(&cx.dibujo.doc, cx.ajustes.alto_texto, cx.ajustes.decimales);
    let (v, c) = cb::dibujo_de_cota(&dim, &m);
    let t = cb::texto_de_cota(&dim, &m)
        .replace("%%d", "°")
        .replace("%%c", "Ø");
    let mut p = Previa::default().con(&v);
    p.etiquetas.push((c, t));
    p
}

// --------------------------------------------------- lineal y alineada

#[derive(Debug, Clone, Copy, PartialEq)]
enum Orientacion {
    Horizontal,
    Vertical,
}

/// La direccion de una cota lineal de `a` a `b` con la linea de cota en `c`:
/// horizontal si el cursor esta arriba o abajo, vertical si a un lado.
fn rotacion_por_cursor(a: P, b: P, c: P) -> f64 {
    let (x0, x1) = (a[0].min(b[0]), a[0].max(b[0]));
    let (y0, y1) = (a[1].min(b[1]), a[1].max(b[1]));
    let fuera_x = (x0 - c[0]).max(c[0] - x1).max(0.0);
    let fuera_y = (y0 - c[1]).max(c[1] - y1).max(0.0);
    let horizontal = if fuera_x == 0.0 && fuera_y == 0.0 {
        x1 - x0 >= y1 - y0
    } else {
        fuera_y >= fuera_x
    };
    if horizontal { 0.0 } else { FRAC_PI_2 }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PasoLineal {
    Origen1,
    /// Enter en el primer origen: un clic toma una linea o un arco.
    Objeto,
    Origen2(P),
    Posicion(P, P),
}

/// **COTALINEAL y COTAALINEADA**: dos origenes (o Enter y un objeto) y
/// donde va la linea de cota.
struct Lineal {
    alineada: bool,
    forzar: Option<Orientacion>,
    paso: PasoLineal,
}

impl Lineal {
    fn nueva(alineada: bool) -> Lineal {
        Lineal {
            alineada,
            forzar: None,
            paso: PasoLineal::Origen1,
        }
    }

    fn cota(&self, a: P, b: P, c: P) -> Option<Dimension> {
        if self.alineada {
            return cb::alineada(a, b, c);
        }
        let rot = match self.forzar {
            Some(Orientacion::Horizontal) => 0.0,
            Some(Orientacion::Vertical) => FRAC_PI_2,
            None => rotacion_por_cursor(a, b, c),
        };
        cb::lineal(a, b, c, rot)
    }

    fn nombre(&self) -> &'static str {
        if self.alineada {
            "COTAALINEADA"
        } else {
            "COTALINEAL"
        }
    }
}

impl Herramienta for Lineal {
    fn indicacion(&self, _cx: &Lectura) -> String {
        match self.paso {
            PasoLineal::Origen1 => {
                "Origen de la primera linea de extension (Enter: un objeto):".into()
            }
            PasoLineal::Objeto => "Elija la linea o el arco que acotar:".into(),
            PasoLineal::Origen2(_) => "Origen de la segunda linea de extension:".into(),
            PasoLineal::Posicion(..) if self.alineada => "Donde va la linea de cota:".into(),
            PasoLineal::Posicion(..) => {
                "Donde va la linea de cota (H horizontal, V vertical):".into()
            }
        }
    }

    fn base(&self) -> Option<P> {
        match self.paso {
            PasoLineal::Origen2(a) => Some(a),
            _ => None,
        }
    }

    fn pide_objeto(&self) -> bool {
        self.paso == PasoLineal::Objeto
    }

    fn punto(&mut self, cx: &mut Contexto, p: P) -> Flujo {
        match self.paso {
            PasoLineal::Origen1 => self.paso = PasoLineal::Origen2(p),
            PasoLineal::Objeto => return Flujo::NoEntiende,
            PasoLineal::Origen2(a) => {
                if geo::dist(a, p) < geo::EPS {
                    cx.decir("Los dos origenes coinciden");
                } else {
                    self.paso = PasoLineal::Posicion(a, p);
                }
            }
            PasoLineal::Posicion(a, b) => match self.cota(a, b, p) {
                Some(d) => {
                    hacer(cx, self.nombre(), d);
                    return Flujo::Termina;
                }
                None => cx.decir("No mide nada en esa direccion"),
            },
        }
        Flujo::Sigue
    }

    fn objeto(&mut self, cx: &mut Contexto, h: Handle, p: P) -> Flujo {
        let Some(e) = cx.dibujo.entidad(h) else {
            return Flujo::Sigue;
        };
        match cb::extremos_para_cotar(e, p) {
            Some((a, b)) if geo::dist(a, b) > geo::EPS => self.paso = PasoLineal::Posicion(a, b),
            _ => cx.decir("Solo lineas, tramos de polilinea o arcos"),
        }
        Flujo::Sigue
    }

    fn texto(&mut self, _cx: &mut Contexto, t: &str) -> Flujo {
        if self.alineada || !matches!(self.paso, PasoLineal::Posicion(..)) {
            return Flujo::NoEntiende;
        }
        match t.trim().to_uppercase().as_str() {
            "H" => self.forzar = Some(Orientacion::Horizontal),
            "V" => self.forzar = Some(Orientacion::Vertical),
            _ => return Flujo::NoEntiende,
        }
        Flujo::Sigue
    }

    fn enter(&mut self, _cx: &mut Contexto) -> Flujo {
        if self.paso == PasoLineal::Origen1 {
            self.paso = PasoLineal::Objeto;
            return Flujo::Sigue;
        }
        Flujo::Termina
    }

    fn previa(&self, cx: &Lectura, c: P) -> Previa {
        match self.paso {
            PasoLineal::Posicion(a, b) => previa_de(cx, self.cota(a, b, c)),
            _ => Previa::default(),
        }
    }
}

// --------------------------------------------- continua y linea base

/// Los datos de una cota de la que se sigue.
#[derive(Debug, Clone, Copy)]
struct Base {
    p1: P,
    p2: P,
    /// En la linea de cota, sobre la segunda extension.
    c2: P,
    /// `Some(rot)`: lineal; `None`: alineada.
    rot: Option<f64>,
}

fn base_de(d: &Dibujo, h: Handle) -> Option<Base> {
    match d.entidad(h)? {
        EntityType::Dimension(Dimension::Linear(l)) => Some(Base {
            p1: p2(&l.first_point),
            p2: p2(&l.second_point),
            c2: p2(&l.definition_point),
            rot: Some(l.rotation),
        }),
        EntityType::Dimension(Dimension::Aligned(l)) => Some(Base {
            p1: p2(&l.first_point),
            p2: p2(&l.second_point),
            c2: p2(&l.definition_point),
            rot: None,
        }),
        _ => None,
    }
}

/// La cota que sigue de `b` hasta `q`: continua (desde su segunda extension,
/// en la misma linea de cota) o de linea base (desde la primera, un escalon
/// mas afuera).
fn siguiente(b: &Base, q: P, linea_base: bool, escalon: f64) -> Option<Dimension> {
    let desde = if linea_base { b.p1 } else { b.p2 };
    if geo::dist(desde, q) < geo::EPS {
        return None;
    }
    match b.rot {
        Some(rot) => {
            let n = geo::perp([rot.cos(), rot.sin()]);
            let mut por = b.c2;
            if linea_base {
                let lado = geo::punto(geo::restar(b.c2, b.p2), n);
                let s = if lado < 0.0 { -1.0 } else { 1.0 };
                por = geo::sumar(por, geo::por(n, s * escalon));
            }
            cb::lineal(desde, q, por, rot)
        }
        None => {
            let n = geo::perp(geo::unitario(geo::restar(q, desde)));
            let mut lado = geo::punto(geo::restar(b.c2, desde), n);
            if linea_base {
                lado += if lado < 0.0 { -escalon } else { escalon };
            }
            cb::alineada(desde, q, geo::sumar(desde, geo::por(n, lado)))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Desde {
    /// La ultima cota hecha (si la hay).
    Ultima,
    Cota(Handle),
    /// Un clic elige la cota.
    Elegir,
}

/// **CONTINUA y LINEABASE**: siguen de la ultima cota (o de la que se
/// elija) a cada punto, hasta Enter.
struct Encadenada {
    linea_base: bool,
    desde: Desde,
}

impl Encadenada {
    fn nueva(linea_base: bool) -> Encadenada {
        Encadenada {
            linea_base,
            desde: Desde::Ultima,
        }
    }

    fn cota(&self, d: &Dibujo) -> Option<Handle> {
        match self.desde {
            Desde::Ultima => ultima(d),
            Desde::Cota(h) => encadenable(d, h).then_some(h),
            Desde::Elegir => None,
        }
    }

    fn escalon(cx: &Lectura) -> f64 {
        cb::medidas_del_dibujo(&cx.dibujo.doc, cx.ajustes.alto_texto, cx.ajustes.decimales).escalon
    }

    fn nueva_cota(&self, cx: &Lectura, q: P) -> Option<Dimension> {
        let b = base_de(cx.dibujo, self.cota(cx.dibujo)?)?;
        siguiente(&b, q, self.linea_base, Self::escalon(cx))
    }
}

impl Herramienta for Encadenada {
    fn indicacion(&self, cx: &Lectura) -> String {
        match self.cota(cx.dibujo) {
            Some(_) => {
                "Origen de la siguiente linea de extension (Enter: elegir otra cota):".into()
            }
            None => "Elija la cota de la que seguir (Enter termina):".into(),
        }
    }

    fn base(&self) -> Option<P> {
        None
    }

    fn punto(&mut self, cx: &mut Contexto, p: P) -> Flujo {
        if self.cota(cx.dibujo).is_none() {
            let tol = tolerancia(cx.ajustes.alto_texto);
            match cb::elegir(cx.dibujo, p, tol, false, true) {
                Some(h) if encadenable(cx.dibujo, h) => self.desde = Desde::Cota(h),
                Some(_) => cx.decir("Solo de una cota lineal o alineada"),
                None => cx.decir("No hay una cota ahi"),
            }
            return Flujo::Sigue;
        }
        let nombre = if self.linea_base {
            "LINEABASE"
        } else {
            "CONTINUA"
        };
        match self.nueva_cota(&cx.leer(), p) {
            Some(d) => {
                if let Some(h) = hacer(cx, nombre, d) {
                    self.desde = Desde::Cota(h);
                }
            }
            None => cx.decir("No mide nada hasta ese punto"),
        }
        Flujo::Sigue
    }

    fn enter(&mut self, cx: &mut Contexto) -> Flujo {
        if self.cota(cx.dibujo).is_some() {
            self.desde = Desde::Elegir;
            return Flujo::Sigue;
        }
        Flujo::Termina
    }

    fn previa(&self, cx: &Lectura, c: P) -> Previa {
        previa_de(cx, self.nueva_cota(cx, c))
    }
}

// ------------------------------------------------- radio y diametro

/// **RADIO y DIAMETRO**: un arco o un circulo y donde va el texto.
struct Radial {
    diametro: bool,
    circulo: Option<(P, f64)>,
}

impl Radial {
    fn nueva(diametro: bool) -> Radial {
        Radial {
            diametro,
            circulo: None,
        }
    }

    fn cota(&self, q: P) -> Option<Dimension> {
        let (c, r) = self.circulo?;
        if self.diametro {
            cb::diametro(c, r, q)
        } else {
            cb::radio(c, r, q)
        }
    }
}

impl Herramienta for Radial {
    fn indicacion(&self, _cx: &Lectura) -> String {
        match self.circulo {
            None => "Elija el arco o el circulo:".into(),
            Some(_) => "Donde va la cota:".into(),
        }
    }

    fn pide_objeto(&self) -> bool {
        self.circulo.is_none()
    }

    fn objeto(&mut self, cx: &mut Contexto, h: Handle, _p: P) -> Flujo {
        match cx.dibujo.entidad(h).and_then(cb::circulo_de) {
            Some(c) => self.circulo = Some(c),
            None => cx.decir("Solo arcos o circulos"),
        }
        Flujo::Sigue
    }

    fn punto(&mut self, cx: &mut Contexto, p: P) -> Flujo {
        match self.cota(p) {
            Some(d) => {
                let n = if self.diametro { "DIAMETRO" } else { "RADIO" };
                hacer(cx, n, d);
                Flujo::Termina
            }
            None => Flujo::NoEntiende,
        }
    }

    fn previa(&self, cx: &Lectura, c: P) -> Previa {
        previa_de(cx, self.cota(c))
    }
}

// ----------------------------------------------------------- angular

/// **ANGULAR**: dos lineas y por donde pasa el arco (que elige el angulo).
#[derive(Default)]
struct Angular {
    lineas: Vec<(P, P)>,
}

impl Angular {
    fn cota(&self, q: P) -> Option<Dimension> {
        match self.lineas[..] {
            [a, b] => cb::angular(a, b, q),
            _ => None,
        }
    }
}

impl Herramienta for Angular {
    fn indicacion(&self, _cx: &Lectura) -> String {
        match self.lineas.len() {
            0 => "Elija la primera linea:".into(),
            1 => "Elija la segunda linea:".into(),
            _ => "Donde va el arco de la cota:".into(),
        }
    }

    fn pide_objeto(&self) -> bool {
        self.lineas.len() < 2
    }

    fn objeto(&mut self, cx: &mut Contexto, h: Handle, p: P) -> Flujo {
        let Some(l) = cx.dibujo.entidad(h).and_then(|e| cb::tramo_cercano(e, p)) else {
            cx.decir("Solo lineas o tramos rectos de polilinea");
            return Flujo::Sigue;
        };
        if let [a] = self.lineas[..] {
            let (u, w) = (geo::restar(a.1, a.0), geo::restar(l.1, l.0));
            if geo::cruz(geo::unitario(u), geo::unitario(w)).abs() < 1e-9 {
                cx.decir("Las lineas son paralelas: no hay angulo");
                return Flujo::Sigue;
            }
        }
        self.lineas.push(l);
        Flujo::Sigue
    }

    fn punto(&mut self, cx: &mut Contexto, p: P) -> Flujo {
        if self.lineas.len() < 2 {
            return Flujo::NoEntiende;
        }
        match self.cota(p) {
            Some(d) => {
                hacer(cx, "ANGULAR", d);
                Flujo::Termina
            }
            None => {
                cx.decir("El arco no puede pasar por el vertice");
                Flujo::Sigue
            }
        }
    }

    fn previa(&self, cx: &Lectura, c: P) -> Previa {
        previa_de(cx, self.cota(c))
    }
}

// ------------------------------------------------------- escala

/// **ESCALACOTA** (DIMSCALE): cuantas veces el tamano en el papel (2,5 mm
/// de texto) miden las cotas nuevas.
struct EscalaCota;

impl Herramienta for EscalaCota {
    fn indicacion(&self, cx: &Lectura) -> String {
        let k = cb::escala(&cx.dibujo.doc).unwrap_or(cx.ajustes.alto_texto / 2.5);
        format!("Escala de las cotas <{k}>:")
    }

    fn numero(&mut self, cx: &mut Contexto, v: f64) -> Flujo {
        if !(v > 0.0 && v.is_finite()) {
            cx.decir("La escala tiene que ser mayor que cero");
            return Flujo::Sigue;
        }
        let (alto, dec) = (cx.ajustes.alto_texto, cx.ajustes.decimales);
        cb::poner_escala(cx.dibujo, alto, dec, v);
        cx.decir(format!("Escala de cotas: {v}"));
        Flujo::Termina
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::{Dibujo, Sesion};

    fn sesion() -> Sesion {
        Sesion::nueva(Dibujo::nuevo())
    }

    fn cotas(s: &Sesion) -> Vec<Dimension> {
        s.dibujo
            .doc
            .model_space_entities()
            .filter_map(|e| match e {
                EntityType::Dimension(d) => Some(d.clone()),
                _ => None,
            })
            .collect()
    }

    /// Las piezas del bloque de una cota.
    fn piezas(d: &Dibujo, dim: &Dimension) -> Vec<EntityType> {
        d.doc
            .entities_in_block(&dim.base().block_name)
            .cloned()
            .collect()
    }

    fn texto_del_bloque(d: &Dibujo, dim: &Dimension) -> String {
        piezas(d, dim)
            .iter()
            .find_map(|e| match e {
                EntityType::MText(t) => Some(t.value.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }

    #[test]
    fn lineal_horizontal_o_vertical_segun_el_cursor() {
        let mut s = sesion();
        s.escribir("dli");
        s.escribir("0,0");
        s.escribir("3000,1000");
        s.escribir("1500,-600");
        assert_eq!(s.activa(), None);
        s.escribir("dimlinear");
        s.escribir("0,0");
        s.escribir("3000,1000");
        s.escribir("-600,500");
        let v = cotas(&s);
        assert_eq!(v.len(), 2);
        let Dimension::Linear(h) = &v[0] else {
            panic!()
        };
        assert_eq!(h.rotation, 0.0);
        assert!((h.base.actual_measurement - 3000.0).abs() < 1e-9);
        let Dimension::Linear(w) = &v[1] else {
            panic!()
        };
        assert!((w.rotation - FRAC_PI_2).abs() < 1e-12);
        assert!((w.base.actual_measurement - 1000.0).abs() < 1e-9);
        // Con su bloque *D, su estilo y en la capa de cotas.
        for d in &v {
            let b = d.base();
            assert!(b.block_name.starts_with("*D"), "{}", b.block_name);
            assert!(s.dibujo.doc.block_records.get(&b.block_name).is_some());
            assert_eq!(b.style_name, cb::ESTILO);
            assert_eq!(b.common.layer, crate::especialidad::CAPA_COTAS);
            let p = piezas(&s.dibujo, d);
            assert!(
                p.iter()
                    .filter(|e| matches!(e, EntityType::Line(_)))
                    .count()
                    >= 3
            );
        }
        assert_ne!(v[0].base().block_name, v[1].base().block_name);
        assert_eq!(texto_del_bloque(&s.dibujo, &v[0]), "3000");
        assert_eq!(texto_del_bloque(&s.dibujo, &v[1]), "1000");
        // El estilo, a la escala del dibujo: texto de 250 (2,5 mm a 1/100).
        let e = s.dibujo.doc.dim_styles.get(cb::ESTILO).unwrap();
        assert!((e.dimtxt * e.dimscale - s.ajustes.alto_texto).abs() < 1e-9);
        // Un paso de deshacer por cota.
        s.escribir("u");
        assert_eq!(cotas(&s).len(), 1);
    }

    #[test]
    fn lineal_forzada_y_casos_negativos() {
        let mut s = sesion();
        s.escribir("dli");
        s.escribir("0,0");
        // Caso negativo: el mismo punto dos veces.
        s.escribir("0,0");
        s.escribir("0,2000");
        // Horizontal a la fuerza de dos puntos en vertical: no mide nada.
        s.escribir("h");
        s.escribir("500,1000");
        assert!(cotas(&s).is_empty());
        assert!(s.registro.iter().any(|l| l.contains("No mide nada")));
        s.escribir("v");
        s.escribir("500,1000");
        let v = cotas(&s);
        assert_eq!(v.len(), 1);
        assert!((v[0].base().actual_measurement - 2000.0).abs() < 1e-9);
    }

    #[test]
    fn alineada_y_enter_toma_una_linea() {
        let mut s = sesion();
        s.escribir("l");
        s.escribir("0,0");
        s.escribir("3000,4000");
        s.escribir("");
        s.escribir("dal");
        s.escribir("");
        // Un clic en la linea.
        s.clic([1500.0, 2000.0], 10.0);
        s.escribir("-1000,1000");
        let v = cotas(&s);
        assert_eq!(v.len(), 1);
        let Dimension::Aligned(a) = &v[0] else {
            panic!("{:?}", v[0])
        };
        assert!((a.base.actual_measurement - 5000.0).abs() < 1e-9);
        assert_eq!(texto_del_bloque(&s.dibujo, &v[0]), "5000");
    }

    #[test]
    fn continua_y_linea_base_siguen_de_la_ultima() {
        let mut s = sesion();
        s.escribir("dli");
        s.escribir("0,0");
        s.escribir("1000,0");
        s.escribir("500,-500");
        s.escribir("dco");
        s.escribir("2500,0");
        s.escribir("3000,0");
        s.escribir("");
        s.escribir("");
        assert_eq!(s.activa(), None);
        let v = cotas(&s);
        assert_eq!(v.len(), 3);
        let medidas: Vec<f64> = v.iter().map(|d| d.base().actual_measurement).collect();
        assert_eq!(medidas, vec![1000.0, 1500.0, 500.0]);
        // Todas en la misma linea de cota, una tras otra.
        for d in &v {
            let Dimension::Linear(l) = d else { panic!() };
            assert!((l.definition_point.y + 500.0).abs() < 1e-9);
        }
        let Dimension::Linear(l) = &v[2] else {
            panic!()
        };
        assert_eq!(l.first_point.x, 2500.0);

        // Linea base: desde el primer origen de la ultima, un escalon mas afuera.
        s.escribir("dba");
        s.escribir("4000,0");
        s.escribir("5000,0");
        s.escribir("");
        s.escribir("");
        let v = cotas(&s);
        assert_eq!(v.len(), 5);
        let Dimension::Linear(b1) = &v[3] else {
            panic!()
        };
        let Dimension::Linear(b2) = &v[4] else {
            panic!()
        };
        assert_eq!(b1.first_point.x, 2500.0);
        assert_eq!(b2.first_point.x, 2500.0);
        assert!((b1.base.actual_measurement - 1500.0).abs() < 1e-9);
        assert!((b2.base.actual_measurement - 2500.0).abs() < 1e-9);
        assert!(b1.definition_point.y < -500.0);
        assert!(b2.definition_point.y < b1.definition_point.y);
    }

    #[test]
    fn continua_sin_ultima_pide_elegir_una_cota() {
        let mut s = sesion();
        s.escribir("dli");
        s.escribir("0,0");
        s.escribir("1000,0");
        s.escribir("500,-500");
        // Otro dibujo: no hay ultima; se elige con un clic en la linea de cota.
        let cota = s.dibujo.doc.clone();
        let mut t = Sesion::nueva(Dibujo::de_documento(cota, None));
        t.escribir("dco");
        assert!(t.indicacion().contains("Elija la cota"));
        // Caso negativo: nada ahi.
        t.escribir("500,3000");
        assert!(t.registro.iter().any(|l| l.contains("No hay una cota")));
        t.escribir("500,-500");
        t.escribir("2000,0");
        t.escribir("");
        t.escribir("");
        let v = cotas(&t);
        assert_eq!(v.len(), 2);
        assert!((v[1].base().actual_measurement - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn radio_y_diametro_de_un_circulo() {
        let mut s = sesion();
        s.escribir("c");
        s.escribir("0,0");
        s.escribir("1000");
        s.escribir("dra");
        // Caso negativo: un clic en nada no hace nada.
        s.clic([5000.0, 5000.0], 10.0);
        s.clic([1000.0, 0.0], 10.0);
        s.escribir("300,300");
        s.escribir("ddi");
        s.clic([0.0, 1000.0], 10.0);
        s.escribir("2000,0");
        let v = cotas(&s);
        assert_eq!(v.len(), 2);
        let Dimension::Radius(r) = &v[0] else {
            panic!()
        };
        assert!((r.base.actual_measurement - 1000.0).abs() < 1e-9);
        assert!((r.measurement() - 1000.0).abs() < 1e-9);
        assert_eq!(texto_del_bloque(&s.dibujo, &v[0]), "R1000");
        let Dimension::Diameter(d) = &v[1] else {
            panic!()
        };
        assert!((d.base.actual_measurement - 2000.0).abs() < 1e-9);
        assert!((d.measurement() - 2000.0).abs() < 1e-9);
        assert_eq!(texto_del_bloque(&s.dibujo, &v[1]), "%%c2000");
        // Con flechas (solidos) en el bloque.
        assert!(
            piezas(&s.dibujo, &v[1])
                .iter()
                .any(|e| matches!(e, EntityType::Solid(_)))
        );
    }

    #[test]
    fn angular_entre_dos_lineas_y_paralelas_no() {
        let mut s = sesion();
        for (a, b) in [
            ("0,0", "2000,0"),
            ("0,0", "2000,2000"),
            ("0,500", "2000,500"),
        ] {
            s.escribir("l");
            s.escribir(a);
            s.escribir(b);
            s.escribir("");
        }
        s.escribir("dan");
        s.clic([1000.0, 0.0], 5.0);
        // Caso negativo: la paralela no vale.
        s.clic([1500.0, 500.0], 5.0);
        assert!(s.registro.iter().any(|l| l.contains("paralelas")));
        s.clic([1000.0, 1000.0], 5.0);
        s.escribir("1500,300");
        let v = cotas(&s);
        assert_eq!(v.len(), 1);
        let Dimension::Angular2Ln(a) = &v[0] else {
            panic!()
        };
        assert!((a.base.actual_measurement.to_degrees() - 45.0).abs() < 1e-9);
        assert!((a.measurement_degrees() - 45.0).abs() < 1e-6);
        assert_eq!(texto_del_bloque(&s.dibujo, &v[0]), "45%%d");
        assert!(
            piezas(&s.dibujo, &v[0])
                .iter()
                .any(|e| matches!(e, EntityType::Arc(_)))
        );
    }

    #[test]
    fn escala_de_cotas() {
        let mut s = sesion();
        s.escribir("ec");
        assert!(s.indicacion().contains("100"), "{}", s.indicacion());
        // Caso negativo.
        s.escribir("-5");
        assert!(s.activa().is_some());
        s.escribir("50");
        assert_eq!(s.activa(), None);
        s.escribir("dli");
        s.escribir("0,0");
        s.escribir("1000,0");
        s.escribir("500,-500");
        let v = cotas(&s);
        let EntityType::MText(t) = piezas(&s.dibujo, &v[0])
            .into_iter()
            .find(|e| matches!(e, EntityType::MText(_)))
            .unwrap()
        else {
            panic!()
        };
        assert!((t.height - 125.0).abs() < 1e-9);
    }

    /// Mueve lo elegido de `a` a `b` con MOVER.
    fn mover(s: &mut Sesion, hs: &[Handle], a: &str, b: &str) {
        s.seleccion = hs.to_vec();
        s.escribir("m");
        s.escribir(a);
        s.escribir(b);
    }

    fn la_linea(s: &Sesion) -> Handle {
        s.dibujo
            .doc
            .model_space_entities()
            .find(|e| matches!(e, EntityType::Line(_)))
            .unwrap()
            .common()
            .handle
    }

    fn texto_de(s: &Sesion, k: usize) -> String {
        texto_del_bloque(&s.dibujo, &cotas(s)[k])
    }

    /// Una linea de 0,0 a 3000,0 y su cota alineada (tomada con Enter).
    fn linea_acotada(alineada: bool) -> Sesion {
        let mut s = sesion();
        s.escribir("l");
        s.escribir("0,0");
        s.escribir("3000,0");
        s.escribir("");
        s.escribir(if alineada { "dal" } else { "dli" });
        s.escribir("0,0");
        s.escribir("3000,0");
        s.escribir("1500,-500");
        s
    }

    #[test]
    fn la_cota_sigue_a_la_linea_al_moverla_estirarla_y_girarla() {
        let mut s = linea_acotada(true);
        let l = la_linea(&s);
        let asoc = crate::asociar::leer_cota(&EntityType::Dimension(cotas(&s)[0].clone()));
        assert_eq!(asoc.len(), 2, "los dos puntos van a la linea");
        assert!(asoc.iter().all(|a| a.de.handle() == l));
        // Mover la linea: la cota va con ella, en el mismo paso.
        let pasos = s.dibujo.se_puede_deshacer();
        mover(&mut s, &[l], "0,0", "100,200");
        let Dimension::Aligned(a) = &cotas(&s)[0] else {
            panic!()
        };
        assert_eq!((a.first_point.x, a.first_point.y), (100.0, 200.0));
        assert_eq!((a.second_point.x, a.second_point.y), (3100.0, 200.0));
        // La linea de cota a la misma distancia.
        assert!((a.definition_point.y - (200.0 - 500.0)).abs() < 1e-6);
        assert!(pasos);
        // Las piezas del bloque tambien se movieron.
        let piezas = piezas(&s.dibujo, &cotas(&s)[0]);
        let caja = piezas
            .iter()
            .flat_map(crate::formas::formas)
            .fold(crate::geo::CAJA_VACIA, |c, f| {
                crate::geo::unir_cajas(c, f.caja())
            });
        // (antes, todo por debajo de -100; ahora la extension llega a 200)
        assert!(caja[3] > 0.0 && caja[2] > 3200.0, "{caja:?}");
        // Estirar un extremo con un pinzamiento: cambia la medida y el texto.
        s.seleccion = vec![l];
        assert!(s.tomar_pinzamiento([3100.0, 200.0], 2.0));
        s.clic([4100.0, 200.0], 2.0);
        assert!((cotas(&s)[0].base().actual_measurement - 4000.0).abs() < 1e-6);
        assert_eq!(texto_de(&s, 0), "4000");
        // Girar la linea: la alineada gira con ella.
        s.seleccion = vec![l];
        s.escribir("gi");
        s.escribir("100,200");
        s.escribir("90");
        let Dimension::Aligned(a) = &cotas(&s)[0] else {
            panic!()
        };
        assert!(
            (a.second_point.x - 100.0).abs() < 1e-6 && (a.second_point.y - 4200.0).abs() < 1e-6
        );
        assert!((a.base.actual_measurement - 4000.0).abs() < 1e-6);
        // Deshacer de una vez linea y cota.
        s.escribir("u");
        let Dimension::Aligned(a) = &cotas(&s)[0] else {
            panic!()
        };
        assert!((a.second_point.x - 4100.0).abs() < 1e-6);
        s.escribir("u");
        assert_eq!(texto_de(&s, 0), "3000");
        s.escribir("u");
        let Dimension::Aligned(a) = &cotas(&s)[0] else {
            panic!()
        };
        assert_eq!(a.first_point.x, 0.0);
        assert_eq!(texto_de(&s, 0), "3000");
    }

    #[test]
    fn la_lineal_estirada_deja_la_linea_de_cota_donde_estaba() {
        let mut s = linea_acotada(false);
        let l = la_linea(&s);
        s.seleccion = vec![l];
        assert!(s.tomar_pinzamiento([3000.0, 0.0], 2.0));
        s.clic([3000.0, 800.0], 2.0);
        let Dimension::Linear(d) = &cotas(&s)[0] else {
            panic!()
        };
        assert_eq!(d.rotation, 0.0);
        assert!((d.base.actual_measurement - 3000.0).abs() < 1e-6);
        assert!((d.definition_point.y + 500.0).abs() < 1e-6);
        assert_eq!((d.second_point.x, d.second_point.y), (3000.0, 800.0));
    }

    #[test]
    fn borrar_lo_medido_deja_la_cota_sin_asociar_y_deshacer_la_devuelve() {
        let mut s = linea_acotada(true);
        let l = la_linea(&s);
        s.seleccion = vec![l];
        s.escribir("borrar");
        assert!(s.dibujo.entidad(l).is_none());
        let v = cotas(&s);
        assert_eq!(v.len(), 1, "la cota se queda");
        let e = EntityType::Dimension(v[0].clone());
        assert!(crate::asociar::leer_cota(&e).is_empty());
        assert!((v[0].base().actual_measurement - 3000.0).abs() < 1e-6);
        s.escribir("u");
        let e = EntityType::Dimension(cotas(&s)[0].clone());
        assert_eq!(crate::asociar::leer_cota(&e).len(), 2);
        // Y vuelve a seguir a la linea.
        mover(&mut s, &[l], "0,0", "0,1000");
        assert!((cotas(&s)[0].definition_point().y - 500.0).abs() < 1e-6);
    }

    #[test]
    fn mover_la_cota_sola_la_suelta_y_rehace_su_bloque() {
        let mut s = linea_acotada(true);
        let l = la_linea(&s);
        let h = s
            .dibujo
            .doc
            .model_space_entities()
            .find(|e| matches!(e, EntityType::Dimension(_)))
            .unwrap()
            .common()
            .handle;
        mover(&mut s, &[h], "0,0", "0,-1000");
        let e = s.dibujo.entidad(h).unwrap().clone();
        assert!(crate::asociar::leer_cota(&e).is_empty());
        // Su bloque se movio con ella.
        let p = piezas(&s.dibujo, &cotas(&s)[0]);
        let caja = p
            .iter()
            .flat_map(crate::formas::formas)
            .fold(crate::geo::CAJA_VACIA, |c, f| {
                crate::geo::unir_cajas(c, f.caja())
            });
        assert!(caja[3] < -900.0, "{caja:?}");
        // Mover la linea ya no la mueve (caso negativo).
        mover(&mut s, &[l], "0,0", "0,5000");
        assert!((cotas(&s)[0].definition_point().y + 1500.0).abs() < 1e-6);
    }

    #[test]
    fn la_copia_de_una_cota_no_sigue_al_original() {
        let mut s = linea_acotada(true);
        s.elegir_todo();
        s.escribir("co");
        s.escribir("0,0");
        s.escribir("0,5000");
        s.escribir("");
        let v = cotas(&s);
        assert_eq!(v.len(), 2);
        let asociadas = v
            .iter()
            .filter(|d| !crate::asociar::leer_cota(&EntityType::Dimension((*d).clone())).is_empty())
            .count();
        assert_eq!(asociadas, 1, "solo la original");
        // Mover la linea original no mueve la copia.
        let l = la_linea(&s);
        mover(&mut s, &[l], "0,0", "100,0");
        let xs: Vec<f64> = cotas(&s).iter().map(|d| d.definition_point().x).collect();
        assert!(xs.contains(&3100.0) && xs.contains(&3000.0), "{xs:?}");
    }

    #[test]
    fn una_cota_de_puntos_sueltos_no_se_asocia() {
        let mut s = sesion();
        s.escribir("dli");
        s.escribir("0,0");
        s.escribir("1000,0");
        s.escribir("500,-500");
        let e = EntityType::Dimension(cotas(&s)[0].clone());
        assert!(crate::asociar::leer_cota(&e).is_empty());
    }

    #[test]
    fn radio_y_angular_siguen_a_su_circulo_y_sus_lineas() {
        let mut s = sesion();
        s.escribir("c");
        s.escribir("0,0");
        s.escribir("1000");
        s.escribir("dra");
        s.clic([1000.0, 0.0], 10.0);
        s.escribir("300,300");
        let c = s
            .dibujo
            .doc
            .model_space_entities()
            .find(|e| matches!(e, EntityType::Circle(_)))
            .unwrap()
            .common()
            .handle;
        // Escalar el circulo al doble desde su centro.
        s.seleccion = vec![c];
        s.escribir("es");
        s.escribir("0,0");
        s.escribir("2");
        assert!((cotas(&s)[0].base().actual_measurement - 2000.0).abs() < 1e-6);
        assert_eq!(texto_de(&s, 0), "R2000");
        // Moverlo con un pinzamiento del centro.
        mover(&mut s, &[c], "0,0", "500,0");
        let Dimension::Radius(r) = &cotas(&s)[0] else {
            panic!()
        };
        assert_eq!((r.angle_vertex.x, r.angle_vertex.y), (500.0, 0.0));

        let mut s = sesion();
        for (a, b) in [("0,0", "2000,0"), ("0,0", "2000,2000")] {
            s.escribir("l");
            s.escribir(a);
            s.escribir(b);
            s.escribir("");
        }
        s.escribir("dan");
        s.clic([1000.0, 0.0], 5.0);
        s.clic([1000.0, 1000.0], 5.0);
        s.escribir("1500,300");
        let diagonal = s
            .dibujo
            .doc
            .model_space_entities()
            .filter(|e| matches!(e, EntityType::Line(_)))
            .nth(1)
            .unwrap()
            .common()
            .handle;
        // Girar la diagonal 15 grados mas: el angulo pasa a 60.
        s.seleccion = vec![diagonal];
        s.escribir("gi");
        s.escribir("0,0");
        s.escribir("15");
        let a = &cotas(&s)[0];
        assert!(
            (a.base().actual_measurement.to_degrees() - 60.0).abs() < 1e-6,
            "{}",
            a.base().actual_measurement.to_degrees()
        );
        assert_eq!(texto_de(&s, 0), "60%%d");
    }

    #[test]
    fn deshacer_una_cota_quita_su_bloque_y_rehacer_lo_devuelve() {
        let mut s = linea_acotada(true);
        let nombre = cotas(&s)[0].base().block_name.clone();
        let n = piezas(&s.dibujo, &cotas(&s)[0]).len();
        assert!(s.dibujo.doc.block_records.contains(&nombre));
        s.escribir("u");
        assert!(cotas(&s).is_empty());
        assert!(
            !s.dibujo.doc.block_records.contains(&nombre),
            "sin *D huerfano"
        );
        s.escribir("redo");
        assert_eq!(cotas(&s).len(), 1);
        assert!(s.dibujo.doc.block_records.contains(&nombre));
        assert_eq!(piezas(&s.dibujo, &cotas(&s)[0]).len(), n);
        // Una cota nueva tras deshacer reusa el nombre sin chocar.
        s.escribir("u");
        s.escribir("dli");
        s.escribir("0,0");
        s.escribir("1000,0");
        s.escribir("500,-500");
        assert_eq!(cotas(&s).len(), 1);
        assert!(!piezas(&s.dibujo, &cotas(&s)[0]).is_empty());
    }

    #[test]
    fn radio_y_angular_con_flechas_y_estilos_hijos() {
        let mut s = sesion();
        s.escribir("c");
        s.escribir("0,0");
        s.escribir("1000");
        s.escribir("dra");
        s.clic([1000.0, 0.0], 10.0);
        s.escribir("300,300");
        let doc = &s.dibujo.doc;
        for n in cb::HIJOS {
            let e = doc.dim_styles.get(&cb::nombre_hijo(n)).expect("el hijo");
            assert_eq!(e.dimtsz, 0.0);
        }
        assert!(doc.dim_styles.get(cb::ESTILO).unwrap().dimtsz > 0.0);
        let r = &cotas(&s)[0];
        let m = cb::medidas_de(doc, r, cb::Medidas::para(1.0, 0));
        assert_eq!(m.marca, 0.0);
        // Y la cota lo dice ella misma (DSTYLE con DIMTSZ 0).
        let acad = r.base().common.extended_data.get_record("ACAD").unwrap();
        assert!(
            acad.values
                .iter()
                .any(|v| matches!(v, opencadcodec::xdata::XDataValue::Integer16(142)))
        );
        // La lineal sigue con marcas y sin DSTYLE.
        s.escribir("dli");
        s.escribir("0,0");
        s.escribir("3000,0");
        s.escribir("0,-500");
        let l = &cotas(&s)[1];
        let m = cb::medidas_de(&s.dibujo.doc, l, cb::Medidas::para(1.0, 0));
        assert!(m.marca > 0.0);
        assert!(l.base().common.extended_data.get_record("ACAD").is_none());
        // La escala cambia tambien la de los hijos.
        s.escribir("ec");
        s.escribir("20");
        for n in cb::HIJOS {
            let e = s.dibujo.doc.dim_styles.get(&cb::nombre_hijo(n)).unwrap();
            assert_eq!(e.dimscale, 20.0);
        }
    }

    #[test]
    fn la_asociacion_y_los_estilos_vuelven_en_dwg_y_dxf_y_siguen_valiendo() {
        let mut s = linea_acotada(true);
        s.escribir("c");
        s.escribir("5000,0");
        s.escribir("1000");
        s.escribir("ddi");
        s.clic([6000.0, 0.0], 10.0);
        s.escribir("7000,500");
        let dir = std::env::temp_dir().join(format!("cad-nucleo-asoc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for ext in ["dwg", "dxf"] {
            let ruta = dir.join(format!("asoc.{ext}"));
            s.dibujo.guardar_como(&ruta).unwrap();
            let mut s2 = Sesion::nueva(Dibujo::abrir(&ruta).unwrap());
            for n in cb::HIJOS {
                assert!(
                    s2.dibujo.doc.dim_styles.contains(&cb::nombre_hijo(n)),
                    "{ext}: el estilo hijo {n}"
                );
            }
            let v = cotas(&s2);
            assert_eq!(v.len(), 2, "{ext}");
            let l = la_linea(&s2);
            let asoc = crate::asociar::leer_cota(&EntityType::Dimension(v[0].clone()));
            assert_eq!(asoc.len(), 2, "{ext}: la asociacion se guarda");
            assert!(asoc.iter().all(|a| a.de.handle() == l), "{ext}");
            let asoc = crate::asociar::leer_cota(&EntityType::Dimension(v[1].clone()));
            assert!(
                matches!(
                    asoc[..],
                    [crate::asociar::Asociacion {
                        de: crate::asociar::Referencia::Circulo(_),
                        ..
                    }]
                ),
                "{ext}: {asoc:?}"
            );
            // Y sigue valiendo en lo abierto.
            mover(&mut s2, &[l], "0,0", "0,700");
            let Dimension::Aligned(a) = &cotas(&s2)[0] else {
                panic!()
            };
            assert!((a.first_point.y - 700.0).abs() < 1e-6, "{ext}");
            assert_eq!(texto_de(&s2, 0), "3000", "{ext}");
            // Y otra vuelta: lo cambiado se guarda bien (sin la copia vieja).
            let ruta2 = dir.join(format!("asoc2.{ext}"));
            s2.dibujo.guardar_como(&ruta2).unwrap();
            let s3 = Sesion::nueva(Dibujo::abrir(&ruta2).unwrap());
            let v = cotas(&s3);
            assert_eq!(
                crate::asociar::leer_cota(&EntityType::Dimension(v[0].clone())).len(),
                2,
                "{ext}"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn las_cotas_vuelven_en_dwg_y_dxf_con_su_bloque() {
        let mut s = sesion();
        s.escribir("c");
        s.escribir("0,0");
        s.escribir("1000");
        s.escribir("l");
        s.escribir("0,0");
        s.escribir("2000,2000");
        s.escribir("");
        s.escribir("dli");
        s.escribir("0,0");
        s.escribir("3000,1000");
        s.escribir("1500,-600");
        s.escribir("dal");
        s.escribir("0,0");
        s.escribir("3000,4000");
        s.escribir("-800,600");
        s.escribir("dra");
        s.clic([1000.0, 0.0], 10.0);
        s.escribir("300,300");
        s.escribir("ddi");
        s.clic([0.0, -1000.0], 10.0);
        s.escribir("-2000,0");
        s.escribir("l");
        s.escribir("0,0");
        s.escribir("-3000,0");
        s.escribir("");
        s.escribir("dan");
        s.clic([1000.0, 1000.0], 5.0);
        s.clic([-1500.0, 0.0], 5.0);
        s.escribir("-500,1000");
        let originales = cotas(&s);
        assert_eq!(originales.len(), 5);
        let Dimension::Angular2Ln(a) = &originales[4] else {
            panic!()
        };
        assert!((a.base.actual_measurement.to_degrees() - 135.0).abs() < 1e-9);
        let dir = std::env::temp_dir().join(format!("cad-nucleo-cotas-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for ext in ["dwg", "dxf"] {
            let ruta = dir.join(format!("cotas.{ext}"));
            s.dibujo.guardar_como(&ruta).unwrap();
            let d2 = Dibujo::abrir(&ruta).unwrap();
            let v: Vec<Dimension> = d2
                .doc
                .model_space_entities()
                .filter_map(|e| match e {
                    EntityType::Dimension(d) => Some(d.clone()),
                    _ => None,
                })
                .collect();
            assert_eq!(v.len(), 5, "{ext}");
            for (a, b) in originales.iter().zip(&v) {
                assert_eq!(
                    std::mem::discriminant(a),
                    std::mem::discriminant(b),
                    "{ext}"
                );
                assert!(
                    (a.base().actual_measurement - b.base().actual_measurement).abs() < 1e-6,
                    "{ext}"
                );
                assert!(
                    (a.measurement() - b.measurement()).abs() < 1e-6,
                    "{ext}: {} {}",
                    a.measurement(),
                    b.measurement()
                );
                // El DWG vuelve a numerar los bloques anonimos (*D0, *D1…).
                assert!(b.base().block_name.starts_with("*D"), "{ext}");
                let p = piezas(&d2, b);
                assert!(!p.is_empty(), "{ext}: bloque vacio {}", b.base().block_name);
                assert_eq!(p.len(), piezas(&s.dibujo, a).len(), "{ext}");
                assert_eq!(texto_del_bloque(&d2, b), texto_del_bloque(&s.dibujo, a));
                assert!(
                    d2.doc.dim_styles.get(&b.base().style_name).is_some(),
                    "{ext}: {}",
                    b.base().style_name
                );
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
