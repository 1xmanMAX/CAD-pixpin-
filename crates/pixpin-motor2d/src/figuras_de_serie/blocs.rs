//! **Los blocs**: hojas para escribir o dibujar encima (rayado, Cornell,
//! columnas, cuadricula, puntos, isometrica, hexagonal, tianzige y mizige).
//! Sin titulo grande: solo el papel, una cabecera chica con «Tema» y
//! «Fecha», y la pauta.

use super::plantillas::{BLANCO, BORDE, H, M, ROTULO, W};
use super::{Al, Lapiz, WORK_SANS, hex};
use crate::biblioteca::FiguraGuardada;
use crate::elemento::{ColorRgba, EstiloTrazo};

const CATEGORIA: &str = "Blocs";
const AZUL: ColorRgba = hex(0xb9cde6);
const ROJO: ColorRgba = hex(0xe8a0a8);
const GRIS: ColorRgba = hex(0xd3d9e2);
const GRIS_FUERTE: ColorRgba = hex(0xaab4c3);
/// Donde empieza la pauta, bajo la cabecera.
const Y0: f32 = 96.0;

pub(super) fn todas(v: &mut Vec<FiguraGuardada>) {
    v.extend([
        rayado(),
        cornell(),
        columnas(2),
        columnas(3),
        cuadricula(),
        puntos(),
        isometrica(),
        hexagonal(),
        tianzige(),
        mizige(),
    ]);
}

/// El papel con su cabecera chica.
fn hoja() -> Lapiz {
    let mut l = Lapiz::default();
    l.rect(
        0.0,
        0.0,
        W,
        H,
        Some(hex(0xdde3ec)),
        Some(BLANCO),
        1.0,
        false,
    );
    l.texto(M, 40.0, "TEMA", 10.0, WORK_SANS, ROTULO, true, Al::Izq);
    l.linea(M + 42.0, 56.0, W - M - 230.0, 56.0, BORDE, 1.0);
    l.texto(
        W - M - 200.0,
        40.0,
        "FECHA",
        10.0,
        WORK_SANS,
        ROTULO,
        true,
        Al::Izq,
    );
    l.linea(W - M - 150.0, 56.0, W - M, 56.0, BORDE, 1.0);
    l
}

fn rayado() -> FiguraGuardada {
    let mut l = hoja();
    let mut y = Y0 + 20.0;
    while y < H - 30.0 {
        l.linea(0.0, y, W, y, AZUL, 1.0);
        y += 30.0;
    }
    l.linea(92.0, Y0 - 10.0, 92.0, H, ROJO, 1.25);
    l.figura("bloc-rayado", CATEGORIA, "Rayado")
}

fn cornell() -> FiguraGuardada {
    let mut l = hoja();
    let x = 230.0;
    let abajo = H - 210.0;
    l.texto(
        M,
        Y0 - 6.0,
        "PREGUNTAS / CLAVES",
        9.5,
        WORK_SANS,
        ROTULO,
        true,
        Al::Izq,
    );
    l.texto(
        x + 20.0,
        Y0 - 6.0,
        "NOTAS",
        9.5,
        WORK_SANS,
        ROTULO,
        true,
        Al::Izq,
    );
    l.linea(0.0, Y0 + 14.0, W, Y0 + 14.0, GRIS_FUERTE, 1.25);
    l.linea(x, Y0 + 14.0, x, abajo, GRIS_FUERTE, 1.25);
    let mut y = Y0 + 44.0;
    while y < abajo - 4.0 {
        l.linea(x + 14.0, y, W - M, y, AZUL, 1.0);
        y += 28.0;
    }
    l.linea(0.0, abajo, W, abajo, GRIS_FUERTE, 1.25);
    l.texto(
        M,
        abajo + 12.0,
        "RESUMEN",
        9.5,
        WORK_SANS,
        ROTULO,
        true,
        Al::Izq,
    );
    l.figura("bloc-cornell", CATEGORIA, "Notas Cornell")
}

fn columnas(n: usize) -> FiguraGuardada {
    let mut l = hoja();
    let hueco = 26.0;
    let ancho = (W - 2.0 * M - hueco * (n - 1) as f32) / n as f32;
    for c in 0..n {
        let x = M + (ancho + hueco) * c as f32;
        l.linea(x, Y0 + 10.0, x + ancho, Y0 + 10.0, GRIS_FUERTE, 1.5);
        let mut y = Y0 + 40.0;
        while y < H - M {
            l.linea(x, y, x + ancho, y, AZUL, 1.0);
            y += 28.0;
        }
    }
    let (clave, nombre) = if n == 2 {
        ("bloc-dos-columnas", "Dos columnas")
    } else {
        ("bloc-tres-columnas", "Tres columnas")
    };
    l.figura(clave, CATEGORIA, nombre)
}

/// La rejilla de ingenieria: fina cada 5 mm, fuerte cada 5 cuadros.
fn cuadricula() -> FiguraGuardada {
    let mut l = hoja();
    let paso = 18.9;
    let (x0, y0) = (M - 10.0, Y0);
    let cols = ((W - 2.0 * x0) / paso) as usize;
    let filas = ((H - y0 - 30.0) / paso) as usize;
    let (x1, y1) = (x0 + paso * cols as f32, y0 + paso * filas as f32);
    for c in 0..=cols {
        let x = x0 + paso * c as f32;
        let (color, g) = if c % 5 == 0 {
            (GRIS_FUERTE, 1.0)
        } else {
            (GRIS, 0.75)
        };
        l.linea(x, y0, x, y1, color, g);
    }
    for f in 0..=filas {
        let y = y0 + paso * f as f32;
        let (color, g) = if f % 5 == 0 {
            (GRIS_FUERTE, 1.0)
        } else {
            (GRIS, 0.75)
        };
        l.linea(x0, y, x1, y, color, g);
    }
    l.figura("bloc-cuadricula", CATEGORIA, "Cuadrícula")
}

fn puntos() -> FiguraGuardada {
    let mut l = hoja();
    let paso = 24.0;
    let mut y = Y0 + 12.0;
    while y < H - 30.0 {
        let mut x = M - 8.0;
        while x < W - M + 10.0 {
            l.circulo(x, y, 1.8, None, Some(GRIS_FUERTE), 1.0);
            x += paso;
        }
        y += paso;
    }
    l.figura("bloc-puntos", CATEGORIA, "Puntos")
}

/// La recta por `p` con direccion `d`, recortada al rectangulo.
fn recortar(
    p: (f32, f32),
    d: (f32, f32),
    caja: (f32, f32, f32, f32),
) -> Option<((f32, f32), (f32, f32))> {
    let (x0, y0, x1, y1) = caja;
    let (mut t0, mut t1) = (f32::MIN, f32::MAX);
    for (o, v, a, b) in [(p.0, d.0, x0, x1), (p.1, d.1, y0, y1)] {
        if v.abs() < 1e-6 {
            if o < a || o > b {
                return None;
            }
        } else {
            let (ta, tb) = ((a - o) / v, (b - o) / v);
            t0 = t0.max(ta.min(tb));
            t1 = t1.min(ta.max(tb));
        }
    }
    (t1 > t0 + 1.0).then_some((
        (p.0 + d.0 * t0, p.1 + d.1 * t0),
        (p.0 + d.0 * t1, p.1 + d.1 * t1),
    ))
}

/// La isometrica: verticales y dos familias a 30°, todo recortado al papel.
fn isometrica() -> FiguraGuardada {
    let mut l = hoja();
    let caja = (M - 10.0, Y0, W - M + 10.0, H - 30.0);
    let lado = 24.0;
    let dx = lado * 3f32.sqrt() / 2.0;
    let mut x = caja.0;
    while x <= caja.2 + 0.1 {
        l.linea(x, caja.1, x, caja.3, GRIS, 0.75);
        x += dx;
    }
    let (c, s) = (30f32.to_radians().cos(), 30f32.to_radians().sin());
    for d in [(c, s), (c, -s)] {
        // Las rectas cortan la vertical de la izquierda cada `lado`.
        let alto = caja.3 - caja.1;
        let ancho = caja.2 - caja.0;
        let mut k = -((ancho * s / c) / lado).ceil() - 1.0;
        while caja.1 + k * lado <= caja.1 + alto + ancho * s / c + lado {
            if let Some((a, b)) = recortar((caja.0, caja.1 + k * lado), d, caja) {
                l.linea(a.0, a.1, b.0, b.1, GRIS, 0.75);
            }
            k += 1.0;
        }
    }
    l.figura("bloc-isometrica", CATEGORIA, "Isométrica")
}

/// Hexagonos con la punta arriba, fila a fila: la linea quebrada de arriba
/// de cada fila y sus lados verticales.
fn hexagonal() -> FiguraGuardada {
    let mut l = hoja();
    let r = 16.0;
    let w = r * 3f32.sqrt();
    let (x0, y0) = (M - 6.0, Y0 + 4.0);
    let cols = ((W - 2.0 * x0) / w) as usize;
    let filas = ((H - y0 - 40.0) / (1.5 * r)) as usize;
    for f in 0..=filas {
        let desp = if f % 2 == 1 { w / 2.0 } else { 0.0 };
        let top = y0 + 1.5 * r * f as f32;
        // La quebrada de arriba de la fila (y de abajo de la anterior).
        let mut pts = Vec::new();
        for c in 0..=cols {
            let cx = x0 + desp + w * c as f32;
            pts.push((cx - w / 2.0, top + r / 2.0));
            pts.push((cx, top));
        }
        pts.push((x0 + desp + w * cols as f32 + w / 2.0, top + r / 2.0));
        l.poli(&pts, GRIS_FUERTE, 0.75, None);
        if f < filas {
            for c in 0..=cols + 1 {
                let x = x0 + desp + w * c as f32 - w / 2.0;
                l.linea(x, top + r / 2.0, x, top + 1.5 * r, GRIS_FUERTE, 0.75);
            }
        }
    }
    l.figura("bloc-hexagonal", CATEGORIA, "Hexagonal")
}

/// Cuadros para caligrafia china: con la cruz (tianzige) o ademas las
/// diagonales (mizige), a trazos.
fn cuadros_de_caligrafia(diagonales: bool) -> Lapiz {
    let mut l = hoja();
    let lado = 70.0;
    let cols = ((W - 2.0 * M) / lado) as usize;
    let x0 = (W - lado * cols as f32) / 2.0;
    let filas = ((H - Y0 - 30.0) / lado) as usize;
    let y0 = Y0 + 6.0;
    for f in 0..filas {
        for c in 0..cols {
            let (x, y) = (x0 + lado * c as f32, y0 + lado * f as f32);
            let (mx, my) = (x + lado / 2.0, y + lado / 2.0);
            l.linea_con(mx, y, mx, y + lado, GRIS, 0.75, EstiloTrazo::Discontinuo);
            l.linea_con(x, my, x + lado, my, GRIS, 0.75, EstiloTrazo::Discontinuo);
            if diagonales {
                l.linea_con(
                    x,
                    y,
                    x + lado,
                    y + lado,
                    GRIS,
                    0.75,
                    EstiloTrazo::Discontinuo,
                );
                l.linea_con(
                    x + lado,
                    y,
                    x,
                    y + lado,
                    GRIS,
                    0.75,
                    EstiloTrazo::Discontinuo,
                );
            }
        }
    }
    for c in 0..=cols {
        let x = x0 + lado * c as f32;
        l.linea(x, y0, x, y0 + lado * filas as f32, hex(0xc47b7b), 1.0);
    }
    for f in 0..=filas {
        let y = y0 + lado * f as f32;
        l.linea(x0, y, x0 + lado * cols as f32, y, hex(0xc47b7b), 1.0);
    }
    l
}

fn tianzige() -> FiguraGuardada {
    cuadros_de_caligrafia(false).figura("bloc-tianzige", CATEGORIA, "Tianzige (caligrafía)")
}

fn mizige() -> FiguraGuardada {
    cuadros_de_caligrafia(true).figura("bloc-mizige", CATEGORIA, "Mizige (caligrafía)")
}
