//! **Las cotas de verdad**: entidades DIMENSION con su bloque anonimo `*Dn`
//! (lineas de extension, linea de cota, marcas o flechas y el texto), como
//! las deja AutoCAD. El bloque es lo que se ve (pixpin-cad y AutoCAD pintan
//! la cota por su bloque); la entidad guarda lo medido y se puede editar.
//!
//! Tambien el estilo de cota del dibujo ([`ESTILO`]), con sus medidas en
//! milimetros de papel multiplicadas por la escala (DIMSCALE), y la eleccion
//! de un texto o una cota bajo el cursor.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use opencadcodec::entities::{
    AttachmentPoint, Dimension, DimensionAligned, DimensionAngular2Ln, DimensionDiameter,
    DimensionLinear, DimensionRadius, MText,
};
use opencadcodec::tables::{BlockRecord, DimStyle};
use opencadcodec::{CadDocument, Color, EntityType, Handle};

use crate::dibujo::{Dibujo, Transaccion};
use crate::especialidad as esp;
use crate::formas::{self, p2, v3};
use crate::geo::{self, Caja, P};

/// El nombre del estilo de cota que crean las ordenes.
pub const ESTILO: &str = "PIXPIN";

/// El alto del texto de cota en el papel, en milimetros.
const ALTO_PAPEL: f64 = 2.5;

/// **Las medidas de una cota** ya en unidades del dibujo (las del estilo por
/// su escala).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Medidas {
    /// Alto del texto (DIMTXT).
    pub texto: f64,
    /// Media marca de arquitectura (DIMTSZ); 0: flechas.
    pub marca: f64,
    /// Largo de la flecha (DIMASZ).
    pub flecha: f64,
    /// Hueco entre lo medido y la linea de extension (DIMEXO).
    pub separa: f64,
    /// Lo que la extension pasa de la linea de cota (DIMEXE).
    pub pasa: f64,
    /// Hueco entre la linea y el texto (DIMGAP).
    pub hueco: f64,
    /// Lo que la linea de cota pasa de las extensiones con marcas (DIMDLE).
    pub prolonga: f64,
    /// La separacion entre cotas de linea base (DIMDLI).
    pub escalon: f64,
    pub decimales: usize,
    pub decimales_angulo: usize,
}

impl Medidas {
    /// Las de un estilo.
    pub fn de(e: &DimStyle) -> Medidas {
        let k = if e.dimscale > 0.0 { e.dimscale } else { 1.0 };
        Medidas {
            texto: e.dimtxt * k,
            marca: e.dimtsz * k,
            flecha: e.dimasz * k,
            separa: e.dimexo * k,
            pasa: e.dimexe * k,
            hueco: e.dimgap.abs() * k,
            prolonga: e.dimdle * k,
            escalon: e.dimdli * k,
            decimales: e.dimdec.max(0) as usize,
            decimales_angulo: e.dimadec.max(0) as usize,
        }
    }

    /// Las de un dibujo sin estilo propio, para un alto de texto.
    pub fn para(alto_texto: f64, decimales: usize) -> Medidas {
        Medidas::de(&estilo_nuevo(alto_texto, decimales))
    }
}

/// Un estilo de arquitectura (marcas a 45 grados, texto encima) para que el
/// texto mida `alto_texto` en el dibujo.
fn estilo_nuevo(alto_texto: f64, decimales: usize) -> DimStyle {
    let mut e = DimStyle::new(ESTILO);
    e.dimscale = (alto_texto / ALTO_PAPEL).max(1e-6);
    e.dimtxt = ALTO_PAPEL;
    e.dimtsz = 1.25;
    e.dimasz = 2.5;
    e.dimexo = 1.25;
    e.dimexe = 1.25;
    e.dimgap = 0.8;
    e.dimdle = 1.25;
    e.dimdli = 6.0;
    e.dimtad = 1;
    e.dimtih = false;
    e.dimtoh = false;
    e.dimdec = decimales as i16;
    e.dimadec = 0;
    e.dimzin = 8;
    e.dimlunit = 2;
    e
}

/// **El estilo de cota del dibujo**: lo crea (para `alto_texto`) si no esta,
/// lo deja como el actual y devuelve sus medidas.
pub fn asegurar_estilo(doc: &mut CadDocument, alto_texto: f64, decimales: usize) -> Medidas {
    if !doc.dim_styles.contains(ESTILO) {
        let mut e = estilo_nuevo(alto_texto, decimales);
        e.handle = doc.allocate_handle();
        e.dimtxsty_handle = doc.header.current_text_style_handle;
        doc.header.current_dimstyle_handle = e.handle;
        doc.header.current_dimstyle_name = ESTILO.to_string();
        doc.header.dim_scale = e.dimscale;
        let _ = doc.dim_styles.add(e);
    }
    let e = doc.dim_styles.get(ESTILO).expect("recien creado");
    Medidas::de(e)
}

/// **Las medidas de las cotas nuevas**: las del estilo del dibujo o, si aun
/// no lo tiene, las que tendra (para `alto_texto`). Sin cambiar nada.
pub fn medidas_del_dibujo(doc: &CadDocument, alto_texto: f64, decimales: usize) -> Medidas {
    doc.dim_styles
        .get(ESTILO)
        .map_or_else(|| Medidas::para(alto_texto, decimales), Medidas::de)
}

/// La escala del estilo (DIMSCALE), si el dibujo ya lo tiene.
pub fn escala(doc: &CadDocument) -> Option<f64> {
    doc.dim_styles.get(ESTILO).map(|e| e.dimscale)
}

/// **Cambia la escala de las cotas** que se hagan desde ahora.
pub fn poner_escala(dibujo: &mut Dibujo, alto_texto: f64, decimales: usize, k: f64) {
    asegurar_estilo(&mut dibujo.doc, alto_texto, decimales);
    if let Some(e) = dibujo.doc.dim_styles.get_mut(ESTILO) {
        e.dimscale = k;
    }
    dibujo.doc.header.dim_scale = k;
    dibujo.tocar();
}

/// Las medidas con que se dibuja una cota (las de su estilo).
pub fn medidas_de(doc: &CadDocument, dim: &Dimension, por_defecto: Medidas) -> Medidas {
    doc.dim_styles
        .get(&dim.base().style_name)
        .filter(|e| e.dimtxt > 0.0)
        .map_or(por_defecto, Medidas::de)
}

// ---------------------------------------------------------------- crear cotas

/// El angulo `a` llevado a (-90, 90] grados: un texto que no queda boca abajo.
fn legible(a: f64) -> f64 {
    let mut a = geo::normalizar(a);
    if a > FRAC_PI_2 + 1e-9 {
        a -= PI;
    }
    if a > FRAC_PI_2 + 1e-9 {
        a -= PI;
    }
    a
}

/// **Cota lineal** de `p1` a `p2` medida en la direccion `rot`, con la
/// linea de cota pasando por `por`. `None` si no mide nada en esa direccion.
pub fn lineal(p1: P, p2: P, por: P, rot: f64) -> Option<Dimension> {
    let u = [rot.cos(), rot.sin()];
    let medida = geo::punto(geo::restar(p2, p1), u).abs();
    if medida < geo::EPS {
        return None;
    }
    let n = geo::perp(u);
    let c2 = geo::sumar(p2, geo::por(n, geo::punto(geo::restar(por, p2), n)));
    let mut d = DimensionLinear::rotated(v3(p1), v3(p2), rot);
    d.base.actual_measurement = medida;
    let mut dim = Dimension::Linear(d);
    dim.set_definition_point(v3(c2));
    Some(dim)
}

/// **Cota alineada** de `p1` a `p2`, con la linea de cota pasando por `por`.
pub fn alineada(p1: P, p2: P, por: P) -> Option<Dimension> {
    let medida = geo::dist(p1, p2);
    if medida < geo::EPS {
        return None;
    }
    let n = geo::perp(geo::unitario(geo::restar(p2, p1)));
    let c2 = geo::sumar(p2, geo::por(n, geo::punto(geo::restar(por, p2), n)));
    let mut d = DimensionAligned::new(v3(p1), v3(p2));
    d.base.actual_measurement = medida;
    let mut dim = Dimension::Aligned(d);
    dim.set_definition_point(v3(c2));
    Some(dim)
}

/// **Cota de radio** de un circulo o arco (centro `c`, radio `r`), hacia
/// `por` (dentro: el texto en el radio; fuera: en la prolongacion).
pub fn radio(c: P, r: f64, por: P) -> Option<Dimension> {
    if r < geo::EPS {
        return None;
    }
    let dir = direccion(c, por);
    let cuerda = geo::sumar(c, geo::por(dir, r));
    let mut d = DimensionRadius::new(v3(c), v3(cuerda));
    d.leader_length = (geo::dist(c, por) - r).max(0.0);
    d.base.actual_measurement = r;
    Some(Dimension::Radius(d))
}

/// **Cota de diametro** de un circulo o arco, en la direccion de `por`.
pub fn diametro(c: P, r: f64, por: P) -> Option<Dimension> {
    if r < geo::EPS {
        return None;
    }
    let dir = direccion(c, por);
    let a = geo::sumar(c, geo::por(dir, -r));
    let b = geo::sumar(c, geo::por(dir, r));
    let mut d = DimensionDiameter::new(v3(a), v3(b));
    d.leader_length = (geo::dist(c, por) - r).max(0.0);
    d.base.actual_measurement = 2.0 * r;
    Some(Dimension::Diameter(d))
}

fn direccion(c: P, hacia: P) -> P {
    let d = geo::restar(hacia, c);
    if geo::norma(d) < geo::EPS {
        [1.0, 0.0]
    } else {
        geo::unitario(d)
    }
}

/// **Cota angular entre dos lineas**, con el arco pasando por `por` (que
/// elige cual de los cuatro angulos se mide). `None` si son paralelas o
/// `por` cae en el vertice.
pub fn angular(l1: (P, P), l2: (P, P), por: P) -> Option<Dimension> {
    let s = sector(l1, l2, por)?;
    let mut d = DimensionAngular2Ln {
        first_point: v3(l1.0),
        second_point: v3(l1.1),
        angle_vertex: v3(l2.0),
        definition_point: v3(l2.1),
        dimension_arc: v3(por),
        ..Default::default()
    };
    d.base.actual_measurement = s.barrido;
    d.base.definition_point = v3(l2.1);
    Some(Dimension::Angular2Ln(d))
}

/// El angulo que mide una cota angular: vertice, radio del arco, angulo de
/// inicio y barrido antihorario (menos de media vuelta).
#[derive(Debug, Clone, Copy)]
struct Sector {
    v: P,
    r: f64,
    inicio: f64,
    barrido: f64,
    /// La direccion de cada lado (la del inicio y la del final).
    lados: [P; 2],
}

fn sector(l1: (P, P), l2: (P, P), por: P) -> Option<Sector> {
    let (d1, d2) = (geo::restar(l1.1, l1.0), geo::restar(l2.1, l2.0));
    if geo::norma(d1) < geo::EPS || geo::norma(d2) < geo::EPS {
        return None;
    }
    let (d1, d2) = (geo::unitario(d1), geo::unitario(d2));
    if geo::cruz(d1, d2).abs() < 1e-9 {
        return None;
    }
    let v = geo::corte_rectas(l1.0, l1.1, l2.0, l2.1)?;
    let r = geo::dist(v, por);
    if r < geo::EPS {
        return None;
    }
    let aq = geo::angulo(v, por);
    let mut lados = Vec::new();
    for a in [d1, geo::por(d1, -1.0)] {
        for b in [d2, geo::por(d2, -1.0)] {
            lados.push((a, b));
            lados.push((b, a));
        }
    }
    for (a, b) in lados {
        let inicio = geo::normalizar(a[1].atan2(a[0]));
        let barrido = geo::desde(inicio, b[1].atan2(b[0]));
        if barrido > geo::EPS && barrido < PI && geo::desde(inicio, aq) <= barrido + 1e-12 {
            return Some(Sector {
                v,
                r,
                inicio,
                barrido,
                lados: [a, b],
            });
        }
    }
    None
}

// -------------------------------------------------------- la cota dibujada

/// Lo medido, como texto (sin el texto propio).
pub fn texto_medido(dim: &Dimension, m: &Medidas) -> String {
    let b = dim.base();
    match dim {
        Dimension::Angular2Ln(_) | Dimension::Angular3Pt(_) => {
            let g = b.actual_measurement.to_degrees();
            format!("{}%%d", esp::texto_de_medida(g, m.decimales_angulo))
        }
        Dimension::Radius(_) => format!(
            "R{}",
            esp::texto_de_medida(b.actual_measurement, m.decimales)
        ),
        Dimension::Diameter(_) => {
            format!(
                "%%c{}",
                esp::texto_de_medida(b.actual_measurement, m.decimales)
            )
        }
        _ => esp::texto_de_medida(b.actual_measurement, m.decimales),
    }
}

/// **El texto que se ve** en la cota: el propio (con `<>` cambiado por lo
/// medido) o lo medido.
pub fn texto_de_cota(dim: &Dimension, m: &Medidas) -> String {
    let medido = texto_medido(dim, m);
    match dim.base().text_override() {
        Some(t) if !t.is_empty() => t.replace("<>", &medido),
        _ => medido,
    }
}

/// Una pieza del bloque: en la capa 0 y por bloque, para que tome la capa y
/// el color de la cota.
fn pieza(mut e: EntityType) -> EntityType {
    let c = e.common_mut();
    c.layer = "0".into();
    c.color = Color::ByBlock;
    e
}

fn raya(a: P, b: P) -> EntityType {
    pieza(esp::linea(a, b))
}

/// Una flecha rellena con la punta en `punta`, apuntando hacia `dir`.
fn flecha(punta: P, dir: P, largo: f64) -> EntityType {
    let atras = geo::sumar(punta, geo::por(dir, -largo));
    let w = geo::por(geo::perp(dir), largo / 6.0);
    pieza(esp::relleno(
        punta,
        geo::sumar(atras, w),
        geo::restar(atras, w),
    ))
}

/// La marca de arquitectura (un trazo a 45 grados) en `c`, sobre una linea
/// de direccion `dir`.
fn marca(c: P, dir: P, media: f64) -> EntityType {
    let m = geo::por(geo::unitario(geo::sumar(dir, geo::perp(dir))), media);
    raya(geo::restar(c, m), geo::sumar(c, m))
}

/// Un texto centrado en `p`, girado `rot`.
fn texto(valor: &str, p: P, alto: f64, rot: f64) -> EntityType {
    let mut t = MText::with_value(valor, v3(p)).with_height(alto);
    t.rectangle_width = 0.0;
    t.rotation = rot;
    t.attachment_point = AttachmentPoint::MiddleCenter;
    pieza(EntityType::MText(t))
}

/// Un texto sobre una linea de direccion `dir` que pasa por `p`: legible y
/// del lado de arriba (al leerlo). Da la entidad y su centro.
fn texto_sobre(valor: &str, p: P, dir: P, m: &Medidas) -> (EntityType, P) {
    let rot = legible(dir[1].atan2(dir[0]));
    let arriba = geo::perp([rot.cos(), rot.sin()]);
    let c = geo::sumar(p, geo::por(arriba, m.hueco + m.texto / 2.0));
    (texto(valor, c, m.texto, rot), c)
}

/// Una linea de extension de `p` (lo medido) a `c` (la linea de cota).
fn extension(p: P, c: P, m: &Medidas, v: &mut Vec<EntityType>) {
    let d = geo::dist(p, c);
    if d <= m.separa {
        return;
    }
    let u = geo::unitario(geo::restar(c, p));
    v.push(raya(
        geo::sumar(p, geo::por(u, m.separa)),
        geo::sumar(c, geo::por(u, m.pasa)),
    ));
}

/// **Lo que se ve de una cota** (las piezas de su bloque, en coordenadas
/// del dibujo) y el centro de su texto.
pub fn dibujo_de_cota(dim: &Dimension, m: &Medidas) -> (Vec<EntityType>, P) {
    let valor = texto_de_cota(dim, m);
    let mut v = Vec::new();
    let centro = match dim {
        Dimension::Linear(d) => {
            let u = [d.rotation.cos(), d.rotation.sin()];
            lineal_o_alineada(
                p2(&d.first_point),
                p2(&d.second_point),
                p2(&d.definition_point),
                u,
                &valor,
                m,
                &mut v,
            )
        }
        Dimension::Aligned(d) => {
            let (a, b) = (p2(&d.first_point), p2(&d.second_point));
            let u = geo::unitario(geo::restar(b, a));
            lineal_o_alineada(a, b, p2(&d.definition_point), u, &valor, m, &mut v)
        }
        Dimension::Radius(d) => {
            let (c, q) = (p2(&d.angle_vertex), p2(&d.definition_point));
            let dir = direccion(c, q);
            if d.leader_length > geo::EPS {
                let lejos = geo::sumar(q, geo::por(dir, d.leader_length));
                v.push(raya(q, lejos));
                v.push(flecha(q, geo::por(dir, -1.0), m.flecha));
                let (t, k) = texto_sobre(&valor, geo::medio(q, lejos), dir, m);
                v.push(t);
                k
            } else {
                v.push(raya(c, q));
                v.push(flecha(q, dir, m.flecha));
                let (t, k) = texto_sobre(&valor, geo::medio(c, q), dir, m);
                v.push(t);
                k
            }
        }
        Dimension::Diameter(d) => {
            let (a, b) = (p2(&d.angle_vertex), p2(&d.definition_point));
            let dir = direccion(a, b);
            v.push(flecha(a, geo::por(dir, -1.0), m.flecha));
            v.push(flecha(b, dir, m.flecha));
            if d.leader_length > geo::EPS {
                let lejos = geo::sumar(b, geo::por(dir, d.leader_length));
                v.push(raya(a, lejos));
                let (t, k) = texto_sobre(&valor, geo::medio(b, lejos), dir, m);
                v.push(t);
                k
            } else {
                v.push(raya(a, b));
                let (t, k) = texto_sobre(&valor, geo::medio(a, b), dir, m);
                v.push(t);
                k
            }
        }
        Dimension::Angular2Ln(d) => {
            let l1 = (p2(&d.first_point), p2(&d.second_point));
            let l2 = (p2(&d.angle_vertex), p2(&d.definition_point));
            match sector(l1, l2, p2(&d.dimension_arc)) {
                Some(s) => angular_dibujado(&s, [l1, l2], &valor, m, &mut v),
                None => p2(&d.dimension_arc),
            }
        }
        otra => p2(&otra.base().text_middle_point),
    };
    (v, centro)
}

fn lineal_o_alineada(
    p1: P,
    p2: P,
    c2: P,
    u: P,
    valor: &str,
    m: &Medidas,
    v: &mut Vec<EntityType>,
) -> P {
    let c1 = geo::sumar(c2, geo::por(u, geo::punto(geo::restar(p1, c2), u)));
    let w = if geo::dist(c1, c2) > geo::EPS {
        geo::unitario(geo::restar(c2, c1))
    } else {
        u
    };
    extension(p1, c1, m, v);
    extension(p2, c2, m, v);
    if m.marca > 0.0 {
        v.push(raya(
            geo::sumar(c1, geo::por(w, -m.prolonga)),
            geo::sumar(c2, geo::por(w, m.prolonga)),
        ));
        v.push(marca(c1, w, m.marca));
        v.push(marca(c2, w, m.marca));
    } else {
        v.push(raya(c1, c2));
        v.push(flecha(c1, geo::por(w, -1.0), m.flecha));
        v.push(flecha(c2, w, m.flecha));
    }
    let (t, k) = texto_sobre(valor, geo::medio(c1, c2), w, m);
    v.push(t);
    k
}

fn angular_dibujado(
    s: &Sector,
    lineas: [(P, P); 2],
    valor: &str,
    m: &Medidas,
    v: &mut Vec<EntityType>,
) -> P {
    let fin = s.inicio + s.barrido;
    v.push(pieza(esp::arco(s.v, s.r, s.inicio, fin)));
    // Las extensiones, si el arco cae fuera de la linea medida.
    for lado in s.lados {
        let l = lineas
            .iter()
            .copied()
            .max_by(|a, b| {
                let pa = geo::cruz(geo::unitario(geo::restar(a.1, a.0)), lado).abs();
                let pb = geo::cruz(geo::unitario(geo::restar(b.1, b.0)), lado).abs();
                pb.total_cmp(&pa)
            })
            .expect("dos lineas");
        let t0 = geo::punto(geo::restar(l.0, s.v), lado);
        let t1 = geo::punto(geo::restar(l.1, s.v), lado);
        let (tmin, tmax) = (t0.min(t1), t0.max(t1));
        let en = |t: f64| geo::sumar(s.v, geo::por(lado, t));
        if s.r > tmax + m.separa {
            v.push(raya(en(tmax + m.separa), en(s.r + m.pasa)));
        } else if s.r < tmin - m.separa {
            v.push(raya(en(tmin - m.separa), en(s.r - m.pasa)));
        }
    }
    let (a0, a1) = (geo::polar(s.v, s.r, s.inicio), geo::polar(s.v, s.r, fin));
    v.push(flecha(a0, [s.inicio.sin(), -s.inicio.cos()], m.flecha));
    v.push(flecha(a1, [-fin.sin(), fin.cos()], m.flecha));
    let medio = s.inicio + s.barrido / 2.0;
    let c = geo::polar(s.v, s.r + m.hueco + m.texto / 2.0, medio);
    v.push(texto(valor, c, m.texto, legible(medio + FRAC_PI_2)));
    c
}

// ------------------------------------------------------ al dibujo, con bloque

/// El siguiente nombre libre de bloque de cota (`*D1`, `*D2`…).
fn nombre_libre(doc: &CadDocument) -> String {
    let n = doc
        .block_records
        .iter()
        .filter_map(|b| {
            let r = b.name.strip_prefix("*D").or(b.name.strip_prefix("*d"))?;
            r.parse::<u64>().ok()
        })
        .max()
        .unwrap_or(0);
    format!("*D{}", n + 1)
}

/// **Anade una cota al dibujo** con su bloque `*Dn`, en la capa de cotas y
/// con el estilo del dibujo (de una vez: un paso de deshacer).
pub fn crear(
    dibujo: &mut Dibujo,
    nombre: &str,
    mut dim: Dimension,
    alto_texto: f64,
    decimales: usize,
) -> Option<Handle> {
    let m = asegurar_estilo(&mut dibujo.doc, alto_texto, decimales);
    dibujo.asegurar_capa(esp::CAPA_COTAS, esp::color_de_capa(esp::CAPA_COTAS));
    let doc = &mut dibujo.doc;
    let bloque = nombre_libre(doc);
    let mut br = BlockRecord::new(&bloque);
    br.handle = doc.allocate_handle();
    br.block_entity_handle = doc.allocate_handle();
    br.block_end_handle = doc.allocate_handle();
    br.flags.anonymous = true;
    let dueno = br.handle;
    doc.block_records.add(br).ok()?;
    {
        let b = dim.base_mut();
        b.style_name = ESTILO.to_string();
        b.block_name = bloque;
    }
    let (piezas, centro) = dibujo_de_cota(&dim, &m);
    dim.base_mut().text_middle_point = v3(centro);
    dibujo.editar(nombre, |t| {
        poner_piezas(t, dueno, piezas);
        t.anadir_en(EntityType::Dimension(dim), esp::CAPA_COTAS)
    })
}

fn poner_piezas(t: &mut Transaccion, dueno: Handle, piezas: Vec<EntityType>) {
    for mut p in piezas {
        p.common_mut().owner_handle = dueno;
        t.anadir(p);
    }
}

/// **Vuelve a dibujar el bloque** de la cota `h` (tras cambiarle el texto o
/// los puntos), dentro de una orden.
pub fn rehacer_bloque(t: &mut Transaccion, h: Handle, por_defecto: Medidas) -> bool {
    let Some(EntityType::Dimension(dim)) = t.entidad(h).cloned() else {
        return false;
    };
    let m = medidas_de(t.doc(), &dim, por_defecto);
    let (piezas, centro) = dibujo_de_cota(&dim, &m);
    let Some((dueno, viejas)) = t
        .doc()
        .block_records
        .get(&dim.base().block_name)
        .map(|b| (b.handle, b.entity_handles.clone()))
    else {
        return false;
    };
    for v in viejas {
        t.quitar(v);
    }
    poner_piezas(t, dueno, piezas);
    t.cambiar(h, |e| {
        if let EntityType::Dimension(d) = e {
            d.base_mut().text_middle_point = v3(centro);
        }
    });
    true
}

// --------------------------------------------------------------- elegir

/// Lo que dista `p` de una caja (0 dentro).
fn a_caja(c: &Caja, p: P) -> f64 {
    let dx = (c[0] - p[0]).max(0.0).max(p[0] - c[2]);
    let dy = (c[1] - p[1]).max(0.0).max(p[1] - c[3]);
    dx.hypot(dy)
}

/// Lo que dista `p` de una pieza dibujada (un texto, por su caja).
fn a_entidad(e: &EntityType, p: P) -> f64 {
    let f = formas::formas(e);
    if f.is_empty() {
        return formas::caja(e, &f).map_or(f64::INFINITY, |c| a_caja(&c, p));
    }
    f.iter()
        .map(|f| f.distancia(p))
        .fold(f64::INFINITY, f64::min)
}

/// **El texto o la cota bajo el cursor** (a menos de `tol`): un TEXT, un
/// MTEXT o una DIMENSION, medida a lo que se ve de ella (su bloque). Busca
/// solo cerca de `p`, en el indice.
pub fn elegir(dibujo: &Dibujo, p: P, tol: f64, textos: bool, cotas: bool) -> Option<Handle> {
    let ind = dibujo.indice();
    // El texto de una cota sale de su caja (que es la de sus puntos).
    let zona = geo::ensanchar([p[0], p[1], p[0], p[1]], tol * 4.0);
    let mut mejor: Option<(f64, Handle)> = None;
    for en in ind.en(&zona) {
        if en.bloqueada {
            continue;
        }
        let d = match dibujo.entidad(en.handle) {
            Some(EntityType::Text(_) | EntityType::MText(_)) if textos => a_caja(&en.caja, p),
            Some(EntityType::Dimension(dim)) if cotas => {
                let partes: Vec<_> = dibujo
                    .doc
                    .entities_in_block(&dim.base().block_name)
                    .collect();
                if partes.is_empty() {
                    en.formas
                        .iter()
                        .map(|f| f.distancia(p))
                        .fold(f64::INFINITY, f64::min)
                } else {
                    partes
                        .iter()
                        .map(|e| a_entidad(e, p))
                        .fold(f64::INFINITY, f64::min)
                }
            }
            _ => continue,
        };
        if d <= tol && mejor.is_none_or(|(m, _)| d < m) {
            mejor = Some((d, en.handle));
        }
    }
    mejor.map(|(_, h)| h)
}

/// El tramo recto de una entidad mas cerca de `p` (una linea o un tramo de
/// polilinea).
pub fn tramo_cercano(e: &EntityType, p: P) -> Option<(P, P)> {
    formas::formas(e)
        .into_iter()
        .filter_map(|f| match f {
            geo::Forma::Segmento { a, b } => Some((f.distancia(p), (a, b))),
            _ => None,
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, s)| s)
}

/// El centro y el radio de un circulo o un arco.
pub fn circulo_de(e: &EntityType) -> Option<(P, f64)> {
    if !matches!(e, EntityType::Circle(_) | EntityType::Arc(_)) {
        return None;
    }
    match formas::formas(e).first()? {
        geo::Forma::Circulo { c, r } | geo::Forma::Arco { c, r, .. } => Some((*c, *r)),
        _ => None,
    }
}

/// Los extremos de lo que se acota con un clic: una linea, un tramo de
/// polilinea o un arco.
pub fn extremos_para_cotar(e: &EntityType, p: P) -> Option<(P, P)> {
    match e {
        EntityType::Arc(_) => {
            let f = *formas::formas(e).first()?;
            let x = f.extremos();
            (x.len() == 2).then(|| (x[0], x[1]))
        }
        EntityType::Line(_) | EntityType::LwPolyline(_) => {
            let f = formas::formas(e)
                .into_iter()
                .min_by(|a, b| a.distancia(p).total_cmp(&b.distancia(p)))?;
            let x = f.extremos();
            (x.len() == 2).then(|| (x[0], x[1]))
        }
        _ => None,
    }
}

/// Un angulo en radianes, en grados de 0 a 360.
pub fn grados(rad: f64) -> f64 {
    rad.rem_euclid(TAU).to_degrees()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn m() -> Medidas {
        Medidas::para(250.0, 0)
    }

    #[test]
    fn la_lineal_mide_en_su_direccion() {
        let d = lineal([0.0, 0.0], [3000.0, 1000.0], [0.0, -500.0], 0.0).unwrap();
        assert!((d.base().actual_measurement - 3000.0).abs() < 1e-9);
        assert!((d.measurement() - 3000.0).abs() < 1e-9);
        // El punto de la linea de cota, sobre la segunda extension.
        assert_eq!(p2(&d.definition_point()), [3000.0, -500.0]);
        let v = lineal([0.0, 0.0], [3000.0, 1000.0], [-500.0, 0.0], FRAC_PI_2).unwrap();
        assert!((v.measurement() - 1000.0).abs() < 1e-9);
        // Caso negativo: nada que medir en esa direccion.
        assert!(lineal([0.0, 0.0], [0.0, 1000.0], [0.0, -500.0], 0.0).is_none());
        assert!(alineada([5.0, 5.0], [5.0, 5.0], [0.0, 0.0]).is_none());
    }

    #[test]
    fn el_dibujo_tiene_extensiones_linea_marcas_y_texto() {
        let d = alineada([0.0, 0.0], [3000.0, 4000.0], [-800.0, 600.0]).unwrap();
        let (v, c) = dibujo_de_cota(&d, &m());
        let rayas = v
            .iter()
            .filter(|e| matches!(e, EntityType::Line(_)))
            .count();
        // 2 extensiones + linea de cota + 2 marcas.
        assert_eq!(rayas, 5);
        let EntityType::MText(t) = v
            .iter()
            .find(|e| matches!(e, EntityType::MText(_)))
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(t.value, "5000");
        assert!((t.height - 250.0).abs() < 1e-9);
        assert!(v.iter().all(|e| e.common().color == Color::ByBlock));
        // El texto queda del lado de la linea de cota, no sobre lo medido.
        assert!(geo::cruz([3000.0, 4000.0], c) > 0.0);
    }

    #[test]
    fn la_angular_elige_el_angulo_del_cursor() {
        let l1 = ([0.0, 0.0], [1000.0, 0.0]);
        let l2 = ([0.0, 0.0], [1000.0, 1000.0]);
        let d = angular(l1, l2, [500.0, 100.0]).unwrap();
        assert!((grados(d.base().actual_measurement) - 45.0).abs() < 1e-9);
        let d = angular(l1, l2, [-500.0, 100.0]).unwrap();
        assert!((grados(d.base().actual_measurement) - 135.0).abs() < 1e-9);
        // Caso negativo: paralelas.
        assert!(angular(l1, ([0.0, 5.0], [10.0, 5.0]), [3.0, 3.0]).is_none());
    }
}
