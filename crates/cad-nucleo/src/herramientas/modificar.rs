//! **Modificar, lo que faltaba**: ESTIRAR, PARTIR, UNIR, LONGITUD, ALINEAR,
//! IGUALAR y EDITARPOL, como en AutoCAD. Las rayas, arcos, circulos y
//! polilineas se llevan aqui a una cadena de [`Tramo`]s (rectos o con su
//! abombado), que es lo que se parte, se une y se recorre.

use std::f64::consts::{PI, TAU};

use opencadcodec::entities::LwVertex;
use opencadcodec::{EntityType, Handle, LwPolyline, Vector2, Vector3};

use crate::especialidad as esp;
use crate::formas::{self, p2};
use crate::geo::{self, Caja, Forma, P};
use crate::herramienta::{Contexto, Flujo, Herramienta, Lectura, Previa, Registro};
use crate::operaciones as op;
use crate::ordenes::{Grupo, Id, Orden, sin_tildes};

/// Cuantas entidades se pintan, como mucho, en una vista previa.
const EN_PREVIA: usize = 300;

macro_rules! registro {
    ($n:literal, [$($a:literal),*], $g:ident, $obj:literal, $et:literal, $ay:literal, $t:ty) => {
        Registro {
            orden: Orden {
                id: Id::Otra($n),
                nombre: $n,
                alias: &[$($a),*],
                grupo: Grupo::$g,
                ayuda: $ay,
                con_objetos: $obj,
            },
            etiqueta: $et,
            crear: || Box::new(<$t>::default()),
        }
    };
}

pub const REGISTROS: &[Registro] = &[
    registro!(
        "ESTIRAR",
        ["S", "EST", "STRETCH"],
        Modificar,
        false,
        "Estirar",
        "Mueve los vertices que caen en una ventana de captura",
        Estirar
    ),
    registro!(
        "PARTIR",
        ["BR", "PAR", "BREAK"],
        Modificar,
        false,
        "Partir",
        "Quita un trozo entre dos puntos o parte en un punto",
        Partir
    ),
    registro!(
        "UNIR",
        ["J", "UN", "JOIN"],
        Modificar,
        true,
        "Unir",
        "Junta rayas, arcos y polilineas que se tocan en una sola",
        Unir
    ),
    registro!(
        "LONGITUD",
        ["LEN", "LONG", "LENGTHEN"],
        Modificar,
        false,
        "Longitud",
        "Alarga o acorta rayas y arcos (delta, total o porcentaje)",
        Longitud
    ),
    registro!(
        "ALINEAR",
        ["ALI", "ALIGN"],
        Modificar,
        true,
        "Alinear",
        "Mueve y gira (y escala) con dos pares de puntos",
        Alinear
    ),
    registro!(
        "IGUALAR",
        ["MA", "IG", "MATCHPROP", "PAINTER"],
        Modificar,
        false,
        "Igualar",
        "Copia capa, color, tipo y grosor de linea de un objeto a otros",
        Igualar
    ),
    registro!(
        "EDITARPOL",
        ["PE", "EP", "PEDIT"],
        Modificar,
        false,
        "Editar polilínea",
        "Cierra, abre o une una polilinea (convierte rayas y arcos)",
        EditarPol
    ),
];

/// Lo escrito como opcion: sin espacios ni tildes y en mayusculas.
pub(super) fn opcion(t: &str) -> String {
    sin_tildes(t.trim()).to_uppercase()
}

// =================================================================== tramos

/// **Un tramo de una cadena**: de `a` a `b`, recto o curvo con su abombado
/// (`bulge`: tan(angulo/4), positivo antihorario), como en una polilinea.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Tramo {
    pub a: P,
    pub b: P,
    pub bulge: f64,
}

/// Una cadena de tramos seguidos.
pub(super) type Cadena = Vec<Tramo>;

impl Tramo {
    pub fn recto(a: P, b: P) -> Tramo {
        Tramo { a, b, bulge: 0.0 }
    }

    fn es_recto(&self) -> bool {
        self.bulge.abs() < 1e-12
    }

    /// El angulo que barre (con signo: positivo antihorario).
    fn theta(&self) -> f64 {
        4.0 * self.bulge.atan()
    }

    pub fn forma(&self) -> Forma {
        Forma::de_abombado(self.a, self.b, self.bulge)
    }

    pub fn longitud(&self) -> f64 {
        if geo::dist(self.a, self.b) < geo::EPS {
            return 0.0;
        }
        self.forma().longitud()
    }

    /// El punto a la fraccion `f` de su largo (0 en `a`, 1 en `b`).
    pub fn punto(&self, f: f64) -> P {
        match self.forma() {
            Forma::Arco { c, r, .. } => geo::polar(c, r, geo::angulo(c, self.a) + self.theta() * f),
            _ => geo::sumar(self.a, geo::por(geo::restar(self.b, self.a), f)),
        }
    }

    /// El trozo entre las fracciones `f0` y `f1`.
    pub fn parte(&self, f0: f64, f1: f64) -> Tramo {
        Tramo {
            a: self.punto(f0),
            b: self.punto(f1),
            bulge: if self.es_recto() {
                0.0
            } else {
                (self.theta() * (f1 - f0) / 4.0).tan()
            },
        }
    }

    pub fn invertido(&self) -> Tramo {
        Tramo {
            a: self.b,
            b: self.a,
            bulge: -self.bulge,
        }
    }

    /// La fraccion del punto del tramo mas cerca de `p`.
    pub fn fraccion(&self, p: P) -> f64 {
        match self.forma() {
            Forma::Arco { c, .. } => {
                let th = self.theta();
                let (a0, ap) = (geo::angulo(c, self.a), geo::angulo(c, p));
                let d = if th > 0.0 {
                    geo::desde(a0, ap)
                } else {
                    geo::desde(ap, a0)
                };
                let barrido = th.abs();
                if d <= barrido {
                    d / barrido
                } else if d - barrido < TAU - d {
                    1.0
                } else {
                    0.0
                }
            }
            _ => geo::parametro_en_segmento(self.a, self.b, p).clamp(0.0, 1.0),
        }
    }
}

/// El tramo de un arco antihorario.
fn tramo_de_arco(c: P, r: f64, inicio: f64, barrido: f64) -> Tramo {
    Tramo {
        a: geo::polar(c, r, inicio),
        b: geo::polar(c, r, inicio + barrido),
        bulge: (barrido / 4.0).tan(),
    }
}

pub(super) fn largo(c: &[Tramo]) -> f64 {
    c.iter().map(Tramo::longitud).sum()
}

/// La cadena recorrida al reves.
pub(super) fn invertir(c: &[Tramo]) -> Cadena {
    c.iter().rev().map(Tramo::invertido).collect()
}

/// Donde cae `p` en la cadena: la distancia desde el principio hasta el
/// punto de la cadena mas cerca.
pub(super) fn situar(c: &[Tramo], p: P) -> f64 {
    let mut mejor = (f64::INFINITY, 0.0);
    let mut s = 0.0;
    for t in c {
        let f = t.fraccion(p);
        let l = t.longitud();
        let d = geo::dist(p, t.punto(f));
        if d < mejor.0 {
            mejor = (d, s + f * l);
        }
        s += l;
    }
    mejor.1
}

/// El punto a distancia `s` del principio de la cadena.
pub(super) fn punto_a(c: &[Tramo], s: f64) -> Option<P> {
    let mut acc = 0.0;
    for t in c {
        let l = t.longitud();
        if l > geo::EPS && s <= acc + l + 1e-9 {
            return Some(t.punto(((s - acc) / l).clamp(0.0, 1.0)));
        }
        acc += l;
    }
    c.last().map(|t| t.b)
}

/// El trozo de la cadena entre las distancias `s0` y `s1`.
pub(super) fn trozo(c: &[Tramo], s0: f64, s1: f64) -> Cadena {
    let mut v = Vec::new();
    let mut acc = 0.0;
    for t in c {
        let l = t.longitud();
        let (i0, i1) = (acc, acc + l);
        acc = i1;
        if l < geo::EPS {
            continue;
        }
        let (a, b) = (s0.max(i0), s1.min(i1));
        if b - a > 1e-9 * (1.0 + l) {
            v.push(t.parte((a - i0) / l, (b - i0) / l));
        }
    }
    v
}

/// **La cadena de una entidad** (raya, arco, circulo o polilinea ligera) y
/// si es cerrada. `None` para lo demas.
pub(super) fn cadena_de(e: &EntityType) -> Option<(Cadena, bool)> {
    match e {
        EntityType::Line(l) => {
            let (a, b) = (p2(&l.start), p2(&l.end));
            (geo::dist(a, b) > geo::EPS).then(|| (vec![Tramo::recto(a, b)], false))
        }
        EntityType::Arc(_) => match formas::formas(e).first().copied()? {
            Forma::Arco {
                c,
                r,
                inicio,
                barrido,
            } if r > geo::EPS => Some((vec![tramo_de_arco(c, r, inicio, barrido)], false)),
            _ => None,
        },
        EntityType::Circle(_) => match formas::formas(e).first().copied()? {
            Forma::Circulo { c, r } if r > geo::EPS => {
                let (a, b) = (geo::polar(c, r, 0.0), geo::polar(c, r, PI));
                Some((
                    vec![
                        Tramo { a, b, bulge: 1.0 },
                        Tramo {
                            a: b,
                            b: a,
                            bulge: 1.0,
                        },
                    ],
                    true,
                ))
            }
            _ => None,
        },
        EntityType::LwPolyline(p) => {
            let n = p.vertices.len();
            let espejo = p.normal.z < 0.0;
            let pt = |i: usize| {
                let l = &p.vertices[i].location;
                if espejo { [-l.x, l.y] } else { [l.x, l.y] }
            };
            let tramos = if p.is_closed { n } else { n.saturating_sub(1) };
            let c: Cadena = (0..tramos)
                .map(|i| Tramo {
                    a: pt(i),
                    b: pt((i + 1) % n),
                    bulge: p.vertices[i].bulge * if espejo { -1.0 } else { 1.0 },
                })
                .filter(|t| geo::dist(t.a, t.b) > geo::EPS)
                .collect();
            (!c.is_empty()).then_some((c, p.is_closed && n > 1))
        }
        _ => None,
    }
}

/// Copia lo comun de `base` (capa, color, tipo de linea…) en `nueva`.
pub(super) fn con_comun(base: &EntityType, mut nueva: EntityType) -> EntityType {
    let mut c = base.common().clone();
    c.handle = Handle::NULL;
    c.raw_record = None;
    *nueva.common_mut() = c;
    nueva
}

/// La entidad de un solo tramo: una raya o un arco.
pub(super) fn entidad_de_tramo(base: &EntityType, t: &Tramo) -> EntityType {
    let e = match t.forma() {
        Forma::Arco {
            c,
            r,
            inicio,
            barrido,
        } => esp::arco(c, r, inicio, inicio + barrido),
        _ => esp::linea(t.a, t.b),
    };
    con_comun(base, e)
}

/// **Una polilinea ligera** con los tramos de la cadena (y lo comun, el
/// ancho y la elevacion de `base`).
pub(super) fn polilinea_de(base: &EntityType, c: &[Tramo], cerrada: bool) -> EntityType {
    let mut p = match base {
        EntityType::LwPolyline(p) => {
            let mut p = p.clone();
            if p.normal.z < 0.0 {
                p.elevation = -p.elevation;
            }
            p
        }
        _ => LwPolyline::new(),
    };
    p.normal = Vector3::UNIT_Z;
    p.vertices = c
        .iter()
        .map(|t| LwVertex::with_bulge(Vector2::new(t.a[0], t.a[1]), t.bulge))
        .collect();
    if !cerrada && let Some(u) = c.last() {
        p.vertices.push(LwVertex::new(Vector2::new(u.b[0], u.b[1])));
    }
    p.is_closed = cerrada;
    con_comun(base, EntityType::LwPolyline(p))
}

/// Si dos extremos se tocan (con una tolerancia que crece con las coordenadas).
fn coinciden(a: P, b: P) -> bool {
    let escala = a[0].abs().max(a[1].abs()).max(b[0].abs()).max(b[1].abs());
    geo::dist(a, b) <= 1e-6 + 1e-9 * escala
}

/// **Encadena las piezas** que se tocan por sus extremos, empezando por la
/// primera. Da cada cadena con las piezas que la forman (en orden de union).
pub(super) fn cadenas(piezas: &[Cadena]) -> Vec<(Cadena, Vec<usize>)> {
    let n = piezas.len();
    let mut usada = vec![false; n];
    let mut v = Vec::new();
    for (i, pi) in piezas.iter().enumerate() {
        if usada[i] || pi.is_empty() {
            continue;
        }
        usada[i] = true;
        let mut c = pi.clone();
        let mut miembros = vec![i];
        loop {
            let mut crecio = false;
            for (j, pj) in piezas.iter().enumerate() {
                if usada[j] || pj.is_empty() {
                    continue;
                }
                let (ini, fin) = (c[0].a, c[c.len() - 1].b);
                if coinciden(ini, fin) {
                    break;
                }
                let (ja, jb) = (pj[0].a, pj[pj.len() - 1].b);
                if coinciden(fin, ja) {
                    c.extend_from_slice(pj);
                } else if coinciden(fin, jb) {
                    c.extend(invertir(pj));
                } else if coinciden(ini, jb) {
                    let mut k = pj.clone();
                    k.extend(c);
                    c = k;
                } else if coinciden(ini, ja) {
                    let mut k = invertir(pj);
                    k.extend(c);
                    c = k;
                } else {
                    continue;
                }
                usada[j] = true;
                miembros.push(j);
                crecio = true;
            }
            if !crecio {
                break;
            }
        }
        v.push((c, miembros));
    }
    v
}

/// Si una cadena unida cierra sobre si misma.
fn cierra(c: &[Tramo]) -> bool {
    c.len() >= 2 && coinciden(c[0].a, c[c.len() - 1].b)
}

// ================================================================= ESTIRAR

/// **ESTIRAR**: lo elegido con ventanas de captura; los vertices que caen
/// dentro se mueven, lo que cabe entero se mueve entero.
#[derive(Default)]
struct Estirar {
    cajas: Vec<Caja>,
    objetos: Vec<Handle>,
    listos: bool,
    base: Option<P>,
}

/// **Estira una entidad** `d` en lo que cae en `cajas`. Dice si cambio.
fn estirar(e: &mut EntityType, cajas: &[Caja], d: P) -> bool {
    let dentro = |p: P| cajas.iter().any(|c| geo::en_caja(c, p));
    let cambio = match e {
        EntityType::Line(l) => {
            let (ia, ib) = (dentro(p2(&l.start)), dentro(p2(&l.end)));
            if ia {
                l.start = Vector3::new(l.start.x + d[0], l.start.y + d[1], l.start.z);
            }
            if ib {
                l.end = Vector3::new(l.end.x + d[0], l.end.y + d[1], l.end.z);
            }
            ia || ib
        }
        EntityType::LwPolyline(p) => {
            let espejo = p.normal.z < 0.0;
            let mut alguno = false;
            for v in &mut p.vertices {
                let w = if espejo {
                    [-v.location.x, v.location.y]
                } else {
                    [v.location.x, v.location.y]
                };
                if dentro(w) {
                    let q = geo::sumar(w, d);
                    v.location = Vector2::new(if espejo { -q[0] } else { q[0] }, q[1]);
                    alguno = true;
                }
            }
            alguno
        }
        EntityType::Arc(_) => {
            let Some(Forma::Arco {
                c,
                r,
                inicio,
                barrido,
            }) = formas::formas(e).first().copied()
            else {
                return false;
            };
            let t = tramo_de_arco(c, r, inicio, barrido);
            match (dentro(t.a), dentro(t.b)) {
                (false, false) => false,
                (true, true) => {
                    op::trasladar(e, d);
                    true
                }
                (ia, ib) => {
                    // Se mueve un extremo y el arco conserva su flecha.
                    let flecha = t.bulge * geo::dist(t.a, t.b) / 2.0;
                    let na = if ia { geo::sumar(t.a, d) } else { t.a };
                    let nb = if ib { geo::sumar(t.b, d) } else { t.b };
                    let cuerda = geo::dist(na, nb);
                    if cuerda < geo::EPS {
                        return false;
                    }
                    let nuevo = Tramo {
                        a: na,
                        b: nb,
                        bulge: 2.0 * flecha / cuerda,
                    };
                    *e = entidad_de_tramo(e, &nuevo);
                    true
                }
            }
        }
        _ => {
            let pts = match &*e {
                EntityType::Circle(_) => formas::formas(e)
                    .first()
                    .and_then(Forma::centro)
                    .into_iter()
                    .collect(),
                EntityType::Ellipse(x) => vec![p2(&x.center)],
                otro => formas::puntos_de_insercion(otro),
            };
            let mueve = if pts.is_empty() {
                // Lo demas, solo si cabe entero en una ventana.
                let f = formas::formas(e);
                formas::caja(e, &f).is_some_and(|k| cajas.iter().any(|c| geo::contiene(c, &k)))
            } else {
                pts.iter().any(|p| dentro(*p))
            };
            if mueve {
                op::trasladar(e, d);
            }
            mueve
        }
    };
    if cambio {
        e.common_mut().raw_record = None;
    }
    cambio
}

impl Estirar {
    fn aplicar(&self, cx: &mut Contexto, d: P) {
        let (cajas, hs) = (self.cajas.clone(), self.objetos.clone());
        let n = cx.dibujo.editar("ESTIRAR", |t| {
            let mut n = 0;
            for h in hs {
                let Some(mut e) = t.entidad(h).cloned() else {
                    continue;
                };
                if estirar(&mut e, &cajas, d) && t.reemplazar(h, e) {
                    n += 1;
                }
            }
            n
        });
        cx.decir(format!("{n} objetos estirados"));
    }
}

impl Herramienta for Estirar {
    fn indicacion(&self, _cx: &Lectura) -> String {
        if !self.listos {
            "Designe objetos con una ventana de captura (Enter para terminar):".into()
        } else if self.base.is_none() {
            "Punto base:".into()
        } else {
            "Segundo punto (Enter: el primero es el desplazamiento):".into()
        }
    }

    fn base(&self) -> Option<P> {
        self.base
    }

    fn pide_objeto(&self) -> bool {
        !self.listos
    }

    fn objeto(&mut self, cx: &mut Contexto, _h: Handle, _p: P) -> Flujo {
        cx.decir("ESTIRAR elige con una ventana de captura (arrastre de derecha a izquierda)");
        Flujo::Sigue
    }

    fn recuadro(&mut self, cx: &mut Contexto, _hs: Vec<Handle>, a: P, b: P) -> Flujo {
        let caja = [
            a[0].min(b[0]),
            a[1].min(b[1]),
            a[0].max(b[0]),
            a[1].max(b[1]),
        ];
        // Siempre de captura: de la derecha a la izquierda.
        let hs = crate::seleccion::por_recuadro(
            &cx.dibujo.indice(),
            [caja[2], caja[3]],
            [caja[0], caja[1]],
        );
        let antes = self.objetos.len();
        for h in hs {
            if !self.objetos.contains(&h) {
                self.objetos.push(h);
            }
        }
        self.cajas.push(caja);
        cx.decir(format!(
            "{} encontrados, {} en total",
            self.objetos.len() - antes,
            self.objetos.len()
        ));
        Flujo::Sigue
    }

    fn punto(&mut self, cx: &mut Contexto, p: P) -> Flujo {
        if !self.listos {
            cx.decir("Designe con una ventana de captura");
            return Flujo::Sigue;
        }
        match self.base {
            None => {
                self.base = Some(p);
                Flujo::Sigue
            }
            Some(b) => {
                self.aplicar(cx, geo::restar(p, b));
                Flujo::Termina
            }
        }
    }

    fn enter(&mut self, cx: &mut Contexto) -> Flujo {
        if !self.listos {
            if self.objetos.is_empty() {
                cx.decir("No hay objetos elegidos");
                return Flujo::Termina;
            }
            self.listos = true;
            return Flujo::Sigue;
        }
        if let Some(b) = self.base {
            self.aplicar(cx, b);
        }
        Flujo::Termina
    }

    fn previa(&self, cx: &Lectura, cursor: P) -> Previa {
        let Some(b) = self.base else {
            return Previa::default();
        };
        let d = geo::restar(cursor, b);
        let v: Vec<EntityType> = self
            .objetos
            .iter()
            .take(EN_PREVIA)
            .filter_map(|h| {
                let mut e = cx.dibujo.entidad(*h)?.clone();
                estirar(&mut e, &self.cajas, d).then_some(e)
            })
            .collect();
        Previa::default().con(&v)
    }
}

// ================================================================== PARTIR

/// **PARTIR**: quita de un objeto el trozo entre dos puntos (o lo parte en
/// uno, con `@` o el mismo punto).
#[derive(Default)]
struct Partir {
    objeto: Option<Handle>,
    primero: Option<P>,
    pide_primero: bool,
}

/// **Lo que queda** de `e` partido entre `p1` y `p2` (vacio: no queda nada).
fn partido(e: &EntityType, p1: P, p2: P) -> Result<Vec<EntityType>, &'static str> {
    let (c, cerrada) = cadena_de(e).ok_or("Solo se parten rayas, arcos, circulos y polilineas")?;
    if let EntityType::Circle(_) = e {
        let Some(Forma::Circulo { c: k, r }) = formas::formas(e).first().copied() else {
            return Err("Circulo sin radio");
        };
        let (a1, a2) = (geo::angulo(k, p1), geo::angulo(k, p2));
        let d = geo::desde(a1, a2);
        if !(1e-9..=TAU - 1e-9).contains(&d) {
            return Err("Un circulo no se parte en un solo punto");
        }
        // Se quita de p1 a p2 en sentido antihorario.
        return Ok(vec![con_comun(e, esp::arco(k, r, a2, a1))]);
    }
    let total = largo(&c);
    let (s1, s2) = (situar(&c, p1), situar(&c, p2));
    let mut piezas: Vec<Cadena> = if cerrada {
        // Se quita de s1 a s2 en el sentido de la polilinea.
        if s2 >= s1 {
            let mut k = trozo(&c, s2, total);
            k.extend(trozo(&c, 0.0, s1));
            vec![k]
        } else {
            vec![trozo(&c, s2, s1)]
        }
    } else {
        let (a, b) = (s1.min(s2), s1.max(s2));
        vec![trozo(&c, 0.0, a), trozo(&c, b, total)]
    };
    piezas.retain(|k| !k.is_empty());
    Ok(piezas
        .iter()
        .map(|k| match e {
            EntityType::LwPolyline(_) => polilinea_de(e, k, false),
            _ => entidad_de_tramo(e, &k[0]),
        })
        .collect())
}

impl Herramienta for Partir {
    fn indicacion(&self, _cx: &Lectura) -> String {
        if self.objeto.is_none() {
            "Designe el objeto a partir:".into()
        } else if self.pide_primero {
            "Primer punto de rotura:".into()
        } else {
            "Segundo punto de rotura o [Primer punto] (@: partir en el punto):".into()
        }
    }

    fn base(&self) -> Option<P> {
        self.objeto.and(self.primero)
    }

    fn pide_objeto(&self) -> bool {
        self.objeto.is_none()
    }

    fn objeto(&mut self, cx: &mut Contexto, h: Handle, p: P) -> Flujo {
        if cx.dibujo.entidad(h).and_then(cadena_de).is_none() {
            cx.decir("Solo se parten rayas, arcos, circulos y polilineas");
            return Flujo::Sigue;
        }
        self.objeto = Some(h);
        self.primero = Some(p);
        Flujo::Sigue
    }

    fn texto(&mut self, _cx: &mut Contexto, t: &str) -> Flujo {
        match opcion(t).as_str() {
            "P" | "F" | "PRIMER" | "FIRST" if self.objeto.is_some() => {
                self.pide_primero = true;
                Flujo::Sigue
            }
            _ => Flujo::NoEntiende,
        }
    }

    fn punto(&mut self, cx: &mut Contexto, p: P) -> Flujo {
        let Some(h) = self.objeto else {
            return Flujo::NoEntiende;
        };
        if self.pide_primero {
            self.primero = Some(p);
            self.pide_primero = false;
            return Flujo::Sigue;
        }
        let (Some(e), Some(p1)) = (cx.dibujo.entidad(h).cloned(), self.primero) else {
            return Flujo::Termina;
        };
        match partido(&e, p1, p) {
            Err(m) => {
                cx.decir(m);
                Flujo::Sigue
            }
            Ok(piezas) => {
                let n = piezas.len();
                cx.dibujo.editar("PARTIR", |t| {
                    let mut it = piezas.into_iter();
                    match it.next() {
                        Some(primera) => {
                            t.reemplazar(h, primera);
                        }
                        None => {
                            t.quitar(h);
                        }
                    }
                    for e in it {
                        t.anadir(e);
                    }
                });
                cx.decir(format!("Partido: quedan {n} piezas"));
                Flujo::Termina
            }
        }
    }

    fn previa(&self, cx: &Lectura, cursor: P) -> Previa {
        if self.pide_primero {
            return Previa::default();
        }
        let (Some(h), Some(p1)) = (self.objeto, self.primero) else {
            return Previa::default();
        };
        match cx.dibujo.entidad(h).map(|e| partido(e, p1, cursor)) {
            Some(Ok(v)) => Previa::default().con(&v),
            _ => Previa::default(),
        }
    }
}

// ==================================================================== UNIR

/// **UNIR**: rayas alineadas en una raya; rayas, arcos y polilineas que se
/// tocan por los extremos, en una polilinea.
#[derive(Default)]
struct Unir;

/// Si todas son rayas en la misma recta: la raya que las cubre.
fn rayas_alineadas(v: &[EntityType]) -> Option<(P, P)> {
    let mut ext = Vec::new();
    for e in v {
        let (a, b) = op::extremos_de_linea(e)?;
        ext.push((a, b));
    }
    let (a0, b0) = *ext.first()?;
    let dir = geo::unitario(geo::restar(b0, a0));
    let escala = ext
        .iter()
        .flat_map(|(a, b)| [a[0].abs(), a[1].abs(), b[0].abs(), b[1].abs()])
        .fold(1.0, f64::max);
    let tol = 1e-6 + 1e-9 * escala;
    let (mut t0, mut t1) = (f64::INFINITY, f64::NEG_INFINITY);
    for (a, b) in &ext {
        for p in [a, b] {
            let d = geo::restar(*p, a0);
            if geo::cruz(dir, d).abs() > tol {
                return None;
            }
            let t = geo::punto(dir, d);
            t0 = t0.min(t);
            t1 = t1.max(t);
        }
    }
    Some((
        geo::sumar(a0, geo::por(dir, t0)),
        geo::sumar(a0, geo::por(dir, t1)),
    ))
}

/// **Une** lo que se pueda de `hs` en un paso de deshacer. Da cuantos
/// objetos se juntaron.
fn unir(cx: &mut Contexto, hs: &[Handle]) -> usize {
    let mut piezas: Vec<(Handle, EntityType, Cadena)> = Vec::new();
    for h in hs {
        let Some(e) = cx.dibujo.entidad(*h) else {
            continue;
        };
        if matches!(
            e,
            EntityType::Line(_) | EntityType::Arc(_) | EntityType::LwPolyline(_)
        ) && let Some((c, false)) = cadena_de(e)
        {
            piezas.push((*h, e.clone(), c));
        }
    }
    if piezas.len() < 2 {
        cx.decir("Hacen falta al menos dos rayas, arcos o polilineas abiertas");
        return 0;
    }
    let ents: Vec<EntityType> = piezas.iter().map(|p| p.1.clone()).collect();
    if let Some((a, b)) = rayas_alineadas(&ents) {
        let nueva = op::con_extremos(&piezas[0].1, a, b);
        let n = piezas.len();
        cx.dibujo.editar("UNIR", |t| {
            t.reemplazar(piezas[0].0, nueva);
            for p in &piezas[1..] {
                t.quitar(p.0);
            }
        });
        cx.decir(format!("{n} rayas unidas en una"));
        return n;
    }
    let solo: Vec<Cadena> = piezas.iter().map(|p| p.2.clone()).collect();
    let juntas: Vec<_> = cadenas(&solo)
        .into_iter()
        .filter(|(_, m)| m.len() >= 2)
        .collect();
    if juntas.is_empty() {
        cx.decir("Nada que unir: los extremos no se tocan");
        return 0;
    }
    let mut n = 0;
    cx.dibujo.editar("UNIR", |t| {
        for (c, m) in &juntas {
            let base = &piezas[m[0]];
            t.reemplazar(base.0, polilinea_de(&base.1, c, cierra(c)));
            for k in &m[1..] {
                t.quitar(piezas[*k].0);
            }
            n += m.len();
        }
    });
    cx.decir(format!("{n} objetos unidos en {} polilineas", juntas.len()));
    n
}

impl Herramienta for Unir {
    fn indicacion(&self, _cx: &Lectura) -> String {
        String::new()
    }

    fn al_tener_objetos(&mut self, cx: &mut Contexto) -> Flujo {
        let hs = cx.seleccion.clone();
        unir(cx, &hs);
        cx.seleccion.clear();
        Flujo::Termina
    }
}

// ================================================================ LONGITUD

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Modo {
    #[default]
    Medir,
    Delta,
    Total,
    Porcentaje,
}

/// **LONGITUD**: mide, o alarga y acorta por el extremo tocado (delta,
/// longitud total o porcentaje).
#[derive(Default)]
struct Longitud {
    modo: Modo,
    valor: Option<f64>,
}

/// El largo nuevo segun el modo.
fn largo_nuevo(modo: Modo, v: f64, l: f64) -> f64 {
    match modo {
        Modo::Delta => l + v,
        Modo::Total => v,
        Modo::Porcentaje => l * v / 100.0,
        Modo::Medir => l,
    }
}

/// **La raya o el arco** con el extremo mas cerca de `p` llevado al largo nuevo.
fn alargado(e: &EntityType, p: P, modo: Modo, v: f64) -> Result<EntityType, &'static str> {
    match e {
        EntityType::Line(_) => {
            let (a, b) = op::extremos_de_linea(e).ok_or("Raya sin extremos")?;
            let l = geo::dist(a, b);
            let nl = largo_nuevo(modo, v, l);
            if l < geo::EPS || nl <= geo::EPS {
                return Err("La raya quedaria sin longitud");
            }
            let dir = geo::unitario(geo::restar(b, a));
            Ok(if geo::dist(p, b) < geo::dist(p, a) {
                op::con_extremos(e, a, geo::sumar(a, geo::por(dir, nl)))
            } else {
                op::con_extremos(e, geo::restar(b, geo::por(dir, nl)), b)
            })
        }
        EntityType::Arc(_) => {
            let Some(Forma::Arco {
                c,
                r,
                inicio,
                barrido,
            }) = formas::formas(e).first().copied()
            else {
                return Err("Arco sin radio");
            };
            let nb = largo_nuevo(modo, v, r * barrido) / r.max(geo::EPS);
            if nb <= 1e-9 {
                return Err("El arco quedaria sin longitud");
            }
            if nb >= TAU - 1e-9 {
                return Err("El arco daria la vuelta entera");
            }
            let fin = geo::polar(c, r, inicio + barrido);
            let ini = if geo::dist(p, fin) < geo::dist(p, geo::polar(c, r, inicio)) {
                inicio
            } else {
                inicio + barrido - nb
            };
            Ok(con_comun(e, esp::arco(c, r, ini, ini + nb)))
        }
        _ => Err("Solo se alargan rayas y arcos"),
    }
}

impl Herramienta for Longitud {
    fn indicacion(&self, _cx: &Lectura) -> String {
        match (self.modo, self.valor) {
            (Modo::Medir, _) => "Designe un objeto para medirlo o [Delta/Total/Porcentaje]:".into(),
            (Modo::Delta, None) => "Incremento de longitud (negativo acorta):".into(),
            (Modo::Total, None) => "Longitud total:".into(),
            (Modo::Porcentaje, None) => "Porcentaje de la longitud (100 = igual):".into(),
            _ => "Designe el extremo a cambiar o [Delta/Total/Porcentaje] (Enter termina):".into(),
        }
    }

    fn pide_objeto(&self) -> bool {
        self.modo == Modo::Medir || self.valor.is_some()
    }

    fn texto(&mut self, _cx: &mut Contexto, t: &str) -> Flujo {
        let modo = match opcion(t).as_str() {
            "D" | "DE" | "DELTA" => Modo::Delta,
            "T" | "TOTAL" => Modo::Total,
            "P" | "PORCENTAJE" | "PERCENT" => Modo::Porcentaje,
            _ => return Flujo::NoEntiende,
        };
        self.modo = modo;
        self.valor = None;
        Flujo::Sigue
    }

    fn numero(&mut self, cx: &mut Contexto, v: f64) -> Flujo {
        if self.modo == Modo::Medir || self.valor.is_some() {
            cx.decir("Elija antes Delta, Total o Porcentaje");
            return Flujo::Sigue;
        }
        let vale = match self.modo {
            Modo::Delta => v != 0.0,
            _ => v > 0.0,
        };
        if !vale {
            cx.decir(match self.modo {
                Modo::Delta => "El incremento no puede ser cero",
                _ => "Tiene que ser mayor que cero",
            });
            return Flujo::Sigue;
        }
        self.valor = Some(v);
        Flujo::Sigue
    }

    fn objeto(&mut self, cx: &mut Contexto, h: Handle, p: P) -> Flujo {
        let Some(e) = cx.dibujo.entidad(h).cloned() else {
            return Flujo::Sigue;
        };
        let Some(v) = self.valor else {
            let (l, _) = formas::largo_y_area(&e);
            let mut m = format!("Longitud actual: {}", cx.medida(l));
            if let Some(Forma::Arco { barrido, .. }) = formas::formas(&e).first()
                && matches!(e, EntityType::Arc(_))
            {
                m.push_str(&format!(", angulo incluido: {:.2}°", barrido.to_degrees()));
            }
            cx.decir(m);
            return Flujo::Sigue;
        };
        match alargado(&e, p, self.modo, v) {
            Ok(nueva) => {
                cx.dibujo.editar("LONGITUD", |t| t.reemplazar(h, nueva));
            }
            Err(m) => cx.decir(m),
        }
        Flujo::Sigue
    }
}

// ================================================================= ALINEAR

/// **ALINEAR**: lo elegido, de dos puntos de origen a dos de destino
/// (mueve y gira; si se pide, escala).
#[derive(Default)]
struct Alinear {
    /// Origen 1, destino 1, origen 2, destino 2.
    puntos: Vec<P>,
}

/// Lleva `e` de los puntos de origen a los de destino (`pts`: o1, d1[, o2, d2]).
fn alinear(e: &mut EntityType, pts: &[P], escala: bool) {
    if pts.len() < 2 {
        return;
    }
    let (o1, d1) = (pts[0], pts[1]);
    op::trasladar(e, geo::restar(d1, o1));
    if pts.len() >= 4 {
        let (o2, d2) = (pts[2], pts[3]);
        op::girar(e, d1, geo::angulo(d1, d2) - geo::angulo(o1, o2));
        let lo = geo::dist(o1, o2);
        if escala && lo > geo::EPS {
            op::escalar(e, d1, geo::dist(d1, d2) / lo);
        }
    }
}

impl Alinear {
    fn aplicar(&self, cx: &mut Contexto, escala: bool) {
        let pts = self.puntos.clone();
        let hs = cx.seleccion.clone();
        let n = cx.dibujo.editar("ALINEAR", |t| {
            hs.iter()
                .filter(|h| t.cambiar(**h, |e| alinear(e, &pts, escala)))
                .count()
        });
        cx.decir(format!("{n} objetos alineados"));
    }
}

impl Herramienta for Alinear {
    fn indicacion(&self, _cx: &Lectura) -> String {
        match self.puntos.len() {
            0 => "Primer punto de origen:".into(),
            1 => "Primer punto de destino:".into(),
            2 => "Segundo punto de origen (Enter: solo mover):".into(),
            3 => "Segundo punto de destino:".into(),
            _ => "¿Ajustar la escala a los puntos de alineacion? [Si/No] <No>:".into(),
        }
    }

    fn base(&self) -> Option<P> {
        match self.puntos.len() {
            1 => Some(self.puntos[0]),
            3 => Some(self.puntos[2]),
            _ => None,
        }
    }

    fn punto(&mut self, cx: &mut Contexto, p: P) -> Flujo {
        let n = self.puntos.len();
        if n >= 4 {
            return Flujo::NoEntiende;
        }
        if n == 2 && geo::dist(p, self.puntos[0]) < geo::EPS {
            cx.decir("El segundo origen no puede coincidir con el primero");
            return Flujo::Sigue;
        }
        if n == 3 && geo::dist(p, self.puntos[1]) < geo::EPS {
            cx.decir("El segundo destino no puede coincidir con el primero");
            return Flujo::Sigue;
        }
        self.puntos.push(p);
        Flujo::Sigue
    }

    fn texto(&mut self, cx: &mut Contexto, t: &str) -> Flujo {
        if self.puntos.len() < 4 {
            return Flujo::NoEntiende;
        }
        match opcion(t).as_str() {
            "S" | "SI" | "Y" | "YES" => self.aplicar(cx, true),
            "N" | "NO" => self.aplicar(cx, false),
            _ => return Flujo::NoEntiende,
        }
        Flujo::Termina
    }

    fn enter(&mut self, cx: &mut Contexto) -> Flujo {
        if matches!(self.puntos.len(), 2 | 4) {
            self.aplicar(cx, false);
        }
        Flujo::Termina
    }

    fn previa(&self, cx: &Lectura, cursor: P) -> Previa {
        let mut pts = self.puntos.clone();
        if matches!(pts.len(), 1 | 3) {
            pts.push(cursor);
        }
        if pts.len() < 2 {
            return Previa::default();
        }
        let v: Vec<EntityType> = cx
            .seleccion
            .iter()
            .take(EN_PREVIA)
            .filter_map(|h| {
                let mut e = cx.dibujo.entidad(*h)?.clone();
                alinear(&mut e, &pts, false);
                Some(e)
            })
            .collect();
        let mut pv = Previa::default().con(&v);
        pv.guias.push((pts[0], pts[1]));
        pv
    }
}

// ================================================================= IGUALAR

/// **IGUALAR**: copia capa, color, tipo de linea, su escala y el grosor de
/// un objeto a los que se toquen despues.
#[derive(Default)]
struct Igualar {
    fuente: Option<Handle>,
}

impl Igualar {
    fn igualar(&self, cx: &mut Contexto, hs: &[Handle]) {
        let Some(f) = self.fuente.and_then(|h| cx.dibujo.entidad(h)) else {
            return;
        };
        let c = f.common().clone();
        let fuente = self.fuente;
        let n = cx.dibujo.editar("IGUALAR", |t| {
            hs.iter()
                .filter(|h| Some(**h) != fuente)
                .filter(|h| {
                    t.cambiar(**h, |e| {
                        let d = e.common_mut();
                        d.layer = c.layer.clone();
                        d.layer_handle = c.layer_handle;
                        d.color = c.color;
                        d.linetype = c.linetype.clone();
                        d.linetype_handle = c.linetype_handle;
                        d.line_weight = c.line_weight;
                        d.linetype_scale = c.linetype_scale;
                    })
                })
                .count()
        });
        if n > 0 {
            cx.decir(format!("Propiedades copiadas a {n} objetos"));
        }
    }
}

impl Herramienta for Igualar {
    fn indicacion(&self, _cx: &Lectura) -> String {
        if self.fuente.is_none() {
            "Designe el objeto de origen:".into()
        } else {
            "Designe los objetos de destino (Enter termina):".into()
        }
    }

    fn pide_objeto(&self) -> bool {
        true
    }

    fn objeto(&mut self, cx: &mut Contexto, h: Handle, _p: P) -> Flujo {
        if self.fuente.is_none() {
            self.fuente = Some(h);
            if let Some(e) = cx.dibujo.entidad(h) {
                let capa = e.common().layer.clone();
                cx.decir(format!("Origen en la capa {capa}"));
            }
        } else {
            self.igualar(cx, &[h]);
        }
        Flujo::Sigue
    }

    fn recuadro(&mut self, cx: &mut Contexto, hs: Vec<Handle>, _a: P, _b: P) -> Flujo {
        if self.fuente.is_none() {
            cx.decir("Designe primero un solo objeto de origen");
        } else {
            self.igualar(cx, &hs);
        }
        Flujo::Sigue
    }
}

// =============================================================== EDITARPOL

/// **EDITARPOL** (lo basico): cerrar, abrir y unir una polilinea; una raya o
/// un arco se convierte antes.
#[derive(Default)]
struct EditarPol {
    pol: Option<Handle>,
    convertir: Option<Handle>,
    uniendo: Option<Vec<Handle>>,
}

impl EditarPol {
    fn convertir(&mut self, cx: &mut Contexto, h: Handle) {
        let Some(e) = cx.dibujo.entidad(h).cloned() else {
            return;
        };
        let Some((c, _)) = cadena_de(&e) else {
            return;
        };
        cx.dibujo.editar("EDITARPOL", |t| {
            t.reemplazar(h, polilinea_de(&e, &c, false))
        });
        cx.decir("Convertido en polilinea");
        self.pol = Some(h);
        self.convertir = None;
    }

    fn cerrada(&self, cx: &Contexto) -> Option<bool> {
        match cx.dibujo.entidad(self.pol?)? {
            EntityType::LwPolyline(p) => Some(p.is_closed),
            _ => None,
        }
    }

    fn cerrar(&mut self, cx: &mut Contexto, cerrar: bool) {
        let Some(h) = self.pol else { return };
        match self.cerrada(cx) {
            Some(c) if c == cerrar => {
                cx.decir(if cerrar {
                    "Ya esta cerrada"
                } else {
                    "Ya esta abierta"
                });
                return;
            }
            None => return,
            _ => {}
        }
        cx.dibujo.editar("EDITARPOL", |t| {
            t.cambiar(h, |e| {
                if let EntityType::LwPolyline(p) = e {
                    // El ultimo vertice encima del primero sobra al cerrar.
                    let n = p.vertices.len();
                    if cerrar && n > 2 {
                        let (a, b) = (&p.vertices[0].location, &p.vertices[n - 1].location);
                        if coinciden([a.x, a.y], [b.x, b.y]) {
                            p.vertices.pop();
                        }
                    }
                    p.is_closed = cerrar;
                }
            })
        });
    }

    fn unir(&mut self, cx: &mut Contexto, otros: Vec<Handle>) {
        let Some(h) = self.pol else { return };
        let Some(base) = cx.dibujo.entidad(h).cloned() else {
            return;
        };
        let Some((c0, false)) = cadena_de(&base) else {
            return;
        };
        let mut hs = vec![h];
        let mut piezas = vec![c0];
        for o in otros {
            if o == h || hs.contains(&o) {
                continue;
            }
            if let Some(e) = cx.dibujo.entidad(o)
                && matches!(
                    e,
                    EntityType::Line(_) | EntityType::Arc(_) | EntityType::LwPolyline(_)
                )
                && let Some((c, false)) = cadena_de(e)
            {
                hs.push(o);
                piezas.push(c);
            }
        }
        let Some((c, m)) = cadenas(&piezas).into_iter().next() else {
            return;
        };
        if m.len() < 2 {
            cx.decir("Nada que unir: los extremos no se tocan");
            return;
        }
        cx.dibujo.editar("EDITARPOL", |t| {
            t.reemplazar(h, polilinea_de(&base, &c, cierra(&c)));
            for k in &m[1..] {
                t.quitar(hs[*k]);
            }
        });
        cx.decir(format!("{} objetos unidos a la polilinea", m.len() - 1));
    }
}

impl Herramienta for EditarPol {
    fn indicacion(&self, _cx: &Lectura) -> String {
        if self.convertir.is_some() {
            "El objeto no es una polilinea. ¿Convertirlo? [Si/No] <Si>:".into()
        } else if self.pol.is_none() {
            "Designe la polilinea:".into()
        } else if self.uniendo.is_some() {
            "Designe los objetos que unir (Enter para unir):".into()
        } else {
            "Opcion [Cerrar/Abrir/Unir] (Enter termina):".into()
        }
    }

    fn pide_objeto(&self) -> bool {
        (self.pol.is_none() && self.convertir.is_none()) || self.uniendo.is_some()
    }

    fn objeto(&mut self, cx: &mut Contexto, h: Handle, _p: P) -> Flujo {
        if let Some(v) = &mut self.uniendo {
            if !v.contains(&h) {
                v.push(h);
            }
            return Flujo::Sigue;
        }
        match cx.dibujo.entidad(h) {
            Some(EntityType::LwPolyline(_)) => self.pol = Some(h),
            Some(EntityType::Line(_) | EntityType::Arc(_)) => self.convertir = Some(h),
            _ => cx.decir("No es una polilinea ni se puede convertir en una"),
        }
        Flujo::Sigue
    }

    fn recuadro(&mut self, _cx: &mut Contexto, hs: Vec<Handle>, _a: P, _b: P) -> Flujo {
        let Some(v) = &mut self.uniendo else {
            return Flujo::NoEntiende;
        };
        for h in hs {
            if !v.contains(&h) {
                v.push(h);
            }
        }
        Flujo::Sigue
    }

    fn texto(&mut self, cx: &mut Contexto, t: &str) -> Flujo {
        let o = opcion(t);
        if let Some(h) = self.convertir {
            return match o.as_str() {
                "S" | "SI" | "Y" | "YES" => {
                    self.convertir(cx, h);
                    Flujo::Sigue
                }
                "N" | "NO" => Flujo::Termina,
                _ => Flujo::NoEntiende,
            };
        }
        if self.pol.is_none() || self.uniendo.is_some() {
            return Flujo::NoEntiende;
        }
        match o.as_str() {
            "C" | "CL" | "CERRAR" | "CLOSE" => self.cerrar(cx, true),
            "A" | "O" | "ABRIR" | "OPEN" => self.cerrar(cx, false),
            "U" | "J" | "UNIR" | "JOIN" => {
                if self.cerrada(cx) == Some(true) {
                    cx.decir("Una polilinea cerrada no se puede unir");
                } else {
                    self.uniendo = Some(Vec::new());
                }
            }
            _ => return Flujo::NoEntiende,
        }
        Flujo::Sigue
    }

    fn enter(&mut self, cx: &mut Contexto) -> Flujo {
        if let Some(h) = self.convertir {
            self.convertir(cx, h);
            return Flujo::Sigue;
        }
        if let Some(v) = self.uniendo.take() {
            self.unir(cx, v);
            return Flujo::Sigue;
        }
        Flujo::Termina
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::{Dibujo, Sesion};

    /// Una sesion con estas entidades ya dibujadas.
    fn con(v: Vec<EntityType>) -> (Sesion, Vec<Handle>) {
        let mut d = Dibujo::nuevo();
        let hs = d.editar("PREPARAR", |t| {
            v.into_iter().filter_map(|e| t.anadir(e)).collect()
        });
        (Sesion::nueva(d), hs)
    }

    fn todas(s: &Sesion) -> Vec<EntityType> {
        s.dibujo.doc.model_space_entities().cloned().collect()
    }

    fn rayas(s: &Sesion) -> Vec<(P, P)> {
        todas(s).iter().filter_map(op::extremos_de_linea).collect()
    }

    fn cerca(a: P, b: P) -> bool {
        geo::dist(a, b) < 1e-6
    }

    #[test]
    fn tramos_partes_y_situar() {
        // Media vuelta antihoraria de (10,0) a (-10,0).
        let t = Tramo {
            a: [10.0, 0.0],
            b: [-10.0, 0.0],
            bulge: 1.0,
        };
        assert!((t.longitud() - PI * 10.0).abs() < 1e-9);
        assert!(cerca(t.punto(0.5), [0.0, 10.0]));
        let c = vec![t, Tramo::recto([-10.0, 0.0], [-10.0, -5.0])];
        assert!((situar(&c, [0.0, 12.0]) - PI * 5.0).abs() < 1e-9);
        let k = trozo(&c, PI * 5.0, PI * 10.0 + 2.0);
        assert_eq!(k.len(), 2);
        assert!(cerca(k[0].a, [0.0, 10.0]) && cerca(k[1].b, [-10.0, -2.0]));
        assert!(cerca(invertir(&c)[0].a, [-10.0, -5.0]));
    }

    #[test]
    fn estirar_mueve_lo_de_dentro_de_la_ventana() {
        let (mut s, hs) = con(vec![
            esp::linea([0.0, 0.0], [100.0, 0.0]),
            esp::linea([100.0, 0.0], [100.0, 50.0]),
            esp::circulo([200.0, 200.0], 5.0),
            esp::polilinea(&[[0.0, 10.0], [100.0, 10.0], [0.0, 40.0]], false),
        ]);
        s.escribir("s");
        assert!(s.pide_objetos());
        // Un clic no vale: hace falta ventana.
        s.clic([50.0, 0.0], 1.0);
        assert!(s.registro.last().unwrap().contains("ventana"));
        s.recuadro([110.0, 60.0], [90.0, -10.0]);
        s.escribir("");
        s.escribir("0,0");
        assert!(s.previa([20.0, 0.0]).formas.len() >= 3);
        s.escribir("20,0");
        assert_eq!(s.activa(), None);
        let EntityType::Line(l) = s.dibujo.entidad(hs[0]).unwrap() else {
            panic!()
        };
        assert!(cerca(p2(&l.start), [0.0, 0.0]) && cerca(p2(&l.end), [120.0, 0.0]));
        let EntityType::Line(l) = s.dibujo.entidad(hs[1]).unwrap() else {
            panic!()
        };
        assert!(cerca(p2(&l.start), [120.0, 0.0]) && cerca(p2(&l.end), [120.0, 50.0]));
        let EntityType::Circle(c) = s.dibujo.entidad(hs[2]).unwrap() else {
            panic!()
        };
        assert_eq!(c.center.x, 200.0);
        let EntityType::LwPolyline(p) = s.dibujo.entidad(hs[3]).unwrap() else {
            panic!()
        };
        assert_eq!(p.vertices[1].location.x, 120.0);
        assert_eq!(p.vertices[0].location.x, 0.0);
        // Un solo paso de deshacer.
        s.escribir("u");
        let EntityType::Line(l) = s.dibujo.entidad(hs[1]).unwrap() else {
            panic!()
        };
        assert_eq!(l.start.x, 100.0);
    }

    #[test]
    fn estirar_un_arco_por_un_extremo() {
        let (mut s, hs) = con(vec![esp::arco([0.0, 0.0], 100.0, 0.0, PI / 2.0)]);
        s.escribir("stretch");
        s.recuadro([105.0, 5.0], [95.0, -5.0]);
        s.escribir("");
        s.escribir("0,0");
        s.escribir("@10,0");
        let f = formas::formas(s.dibujo.entidad(hs[0]).unwrap());
        let ext = f[0].extremos();
        assert!(cerca(ext[0], [110.0, 0.0]), "{ext:?}");
        assert!(cerca(ext[1], [0.0, 100.0]), "{ext:?}");
    }

    #[test]
    fn estirar_sin_ventana_no_hace_nada() {
        let (mut s, _) = con(vec![esp::linea([0.0, 0.0], [100.0, 0.0])]);
        let v = s.dibujo.version();
        s.escribir("s");
        s.escribir("");
        assert_eq!(s.activa(), None);
        assert_eq!(s.dibujo.version(), v);
        assert!(s.registro.last().unwrap().contains("No hay"));
    }

    #[test]
    fn partir_una_raya_entre_dos_puntos_y_en_un_punto() {
        let (mut s, _) = con(vec![esp::linea([0.0, 0.0], [100.0, 0.0])]);
        s.escribir("br");
        s.clic([20.0, 0.0], 1.0);
        assert_eq!(s.previa([80.0, 0.0]).formas.len(), 2);
        s.escribir("80,0");
        let mut r = rayas(&s);
        r.sort_by(|a, b| a.0[0].total_cmp(&b.0[0]));
        assert_eq!(r.len(), 2);
        assert!(cerca(r[0].1, [20.0, 0.0]) && cerca(r[1].0, [80.0, 0.0]));
        // En un punto con @: dos piezas que se tocan.
        s.escribir("break");
        s.clic([10.0, 0.0], 1.0);
        s.escribir("@");
        assert_eq!(rayas(&s).len(), 3);
        assert!(
            rayas(&s)
                .iter()
                .any(|(a, b)| cerca(*a, [0.0, 0.0]) && cerca(*b, [10.0, 0.0]))
        );
    }

    #[test]
    fn partir_un_circulo_y_una_polilinea_cerrada() {
        let (mut s, hs) = con(vec![
            esp::circulo([0.0, 0.0], 10.0),
            esp::polilinea(
                &[[100.0, 0.0], [200.0, 0.0], [200.0, 100.0], [100.0, 100.0]],
                true,
            ),
        ]);
        // Caso negativo: un circulo en un solo punto.
        s.escribir("br");
        s.clic([10.0, 0.0], 1.0);
        s.escribir("@");
        assert!(s.registro.last().unwrap().contains("solo punto"));
        s.escape();
        s.escribir("br");
        s.clic([10.0, 0.0], 1.0);
        s.escribir("0,10");
        let EntityType::Arc(a) = s.dibujo.entidad(hs[0]).unwrap() else {
            panic!()
        };
        assert!((a.start_angle - PI / 2.0).abs() < 1e-9);
        assert!(geo::normalizar(a.end_angle).abs() < 1e-9);
        // La polilinea: se quita de (150,0) a (200,50) y queda abierta.
        s.escribir("br");
        s.clic([150.0, 0.0], 1.0);
        s.escribir("p");
        s.escribir("150,0");
        s.escribir("200,50");
        let EntityType::LwPolyline(p) = s.dibujo.entidad(hs[1]).unwrap() else {
            panic!()
        };
        assert!(!p.is_closed);
        let (l, _) = formas::largo_y_area(s.dibujo.entidad(hs[1]).unwrap());
        assert!((l - 300.0).abs() < 1e-6, "{l}");
    }

    #[test]
    fn partir_un_texto_no_se_puede() {
        let (mut s, _) = con(vec![esp::texto("HOLA", [0.0, 0.0], 10.0, 0.0, false)]);
        s.escribir("br");
        s.clic([5.0, 5.0], 1.0);
        assert!(s.registro.last().unwrap().contains("Solo se parten"));
        assert!(s.pide_objetos());
    }

    #[test]
    fn unir_rayas_alineadas_en_una() {
        let (mut s, _) = con(vec![
            esp::linea([0.0, 0.0], [50.0, 0.0]),
            esp::linea([60.0, 0.0], [100.0, 0.0]),
        ]);
        s.elegir_todo();
        s.escribir("j");
        assert_eq!(s.activa(), None);
        let r = rayas(&s);
        assert_eq!(r.len(), 1);
        assert!(cerca(r[0].0, [0.0, 0.0]) && cerca(r[0].1, [100.0, 0.0]));
    }

    #[test]
    fn unir_rayas_y_arco_en_una_polilinea_cerrada() {
        let (mut s, _) = con(vec![
            esp::linea([0.0, 0.0], [100.0, 0.0]),
            esp::arco([100.0, 50.0], 50.0, -PI / 2.0, PI / 2.0),
            esp::linea([0.0, 100.0], [100.0, 100.0]),
            esp::linea([0.0, 100.0], [0.0, 0.0]),
        ]);
        s.escribir("unir");
        s.recuadro([-10.0, -10.0], [200.0, 200.0]);
        s.escribir("");
        let v = todas(&s);
        assert_eq!(v.len(), 1);
        let EntityType::LwPolyline(p) = &v[0] else {
            panic!("{v:?}")
        };
        assert!(p.is_closed);
        let (_, area) = formas::largo_y_area(&v[0]);
        let esperada = 100.0 * 100.0 + PI * 50.0 * 50.0 / 2.0;
        assert!((area.unwrap() - esperada).abs() < 1e-6, "{area:?}");
        s.escribir("u");
        assert_eq!(todas(&s).len(), 4);
    }

    #[test]
    fn unir_lo_que_no_se_toca_no_hace_nada() {
        let (mut s, _) = con(vec![
            esp::linea([0.0, 0.0], [100.0, 0.0]),
            esp::linea([0.0, 50.0], [100.0, 80.0]),
        ]);
        s.elegir_todo();
        s.escribir("join");
        assert!(s.registro.last().unwrap().contains("Nada que unir"));
        assert_eq!(todas(&s).len(), 2);
        // Uno solo tampoco.
        let (mut s, hs) = con(vec![esp::linea([0.0, 0.0], [100.0, 0.0])]);
        s.seleccion = hs;
        s.escribir("j");
        assert!(s.registro.last().unwrap().contains("al menos dos"));
    }

    #[test]
    fn longitud_delta_total_y_porcentaje() {
        let (mut s, hs) = con(vec![
            esp::linea([0.0, 0.0], [100.0, 0.0]),
            esp::arco([0.0, 0.0], 10.0, 0.0, PI / 2.0),
        ]);
        s.escribir("len");
        s.clic([50.0, 0.0], 1.0);
        assert!(s.registro.last().unwrap().contains("100"));
        s.escribir("de");
        s.escribir("20");
        s.clic([95.0, 0.0], 1.0);
        let fin = |s: &Sesion| op::extremos_de_linea(s.dibujo.entidad(hs[0]).unwrap()).unwrap();
        assert!(cerca(fin(&s).1, [120.0, 0.0]));
        s.escribir("t");
        s.escribir("50");
        s.clic([5.0, 0.0], 1.0);
        assert!(cerca(fin(&s).0, [70.0, 0.0]));
        s.escribir("p");
        s.escribir("200");
        s.clic([110.0, 0.0], 1.0);
        assert!(cerca(fin(&s).1, [170.0, 0.0]));
        // El arco, por su final (a 90 grados), se alarga 90 grados mas.
        s.escribir("t");
        s.escribir(&format!("{}", PI * 10.0));
        s.clic(geo::polar([0.0, 0.0], 10.0, 1.4), 1.0);
        let EntityType::Arc(a) = s.dibujo.entidad(hs[1]).unwrap() else {
            panic!()
        };
        assert!(a.start_angle.abs() < 1e-9 && (a.end_angle - PI).abs() < 1e-9);
    }

    #[test]
    fn longitud_rechaza_lo_que_no_vale() {
        let (mut s, hs) = con(vec![
            esp::linea([0.0, 0.0], [100.0, 0.0]),
            esp::circulo([500.0, 0.0], 10.0),
        ]);
        s.escribir("lengthen");
        s.escribir("t");
        s.escribir("-5");
        assert!(s.registro.last().unwrap().contains("mayor que cero"));
        s.escribir("x");
        assert!(s.registro.last().unwrap().contains("no valida"));
        s.escribir("de");
        s.escribir("-100");
        s.clic([90.0, 0.0], 1.0);
        assert!(s.registro.last().unwrap().contains("sin longitud"));
        s.clic([510.0, 0.0], 1.0);
        assert!(s.registro.last().unwrap().contains("Solo se alargan"));
        let (a, b) = op::extremos_de_linea(s.dibujo.entidad(hs[0]).unwrap()).unwrap();
        assert!(cerca(a, [0.0, 0.0]) && cerca(b, [100.0, 0.0]));
    }

    #[test]
    fn alinear_con_dos_pares_y_escala() {
        let (mut s, hs) = con(vec![esp::linea([0.0, 0.0], [10.0, 0.0])]);
        s.elegir_todo();
        s.escribir("ali");
        s.escribir("0,0");
        s.escribir("100,100");
        // Caso negativo: el segundo origen encima del primero.
        s.escribir("0,0");
        assert!(s.registro.last().unwrap().contains("no puede coincidir"));
        s.escribir("10,0");
        assert!(!s.previa([100.0, 150.0]).formas.is_empty());
        s.escribir("100,120");
        s.escribir("");
        let (a, b) = op::extremos_de_linea(s.dibujo.entidad(hs[0]).unwrap()).unwrap();
        assert!(
            cerca(a, [100.0, 100.0]) && cerca(b, [100.0, 110.0]),
            "{a:?} {b:?}"
        );
        s.escribir("u");
        s.elegir_todo();
        s.escribir("align");
        for p in ["0,0", "100,100", "10,0", "100,120"] {
            s.escribir(p);
        }
        s.escribir("s");
        let (a, b) = op::extremos_de_linea(s.dibujo.entidad(hs[0]).unwrap()).unwrap();
        assert!(
            cerca(a, [100.0, 100.0]) && cerca(b, [100.0, 120.0]),
            "{a:?} {b:?}"
        );
    }

    #[test]
    fn igualar_capa_y_color() {
        let mut fuente = esp::linea([0.0, 0.0], [100.0, 0.0]);
        fuente.common_mut().layer = esp::CAPA_MUROS.into();
        fuente.common_mut().color = opencadcodec::Color::from_index(3);
        fuente.common_mut().linetype_scale = 2.0;
        let (mut s, hs) = con(vec![
            fuente,
            esp::linea([0.0, 50.0], [100.0, 50.0]),
            esp::circulo([300.0, 0.0], 10.0),
        ]);
        s.escribir("ma");
        // Caso negativo: un recuadro antes del origen no cambia nada.
        s.recuadro([-10.0, -10.0], [400.0, 100.0]);
        assert!(s.registro.last().unwrap().contains("primero"));
        s.clic([50.0, 0.0], 1.0);
        s.clic([50.0, 50.0], 1.0);
        s.recuadro([280.0, -20.0], [320.0, 20.0]);
        s.escribir("");
        assert_eq!(s.activa(), None);
        for h in &hs[1..] {
            let c = s.dibujo.entidad(*h).unwrap().common();
            assert_eq!(c.layer, esp::CAPA_MUROS);
            assert_eq!(c.color, opencadcodec::Color::from_index(3));
            assert_eq!(c.linetype_scale, 2.0);
        }
    }

    #[test]
    fn editarpol_convierte_cierra_y_abre() {
        let (mut s, hs) = con(vec![esp::linea([0.0, 0.0], [100.0, 0.0])]);
        s.escribir("pe");
        s.clic([50.0, 0.0], 1.0);
        assert!(s.indicacion().contains("Convertirlo"));
        s.escribir("");
        assert!(matches!(
            s.dibujo.entidad(hs[0]),
            Some(EntityType::LwPolyline(_))
        ));
        s.escribir("c");
        let cerrada = |s: &Sesion| match s.dibujo.entidad(hs[0]) {
            Some(EntityType::LwPolyline(p)) => p.is_closed,
            _ => panic!(),
        };
        assert!(cerrada(&s));
        // Caso negativo: unir a una cerrada.
        s.escribir("u");
        assert!(s.registro.last().unwrap().contains("cerrada"));
        s.escribir("a");
        assert!(!cerrada(&s));
        s.escribir("zz");
        assert!(s.registro.last().unwrap().contains("no valida"));
        s.escribir("");
        assert_eq!(s.activa(), None);
    }

    #[test]
    fn editarpol_une_una_raya() {
        let (mut s, hs) = con(vec![
            esp::polilinea(&[[0.0, 0.0], [100.0, 0.0]], false),
            esp::linea([100.0, 0.0], [100.0, 100.0]),
            esp::linea([500.0, 0.0], [600.0, 0.0]),
            esp::texto("X", [700.0, 0.0], 10.0, 0.0, false),
        ]);
        s.escribir("pedit");
        // Un texto no es polilinea.
        s.clic([702.0, 2.0], 1.0);
        assert!(s.registro.last().unwrap().contains("No es una polilinea"));
        s.clic([50.0, 0.0], 1.0);
        s.escribir("u");
        s.clic([100.0, 50.0], 1.0);
        s.clic([550.0, 0.0], 1.0);
        s.escribir("");
        let EntityType::LwPolyline(p) = s.dibujo.entidad(hs[0]).unwrap() else {
            panic!()
        };
        assert_eq!(p.vertices.len(), 3);
        assert!(s.dibujo.entidad(hs[1]).is_none());
        assert!(s.dibujo.entidad(hs[2]).is_some());
    }
}
