//! Porte fiel de `LaserPointer` de Excalidraw (MIT, Copyright (c) 2020
//! Excalidraw), `packages/laser-pointer/src/state.ts` y `math.ts` @afa3a65,
//! publicado como `@excalidraw/laser-pointer@1.3.1`.
//!
//! Es la pluma de grosor constante: Excalidraw la usa con `simplify: 0`,
//! presion fijada a 1 y `sizeMapping = max(0.1, presion)`, asi que el
//! tamano es siempre `size`. Solo se porta ese camino: la simplificacion,
//! `keepHead` y la cola estable/inestable no cambian la salida con esas
//! opciones (la cola solo agrupa puntos, el contorno se hace con todos).
//!
//! Los bucles de angulo suman `PI / 16` en f64, como el original: ver
//! `freehand.rs` para por que eso importa.
//!
//! La rama de `state.ts` que trata `cSize === 0` y calcula `visibleStartIndex`
//! no se porta: ese camino existe para el trazo que se desvanece con el
//! tiempo (`fadeOutTime`), donde el tamano visible del rastro puede llegar a
//! cero puntos; aqui el tamano es constante y siempre mayor que cero, asi que
//! esa rama nunca se toma.

use std::f64::consts::PI;

type V = [f64; 2];

const CORNER_DETECTION_MAX_ANGLE: f64 = 75.0;

fn add(a: V, b: V) -> V {
    [a[0] + b[0], a[1] + b[1]]
}
fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1]]
}
fn smul(a: V, s: f64) -> V {
    [a[0] * s, a[1] * s]
}
fn norm(a: V) -> V {
    let m = (a[0].powi(2) + a[1].powi(2)).sqrt();
    [a[0] / m, a[1] / m]
}
fn rot(a: V, rad: f64) -> V {
    [
        rad.cos() * a[0] - rad.sin() * a[1],
        rad.sin() * a[0] + rad.cos() * a[1],
    ]
}
fn plerp(a: V, b: V, t: f64) -> V {
    add(a, smul(sub(b, a), t))
}
fn angle(p: V, p1: V, p2: V) -> f64 {
    (p2[1] - p[1]).atan2(p2[0] - p[0]) - (p1[1] - p[1]).atan2(p1[0] - p[0])
}
fn norm_angle(a: f64) -> f64 {
    a.sin().atan2(a.cos())
}
fn mag(a: V) -> f64 {
    (a[0].powi(2) + a[1].powi(2)).sqrt()
}
fn dist(a: V, b: V) -> f64 {
    ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt()
}

/// `addPoint` repetido: descarta duplicados exactos del punto ORIGINAL y
/// suaviza contra el ultimo punto YA suavizado.
fn suavizar(puntos: &[V], streamline: f64) -> Vec<V> {
    let mut originales: Vec<V> = Vec::new();
    let mut salida: Vec<V> = Vec::new();
    for &p in puntos {
        if originales.last() == Some(&p) {
            continue;
        }
        originales.push(p);
        let q = match salida.last() {
            Some(&ultimo) if streamline > 0.0 => plerp(ultimo, p, 1.0 - streamline),
            _ => p,
        };
        salida.push(q);
    }
    salida
}

pub fn contorno(puntos: &[V], size: f64, streamline: f64) -> Vec<V> {
    let points = suavizar(puntos, streamline);
    let len = points.len();
    if len == 0 {
        return Vec::new();
    }
    let paso = PI / 16.0;

    if len == 1 {
        let c = points[0];
        if size < 0.5 {
            return Vec::new();
        }
        let mut ps = Vec::new();
        let mut theta = 0.0;
        while theta <= PI * 2.0 {
            ps.push(add(c, smul(rot([1.0, 0.0], theta), size)));
            theta += paso;
        }
        ps.push(add(c, smul([1.0, 0.0], size)));
        return ps;
    }

    if len == 2 {
        let (c, n) = (points[0], points[1]);
        if size < 0.5 {
            return Vec::new();
        }
        let mut ps = Vec::new();
        let p_angle = angle(c, [c[0], c[1] - 100.0], n);
        let mut theta = p_angle;
        while theta <= PI + p_angle {
            ps.push(add(c, smul(rot([1.0, 0.0], theta), size)));
            theta += paso;
        }
        let mut theta = PI + p_angle;
        while theta <= PI * 2.0 + p_angle {
            ps.push(add(n, smul(rot([1.0, 0.0], theta), size)));
            theta += paso;
        }
        ps.push(ps[0]);
        return ps;
    }

    let mut forward: Vec<V> = Vec::new();
    let mut backward: Vec<V> = Vec::new();
    let mut prev_speed = 0.0;

    for i in 1..len - 1 {
        let (p, c, n) = (points[i - 1], points[i], points[i + 1]);
        let d = dist(p, c);
        let speed = prev_speed + (d - prev_speed) * 0.2;
        let c_size = size;

        let dir_pc = norm(sub(p, c));
        let dir_nc = norm(sub(n, c));
        let p1dir_pc = rot(dir_pc, PI / 2.0);
        let p2dir_pc = rot(dir_pc, -PI / 2.0);
        let p1dir_nc = rot(dir_nc, PI / 2.0);
        let p2dir_nc = rot(dir_nc, -PI / 2.0);
        let p1_pc = add(c, smul(p1dir_pc, c_size));
        let p2_pc = add(c, smul(p2dir_pc, c_size));
        let p1_nc = add(c, smul(p1dir_nc, c_size));
        let p2_nc = add(c, smul(p2dir_nc, c_size));
        let ftdir = add(p1dir_pc, p2dir_nc);
        let btdir = add(p2dir_pc, p1dir_nc);
        let pa_pc = add(
            c,
            smul(
                if mag(ftdir) == 0.0 {
                    dir_pc
                } else {
                    norm(ftdir)
                },
                c_size,
            ),
        );
        let pa_nc = add(
            c,
            smul(
                if mag(btdir) == 0.0 {
                    dir_nc
                } else {
                    norm(btdir)
                },
                c_size,
            ),
        );

        let c_angle = norm_angle(angle(c, p, n));
        let variance = if speed > 35.0 { 0.5 } else { 1.0 };
        let d_angle = (CORNER_DETECTION_MAX_ANGLE / 180.0) * PI * variance;

        if c_angle.abs() < d_angle {
            let t_angle = norm_angle(PI - c_angle).abs();
            if t_angle == 0.0 {
                // El original hace `continue` ANTES de `prevSpeed = speed`.
                continue;
            }
            if c_angle < 0.0 {
                backward.push(p2_pc);
                backward.push(pa_nc);
                let mut theta = 0.0;
                while theta <= t_angle {
                    forward.push(add(c, rot(smul(p1dir_pc, c_size), theta)));
                    theta += t_angle / 4.0;
                }
                let mut theta = t_angle;
                while theta >= 0.0 {
                    backward.push(add(c, rot(smul(p1dir_pc, c_size), theta)));
                    theta -= t_angle / 4.0;
                }
                backward.push(pa_nc);
                backward.push(p1_nc);
            } else {
                forward.push(p1_pc);
                forward.push(pa_pc);
                let mut theta = 0.0;
                while theta <= t_angle {
                    backward.push(add(c, rot(smul(p1dir_pc, -c_size), -theta)));
                    theta += t_angle / 4.0;
                }
                let mut theta = t_angle;
                while theta >= 0.0 {
                    forward.push(add(c, rot(smul(p1dir_pc, -c_size), -theta)));
                    theta -= t_angle / 4.0;
                }
                forward.push(pa_pc);
                forward.push(p2_nc);
            }
        } else {
            forward.push(pa_pc);
            backward.push(pa_nc);
        }
        prev_speed = speed;
    }

    let first = points[0];
    let second = points[1];
    let penultimate = points[len - 2];
    let ultimate = points[len - 1];
    let ppdir_fs = rot(norm(sub(second, first)), -PI / 2.0);
    let ppdir_pu = rot(norm(sub(penultimate, ultimate)), PI / 2.0);

    let mut start_cap: Vec<V> = Vec::new();
    if size > 0.1 {
        let mut theta = 0.0;
        while theta <= PI {
            start_cap.insert(0, add(first, rot(smul(ppdir_fs, size), -theta)));
            theta += paso;
        }
        start_cap.insert(0, add(first, smul(ppdir_fs, -size)));
    } else {
        start_cap.push(first);
    }

    let mut end_cap: Vec<V> = Vec::new();
    let mut theta = 0.0;
    while theta <= PI * 3.0 {
        end_cap.push(add(ultimate, rot(smul(ppdir_pu, -size), -theta)));
        theta += paso;
    }

    let mut salida = start_cap.clone();
    salida.extend(forward);
    salida.extend(end_cap.into_iter().rev());
    salida.extend(backward.into_iter().rev());
    if let Some(&primero) = start_cap.first() {
        salida.push(primero);
    }
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn sin_puntos_no_hay_contorno() {
        assert!(contorno(&[], 1.4, 0.5).is_empty());
    }

    #[test]
    fn un_punto_con_tamano_menor_de_medio_pixel_no_se_dibuja() {
        assert!(contorno(&[[0.0, 0.0]], 0.4, 0.5).is_empty());
    }

    #[test]
    fn los_puntos_repetidos_no_cambian_el_contorno() {
        let a = contorno(&[[0.0, 0.0], [10.0, 0.0], [20.0, 5.0]], 1.4, 0.5);
        let b = contorno(
            &[[0.0, 0.0], [10.0, 0.0], [10.0, 0.0], [20.0, 5.0]],
            1.4,
            0.5,
        );
        assert_eq!(a, b);
    }

    #[test]
    fn el_grosor_de_una_recta_larga_es_constante() {
        let recta: Vec<V> = (0..50).map(|i| [i as f64 * 5.0, 0.0]).collect();
        let c = contorno(&recta, 2.0, 0.5);
        // Lejos de las tapas, todos los puntos quedan a `size` de la linea.
        let medios: Vec<&V> = c.iter().filter(|p| p[0] > 20.0 && p[0] < 200.0).collect();
        assert!(!medios.is_empty());
        assert!(medios.iter().all(|p| (p[1].abs() - 2.0).abs() < 1e-6));
    }
}
