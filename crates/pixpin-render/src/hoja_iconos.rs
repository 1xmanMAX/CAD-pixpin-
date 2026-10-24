//! Los 20 conjuntos de iconos del formato condicional de Excel, dibujados
//! con poligonos, circulos y lineas (sin imagenes): nitidos a cualquier zoom.

use std::f32::consts::PI;

use pixpin_hojas::condicional::Conjunto;

use crate::lienzo::{Pintor, RectF};
use crate::motor::Color;

const VERDE: [u8; 3] = [0x3F, 0xA9, 0x4F];
const AMARILLO: [u8; 3] = [0xF5, 0xBE, 0x1E];
const ROJO: [u8; 3] = [0xE0, 0x3C, 0x31];
const ROSA: [u8; 3] = [0xF0, 0x9A, 0x96];
const GRIS: [u8; 3] = [0x80, 0x80, 0x80];
const GRIS_CLARO: [u8; 3] = [0xC8, 0xC8, 0xC8];
const NEGRO: [u8; 3] = [0x26, 0x26, 0x26];
const BLANCO: [u8; 3] = [0xFF, 0xFF, 0xFF];

fn c(rgb: [u8; 3]) -> Color {
    Color {
        r: rgb[0] as f32 / 255.0,
        g: rgb[1] as f32 / 255.0,
        b: rgb[2] as f32 / 255.0,
        a: 1.0,
    }
}

/// Un poligono en coordenadas del icono (-1..1) llevado a pantalla.
struct Caja {
    cx: f32,
    cy: f32,
    r: f32,
}

impl Caja {
    fn p(&self, x: f32, y: f32) -> (f32, f32) {
        (self.cx + x * self.r, self.cy + y * self.r)
    }

    fn poli(&self, p: &Pintor, v: &[(f32, f32)], col: [u8; 3]) {
        let v: Vec<(f32, f32)> = v.iter().map(|&(x, y)| self.p(x, y)).collect();
        p.poligono(&v, c(col));
    }

    fn rotado(&self, v: &[(f32, f32)], grados: f32) -> Vec<(f32, f32)> {
        let (s, co) = (grados.to_radians().sin(), grados.to_radians().cos());
        v.iter()
            .map(|&(x, y)| (x * co - y * s, x * s + y * co))
            .collect()
    }
}

/// Una flecha que apunta hacia arriba, girada `grados` en sentido horario.
fn flecha(p: &Pintor, b: &Caja, grados: f32, col: [u8; 3]) {
    let base = [
        (0.0, -1.0),
        (0.9, -0.05),
        (0.38, -0.05),
        (0.38, 1.0),
        (-0.38, 1.0),
        (-0.38, -0.05),
        (-0.9, -0.05),
    ];
    b.poli(p, &b.rotado(&base, grados), col);
}

fn sector(b: &Caja, cuartos: u32, radio: f32) -> Vec<(f32, f32)> {
    // Desde las 12 en punto, en sentido horario.
    let pasos = 8 * cuartos;
    let mut v = vec![(0.0, 0.0)];
    for i in 0..=pasos {
        let a = -PI / 2.0 + (PI / 2.0) * cuartos as f32 * i as f32 / pasos.max(1) as f32;
        v.push((a.cos() * radio, a.sin() * radio));
    }
    let _ = b;
    v
}

fn estrella(centro: (f32, f32), r: f32) -> Vec<(f32, f32)> {
    (0..10)
        .map(|i| {
            let a = -PI / 2.0 + i as f32 * PI / 5.0;
            let rr = if i % 2 == 0 { r } else { r * 0.42 };
            (centro.0 + a.cos() * rr, centro.1 + a.sin() * rr)
        })
        .collect()
}

/// Pinta el icono `idx` (0 = el peor) de `conjunto` centrado en `(cx, cy)`
/// dentro de un cuadrado de lado `lado`.
pub fn pintar_icono(p: &Pintor, conjunto: Conjunto, idx: u8, cx: f32, cy: f32, lado: f32) {
    let b = Caja {
        cx,
        cy,
        r: lado / 2.0,
    };
    let idx = idx as usize;
    let k = conjunto.cantidad();
    let idx = idx.min(k - 1);
    let semaforo = [ROJO, AMARILLO, VERDE];
    match conjunto {
        Conjunto::Flechas3
        | Conjunto::FlechasGris3
        | Conjunto::Flechas4
        | Conjunto::FlechasGris4
        | Conjunto::Flechas5
        | Conjunto::FlechasGris5 => {
            let gris = matches!(
                conjunto,
                Conjunto::FlechasGris3 | Conjunto::FlechasGris4 | Conjunto::FlechasGris5
            );
            let (grados, col): (f32, [u8; 3]) = match (k, idx) {
                (3, 0) => (180.0, ROJO),
                (3, 1) => (90.0, AMARILLO),
                (3, _) => (0.0, VERDE),
                (4, 0) => (180.0, ROJO),
                (4, 1) => (135.0, AMARILLO),
                (4, 2) => (45.0, AMARILLO),
                (4, _) => (0.0, VERDE),
                (_, 0) => (180.0, ROJO),
                (_, 1) => (135.0, AMARILLO),
                (_, 2) => (90.0, AMARILLO),
                (_, 3) => (45.0, AMARILLO),
                _ => (0.0, VERDE),
            };
            flecha(p, &b, grados, if gris { GRIS } else { col });
        }
        Conjunto::Banderas3 => {
            let col = semaforo[idx];
            p.linea(
                b.p(-0.7, -0.95),
                b.p(-0.7, 0.95),
                (b.r * 0.2).max(1.0),
                c(GRIS),
            );
            b.poli(p, &[(-0.7, -0.9), (0.9, -0.55), (-0.7, 0.1)], col);
        }
        Conjunto::Semaforo3 | Conjunto::Semaforo3Borde | Conjunto::Semaforo4 => {
            let col = if k == 4 {
                [NEGRO, ROJO, AMARILLO, VERDE][idx]
            } else {
                semaforo[idx]
            };
            p.circulo((cx, cy), b.r * 0.9, c(col));
            if conjunto == Conjunto::Semaforo3Borde {
                p.anillo((cx, cy), b.r * 0.86, (b.r * 0.16).max(1.0), c(NEGRO));
            }
        }
        Conjunto::RojoNegro4 => {
            let col = [NEGRO, GRIS, ROSA, ROJO][idx];
            p.circulo((cx, cy), b.r * 0.9, c(col));
        }
        Conjunto::Senales3 => match idx {
            0 => b.poli(
                p,
                &[(0.0, -1.0), (0.95, 0.0), (0.0, 1.0), (-0.95, 0.0)],
                ROJO,
            ),
            1 => b.poli(p, &[(0.0, -0.9), (1.0, 0.85), (-1.0, 0.85)], AMARILLO),
            _ => p.circulo((cx, cy), b.r * 0.9, c(VERDE)),
        },
        Conjunto::Simbolos3 | Conjunto::Simbolos3Sin => {
            let con_circulo = conjunto == Conjunto::Simbolos3;
            let col = semaforo[idx];
            let tinta = if con_circulo { BLANCO } else { col };
            if con_circulo {
                p.circulo((cx, cy), b.r * 0.9, c(col));
            }
            let g = (b.r * 0.26).max(1.2);
            match idx {
                0 => {
                    p.linea(b.p(-0.4, -0.4), b.p(0.4, 0.4), g, c(tinta));
                    p.linea(b.p(0.4, -0.4), b.p(-0.4, 0.4), g, c(tinta));
                }
                1 => {
                    p.linea(b.p(0.0, -0.5), b.p(0.0, 0.15), g, c(tinta));
                    p.circulo(b.p(0.0, 0.55), g * 0.55, c(tinta));
                }
                _ => p.polilinea(
                    &[b.p(-0.45, 0.0), b.p(-0.1, 0.4), b.p(0.5, -0.4)],
                    g,
                    c(tinta),
                ),
            }
        }
        Conjunto::Valoracion4 | Conjunto::Valoracion5 => {
            let llenas = if k == 4 { idx + 1 } else { idx };
            for i in 0..4 {
                let x0 = -0.95 + i as f32 * 0.5;
                let alto = 0.5 + 0.5 * i as f32 / 3.0 * 1.0;
                let top = 1.0 - 2.0 * alto;
                let r = RectF {
                    x: b.p(x0, 0.0).0,
                    y: b.p(0.0, top).1,
                    ancho: b.r * 0.38,
                    alto: b.r * 2.0 * alto,
                };
                p.rellenar(r, c(if i < llenas { NEGRO } else { GRIS_CLARO }));
            }
        }
        Conjunto::Cuartos5 => {
            let radio = b.r * 0.88;
            p.circulo((cx, cy), radio, c(BLANCO));
            if idx > 0 {
                let v: Vec<(f32, f32)> = sector(&b, idx as u32, 1.0)
                    .iter()
                    .map(|&(x, y)| (cx + x * radio, cy + y * radio))
                    .collect();
                p.poligono(&v, c(NEGRO));
            }
            p.anillo((cx, cy), radio, (b.r * 0.14).max(1.0), c(NEGRO));
        }
        Conjunto::Cajas5 => {
            let r = RectF {
                x: cx - b.r * 0.85,
                y: cy - b.r * 0.85,
                ancho: b.r * 1.7,
                alto: b.r * 1.7,
            };
            if idx > 0 {
                let alto = r.alto * idx as f32 / 4.0;
                p.rellenar(
                    RectF {
                        y: r.y + r.alto - alto,
                        alto,
                        ..r
                    },
                    c(NEGRO),
                );
            }
            p.trazar(r, (b.r * 0.14).max(1.0), c(NEGRO));
        }
        Conjunto::Estrellas3 => {
            let est = estrella((cx, cy + b.r * 0.06), b.r * 1.0);
            let dorado = [0xF2, 0xB4, 0x00];
            match idx {
                2 => p.poligono(&est, c(dorado)),
                1 => {
                    p.poligono(&est, c(GRIS_CLARO));
                    p.empujar_recorte(RectF {
                        x: cx - b.r * 1.1,
                        y: cy - b.r * 1.1,
                        ancho: b.r * 1.1,
                        alto: b.r * 2.2,
                    });
                    p.poligono(&est, c(dorado));
                    p.soltar_recorte();
                }
                _ => p.poligono(&est, c(GRIS_CLARO)),
            }
        }
        Conjunto::Triangulos3 => match idx {
            0 => b.poli(p, &[(-0.9, -0.6), (0.9, -0.6), (0.0, 0.8)], ROJO),
            1 => p.rellenar(
                RectF {
                    x: cx - b.r * 0.9,
                    y: cy - b.r * 0.14,
                    ancho: b.r * 1.8,
                    alto: b.r * 0.28,
                },
                c(AMARILLO),
            ),
            _ => b.poli(p, &[(-0.9, 0.6), (0.9, 0.6), (0.0, -0.8)], VERDE),
        },
    }
}
