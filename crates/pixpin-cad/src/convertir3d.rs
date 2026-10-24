//! **Un plano DWG o DXF en 3D**: lo que tiene cota (curvas de nivel a su
//! altura, polilineas 3D de un trazado, puntos de un levantamiento, caras 3D
//! y mallas de un terreno, el dibujo de reserva de los objetos de Civil 3D)
//! como modelo 3D para girarlo en [`crate::ventana3d`]. Los textos y
//! sombreados se quedan en la vista en planta.
//!
//! Las caras de una misma capa y color se juntan en una malla: asi un
//! terreno de miles de caras 3D muestra solo sus aristas vivas, no cada
//! triangulo.

use std::collections::{HashMap, HashSet};

use opencadcodec::CadDocument;
use opencadcodec::entities::EntityType;
use opencadcodec::types::{Color, Vector3};

use crate::convertir::{Afin, COLOR_7, de_color, ocs, polilinea_2d};
use crate::modelo3d::{Constructor3d, Modelo3d};
use crate::teselar;

const PROFUNDIDAD_MAX: u32 = 32;
/// Mas puntos que esto se dibujan sin poder tocarlos uno a uno.
const PUNTOS_CON_NOMBRE: usize = 50_000;

fn p3(m: &Afin, x: f64, y: f64, z: f64) -> [f64; 3] {
    [
        m.m[0][0] * x + m.m[0][1] * y + m.m[0][2] * z + m.m[0][3],
        m.m[1][0] * x + m.m[1][1] * y + m.m[1][2] * z + m.m[1][3],
        m.m[2][0] * x + m.m[2][1] * y + m.m[2][2] * z + m.m[2][3],
    ]
}

fn v3(m: &Afin, v: &Vector3) -> [f64; 3] {
    p3(m, v.x, v.y, v.z)
}

/// RGBA empaquetado (rojo abajo) a `f32`; el color 7, gris claro.
fn a_f32(c: u32) -> [f32; 4] {
    if c == COLOR_7 {
        return [0.82, 0.83, 0.85, 1.0];
    }
    [
        (c & 255) as f32 / 255.0,
        ((c >> 8) & 255) as f32 / 255.0,
        ((c >> 16) & 255) as f32 / 255.0,
        1.0,
    ]
}

#[derive(Clone)]
struct Cx<'a> {
    m: Afin,
    capa: Option<&'a str>,
    color: u32,
    profundidad: u32,
}

struct Conv<'d> {
    doc: &'d CadDocument,
    c: Constructor3d,
    abiertos: HashSet<String>,
    /// Un elemento por capa (al tocar algo, se nombra su capa).
    capas: HashMap<String, u32>,
    /// Las caras, juntas por capa y color hasta el final.
    caras: HashMap<(u32, u32), Vec<[f64; 3]>>,
    puntos: usize,
}

impl<'d> Conv<'d> {
    fn elemento(&mut self, capa: &str) -> u32 {
        if let Some(e) = self.capas.get(capa) {
            return *e;
        }
        let e = self.c.elemento("Capa", capa);
        self.capas.insert(capa.to_string(), e);
        e
    }

    fn estilo<'a>(&self, e: &'a EntityType, cx: &Cx<'a>) -> Option<(&'a str, u32)>
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
        if self.doc.layers.get(capa).is_some_and(|l| !l.is_visible()) {
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

    fn raya(&mut self, capa: &str, pts: &[[f64; 3]], color: u32) {
        if pts.len() >= 2 && pts.iter().all(|p| p.iter().all(|v| v.is_finite())) {
            let e = self.elemento(capa);
            self.c.linea(e, pts, a_f32(color));
        }
    }

    fn caras(&mut self, capa: &str, tri: &[[f64; 3]], color: u32) {
        if tri.iter().any(|p| p.iter().any(|v| !v.is_finite())) {
            return;
        }
        let e = self.elemento(capa);
        self.caras
            .entry((e, color))
            .or_default()
            .extend_from_slice(tri);
    }

    fn entidad<'a>(&mut self, e: &'a EntityType, cx: &Cx<'a>)
    where
        'd: 'a,
    {
        let Some((capa, color)) = self.estilo(e, cx) else {
            return;
        };
        let m = cx.m;
        match e {
            EntityType::Line(l) => self.raya(capa, &[v3(&m, &l.start), v3(&m, &l.end)], color),
            EntityType::Point(p) => {
                let q = v3(&m, &p.location);
                if !q.iter().all(|v| v.is_finite()) {
                    return;
                }
                self.puntos += 1;
                let el = if self.puntos <= PUNTOS_CON_NOMBRE {
                    self.c.elemento("Punto", &format!("{capa} · Z {:.3}", q[2]))
                } else {
                    self.elemento(capa)
                };
                let col = if color == COLOR_7 {
                    [1.0, 0.85, 0.15, 1.0]
                } else {
                    a_f32(color)
                };
                self.c.punto(el, q, col);
            }
            EntityType::Circle(k) => {
                let t = m.por(&ocs(&k.normal));
                let pts: Vec<[f64; 3]> = teselar::circulo([k.center.x, k.center.y], k.radius)
                    .into_iter()
                    .map(|p| p3(&t, p[0], p[1], k.center.z))
                    .collect();
                self.raya(capa, &pts, color);
            }
            EntityType::Arc(a) => {
                let t = m.por(&ocs(&a.normal));
                let pts: Vec<[f64; 3]> = teselar::arco(
                    [a.center.x, a.center.y],
                    a.radius,
                    a.start_angle,
                    a.end_angle,
                )
                .into_iter()
                .map(|p| p3(&t, p[0], p[1], a.center.z))
                .collect();
                self.raya(capa, &pts, color);
            }
            EntityType::Ellipse(el) => {
                let n = &el.normal;
                let a = &el.major_axis;
                let menor = [
                    n.y * a.z - n.z * a.y,
                    n.z * a.x - n.x * a.z,
                    n.x * a.y - n.y * a.x,
                ];
                let r = el.minor_axis_ratio;
                let pts: Vec<[f64; 3]> = teselar::elipse(
                    [0.0, 0.0],
                    [1.0, 0.0],
                    1.0,
                    el.start_parameter,
                    el.end_parameter,
                )
                .into_iter()
                .map(|p| {
                    let (k, s) = (p[0], p[1]);
                    p3(
                        &m,
                        el.center.x + a.x * k + menor[0] * r * s,
                        el.center.y + a.y * k + menor[1] * r * s,
                        el.center.z + a.z * k + menor[2] * r * s,
                    )
                })
                .collect();
                self.raya(capa, &pts, color);
            }
            EntityType::LwPolyline(p) => {
                let t = m.por(&ocs(&p.normal));
                let v: Vec<([f64; 2], f64)> = p
                    .vertices
                    .iter()
                    .map(|v| ([v.location.x, v.location.y], v.bulge))
                    .collect();
                let pts: Vec<[f64; 3]> = polilinea_2d(&v, p.is_closed)
                    .into_iter()
                    .map(|q| p3(&t, q[0], q[1], p.elevation))
                    .collect();
                self.raya(capa, &pts, color);
            }
            EntityType::Polyline2D(p) => {
                let t = m.por(&ocs(&p.normal));
                let v: Vec<([f64; 2], f64)> = p
                    .vertices
                    .iter()
                    .filter(|v| v.flags.bits() & 16 == 0)
                    .map(|v| ([v.location.x, v.location.y], v.bulge))
                    .collect();
                let pts: Vec<[f64; 3]> = polilinea_2d(&v, p.flags.is_closed())
                    .into_iter()
                    .map(|q| p3(&t, q[0], q[1], p.elevation))
                    .collect();
                self.raya(capa, &pts, color);
            }
            EntityType::Polyline3D(p) => {
                let mut pts: Vec<[f64; 3]> =
                    p.vertices.iter().map(|v| v3(&m, &v.position)).collect();
                if p.flags.closed && pts.len() > 2 {
                    pts.push(pts[0]);
                }
                self.raya(capa, &pts, color);
            }
            EntityType::Spline(s) => {
                let z = if s.control_points.is_empty() {
                    s.fit_points.iter().map(|c| c.z).sum::<f64>() / s.fit_points.len().max(1) as f64
                } else {
                    s.control_points.iter().map(|c| c.z).sum::<f64>()
                        / s.control_points.len() as f64
                };
                let pts = if s.control_points.len() >= 2 {
                    let cp: Vec<[f64; 3]> =
                        s.control_points.iter().map(|c| [c.x, c.y, c.z]).collect();
                    teselar::spline(s.degree.max(1) as usize, &s.knots, &cp, &s.weights)
                } else {
                    teselar::por_puntos(
                        &s.fit_points.iter().map(|c| [c.x, c.y]).collect::<Vec<_>>(),
                    )
                };
                let pts: Vec<[f64; 3]> = pts.into_iter().map(|q| p3(&m, q[0], q[1], z)).collect();
                self.raya(capa, &pts, color);
            }
            EntityType::Face3D(f) => {
                let p = [
                    &f.first_corner,
                    &f.second_corner,
                    &f.third_corner,
                    &f.fourth_corner,
                ]
                .map(|v| v3(&m, v));
                let mut tri = vec![p[0], p[1], p[2]];
                if p[3] != p[2] {
                    tri.extend([p[0], p[2], p[3]]);
                }
                self.caras(capa, &tri, color);
            }
            EntityType::Solid(s) => {
                let t = m.por(&ocs(&s.normal));
                let p = [
                    &s.first_corner,
                    &s.second_corner,
                    &s.third_corner,
                    &s.fourth_corner,
                ]
                .map(|v| p3(&t, v.x, v.y, v.z));
                self.caras(capa, &[p[0], p[1], p[2], p[1], p[3], p[2]], color);
            }
            EntityType::PolyfaceMesh(p) => {
                let v: Vec<[f64; 3]> = p.vertices.iter().map(|v| v3(&m, &v.location)).collect();
                let mut tri = Vec::new();
                for f in &p.faces {
                    let idx: Vec<usize> = f
                        .vertex_indices()
                        .into_iter()
                        .filter(|&i| i != 0)
                        .map(|i| i.unsigned_abs() as usize - 1)
                        .collect();
                    if idx.iter().any(|&i| i >= v.len()) || idx.len() < 3 {
                        continue;
                    }
                    for k in 1..idx.len() - 1 {
                        tri.extend([v[idx[0]], v[idx[k]], v[idx[k + 1]]]);
                    }
                }
                self.caras(capa, &tri, color);
            }
            EntityType::PolygonMesh(p) => {
                let (fm, cn) = (
                    p.m_vertex_count.max(0) as usize,
                    p.n_vertex_count.max(0) as usize,
                );
                let v: Vec<[f64; 3]> = p.vertices.iter().map(|v| v3(&m, &v.location)).collect();
                if fm * cn == v.len() {
                    let mut tri = Vec::new();
                    for i in 0..fm.saturating_sub(1) {
                        for j in 0..cn.saturating_sub(1) {
                            let (a, b, c, d) = (
                                v[i * cn + j],
                                v[i * cn + j + 1],
                                v[(i + 1) * cn + j + 1],
                                v[(i + 1) * cn + j],
                            );
                            tri.extend([a, b, c, a, c, d]);
                        }
                    }
                    self.caras(capa, &tri, color);
                }
            }
            EntityType::Mesh(me) => {
                let v: Vec<[f64; 3]> = me.vertices.iter().map(|q| v3(&m, q)).collect();
                let mut tri = Vec::new();
                for f in &me.faces {
                    let idx: Vec<usize> = f
                        .vertices
                        .iter()
                        .copied()
                        .filter(|&i| i < v.len())
                        .collect();
                    for k in 1..idx.len().saturating_sub(1) {
                        tri.extend([v[idx[0]], v[idx[k]], v[idx[k + 1]]]);
                    }
                }
                self.caras(capa, &tri, color);
            }
            EntityType::Unknown(u) => {
                if let Some(crate::proxy::Reserva::Dibujo(prims)) = u
                    .common
                    .graphic_data
                    .as_deref()
                    .and_then(crate::proxy::leer)
                {
                    for p in prims {
                        match p {
                            crate::proxy::Primitiva::Raya { puntos, color: c } => {
                                let pts: Vec<[f64; 3]> =
                                    puntos.iter().map(|q| p3(&m, q[0], q[1], q[2])).collect();
                                self.raya(capa, &pts, c.unwrap_or(color));
                            }
                            crate::proxy::Primitiva::Caras { puntos, color: c } => {
                                let pts: Vec<[f64; 3]> =
                                    puntos.iter().map(|q| p3(&m, q[0], q[1], q[2])).collect();
                                self.caras(capa, &pts, c.unwrap_or(color));
                            }
                            crate::proxy::Primitiva::Texto { .. } => {}
                        }
                    }
                }
            }
            EntityType::Insert(ins) => {
                if cx.profundidad >= PROFUNDIDAD_MAX || self.abiertos.contains(&ins.block_name) {
                    return;
                }
                let base = match self
                    .doc
                    .block_records
                    .get(&ins.block_name)
                    .and_then(|_| self.doc.entities_in_block(&ins.block_name).next())
                {
                    Some(EntityType::Block(b)) => b.base_point,
                    _ => Vector3::new(0., 0., 0.),
                };
                let (filas, cols) = (
                    ins.row_count.max(1) as usize,
                    ins.column_count.max(1) as usize,
                );
                let comun = m
                    .por(&ocs(&ins.normal))
                    .por(&Afin::traslacion(
                        ins.insert_point.x,
                        ins.insert_point.y,
                        ins.insert_point.z,
                    ))
                    .por(&Afin::giro_z(ins.rotation));
                let escala = Afin::escala(ins.x_scale(), ins.y_scale(), ins.z_scale())
                    .por(&Afin::traslacion(-base.x, -base.y, -base.z));
                self.abiertos.insert(ins.block_name.clone());
                for i in 0..(filas * cols).min(2000) {
                    let (f, c) = (i / cols, i % cols);
                    let hijo = Cx {
                        m: comun
                            .por(&Afin::traslacion(
                                c as f64 * ins.column_spacing,
                                f as f64 * ins.row_spacing,
                                0.0,
                            ))
                            .por(&escala),
                        capa: Some(capa),
                        color,
                        profundidad: cx.profundidad + 1,
                    };
                    let doc = self.doc;
                    for e in doc.entities_in_block(&ins.block_name) {
                        self.entidad(e, &hijo);
                    }
                }
                self.abiertos.remove(&ins.block_name);
            }
            _ => {}
        }
    }
}

/// El plano en 3D (el espacio modelo).
pub fn convertir3d(doc: &CadDocument) -> Modelo3d {
    let mut cv = Conv {
        doc,
        c: Constructor3d::nuevo(),
        abiertos: HashSet::new(),
        capas: HashMap::new(),
        caras: HashMap::new(),
        puntos: 0,
    };
    let raiz = Cx {
        m: Afin::IDENTIDAD,
        capa: None,
        color: COLOR_7,
        profundidad: 0,
    };
    for e in doc.model_space_entities() {
        cv.entidad(e, &raiz);
    }
    let caras = std::mem::take(&mut cv.caras);
    for ((e, color), pts) in caras {
        let idx: Vec<u32> = (0..pts.len() as u32).collect();
        let col = if color == COLOR_7 {
            [0.78, 0.78, 0.76, 1.0]
        } else {
            a_f32(color)
        };
        cv.c.malla(e, &pts, None, &idx, col);
    }
    let mut m = cv.c.terminar();
    // Un plano no dice en que mide sus cotas.
    m.metros = 0.0;
    m
}

pub fn convertir3d_fichero(ruta: &std::path::Path) -> Result<Modelo3d, String> {
    let doc = crate::leer::leer(ruta)?;
    Ok(convertir3d(&doc))
}
