//! **Geometria plana**: puntos, rayas, arcos y circulos, y donde se cortan.
//! Todo en coordenadas del dibujo (la y sube) y en `f64`.

use std::f64::consts::{PI, TAU};

/// Un punto (o un vector) del plano.
pub type P = [f64; 2];

/// Lo que se considera cero al comparar longitudes.
pub const EPS: f64 = 1e-9;

pub fn sumar(a: P, b: P) -> P {
    [a[0] + b[0], a[1] + b[1]]
}

pub fn restar(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1]]
}

pub fn por(a: P, k: f64) -> P {
    [a[0] * k, a[1] * k]
}

pub fn punto(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

pub fn cruz(a: P, b: P) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

pub fn norma(a: P) -> f64 {
    a[0].hypot(a[1])
}

pub fn dist(a: P, b: P) -> f64 {
    norma(restar(b, a))
}

pub fn medio(a: P, b: P) -> P {
    [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]
}

/// El vector de largo 1 (o cero si no tiene largo).
pub fn unitario(a: P) -> P {
    let n = norma(a);
    if n < EPS {
        [0.0, 0.0]
    } else {
        [a[0] / n, a[1] / n]
    }
}

/// Girado 90 grados a la izquierda.
pub fn perp(a: P) -> P {
    [-a[1], a[0]]
}

/// El angulo de `a` a `b`, en radianes (de -PI a PI).
pub fn angulo(a: P, b: P) -> f64 {
    (b[1] - a[1]).atan2(b[0] - a[0])
}

/// El punto a `r` de `c` con angulo `ang`.
pub fn polar(c: P, r: f64, ang: f64) -> P {
    [c[0] + r * ang.cos(), c[1] + r * ang.sin()]
}

/// `p` girado `ang` alrededor de `c`.
pub fn rotar(p: P, c: P, ang: f64) -> P {
    let (s, k) = ang.sin_cos();
    let d = restar(p, c);
    [c[0] + d[0] * k - d[1] * s, c[1] + d[0] * s + d[1] * k]
}

/// El angulo llevado a [0, 2PI).
pub fn normalizar(a: f64) -> f64 {
    let r = a.rem_euclid(TAU);
    if r >= TAU { 0.0 } else { r }
}

/// Lo que hay que girar desde `inicio` (en sentido antihorario) para llegar a `a`.
pub fn desde(inicio: f64, a: f64) -> f64 {
    normalizar(a - inicio)
}

/// Una caja `[x0, y0, x1, y1]`.
pub type Caja = [f64; 4];

pub const CAJA_VACIA: Caja = [
    f64::INFINITY,
    f64::INFINITY,
    f64::NEG_INFINITY,
    f64::NEG_INFINITY,
];

pub fn caja_de(puntos: &[P]) -> Caja {
    let mut c = CAJA_VACIA;
    for p in puntos {
        c = caja_con(c, *p);
    }
    c
}

pub fn caja_con(c: Caja, p: P) -> Caja {
    [
        c[0].min(p[0]),
        c[1].min(p[1]),
        c[2].max(p[0]),
        c[3].max(p[1]),
    ]
}

pub fn unir_cajas(a: Caja, b: Caja) -> Caja {
    [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[2].max(b[2]),
        a[3].max(b[3]),
    ]
}

pub fn caja_valida(c: &Caja) -> bool {
    c[0] <= c[2] && c[1] <= c[3] && c.iter().all(|v| v.is_finite())
}

pub fn ensanchar(c: Caja, m: f64) -> Caja {
    [c[0] - m, c[1] - m, c[2] + m, c[3] + m]
}

pub fn se_tocan(a: &Caja, b: &Caja) -> bool {
    a[0] <= b[2] && a[2] >= b[0] && a[1] <= b[3] && a[3] >= b[1]
}

/// Si `dentro` cabe entera en `fuera`.
pub fn contiene(fuera: &Caja, dentro: &Caja) -> bool {
    dentro[0] >= fuera[0] && dentro[1] >= fuera[1] && dentro[2] <= fuera[2] && dentro[3] <= fuera[3]
}

pub fn en_caja(c: &Caja, p: P) -> bool {
    p[0] >= c[0] && p[0] <= c[2] && p[1] >= c[1] && p[1] <= c[3]
}

/// **Una pieza simple** de lo dibujado: con esto se engancha, se elige, se
/// recorta y se alarga.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Forma {
    Segmento {
        a: P,
        b: P,
    },
    /// En sentido antihorario desde `inicio`, `barrido` radianes (0 a 2PI).
    Arco {
        c: P,
        r: f64,
        inicio: f64,
        barrido: f64,
    },
    Circulo {
        c: P,
        r: f64,
    },
}

impl Forma {
    /// El arco de `a` a `b` que pasa por `m`, o `None` si los tres estan
    /// alineados.
    pub fn arco_por_tres(a: P, m: P, b: P) -> Option<Forma> {
        let c = centro_por_tres(a, m, b)?;
        let r = dist(c, a);
        let (ia, im, ib) = (angulo(c, a), angulo(c, m), angulo(c, b));
        // Antihorario de a a b pasando por m, o al reves.
        let (inicio, barrido) = if desde(ia, im) <= desde(ia, ib) {
            (ia, desde(ia, ib))
        } else {
            (ib, desde(ib, ia))
        };
        Some(Forma::Arco {
            c,
            r,
            inicio: normalizar(inicio),
            barrido,
        })
    }

    /// El arco de un tramo de polilinea con abombado `bulge` (tan(theta/4)).
    pub fn de_abombado(a: P, b: P, bulge: f64) -> Forma {
        if bulge.abs() < 1e-12 {
            return Forma::Segmento { a, b };
        }
        let theta = 4.0 * bulge.atan();
        let cuerda = dist(a, b);
        let r = cuerda / (2.0 * (theta / 2.0).sin()).abs();
        let m = medio(a, b);
        // Del medio de la cuerda al centro: a la izquierda del avance si va
        // antihorario y abomba menos de media vuelta (cos > 0).
        let h = r * (theta / 2.0).cos();
        let dir = unitario(restar(b, a));
        let c = sumar(m, por(perp(dir), h * bulge.signum()));
        // Con bulge > 0 va antihorario de a a b; si no, de b a a.
        let (desde_p, hasta_p) = if bulge > 0.0 { (a, b) } else { (b, a) };
        let inicio = normalizar(angulo(c, desde_p));
        let barrido = desde(inicio, angulo(c, hasta_p));
        Forma::Arco {
            c,
            r,
            inicio,
            barrido: if barrido < EPS { TAU } else { barrido },
        }
    }

    pub fn caja(&self) -> Caja {
        match *self {
            Forma::Segmento { a, b } => caja_de(&[a, b]),
            Forma::Circulo { c, r } => [c[0] - r, c[1] - r, c[0] + r, c[1] + r],
            Forma::Arco {
                c,
                r,
                inicio,
                barrido,
            } => {
                let mut k = caja_de(&[polar(c, r, inicio), polar(c, r, inicio + barrido)]);
                for q in 0..4 {
                    let a = q as f64 * PI / 2.0;
                    if desde(inicio, a) <= barrido {
                        k = caja_con(k, polar(c, r, a));
                    }
                }
                k
            }
        }
    }

    /// El punto de la forma mas cerca de `p`.
    pub fn cercano(&self, p: P) -> P {
        match *self {
            Forma::Segmento { a, b } => {
                let t = parametro_en_segmento(a, b, p).clamp(0.0, 1.0);
                sumar(a, por(restar(b, a), t))
            }
            Forma::Circulo { c, r } => {
                let d = restar(p, c);
                if norma(d) < EPS {
                    polar(c, r, 0.0)
                } else {
                    sumar(c, por(unitario(d), r))
                }
            }
            Forma::Arco {
                c,
                r,
                inicio,
                barrido,
            } => {
                let a = angulo(c, p);
                if desde(inicio, a) <= barrido {
                    polar(c, r, a)
                } else {
                    let (p0, p1) = (polar(c, r, inicio), polar(c, r, inicio + barrido));
                    if dist(p, p0) <= dist(p, p1) { p0 } else { p1 }
                }
            }
        }
    }

    pub fn distancia(&self, p: P) -> f64 {
        dist(p, self.cercano(p))
    }

    /// Los extremos (un circulo no tiene).
    pub fn extremos(&self) -> Vec<P> {
        match *self {
            Forma::Segmento { a, b } => vec![a, b],
            Forma::Arco {
                c,
                r,
                inicio,
                barrido,
            } => {
                vec![polar(c, r, inicio), polar(c, r, inicio + barrido)]
            }
            Forma::Circulo { .. } => Vec::new(),
        }
    }

    pub fn punto_medio(&self) -> Option<P> {
        match *self {
            Forma::Segmento { a, b } => Some(medio(a, b)),
            Forma::Arco {
                c,
                r,
                inicio,
                barrido,
            } => Some(polar(c, r, inicio + barrido / 2.0)),
            Forma::Circulo { .. } => None,
        }
    }

    pub fn centro(&self) -> Option<P> {
        match *self {
            Forma::Segmento { .. } => None,
            Forma::Arco { c, .. } | Forma::Circulo { c, .. } => Some(c),
        }
    }

    /// Los cuadrantes (0, 90, 180 y 270 grados) que caen en la forma.
    pub fn cuadrantes(&self) -> Vec<P> {
        match *self {
            Forma::Segmento { .. } => Vec::new(),
            Forma::Circulo { c, r } => (0..4).map(|q| polar(c, r, q as f64 * PI / 2.0)).collect(),
            Forma::Arco {
                c,
                r,
                inicio,
                barrido,
            } => (0..4)
                .map(|q| q as f64 * PI / 2.0)
                .filter(|a| desde(inicio, *a) <= barrido)
                .map(|a| polar(c, r, a))
                .collect(),
        }
    }

    pub fn longitud(&self) -> f64 {
        match *self {
            Forma::Segmento { a, b } => dist(a, b),
            Forma::Arco { r, barrido, .. } => r * barrido,
            Forma::Circulo { r, .. } => TAU * r,
        }
    }

    /// El pie de la perpendicular desde `p` (sobre la forma alargada: la
    /// recta entera o el circulo entero).
    pub fn pie_perpendicular(&self, p: P) -> Option<P> {
        match *self {
            Forma::Segmento { a, b } => {
                if dist(a, b) < EPS {
                    return None;
                }
                let t = parametro_en_segmento(a, b, p);
                Some(sumar(a, por(restar(b, a), t)))
            }
            Forma::Arco { c, r, .. } | Forma::Circulo { c, r } => {
                let d = restar(p, c);
                (norma(d) > EPS).then(|| sumar(c, por(unitario(d), r)))
            }
        }
    }

    /// Si el punto `p` (que esta sobre la forma alargada) cae en la forma.
    pub fn contiene_punto(&self, p: P, tol: f64) -> bool {
        match *self {
            Forma::Segmento { a, b } => {
                let t = parametro_en_segmento(a, b, p);
                let l = dist(a, b).max(EPS);
                t >= -tol / l && t <= 1.0 + tol / l
            }
            Forma::Circulo { .. } => true,
            Forma::Arco {
                c,
                r,
                inicio,
                barrido,
            } => {
                let margen = tol / r.max(EPS);
                let d = desde(inicio, angulo(c, p));
                d <= barrido + margen || d >= TAU - margen
            }
        }
    }

    /// Los extremos sin pedir memoria (un circulo no tiene).
    pub fn extremos_fijos(&self) -> Option<[P; 2]> {
        match *self {
            Forma::Segmento { a, b } => Some([a, b]),
            Forma::Arco {
                c,
                r,
                inicio,
                barrido,
            } => Some([polar(c, r, inicio), polar(c, r, inicio + barrido)]),
            Forma::Circulo { .. } => None,
        }
    }

    /// **Si la forma toca la caja** `k` (algun punto de la forma dentro o en
    /// el borde), de forma exacta: sin muestrear.
    pub fn toca_caja(&self, k: &Caja) -> bool {
        match *self {
            Forma::Segmento { a, b } => segmento_toca_caja(a, b, k),
            Forma::Circulo { c, r } => {
                // La curva toca la caja llena si la caja tiene puntos a menos
                // de r del centro y puntos a mas de r.
                let dx = (k[0] - c[0]).max(0.0).max(c[0] - k[2]);
                let dy = (k[1] - c[1]).max(0.0).max(c[1] - k[3]);
                let cerca = dx.hypot(dy);
                let fx = (c[0] - k[0]).abs().max((c[0] - k[2]).abs());
                let fy = (c[1] - k[1]).abs().max((c[1] - k[3]).abs());
                cerca <= r && fx.hypot(fy) >= r
            }
            Forma::Arco {
                c,
                r,
                inicio,
                barrido,
            } => {
                let p0 = polar(c, r, inicio);
                let p1 = polar(c, r, inicio + barrido);
                if en_caja(k, p0) || en_caja(k, p1) {
                    return true;
                }
                // Si no empieza ni acaba dentro, toca solo si cruza un lado.
                let en_arco = |p: P| desde(inicio, angulo(c, p)) <= barrido;
                let corta_x = |x: f64| {
                    let h2 = r * r - (x - c[0]) * (x - c[0]);
                    h2 >= 0.0 && {
                        let h = h2.sqrt();
                        [c[1] - h, c[1] + h]
                            .iter()
                            .any(|&y| y >= k[1] && y <= k[3] && en_arco([x, y]))
                    }
                };
                let corta_y = |y: f64| {
                    let h2 = r * r - (y - c[1]) * (y - c[1]);
                    h2 >= 0.0 && {
                        let h = h2.sqrt();
                        [c[0] - h, c[0] + h]
                            .iter()
                            .any(|&x| x >= k[0] && x <= k[2] && en_arco([x, y]))
                    }
                };
                corta_x(k[0]) || corta_x(k[2]) || corta_y(k[1]) || corta_y(k[3])
            }
        }
    }

    /// Puntos repartidos por la forma (para elegir con ventana o pintar).
    pub fn muestras(&self, n: usize) -> Vec<P> {
        let n = n.max(2);
        match *self {
            Forma::Segmento { a, b } => vec![a, b],
            Forma::Circulo { c, r } => (0..=n)
                .map(|k| polar(c, r, TAU * k as f64 / n as f64))
                .collect(),
            Forma::Arco {
                c,
                r,
                inicio,
                barrido,
            } => (0..=n)
                .map(|k| polar(c, r, inicio + barrido * k as f64 / n as f64))
                .collect(),
        }
    }
}

/// **Una semejanza del plano**: mover, girar, escalar por igual y hacer
/// simetria, juntos. Lleva `p` a `t + k * R(giro) * E(p)`, donde `E` da la
/// vuelta a la y si `espejo`. Asi una forma sigue siendo una forma (un arco
/// sigue siendo un arco) y se transforma sin pasar por la entidad.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Semejanza {
    pub t: P,
    pub k: f64,
    pub giro: f64,
    pub espejo: bool,
}

impl Semejanza {
    /// La que no hace nada.
    pub const NADA: Semejanza = Semejanza {
        t: [0.0, 0.0],
        k: 1.0,
        giro: 0.0,
        espejo: false,
    };

    /// Mover `d`.
    pub fn mover(d: P) -> Semejanza {
        Semejanza {
            t: d,
            ..Semejanza::NADA
        }
    }

    /// Lo que deja `c` en su sitio.
    fn fijando(c: P, k: f64, giro: f64, espejo: bool) -> Semejanza {
        let s = Semejanza {
            t: [0.0, 0.0],
            k,
            giro,
            espejo,
        };
        Semejanza {
            t: restar(c, s.aplicar(c)),
            ..s
        }
    }

    /// Girar `ang` radianes (antihorario) alrededor de `c`.
    pub fn girar(c: P, ang: f64) -> Semejanza {
        Semejanza::fijando(c, 1.0, ang, false)
    }

    /// Escalar `k` veces desde `c` (con `k` negativo, ademas media vuelta).
    pub fn escalar(c: P, k: f64) -> Semejanza {
        if k < 0.0 {
            Semejanza::fijando(c, -k, PI, false)
        } else {
            Semejanza::fijando(c, k, 0.0, false)
        }
    }

    /// Simetria respecto a la recta `a`-`b` (`None` si `a` y `b` coinciden).
    pub fn simetria(a: P, b: P) -> Option<Semejanza> {
        if dist(a, b) < EPS {
            return None;
        }
        // Reflejar en una recta de angulo phi es dar la vuelta a la y y
        // girar 2 phi.
        Some(Semejanza::fijando(a, 1.0, 2.0 * angulo(a, b), true))
    }

    pub fn aplicar(&self, p: P) -> P {
        let q = if self.espejo { [p[0], -p[1]] } else { p };
        sumar(self.t, por(rotar(q, [0.0, 0.0], self.giro), self.k))
    }

    /// La forma transformada.
    pub fn forma(&self, f: &Forma) -> Forma {
        match *f {
            Forma::Segmento { a, b } => Forma::Segmento {
                a: self.aplicar(a),
                b: self.aplicar(b),
            },
            Forma::Circulo { c, r } => Forma::Circulo {
                c: self.aplicar(c),
                r: r * self.k,
            },
            Forma::Arco {
                c,
                r,
                inicio,
                barrido,
            } => {
                // En el espejo el angulo a pasa a -a y el arco va al reves.
                let inicio = if self.espejo {
                    self.giro - inicio - barrido
                } else {
                    self.giro + inicio
                };
                Forma::Arco {
                    c: self.aplicar(c),
                    r: r * self.k,
                    inicio: normalizar(inicio),
                    barrido,
                }
            }
        }
    }
}

/// Donde cae la proyeccion de `p` sobre la recta `a`-`b`: 0 en `a`, 1 en `b`.
pub fn parametro_en_segmento(a: P, b: P, p: P) -> f64 {
    let d = restar(b, a);
    let l2 = punto(d, d);
    if l2 < EPS * EPS {
        0.0
    } else {
        punto(restar(p, a), d) / l2
    }
}

/// El centro del circulo que pasa por tres puntos.
pub fn centro_por_tres(a: P, b: P, c: P) -> Option<P> {
    let d = 2.0 * (a[0] * (b[1] - c[1]) + b[0] * (c[1] - a[1]) + c[0] * (a[1] - b[1]));
    if d.abs() < EPS {
        return None;
    }
    let (a2, b2, c2) = (punto(a, a), punto(b, b), punto(c, c));
    Some([
        (a2 * (b[1] - c[1]) + b2 * (c[1] - a[1]) + c2 * (a[1] - b[1])) / d,
        (a2 * (c[0] - b[0]) + b2 * (a[0] - c[0]) + c2 * (b[0] - a[0])) / d,
    ])
}

/// Donde se cortan las rectas (infinitas) `a1`-`a2` y `b1`-`b2`.
pub fn corte_rectas(a1: P, a2: P, b1: P, b2: P) -> Option<P> {
    let (r, s) = (restar(a2, a1), restar(b2, b1));
    let den = cruz(r, s);
    if den.abs() < EPS * norma(r).max(1.0) * norma(s).max(1.0) {
        return None;
    }
    let t = cruz(restar(b1, a1), s) / den;
    Some(sumar(a1, por(r, t)))
}

/// Donde corta la recta (infinita) `a`-`b` al circulo de centro `c` y radio `r`.
pub fn corte_recta_circulo(a: P, b: P, c: P, r: f64) -> Vec<P> {
    let d = restar(b, a);
    let f = restar(a, c);
    let (aa, bb, cc) = (punto(d, d), 2.0 * punto(f, d), punto(f, f) - r * r);
    if aa < EPS * EPS {
        return Vec::new();
    }
    let disc = bb * bb - 4.0 * aa * cc;
    if disc < -EPS {
        return Vec::new();
    }
    let raiz = disc.max(0.0).sqrt();
    let t1 = (-bb - raiz) / (2.0 * aa);
    let t2 = (-bb + raiz) / (2.0 * aa);
    if (t1 - t2).abs() < EPS {
        vec![sumar(a, por(d, t1))]
    } else {
        vec![sumar(a, por(d, t1)), sumar(a, por(d, t2))]
    }
}

/// Donde se cortan dos circulos.
pub fn corte_circulos(c1: P, r1: f64, c2: P, r2: f64) -> Vec<P> {
    let d = dist(c1, c2);
    if d < EPS || d > r1 + r2 + EPS || d < (r1 - r2).abs() - EPS {
        return Vec::new();
    }
    let a = (r1 * r1 - r2 * r2 + d * d) / (2.0 * d);
    let h = (r1 * r1 - a * a).max(0.0).sqrt();
    let u = unitario(restar(c2, c1));
    let m = sumar(c1, por(u, a));
    if h < EPS {
        vec![m]
    } else {
        vec![sumar(m, por(perp(u), h)), sumar(m, por(perp(u), -h))]
    }
}

/// La forma alargada (la recta entera o el circulo entero), para alargar.
fn alargada(f: &Forma) -> Forma {
    match *f {
        Forma::Arco { c, r, .. } => Forma::Circulo { c, r },
        o => o,
    }
}

fn cortes_sin_limites(a: &Forma, b: &Forma) -> Vec<P> {
    use Forma::*;
    match (alargada(a), alargada(b)) {
        (Segmento { a: a1, b: a2 }, Segmento { a: b1, b: b2 }) => {
            corte_rectas(a1, a2, b1, b2).into_iter().collect()
        }
        (Segmento { a: s1, b: s2 }, Circulo { c, r })
        | (Circulo { c, r }, Segmento { a: s1, b: s2 }) => corte_recta_circulo(s1, s2, c, r),
        (Circulo { c: c1, r: r1 }, Circulo { c: c2, r: r2 }) => corte_circulos(c1, r1, c2, r2),
        _ => Vec::new(),
    }
}

/// **Donde se cortan dos formas** (los puntos que caen en las dos).
pub fn cortes(a: &Forma, b: &Forma) -> Vec<P> {
    let tol = 1e-7 * (1.0 + escala_de(a).max(escala_de(b)));
    cortes_sin_limites(a, b)
        .into_iter()
        .filter(|p| a.contiene_punto(*p, tol) && b.contiene_punto(*p, tol))
        .collect()
}

/// Donde corta `b` a la forma `a` alargada (para alargar `a` hasta `b`).
pub fn cortes_alargando(a: &Forma, b: &Forma) -> Vec<P> {
    let tol = 1e-7 * (1.0 + escala_de(a).max(escala_de(b)));
    cortes_sin_limites(a, b)
        .into_iter()
        .filter(|p| b.contiene_punto(*p, tol))
        .collect()
}

fn escala_de(f: &Forma) -> f64 {
    let c = f.caja();
    (c[2] - c[0]).abs().max((c[3] - c[1]).abs())
}

/// Si el segmento `a`-`b` toca la caja `c` (para elegir con ventana cruzada).
pub fn segmento_toca_caja(a: P, b: P, c: &Caja) -> bool {
    if en_caja(c, a) || en_caja(c, b) {
        return true;
    }
    let esquinas = [[c[0], c[1]], [c[2], c[1]], [c[2], c[3]], [c[0], c[3]]];
    (0..4).any(|i| {
        let (p, q) = (esquinas[i], esquinas[(i + 1) % 4]);
        segmentos_se_cortan(a, b, p, q)
    })
}

pub fn segmentos_se_cortan(a: P, b: P, c: P, d: P) -> bool {
    let o = |p: P, q: P, r: P| cruz(restar(q, p), restar(r, p));
    let (d1, d2, d3, d4) = (o(c, d, a), o(c, d, b), o(a, b, c), o(a, b, d));
    ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0))
}

/// El area con signo de un poligono (positiva si va antihorario).
pub fn area_poligono(p: &[P]) -> f64 {
    let n = p.len();
    if n < 3 {
        return 0.0;
    }
    (0..n).map(|i| cruz(p[i], p[(i + 1) % n])).sum::<f64>() / 2.0
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn cerca(a: P, b: P) -> bool {
        dist(a, b) < 1e-6
    }

    #[test]
    fn rectas_que_se_cortan_y_paralelas() {
        let p = corte_rectas([0.0, 0.0], [10.0, 0.0], [5.0, -5.0], [5.0, 5.0]).unwrap();
        assert!(cerca(p, [5.0, 0.0]));
        assert!(corte_rectas([0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]).is_none());
    }

    #[test]
    fn cortes_de_segmentos_respetan_sus_limites() {
        let a = Forma::Segmento {
            a: [0.0, 0.0],
            b: [10.0, 0.0],
        };
        let b = Forma::Segmento {
            a: [5.0, 1.0],
            b: [5.0, 5.0],
        };
        assert!(cortes(&a, &b).is_empty());
        // Alargando a, sigue sin cortar (b no baja hasta la recta).
        assert!(cortes_alargando(&a, &b).is_empty());
        // Alargando b si corta a.
        assert_eq!(cortes_alargando(&b, &a).len(), 1);
    }

    #[test]
    fn circulo_y_recta() {
        let c = Forma::Circulo {
            c: [0.0, 0.0],
            r: 5.0,
        };
        let s = Forma::Segmento {
            a: [-10.0, 0.0],
            b: [10.0, 0.0],
        };
        let mut v = cortes(&c, &s);
        v.sort_by(|a, b| a[0].total_cmp(&b[0]));
        assert!(cerca(v[0], [-5.0, 0.0]) && cerca(v[1], [5.0, 0.0]));
    }

    #[test]
    fn arco_por_tres_puntos() {
        let f = Forma::arco_por_tres([1.0, 0.0], [0.0, 1.0], [-1.0, 0.0]).unwrap();
        let Forma::Arco {
            c,
            r,
            inicio,
            barrido,
        } = f
        else {
            panic!()
        };
        assert!(cerca(c, [0.0, 0.0]) && (r - 1.0).abs() < 1e-9);
        assert!(inicio.abs() < 1e-9 && (barrido - PI).abs() < 1e-9);
        // Al reves: el mismo semicirculo de arriba.
        let g = Forma::arco_por_tres([-1.0, 0.0], [0.0, 1.0], [1.0, 0.0]).unwrap();
        assert!(cerca(g.punto_medio().unwrap(), [0.0, 1.0]));
        assert!(Forma::arco_por_tres([0.0, 0.0], [1.0, 1.0], [2.0, 2.0]).is_none());
    }

    #[test]
    fn abombado_de_media_vuelta() {
        // bulge 1: semicirculo antihorario de a a b (por debajo, a la derecha del avance).
        let f = Forma::de_abombado([0.0, 0.0], [2.0, 0.0], 1.0);
        let Forma::Arco { c, r, barrido, .. } = f else {
            panic!()
        };
        assert!(cerca(c, [1.0, 0.0]) && (r - 1.0).abs() < 1e-9 && (barrido - PI).abs() < 1e-9);
        assert!(cerca(f.punto_medio().unwrap(), [1.0, -1.0]));
        let g = Forma::de_abombado([0.0, 0.0], [2.0, 0.0], -1.0);
        assert!(cerca(g.punto_medio().unwrap(), [1.0, 1.0]));
    }

    #[test]
    fn caja_de_un_arco_con_su_cuadrante() {
        let f = Forma::Arco {
            c: [0.0, 0.0],
            r: 1.0,
            inicio: 0.0,
            barrido: PI,
        };
        let k = f.caja();
        assert!((k[3] - 1.0).abs() < 1e-9 && (k[1]).abs() < 1e-9);
    }

    /// Compara una forma transformada con sus puntos transformados uno a uno.
    fn igual_por_puntos(s: &Semejanza, f: &Forma) {
        let g = s.forma(f);
        let (a, mut b) = (f.muestras(8), g.muestras(8));
        assert_eq!(a.len(), b.len());
        // En el espejo un arco va al reves.
        if s.espejo && matches!(f, Forma::Arco { .. }) {
            b.reverse();
        }
        for (p, q) in a.iter().zip(&b) {
            assert!(cerca(s.aplicar(*p), *q), "{f:?} con {s:?}: {p:?} -> {q:?}");
        }
    }

    #[test]
    fn semejanzas_llevan_cada_punto_a_su_sitio() {
        let arco = Forma::Arco {
            c: [3.0, 1.0],
            r: 2.0,
            inicio: 0.4,
            barrido: 2.0,
        };
        let seg = Forma::Segmento {
            a: [0.0, 0.0],
            b: [4.0, 1.0],
        };
        let todas = [
            Semejanza::mover([5.0, -2.0]),
            Semejanza::girar([1.0, 1.0], 0.7),
            Semejanza::escalar([2.0, 0.0], 2.5),
            Semejanza::escalar([2.0, 0.0], -1.5),
            Semejanza::simetria([0.0, 0.0], [1.0, 2.0]).unwrap(),
        ];
        for s in &todas {
            igual_por_puntos(s, &arco);
            igual_por_puntos(s, &seg);
        }
        let g = Semejanza::girar([0.0, 0.0], PI / 2.0);
        assert!(cerca(g.aplicar([1.0, 0.0]), [0.0, 1.0]));
        let m = Semejanza::simetria([0.0, 0.0], [1.0, 1.0]).unwrap();
        assert!(cerca(m.aplicar([1.0, 0.0]), [0.0, 1.0]));
        // Caso negativo: una recta de simetria sin largo no vale.
        assert!(Semejanza::simetria([1.0, 1.0], [1.0, 1.0]).is_none());
        let Forma::Circulo { r, .. } = Semejanza::escalar([0.0, 0.0], 3.0).forma(&Forma::Circulo {
            c: [1.0, 1.0],
            r: 2.0,
        }) else {
            panic!()
        };
        assert!((r - 6.0).abs() < 1e-12);
    }

    #[test]
    fn formas_que_tocan_una_caja_sin_muestrear() {
        let k = [0.0, 0.0, 10.0, 10.0];
        let circulo = |c: P, r: f64| Forma::Circulo { c, r };
        // Corta un lado, rodea la caja, cabe dentro, queda fuera.
        assert!(circulo([12.0, 5.0], 3.0).toca_caja(&k));
        assert!(!circulo([5.0, 5.0], 50.0).toca_caja(&k));
        assert!(circulo([5.0, 5.0], 2.0).toca_caja(&k));
        assert!(!circulo([20.0, 20.0], 3.0).toca_caja(&k));
        // Un arco que solo cruza por arriba, y el mismo circulo sin ese trozo.
        let arriba = Forma::Arco {
            c: [5.0, 12.0],
            r: 3.0,
            inicio: PI,
            barrido: PI,
        };
        assert!(arriba.toca_caja(&k));
        let abajo = Forma::Arco {
            c: [5.0, 12.0],
            r: 3.0,
            inicio: 0.0,
            barrido: PI,
        };
        assert!(!abajo.toca_caja(&k));
        // Un arco grande que rodea la caja sin tocarla.
        let grande = Forma::Arco {
            c: [5.0, 5.0],
            r: 40.0,
            inicio: 0.0,
            barrido: 3.0,
        };
        assert!(!grande.toca_caja(&k));
        let s = Forma::Segmento {
            a: [-5.0, 5.0],
            b: [15.0, 5.0],
        };
        assert!(s.toca_caja(&k));
        let s = Forma::Segmento {
            a: [-5.0, 15.0],
            b: [15.0, 12.0],
        };
        assert!(!s.toca_caja(&k));
    }

    #[test]
    fn area_de_un_cuadrado() {
        let a = area_poligono(&[[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]]);
        assert!((a - 4.0).abs() < 1e-12);
    }
}
