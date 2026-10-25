//! **Las cotas y los sombreados asociativos** (como DIMASSOC=2 y los
//! sombreados asociativos de AutoCAD): una cota recuerda que punto de que
//! entidad mide y un sombreado, que entidades son su borde; al cambiar esas
//! entidades (mover, estirar, girar, un pinzamiento…) la cota y el sombreado
//! se rehacen en la misma orden (un solo paso de deshacer).
//!
//! Se guarda en el plano, asi que sobrevive a guardar y abrir:
//!
//! - La cota, en su XDATA de la aplicacion [`APP`]: `"COTA"` y, por cada
//!   punto asociado, cual es (1070), que se toma de la entidad (1070), su
//!   handle (1005) y el vertice o tramo (1071).
//! - El sombreado, como AutoCAD: `is_associative` y los handles de los
//!   bordes en cada camino; su punto de dentro en `seed_points`, y la
//!   tolerancia de huecos en su XDATA de [`APP`] (`"SOMBREADO"`, 1040).
//!
//! Si se borra lo medido (o el borde), la cota o el sombreado se quedan como
//! estan, pero ya no asociativos. Si se mueve la cota (o el punto de la cota)
//! por si sola, ese punto deja de estar asociado.

use opencadcodec::entities::{Dimension, Hatch};
use opencadcodec::tables::AppId;
use opencadcodec::xdata::{ExtendedDataRecord, XDataValue};
use opencadcodec::{CadDocument, EntityType, Handle};

use crate::dibujo::{Dibujo, PorHandle, Transaccion};
use crate::formas::{self, p2};
use crate::geo::{self, Forma, P, Semejanza};
use crate::patrones::{self, Anillo};

/// El nombre de aplicacion de la XDATA de CAD-pixpin (en la tabla APPID).
pub const APP: &str = "PIXPIN";

/// **De que se toma un punto** de la cota.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Referencia {
    /// El primer punto (de una linea, un arco, una polilinea, un punto, un
    /// INSERT).
    Inicio(Handle),
    /// El ultimo.
    Fin(Handle),
    /// El centro (de un circulo, un arco o una elipse).
    Centro(Handle),
    /// El vertice `i` de una polilinea.
    Vertice(Handle, u32),
    /// El circulo o el arco entero (las de radio y diametro).
    Circulo(Handle),
    /// El tramo recto `i` (de la linea, 0, o del vertice `i` al siguiente de
    /// una polilinea): las angulares.
    Tramo(Handle, u32),
}

impl Referencia {
    pub fn handle(&self) -> Handle {
        match *self {
            Referencia::Inicio(h)
            | Referencia::Fin(h)
            | Referencia::Centro(h)
            | Referencia::Vertice(h, _)
            | Referencia::Circulo(h)
            | Referencia::Tramo(h, _) => h,
        }
    }

    fn codigo(&self) -> (i16, i32) {
        match *self {
            Referencia::Inicio(_) => (0, 0),
            Referencia::Fin(_) => (1, 0),
            Referencia::Centro(_) => (2, 0),
            Referencia::Vertice(_, i) => (3, i as i32),
            Referencia::Circulo(_) => (4, 0),
            Referencia::Tramo(_, i) => (5, i as i32),
        }
    }

    fn de_codigo(tipo: i16, h: Handle, i: i32) -> Option<Referencia> {
        let i = u32::try_from(i).ok()?;
        Some(match tipo {
            0 => Referencia::Inicio(h),
            1 => Referencia::Fin(h),
            2 => Referencia::Centro(h),
            3 => Referencia::Vertice(h, i),
            4 => Referencia::Circulo(h),
            5 => Referencia::Tramo(h, i),
            _ => return None,
        })
    }
}

/// **Un punto asociado de una cota**: cual (0 el primero, 1 el segundo; en
/// las de radio y diametro, 0 el circulo; en las angulares, 0 y 1 sus dos
/// lineas) y de que se toma.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Asociacion {
    pub cual: u8,
    pub de: Referencia,
}

/// **Lo que se toma de una entidad** para una cota.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Objetivo {
    Punto(P),
    Circulo(P, f64),
    Tramo(P, P),
}

// ------------------------------------------------------------- la XDATA

/// **Registra la aplicacion** [`APP`] en el dibujo (sin ella el DWG no
/// guarda la XDATA).
pub fn asegurar_app(doc: &mut CadDocument) {
    if !doc.app_ids.contains(APP) {
        let mut a = AppId::new(APP);
        a.handle = doc.allocate_handle();
        let _ = doc.app_ids.add(a);
    }
}

/// El handle de la aplicacion (para quitar la copia literal que trae un DWG
/// leido, que si no ganaria a la nueva al guardar).
pub fn app_handle(doc: &CadDocument) -> Option<u64> {
    doc.app_ids.get(APP).map(|a| a.handle.value())
}

/// Pone (o quita, si `None`) el registro de [`APP`] de una entidad.
fn poner_registro(app: Option<u64>, e: &mut EntityType, r: Option<ExtendedDataRecord>) {
    let xd = &mut e.common_mut().extended_data;
    if let Some(a) = app {
        xd.raw_dwg_eed.retain(|(h, _)| *h != a);
    }
    match r {
        Some(r) => xd.upsert_record(r),
        None => {
            xd.remove_record(APP);
        }
    }
}

fn registro(e: &EntityType) -> Option<&ExtendedDataRecord> {
    e.common().extended_data.get_record(APP)
}

/// **Los puntos asociados de una cota** (vacio si no es asociativa).
pub fn leer_cota(e: &EntityType) -> Vec<Asociacion> {
    let Some(r) = registro(e) else {
        return Vec::new();
    };
    let v = &r.values;
    if !matches!(v.first(), Some(XDataValue::String(s)) if s == "COTA") {
        return Vec::new();
    }
    let mut out = Vec::new();
    for c in v[1..].chunks_exact(4) {
        if let [
            XDataValue::Integer16(cual),
            XDataValue::Integer16(tipo),
            XDataValue::Handle(h),
            XDataValue::Integer32(i),
        ] = c
            && let Some(de) = Referencia::de_codigo(*tipo, *h, *i)
            && (0..=1).contains(cual)
        {
            out.push(Asociacion {
                cual: *cual as u8,
                de,
            });
        }
    }
    out
}

/// El registro de los puntos asociados de una cota (`None` si no hay).
fn registro_de_cota(asocs: &[Asociacion]) -> Option<ExtendedDataRecord> {
    if asocs.is_empty() {
        return None;
    }
    let mut r = ExtendedDataRecord::new(APP);
    r.add_value(XDataValue::String("COTA".into()));
    for a in asocs {
        let (tipo, i) = a.de.codigo();
        r.add_value(XDataValue::Integer16(a.cual as i16));
        r.add_value(XDataValue::Integer16(tipo));
        r.add_value(XDataValue::Handle(a.de.handle()));
        r.add_value(XDataValue::Integer32(i));
    }
    Some(r)
}

/// **Deja asociada la cota** `e` a esos puntos (o la desasocia si no hay).
/// `app`: [`app_handle`] del dibujo.
pub fn escribir_cota(app: Option<u64>, e: &mut EntityType, asocs: &[Asociacion]) {
    poner_registro(app, e, registro_de_cota(asocs));
}

/// La tolerancia de huecos con que se hizo un sombreado (0 si no se sabe).
pub fn hueco_de(e: &EntityType) -> f64 {
    match registro(e).map(|r| &r.values[..]) {
        Some([XDataValue::String(s), XDataValue::Real(t), ..]) if s == "SOMBREADO" => *t,
        _ => 0.0,
    }
}

/// **Deja asociado el sombreado** `e` a sus bordes: `bordes[k]` son las
/// entidades del camino `k`. Con `hueco` (la tolerancia con que se busco el
/// contorno) se rehace igual. `app`: [`app_handle`] del dibujo.
pub fn asociar_sombreado(app: Option<u64>, e: &mut EntityType, bordes: &[Vec<Handle>], hueco: f64) {
    let EntityType::Hatch(h) = e else {
        return;
    };
    let hay = bordes.iter().any(|b| !b.is_empty()) && bordes.len() == h.paths.len();
    h.is_associative = hay;
    for (k, p) in h.paths.iter_mut().enumerate() {
        p.boundary_handles = if hay { bordes[k].clone() } else { Vec::new() };
    }
    let r = hay.then(|| {
        let mut r = ExtendedDataRecord::new(APP);
        r.add_value(XDataValue::String("SOMBREADO".into()));
        r.add_value(XDataValue::Real(hueco));
        r
    });
    poner_registro(app, e, r);
}

/// **Quita las asociaciones** de una entidad que se copia a otro sitio (a un
/// bloque, al portapapeles): una cota o un sombreado que ya no siguen a
/// nada.
pub fn soltar(e: &mut EntityType) {
    match e {
        EntityType::Dimension(_) => {
            if !leer_cota(e).is_empty() {
                e.common_mut().extended_data.remove_record(APP);
            }
        }
        EntityType::Hatch(h) if h.is_associative => {
            h.is_associative = false;
            for p in &mut h.paths {
                p.boundary_handles.clear();
            }
            e.common_mut().extended_data.remove_record(APP);
        }
        _ => {}
    }
}

/// Los bordes de un sombreado asociativo (sin repetir).
fn bordes_de(h: &Hatch) -> Vec<Handle> {
    let mut v: Vec<Handle> = Vec::new();
    if h.is_associative {
        for b in h.paths.iter().flat_map(|p| &p.boundary_handles) {
            if !b.is_null() && !v.contains(b) {
                v.push(*b);
            }
        }
    }
    v
}

/// De que entidades depende `e` (si es una cota o un sombreado asociativo).
fn referentes(e: &EntityType) -> Vec<Handle> {
    match e {
        EntityType::Dimension(_) => {
            let mut v: Vec<Handle> = leer_cota(e).iter().map(|a| a.de.handle()).collect();
            v.dedup();
            v
        }
        EntityType::Hatch(h) => bordes_de(h),
        _ => Vec::new(),
    }
}

// ------------------------------------------------------------- el mapa

/// **Que depende de que**: para saber, al cambiar una entidad, que cotas y
/// sombreados hay que rehacer sin recorrer el dibujo. Se hace una vez y se
/// pone al dia con lo que cambia en cada orden.
#[derive(Debug, Default)]
pub struct Mapa {
    /// Lo que depende de cada entidad.
    de: PorHandle<Vec<Handle>>,
    /// De que depende cada cota o sombreado.
    usa: PorHandle<Vec<Handle>>,
}

impl Mapa {
    /// **El mapa de un documento** (lo recorre una vez).
    pub fn de(doc: &CadDocument) -> Mapa {
        let mut m = Mapa::default();
        for e in doc.entities() {
            if matches!(e, EntityType::Dimension(_) | EntityType::Hatch(_)) {
                m.poner(e.common().handle, Some(e));
            }
        }
        m
    }

    /// Pone al dia la entidad `h`, que quedo como `e` (`None`: se quito).
    pub fn poner(&mut self, h: Handle, e: Option<&EntityType>) {
        if let Some(viejos) = self.usa.remove(&h) {
            for r in viejos {
                if let Some(v) = self.de.get_mut(&r) {
                    v.retain(|x| *x != h);
                    if v.is_empty() {
                        self.de.remove(&r);
                    }
                }
            }
        }
        let Some(e) = e else { return };
        let rs = referentes(e);
        if rs.is_empty() {
            return;
        }
        for r in &rs {
            self.de.entry(*r).or_default().push(h);
        }
        self.usa.insert(h, rs);
    }

    /// Lo que depende de `h`.
    pub fn dependientes(&self, h: Handle) -> &[Handle] {
        self.de.get(&h).map_or(&[], |v| &v[..])
    }

    /// Cuantas cotas y sombreados asociativos hay.
    pub fn len(&self) -> usize {
        self.usa.len()
    }

    pub fn is_empty(&self) -> bool {
        self.usa.is_empty()
    }
}

/// **Rehace lo que depende de lo cambiado en la orden** (al final de
/// [`Dibujo::editar`], dentro de la misma orden): las cotas asociadas a lo
/// que cambio o que cambiaron ellas mismas, y los sombreados de bordes
/// cambiados. Solo mira lo cambiado y lo que el mapa dice que depende de
/// ello.
pub(crate) fn propagar(t: &mut Transaccion, mapa: &Mapa) {
    let cambiadas = t.cambiadas();
    let mut vistos: PorHandle<()> = PorHandle::default();
    let mut pendientes = Vec::new();
    for h in &cambiadas {
        for d in mapa.dependientes(*h) {
            if vistos.insert(*d, ()).is_none() {
                pendientes.push(*d);
            }
        }
        let propia = match t.entidad(*h) {
            Some(EntityType::Dimension(_)) => true,
            Some(EntityType::Hatch(s)) => s.is_associative,
            _ => false,
        };
        if propia && vistos.insert(*h, ()).is_none() {
            pendientes.push(*h);
        }
    }
    for d in pendientes {
        match t.entidad(d) {
            Some(EntityType::Dimension(_)) => crate::cotas_bloque::seguir(t, d),
            Some(EntityType::Hatch(_)) => seguir_sombreado(t, d),
            _ => {}
        }
    }
}

// --------------------------------------------------- los puntos de las cotas

/// La tolerancia para dar dos puntos por el mismo cerca de `p`.
pub fn tolerancia(p: P) -> f64 {
    1e-6 * (1.0 + p[0].abs().max(p[1].abs()))
}

/// Los vertices de una polilinea ligera, en el plano (como los dibuja).
fn vertices(e: &EntityType) -> Option<(Vec<P>, bool)> {
    let EntityType::LwPolyline(p) = e else {
        return None;
    };
    let espejo = p.normal.z < 0.0;
    let v = p
        .vertices
        .iter()
        .map(|v| {
            let x = if espejo { -v.location.x } else { v.location.x };
            [x, v.location.y]
        })
        .collect();
    Some((v, p.is_closed))
}

/// **Lo que da la referencia `r` en la entidad `e`** (`None` si ya no lo
/// tiene: otro tipo, un vertice que ya no esta).
pub fn objetivo(e: &EntityType, r: Referencia) -> Option<Objetivo> {
    use Objetivo as O;
    let extremos = || {
        let f = formas::formas(e);
        let a = f.first()?.extremos().first().copied()?;
        let b = f.last()?.extremos().last().copied()?;
        Some((a, b))
    };
    Some(match (r, e) {
        (Referencia::Inicio(_), EntityType::Line(_) | EntityType::Arc(_)) => {
            O::Punto(extremos()?.0)
        }
        (Referencia::Fin(_), EntityType::Line(_) | EntityType::Arc(_)) => O::Punto(extremos()?.1),
        (Referencia::Inicio(_), EntityType::Point(p)) => O::Punto(p2(&p.location)),
        (Referencia::Inicio(_), EntityType::Insert(i)) => O::Punto(p2(&i.insert_point)),
        (Referencia::Inicio(_), EntityType::LwPolyline(_)) => O::Punto(*vertices(e)?.0.first()?),
        (Referencia::Fin(_), EntityType::LwPolyline(_)) => O::Punto(*vertices(e)?.0.last()?),
        (Referencia::Vertice(_, i), EntityType::LwPolyline(_)) => {
            O::Punto(*vertices(e)?.0.get(i as usize)?)
        }
        (Referencia::Centro(_), EntityType::Circle(_) | EntityType::Arc(_)) => {
            O::Punto(crate::cotas_bloque::circulo_de(e)?.0)
        }
        (Referencia::Centro(_), EntityType::Ellipse(el)) => O::Punto(p2(&el.center)),
        (Referencia::Circulo(_), _) => {
            let (c, r) = crate::cotas_bloque::circulo_de(e)?;
            O::Circulo(c, r)
        }
        (Referencia::Tramo(_, 0), EntityType::Line(l)) => O::Tramo(p2(&l.start), p2(&l.end)),
        (Referencia::Tramo(_, i), EntityType::LwPolyline(p)) => {
            let (v, cerrada) = vertices(e)?;
            let i = i as usize;
            let j = if i + 1 < v.len() {
                i + 1
            } else if cerrada && v.len() > 2 {
                0
            } else {
                return None;
            };
            // Un tramo con abombado ya no es recto.
            if p.vertices.get(i)?.bulge.abs() > 1e-12 {
                return None;
            }
            O::Tramo(*v.get(i)?, v[j])
        }
        _ => return None,
    })
}

/// Las referencias de punto que ofrece una entidad, con su punto.
fn referencias_de(h: Handle, e: &EntityType) -> Vec<(Referencia, P)> {
    let mut v = Vec::new();
    let mut con = |r: Referencia| {
        if let Some(Objetivo::Punto(p)) = objetivo(e, r) {
            v.push((r, p));
        }
    };
    match e {
        EntityType::Line(_) => {
            con(Referencia::Inicio(h));
            con(Referencia::Fin(h));
        }
        EntityType::Arc(_) => {
            con(Referencia::Inicio(h));
            con(Referencia::Fin(h));
            con(Referencia::Centro(h));
        }
        EntityType::Circle(_) | EntityType::Ellipse(_) => con(Referencia::Centro(h)),
        EntityType::Point(_) | EntityType::Insert(_) => con(Referencia::Inicio(h)),
        EntityType::LwPolyline(p) => {
            for i in 0..p.vertices.len() {
                con(Referencia::Vertice(h, i as u32));
            }
        }
        _ => {}
    }
    v
}

/// Lo que se puede medir cerca de `p`, en el indice.
fn cerca(d: &Dibujo, p: P, tol: f64) -> Vec<Handle> {
    let ind = d.indice();
    let zona = geo::ensanchar([p[0], p[1], p[0], p[1]], tol);
    ind.en(&zona)
        .into_iter()
        .map(|e| e.handle)
        .filter(|h| {
            !matches!(
                d.entidad(*h),
                Some(
                    EntityType::Dimension(_)
                        | EntityType::Hatch(_)
                        | EntityType::Text(_)
                        | EntityType::MText(_)
                ) | None
            )
        })
        .collect()
}

/// **El punto de una entidad que cae en `p`** (un extremo, un vertice, un
/// centro…), si lo hay: lo que el enganche dio al hacer la cota. Mejor un
/// extremo o un vertice que un centro.
pub fn referencia_en(d: &Dibujo, p: P) -> Option<Referencia> {
    let tol = tolerancia(p);
    let mut mejor: Option<(bool, f64, Referencia)> = None;
    for h in cerca(d, p, tol) {
        let Some(e) = d.entidad(h) else { continue };
        for (r, q) in referencias_de(h, e) {
            let dq = geo::dist(p, q);
            if dq > tol {
                continue;
            }
            let centro = matches!(r, Referencia::Centro(_));
            if mejor.is_none_or(|(c, m, _)| (centro, dq) < (c, m)) {
                mejor = Some((centro, dq, r));
            }
        }
    }
    mejor.map(|(_, _, r)| r)
}

/// El circulo o arco de centro `c` y radio `r` que pasa por `q`.
fn circulo_en(d: &Dibujo, c: P, r: f64, q: P) -> Option<Referencia> {
    let tol = tolerancia(c).max(tolerancia(q));
    cerca(d, q, tol)
        .into_iter()
        .find(|h| {
            d.entidad(*h)
                .and_then(crate::cotas_bloque::circulo_de)
                .is_some_and(|(c2, r2)| geo::dist(c, c2) <= tol && (r - r2).abs() <= tol)
        })
        .map(Referencia::Circulo)
}

/// El tramo recto de una entidad que va de `a` a `b` (en un sentido u otro).
fn tramo_en(d: &Dibujo, a: P, b: P) -> Option<Referencia> {
    let tol = tolerancia(a).max(tolerancia(b));
    for h in cerca(d, geo::medio(a, b), tol) {
        let Some(e) = d.entidad(h) else { continue };
        let n = match e {
            EntityType::Line(_) => 1,
            EntityType::LwPolyline(p) => p.vertices.len() as u32,
            _ => 0,
        };
        for i in 0..n {
            let r = Referencia::Tramo(h, i);
            if let Some(Objetivo::Tramo(p, q)) = objetivo(e, r)
                && ((geo::dist(p, a) <= tol && geo::dist(q, b) <= tol)
                    || (geo::dist(p, b) <= tol && geo::dist(q, a) <= tol))
            {
                return Some(r);
            }
        }
    }
    None
}

/// **A que se asocia una cota nueva**: sus puntos que caen en un punto de
/// algo dibujado (los de una lineal o alineada), el circulo que mide (radio
/// y diametro) o las lineas del angulo (angular). Lo que no cae en nada
/// queda suelto.
pub fn detectar(d: &Dibujo, dim: &Dimension) -> Vec<Asociacion> {
    let mut v = Vec::new();
    let mut con = |cual: u8, r: Option<Referencia>| {
        if let Some(de) = r {
            v.push(Asociacion { cual, de });
        }
    };
    match dim {
        Dimension::Linear(l) => {
            con(0, referencia_en(d, p2(&l.first_point)));
            con(1, referencia_en(d, p2(&l.second_point)));
        }
        Dimension::Aligned(l) => {
            con(0, referencia_en(d, p2(&l.first_point)));
            con(1, referencia_en(d, p2(&l.second_point)));
        }
        Dimension::Radius(r) => {
            let (c, q) = (p2(&r.angle_vertex), p2(&r.definition_point));
            con(0, circulo_en(d, c, geo::dist(c, q), q));
        }
        Dimension::Diameter(r) => {
            let (a, b) = (p2(&r.angle_vertex), p2(&r.definition_point));
            con(0, circulo_en(d, geo::medio(a, b), geo::dist(a, b) / 2.0, b));
        }
        Dimension::Angular2Ln(a) => {
            con(0, tramo_en(d, p2(&a.first_point), p2(&a.second_point)));
            con(1, tramo_en(d, p2(&a.angle_vertex), p2(&a.definition_point)));
        }
        _ => {}
    }
    v
}

// --------------------------------------------------------- los sombreados

/// Los puntos que fijan una entidad (los extremos de sus formas y los
/// centros), para saber si se movio sin deformarse.
fn puntos_clave(e: &EntityType) -> Vec<P> {
    let mut v = Vec::new();
    for f in formas::formas(e) {
        v.extend(f.extremos());
        if let Forma::Circulo { c, r } = f {
            v.push(c);
            v.push([c[0] + r, c[1]]);
        }
    }
    v
}

/// **La semejanza que lleva `antes` a `despues`** (punto a punto), si la
/// hay: un movimiento, un giro, una escala o una simetria.
fn semejanza(antes: &[P], despues: &[P]) -> Option<Semejanza> {
    if antes.len() != despues.len() || antes.is_empty() {
        return None;
    }
    let tol = antes
        .iter()
        .chain(despues)
        .fold(0.0f64, |m, p| m.max(tolerancia(*p)));
    let lejos = (0..antes.len())
        .max_by(|a, b| geo::dist(antes[*a], antes[0]).total_cmp(&geo::dist(antes[*b], antes[0])))
        .unwrap_or(0);
    let candidatas = if geo::dist(antes[lejos], antes[0]) <= tol {
        vec![Semejanza::mover(geo::restar(despues[0], antes[0]))]
    } else {
        let (u, w) = (
            geo::restar(antes[lejos], antes[0]),
            geo::restar(despues[lejos], despues[0]),
        );
        let k = geo::norma(w) / geo::norma(u);
        let mut v = Vec::new();
        for espejo in [false, true] {
            let u = if espejo { [u[0], -u[1]] } else { u };
            let giro = w[1].atan2(w[0]) - u[1].atan2(u[0]);
            let mut s = Semejanza {
                t: [0.0, 0.0],
                k,
                giro,
                espejo,
            };
            s.t = geo::restar(despues[0], s.aplicar(antes[0]));
            v.push(s);
        }
        v
    };
    candidatas.into_iter().find(|s| {
        antes
            .iter()
            .zip(despues)
            .all(|(a, d)| geo::dist(s.aplicar(*a), *d) <= tol * 10.0)
    })
}

/// Lleva la entidad con la semejanza `s`.
fn transformar(e: &mut EntityType, s: &Semejanza) {
    use crate::operaciones as op;
    if s.espejo {
        op::simetria(e, [0.0, 0.0], [1.0, 0.0]);
    }
    if s.giro.abs() > 1e-15 {
        op::girar(e, [0.0, 0.0], s.giro);
    }
    if (s.k - 1.0).abs() > 1e-15 {
        op::escalar(e, [0.0, 0.0], s.k);
    }
    op::trasladar(e, s.t);
}

/// **Rehace el sombreado `h`** si cambio alguno de sus bordes: si todos se
/// movieron igual (mover, girar, escalar, simetria), el sombreado se lleva
/// con ellos; si no, se vuelve a buscar su contorno con solo sus bordes
/// (desde su punto de dentro) o con los objetos cerrados que lo forman. Si
/// un borde se borro o ya no cierra, se queda como esta y deja de ser
/// asociativo. Si el que se movio es el sombreado solo, tambien.
pub(crate) fn seguir_sombreado(t: &mut Transaccion, h: Handle) {
    let Some(EntityType::Hatch(s)) = t.entidad(h).cloned() else {
        return;
    };
    let bordes = bordes_de(&s);
    if bordes.is_empty() {
        return;
    }
    let desasociar = |t: &mut Transaccion| {
        let app = app_handle(t.doc());
        t.cambiar(h, |e| asociar_sombreado(app, e, &[], 0.0));
    };
    if bordes.iter().any(|b| t.entidad(*b).is_none()) {
        desasociar(t);
        return;
    }
    let movidos: Vec<Handle> = bordes.iter().copied().filter(|b| t.cambio(*b)).collect();
    if movidos.is_empty() {
        // Solo cambio el sombreado: si fue su contorno (se movio, se estiro),
        // ya no sigue a sus bordes.
        if let Some(EntityType::Hatch(a)) = t.antes(h)
            && a.paths.len() == s.paths.len()
            && a.paths
                .iter()
                .zip(&s.paths)
                .any(|(x, y)| x.edges != y.edges)
        {
            desasociar(t);
        }
        return;
    }
    let el_tambien = t.cambio(h);
    // Todos los bordes llevados igual: el sombreado con ellos.
    if movidos.len() == bordes.len() {
        let mut comun: Option<Semejanza> = None;
        let mut todos = true;
        for b in &bordes {
            let (Some(a), Some(d)) = (t.antes(*b), t.entidad(*b)) else {
                todos = false;
                break;
            };
            let (pa, pd) = (puntos_clave(a), puntos_clave(d));
            match (comun, semejanza(&pa, &pd)) {
                (None, Some(s)) => comun = Some(s),
                (Some(c), Some(_))
                    if pa
                        .iter()
                        .zip(&pd)
                        .all(|(x, y)| geo::dist(c.aplicar(*x), *y) <= tolerancia(*y) * 10.0) => {}
                _ => {
                    todos = false;
                    break;
                }
            }
        }
        if todos && let Some(c) = comun {
            // Si el sombreado se movio con ellos ya esta donde va.
            if !el_tambien {
                t.cambiar(h, |e| transformar(e, &c));
            }
            return;
        }
    }
    // Rehacer el contorno con sus bordes de ahora.
    let hueco = t.entidad(h).map_or(0.0, hueco_de);
    let nuevo: Option<(Vec<Anillo>, Vec<Vec<Handle>>, usize)> = match s.seed_points.first() {
        // Hecho con un punto de dentro.
        Some(sem) => {
            let mut formas = Vec::new();
            for b in &bordes {
                if let Some(e) = t.entidad(*b) {
                    formas.extend(formas::formas(e).into_iter().map(|f| (f, *b)));
                }
            }
            crate::contorno::de_formas(&formas, [sem.x, sem.y], hueco).map(|c| {
                let mut anillos = vec![c.exterior];
                anillos.extend(c.islas);
                let mut de = vec![c.bordes];
                de.extend(c.bordes_islas);
                (anillos, de, 1)
            })
        }
        // Hecho con objetos cerrados: cada camino, de su objeto.
        None => {
            let mut anillos = Vec::new();
            let mut de = Vec::new();
            let mut bien = true;
            for p in &s.paths {
                match p.boundary_handles.as_slice() {
                    [b] => match t
                        .entidad(*b)
                        .and_then(crate::herramientas::sombreado::anillo_de)
                    {
                        Some(a) => {
                            anillos.push(a);
                            de.push(vec![*b]);
                        }
                        None => bien = false,
                    },
                    _ => bien = false,
                }
            }
            let exteriores = s.paths.iter().filter(|p| p.flags.is_external()).count();
            bien.then_some((anillos, de, exteriores))
        }
    };
    let Some((anillos, de, exteriores)) = nuevo else {
        desasociar(t);
        return;
    };
    let caminos = patrones::caminos(&anillos[..exteriores], &anillos[exteriores..]);
    t.cambiar(h, |e| {
        if let EntityType::Hatch(s) = e {
            s.paths = caminos;
            s.is_associative = true;
            for (p, b) in s.paths.iter_mut().zip(&de) {
                p.boundary_handles = b.clone();
            }
        }
    });
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_semejanza_de_un_movimiento_un_giro_y_una_simetria() {
        let a = [[0.0, 0.0], [10.0, 0.0], [10.0, 5.0]];
        let s = Semejanza::girar([3.0, 4.0], 0.7);
        let b: Vec<P> = a.iter().map(|p| s.aplicar(*p)).collect();
        let t = semejanza(&a, &b).unwrap();
        for (x, y) in a.iter().zip(&b) {
            assert!(geo::dist(t.aplicar(*x), *y) < 1e-9);
        }
        let m = Semejanza::simetria([0.0, 0.0], [1.0, 1.0]).unwrap();
        let b: Vec<P> = a.iter().map(|p| m.aplicar(*p)).collect();
        assert!(semejanza(&a, &b).unwrap().espejo);
        // Un punto solo: un movimiento.
        let t = semejanza(&[[1.0, 1.0]], &[[4.0, 5.0]]).unwrap();
        assert_eq!(t.t, [3.0, 4.0]);
        // Caso negativo: deformado (estirar un extremo).
        let b = [[0.0, 0.0], [12.0, 0.0], [10.0, 5.0]];
        assert!(semejanza(&a, &b).is_none());
        assert!(semejanza(&a, &b[..2]).is_none());
    }

    #[test]
    fn la_xdata_de_la_cota_va_y_vuelve() {
        let v = vec![
            Asociacion {
                cual: 0,
                de: Referencia::Vertice(Handle::new(0x2A), 3),
            },
            Asociacion {
                cual: 1,
                de: Referencia::Fin(Handle::new(0x2B)),
            },
        ];
        let mut doc = CadDocument::new();
        asegurar_app(&mut doc);
        assert!(doc.app_ids.contains(APP));
        let mut e = crate::especialidad::linea([0.0, 0.0], [1.0, 0.0]);
        let app = app_handle(&doc);
        assert!(app.is_some());
        escribir_cota(app, &mut e, &v);
        assert_eq!(leer_cota(&e), v);
        escribir_cota(app, &mut e, &[]);
        assert!(leer_cota(&e).is_empty());
        assert!(e.common().extended_data.get_record(APP).is_none());
        // Caso negativo: un registro de otro que no es de cotas.
        let mut r = ExtendedDataRecord::new(APP);
        r.add_value(XDataValue::String("OTRA".into()));
        e.common_mut().extended_data.upsert_record(r);
        assert!(leer_cota(&e).is_empty());
    }
}
