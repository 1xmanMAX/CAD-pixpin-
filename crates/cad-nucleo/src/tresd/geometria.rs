//! **La geometria del 3D**: poligonos de planta, prismas (una planta con
//! su altura), mallas de caras planas, triangular y pasar a MESH y de
//! vuelta.

use std::collections::HashMap;
use std::f64::consts::PI;

use opencadcodec::entities::{Mesh, MeshFace};
use opencadcodec::{EntityType, Vector3};

use crate::geo::{self, Forma, P};
use crate::patrones::Anillo;

pub type P3 = [f64; 3];

/// Cuantos tramos lleva como mucho un cuarto de vuelta al teselar un arco.
const TRAMOS_POR_CUARTO: f64 = 12.0;

/// **Un prisma recto**: una planta (poligono simple, antihorario) levantada
/// de `z0` a `z0 + alto`. Lo que dan EXTRUIR, MURO3D y LOSA, y lo que el
/// IFC escribe como `IfcExtrudedAreaSolid`.
#[derive(Debug, Clone, PartialEq)]
pub struct Prisma {
    pub planta: Vec<P>,
    pub z0: f64,
    pub alto: f64,
}

/// **Una malla de caras planas** (cada cara, sus vertices en orden
/// antihorario vistos desde fuera).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Malla {
    pub puntos: Vec<P3>,
    pub caras: Vec<Vec<u32>>,
}

pub fn resta3(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn cruz3(a: P3, b: P3) -> P3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn unitario3(a: P3) -> P3 {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    if l < 1e-300 {
        [0.0, 0.0, 1.0]
    } else {
        [a[0] / l, a[1] / l, a[2] / l]
    }
}

/// La normal (sin normalizar) de un poligono 3D, por Newell.
pub fn normal_newell(pts: &[P3]) -> P3 {
    let mut n = [0.0; 3];
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    n
}

/// **Un anillo con abombados** (el de una polilinea cerrada o un circulo)
/// como poligono: los arcos en tramos rectos.
pub fn poligono_de_anillo(a: &Anillo) -> Vec<P> {
    let n = a.len();
    let mut v = Vec::new();
    for i in 0..n {
        let (p, b) = a[i];
        let q = a[(i + 1) % n].0;
        v.push(p);
        if b.abs() > 1e-12 {
            let f = Forma::de_abombado(p, q, b);
            if let Forma::Arco { barrido, .. } = f {
                let k = ((barrido / (PI / 2.0)) * TRAMOS_POR_CUARTO).ceil().max(2.0) as usize;
                let mut m = f.muestras(k);
                // Las muestras van antihorario: si el tramo va al reves, tambien.
                if b < 0.0 {
                    m.reverse();
                }
                v.extend(&m[1..m.len() - 1]);
            }
        }
    }
    limpiar(&v)
}

/// Quita los puntos repetidos seguidos (y el ultimo si cierra en el
/// primero) y los que quedan en medio de una recta.
pub fn limpiar(pts: &[P]) -> Vec<P> {
    let mut v: Vec<P> = Vec::with_capacity(pts.len());
    for p in pts {
        if v.last().is_none_or(|q| geo::dist(*q, *p) > 1e-9) {
            v.push(*p);
        }
    }
    while v.len() > 1 && geo::dist(v[0], *v.last().unwrap()) <= 1e-9 {
        v.pop();
    }
    // Los alineados: el area del triangulo con sus vecinos, casi nula.
    let mut i = 0;
    while v.len() > 3 && i < v.len() {
        let n = v.len();
        let (a, b, c) = (v[(i + n - 1) % n], v[i], v[(i + 1) % n]);
        let largo = geo::dist(a, c).max(1e-300);
        if (geo::cruz(geo::restar(b, a), geo::restar(c, a)) / largo).abs() < 1e-9 * largo.max(1.0) {
            v.remove(i);
        } else {
            i += 1;
        }
    }
    v
}

/// El poligono en sentido antihorario.
pub fn antihorario(mut pts: Vec<P>) -> Vec<P> {
    if geo::area_poligono(&pts) < 0.0 {
        pts.reverse();
    }
    pts
}

pub fn es_convexo(pts: &[P]) -> bool {
    let n = pts.len();
    if n < 4 {
        return true;
    }
    let s = geo::area_poligono(pts).signum();
    (0..n).all(|i| {
        let (a, b, c) = (pts[i], pts[(i + 1) % n], pts[(i + 2) % n]);
        geo::cruz(geo::restar(b, a), geo::restar(c, b)) * s >= -1e-12
    })
}

fn dentro_triangulo(p: P, a: P, b: P, c: P) -> bool {
    let d1 = geo::cruz(geo::restar(b, a), geo::restar(p, a));
    let d2 = geo::cruz(geo::restar(c, b), geo::restar(p, b));
    let d3 = geo::cruz(geo::restar(a, c), geo::restar(p, c));
    d1 >= -1e-12 && d2 >= -1e-12 && d3 >= -1e-12
}

/// **Triangula un poligono simple** (antihorario) cortando orejas. Si el
/// poligono se corta a si mismo y no salen orejas, lo que queda va en
/// abanico (se ve, aunque no sea exacto).
pub fn triangular(pts: &[P]) -> Vec<[usize; 3]> {
    let n = pts.len();
    if n < 3 {
        return Vec::new();
    }
    let mut quedan: Vec<usize> = (0..n).collect();
    if geo::area_poligono(pts) < 0.0 {
        quedan.reverse();
    }
    let mut v = Vec::with_capacity(n - 2);
    let mut vueltas = 0;
    let mut i = 0;
    while quedan.len() > 3 {
        let m = quedan.len();
        let (ia, ib, ic) = (quedan[(i + m - 1) % m], quedan[i % m], quedan[(i + 1) % m]);
        let (a, b, c) = (pts[ia], pts[ib], pts[ic]);
        let convexo = geo::cruz(geo::restar(b, a), geo::restar(c, b)) > 1e-14;
        let oreja = convexo
            && quedan
                .iter()
                .filter(|&&k| k != ia && k != ib && k != ic)
                .all(|&k| {
                    let p = pts[k];
                    // Un vertice repetido en la esquina no estorba.
                    geo::dist(p, a) < 1e-12
                        || geo::dist(p, b) < 1e-12
                        || geo::dist(p, c) < 1e-12
                        || !dentro_triangulo(p, a, b, c)
                });
        if oreja {
            v.push([ia, ib, ic]);
            quedan.remove(i % m);
            vueltas = 0;
        } else {
            i += 1;
            vueltas += 1;
            if vueltas > 2 * m {
                break;
            }
        }
    }
    if quedan.len() == 3 {
        v.push([quedan[0], quedan[1], quedan[2]]);
    } else {
        for k in 1..quedan.len().saturating_sub(1) {
            v.push([quedan[0], quedan[k], quedan[k + 1]]);
        }
    }
    v
}

/// **Recorta un poligono convexo** con un semiplano: queda lo que cumple
/// `n · p <= d`.
pub fn recortar(pts: &[P], n: P, d: f64) -> Vec<P> {
    let mut v = Vec::new();
    let k = pts.len();
    for i in 0..k {
        let (a, b) = (pts[i], pts[(i + 1) % k]);
        let (da, db) = (geo::punto(n, a) - d, geo::punto(n, b) - d);
        if da <= 1e-12 {
            v.push(a);
        }
        if (da < -1e-12 && db > 1e-12) || (da > 1e-12 && db < -1e-12) {
            let t = da / (da - db);
            v.push(geo::sumar(a, geo::por(geo::restar(b, a), t)));
        }
    }
    limpiar(&v)
}

impl Prisma {
    /// Un prisma valido: planta de 3 puntos o mas con area y altura positiva.
    pub fn nuevo(planta: Vec<P>, z0: f64, alto: f64) -> Option<Prisma> {
        let (z0, alto) = if alto < 0.0 {
            (z0 + alto, -alto)
        } else {
            (z0, alto)
        };
        let planta = antihorario(limpiar(&planta));
        (planta.len() >= 3 && geo::area_poligono(&planta) > 1e-12 && alto > 1e-12)
            .then_some(Prisma { planta, z0, alto })
    }

    pub fn volumen(&self) -> f64 {
        geo::area_poligono(&self.planta) * self.alto
    }

    /// **Su malla cerrada**: la tapa de abajo (mirando abajo), la de arriba
    /// y un cuadrilatero por lado, hacia fuera. Las tapas concavas van en
    /// triangulos (asi se ven bien en cualquier programa).
    pub fn malla(&self) -> Malla {
        let n = self.planta.len() as u32;
        let z1 = self.z0 + self.alto;
        let mut puntos: Vec<P3> = self.planta.iter().map(|p| [p[0], p[1], self.z0]).collect();
        puntos.extend(self.planta.iter().map(|p| [p[0], p[1], z1]));
        let mut caras = Vec::new();
        if es_convexo(&self.planta) {
            caras.push((0..n).rev().collect());
            caras.push((n..2 * n).collect());
        } else {
            for t in triangular(&self.planta) {
                let [a, b, c] = t.map(|i| i as u32);
                caras.push(vec![c, b, a]);
                caras.push(vec![a + n, b + n, c + n]);
            }
        }
        for i in 0..n {
            let j = (i + 1) % n;
            caras.push(vec![i, j, j + n, i + n]);
        }
        Malla { puntos, caras }
    }
}

impl Malla {
    /// Junta otra malla a esta.
    pub fn anadir(&mut self, otra: &Malla) {
        let base = self.puntos.len() as u32;
        self.puntos.extend(&otra.puntos);
        self.caras.extend(
            otra.caras
                .iter()
                .map(|c| c.iter().map(|i| i + base).collect()),
        );
    }

    pub fn de_prismas(v: &[Prisma]) -> Malla {
        let mut m = Malla::default();
        for p in v {
            m.anadir(&p.malla());
        }
        m
    }

    /// **Los triangulos** de todas las caras (las de mas de tres lados,
    /// cortando orejas en su plano).
    pub fn triangulos(&self) -> Vec<[u32; 3]> {
        let mut v = Vec::new();
        for c in &self.caras {
            let c: Vec<u32> = c
                .iter()
                .copied()
                .filter(|&i| (i as usize) < self.puntos.len())
                .collect();
            match c.len() {
                0..=2 => {}
                3 => v.push([c[0], c[1], c[2]]),
                _ => {
                    let p3: Vec<P3> = c.iter().map(|&i| self.puntos[i as usize]).collect();
                    let n = normal_newell(&p3);
                    // Se proyecta en el plano del eje que mas mira la cara,
                    // conservando el sentido.
                    let (ax, ay, signo) = if n[2].abs() >= n[0].abs() && n[2].abs() >= n[1].abs() {
                        (0, 1, n[2].signum())
                    } else if n[0].abs() >= n[1].abs() {
                        (1, 2, n[0].signum())
                    } else {
                        (2, 0, n[1].signum())
                    };
                    let p2: Vec<P> = p3.iter().map(|p| [p[ax], p[ay] * signo]).collect();
                    for t in triangular(&p2) {
                        v.push(t.map(|k| c[k]));
                    }
                }
            }
        }
        v
    }

    /// La caja `[xmin, ymin, zmin, xmax, ymax, zmax]`.
    pub fn caja(&self) -> Option<[f64; 6]> {
        let i = f64::INFINITY;
        let mut caja = [i, i, i, -i, -i, -i];
        for p in &self.puntos {
            for k in 0..3 {
                caja[k] = caja[k].min(p[k]);
                caja[k + 3] = caja[k + 3].max(p[k]);
            }
        }
        caja[0].is_finite().then_some(caja)
    }

    /// **La entidad MESH** con estas caras (las concavas, en triangulos).
    pub fn a_mesh(&self) -> EntityType {
        let mut m = Mesh::new();
        for p in &self.puntos {
            m.add_vertex(Vector3::new(p[0], p[1], p[2]));
        }
        for c in &self.caras {
            if c.len() < 3 {
                continue;
            }
            let p3: Vec<P3> = c.iter().map(|&i| self.puntos[i as usize]).collect();
            let n = normal_newell(&p3);
            let plana = c.len() == 3 || {
                let (ax, ay) = if n[2].abs() >= n[0].abs() && n[2].abs() >= n[1].abs() {
                    (0, 1)
                } else if n[0].abs() >= n[1].abs() {
                    (1, 2)
                } else {
                    (2, 0)
                };
                es_convexo(&p3.iter().map(|p| [p[ax], p[ay]]).collect::<Vec<_>>())
            };
            if plana {
                m.add_face(MeshFace::new(c.iter().map(|&i| i as usize).collect()));
            } else {
                let sola = Malla {
                    puntos: self.puntos.clone(),
                    caras: vec![c.clone()],
                };
                for t in sola.triangulos() {
                    m.add_triangle(t[0] as usize, t[1] as usize, t[2] as usize);
                }
            }
        }
        m.compute_edges();
        EntityType::Mesh(m)
    }

    /// **Los prismas de que esta hecha**, si cada trozo suelto es un prisma
    /// recto (lo que hacen EXTRUIR, MURO3D y LOSA): asi el IFC los escribe
    /// como solidos extruidos y no como triangulos. `None` si algun trozo
    /// no lo es.
    pub fn prismas(&self) -> Option<Vec<Prisma>> {
        let n = self.puntos.len();
        if n == 0 || self.caras.is_empty() {
            return None;
        }
        // Los trozos sueltos: union de vertices por cara.
        let mut padre: Vec<usize> = (0..n).collect();
        fn raiz(p: &mut [usize], mut i: usize) -> usize {
            while p[i] != i {
                p[i] = p[p[i]];
                i = p[i];
            }
            i
        }
        for c in &self.caras {
            if c.iter().any(|&i| i as usize >= n) {
                return None;
            }
            for w in c.windows(2) {
                let (a, b) = (
                    raiz(&mut padre, w[0] as usize),
                    raiz(&mut padre, w[1] as usize),
                );
                padre[a] = b;
            }
        }
        let mut trozos: HashMap<usize, Vec<usize>> = HashMap::new();
        for (k, c) in self.caras.iter().enumerate() {
            if let Some(&i) = c.first() {
                trozos
                    .entry(raiz(&mut padre, i as usize))
                    .or_default()
                    .push(k);
            }
        }
        let mut claves: Vec<usize> = trozos.keys().copied().collect();
        claves.sort_unstable();
        claves.iter().map(|r| self.prisma_de(&trozos[r])).collect()
    }

    /// Un trozo como prisma: todas sus z en dos alturas, sus caras laterales
    /// verticales de cuatro lados, y los bordes de abajo de estas, un anillo.
    fn prisma_de(&self, caras: &[usize]) -> Option<Prisma> {
        let tol = 1e-7;
        let mut zs: Vec<f64> = caras
            .iter()
            .flat_map(|&k| self.caras[k].iter().map(|&i| self.puntos[i as usize][2]))
            .collect();
        zs.sort_by(f64::total_cmp);
        let (z0, z1) = (*zs.first()?, *zs.last()?);
        let escala = 1.0 + z0.abs().max(z1.abs());
        if z1 - z0 < tol * escala
            || zs
                .iter()
                .any(|z| (z - z0).abs() > tol * escala && (z - z1).abs() > tol * escala)
        {
            return None;
        }
        let abajo = |i: u32| (self.puntos[i as usize][2] - z0).abs() <= tol * escala;
        let xy = |i: u32| [self.puntos[i as usize][0], self.puntos[i as usize][1]];
        let mut bordes: Vec<(P, P)> = Vec::new();
        for &k in caras {
            let c = &self.caras[k];
            let cuantos_abajo = c.iter().filter(|&&i| abajo(i)).count();
            if cuantos_abajo == 0 || cuantos_abajo == c.len() {
                continue;
            }
            if c.len() != 4 || cuantos_abajo != 2 {
                return None;
            }
            // El borde de abajo, en el sentido de la cara (hacia fuera, con
            // la planta antihoraria).
            let pos = (0..4).find(|&m| abajo(c[m]) && abajo(c[(m + 1) % 4]))?;
            let (a, b) = (c[pos], c[(pos + 1) % 4]);
            let (d, e) = (c[(pos + 2) % 4], c[(pos + 3) % 4]);
            // Vertical: lo de arriba encima de lo de abajo.
            if geo::dist(xy(b), xy(d)) > tol * escala.max(1.0) * 10.0
                || geo::dist(xy(a), xy(e)) > tol * escala.max(1.0) * 10.0
            {
                return None;
            }
            bordes.push((xy(a), xy(b)));
        }
        if bordes.len() < 3 {
            return None;
        }
        // Encadenar los bordes en un anillo.
        let mut anillo = vec![bordes[0].0, bordes[0].1];
        let mut usados = vec![false; bordes.len()];
        usados[0] = true;
        for _ in 1..bordes.len() {
            let ult = *anillo.last()?;
            let k = (0..bordes.len())
                .find(|&k| !usados[k] && geo::dist(bordes[k].0, ult) <= 1e-6 * escala)?;
            usados[k] = true;
            anillo.push(bordes[k].1);
        }
        if geo::dist(anillo[0], *anillo.last()?) > 1e-6 * escala {
            return None;
        }
        anillo.pop();
        let p = Prisma::nuevo(anillo, z0, z1 - z0)?;
        // Con sus dos tapas enteras (un tubo sin tapas no es un solido).
        let area = geo::area_poligono(&p.planta);
        let tapa = |abajo_: bool| -> f64 {
            caras
                .iter()
                .map(|&k| &self.caras[k])
                .filter(|c| c.iter().all(|&i| abajo(i) == abajo_))
                .map(|c| {
                    let q: Vec<P> = c.iter().map(|&i| xy(i)).collect();
                    geo::area_poligono(&q).abs()
                })
                .sum()
        };
        let cerca = |a: f64| (a - area).abs() <= 1e-6 * area.max(1.0);
        (cerca(tapa(true)) && cerca(tapa(false))).then_some(p)
    }
}

/// **Las caras de una entidad 3D** (MESH, malla de caras, malla poligonal o
/// cara 3D) como [`Malla`]. `None` si no lo es.
pub fn malla_de_entidad(e: &EntityType) -> Option<Malla> {
    let v = |q: &Vector3| [q.x, q.y, q.z];
    match e {
        EntityType::Mesh(me) => Some(Malla {
            puntos: me.vertices.iter().map(v).collect(),
            caras: me
                .faces
                .iter()
                .map(|f| f.vertices.iter().map(|&i| i as u32).collect())
                .collect(),
        }),
        EntityType::PolyfaceMesh(p) => {
            let puntos: Vec<P3> = p.vertices.iter().map(|q| v(&q.location)).collect();
            let caras = p
                .faces
                .iter()
                .map(|f| {
                    f.vertex_indices()
                        .into_iter()
                        .filter(|&i| i != 0)
                        .map(|i| i.unsigned_abs() as u32 - 1)
                        .collect::<Vec<u32>>()
                })
                .filter(|c| c.len() >= 3 && c.iter().all(|&i| (i as usize) < puntos.len()))
                .collect();
            Some(Malla { puntos, caras })
        }
        EntityType::PolygonMesh(p) => {
            let (fm, cn) = (
                p.m_vertex_count.max(0) as usize,
                p.n_vertex_count.max(0) as usize,
            );
            let puntos: Vec<P3> = p.vertices.iter().map(|q| v(&q.location)).collect();
            if fm * cn != puntos.len() {
                return None;
            }
            let mut caras = Vec::new();
            for i in 0..fm.saturating_sub(1) {
                for j in 0..cn.saturating_sub(1) {
                    let k = |a: usize, b: usize| (a * cn + b) as u32;
                    caras.push(vec![k(i, j), k(i, j + 1), k(i + 1, j + 1), k(i + 1, j)]);
                }
            }
            Some(Malla { puntos, caras })
        }
        EntityType::Face3D(f) => {
            let mut puntos: Vec<P3> = f.corners().iter().map(v).collect();
            if puntos.len() == 4 && resta3(puntos[3], puntos[2]).iter().all(|d| d.abs() < 1e-12) {
                puntos.pop();
            }
            let caras = vec![(0..puntos.len() as u32).collect()];
            Some(Malla { puntos, caras })
        }
        _ => None,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn triangula_una_l_concava() {
        let l = [
            [0.0, 0.0],
            [2.0, 0.0],
            [2.0, 1.0],
            [1.0, 1.0],
            [1.0, 2.0],
            [0.0, 2.0],
        ];
        let t = triangular(&l);
        assert_eq!(t.len(), 4);
        let area: f64 = t
            .iter()
            .map(|t| geo::area_poligono(&[l[t[0]], l[t[1]], l[t[2]]]))
            .sum();
        assert!((area - 3.0).abs() < 1e-9);
        assert!(!es_convexo(&l));
        // Al reves, igual.
        let mut r = l.to_vec();
        r.reverse();
        assert_eq!(triangular(&r).len(), 4);
    }

    #[test]
    fn el_prisma_es_cerrado_y_se_reconoce_al_volver() {
        let p = Prisma::nuevo(
            vec![[0.0, 0.0], [0.0, 2.0], [3.0, 2.0], [3.0, 0.0]],
            1.0,
            5.0,
        )
        .unwrap();
        // Lo dio horario: queda antihorario.
        assert!(geo::area_poligono(&p.planta) > 0.0);
        let m = p.malla();
        assert_eq!(m.caras.len(), 6);
        assert_eq!(m.triangulos().len(), 12);
        let vuelta = m.prismas().unwrap();
        assert_eq!(vuelta.len(), 1);
        assert!((vuelta[0].volumen() - 30.0).abs() < 1e-9);
        assert!((vuelta[0].z0 - 1.0).abs() < 1e-12);
        // Tambien pasando por MESH (y con las tapas concavas en triangulos).
        let l = Prisma::nuevo(
            vec![
                [0.0, 0.0],
                [2.0, 0.0],
                [2.0, 1.0],
                [1.0, 1.0],
                [1.0, 2.0],
                [0.0, 2.0],
            ],
            0.0,
            1.0,
        )
        .unwrap();
        let dos = Malla::de_prismas(&[p.clone(), l]);
        let me = dos.a_mesh();
        let otra = malla_de_entidad(&me).unwrap();
        let v = otra.prismas().unwrap();
        assert_eq!(v.len(), 2);
        let vol: f64 = v.iter().map(Prisma::volumen).sum();
        assert!((vol - 33.0).abs() < 1e-9, "{vol}");
    }

    #[test]
    fn una_cara_suelta_no_es_prisma() {
        let m = Malla {
            puntos: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 1.0]],
            caras: vec![vec![0, 1, 2]],
        };
        assert!(m.prismas().is_none());
        // Un tubo sin tapas (una polilinea cerrada con altura) tampoco.
        let mut tubo = Prisma::nuevo(vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]], 0.0, 1.0)
            .unwrap()
            .malla();
        tubo.caras.drain(..2);
        assert!(tubo.prismas().is_none());
        assert!(Prisma::nuevo(vec![[0.0, 0.0], [1.0, 0.0]], 0.0, 1.0).is_none());
        assert!(Prisma::nuevo(vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]], 0.0, 0.0).is_none());
    }

    #[test]
    fn un_circulo_en_tramos() {
        let a = crate::patrones::anillo_circulo([0.0, 0.0], 1.0);
        let p = poligono_de_anillo(&a);
        assert!(p.len() >= 32);
        assert!((geo::area_poligono(&p) - PI).abs() < 0.02);
    }

    #[test]
    fn recorta_un_cuadrado_por_la_mitad() {
        let c = [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]];
        let r = recortar(&c, [1.0, 0.0], 1.0);
        assert!((geo::area_poligono(&r) - 2.0).abs() < 1e-12);
    }
}
