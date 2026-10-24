//! **Un modelo 3D (BIM) listo para la tarjeta**: triangulos con su normal,
//! su color y el elemento del que salen, mas las aristas vivas (las que
//! marcan la forma) para dibujarlas encima como rayas finas. Lo llena
//! `pixpin-bim` al leer un IFC (o un Revit) y lo dibuja [`crate::ventana3d`].
//!
//! Los puntos van relativos a [`Modelo3d::origen`]: un modelo
//! georreferenciado esta a cientos de kilometros del cero y en `f32` sus
//! milimetros se perderian.

use std::collections::HashMap;

/// Un vertice: 24 bytes, tal cual se sube.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct V3 {
    pub pos: [f32; 3],
    /// La normal en 3 bytes con signo (x, y, z) y uno libre.
    pub normal: u32,
    /// RGBA, rojo en el byte bajo.
    pub color: u32,
    /// El indice del elemento en [`Modelo3d::elementos`].
    pub elemento: u32,
}

/// De que es cada parte: para nombrarla al tocarla.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Elemento {
    /// La clase IFC tal cual (`IfcWall`).
    pub tipo: String,
    pub nombre: String,
    /// En que nivel esta (indice en [`Modelo3d::niveles`]).
    pub nivel: Option<u32>,
    /// **Su clave estable** (20-oct-2026): la que no cambia aunque el
    /// modelo se lea en otro orden, con otra version del lector o en
    /// Android. En un IFC (y un Revit, que se pasa a IFC), su `GlobalId`;
    /// vacia si el formato no trae ninguna (ver
    /// [`crate::marcadores::Claves`], que entonces usa tipo, nombre y orden).
    pub clave: String,
    /// Lo que mide, sacado de sus triangulos (en unidades del modelo).
    pub medidas: Medidas,
}

/// Lo que mide un elemento, de su malla.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Medidas {
    /// El volumen encerrado (si la malla no es cerrada, aproximado).
    pub volumen: f32,
    /// El area en planta: lo que mira arriba, proyectado.
    pub planta: f32,
    /// El area de una cara en alzado: lo vertical, entre dos.
    pub alzado: f32,
    /// Toda la superficie.
    pub superficie: f32,
    /// Si la malla es cerrada (cada arista la comparten dos triangulos):
    /// solo entonces el volumen es exacto.
    pub cerrado: bool,
}

/// Un nivel (planta) del edificio: el `IfcBuildingStorey` o el Level de Revit.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Nivel {
    pub nombre: String,
    /// La cota, en metros.
    pub cota: f64,
    /// Su clave estable: el `GlobalId` del `IfcBuildingStorey`; vacia si
    /// no la hay (entonces cuentan el nombre y la cota).
    pub clave: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Modelo3d {
    pub origen: [f64; 3],
    /// Minimo y maximo (x, y, z), relativos al origen. Z es arriba.
    pub caja: [f32; 6],
    pub vertices: Vec<V3>,
    /// Triangulos opacos y, aparte, los transparentes (vidrios): estos van
    /// despues y sin escribir la profundidad.
    pub opacos: Vec<u32>,
    pub transparentes: Vec<u32>,
    /// Pares de indices: las aristas vivas.
    pub aristas: Vec<u32>,
    /// Pares de indices: rayas sueltas (ejes, curvas de nivel, tuberias).
    pub lineas: Vec<u32>,
    /// Puntos: cada uno, 4 vertices en el mismo sitio (la esquina va en el
    /// byte alto de la normal) y 6 indices; se ven de un tamaño fijo en
    /// pantalla.
    pub puntos: Vec<u32>,
    pub elementos: Vec<Elemento>,
    /// Los niveles, de abajo arriba (vacio si el modelo no los tiene).
    pub niveles: Vec<Nivel>,
    /// 1 si las medidas estan en metros (lo normal en IFC tras leerlo).
    pub metros: f32,
}

pub fn normal_a_u32(n: [f32; 3]) -> u32 {
    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-20);
    let c = |v: f32| ((v / l).clamp(-1.0, 1.0) * 127.0).round() as i8 as u8 as u32;
    c(n[0]) | c(n[1]) << 8 | c(n[2]) << 16
}

pub fn color_a_u32(c: [f32; 4]) -> u32 {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    b(c[0]) | b(c[1]) << 8 | b(c[2]) << 16 | b(c[3]) << 24
}

/// Para ir llenando el modelo malla a malla, en coordenadas del mundo.
pub struct Constructor3d {
    m: Modelo3d,
    origen: Option<[f64; 3]>,
    min: [f64; 3],
    max: [f64; 3],
}

impl Default for Constructor3d {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl Constructor3d {
    pub fn nuevo() -> Constructor3d {
        Constructor3d {
            m: Modelo3d {
                metros: 1.0,
                ..Default::default()
            },
            origen: None,
            min: [f64::INFINITY; 3],
            max: [f64::NEG_INFINITY; 3],
        }
    }

    pub fn elemento(&mut self, tipo: &str, nombre: &str) -> u32 {
        self.m.elementos.push(Elemento {
            tipo: tipo.to_string(),
            nombre: nombre.to_string(),
            ..Default::default()
        });
        (self.m.elementos.len() - 1) as u32
    }

    /// La clave estable de un elemento ya creado (su `GlobalId`).
    pub fn poner_clave(&mut self, elemento: u32, clave: &str) {
        if let Some(el) = self.m.elementos.get_mut(elemento as usize) {
            el.clave = clave.to_string();
        }
    }

    /// Una malla: `puntos` del mundo, `normales` (pueden faltar: se calculan
    /// por cara), `indices` de a tres y su color RGBA (0..1).
    pub fn malla(
        &mut self,
        elemento: u32,
        puntos: &[[f64; 3]],
        normales: Option<&[[f32; 3]]>,
        indices: &[u32],
        color: [f32; 4],
    ) {
        if puntos.is_empty() || indices.len() < 3 {
            return;
        }
        let o = *self.origen.get_or_insert(puntos[0]);
        let base = self.m.vertices.len() as u32;
        let _ = &o;
        let col = color_a_u32(color);
        let normales = normales.filter(|n| n.len() == puntos.len());
        // Sin normales: un vertice por esquina de cada triangulo (caras planas).
        let (mut pts_locales, mut idx): (Vec<[f32; 3]>, Vec<u32>) = (Vec::new(), Vec::new());
        let mut norms: Vec<[f32; 3]> = Vec::new();
        match normales {
            Some(n) => {
                pts_locales = puntos
                    .iter()
                    .map(|p| {
                        [
                            (p[0] - o[0]) as f32,
                            (p[1] - o[1]) as f32,
                            (p[2] - o[2]) as f32,
                        ]
                    })
                    .collect();
                norms = n.to_vec();
                idx = indices
                    .iter()
                    .copied()
                    .filter(|&i| (i as usize) < puntos.len())
                    .collect();
                idx.truncate(idx.len() / 3 * 3);
            }
            None => {
                for t in indices.chunks_exact(3) {
                    if t.iter().any(|&i| i as usize >= puntos.len()) {
                        continue;
                    }
                    let p: Vec<[f32; 3]> = t
                        .iter()
                        .map(|&i| puntos[i as usize])
                        .map(|p| {
                            [
                                (p[0] - o[0]) as f32,
                                (p[1] - o[1]) as f32,
                                (p[2] - o[2]) as f32,
                            ]
                        })
                        .collect();
                    let n = cruz(resta(p[1], p[0]), resta(p[2], p[0]));
                    for q in p {
                        idx.push(pts_locales.len() as u32);
                        pts_locales.push(q);
                        norms.push(n);
                    }
                }
            }
        }
        for p in puntos {
            self.ampliar_caja(*p);
        }
        for (p, n) in pts_locales.iter().zip(&norms) {
            self.m.vertices.push(V3 {
                pos: *p,
                normal: normal_a_u32(*n),
                color: col,
                elemento,
            });
        }
        let destino = if color[3] < 0.98 {
            &mut self.m.transparentes
        } else {
            &mut self.m.opacos
        };
        destino.extend(idx.iter().map(|i| i + base));
        if color[3] >= 0.98 {
            let aristas = aristas_vivas(&pts_locales, &idx, 30.0);
            self.m.aristas.extend(aristas.iter().map(|i| i + base));
        }
    }

    /// La caja del modelo, para que quepa `p`.
    fn ampliar_caja(&mut self, p: [f64; 3]) {
        for ((min, max), v) in self.min.iter_mut().zip(self.max.iter_mut()).zip(p) {
            *min = min.min(v);
            *max = max.max(v);
        }
    }

    fn local(&mut self, p: [f64; 3]) -> [f32; 3] {
        let o = *self.origen.get_or_insert(p);
        self.ampliar_caja(p);
        [
            (p[0] - o[0]) as f32,
            (p[1] - o[1]) as f32,
            (p[2] - o[2]) as f32,
        ]
    }

    /// Una polilinea del mundo.
    pub fn linea(&mut self, elemento: u32, puntos: &[[f64; 3]], color: [f32; 4]) {
        if puntos.len() < 2 {
            return;
        }
        let col = color_a_u32(color);
        let base = self.m.vertices.len() as u32;
        for p in puntos {
            let pos = self.local(*p);
            self.m.vertices.push(V3 {
                pos,
                normal: 0,
                color: col,
                elemento,
            });
        }
        for i in 0..puntos.len() as u32 - 1 {
            self.m.lineas.extend([base + i, base + i + 1]);
        }
    }
    /// Segmentos sueltos: `puntos` de dos en dos.
    /// Segmentos sueltos:  de dos en dos.
    pub fn segmentos(&mut self, elemento: u32, puntos: &[[f64; 3]], color: [f32; 4]) {
        let col = color_a_u32(color);
        for par in puntos.chunks_exact(2) {
            let base = self.m.vertices.len() as u32;
            for p in par {
                let pos = self.local(*p);
                self.m.vertices.push(V3 {
                    pos,
                    normal: 0,
                    color: col,
                    elemento,
                });
            }
            self.m.lineas.extend([base, base + 1]);
        }
    }

    /// Una malla sin aristas vivas (un terreno: sus pliegues no son aristas).
    pub fn malla_sin_aristas(
        &mut self,
        elemento: u32,
        puntos: &[[f64; 3]],
        indices: &[u32],
        color: [f32; 4],
    ) {
        let antes = self.m.aristas.len();
        self.malla(elemento, puntos, None, indices, color);
        self.m.aristas.truncate(antes);
    }

    /// Un punto del mundo.
    pub fn punto(&mut self, elemento: u32, p: [f64; 3], color: [f32; 4]) {
        let col = color_a_u32(color);
        let pos = self.local(p);
        let base = self.m.vertices.len() as u32;
        for k in 0..4u32 {
            self.m.vertices.push(V3 {
                pos,
                normal: (0x80 | k) << 24,
                color: col,
                elemento,
            });
        }
        self.m
            .puntos
            .extend([base, base + 1, base + 2, base + 1, base + 3, base + 2]);
    }

    /// Los niveles del modelo y en cual esta cada elemento
    /// (`(elemento, nivel)`). Se ordenan por cota.
    pub fn niveles(&mut self, niveles: Vec<Nivel>, de: &[(u32, u32)]) {
        let mut orden: Vec<usize> = (0..niveles.len()).collect();
        orden.sort_by(|a, b| niveles[*a].cota.total_cmp(&niveles[*b].cota));
        let mut nuevo = vec![0u32; niveles.len()];
        for (i, k) in orden.iter().enumerate() {
            nuevo[*k] = i as u32;
        }
        self.m.niveles = orden.iter().map(|k| niveles[*k].clone()).collect();
        for (e, n) in de {
            if let (Some(el), Some(n)) = (
                self.m.elementos.get_mut(*e as usize),
                nuevo.get(*n as usize),
            ) {
                el.nivel = Some(*n);
            }
        }
    }

    pub fn terminar(mut self) -> Modelo3d {
        let o = self.origen.unwrap_or([0.0; 3]);
        self.m.origen = o;
        if self.min[0].is_finite() {
            self.m.caja = [
                (self.min[0] - o[0]) as f32,
                (self.min[1] - o[1]) as f32,
                (self.min[2] - o[2]) as f32,
                (self.max[0] - o[0]) as f32,
                (self.max[1] - o[1]) as f32,
                (self.max[2] - o[2]) as f32,
            ];
        }
        medir(&mut self.m);
        self.m
    }
}

/// **Las medidas de cada elemento**, de sus triangulos: el volumen por el
/// teorema de la divergencia (la suma de los tetraedros con el origen), las
/// areas por la normal de cada triangulo, y si la malla es cerrada (cada
/// arista, por su posicion, la comparten un numero par de triangulos).
pub fn medir(m: &mut Modelo3d) {
    let n = m.elementos.len();
    if n == 0 {
        return;
    }
    let mut vol = vec![0f64; n];
    let mut me = vec![Medidas::default(); n];
    // La rejilla para juntar vertices repetidos (uno por cara): una
    // millonesima del modelo.
    let lado = (m.caja[3] - m.caja[0])
        .max(m.caja[4] - m.caja[1])
        .max(m.caja[5] - m.caja[2])
        .max(1e-6) as f64;
    let q = |p: [f32; 3]| -> u64 {
        let k = |v: f32| ((v as f64 / lado * 1.0e6).round() as i64 & 0x1f_ffff) as u64;
        k(p[0]) | k(p[1]) << 21 | k(p[2]) << 42
    };
    let mut aristas: Vec<(u32, u64, u64)> = Vec::new();
    for lista in [&m.opacos, &m.transparentes] {
        for t in lista.chunks_exact(3) {
            let (a, b, c) = (
                m.vertices[t[0] as usize],
                m.vertices[t[1] as usize],
                m.vertices[t[2] as usize],
            );
            let e = a.elemento as usize;
            if e >= n {
                continue;
            }
            let (p0, p1, p2) = (a.pos, b.pos, c.pos);
            let d = |u: [f32; 3]| [u[0] as f64, u[1] as f64, u[2] as f64];
            let (p0, p1, p2) = (d(p0), d(p1), d(p2));
            let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
            let v = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
            let c3 = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            let area = 0.5 * (c3[0] * c3[0] + c3[1] * c3[1] + c3[2] * c3[2]).sqrt();
            if area <= 0.0 {
                continue;
            }
            // p0 · (p1 × p2) / 6
            let x = [
                p1[1] * p2[2] - p1[2] * p2[1],
                p1[2] * p2[0] - p1[0] * p2[2],
                p1[0] * p2[1] - p1[1] * p2[0],
            ];
            vol[e] += (p0[0] * x[0] + p0[1] * x[1] + p0[2] * x[2]) / 6.0;
            let nz = c3[2] / (2.0 * area);
            let mm = &mut me[e];
            mm.superficie += area as f32;
            if nz > 0.0 {
                mm.planta += (area * nz) as f32;
            }
            if nz.abs() < 0.3 {
                mm.alzado += (area / 2.0) as f32;
            }
            let (k0, k1, k2) = (q(a.pos), q(b.pos), q(c.pos));
            for (x, y) in [(k0, k1), (k1, k2), (k2, k0)] {
                if x != y {
                    aristas.push((e as u32, x.min(y), x.max(y)));
                }
            }
        }
    }
    aristas.sort_unstable();
    let mut abierto = vec![false; n];
    let mut i = 0;
    while i < aristas.len() {
        let mut j = i + 1;
        while j < aristas.len() && aristas[j] == aristas[i] {
            j += 1;
        }
        if (j - i) % 2 == 1 {
            abierto[aristas[i].0 as usize] = true;
        }
        i = j;
    }
    for (k, el) in m.elementos.iter_mut().enumerate() {
        let mut mm = me[k];
        mm.volumen = vol[k].abs() as f32;
        mm.cerrado = mm.superficie > 0.0 && !abierto[k];
        el.medidas = mm;
    }
}

fn resta(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cruz(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unitario(a: [f32; 3]) -> Option<[f32; 3]> {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    (l > 1e-12).then(|| [a[0] / l, a[1] / l, a[2] / l])
}

/// Las aristas que marcan la forma: entre dos caras que doblan mas de
/// `grados`, o el borde de una cara suelta. Los puntos repetidos (la misma
/// esquina en dos caras) se juntan por posicion.
pub fn aristas_vivas(puntos: &[[f32; 3]], indices: &[u32], grados: f32) -> Vec<u32> {
    let mut lado = 0f32;
    for p in puntos {
        for v in p {
            lado = lado.max(v.abs());
        }
    }
    let celda = (lado * 1e-6).max(1e-6);
    let mut por_sitio: HashMap<[i64; 3], u32> = HashMap::new();
    let soldado: Vec<u32> = puntos
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let k = [
                (p[0] / celda).round() as i64,
                (p[1] / celda).round() as i64,
                (p[2] / celda).round() as i64,
            ];
            *por_sitio.entry(k).or_insert(i as u32)
        })
        .collect();
    // Cada arista (sus dos extremos soldados) con las normales de sus caras.
    // Las normales de sus dos caras, cuantas caras y los indices originales.
    type Caras = (Option<[f32; 3]>, Option<[f32; 3]>, u8, (u32, u32));
    let mut aristas: HashMap<(u32, u32), Caras> = HashMap::new();
    for t in indices.chunks_exact(3) {
        let p = [
            puntos[t[0] as usize],
            puntos[t[1] as usize],
            puntos[t[2] as usize],
        ];
        let n = unitario(cruz(resta(p[1], p[0]), resta(p[2], p[0])));
        if n.is_none() {
            continue;
        }
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let (sa, sb) = (soldado[a as usize], soldado[b as usize]);
            if sa == sb {
                continue;
            }
            let k = (sa.min(sb), sa.max(sb));
            let e = aristas.entry(k).or_insert((None, None, 0, (a, b)));
            if e.2 == 0 {
                e.0 = n;
            } else if e.2 == 1 {
                e.1 = n;
            }
            e.2 = e.2.saturating_add(1);
        }
    }
    let umbral = grados.to_radians().cos();
    let mut out = Vec::new();
    for (_, (n0, n1, veces, (a, b))) in aristas {
        let viva = match (veces, n0, n1) {
            (1, ..) => true,
            (2, Some(x), Some(y)) => x[0] * y[0] + x[1] * y[1] + x[2] * y[2] < umbral,
            _ => false,
        };
        if viva {
            out.extend([a, b]);
        }
    }
    out
}

// La 4 (20-oct-2026) trae la clave estable de cada elemento y nivel: la
// cache de antes no se entiende y se vuelve a leer el modelo.
const MAGIA: &[u8; 8] = b"PX3D\0\0\0\x04";

impl Modelo3d {
    /// **La clave estable del elemento `el`** (la que guardan los
    /// marcadores, 20-oct-2026): su `GlobalId` si lo trae; si no,
    /// `~<tipo>|<nombre>|<k>`, con `k` el orden entre los de su mismo tipo y
    /// nombre (lo que mas aguanta que se lea en otro orden).
    pub fn clave_de_elemento(&self, el: u32) -> Option<String> {
        let e = self.elementos.get(el as usize)?;
        if !e.clave.is_empty() {
            return Some(e.clave.clone());
        }
        let k = self.elementos[..el as usize]
            .iter()
            .filter(|o| o.clave.is_empty() && o.tipo == e.tipo && o.nombre == e.nombre)
            .count();
        Some(format!("~{}|{}|{k}", e.tipo, e.nombre))
    }

    /// Y al reves: el elemento de esa clave en este modelo.
    pub fn elemento_de_clave(&self, clave: &str) -> Option<u32> {
        if !clave.starts_with('~') {
            return self
                .elementos
                .iter()
                .position(|e| e.clave == clave)
                .map(|i| i as u32);
        }
        (0..self.elementos.len() as u32)
            .find(|&i| self.clave_de_elemento(i).as_deref() == Some(clave))
    }

    /// **La clave estable del nivel `k`**: su `GlobalId`; si no, el nombre y
    /// la cota en milimetros (`<nombre>@<mm>`).
    pub fn clave_de_nivel(&self, k: u32) -> Option<String> {
        let n = self.niveles.get(k as usize)?;
        Some(if n.clave.is_empty() {
            format!("{}@{}", n.nombre, (n.cota * 1000.0).round() as i64)
        } else {
            n.clave.clone()
        })
    }

    /// Y al reves: el nivel de esa clave en este modelo.
    pub fn nivel_de_clave(&self, clave: &str) -> Option<u32> {
        (0..self.niveles.len() as u32).find(|&k| self.clave_de_nivel(k).as_deref() == Some(clave))
    }

    /// La caja de lo que importa: sin los pocos puntos sueltos muy lejos
    /// (una raya con una cota absurda en un plano) que harian ver el resto
    /// diminuto. Del 0,5 % al 99,5 % en cada eje, con un margen.
    pub fn caja_util(&self) -> [f32; 6] {
        let n = self.vertices.len();
        if n < 200 {
            return self.caja;
        }
        let paso = (n / 20_000).max(1);
        let mut out = self.caja;
        for k in 0..3 {
            let mut v: Vec<f32> = self
                .vertices
                .iter()
                .step_by(paso)
                .map(|p| p.pos[k])
                .filter(|x| x.is_finite())
                .collect();
            if v.len() < 100 {
                continue;
            }
            v.sort_by(f32::total_cmp);
            let (a, b) = (v[v.len() / 200], v[v.len() - 1 - v.len() / 200]);
            let margen = (b - a) * 0.05;
            out[k] = (a - margen).max(self.caja[k]);
            out[k + 3] = (b + margen).min(self.caja[k + 3]);
        }
        out
    }

    pub fn vacio(&self) -> bool {
        self.opacos.is_empty()
            && self.transparentes.is_empty()
            && self.lineas.is_empty()
            && self.puntos.is_empty()
    }

    pub fn a_bytes(&self) -> Vec<u8> {
        let indices = self.opacos.len()
            + self.transparentes.len()
            + self.aristas.len()
            + self.lineas.len()
            + self.puntos.len();
        let mut b: Vec<u8> = Vec::with_capacity(self.vertices.len() * 24 + indices * 4 + 1024);
        b.extend_from_slice(MAGIA);
        for v in self.origen {
            b.extend_from_slice(&v.to_le_bytes());
        }
        for v in self.caja {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(&self.metros.to_le_bytes());
        b.extend_from_slice(&(self.vertices.len() as u32).to_le_bytes());
        for v in &self.vertices {
            for x in v.pos {
                b.extend_from_slice(&x.to_le_bytes());
            }
            for x in [v.normal, v.color, v.elemento] {
                b.extend_from_slice(&x.to_le_bytes());
            }
        }
        for l in [
            &self.opacos,
            &self.transparentes,
            &self.aristas,
            &self.lineas,
            &self.puntos,
        ] {
            b.extend_from_slice(&(l.len() as u32).to_le_bytes());
            for x in l.iter() {
                b.extend_from_slice(&x.to_le_bytes());
            }
        }
        b.extend_from_slice(&(self.elementos.len() as u32).to_le_bytes());
        for e in &self.elementos {
            for s in [&e.tipo, &e.nombre, &e.clave] {
                b.extend_from_slice(&(s.len() as u32).to_le_bytes());
                b.extend_from_slice(s.as_bytes());
            }
            b.extend_from_slice(&e.nivel.unwrap_or(u32::MAX).to_le_bytes());
            let md = e.medidas;
            for x in [md.volumen, md.planta, md.alzado, md.superficie] {
                b.extend_from_slice(&x.to_le_bytes());
            }
            b.push(md.cerrado as u8);
        }
        b.extend_from_slice(&(self.niveles.len() as u32).to_le_bytes());
        for nv in &self.niveles {
            b.extend_from_slice(&(nv.nombre.len() as u32).to_le_bytes());
            b.extend_from_slice(nv.nombre.as_bytes());
            b.extend_from_slice(&nv.cota.to_le_bytes());
            b.extend_from_slice(&(nv.clave.len() as u32).to_le_bytes());
            b.extend_from_slice(nv.clave.as_bytes());
        }
        b
    }

    pub fn de_bytes(b: &[u8]) -> Option<Modelo3d> {
        let mut r = Lector { b, i: 0 };
        if r.tomar(8)? != MAGIA {
            return None;
        }
        let mut m = Modelo3d::default();
        for k in 0..3 {
            m.origen[k] = f64::from_le_bytes(r.tomar(8)?.try_into().ok()?);
        }
        for k in 0..6 {
            m.caja[k] = r.f()?;
        }
        m.metros = r.f()?;
        let n = r.u()? as usize;
        if n > b.len() / 24 {
            return None;
        }
        m.vertices.reserve(n);
        for _ in 0..n {
            m.vertices.push(V3 {
                pos: [r.f()?, r.f()?, r.f()?],
                normal: r.u()?,
                color: r.u()?,
                elemento: r.u()?,
            });
        }
        for l in [
            &mut m.opacos,
            &mut m.transparentes,
            &mut m.aristas,
            &mut m.lineas,
            &mut m.puntos,
        ] {
            let n = r.u()? as usize;
            if n > b.len() / 4 {
                return None;
            }
            l.reserve(n);
            for _ in 0..n {
                let i = r.u()?;
                if i as usize >= m.vertices.len() {
                    return None;
                }
                l.push(i);
            }
        }
        let n = r.u()? as usize;
        if n > b.len() {
            return None;
        }
        for _ in 0..n {
            let tipo = r.texto()?;
            let nombre = r.texto()?;
            let clave = r.texto()?;
            let nivel = Some(r.u()?).filter(|n| *n != u32::MAX);
            let medidas = Medidas {
                volumen: r.f()?,
                planta: r.f()?,
                alzado: r.f()?,
                superficie: r.f()?,
                cerrado: r.tomar(1)?[0] != 0,
            };
            m.elementos.push(Elemento {
                tipo,
                nombre,
                nivel,
                clave,
                medidas,
            });
        }
        let n = r.u()? as usize;
        if n > b.len() {
            return None;
        }
        for _ in 0..n {
            let nombre = r.texto()?;
            let cota = f64::from_le_bytes(r.tomar(8)?.try_into().ok()?);
            let clave = r.texto()?;
            m.niveles.push(Nivel {
                nombre,
                cota,
                clave,
            });
        }
        if m.elementos
            .iter()
            .any(|e| e.nivel.is_some_and(|k| k as usize >= m.niveles.len()))
        {
            return None;
        }
        Some(m)
    }
}

struct Lector<'a> {
    b: &'a [u8],
    i: usize,
}

impl Lector<'_> {
    fn tomar(&mut self, n: usize) -> Option<&[u8]> {
        let s = self.b.get(self.i..self.i.checked_add(n)?)?;
        self.i += n;
        Some(s)
    }
    fn u(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.tomar(4)?.try_into().ok()?))
    }
    fn f(&mut self) -> Option<f32> {
        Some(f32::from_bits(self.u()?))
    }
    fn texto(&mut self) -> Option<String> {
        let n = self.u()? as usize;
        Some(String::from_utf8_lossy(self.tomar(n)?).into_owned())
    }
}

/// El nombre en español de las clases IFC mas comunes.
pub fn tipo_legible(tipo: &str) -> String {
    let t = tipo.strip_suffix("StandardCase").unwrap_or(tipo);
    let t = t.strip_suffix("ElementedCase").unwrap_or(t);
    let es = match t.to_ascii_lowercase().as_str() {
        "ifcwall" => "Muro",
        "ifccurtainwall" => "Muro cortina",
        "ifcslab" => "Losa",
        "ifcroof" => "Techo",
        "ifccolumn" => "Columna",
        "ifcbeam" => "Viga",
        "ifcmember" => "Elemento estructural",
        "ifcplate" => "Placa",
        "ifcdoor" => "Puerta",
        "ifcwindow" => "Ventana",
        "ifcstair" => "Escalera",
        "ifcstairflight" => "Tramo de escalera",
        "ifcramp" => "Rampa",
        "ifcrampflight" => "Tramo de rampa",
        "ifcrailing" => "Baranda",
        "ifccovering" => "Revestimiento",
        "ifcfooting" => "Zapata",
        "ifcpile" => "Pilote",
        "ifcfurnishingelement" | "ifcfurniture" => "Mobiliario",
        "ifcbuildingelementproxy" => "Elemento",
        "ifcspace" => "Espacio",
        "ifcopeningelement" => "Vano",
        "ifcreinforcingbar" => "Acero de refuerzo",
        "ifcreinforcingmesh" => "Malla de refuerzo",
        "ifcpipesegment" => "Tuberia",
        "ifcpipefitting" => "Accesorio de tuberia",
        "ifcductsegment" => "Ducto",
        "ifcductfitting" => "Accesorio de ducto",
        "ifcflowterminal" | "ifcsanitaryterminal" => "Aparato",
        "ifccablecarriersegment" => "Bandeja de cables",
        "ifclightfixture" => "Luminaria",
        "ifcsite" => "Terreno",
        "ifcgeographicelement" => "Elemento del terreno",
        "ifcearthworksfill" | "ifcearthworkscut" => "Movimiento de tierras",
        "ifcbearing" => "Apoyo",
        "ifctendon" => "Cable de postensado",
        "ifccourse" => "Capa",
        "ifcpavement" => "Pavimento",
        "ifcbridge" | "ifcbridgepart" => "Puente",
        "ifcbuildingelementpart" | "ifcelementassembly" => "Conjunto",
        "ifcchimney" => "Chimenea",
        "ifcshadingdevice" => "Parasol",
        _ => "",
    };
    if es.is_empty() {
        tipo.strip_prefix("Ifc").unwrap_or(tipo).to_string()
    } else {
        es.to_string()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn cubo(c: &mut Constructor3d, e: u32, color: [f32; 4]) {
        let p: Vec<[f64; 3]> = (0..8)
            .map(|i| {
                [
                    (i & 1) as f64,
                    ((i >> 1) & 1) as f64,
                    ((i >> 2) & 1) as f64 + 1000.0,
                ]
            })
            .collect();
        let caras = [
            [0, 1, 3, 2],
            [4, 6, 7, 5],
            [0, 4, 5, 1],
            [2, 3, 7, 6],
            [0, 2, 6, 4],
            [1, 5, 7, 3],
        ];
        let mut idx = Vec::new();
        for f in caras {
            idx.extend([f[0], f[1], f[2], f[0], f[2], f[3]]);
        }
        c.malla(e, &p, None, &idx, color);
    }

    #[test]
    fn un_cubo_tiene_doce_aristas_vivas_y_no_las_diagonales() {
        let mut c = Constructor3d::nuevo();
        let e = c.elemento("IfcWall", "Muro 1");
        cubo(&mut c, e, [0.8, 0.8, 0.8, 1.0]);
        let m = c.terminar();
        assert_eq!(m.opacos.len(), 36);
        assert_eq!(m.aristas.len(), 24);
        // Relativo al origen: nada a mil metros.
        assert!(m.caja[5] <= 1.0 + 1e-6 && m.caja[2] >= -1e-6);
        assert_eq!(m.origen[2], 1000.0);
    }

    #[test]
    fn un_vidrio_va_aparte_y_sin_aristas() {
        let mut c = Constructor3d::nuevo();
        let e = c.elemento("IfcWindow", "");
        cubo(&mut c, e, [0.5, 0.7, 0.9, 0.3]);
        let m = c.terminar();
        assert!(m.opacos.is_empty() && m.transparentes.len() == 36 && m.aristas.is_empty());
    }

    #[test]
    fn un_cubo_de_un_metro_mide_lo_suyo() {
        let mut c = Constructor3d::nuevo();
        let e = c.elemento("IfcWall", "");
        cubo(&mut c, e, [0.8, 0.8, 0.8, 1.0]);
        let md = c.terminar().elementos[0].medidas;
        assert!((md.volumen - 1.0).abs() < 1e-4, "{md:?}");
        assert!(
            (md.planta - 1.0).abs() < 1e-4
                && (md.alzado - 2.0).abs() < 1e-4
                && (md.superficie - 6.0).abs() < 1e-4
        );
        assert!(md.cerrado);
        // Caso negativo: sin la tapa de arriba, abierto.
        let mut c = Constructor3d::nuevo();
        let e = c.elemento("IfcWall", "");
        let p: Vec<[f64; 3]> = (0..8)
            .map(|i| [(i & 1) as f64, ((i >> 1) & 1) as f64, ((i >> 2) & 1) as f64])
            .collect();
        let mut idx = Vec::new();
        for f in [
            [0, 1, 3, 2],
            [0, 4, 5, 1],
            [2, 3, 7, 6],
            [0, 2, 6, 4],
            [1, 5, 7, 3],
        ] {
            idx.extend([f[0], f[1], f[2], f[0], f[2], f[3]]);
        }
        c.malla(e, &p, None, &idx, [0.8, 0.8, 0.8, 1.0]);
        assert!(!c.terminar().elementos[0].medidas.cerrado);
    }

    #[test]
    fn los_niveles_se_ordenan_por_cota() {
        let mut c = Constructor3d::nuevo();
        let a = c.elemento("IfcWall", "");
        let b = c.elemento("IfcSlab", "");
        let _sin = c.elemento("IfcBeam", "");
        c.niveles(
            vec![
                Nivel {
                    nombre: "2DO PISO".into(),
                    cota: 6.65,
                    clave: "2gRXFgjRn2HPE$YoDLX3FV".into(),
                },
                Nivel {
                    nombre: "1ER PISO".into(),
                    cota: 3.4,
                    ..Default::default()
                },
            ],
            &[(a, 0), (b, 1)],
        );
        c.poner_clave(a, "0K7w7JxVz4MRkRhRWvQ$a9");
        cubo(&mut c, a, [0.8, 0.8, 0.8, 1.0]);
        let m = c.terminar();
        assert_eq!(m.niveles[0].nombre, "1ER PISO");
        assert_eq!(m.elementos[0].nivel, Some(1));
        assert_eq!(m.elementos[1].nivel, Some(0));
        assert_eq!(m.elementos[2].nivel, None);
        // Las claves estables van con lo suyo al ordenar y por los bytes.
        assert_eq!(m.niveles[1].clave, "2gRXFgjRn2HPE$YoDLX3FV");
        assert_eq!(m.elementos[0].clave, "0K7w7JxVz4MRkRhRWvQ$a9");
        assert_eq!(Modelo3d::de_bytes(&m.a_bytes()).as_ref(), Some(&m));
        // Caso negativo: la cache de la version de antes no se entiende.
        let mut vieja = m.a_bytes();
        vieja[7] = 3;
        assert!(Modelo3d::de_bytes(&vieja).is_none());
    }

    #[test]
    fn ida_y_vuelta_por_bytes() {
        let mut c = Constructor3d::nuevo();
        let e = c.elemento("IfcSlab", "Losa ñ");
        cubo(&mut c, e, [0.8, 0.8, 0.8, 1.0]);
        let m = c.terminar();
        let b = m.a_bytes();
        assert_eq!(Modelo3d::de_bytes(&b).as_ref(), Some(&m));
        // Caso negativo: cortado o con otra marca, nada.
        assert!(Modelo3d::de_bytes(&b[..b.len() - 3]).is_none());
        assert!(Modelo3d::de_bytes(b"PXCAD\0\0\x04").is_none());
    }

    #[test]
    fn los_tipos_se_leen_en_espanol() {
        assert_eq!(tipo_legible("IfcWallStandardCase"), "Muro");
        assert_eq!(tipo_legible("IfcDoor"), "Puerta");
        // Caso negativo: lo desconocido, sin el «Ifc».
        assert_eq!(tipo_legible("IfcAlgoRaro"), "AlgoRaro");
    }
}

#[cfg(test)]
mod pruebas_lineas {
    use super::*;

    #[test]
    fn lineas_y_puntos_van_aparte_y_cuentan_en_la_caja() {
        let mut c = Constructor3d::nuevo();
        let e = c.elemento("Alineamiento", "Eje 1");
        c.linea(
            e,
            &[
                [100.0, 200.0, 10.0],
                [110.0, 200.0, 11.0],
                [120.0, 205.0, 12.0],
            ],
            [1.0, 0.0, 0.0, 1.0],
        );
        let p = c.elemento("Punto", "15");
        c.punto(p, [105.0, 190.0, 9.0], [1.0, 1.0, 0.0, 1.0]);
        let m = c.terminar();
        assert_eq!(m.lineas, vec![0, 1, 1, 2]);
        assert_eq!(m.puntos.len(), 6);
        assert!(!m.vacio());
        assert!((m.caja[1] - (-10.0)).abs() < 1e-4 && (m.caja[5] - 2.0).abs() < 1e-4);
        let b = m.a_bytes();
        assert_eq!(Modelo3d::de_bytes(&b).as_ref(), Some(&m));
        // Caso negativo: una linea de un solo punto no dibuja nada.
        let mut c = Constructor3d::nuevo();
        c.linea(0, &[[0.0; 3]], [1.0; 4]);
        assert!(c.terminar().vacio());
    }
}
