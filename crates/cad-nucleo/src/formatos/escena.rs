//! **Lo que se ve del dibujo, listo para exportar**: cada entidad visible
//! del espacio modelo (con los bloques, cotas y sombreados desmontados)
//! pasada a rayas, rellenos y textos con su color y su grosor de papel ya
//! resueltos (por capa, por bloque o propios), como lo imprime AutoCAD: el
//! color 7 sale negro, las capas apagadas, congeladas o que no se imprimen
//! no salen.

use opencadcodec::entities::{BoundaryEdge, Hatch, HatchPatternLine};
use opencadcodec::{CadDocument, Color, EntityType, LineWeight};

use crate::bloque;
use crate::formas::{self, p2};
use crate::geo::{self, Caja, Forma, P};

use super::{Rgb, texto_limpio};

/// Hasta donde se entra en bloques dentro de bloques.
const PROFUNDIDAD: u32 = 8;
/// Cuantas rayas de patron se sacan como mucho de un sombreado.
const RAYAS_POR_SOMBREADO: usize = 20_000;
/// El grosor «por defecto» de AutoCAD, en mm.
pub const GROSOR_DEFECTO: f64 = 0.25;

/// **Un texto exportado**: el punto de alineacion y como se alinea en el.
#[derive(Debug, Clone, PartialEq)]
pub struct TextoExp {
    pub texto: String,
    pub ancla: P,
    /// La altura de las mayusculas.
    pub alto: f64,
    pub giro: f64,
    /// El factor de ancho.
    pub ancho: f64,
    /// 0 a la izquierda, 0.5 al centro, 1 a la derecha del ancla.
    pub h: f64,
    /// Donde cae el ancla en altura: 0 la linea base, 0.5 el medio, 1 arriba,
    /// -0.3 abajo del todo.
    pub v: f64,
}

impl TextoExp {
    /// **El inicio de la linea base** si el texto mide `largo` alturas.
    pub fn origen(&self, largo: f64) -> P {
        let dx = -self.h * largo * self.alto * self.ancho;
        let dy = -self.v * self.alto;
        geo::sumar(self.ancla, geo::rotar([dx, dy], [0.0, 0.0], self.giro))
    }
}

/// Lo que se dibuja de un elemento.
#[derive(Debug, Clone, PartialEq)]
pub enum Pieza {
    /// Rayas, arcos y circulos trazados.
    Formas(Vec<Forma>),
    /// Contornos cerrados rellenos (par-impar).
    Relleno(Vec<Vec<P>>),
    Texto(TextoExp),
}

/// **Una cosa que se dibuja**, con su capa, su color y su grosor (mm).
#[derive(Debug, Clone, PartialEq)]
pub struct Elemento {
    pub pieza: Pieza,
    pub color: Rgb,
    pub grosor: f64,
    pub capa: String,
}

/// **Lo visible del dibujo**, en el orden en que se dibuja.
#[derive(Debug, Clone, Default)]
pub struct Escena {
    pub elementos: Vec<Elemento>,
    /// Las imagenes (no se exportan; se cuentan para avisar).
    pub imagenes: usize,
    /// Lo que no se sabe dibujar (solidos 3D, proxys…).
    pub omitidas: usize,
}

/// Lo que pasa de un bloque a lo que tiene dentro (lo «por bloque»).
#[derive(Debug, Clone, Copy)]
struct Herencia {
    color: Rgb,
    grosor: f64,
}

const RAIZ: Herencia = Herencia {
    color: [0, 0, 0],
    grosor: GROSOR_DEFECTO,
};

/// El RGB de papel de un color (el 7 sale negro).
fn rgb_papel(c: &Color) -> Option<Rgb> {
    match c {
        Color::Index(7) => Some([0, 0, 0]),
        otro => otro.rgb().map(|(r, g, b)| [r, g, b]),
    }
}

/// Las entidades de las que se hace la escena: lo elegido o todo el modelo.
pub enum Que<'a> {
    Todo,
    Estas(&'a [opencadcodec::Handle]),
}

/// **La escena** de `doc` (o solo de unas entidades), con lo que toque
/// `zona` si se da.
pub fn escena(doc: &CadDocument, que: Que, zona: Option<Caja>) -> Escena {
    let mut e = Escena::default();
    let mut ctx = Ctx {
        doc,
        escena: &mut e,
        zona,
    };
    match que {
        Que::Todo => {
            for ent in doc.model_space_entities() {
                ctx.entidad(ent, RAIZ, 0);
            }
        }
        Que::Estas(hs) => {
            for h in hs {
                if let Some(ent) = doc.get_entity(*h) {
                    ctx.entidad(ent, RAIZ, 0);
                }
            }
        }
    }
    e
}

struct Ctx<'a> {
    doc: &'a CadDocument,
    escena: &'a mut Escena,
    zona: Option<Caja>,
}

impl Ctx<'_> {
    fn capa_visible(&self, nombre: &str) -> bool {
        if nombre.eq_ignore_ascii_case("DEFPOINTS") {
            return false;
        }
        self.doc
            .layers
            .get(nombre)
            .is_none_or(|l| l.is_visible() && !l.is_frozen() && l.is_plottable)
    }

    fn color(&self, e: &EntityType, her: Herencia) -> Rgb {
        let c = e.common();
        match c.color {
            Color::ByLayer => self
                .doc
                .layers
                .get(&c.layer)
                .and_then(|l| rgb_papel(&l.color))
                .unwrap_or([0, 0, 0]),
            Color::ByBlock => her.color,
            Color::None => [0, 0, 0],
            otro => rgb_papel(&otro).unwrap_or([0, 0, 0]),
        }
    }

    fn grosor(&self, e: &EntityType, her: Herencia) -> f64 {
        let c = e.common();
        match c.line_weight {
            LineWeight::Value(v) if v >= 0 => v as f64 / 100.0,
            LineWeight::ByBlock => her.grosor,
            LineWeight::ByLayer => match self.doc.layers.get(&c.layer).map(|l| l.line_weight) {
                Some(LineWeight::Value(v)) if v >= 0 => v as f64 / 100.0,
                _ => GROSOR_DEFECTO,
            },
            _ => GROSOR_DEFECTO,
        }
    }

    fn poner(&mut self, pieza: Pieza, color: Rgb, grosor: f64, capa: &str) {
        if let Some(z) = self.zona {
            let caja = caja_de_pieza(&pieza);
            if !geo::caja_valida(&caja) || !geo::se_tocan(&caja, &z) {
                return;
            }
        }
        self.escena.elementos.push(Elemento {
            pieza,
            color,
            grosor,
            capa: capa.to_string(),
        });
    }

    fn entidad(&mut self, e: &EntityType, her: Herencia, prof: u32) {
        let c = e.common();
        if c.invisible || !self.capa_visible(&c.layer) {
            return;
        }
        let color = self.color(e, her);
        let grosor = self.grosor(e, her);
        let capa = c.layer.clone();
        let aqui = Herencia { color, grosor };
        match e {
            EntityType::Line(_)
            | EntityType::Arc(_)
            | EntityType::Circle(_)
            | EntityType::LwPolyline(_)
            | EntityType::Polyline2D(_)
            | EntityType::Polyline3D(_)
            | EntityType::Polyline(_)
            | EntityType::Ellipse(_)
            | EntityType::Spline(_) => {
                let f = formas::formas(e);
                if !f.is_empty() {
                    self.poner(Pieza::Formas(f), color, grosor, &capa);
                }
            }
            EntityType::Text(t) => {
                let (h, v, ancla) = alineacion_texto(t);
                let texto = texto_limpio(&t.value);
                if !texto.trim().is_empty() && t.height > 0.0 {
                    self.poner(
                        Pieza::Texto(TextoExp {
                            texto,
                            ancla,
                            alto: t.height,
                            giro: t.rotation,
                            ancho: if t.width_factor > 0.0 {
                                t.width_factor
                            } else {
                                1.0
                            },
                            h,
                            v,
                        }),
                        color,
                        grosor,
                        &capa,
                    );
                }
            }
            EntityType::AttributeEntity(a) => self.atributo(a, color, grosor, &capa),
            EntityType::MText(t) => {
                for tx in lineas_de_mtext(t) {
                    self.poner(Pieza::Texto(tx), color, grosor, &capa);
                }
            }
            EntityType::Solid(s) => {
                let q = [
                    p2(&s.first_corner),
                    p2(&s.second_corner),
                    p2(&s.fourth_corner),
                    p2(&s.third_corner),
                ];
                let mut v = q.to_vec();
                super::sin_repetidos(&mut v, 1e-12);
                if v.len() >= 3 {
                    self.poner(Pieza::Relleno(vec![v]), color, grosor, &capa);
                }
            }
            EntityType::Hatch(h) => self.sombreado(h, color, grosor, &capa),
            EntityType::Insert(i) if prof < PROFUNDIDAD => {
                for parte in bloque::descomponer(self.doc, i) {
                    self.entidad(&parte, aqui, prof + 1);
                }
                for a in &i.attributes {
                    let ae = EntityType::AttributeEntity(a.clone());
                    self.entidad(&ae, aqui, prof + 1);
                }
            }
            EntityType::Dimension(d) if prof < PROFUNDIDAD => {
                let nombre = &d.base().block_name;
                let partes: Vec<EntityType> = if !nombre.is_empty()
                    && self.doc.block_records.get(nombre).is_some()
                {
                    self.doc
                        .entities_in_block(nombre)
                        .filter(|x| !matches!(x, EntityType::Block(_) | EntityType::BlockEnd(_)))
                        .cloned()
                        .collect()
                } else {
                    e.explode()
                };
                for mut p in partes {
                    if p.common().layer == "0" {
                        p.common_mut().layer = capa.clone();
                    }
                    self.entidad(&p, aqui, prof + 1);
                }
            }
            EntityType::RasterImage(_) => self.escena.imagenes += 1,
            EntityType::Point(_)
            | EntityType::Viewport(_)
            | EntityType::Block(_)
            | EntityType::BlockEnd(_)
            | EntityType::Seqend(_)
            | EntityType::AttributeDefinition(_) => {}
            otro if prof < PROFUNDIDAD => {
                let partes = otro.explode();
                if partes.is_empty() {
                    self.escena.omitidas += 1;
                }
                for p in partes {
                    self.entidad(&p, aqui, prof + 1);
                }
            }
            _ => self.escena.omitidas += 1,
        }
    }

    fn atributo(
        &mut self,
        a: &opencadcodec::entities::AttributeEntity,
        color: Rgb,
        grosor: f64,
        capa: &str,
    ) {
        use opencadcodec::entities::{HorizontalAlignment as H, VerticalAlignment as V};
        if a.flags.invisible || a.value.trim().is_empty() || a.height <= 0.0 {
            return;
        }
        let normal = a.horizontal_alignment == H::Left && a.vertical_alignment == V::Baseline;
        let ancla = if normal {
            p2(&a.insertion_point)
        } else {
            p2(&a.alignment_point)
        };
        let h = match a.horizontal_alignment {
            H::Center | H::Middle => 0.5,
            H::Right => 1.0,
            _ => 0.0,
        };
        let v = match (a.horizontal_alignment, a.vertical_alignment) {
            (H::Middle, _) | (_, V::Middle) => 0.5,
            (_, V::Top) => 1.0,
            (_, V::Bottom) => -0.3,
            _ => 0.0,
        };
        self.poner(
            Pieza::Texto(TextoExp {
                texto: texto_limpio(&a.value),
                ancla,
                alto: a.height,
                giro: a.rotation,
                ancho: if a.width_factor > 0.0 {
                    a.width_factor
                } else {
                    1.0
                },
                h,
                v,
            }),
            color,
            grosor,
            capa,
        );
    }

    fn sombreado(&mut self, h: &Hatch, color: Rgb, grosor: f64, capa: &str) {
        let anillos = anillos_de_sombreado(h);
        if anillos.is_empty() {
            return;
        }
        if h.is_solid || h.pattern.lines.is_empty() {
            self.poner(Pieza::Relleno(anillos), color, grosor, capa);
        } else {
            let rayas = rayas_de_patron(&anillos, &h.pattern.lines, RAYAS_POR_SOMBREADO);
            if !rayas.is_empty() {
                self.poner(Pieza::Formas(rayas), color, grosor, capa);
            }
        }
    }
}

/// El ancla y la alineacion de un TEXT.
fn alineacion_texto(t: &opencadcodec::entities::Text) -> (f64, f64, P) {
    use opencadcodec::entities::{TextHorizontalAlignment as H, TextVerticalAlignment as V};
    let normal = t.horizontal_alignment == H::Left && t.vertical_alignment == V::Baseline;
    if normal || matches!(t.horizontal_alignment, H::Aligned | H::Fit) {
        return (0.0, 0.0, p2(&t.insertion_point));
    }
    let ancla = t
        .alignment_point
        .map(|a| p2(&a))
        .unwrap_or_else(|| p2(&t.insertion_point));
    let h = match t.horizontal_alignment {
        H::Center | H::Middle => 0.5,
        H::Right => 1.0,
        _ => 0.0,
    };
    let v = match (t.horizontal_alignment, t.vertical_alignment) {
        (H::Middle, _) | (_, V::Middle) => 0.5,
        (_, V::Top) => 1.0,
        (_, V::Bottom) => -0.3,
        _ => 0.0,
    };
    (h, v, ancla)
}

/// **Las lineas de un MTEXT**, cada una como texto, segun su punto de enganche.
pub fn lineas_de_mtext(t: &opencadcodec::entities::MText) -> Vec<TextoExp> {
    use opencadcodec::entities::AttachmentPoint as A;
    let plano = opencadcodec::entities::mtext_format::parse_mtext(&t.value, true).to_plain_text();
    let lineas: Vec<String> = plano.lines().map(texto_limpio).collect();
    if lineas.is_empty() || t.height <= 0.0 {
        return Vec::new();
    }
    let paso = t.height
        * 5.0
        * 0.333
        * if t.line_spacing_factor > 0.0 {
            t.line_spacing_factor
        } else {
            1.0
        };
    let n = lineas.len() as f64;
    let h = match t.attachment_point {
        A::TopLeft | A::MiddleLeft | A::BottomLeft => 0.0,
        A::TopCenter | A::MiddleCenter | A::BottomCenter => 0.5,
        _ => 1.0,
    };
    // La linea base de la primera, bajando desde el ancla.
    let primera = match t.attachment_point {
        A::TopLeft | A::TopCenter | A::TopRight => -t.height,
        A::MiddleLeft | A::MiddleCenter | A::MiddleRight => {
            -t.height + ((n - 1.0) * paso + t.height) / 2.0
        }
        _ => (n - 1.0) * paso,
    };
    let ins = p2(&t.insertion_point);
    lineas
        .into_iter()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| {
            let dy = primera - i as f64 * paso;
            TextoExp {
                texto: l,
                ancla: geo::sumar(ins, geo::rotar([0.0, dy], [0.0, 0.0], t.rotation)),
                alto: t.height,
                giro: t.rotation,
                ancho: 1.0,
                h,
                v: 0.0,
            }
        })
        .collect()
}

/// **Los contornos de un sombreado**, cada uno en una polilinea cerrada.
pub fn anillos_de_sombreado(h: &Hatch) -> Vec<Vec<P>> {
    let mut out = Vec::new();
    for camino in &h.paths {
        let mut v: Vec<P> = Vec::new();
        for borde in &camino.edges {
            match borde {
                BoundaryEdge::Line(l) => {
                    v.push([l.start.x, l.start.y]);
                    v.push([l.end.x, l.end.y]);
                }
                BoundaryEdge::Polyline(pl) => {
                    let n = pl.vertices.len();
                    for i in 0..n {
                        let a = [pl.vertices[i].x, pl.vertices[i].y];
                        let b = pl.vertices[i].z;
                        let ultimo = i + 1 == n;
                        if ultimo && !pl.is_closed {
                            v.push(a);
                            break;
                        }
                        let s = &pl.vertices[(i + 1) % n];
                        let f = Forma::de_abombado(a, [s.x, s.y], b);
                        v.extend(muestras(&f));
                    }
                }
                BoundaryEdge::CircularArc(a) => {
                    let c = [a.center.x, a.center.y];
                    // Un arco horario va de su inicio a su fin en sentido
                    // horario: el antihorario del fin al inicio, al reves
                    // (como lo desmonta opencadcodec).
                    let (a0, a1) = if a.counter_clockwise {
                        (a.start_angle, a.end_angle)
                    } else {
                        (a.end_angle, a.start_angle)
                    };
                    let mut barrido = geo::normalizar(a1 - a0);
                    if barrido < 1e-9 {
                        barrido = std::f64::consts::TAU;
                    }
                    let n = ((barrido / 5f64.to_radians()).ceil() as usize).clamp(2, 144);
                    let mut pts: Vec<P> = (0..=n)
                        .map(|k| geo::polar(c, a.radius, a0 + barrido * k as f64 / n as f64))
                        .collect();
                    if !a.counter_clockwise {
                        pts.reverse();
                    }
                    v.extend(pts);
                }
                BoundaryEdge::EllipticArc(e) => {
                    let c = [e.center.x, e.center.y];
                    let mayor = [e.major_axis_endpoint.x, e.major_axis_endpoint.y];
                    let menor = geo::por(geo::perp(mayor), e.minor_axis_ratio);
                    let (t0, mut t1) = (e.start_angle, e.end_angle);
                    if t1 <= t0 {
                        t1 += std::f64::consts::TAU;
                    }
                    let n = 72;
                    let s = if e.counter_clockwise { 1.0 } else { -1.0 };
                    for k in 0..=n {
                        let t = s * (t0 + (t1 - t0) * k as f64 / n as f64);
                        v.push(geo::sumar(
                            c,
                            geo::sumar(geo::por(mayor, t.cos()), geo::por(menor, t.sin())),
                        ));
                    }
                }
                BoundaryEdge::Spline(s) => {
                    if !s.fit_points.is_empty() {
                        v.extend(s.fit_points.iter().map(|p| [p.x, p.y]));
                    } else {
                        v.extend(s.control_points.iter().map(|p| [p.x, p.y]));
                    }
                }
            }
        }
        super::sin_repetidos(&mut v, 1e-12);
        if v.len() > 2 && geo::dist(v[0], v[v.len() - 1]) < 1e-9 {
            v.pop();
        }
        if v.len() >= 3 {
            out.push(v);
        }
    }
    out
}

/// Los puntos de una forma sin el primero (para encadenar).
fn muestras(f: &Forma) -> Vec<P> {
    match f {
        Forma::Segmento { a, b } => vec![*a, *b],
        Forma::Arco { barrido, .. } => {
            let n = ((barrido / 5f64.to_radians()).ceil() as usize).clamp(2, 144);
            f.muestras(n)
        }
        Forma::Circulo { .. } => f.muestras(72),
    }
}

/// **Las rayas de un patron** dentro de unos contornos (par-impar), con sus
/// trazos y huecos. Como mucho `tope` rayas.
pub fn rayas_de_patron(
    anillos: &[Vec<P>],
    familias: &[HatchPatternLine],
    tope: usize,
) -> Vec<Forma> {
    let mut out = Vec::new();
    let caja = anillos
        .iter()
        .flatten()
        .fold(geo::CAJA_VACIA, |c, p| geo::caja_con(c, *p));
    if !geo::caja_valida(&caja) {
        return out;
    }
    for f in familias {
        let dir = [f.angle.cos(), f.angle.sin()];
        let n = geo::perp(dir);
        let base = [f.base_point.x, f.base_point.y];
        let off = [f.offset.x, f.offset.y];
        let paso = geo::punto(off, n);
        if paso.abs() < 1e-9 {
            continue;
        }
        // Lo que ocupan los contornos a lo ancho de la familia.
        let esquinas = [
            [caja[0], caja[1]],
            [caja[2], caja[1]],
            [caja[2], caja[3]],
            [caja[0], caja[3]],
        ];
        let vs: Vec<f64> = esquinas.iter().map(|p| geo::punto(*p, n)).collect();
        let (vmin, vmax) = (
            vs.iter().cloned().fold(f64::MAX, f64::min),
            vs.iter().cloned().fold(f64::MIN, f64::max),
        );
        let v0 = geo::punto(base, n);
        let (k0, k1) = {
            let a = ((vmin - v0) / paso).floor() as i64;
            let b = ((vmax - v0) / paso).ceil() as i64;
            (a.min(b), a.max(b))
        };
        if (k1 - k0) as usize > tope {
            return out;
        }
        let largo: f64 = f.dash_lengths.iter().map(|d| d.abs()).sum();
        for k in k0..=k1 {
            let v = v0 + k as f64 * paso;
            // Donde corta la raya a los contornos (a lo largo de `dir`).
            let mut us = Vec::new();
            for an in anillos {
                for i in 0..an.len() {
                    let (a, b) = (an[i], an[(i + 1) % an.len()]);
                    let (va, vb) = (geo::punto(a, n), geo::punto(b, n));
                    if (va <= v && vb > v) || (vb <= v && va > v) {
                        let t = (v - va) / (vb - va);
                        let p = geo::sumar(a, geo::por(geo::restar(b, a), t));
                        us.push(geo::punto(p, dir));
                    }
                }
            }
            us.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let origen_raya = geo::sumar(base, geo::por(off, k as f64));
            let u_base = geo::punto(origen_raya, dir);
            let pt = |u: f64| geo::sumar(geo::por(dir, u), geo::por(n, v));
            for par in us.chunks_exact(2) {
                let (u0, u1) = (par[0], par[1]);
                if largo < 1e-9 || f.dash_lengths.is_empty() {
                    out.push(Forma::Segmento {
                        a: pt(u0),
                        b: pt(u1),
                    });
                } else {
                    // Los trazos empiezan en el origen de la raya.
                    let mut u = u_base + ((u0 - u_base) / largo).floor() * largo;
                    'trazos: while u < u1 {
                        for d in &f.dash_lengths {
                            let fin = u + d.abs();
                            if *d >= 0.0 {
                                let (a, b) = (u.max(u0), fin.min(u1));
                                if *d == 0.0 && u >= u0 && u <= u1 {
                                    out.push(Forma::Segmento {
                                        a: pt(u),
                                        b: pt(u + largo * 1e-3),
                                    });
                                } else if b > a {
                                    out.push(Forma::Segmento { a: pt(a), b: pt(b) });
                                }
                            }
                            u = fin;
                            if u >= u1 || out.len() > tope {
                                break 'trazos;
                            }
                        }
                    }
                }
                if out.len() > tope {
                    return out;
                }
            }
        }
    }
    out
}

/// La caja de una pieza.
pub fn caja_de_pieza(p: &Pieza) -> Caja {
    match p {
        Pieza::Formas(f) => f
            .iter()
            .fold(geo::CAJA_VACIA, |c, f| geo::unir_cajas(c, f.caja())),
        Pieza::Relleno(a) => a
            .iter()
            .flatten()
            .fold(geo::CAJA_VACIA, |c, p| geo::caja_con(c, *p)),
        Pieza::Texto(t) => {
            let largo = super::shx::largo(&t.texto);
            let o = t.origen(largo);
            let l = largo * t.alto * t.ancho;
            let pts: Vec<P> = [
                [0.0, -0.3 * t.alto],
                [l, -0.3 * t.alto],
                [l, t.alto],
                [0.0, t.alto],
            ]
            .iter()
            .map(|q| geo::sumar(o, geo::rotar(*q, [0.0, 0.0], t.giro)))
            .collect();
            geo::caja_de(&pts)
        }
    }
}

impl Escena {
    /// **La caja de todo lo que hay** (`None` si no hay nada).
    pub fn caja(&self) -> Option<Caja> {
        let c = self.elementos.iter().fold(geo::CAJA_VACIA, |c, e| {
            geo::unir_cajas(c, caja_de_pieza(&e.pieza))
        });
        geo::caja_valida(&c).then_some(c)
    }

    /// Las capas que salen, en el orden en que aparecen.
    pub fn capas(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        for e in &self.elementos {
            if !v.contains(&e.capa) {
                v.push(e.capa.clone());
            }
        }
        v
    }
}

/// **Los marcos de las laminas**: las polilineas cerradas de la capa
/// `L-LAMINA` que no estan dentro de otra (el borde del papel de LAMINA).
pub fn marcos_de_lamina(doc: &CadDocument) -> Vec<Caja> {
    let mut cajas: Vec<Caja> = doc
        .model_space_entities()
        .filter(|e| {
            e.common()
                .layer
                .eq_ignore_ascii_case(crate::especialidad::CAPA_LAMINA)
        })
        .filter_map(|e| match e {
            EntityType::LwPolyline(p) if p.is_closed && p.vertices.len() == 4 => {
                formas::caja(e, &formas::formas(e))
            }
            _ => None,
        })
        .collect();
    let todas = cajas.clone();
    cajas.retain(|c| {
        !todas
            .iter()
            .any(|o| o != c && geo::contiene(o, c) && (o[2] - o[0]) > (c[2] - c[0]) + 1e-9)
    });
    // De arriba a abajo y de izquierda a derecha.
    cajas.sort_by(|a, b| {
        (-a[3], a[0])
            .partial_cmp(&(-b[3], b[0]))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    cajas.dedup();
    cajas
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::Dibujo;
    use crate::especialidad as esp;

    #[test]
    fn colores_por_capa_y_capas_apagadas() {
        let mut d = Dibujo::nuevo();
        d.editar("X", |t| {
            t.anadir_en(esp::linea([0.0, 0.0], [10.0, 0.0]), "A-MUROS");
            t.anadir_en(esp::texto("Hola", [0.0, 5.0], 2.5, 0.0, true), "A-TEXTOS");
            t.anadir_en(esp::relleno([0.0, 0.0], [1.0, 0.0], [0.0, 1.0]), "A-COTAS");
        });
        let e = escena(&d.doc, Que::Todo, None);
        assert_eq!(e.elementos.len(), 3);
        let Pieza::Texto(t) = &e.elementos[1].pieza else {
            panic!()
        };
        assert_eq!((t.h, t.v), (0.5, 0.5));
        assert!(matches!(e.elementos[2].pieza, Pieza::Relleno(_)));
        d.con_capa("A-MUROS", |l| l.turn_off());
        let e = escena(&d.doc, Que::Todo, None);
        assert_eq!(e.elementos.len(), 2);
        // Una zona que no toca nada.
        let e = escena(&d.doc, Que::Todo, Some([100.0, 100.0, 200.0, 200.0]));
        assert!(e.elementos.is_empty());
    }

    #[test]
    fn las_rayas_de_ansi31_quedan_dentro() {
        let cuadro = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        let l = crate::patrones::Patron::Ansi31.lineas(1.0, 0.0);
        let r = rayas_de_patron(&[cuadro], &l, 1000);
        assert!(r.len() > 3);
        for f in &r {
            let c = f.caja();
            assert!(c[0] >= -1e-6 && c[2] <= 10.0 + 1e-6 && c[1] >= -1e-6 && c[3] <= 10.0 + 1e-6);
        }
    }

    #[test]
    fn los_marcos_de_las_laminas() {
        let mut d = Dibujo::nuevo();
        let v = esp::lamina([0.0, 0.0], 420.0, 297.0, 1.0, 1.0, &Default::default());
        let v2 = esp::lamina([500.0, 0.0], 297.0, 210.0, 1.0, 1.0, &Default::default());
        d.editar("LAMINA", |t| {
            for e in v.into_iter().chain(v2) {
                t.anadir_en(e, esp::CAPA_LAMINA);
            }
        });
        let m = marcos_de_lamina(&d.doc);
        assert_eq!(m.len(), 2, "{m:?}");
        assert_eq!(m[0], [0.0, 0.0, 420.0, 297.0]);
    }
}
