//! **Matematicas de secundaria**: ejes cartesianos con su cuadricula,
//! funciones (parabola, seno y coseno, recta con su pendiente), triangulos
//! con sus angulos, la circunferencia y la goniometrica, la recta numerica
//! y una tabla de valores para rellenar.

use super::*;
use crate::elemento::EstiloTrazo;

const CATEGORIA: &str = "Matemáticas";
const PREFIJO: &str = "ciencia-";
const ACENTO: ColorRgba = hex(0xc0692b);
const ACENTO_SUAVE: ColorRgba = hex(0xfbeee3);
/// Lo que mide una unidad en las graficas.
const U: f32 = 30.0;

pub(super) fn todas(v: &mut Vec<FiguraGuardada>) {
    let lista: [Dibujo; 10] = [
        ("ejes-cartesianos", "Ejes cartesianos", ejes_cartesianos),
        ("parabola", "Parábola", parabola),
        ("seno-coseno", "Funciones seno y coseno", seno_coseno),
        ("recta-pendiente", "Recta y pendiente", pendiente),
        (
            "triangulo-rectangulo",
            "Triángulo rectángulo",
            triangulo_rectangulo,
        ),
        (
            "triangulo-angulos",
            "Triángulo con sus ángulos",
            triangulo_angulos,
        ),
        (
            "circunferencia",
            "Circunferencia: radio, diámetro, cuerda y tangente",
            circunferencia,
        ),
        ("goniometrica", "Circunferencia goniométrica", goniometrica),
        ("recta-numerica", "Recta numérica", recta_numerica),
        ("tabla-valores", "Tabla de valores", tabla_valores),
    ];
    meter(v, PREFIJO, CATEGORIA, &lista);
}

/// **Un plano cartesiano** de `x0..x1` por `y0..y1` (en unidades) con su
/// cuadricula y los numeros; devuelve donde cae cada punto.
fn plano(
    l: &mut Lapiz,
    (x0, x1): (i32, i32),
    (y0, y1): (i32, i32),
) -> impl Fn(f32, f32) -> (f32, f32) + use<> {
    let ox = 20.0 - x0 as f32 * U;
    let oy = 20.0 + y1 as f32 * U;
    let a = move |x: f32, y: f32| (ox + x * U, oy - y * U);
    let (izq, arr) = a(x0 as f32, y1 as f32);
    let (der, aba) = a(x1 as f32, y0 as f32);
    cuadricula(l, izq, arr, der, aba, U);
    ejes(
        l,
        (ox, oy),
        (izq - 10.0, der + 16.0),
        (arr - 16.0, aba + 10.0),
        ("x", "y"),
    );
    for i in x0..=x1 {
        if i != 0 {
            let (x, y) = a(i as f32, 0.0);
            l.linea(x, y - 3.0, x, y + 3.0, TINTA, 1.0);
            txt(l, x, y + 12.0, &menos(i), 9.0, GRIS);
        }
    }
    for j in y0..=y1 {
        if j != 0 {
            let (x, y) = a(0.0, j as f32);
            l.linea(x - 3.0, y, x + 3.0, y, TINTA, 1.0);
            txt_a(l, x - 6.0, y, &menos(j), 9.0, Al::Der);
        }
    }
    let (x, y) = a(0.0, 0.0);
    txt_a(l, x - 6.0, y + 11.0, "O", 9.0, Al::Der);
    a
}

/// Un entero con el signo menos de verdad.
fn menos(i: i32) -> String {
    if i < 0 {
        format!("−{}", -i)
    } else {
        i.to_string()
    }
}

fn punto(l: &mut Lapiz, p: (f32, f32), s: &str, c: ColorRgba) {
    l.circulo(p.0, p.1, 4.0, Some(BLANCO), Some(c), 1.5);
    if !s.is_empty() {
        txt_a(l, p.0 + 8.0, p.1 - 10.0, s, 11.0, Al::Izq);
    }
}

fn ejes_cartesianos(l: &mut Lapiz) {
    let _ = plano(l, (-5, 5), (-5, 5));
    let (cx, cy) = (20.0 + 5.0 * U, 20.0 + 5.0 * U);
    for (dx, dy, s) in [
        (1.0, -1.0, "I"),
        (-1.0, -1.0, "II"),
        (-1.0, 1.0, "III"),
        (1.0, 1.0, "IV"),
    ] {
        txt_n(l, cx + dx * 3.5 * U, cy + dy * 3.5 * U, s, 14.0, ACENTO);
    }
}

fn parabola(l: &mut Lapiz) {
    let a = plano(l, (-3, 5), (-5, 6));
    let f = |x: f32| x * x - 2.0 * x - 3.0;
    let pts: Vec<(f32, f32)> = curva(f, -1.9, 3.9, 60)
        .into_iter()
        .map(|(x, y)| a(x, y))
        .collect();
    let (ex, ey0) = a(1.0, 6.0);
    let (_, ey1) = a(1.0, -5.0);
    l.linea_con(ex, ey0, ex, ey1, ACENTO, 1.2, EstiloTrazo::Discontinuo);
    l.poli(&pts, AZUL, 3.0, None);
    let v = a(1.0, -4.0);
    punto(l, v, "", ROJO);
    txt_a(l, v.0 + 10.0, v.1 + 12.0, "V(1, −4)", 11.0, Al::Izq);
    punto(l, a(-1.0, 0.0), "", ROJO);
    punto(l, a(3.0, 0.0), "", ROJO);
    punto(l, a(0.0, -3.0), "", VERDE);
    let (x, y) = a(1.6, 5.4);
    txt_a(l, x, y, "y = x² − 2x − 3", 12.0, Al::Izq);
    l.v.last_mut().unwrap().trazo = AZUL;
}

fn seno_coseno(l: &mut Lapiz) {
    let (ox, oy, ancho, amp) = (40.0, 110.0, 360.0, 70.0);
    cuadricula(l, ox, oy - amp, ox + ancho, oy + amp, ancho / 8.0);
    ejes(
        l,
        (ox, oy),
        (ox - 10.0, ox + ancho + 24.0),
        (oy - amp - 26.0, oy + amp + 14.0),
        ("x", "y"),
    );
    let paso = std::f32::consts::TAU / ancho;
    l.poli(
        &curva(|x| oy - amp * ((x - ox) * paso).sin(), ox, ox + ancho, 90),
        AZUL,
        3.0,
        None,
    );
    l.poli(
        &curva(|x| oy - amp * ((x - ox) * paso).cos(), ox, ox + ancho, 90),
        ROJO,
        3.0,
        None,
    );
    for (i, s) in ["π/2", "π", "3π/2", "2π"].iter().enumerate() {
        let x = ox + ancho / 4.0 * (i + 1) as f32;
        l.linea(x, oy - 3.0, x, oy + 3.0, TINTA, 1.0);
        txt(l, x + 10.0, oy + 12.0, s, 10.0, GRIS);
    }
    txt_a(l, ox - 6.0, oy - amp, "1", 10.0, Al::Der);
    txt_a(l, ox - 6.0, oy + amp, "−1", 10.0, Al::Der);
    l.linea(ox + 250.0, 12.0, ox + 274.0, 12.0, AZUL, 3.0);
    txt_a(l, ox + 280.0, 12.0, "sen x", 11.0, Al::Izq);
    l.linea(ox + 250.0, 28.0, ox + 274.0, 28.0, ROJO, 3.0);
    txt_a(l, ox + 280.0, 28.0, "cos x", 11.0, Al::Izq);
}

fn pendiente(l: &mut Lapiz) {
    let a = plano(l, (-1, 6), (-1, 5));
    let (p, q) = (a(1.0, 1.0), a(5.0, 4.0));
    let (ini, fin) = (a(-0.6, -0.2), a(6.2, 4.9));
    l.linea(ini.0, ini.1, fin.0, fin.1, AZUL, 3.0);
    let r = (q.0, p.1);
    l.linea_con(p.0, p.1, r.0, r.1, ROJO, 1.5, EstiloTrazo::Discontinuo);
    l.linea_con(r.0, r.1, q.0, q.1, VERDE, 1.5, EstiloTrazo::Discontinuo);
    txt(l, (p.0 + r.0) / 2.0, p.1 + 12.0, "Δx", 12.0, ROJO);
    txt_a(l, r.0 + 6.0, (r.1 + q.1) / 2.0, "Δy", 12.0, Al::Izq);
    l.v.last_mut().unwrap().trazo = VERDE;
    punto(l, p, "A", TINTA);
    punto(l, q, "B", TINTA);
    let (x, y) = a(0.2, 4.4);
    txt_a(l, x, y, "m = Δy / Δx", 12.0, Al::Izq);
}

fn triangulo_rectangulo(l: &mut Lapiz) {
    let (a, b, c) = ((40.0, 220.0), (320.0, 220.0), (320.0, 50.0));
    forma(l, &[a, b, c], ACENTO, Some(ACENTO_SUAVE), G);
    recto(l, b, a, c, 16.0);
    angulo(l, a, b, c, 50.0, "α", AZUL);
    angulo(l, c, a, b, 36.0, "β", AZUL);
    txt(l, 180.0, 238.0, "cateto b", 12.0, TINTA);
    txt_a(l, 328.0, 135.0, "cateto a", 12.0, Al::Izq);
    txt(l, 162.0, 118.0, "hipotenusa c", 12.0, TINTA);
    l.v.last_mut().unwrap().extras.negrita = true;
    txt_n(l, 180.0, 264.0, "a² + b² = c²", 13.0, ACENTO);
    txt(
        l,
        180.0,
        284.0,
        "sen α = a/c · cos α = b/c · tg α = a/b",
        11.0,
        TINTA,
    );
}

fn triangulo_angulos(l: &mut Lapiz) {
    let (a, b, c) = ((40.0, 210.0), (330.0, 210.0), (120.0, 40.0));
    forma(l, &[a, b, c], ACENTO, Some(ACENTO_SUAVE), G);
    angulo(l, a, b, c, 34.0, "α", AZUL);
    angulo(l, b, c, a, 40.0, "β", VERDE);
    angulo(l, c, a, b, 30.0, "γ", ROJO);
    txt_n(l, a.0 - 10.0, a.1 + 10.0, "A", 13.0, TINTA);
    txt_n(l, b.0 + 10.0, b.1 + 10.0, "B", 13.0, TINTA);
    txt_n(l, c.0, c.1 - 14.0, "C", 13.0, TINTA);
    txt(l, 185.0, 224.0, "c", 12.0, TINTA);
    txt(l, 236.0, 116.0, "a", 12.0, TINTA);
    txt(l, 68.0, 120.0, "b", 12.0, TINTA);
    txt_n(l, 185.0, 252.0, "α + β + γ = 180°", 13.0, ACENTO);
}

fn circunferencia(l: &mut Lapiz) {
    let (cx, cy, r) = (150.0, 150.0, 110.0);
    l.circulo(cx, cy, r, Some(ACENTO), Some(ACENTO_SUAVE), G);
    l.circulo(cx, cy, 3.5, None, Some(TINTA), 1.0);
    txt_a(l, cx - 6.0, cy + 10.0, "O", 11.0, Al::Der);
    let en = |g: f32| (cx + r * g.to_radians().cos(), cy + r * g.to_radians().sin());
    // Radio, diametro, cuerda y tangente.
    let p = en(-40.0);
    l.linea(cx, cy, p.0, p.1, AZUL, G);
    txt(
        l,
        (cx + p.0) / 2.0 - 4.0,
        (cy + p.1) / 2.0 - 10.0,
        "r",
        13.0,
        AZUL,
    );
    let (d0, d1) = (en(180.0), en(0.0));
    l.linea_con(d0.0, d0.1, d1.0, d1.1, ROJO, 1.5, EstiloTrazo::Discontinuo);
    txt(l, cx + 56.0, cy + 12.0, "d = 2r", 11.0, ROJO);
    let (c0, c1) = (en(120.0), en(50.0));
    l.linea(c0.0, c0.1, c1.0, c1.1, VERDE, G);
    txt(
        l,
        (c0.0 + c1.0) / 2.0,
        (c0.1 + c1.1) / 2.0 + 14.0,
        "cuerda",
        11.0,
        VERDE,
    );
    let t = en(-120.0);
    let dir = (
        (-120.0f32 + 90.0).to_radians().cos(),
        (-120.0f32 + 90.0).to_radians().sin(),
    );
    l.linea(
        t.0 - dir.0 * 80.0,
        t.1 - dir.1 * 80.0,
        t.0 + dir.0 * 80.0,
        t.1 + dir.1 * 80.0,
        MORADO,
        G,
    );
    l.circulo(t.0, t.1, 3.5, None, Some(MORADO), 1.0);
    txt(
        l,
        t.0 - dir.0 * 70.0 - 10.0,
        t.1 - dir.1 * 70.0 + 10.0,
        "tangente",
        11.0,
        MORADO,
    );
    txt(l, cx, cy + r + 22.0, "L = 2πr · A = πr²", 12.0, TINTA);
}

fn goniometrica(l: &mut Lapiz) {
    let (cx, cy, r) = (170.0, 170.0, 120.0);
    ejes(
        l,
        (cx, cy),
        (cx - r - 24.0, cx + r + 30.0),
        (cy - r - 30.0, cy + r + 24.0),
        ("x", "y"),
    );
    l.circulo(cx, cy, r, Some(ACENTO), None, G);
    let t: f32 = 40.0;
    let p = (cx + r * t.to_radians().cos(), cy - r * t.to_radians().sin());
    l.linea(cx, cy, p.0, p.1, TINTA, G);
    l.linea(cx, cy, p.0, cy, VERDE, 3.5);
    l.linea(p.0, cy, p.0, p.1, ROJO, 3.5);
    punto(l, p, "P", TINTA);
    angulo(l, (cx, cy), (cx + 50.0, cy), p, 34.0, "θ", AZUL);
    txt(l, (cx + p.0) / 2.0, cy + 14.0, "cos θ", 11.0, VERDE);
    txt_a(l, p.0 + 10.0, (cy + p.1) / 2.0, "sen θ", 11.0, Al::Izq);
    l.v.last_mut().unwrap().trazo = ROJO;
    txt(
        l,
        (cx + p.0) / 2.0 - 10.0,
        (cy + p.1) / 2.0 - 10.0,
        "1",
        11.0,
        TINTA,
    );
    txt_a(l, cx + r + 4.0, cy + 12.0, "1", 10.0, Al::Izq);
    txt_a(l, cx - r - 4.0, cy + 12.0, "−1", 10.0, Al::Der);
    for (dx, dy, s) in [
        (1.0, -1.0, "I"),
        (-1.0, -1.0, "II"),
        (-1.0, 1.0, "III"),
        (1.0, 1.0, "IV"),
    ] {
        txt_n(l, cx + dx * 1.0 * r, cy + dy * 1.0 * r, s, 13.0, ACENTO);
    }
}

fn recta_numerica(l: &mut Lapiz) {
    let (x0, y, paso) = (20.0, 40.0, 26.0);
    l.flecha(&[(x0 + paso * 10.0, y), (x0 - 4.0, y)], TINTA, G, false);
    l.flecha(
        &[(x0 + paso * 10.0, y), (x0 + paso * 20.0 + 24.0, y)],
        TINTA,
        G,
        false,
    );
    for i in -10..=10 {
        let x = x0 + 12.0 + paso * (i + 10) as f32;
        let alto = if i % 5 == 0 { 8.0 } else { 5.0 };
        l.linea(x, y - alto, x, y + alto, TINTA, 1.5);
        if i == 0 {
            txt_n(l, x, y + 20.0, "0", 12.0, ACENTO);
        } else {
            txt(l, x, y + 20.0, &menos(i), 10.0, TINTA);
        }
    }
}

fn tabla_valores(l: &mut Lapiz) {
    let (x, y, w, fh, filas) = (0.0, 0.0, 100.0, 30.0, 8);
    l.rect(x, y, 2.0 * w, fh, None, Some(ACENTO_SUAVE), 1.0, false);
    txt_n(l, x + w / 2.0, y + fh / 2.0, "x", 13.0, ACENTO);
    txt_n(l, x + 1.5 * w, y + fh / 2.0, "y = f(x)", 13.0, ACENTO);
    for i in 1..=filas {
        let yy = y + fh * i as f32;
        l.linea(x, yy, x + 2.0 * w, yy, GRIS_CLARO, 1.0);
    }
    l.linea(x + w, y, x + w, y + fh * (filas + 1) as f32, GRIS, 1.2);
    l.linea(x, y + fh, x + 2.0 * w, y + fh, ACENTO, 1.5);
    l.rect(
        x,
        y,
        2.0 * w,
        fh * (filas + 1) as f32,
        Some(GRIS),
        None,
        1.5,
        false,
    );
}
