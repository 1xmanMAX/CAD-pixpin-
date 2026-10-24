//! Curvas a rayas: arcos, arcos de polilinea (*bulge*), elipses y splines.
//! Se trocean una vez, al abrir: lo bastante finas para verse redondas
//! acercandose mucho, sin pasarse (cada vertice es memoria de la GPU).

use std::f64::consts::TAU;

thread_local! {
    /// El error de cuerda que no se nota en este plano: una parte en dos
    /// millones de lo que mide (ver [`poner_tamano_del_plano`]).
    static TOLERANCIA: std::cell::Cell<f64> = const { std::cell::Cell::new(0.0) };
}

/// Lo que mide el plano (su lado mayor), para trocear las curvas solo lo
/// que se pueda ver: un circulo de medio metro en un plano de cinco
/// kilometros no necesita cien lados.
pub fn poner_tamano_del_plano(lado: f64) {
    let t = if lado.is_finite() && lado > 0.0 {
        lado * 5e-7
    } else {
        0.0
    };
    TOLERANCIA.with(|c| c.set(t));
}

/// Cuantos trozos para un arco de `barrido` radianes y radio `r`: el error
/// de cuerda es 1/500 del radio o la tolerancia del plano, lo que sea mayor.
pub fn trozos(barrido: f64, r: f64) -> usize {
    let r = r.abs();
    if !(r > 0.0) || !r.is_finite() {
        return 4;
    }
    let tol = (r * 0.002).max(TOLERANCIA.with(|c| c.get())).min(r * 0.5);
    let paso = 2.0 * (1.0 - tol / r).clamp(-1.0, 1.0).acos();
    ((barrido.abs() / paso.max(1e-3)).ceil() as usize).clamp(4, 360)
}

/// Un arco de `a0` a `a1` (radianes, contra reloj).
pub fn arco(c: [f64; 2], r: f64, a0: f64, a1: f64) -> Vec<[f64; 2]> {
    let mut barrido = a1 - a0;
    while barrido <= 0.0 {
        barrido += TAU;
    }
    if barrido > TAU {
        barrido = TAU;
    }
    let n = trozos(barrido, r);
    (0..=n)
        .map(|i| {
            let a = a0 + barrido * i as f64 / n as f64;
            [c[0] + r * a.cos(), c[1] + r * a.sin()]
        })
        .collect()
}

pub fn circulo(c: [f64; 2], r: f64) -> Vec<[f64; 2]> {
    let n = trozos(TAU, r);
    (0..=n)
        .map(|i| {
            let a = TAU * i as f64 / n as f64;
            [c[0] + r * a.cos(), c[1] + r * a.sin()]
        })
        .collect()
}

/// El tramo de polilinea de `p` a `q` con su `bulge` (tangente de un
/// cuarto del angulo; positivo = contra reloj), sin el punto `p`.
pub fn tramo_bulge(p: [f64; 2], q: [f64; 2], bulge: f64, salida: &mut Vec<[f64; 2]>) {
    let bulge = bulge.clamp(-1e6, 1e6);
    let (dx, dy) = (q[0] - p[0], q[1] - p[1]);
    let cuerda = (dx * dx + dy * dy).sqrt();
    if bulge.abs() < 1e-9 || cuerda < 1e-12 || !bulge.is_finite() {
        salida.push(q);
        return;
    }
    let theta = 4.0 * bulge.atan();
    let r = cuerda * (1.0 + bulge * bulge) / (4.0 * bulge.abs());
    // Centro: en la mediatriz, a la izquierda de p→q si el arco va contra
    // reloj y es menor que media vuelta.
    let (mx, my) = ((p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0);
    let (nx, ny) = (-dy / cuerda, dx / cuerda);
    let h = cuerda * (1.0 - bulge * bulge) / (4.0 * bulge);
    let c = [mx + nx * h, my + ny * h];
    let a0 = (p[1] - c[1]).atan2(p[0] - c[0]);
    let n = trozos(theta, r);
    for i in 1..=n {
        let a = a0 + theta * i as f64 / n as f64;
        salida.push([c[0] + r * a.cos(), c[1] + r * a.sin()]);
    }
    if let Some(u) = salida.last_mut() {
        *u = q;
    }
}

/// Una elipse (o un trozo): centro, semieje mayor como vector, razon del
/// menor y parametros de inicio y fin.
pub fn elipse(c: [f64; 2], mayor: [f64; 2], razon: f64, t0: f64, t1: f64) -> Vec<[f64; 2]> {
    let menor = [-mayor[1] * razon, mayor[0] * razon];
    let mut barrido = t1 - t0;
    while barrido <= 0.0 {
        barrido += TAU;
    }
    if barrido > TAU + 1e-9 {
        barrido = TAU;
    }
    let r = (mayor[0] * mayor[0] + mayor[1] * mayor[1]).sqrt();
    let n = trozos(barrido, r);
    (0..=n)
        .map(|i| {
            let t = t0 + barrido * i as f64 / n as f64;
            let (s, k) = t.sin_cos();
            [
                c[0] + mayor[0] * k + menor[0] * s,
                c[1] + mayor[1] * k + menor[1] * s,
            ]
        })
        .collect()
}

/// Una B-spline (racional si hay pesos), evaluada con de Boor en
/// `muestras` puntos repartidos por su dominio.
pub fn spline(grado: usize, nudos: &[f64], control: &[[f64; 3]], pesos: &[f64]) -> Vec<[f64; 2]> {
    let n = control.len();
    if n < 2 {
        return Vec::new();
    }
    let p = grado.clamp(1, 10).min(n - 1);
    // Nudos que faltan o no cuadran: uniformes y anclados.
    let nudos: Vec<f64> = if nudos.len() == n + p + 1 {
        nudos.to_vec()
    } else {
        let mut k = vec![0.0; p + 1];
        for i in 1..(n - p) {
            k.push(i as f64);
        }
        k.extend(std::iter::repeat_n((n - p) as f64, p + 1));
        k
    };
    let peso = |i: usize| -> f64 {
        let w = pesos.get(i).copied().unwrap_or(1.0);
        if w > 0.0 && w.is_finite() { w } else { 1.0 }
    };
    let (u0, u1) = (nudos[p], nudos[n]);
    if !(u1 > u0) {
        return control.iter().map(|c| [c[0], c[1]]).collect();
    }
    let muestras = (n * 12).clamp(16, 2000);
    let mut out = Vec::with_capacity(muestras + 1);
    for s in 0..=muestras {
        let u = u0 + (u1 - u0) * s as f64 / muestras as f64;
        // El tramo de nudos donde cae u.
        let mut k = p;
        while k < n - 1 && nudos[k + 1] <= u {
            k += 1;
        }
        let mut d: Vec<[f64; 3]> = (0..=p)
            .map(|j| {
                let c = control[j + k - p];
                let w = peso(j + k - p);
                [c[0] * w, c[1] * w, w]
            })
            .collect();
        for r in 1..=p {
            for j in (r..=p).rev() {
                let i = j + k - p;
                let den = nudos[i + p + 1 - r] - nudos[i];
                let a = if den.abs() < 1e-300 {
                    0.0
                } else {
                    (u - nudos[i]) / den
                };
                let antes = d[j - 1];
                for (e, v) in d[j].iter_mut().enumerate() {
                    *v = (1.0 - a) * antes[e] + a * *v;
                }
            }
        }
        let w = if d[p][2].abs() < 1e-300 { 1.0 } else { d[p][2] };
        out.push([d[p][0] / w, d[p][1] / w]);
    }
    out
}

/// Una curva suave por los puntos (spline solo con puntos de paso): Catmull-Rom.
pub fn por_puntos(pts: &[[f64; 2]]) -> Vec<[f64; 2]> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let mut out = vec![pts[0]];
    for i in 0..pts.len() - 1 {
        let p0 = pts[i.saturating_sub(1)];
        let (p1, p2) = (pts[i], pts[i + 1]);
        let p3 = pts[(i + 2).min(pts.len() - 1)];
        for s in 1..=8 {
            let t = s as f64 / 8.0;
            let (t2, t3) = (t * t, t * t * t);
            let f = |a: f64, b: f64, c: f64, d: f64| {
                0.5 * (2.0 * b
                    + (-a + c) * t
                    + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
                    + (-a + 3.0 * b - 3.0 * c + d) * t3)
            };
            out.push([f(p0[0], p1[0], p2[0], p3[0]), f(p0[1], p1[1], p2[1], p3[1])]);
        }
    }
    out
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn cerca(a: [f64; 2], b: [f64; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9
    }

    #[test]
    fn un_bulge_de_uno_es_medio_circulo_hacia_la_izquierda() {
        let mut v = Vec::new();
        tramo_bulge([0.0, 0.0], [2.0, 0.0], 1.0, &mut v);
        assert!(cerca(*v.last().unwrap(), [2.0, 0.0]));
        // Contra reloj de (0,0) a (2,0): el medio pasa por (1,-1).
        let medio = v[v.len() / 2 - 1];
        assert!((medio[1] + 1.0).abs() < 0.01, "{medio:?}");
        // Negativo: por arriba.
        let mut w = Vec::new();
        tramo_bulge([0.0, 0.0], [2.0, 0.0], -1.0, &mut w);
        assert!((w[w.len() / 2 - 1][1] - 1.0).abs() < 0.01);
        // Caso negativo: sin bulge, recto, un solo punto.
        let mut r = Vec::new();
        tramo_bulge([0.0, 0.0], [2.0, 0.0], 0.0, &mut r);
        assert_eq!(r, vec![[2.0, 0.0]]);
    }

    #[test]
    fn el_arco_va_contra_reloj_y_cruza_el_cero() {
        let a = arco([0.0, 0.0], 1.0, 350f64.to_radians(), 10f64.to_radians());
        assert!(cerca(
            a[0],
            [350f64.to_radians().cos(), 350f64.to_radians().sin()]
        ));
        assert!(a.iter().all(|p| p[0] > 0.9));
        assert!(circulo([0.0, 0.0], 5.0).len() > 50);
    }

    #[test]
    fn la_spline_pasa_por_sus_extremos() {
        let s = spline(
            3,
            &[0., 0., 0., 0., 1., 1., 1., 1.],
            &[[0., 0., 0.], [1., 2., 0.], [3., 2., 0.], [4., 0., 0.]],
            &[],
        );
        assert!(cerca(s[0], [0.0, 0.0]) && cerca(*s.last().unwrap(), [4.0, 0.0]));
        // Nudos que no cuadran: se inventan y sigue saliendo algo.
        assert!(!spline(3, &[1.0], &[[0., 0., 0.], [1., 1., 0.], [2., 0., 0.]], &[]).is_empty());
        let e = elipse([0.0, 0.0], [2.0, 0.0], 0.5, 0.0, TAU);
        assert!(cerca(e[0], [2.0, 0.0]));
    }
}
