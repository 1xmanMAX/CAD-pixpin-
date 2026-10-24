//! **El hueco entre varias figuras, exacto** (F2).
//!
//! Port de `motor/CaraExacta.kt` del PixPin de Android (v0.76-v0.77).
//!
//! El bote de [`crate::regiones`] derrama por una rejilla, y en los huecos
//! **entre** figuras —dos circulos que se pisan, un rectangulo partido por una
//! raya— eso se nota: una rejilla de celdas arrimada a las paredes se come los
//! picos y se abomba en las curvas. En el movil el usuario lo dijo asi: «hay
//! zonas en las que se traza de forma extrana o se sale de su contenedor; son
//! lineas vectoriales, no deberia pasar». Tiene razon.
//!
//! Aqui no hay celdas. Los tramos de todas las paredes se **cortan entre si de
//! verdad**, con eso se arma el plano —nudos y aristas— y se recorre **la cara
//! que contiene el punto tocado**: su borde son los propios tramos, con sus
//! esquinas donde se cruzan. Las islas que queden dentro salen como agujeros.
//!
//! Lo dibujado a mano no cierra al milimetro, asi que **un cabo suelto** que se
//! queda a menos de `tolerancia` de otra raya se da por unido a ella. Si aun
//! asi la cara no cierra, sale `None` y decide la rejilla, que tapa rendijas
//! mas gordas: por eso esto va **delante** de la rejilla y no en su lugar.
//!
//! Todo en f64 aunque la escena sea f32: los cortes entre tramos casi
//! paralelos pierden los decimales que deciden si dos nudos son el mismo.

use std::collections::{HashMap, HashSet};

use crate::regiones::Region;
use crate::vector::Punto2;

/// La holgura de fabrica con la que un cabo suelto se une a otra raya, en
/// unidades del documento. La del movil.
pub const TOLERANCIA_DE_CARA: f32 = 2.0;

/// Por encima de esto el plano sale caro (los cortes crecen con el cuadrado en
/// el peor caso) y se deja a la rejilla, que cuesta lo mismo con mil tramos
/// que con cien mil.
const MAXIMO_DE_TRAMOS: usize = 12_000;

/// Dos nudos a menos de esto son el mismo nudo.
const JUNTO: f64 = 1e-3;

/// **La cara del plano de `segmentos` que contiene `p`, con sus islas como
/// agujeros; o `None` si no cierra.**
///
/// `None` no es un fallo: quiere decir «no lo se decir exacto» y quien llama
/// prueba con la rejilla. Tambien sale `None` con menos de tres tramos (no
/// encierran nada) y con mas de [`MAXIMO_DE_TRAMOS`].
pub fn cara_exacta(segmentos: &[(Punto2, Punto2)], p: Punto2, tolerancia: f32) -> Option<Region> {
    let n = segmentos.len();
    if !(3..=MAXIMO_DE_TRAMOS).contains(&n) {
        return None;
    }
    let tramos: Vec<([f64; 2], [f64; 2])> = segmentos
        .iter()
        .map(|(a, b)| ([a.x as f64, a.y as f64], [b.x as f64, b.y as f64]))
        .collect();
    if tramos.iter().any(|(a, b)| {
        !(a[0].is_finite() && a[1].is_finite() && b[0].is_finite() && b[1].is_finite())
    }) {
        // Un NaN en una pared rompe el orden por angulo y los cortes; mejor la
        // rejilla, que se lo salta.
        return None;
    }
    Plano::nuevo(&tramos, tolerancia as f64).cara_de(p.x as f64, p.y as f64)
}

/// La llave de una casilla. Las colisiones solo traen candidatos de mas, que
/// se descartan comparando coordenadas: por eso vale un simple producto.
fn clave(ix: i64, iy: i64) -> i64 {
    ix.wrapping_mul(2_000_003).wrapping_add(iy)
}

struct Plano {
    xs: Vec<f64>,
    ys: Vec<f64>,
    casillas: HashMap<i64, Vec<usize>>,
    cortes: Vec<Vec<(f64, usize)>>,
    uniones: Vec<(usize, usize)>,
    vecinos: Vec<Vec<usize>>,
    tolerancia: f64,
}

/// Los extremos de los tramos, a pelo, para no ir y volver por las tuplas.
struct Tramos {
    ax: Vec<f64>,
    ay: Vec<f64>,
    bx: Vec<f64>,
    by: Vec<f64>,
    na: Vec<usize>,
    nb: Vec<usize>,
}

impl Plano {
    fn nuevo(segmentos: &[([f64; 2], [f64; 2])], tolerancia: f64) -> Plano {
        let n = segmentos.len();
        let mut pl = Plano {
            xs: Vec::new(),
            ys: Vec::new(),
            casillas: HashMap::new(),
            cortes: vec![Vec::with_capacity(4); n],
            uniones: Vec::new(),
            vecinos: Vec::new(),
            tolerancia,
        };
        let ax: Vec<f64> = segmentos.iter().map(|s| s.0[0]).collect();
        let ay: Vec<f64> = segmentos.iter().map(|s| s.0[1]).collect();
        let bx: Vec<f64> = segmentos.iter().map(|s| s.1[0]).collect();
        let by: Vec<f64> = segmentos.iter().map(|s| s.1[1]).collect();
        let na: Vec<usize> = (0..n).map(|i| pl.nudo(ax[i], ay[i])).collect();
        let nb: Vec<usize> = (0..n).map(|i| pl.nudo(bx[i], by[i])).collect();
        let t = Tramos {
            ax,
            ay,
            bx,
            by,
            na,
            nb,
        };
        for i in 0..n {
            pl.cortes[i].push((0.0, t.na[i]));
            pl.cortes[i].push((1.0, t.nb[i]));
        }

        // Una criba por casillas, para no cruzar todos con todos.
        let (mut x1, mut y1, mut x2, mut y2) = (f64::MAX, f64::MAX, -f64::MAX, -f64::MAX);
        for i in 0..n {
            x1 = x1.min(t.ax[i].min(t.bx[i]));
            y1 = y1.min(t.ay[i].min(t.by[i]));
            x2 = x2.max(t.ax[i].max(t.bx[i]));
            y2 = y2.max(t.ay[i].max(t.by[i]));
        }
        let lado = ((x2 - x1).max(y2 - y1) / 96.0)
            .max(tolerancia * 2.0)
            .max(1e-6);
        let casillas_de = |i: usize, holgura: f64| {
            let cx1 = ((t.ax[i].min(t.bx[i]) - holgura - x1) / lado).floor() as i64;
            let cx2 = ((t.ax[i].max(t.bx[i]) + holgura - x1) / lado).floor() as i64;
            let cy1 = ((t.ay[i].min(t.by[i]) - holgura - y1) / lado).floor() as i64;
            let cy2 = ((t.ay[i].max(t.by[i]) + holgura - y1) / lado).floor() as i64;
            (cx1..=cx2).flat_map(move |cx| (cy1..=cy2).map(move |cy| clave(cx, cy)))
        };
        let mut criba: HashMap<i64, Vec<usize>> = HashMap::new();
        for i in 0..n {
            for c in casillas_de(i, 0.0) {
                criba.entry(c).or_default().push(i);
            }
        }

        // Cuantos tramos salen de cada nudo: con uno solo, es un cabo suelto.
        let mut grado: HashMap<usize, u32> = HashMap::new();
        for i in 0..n {
            *grado.entry(t.na[i]).or_default() += 1;
            *grado.entry(t.nb[i]).or_default() += 1;
        }
        let suelto = |k: usize| grado.get(&k) == Some(&1);

        let mut visto = vec![usize::MAX; n];
        for i in 0..n {
            let cabo_a = suelto(t.na[i]);
            let cabo_b = suelto(t.nb[i]);
            for c in casillas_de(i, tolerancia) {
                let Some(lista) = criba.get(&c) else { continue };
                for &j in lista {
                    if j == i || visto[j] == i {
                        continue;
                    }
                    visto[j] = i;
                    if j > i {
                        pl.cruzar(i, j, &t);
                    }
                    // Los cabos sueltos de i, contra j.
                    if cabo_a {
                        pl.arrimar(t.na[i], t.ax[i], t.ay[i], j, &t);
                    }
                    if cabo_b {
                        pl.arrimar(t.nb[i], t.bx[i], t.by[i], j, &t);
                    }
                }
            }
        }

        let mut jefe: Vec<usize> = (0..pl.xs.len()).collect();
        for &(a, b) in &pl.uniones {
            let ra = raiz(&mut jefe, a);
            let rb = raiz(&mut jefe, b);
            jefe[ra] = rb;
        }

        // Aristas sin repetir, y los vecinos de cada nudo ordenados por angulo.
        let mut aristas: HashSet<(usize, usize)> = HashSet::new();
        let mut listas: Vec<Vec<usize>> = vec![Vec::new(); pl.xs.len()];
        for i in 0..n {
            let mut orden = std::mem::take(&mut pl.cortes[i]);
            // Estable, como el `sortedBy` del movil: dos cortes en el mismo t
            // conservan el orden en que llegaron.
            orden.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            let mut antes = usize::MAX;
            for (tt, k) in orden {
                if !(-1e-9..=1.0 + 1e-9).contains(&tt) {
                    continue;
                }
                let u = raiz(&mut jefe, k);
                if antes != usize::MAX && antes != u && aristas.insert((antes.min(u), antes.max(u)))
                {
                    listas[antes].push(u);
                    listas[u].push(antes);
                }
                antes = u;
            }
        }
        for (u, l) in listas.iter_mut().enumerate() {
            let (xu, yu) = (pl.xs[u], pl.ys[u]);
            let (xs, ys) = (&pl.xs, &pl.ys);
            l.sort_by(|&a, &b| {
                let fa = (ys[a] - yu).atan2(xs[a] - xu);
                let fb = (ys[b] - yu).atan2(xs[b] - xu);
                fa.partial_cmp(&fb).unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        pl.vecinos = listas;
        pl
    }

    /// Un punto, un numero. Los que caen a menos de [`JUNTO`] son el mismo.
    fn nudo(&mut self, x: f64, y: f64) -> usize {
        let ix = (x / JUNTO / 4.0).floor() as i64;
        let iy = (y / JUNTO / 4.0).floor() as i64;
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(l) = self.casillas.get(&clave(ix + dx, iy + dy)) {
                    for &k in l {
                        if (self.xs[k] - x).abs() <= JUNTO && (self.ys[k] - y).abs() <= JUNTO {
                            return k;
                        }
                    }
                }
            }
        }
        self.xs.push(x);
        self.ys.push(y);
        let k = self.xs.len() - 1;
        self.casillas.entry(clave(ix, iy)).or_default().push(k);
        k
    }

    fn cruzar(&mut self, i: usize, j: usize, t: &Tramos) {
        let (rx, ry) = (t.bx[i] - t.ax[i], t.by[i] - t.ay[i]);
        let (sx, sy) = (t.bx[j] - t.ax[j], t.by[j] - t.ay[j]);
        let den = rx * sy - ry * sx;
        let li = rx.hypot(ry);
        let lj = sx.hypot(sy);
        if li < 1e-12 || lj < 1e-12 {
            return;
        }
        if den.abs() < 1e-12 * li * lj {
            // Paralelos. Si ademas van por la misma recta y se pisan, cada uno
            // corta al otro en los extremos que le caen encima: dos
            // rectangulos con un lado comun.
            if ((t.ax[j] - t.ax[i]) * ry - (t.ay[j] - t.ay[i]) * rx).abs() / li > JUNTO {
                return;
            }
            self.sobre(i, t.ax[j], t.ay[j], t, rx, ry, li);
            self.sobre(i, t.bx[j], t.by[j], t, rx, ry, li);
            self.sobre(j, t.ax[i], t.ay[i], t, sx, sy, lj);
            self.sobre(j, t.bx[i], t.by[i], t, sx, sy, lj);
            return;
        }
        let (qx, qy) = (t.ax[j] - t.ax[i], t.ay[j] - t.ay[i]);
        let ti = (qx * sy - qy * sx) / den;
        let uj = (qx * ry - qy * rx) / den;
        let (ei, ej) = (JUNTO / li, JUNTO / lj);
        if ti < -ei || ti > 1.0 + ei || uj < -ej || uj > 1.0 + ej {
            return;
        }
        let tt = ti.clamp(0.0, 1.0);
        let uu = uj.clamp(0.0, 1.0);
        let k = self.nudo(t.ax[i] + rx * tt, t.ay[i] + ry * tt);
        self.cortes[i].push((tt, k));
        self.cortes[j].push((uu, k));
    }

    #[allow(clippy::too_many_arguments)]
    fn sobre(&mut self, i: usize, x: f64, y: f64, t: &Tramos, rx: f64, ry: f64, l: f64) {
        let tt = ((x - t.ax[i]) * rx + (y - t.ay[i]) * ry) / (l * l);
        if !(0.0..=1.0).contains(&tt) {
            return;
        }
        let k = self.nudo(x, y);
        self.cortes[i].push((tt, k));
    }

    /// El cabo suelto `cabo`, en (x, y), se une a la raya `j` si le queda a
    /// menos de la tolerancia.
    fn arrimar(&mut self, cabo: usize, x: f64, y: f64, j: usize, t: &Tramos) {
        if t.na[j] == cabo || t.nb[j] == cabo {
            return;
        }
        let (sx, sy) = (t.bx[j] - t.ax[j], t.by[j] - t.ay[j]);
        let l2 = sx * sx + sy * sy;
        if l2 < 1e-18 {
            return;
        }
        let tt = (((x - t.ax[j]) * sx + (y - t.ay[j]) * sy) / l2).clamp(0.0, 1.0);
        let (px, py) = (t.ax[j] + sx * tt, t.ay[j] + sy * tt);
        if (px - x).hypot(py - y) > self.tolerancia {
            return;
        }
        // Cerca de una punta de j, a la punta: dos rayas que casi hacen
        // esquina, la hacen. Si no, se abriria un nudo nuevo a un pelo de la
        // esquina y la cara tendria un diente.
        let l = l2.sqrt();
        let destino = if tt * l <= self.tolerancia {
            t.na[j]
        } else if (1.0 - tt) * l <= self.tolerancia {
            t.nb[j]
        } else {
            let k = self.nudo(px, py);
            self.cortes[j].push((tt, k));
            k
        };
        self.uniones.push((cabo, destino));
    }

    fn area(&self, c: &[usize]) -> f64 {
        let mut s = 0.0;
        for i in 0..c.len() {
            let (a, b) = (c[i], c[(i + 1) % c.len()]);
            s += self.xs[a] * self.ys[b] - self.xs[b] * self.ys[a];
        }
        s / 2.0
    }

    fn dentro(&self, x: f64, y: f64, c: &[usize]) -> bool {
        let mut si = false;
        let mut j = c.len() - 1;
        for i in 0..c.len() {
            let (yi, yj) = (self.ys[c[i]], self.ys[c[j]]);
            if (yi > y) != (yj > y)
                && x < (self.xs[c[j]] - self.xs[c[i]]) * (y - yi) / (yj - yi) + self.xs[c[i]]
            {
                si = !si;
            }
            j = i;
        }
        si
    }

    fn cara_de(&self, px: f64, py: f64) -> Option<Region> {
        // Todas las caras: cada media arista se recorre una vez, girando
        // siempre hacia el mismo lado de la que se viene, y la cara queda a
        // un lado fijo. Las de area positiva son caras; las negativas, el
        // borde de fuera de cada isla.
        let nv = self.vecinos.len();
        let mut desde = vec![0usize; nv + 1];
        for u in 0..nv {
            desde[u + 1] = desde[u] + self.vecinos[u].len();
        }
        let mut hecha = vec![false; desde[nv]];
        let mut llenas: Vec<(Vec<usize>, f64)> = Vec::new();
        let mut islas: Vec<(Vec<usize>, f64)> = Vec::new();
        let mut ciclo = Vec::new();
        for u0 in 0..nv {
            for k0 in 0..self.vecinos[u0].len() {
                if hecha[desde[u0] + k0] {
                    continue;
                }
                ciclo.clear();
                let (mut u, mut k) = (u0, k0);
                while !hecha[desde[u] + k] {
                    hecha[desde[u] + k] = true;
                    ciclo.push(u);
                    let v = self.vecinos[u][k];
                    let vs = &self.vecinos[v];
                    // Siempre esta: las aristas se meten en los dos sentidos.
                    let vuelta = vs.iter().position(|&w| w == u).unwrap_or(0);
                    k = (vuelta + vs.len() - 1) % vs.len();
                    u = v;
                }
                let c = sin_rabos(&ciclo);
                if c.len() < 3 {
                    continue;
                }
                let a = self.area(&c);
                if a > 1e-9 {
                    llenas.push((c, a));
                } else if a < -1e-9 {
                    islas.push((c, a));
                }
            }
        }
        // La mas pequena de las que contienen el punto: las demas la envuelven.
        let (fuera, _) = llenas
            .iter()
            .filter(|(c, _)| self.dentro(px, py, c))
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))?;
        let del_borde: HashSet<usize> = fuera.iter().copied().collect();
        let candidatas: Vec<&(Vec<usize>, f64)> = islas
            .iter()
            .filter(|(isla, _)| {
                !isla.iter().any(|k| del_borde.contains(k))
                    && self.dentro(self.xs[isla[0]], self.ys[isla[0]], fuera)
            })
            .collect();
        // Una isla dentro de otra isla no es agujero de esta cara: esta en una
        // de mas adentro.
        let huecos: Vec<Vec<Punto2>> = candidatas
            .iter()
            .enumerate()
            .filter(|(ih, (h, ah))| {
                !candidatas.iter().enumerate().any(|(ik, (k, ak))| {
                    ik != *ih && ak.abs() > ah.abs() && self.dentro(self.xs[h[0]], self.ys[h[0]], k)
                })
            })
            .map(|(_, (h, _))| self.a_puntos(h))
            .collect();
        Some(Region {
            contorno: self.a_puntos(fuera),
            huecos,
        })
    }

    fn a_puntos(&self, c: &[usize]) -> Vec<Punto2> {
        c.iter()
            .map(|&k| Punto2::nuevo(self.xs[k] as f32, self.ys[k] as f32))
            .collect()
    }
}

fn raiz(jefe: &mut [usize], a: usize) -> usize {
    let mut r = a;
    while jefe[r] != r {
        jefe[r] = jefe[jefe[r]];
        r = jefe[r];
    }
    r
}

/// Quita las idas y vueltas por una raya que se mete en la cara y no la
/// parte: a, b, a -> a. Sin esto la raya suelta dejaria una costura de ancho
/// cero en el relleno, y el area y el «esta dentro» saldrian torcidos.
fn sin_rabos(ciclo: &[usize]) -> Vec<usize> {
    let mut pila: Vec<usize> = Vec::with_capacity(ciclo.len());
    for &u in ciclo {
        if pila.len() >= 2 && pila[pila.len() - 2] == u {
            pila.pop();
        } else {
            pila.push(u);
        }
    }
    // Y por la costura, donde el ciclo se cierra.
    let mut cambia = true;
    while cambia && pila.len() >= 3 {
        cambia = false;
        if pila[pila.len() - 1] == pila[1] {
            pila.remove(0);
            pila.pop();
            cambia = true;
            continue;
        }
        if pila[pila.len() - 2] == pila[0] {
            pila.pop();
            pila.pop();
            cambia = true;
        }
    }
    pila
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::regiones::area_de;

    fn p(x: f32, y: f32) -> Punto2 {
        Punto2::nuevo(x, y)
    }

    fn cerrado(v: &[Punto2]) -> Vec<(Punto2, Punto2)> {
        (0..v.len()).map(|i| (v[i], v[(i + 1) % v.len()])).collect()
    }

    fn caja(x1: f32, y1: f32, x2: f32, y2: f32) -> Vec<(Punto2, Punto2)> {
        cerrado(&[p(x1, y1), p(x2, y1), p(x2, y2), p(x1, y2)])
    }

    fn circulo(cx: f32, cy: f32, r: f32) -> Vec<(Punto2, Punto2)> {
        let n = 72;
        let v: Vec<Punto2> = (0..n)
            .map(|i| {
                let a = i as f64 * 2.0 * std::f64::consts::PI / n as f64;
                p(cx + r * a.cos() as f32, cy + r * a.sin() as f32)
            })
            .collect();
        cerrado(&v)
    }

    fn area(a: &[Punto2]) -> f64 {
        area_de(a).abs() as f64
    }

    fn cara(t: &[(Punto2, Punto2)], x: f32, y: f32) -> Option<Region> {
        cara_exacta(t, p(x, y), TOLERANCIA_DE_CARA)
    }

    #[test]
    fn una_caja_partida_por_una_raya_da_justo_sus_dos_trozos() {
        let mut t = caja(0.0, 0.0, 100.0, 100.0);
        t.push((p(40.0, -20.0), p(40.0, 120.0)));
        let izq = cara(&t, 10.0, 50.0).unwrap();
        let der = cara(&t, 90.0, 50.0).unwrap();
        assert!((area(&izq.contorno) - 4000.0).abs() < 1e-2);
        assert!((area(&der.contorno) - 6000.0).abs() < 1e-2);
        assert!(izq.huecos.is_empty());
    }

    #[test]
    fn la_esquina_del_cruce_es_un_vertice_exacto() {
        let mut t = caja(0.0, 0.0, 100.0, 100.0);
        t.extend(caja(50.0, 50.0, 150.0, 150.0));
        let medio = cara(&t, 75.0, 75.0).unwrap();
        assert!((area(&medio.contorno) - 2500.0).abs() < 1e-2);
        assert!(
            medio
                .contorno
                .iter()
                .any(|q| (q.x - 100.0).abs() < 1e-3 && (q.y - 50.0).abs() < 1e-3)
        );
        assert!((area(&cara(&t, 10.0, 10.0).unwrap().contorno) - 7500.0).abs() < 1e-2);
    }

    #[test]
    fn una_isla_dentro_sale_como_agujero_y_la_de_mas_adentro_no() {
        let mut t = caja(0.0, 0.0, 200.0, 200.0);
        t.extend(caja(50.0, 50.0, 150.0, 150.0));
        t.extend(caja(80.0, 80.0, 120.0, 120.0));
        let r = cara(&t, 10.0, 10.0).unwrap();
        assert!((area(&r.contorno) - 40000.0).abs() < 1e-1);
        assert_eq!(r.huecos.len(), 1);
        assert!((area(&r.huecos[0]) - 10000.0).abs() < 1e-1);
        let anillo = cara(&t, 60.0, 60.0).unwrap();
        assert!((area(&anillo.contorno) - 10000.0).abs() < 1e-1);
        assert_eq!(anillo.huecos.len(), 1);
        assert!((area(&anillo.huecos[0]) - 1600.0).abs() < 1e-1);
    }

    #[test]
    fn dos_circulos_que_se_pisan_dan_la_lente_sin_salirse() {
        let mut t = circulo(0.0, 0.0, 100.0);
        t.extend(circulo(100.0, 0.0, 100.0));
        let lente = cara(&t, 50.0, 0.0).unwrap();
        for q in &lente.contorno {
            assert!(q.x.hypot(q.y) <= 100.0 + 1e-3);
            assert!((q.x - 100.0).hypot(q.y) <= 100.0 + 1e-3);
        }
        // La lente de dos circulos de radio r a distancia r: r²(2π/3 − √3/2).
        let esperada = 10000.0 * (2.0 * std::f64::consts::PI / 3.0 - 3f64.sqrt() / 2.0);
        assert!((area(&lente.contorno) - esperada).abs() < 60.0);
    }

    #[test]
    fn una_raya_que_entra_y_no_parte_no_estorba() {
        let mut t = caja(0.0, 0.0, 100.0, 100.0);
        t.push((p(50.0, -10.0), p(50.0, 60.0)));
        assert!((area(&cara(&t, 10.0, 10.0).unwrap().contorno) - 10000.0).abs() < 1e-2);
    }

    #[test]
    fn cuatro_rayas_que_casi_hacen_esquina_cierran() {
        let t = vec![
            (p(0.0, 0.0), p(100.0, 0.5)),
            (p(100.8, 1.0), p(100.0, 100.0)),
            (p(99.5, 100.9), p(0.0, 100.0)),
            (p(-0.7, 99.0), p(0.5, 0.9)),
        ];
        let r = cara(&t, 50.0, 50.0).expect("las juntas de menos de dos unidades se unen");
        assert!((area(&r.contorno) - 10000.0).abs() < 250.0);
    }

    #[test]
    fn rayas_que_se_quedan_lejos_de_la_esquina_no_cierran() {
        // El negativo del anterior: una rendija de diez unidades no es un
        // temblor de la mano, y aqui no se tapa (lo decidira la rejilla).
        let t = vec![
            (p(0.0, 0.0), p(100.0, 0.0)),
            (p(110.0, 0.0), p(110.0, 100.0)),
            (p(100.0, 100.0), p(0.0, 100.0)),
            (p(0.0, 100.0), p(0.0, 0.0)),
        ];
        assert!(cara(&t, 50.0, 50.0).is_none());
    }

    #[test]
    fn lo_abierto_no_es_cara() {
        let t = vec![
            (p(0.0, 0.0), p(100.0, 0.0)),
            (p(100.0, 0.0), p(100.0, 100.0)),
            (p(100.0, 100.0), p(0.0, 100.0)),
        ];
        assert!(cara(&t, 50.0, 50.0).is_none());
        assert!(cara(&caja(0.0, 0.0, 10.0, 10.0), 50.0, 50.0).is_none());
    }

    #[test]
    fn dos_cajas_con_un_lado_comun() {
        let mut t = caja(0.0, 0.0, 100.0, 100.0);
        t.extend(caja(100.0, 20.0, 160.0, 80.0));
        assert!((area(&cara(&t, 130.0, 50.0).unwrap().contorno) - 3600.0).abs() < 1e-2);
        assert!((area(&cara(&t, 50.0, 50.0).unwrap().contorno) - 10000.0).abs() < 1e-2);
    }

    #[test]
    fn con_menos_de_tres_tramos_o_con_un_nan_no_se_intenta() {
        let dos = vec![(p(0.0, 0.0), p(10.0, 0.0)), (p(10.0, 0.0), p(0.0, 10.0))];
        assert!(cara(&dos, 2.0, 2.0).is_none());
        let mut t = caja(0.0, 0.0, 100.0, 100.0);
        t.push((p(f32::NAN, 0.0), p(50.0, 50.0)));
        assert!(cara(&t, 10.0, 10.0).is_none());
    }
}
