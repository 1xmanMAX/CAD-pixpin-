//! **La regla se engancha a las rayas, no solo a sus puntas** (como PixPin
//! Android v0.113.0, que lo pidio el usuario el 9-oct-2026: «que la regla se
//! enganche a una parte de la linea, y que reconozca los angulos rectos»).
//!
//! Los tramos rectos del plano (las tiras partidas en segmentos) repartidos
//! en una rejilla de 256 x 256 —cada uno en las celdas que pisa su caja; los
//! que pisan demasiadas, aparte— y los circulos y arcos. Con eso se sabe el
//! punto de raya mas cercano y el pie de la perpendicular desde un punto.
//! El orden, como AutoCAD: un punto (vertice, extremo, centro), la
//! perpendicular, el punto mas cercano de una raya o un arco; si nada, libre.

use crate::modelo::{CORTE, Modelo};
use crate::ventana::Enganches;

const LADO: usize = 256;
const TOPE_DE_CELDAS: usize = 64;

/// De que clase es un enganche (cambia la marca que se ve).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    Punto,
    Perpendicular,
    EnLaRaya,
    Libre,
}

pub struct Rayas {
    x0: f64,
    y0: f64,
    celda: f64,
    inicio: Vec<u32>,
    en_celda: Vec<u32>,
    grandes: Vec<u32>,
    segmentos: Vec<[f32; 4]>,
    /// Centro, radio, inicio y barrido.
    arcos: Vec<[f32; 5]>,
}

/// El punto del segmento a–b mas cercano a p.
pub fn al_segmento(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    if l2 < 1e-24 {
        return a;
    }
    let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0);
    [a[0] + t * dx, a[1] + t * dy]
}

/// El pie de la perpendicular desde p a la recta a–b, si cae en el segmento.
pub fn pie_de_perpendicular(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> Option<[f64; 2]> {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    if l2 < 1e-24 {
        return None;
    }
    let t = ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2;
    (-1e-9..=1.0 + 1e-9)
        .contains(&t)
        .then(|| [a[0] + t * dx, a[1] + t * dy])
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

impl Rayas {
    pub fn de(m: &Modelo) -> Rayas {
        let mut segmentos = Vec::new();
        let mut antes = CORTE;
        for &v in &m.lineas {
            if v != CORTE && antes != CORTE {
                let (a, b) = (&m.vertices[antes as usize], &m.vertices[v as usize]);
                segmentos.push([a.x, a.y, b.x, b.y]);
            }
            antes = v;
        }
        let arcos = m
            .arcos
            .iter()
            .map(|a| [a.centro[0], a.centro[1], a.radio, a.inicio, a.barrido])
            .collect();
        Rayas::de_segmentos(m.caja, segmentos, arcos)
    }

    pub fn de_segmentos(caja: [f32; 4], segmentos: Vec<[f32; 4]>, arcos: Vec<[f32; 5]>) -> Rayas {
        let lado = ((caja[2] - caja[0]).max(caja[3] - caja[1]) as f64).max(1e-6);
        let celda = lado / LADO as f64 * 1.0001;
        let (x0, y0) = (caja[0] as f64, caja[1] as f64);
        let c = |v: f32, o: f64| {
            (((v as f64 - o) / celda).floor() as i64).clamp(0, LADO as i64 - 1) as usize
        };
        let celdas_de = |s: &[f32; 4]| {
            let (i0, i1) = (c(s[0].min(s[2]), x0), c(s[0].max(s[2]), x0));
            let (j0, j1) = (c(s[1].min(s[3]), y0), c(s[1].max(s[3]), y0));
            ((i1 - i0 + 1) * (j1 - j0 + 1) <= TOPE_DE_CELDAS).then_some((i0, i1, j0, j1))
        };
        let mut inicio = vec![0u32; LADO * LADO + 1];
        let mut grandes = Vec::new();
        for (k, s) in segmentos.iter().enumerate() {
            match celdas_de(s) {
                Some((i0, i1, j0, j1)) => {
                    for j in j0..=j1 {
                        for i in i0..=i1 {
                            inicio[j * LADO + i + 1] += 1;
                        }
                    }
                }
                None => grandes.push(k as u32),
            }
        }
        for k in 1..=LADO * LADO {
            inicio[k] += inicio[k - 1];
        }
        let mut puesto = inicio.clone();
        let mut en_celda = vec![0u32; inicio[LADO * LADO] as usize];
        for (k, s) in segmentos.iter().enumerate() {
            if let Some((i0, i1, j0, j1)) = celdas_de(s) {
                for j in j0..=j1 {
                    for i in i0..=i1 {
                        let cc = j * LADO + i;
                        en_celda[puesto[cc] as usize] = k as u32;
                        puesto[cc] += 1;
                    }
                }
            }
        }
        Rayas {
            x0,
            y0,
            celda,
            inicio,
            en_celda,
            grandes,
            segmentos,
            arcos,
        }
    }

    fn candidatos(&self, p: [f64; 2], radio: f64, mut haz: impl FnMut(usize)) {
        let n = ((radio / self.celda).ceil() as i64).min(8);
        let cx = ((p[0] - self.x0) / self.celda).floor() as i64;
        let cy = ((p[1] - self.y0) / self.celda).floor() as i64;
        for j in (cy - n).max(0)..=(cy + n).min(LADO as i64 - 1) {
            for i in (cx - n).max(0)..=(cx + n).min(LADO as i64 - 1) {
                let c = j as usize * LADO + i as usize;
                for k in self.inicio[c]..self.inicio[c + 1] {
                    haz(self.en_celda[k as usize] as usize);
                }
            }
        }
        for &s in &self.grandes {
            haz(s as usize);
        }
    }

    fn extremos(&self, s: usize) -> ([f64; 2], [f64; 2]) {
        let g = self.segmentos[s];
        ([g[0] as f64, g[1] as f64], [g[2] as f64, g[3] as f64])
    }

    /// El segmento mas cercano a `p` a menos de `radio`, y su punto mas cercano.
    pub fn cercano(&self, p: [f64; 2], radio: f64) -> Option<(usize, [f64; 2])> {
        let mut mejor = None;
        let mut dm = radio;
        self.candidatos(p, radio, |s| {
            let (a, b) = self.extremos(s);
            let q = al_segmento(p, a, b);
            let d = dist(p, q);
            if d <= dm {
                dm = d;
                mejor = Some((s, q));
            }
        });
        mejor
    }

    /// El punto de circulo o arco mas cercano a `p` a menos de `radio`.
    pub fn en_arco(&self, p: [f64; 2], radio: f64) -> Option<[f64; 2]> {
        let mut mejor = None;
        let mut dm = radio;
        for a in &self.arcos {
            let (c, r, a0, b) = (
                [a[0] as f64, a[1] as f64],
                a[2] as f64,
                a[3] as f64,
                a[4] as f64,
            );
            let d0 = dist(p, c);
            if (d0 - r).abs() > dm || d0 < 1e-12 {
                continue;
            }
            let ang = (p[1] - c[1]).atan2(p[0] - c[0]);
            // Menos que una vuelta entera (con su margen de redondeo).
            if b < std::f64::consts::TAU - 1e-4 {
                let mut rel = ang - a0;
                rel -= (rel / std::f64::consts::TAU).floor() * std::f64::consts::TAU;
                if rel > b {
                    continue;
                }
            }
            let q = [c[0] + r * ang.cos(), c[1] + r * ang.sin()];
            let d = dist(p, q);
            if d <= dm {
                dm = d;
                mejor = Some(q);
            }
        }
        mejor
    }

    /// El enganche para la mira en `p` con el punto anterior `desde`.
    pub fn ajustar(
        &self,
        p: [f64; 2],
        radio: f64,
        desde: Option<[f64; 2]>,
        puntos: Option<&Enganches>,
    ) -> ([f64; 2], Tipo) {
        if let Some(q) = puntos.and_then(|g| g.cerca(p, radio)) {
            return (q, Tipo::Punto);
        }
        let cerca = self.cercano(p, radio);
        if let (Some((s, _)), Some(d)) = (cerca, desde) {
            let (a, b) = self.extremos(s);
            if let Some(pie) = pie_de_perpendicular(d, a, b)
                && dist(pie, p) <= radio * 1.5
                && dist(pie, d) > 1e-9
            {
                return (pie, Tipo::Perpendicular);
            }
        }
        let arco = self.en_arco(p, radio);
        match (cerca, arco) {
            (Some((_, q)), Some(r)) if dist(q, p) > dist(r, p) => (r, Tipo::EnLaRaya),
            (Some((_, q)), _) => (q, Tipo::EnLaRaya),
            (None, Some(r)) => (r, Tipo::EnLaRaya),
            (None, None) => (p, Tipo::Libre),
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_regla_se_engancha_a_la_raya_y_a_la_perpendicular() {
        // Una raya horizontal de (0,0) a (10,0) y una vertical larga de (20,-50) a (20,50).
        let r = Rayas::de_segmentos(
            [-1.0, -50.0, 21.0, 50.0],
            vec![[0.0, 0.0, 10.0, 0.0], [20.0, -50.0, 20.0, 50.0]],
            vec![],
        );
        // En mitad de la raya, sin punto anterior: se pega a la raya.
        let (q, t) = r.ajustar([4.0, 0.3], 0.5, None, None);
        assert_eq!(t, Tipo::EnLaRaya);
        assert!((q[0] - 4.0).abs() < 1e-9 && q[1].abs() < 1e-9);
        // Desde (4,0), cerca de la vertical: el pie de la perpendicular (20, 0).
        let (q, t) = r.ajustar([19.7, 0.4], 0.6, Some([4.0, 0.0]), None);
        assert_eq!(t, Tipo::Perpendicular);
        assert!((q[0] - 20.0).abs() < 1e-9 && q[1].abs() < 1e-9);
        // Lejos de todo: libre.
        assert_eq!(r.ajustar([5.0, 30.0], 0.5, None, None).1, Tipo::Libre);
        // El pie fuera del segmento no vale.
        assert!(pie_de_perpendicular([15.0, 5.0], [0.0, 0.0], [10.0, 0.0]).is_none());
    }

    #[test]
    fn tambien_se_engancha_a_un_arco() {
        // Medio circulo de radio 5 alrededor de (0,0), de 0 a π.
        let r = Rayas::de_segmentos(
            [-6.0, -6.0, 6.0, 6.0],
            vec![],
            vec![[0.0, 0.0, 5.0, 0.0, std::f32::consts::PI]],
        );
        let (q, t) = r.ajustar([0.2, 5.3], 0.5, None, None);
        assert_eq!(t, Tipo::EnLaRaya);
        assert!((q[0].hypot(q[1]) - 5.0).abs() < 1e-5);
        // Caso negativo: la otra mitad (sin arco) queda libre.
        assert_eq!(r.ajustar([0.0, -5.2], 0.5, None, None).1, Tipo::Libre);
    }
}
