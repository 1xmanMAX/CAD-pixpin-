//! **Fisica de secundaria**: graficas del movimiento, fuerzas (cuerpo
//! libre, plano inclinado, poleas, palancas), circuitos y sus simbolos,
//! optica con sus rayos, ondas y vectores. Cada fuerza o rayo con su
//! color, para que se distingan en la pizarra.

use super::*;
use crate::elemento::EstiloTrazo;

const CATEGORIA: &str = "Física";
const PREFIJO: &str = "ciencia-";
const MADERA: ColorRgba = hex(0xd9b48a);

pub(super) fn todas(v: &mut Vec<FiguraGuardada>) {
    let lista: [Dibujo; 29] = [
        ("grafica-x-t", "Gráfica posición-tiempo (x-t)", grafica_x_t),
        ("grafica-v-t", "Gráfica velocidad-tiempo (v-t)", grafica_v_t),
        (
            "grafica-a-t",
            "Gráfica aceleración-tiempo (a-t)",
            grafica_a_t,
        ),
        ("cuerpo-libre", "Diagrama de cuerpo libre", cuerpo_libre),
        ("plano-inclinado", "Plano inclinado", plano_inclinado),
        ("polea-simple", "Polea simple", polea_simple),
        ("polipasto", "Polea móvil (polipasto)", polipasto),
        ("palanca-1", "Palanca de primer grado", |l| palanca(l, 1)),
        ("palanca-2", "Palanca de segundo grado", |l| palanca(l, 2)),
        ("palanca-3", "Palanca de tercer grado", |l| palanca(l, 3)),
        ("circuito-serie", "Circuito en serie", circuito_serie),
        (
            "circuito-paralelo",
            "Circuito en paralelo",
            circuito_paralelo,
        ),
        ("simbolo-resistencia", "Símbolo: resistencia", |l| {
            solo(l, resistencia, "resistencia")
        }),
        ("simbolo-pila", "Símbolo: pila", |l| solo(l, pila, "pila")),
        ("simbolo-bombilla", "Símbolo: bombilla", |l| {
            solo(l, bombilla, "bombilla")
        }),
        ("simbolo-interruptor", "Símbolo: interruptor", |l| {
            solo(l, interruptor, "interruptor")
        }),
        ("simbolo-amperimetro", "Símbolo: amperímetro", |l| {
            solo(l, amperimetro, "amperímetro")
        }),
        ("simbolo-voltimetro", "Símbolo: voltímetro", |l| {
            solo(l, voltimetro, "voltímetro")
        }),
        (
            "lente-convergente",
            "Lente convergente con rayos",
            lente_convergente,
        ),
        (
            "lente-divergente",
            "Lente divergente con rayos",
            lente_divergente,
        ),
        ("espejo-concavo", "Espejo cóncavo con rayos", espejo_concavo),
        ("reflexion", "Reflexión en espejo plano", reflexion),
        ("refraccion", "Refracción (ley de Snell)", refraccion),
        ("onda-transversal", "Onda transversal", onda_transversal),
        ("onda-longitudinal", "Onda longitudinal", onda_longitudinal),
        ("suma-vectores", "Suma de vectores", suma_vectores),
        (
            "componentes-vector",
            "Componentes de un vector",
            componentes,
        ),
        ("tiro-parabolico", "Tiro parabólico", tiro_parabolico),
        ("pendulo", "Péndulo simple", pendulo),
    ];
    meter(v, PREFIJO, CATEGORIA, &lista);
}

// ------------------------------------------------------------- graficas

/// Los ejes de una grafica del movimiento con su cuadricula.
fn grafica(l: &mut Lapiz, o: (f32, f32), arriba: f32, abajo: f32, y: &str) {
    cuadricula(l, o.0, arriba + 10.0, o.0 + 300.0, abajo, 30.0);
    ejes(l, o, (o.0, o.0 + 320.0), (arriba, abajo), ("t (s)", y));
}

fn puntos(l: &mut Lapiz, pts: &[(f32, f32)], c: ColorRgba) {
    l.poli(pts, c, 3.0, None);
    for p in pts {
        l.circulo(p.0, p.1, 3.5, None, Some(c), 1.0);
    }
}

fn grafica_x_t(l: &mut Lapiz) {
    grafica(l, (40.0, 220.0), 0.0, 220.0, "x (m)");
    puntos(
        l,
        &[
            (40.0, 190.0),
            (130.0, 100.0),
            (220.0, 100.0),
            (310.0, 160.0),
        ],
        AZUL,
    );
    txt(l, 70.0, 128.0, "v > 0", 11.0, AZUL);
    txt(l, 175.0, 86.0, "reposo", 11.0, AZUL);
    txt(l, 290.0, 118.0, "v < 0", 11.0, AZUL);
}

fn grafica_v_t(l: &mut Lapiz) {
    let pts = [(40.0, 190.0), (130.0, 70.0), (250.0, 70.0), (310.0, 220.0)];
    let mut area = pts.to_vec();
    area.push((40.0, 220.0));
    forma(l, &area, AZUL_SUAVE, Some(AZUL_SUAVE), 1.0);
    grafica(l, (40.0, 220.0), 0.0, 220.0, "v (m/s)");
    puntos(l, &pts, AZUL);
    txt(l, 185.0, 150.0, "área = desplazamiento", 11.0, AZUL);
    txt(l, 70.0, 108.0, "a > 0", 11.0, ROJO);
    txt(l, 190.0, 56.0, "a = 0", 11.0, ROJO);
    txt(l, 302.0, 120.0, "a < 0", 11.0, ROJO);
}

fn grafica_a_t(l: &mut Lapiz) {
    let o = (40.0, 130.0);
    grafica(l, o, 0.0, 240.0, "a (m/s²)");
    l.poli(&[(40.0, 70.0), (130.0, 70.0)], ROJO, 3.0, None);
    l.poli(&[(130.0, 130.0), (230.0, 130.0)], ROJO, 3.0, None);
    l.poli(&[(230.0, 190.0), (310.0, 190.0)], ROJO, 3.0, None);
    l.linea_con(
        130.0,
        70.0,
        130.0,
        130.0,
        ROJO,
        1.2,
        EstiloTrazo::Discontinuo,
    );
    l.linea_con(
        230.0,
        130.0,
        230.0,
        190.0,
        ROJO,
        1.2,
        EstiloTrazo::Discontinuo,
    );
    txt(l, 85.0, 56.0, "acelera", 11.0, ROJO);
    txt(l, 180.0, 116.0, "velocidad constante", 11.0, ROJO);
    txt(l, 270.0, 206.0, "frena", 11.0, ROJO);
}

// --------------------------------------------------------------- fuerzas

fn cuerpo_libre(l: &mut Lapiz) {
    l.linea_con(
        20.0,
        165.0,
        300.0,
        165.0,
        GRIS,
        1.2,
        EstiloTrazo::Discontinuo,
    );
    l.rect(115.0, 95.0, 90.0, 70.0, Some(TINTA), Some(CLARO), G, true);
    txt_n(l, 160.0, 130.0, "m", 16.0, TINTA);
    vector(l, (160.0, 95.0), (160.0, 20.0), VERDE, "N");
    vector(l, (160.0, 165.0), (160.0, 250.0), ROJO, "P = m·g");
    vector(l, (205.0, 130.0), (300.0, 130.0), AZUL, "F");
    vector(l, (115.0, 145.0), (40.0, 145.0), NARANJA, "Fr");
}

fn plano_inclinado(l: &mut Lapiz) {
    let theta: f32 = 30.0;
    let (a, b) = ((20.0, 240.0), (340.0, 240.0));
    let alto = 320.0 * theta.to_radians().tan();
    let c = (340.0, 240.0 - alto);
    forma(l, &[a, b, c], TINTA, Some(CLARO), G);
    angulo(l, a, b, c, 52.0, "θ", TINTA);
    recto(l, b, a, c, 12.0);
    // El bloque, apoyado en la rampa a algo mas de la mitad.
    let t = 0.55;
    let p = (a.0 + (c.0 - a.0) * t, a.1 + (c.1 - a.1) * t);
    let bloque = girar(
        &[
            (p.0 - 32.0, p.1),
            (p.0 + 32.0, p.1),
            (p.0 + 32.0, p.1 - 44.0),
            (p.0 - 32.0, p.1 - 44.0),
        ],
        p,
        -theta,
    );
    forma(l, &bloque, TINTA, Some(hex(0xf6d9a8)), G);
    let centro = girar(&[(p.0, p.1 - 22.0)], p, -theta)[0];
    let (s, co) = theta.to_radians().sin_cos();
    let peso = 96.0;
    vector(l, centro, (centro.0, centro.1 + peso), ROJO, "P");
    vector(
        l,
        centro,
        (centro.0 - s * 82.0, centro.1 - co * 82.0),
        VERDE,
        "N",
    );
    // Las componentes del peso, a trazos.
    let px = (centro.0 - co * peso * s, centro.1 + s * peso * s);
    let py = (centro.0 + s * peso * co, centro.1 + co * peso * co);
    l.flecha(&[centro, px], ROJO, 1.5, false);
    l.v.last_mut().unwrap().estilo = EstiloTrazo::Discontinuo;
    l.flecha(&[centro, py], ROJO, 1.5, false);
    l.v.last_mut().unwrap().estilo = EstiloTrazo::Discontinuo;
    txt(l, px.0 - 14.0, px.1 + 10.0, "Px", 11.0, ROJO);
    txt(l, py.0 + 16.0, py.1, "Py", 11.0, ROJO);
    let base = girar(&[(p.0 + 30.0, p.1 - 4.0)], p, -theta)[0];
    vector(
        l,
        base,
        (base.0 + co * 64.0, base.1 - s * 64.0),
        NARANJA,
        "Fr",
    );
}

/// Una polea: la rueda, su eje y el soporte desde `techo`.
fn polea(l: &mut Lapiz, cx: f32, cy: f32, r: f32) {
    l.circulo(cx, cy, r, Some(TINTA), Some(CLARO), G);
    l.circulo(cx, cy, r - 7.0, Some(GRIS), None, 1.0);
    l.circulo(cx, cy, 4.0, None, Some(TINTA), 1.0);
}

fn pesa(l: &mut Lapiz, x: f32, y: f32, w: f32, h: f32, s: &str) {
    l.rect(x, y, w, h, Some(TINTA), Some(hex(0xc9d1dc)), G, true);
    txt_n(l, x + w / 2.0, y + h / 2.0, s, 14.0, TINTA);
}

fn polea_simple(l: &mut Lapiz) {
    rayado(l, 50.0, 210.0, 12.0, true);
    l.linea(130.0, 12.0, 130.0, 62.0, TINTA, 3.0);
    l.linea(100.0, 62.0, 100.0, 180.0, TINTA, 1.8);
    l.linea(160.0, 62.0, 160.0, 140.0, TINTA, 1.8);
    l.arco(130.0, 62.0, 30.0, 180.0, 360.0, TINTA, 1.8);
    polea(l, 130.0, 62.0, 30.0);
    pesa(l, 76.0, 180.0, 48.0, 46.0, "m₁");
    pesa(l, 140.0, 140.0, 40.0, 36.0, "m₂");
    vector(l, (100.0, 178.0), (100.0, 128.0), VERDE, "T");
    vector(l, (160.0, 138.0), (160.0, 98.0), VERDE, "T");
    vector(l, (100.0, 226.0), (100.0, 270.0), ROJO, "P₁");
    vector(l, (160.0, 176.0), (160.0, 212.0), ROJO, "P₂");
}

fn polipasto(l: &mut Lapiz) {
    rayado(l, 40.0, 220.0, 12.0, true);
    l.linea(100.0, 12.0, 100.0, 60.0, TINTA, 3.0);
    // La cuerda: del techo, por debajo de la movil, por encima de la fija y
    // hasta la mano.
    l.linea(176.0, 12.0, 176.0, 170.0, TINTA, 1.8);
    l.arco(150.0, 170.0, 26.0, 0.0, 180.0, TINTA, 1.8);
    l.linea(124.0, 170.0, 124.0, 60.0, TINTA, 1.8);
    l.arco(100.0, 60.0, 24.0, 180.0, 360.0, TINTA, 1.8);
    l.linea(76.0, 60.0, 76.0, 190.0, TINTA, 1.8);
    polea(l, 100.0, 60.0, 24.0);
    polea(l, 150.0, 170.0, 26.0);
    l.linea(150.0, 170.0, 150.0, 214.0, TINTA, 2.5);
    pesa(l, 122.0, 214.0, 56.0, 46.0, "P");
    vector(l, (76.0, 190.0), (76.0, 250.0), AZUL, "F");
    txt_n(l, 60.0, 284.0, "F = P / 2", 13.0, AZUL);
}

/// Una palanca de primer, segundo o tercer grado: el fulcro, la potencia
/// (azul) y la resistencia (roja), con sus brazos.
fn palanca(l: &mut Lapiz, grado: u8) {
    let (f, p, r) = match grado {
        1 => (190.0, 330.0, 50.0),
        2 => (30.0, 330.0, 180.0),
        _ => (30.0, 180.0, 330.0),
    };
    l.rect(
        16.0,
        94.0,
        328.0,
        12.0,
        Some(hex(0x9c7448)),
        Some(MADERA),
        1.5,
        true,
    );
    forma(
        l,
        &[(f, 106.0), (f - 20.0, 144.0), (f + 20.0, 144.0)],
        TINTA,
        Some(hex(0xaab4c3)),
        G,
    );
    rayado(l, f - 40.0, f + 40.0, 144.0, false);
    txt(l, f, 166.0, "F (fulcro)", 11.0, TINTA);
    pesa(l, r - 22.0, 56.0, 44.0, 38.0, "R");
    if grado == 1 {
        vector(l, (p, 24.0), (p, 92.0), AZUL, "");
        txt_n(l, p, 14.0, "P", 13.0, AZUL);
    } else {
        vector(l, (p, 176.0), (p, 108.0), AZUL, "");
        txt_n(l, p, 188.0, "P", 13.0, AZUL);
    }
    let y = 212.0;
    let (a, b) = if p < f { (p, f) } else { (f, p) };
    l.flecha(&[(a, y), (b, y)], AZUL, 1.2, true);
    txt(l, (a + b) / 2.0, y - 9.0, "bp", 11.0, AZUL);
    let (a, b) = if r < f { (r, f) } else { (f, r) };
    l.flecha(&[(a, y + 22.0), (b, y + 22.0)], ROJO, 1.2, true);
    txt(l, (a + b) / 2.0, y + 13.0, "br", 11.0, ROJO);
    txt_n(l, 180.0, 262.0, "P · bp = R · br", 13.0, TINTA);
}

// ------------------------------------------------------------ circuitos

/// Las piezas de un circuito: miden 60 a lo largo y van centradas en
/// `(cx, cy)`, tumbadas o de pie.
type Pieza = fn(&mut Lapiz, f32, f32, bool);

/// Un punto de la pieza, girado si va de pie.
fn en(cx: f32, cy: f32, de_pie: bool, dx: f32, dy: f32) -> (f32, f32) {
    if de_pie {
        (cx + dy, cy + dx)
    } else {
        (cx + dx, cy + dy)
    }
}

fn patas(l: &mut Lapiz, cx: f32, cy: f32, de_pie: bool, hueco: f32) {
    let a = en(cx, cy, de_pie, -30.0, 0.0);
    let b = en(cx, cy, de_pie, -hueco, 0.0);
    l.linea(a.0, a.1, b.0, b.1, TINTA, G);
    let a = en(cx, cy, de_pie, hueco, 0.0);
    let b = en(cx, cy, de_pie, 30.0, 0.0);
    l.linea(a.0, a.1, b.0, b.1, TINTA, G);
}

fn resistencia(l: &mut Lapiz, cx: f32, cy: f32, de_pie: bool) {
    patas(l, cx, cy, de_pie, 20.0);
    let (a, b) = (
        en(cx, cy, de_pie, -20.0, -8.0),
        en(cx, cy, de_pie, 20.0, 8.0),
    );
    l.rect(
        a.0.min(b.0),
        a.1.min(b.1),
        (b.0 - a.0).abs(),
        (b.1 - a.1).abs(),
        Some(TINTA),
        Some(BLANCO),
        G,
        false,
    );
}

fn pila(l: &mut Lapiz, cx: f32, cy: f32, de_pie: bool) {
    patas(l, cx, cy, de_pie, 5.0);
    let (a, b) = (
        en(cx, cy, de_pie, -5.0, -18.0),
        en(cx, cy, de_pie, -5.0, 18.0),
    );
    l.linea(a.0, a.1, b.0, b.1, TINTA, G);
    let (a, b) = (en(cx, cy, de_pie, 5.0, -9.0), en(cx, cy, de_pie, 5.0, 9.0));
    l.linea(a.0, a.1, b.0, b.1, TINTA, 5.0);
    let mas = en(cx, cy, de_pie, -15.0, -20.0);
    txt_n(l, mas.0, mas.1, "+", 13.0, ROJO);
    let menos = en(cx, cy, de_pie, 15.0, -20.0);
    txt_n(l, menos.0, menos.1, "−", 13.0, AZUL);
}

fn bombilla(l: &mut Lapiz, cx: f32, cy: f32, de_pie: bool) {
    patas(l, cx, cy, de_pie, 15.0);
    l.circulo(cx, cy, 15.0, Some(TINTA), Some(hex(0xfff4c2)), G);
    l.linea(cx - 10.6, cy - 10.6, cx + 10.6, cy + 10.6, TINTA, 1.5);
    l.linea(cx - 10.6, cy + 10.6, cx + 10.6, cy - 10.6, TINTA, 1.5);
}

fn interruptor(l: &mut Lapiz, cx: f32, cy: f32, de_pie: bool) {
    patas(l, cx, cy, de_pie, 16.0);
    let (a, b) = (
        en(cx, cy, de_pie, -16.0, 0.0),
        en(cx, cy, de_pie, 16.0, 0.0),
    );
    let c = en(cx, cy, de_pie, 13.0, -16.0);
    l.linea(a.0, a.1, c.0, c.1, TINTA, G);
    l.circulo(a.0, a.1, 3.0, Some(TINTA), Some(BLANCO), 1.5);
    l.circulo(b.0, b.1, 3.0, Some(TINTA), Some(BLANCO), 1.5);
}

fn medidor(l: &mut Lapiz, cx: f32, cy: f32, de_pie: bool, letra: &str) {
    patas(l, cx, cy, de_pie, 15.0);
    l.circulo(cx, cy, 15.0, Some(TINTA), Some(BLANCO), G);
    txt_n(l, cx, cy + 1.0, letra, 14.0, TINTA);
}

fn amperimetro(l: &mut Lapiz, cx: f32, cy: f32, de_pie: bool) {
    medidor(l, cx, cy, de_pie, "A");
}

fn voltimetro(l: &mut Lapiz, cx: f32, cy: f32, de_pie: bool) {
    medidor(l, cx, cy, de_pie, "V");
}

/// Un simbolo suelto, tumbado, con su nombre debajo.
fn solo(l: &mut Lapiz, pieza: Pieza, nombre: &str) {
    pieza(l, 40.0, 30.0, false);
    txt(l, 40.0, 64.0, nombre, 10.0, GRIS);
}

/// Un nudo de cables.
fn nudo(l: &mut Lapiz, x: f32, y: f32) {
    l.circulo(x, y, 3.5, None, Some(TINTA), 1.0);
}

fn cable(l: &mut Lapiz, pts: &[(f32, f32)]) {
    l.poli(pts, TINTA, G, None);
}

fn circuito_serie(l: &mut Lapiz) {
    pila(l, 40.0, 130.0, true);
    interruptor(l, 110.0, 40.0, false);
    resistencia(l, 230.0, 40.0, false);
    bombilla(l, 320.0, 130.0, true);
    amperimetro(l, 180.0, 220.0, false);
    cable(l, &[(40.0, 100.0), (40.0, 40.0), (80.0, 40.0)]);
    cable(l, &[(140.0, 40.0), (200.0, 40.0)]);
    cable(l, &[(260.0, 40.0), (320.0, 40.0), (320.0, 100.0)]);
    cable(l, &[(320.0, 160.0), (320.0, 220.0), (210.0, 220.0)]);
    cable(l, &[(150.0, 220.0), (40.0, 220.0), (40.0, 160.0)]);
    txt(l, 230.0, 18.0, "R", 12.0, TINTA);
    txt(l, 4.0, 130.0, "ε", 13.0, TINTA);
    l.flecha(&[(160.0, 26.0), (196.0, 26.0)], ROJO, 1.5, false);
    txt(l, 178.0, 14.0, "I", 12.0, ROJO);
}

fn circuito_paralelo(l: &mut Lapiz) {
    pila(l, 40.0, 130.0, true);
    interruptor(l, 95.0, 40.0, false);
    bombilla(l, 160.0, 130.0, true);
    bombilla(l, 260.0, 130.0, true);
    voltimetro(l, 340.0, 130.0, true);
    cable(l, &[(40.0, 100.0), (40.0, 40.0), (65.0, 40.0)]);
    cable(l, &[(125.0, 40.0), (260.0, 40.0), (260.0, 100.0)]);
    cable(l, &[(160.0, 40.0), (160.0, 100.0)]);
    cable(
        l,
        &[(40.0, 160.0), (40.0, 220.0), (260.0, 220.0), (260.0, 160.0)],
    );
    cable(l, &[(160.0, 160.0), (160.0, 220.0)]);
    cable(l, &[(260.0, 70.0), (340.0, 70.0), (340.0, 100.0)]);
    cable(l, &[(340.0, 160.0), (340.0, 190.0), (260.0, 190.0)]);
    for (x, y) in [(160.0, 40.0), (160.0, 220.0), (260.0, 70.0), (260.0, 190.0)] {
        nudo(l, x, y);
    }
    txt(l, 132.0, 130.0, "L₁", 12.0, TINTA);
    txt(l, 232.0, 130.0, "L₂", 12.0, TINTA);
}

// ----------------------------------------------------------------- optica

/// El eje optico, a raya y punto.
fn eje_optico(l: &mut Lapiz, x0: f32, x1: f32, y: f32) {
    l.linea_con(x0, y, x1, y, GRIS, 1.2, EstiloTrazo::RayaPunto);
}

/// Un foco marcado en el eje.
fn foco(l: &mut Lapiz, x: f32, y: f32, s: &str) {
    l.circulo(x, y, 3.5, None, Some(TINTA), 1.0);
    txt(l, x, y + 16.0, s, 11.0, TINTA);
}

/// Un objeto (o imagen) como flecha de pie en `x`.
fn objeto(l: &mut Lapiz, x: f32, eje: f32, punta: f32, c: ColorRgba, a_trazos: bool) {
    l.flecha(&[(x, eje), (x, punta)], c, 3.0, false);
    if a_trazos {
        l.v.last_mut().unwrap().estilo = EstiloTrazo::Discontinuo;
    }
}

/// Un rayo con su flechita en medio del primer tramo.
fn rayo(l: &mut Lapiz, pts: &[(f32, f32)], c: ColorRgba) {
    l.poli(pts, c, 1.6, None);
    if let [a, b, ..] = pts {
        let m = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let n = (dx * dx + dy * dy).sqrt().max(1.0);
        l.flecha(
            &[
                (m.0 - dx / n * 6.0, m.1 - dy / n * 6.0),
                (m.0 + dx / n * 6.0, m.1 + dy / n * 6.0),
            ],
            c,
            1.6,
            false,
        );
    }
}

/// Donde corta en `x` la recta que pasa por `a` y `b`.
fn en_x(a: (f32, f32), b: (f32, f32), x: f32) -> (f32, f32) {
    (x, a.1 + (b.1 - a.1) * (x - a.0) / (b.0 - a.0))
}

fn lente_convergente(l: &mut Lapiz) {
    let (cx, eje, f) = (220.0, 130.0, 80.0);
    eje_optico(l, 0.0, 440.0, eje);
    let mut lente = bezier((cx, 40.0), (cx + 26.0, eje), (cx, 220.0), 12);
    lente.extend(bezier((cx, 220.0), (cx - 26.0, eje), (cx, 40.0), 12));
    forma(l, &lente, AZUL, Some(AZUL_SUAVE), 1.5);
    for (x, s) in [
        (cx - f, "F"),
        (cx + f, "F'"),
        (cx - 2.0 * f, "2F"),
        (cx + 2.0 * f, "2F'"),
    ] {
        foco(l, x, eje, s);
    }
    // Objeto mas alla de 2F: imagen real, invertida y menor... casi igual.
    let (xo, ho) = (cx - 150.0, 60.0);
    let di = 1.0 / (1.0 / f - 1.0 / 150.0);
    let hi = -ho * di / 150.0;
    let (xi, pi) = (cx + di, eje - hi);
    let punta = (xo, eje - ho);
    rayo(
        l,
        &[
            punta,
            (cx, eje - ho),
            en_x((cx, eje - ho), (cx + f, eje), xi + 20.0),
        ],
        ROJO,
    );
    rayo(l, &[punta, en_x(punta, (cx, eje), xi + 20.0)], VERDE);
    let corte = en_x(punta, (cx - f, eje), cx);
    rayo(l, &[punta, corte, (xi + 20.0, corte.1)], NARANJA);
    objeto(l, xo, eje, eje - ho, TINTA, false);
    objeto(l, xi, eje, pi, MORADO, false);
    txt(l, xo, eje - ho - 14.0, "objeto", 11.0, TINTA);
    txt(l, xi, pi + 14.0, "imagen real", 11.0, MORADO);
}

fn lente_divergente(l: &mut Lapiz) {
    let (cx, eje, f) = (220.0, 130.0, 80.0);
    eje_optico(l, 0.0, 400.0, eje);
    let mut lente = vec![(cx - 16.0, 40.0), (cx + 16.0, 40.0)];
    lente.extend(bezier(
        (cx + 16.0, 40.0),
        (cx - 2.0, eje),
        (cx + 16.0, 220.0),
        12,
    ));
    lente.push((cx - 16.0, 220.0));
    lente.extend(bezier(
        (cx - 16.0, 220.0),
        (cx + 2.0, eje),
        (cx - 16.0, 40.0),
        12,
    ));
    forma(l, &lente, AZUL, Some(AZUL_SUAVE), 1.5);
    foco(l, cx - f, eje, "F");
    foco(l, cx + f, eje, "F'");
    let (xo, ho, d) = (cx - 130.0, 56.0, 130.0);
    let di = 1.0 / (1.0 / f + 1.0 / d);
    let (xi, pi) = (cx - di, eje - ho * di / d);
    let punta = (xo, eje - ho);
    let salida = (cx, eje - ho);
    let lejos = (cx + 160.0, salida.1 + (salida.1 - eje) * 160.0 / f);
    rayo(l, &[punta, salida, lejos], ROJO);
    l.linea_con(
        cx - f,
        eje,
        salida.0,
        salida.1,
        ROJO,
        1.0,
        EstiloTrazo::Discontinuo,
    );
    rayo(l, &[punta, en_x(punta, (cx, eje), cx + 160.0)], VERDE);
    objeto(l, xo, eje, eje - ho, TINTA, false);
    objeto(l, xi, eje, pi, MORADO, true);
    txt(l, xo, eje - ho - 14.0, "objeto", 11.0, TINTA);
    txt(l, xi - 30.0, pi - 14.0, "imagen virtual", 11.0, MORADO);
}

fn espejo_concavo(l: &mut Lapiz) {
    let (c, eje, r) = (200.0, 140.0, 200.0);
    eje_optico(l, 20.0, 420.0, eje);
    // El espejo y su rayado por detras.
    l.arco(c, eje, r, -32.0, 32.0, AZUL, 4.0);
    for i in 0..=10 {
        let a = (-30.0 + 6.0 * i as f32).to_radians();
        let (x, y) = (c + r * a.cos(), eje + r * a.sin());
        l.linea(x + 2.0, y, x + 12.0, y - 7.0, GRIS, 1.0);
    }
    let f = r / 2.0;
    foco(l, c, eje, "C");
    foco(l, c + f, eje, "F");
    let (xo, ho) = (60.0, 56.0);
    let punta = (xo, eje - ho);
    let toca = |y: f32| (c + (r * r - (y - eje).powi(2)).sqrt(), y);
    // Paralelo al eje: vuelve por el foco.
    let p1 = toca(eje - ho);
    rayo(l, &[punta, p1, en_x(p1, (c + f, eje), 180.0)], ROJO);
    // Por el foco: vuelve paralelo.
    let mut p2 = en_x(punta, (c + f, eje), c + r);
    for _ in 0..4 {
        p2 = en_x(punta, (c + f, eje), toca(p2.1).0);
    }
    rayo(l, &[punta, p2, (180.0, p2.1)], NARANJA);
    // La imagen, donde se cruzan.
    let d = c + r - xo;
    let di = 1.0 / (1.0 / f - 1.0 / d);
    let xi = c + r - di;
    let pi = eje + ho * di / d;
    objeto(l, xo, eje, eje - ho, TINTA, false);
    objeto(l, xi, eje, pi, MORADO, false);
    txt(l, xo, eje - ho - 14.0, "objeto", 11.0, TINTA);
    txt(l, xi, pi + 14.0, "imagen", 11.0, MORADO);
}

fn reflexion(l: &mut Lapiz) {
    let p = (180.0, 200.0);
    l.rect(40.0, 200.0, 280.0, 8.0, None, Some(AZUL_SUAVE), 1.0, false);
    rayado(l, 40.0, 320.0, 200.0, false);
    l.linea_con(p.0, p.1, p.0, 50.0, GRIS, 1.2, EstiloTrazo::Discontinuo);
    txt(l, p.0, 38.0, "normal", 11.0, GRIS);
    let a = (70.0, 90.0);
    let b = (290.0, 90.0);
    rayo(l, &[a, p], ROJO);
    rayo(l, &[p, b], AZUL);
    angulo(l, p, a, (p.0, 0.0), 46.0, "i", ROJO);
    angulo(l, p, (p.0, 0.0), b, 46.0, "r", AZUL);
    txt(l, 70.0, 76.0, "rayo incidente", 11.0, ROJO);
    txt(l, 290.0, 76.0, "rayo reflejado", 11.0, AZUL);
    txt_n(l, 180.0, 236.0, "i = r", 13.0, TINTA);
}

fn refraccion(l: &mut Lapiz) {
    let p = (180.0, 130.0);
    l.rect(
        30.0,
        130.0,
        300.0,
        130.0,
        None,
        Some(AZUL_SUAVE),
        1.0,
        false,
    );
    l.linea(30.0, 130.0, 330.0, 130.0, AZUL, 2.0);
    l.linea_con(p.0, 20.0, p.0, 250.0, GRIS, 1.2, EstiloTrazo::Discontinuo);
    txt_a(l, 40.0, 30.0, "aire (n₁)", 12.0, Al::Izq);
    txt_a(l, 40.0, 245.0, "agua (n₂)", 12.0, Al::Izq);
    let a = (90.0, 40.0);
    let r2 = (32.0f32).to_radians();
    let b = (p.0 + 130.0 * r2.sin(), p.1 + 130.0 * r2.cos());
    rayo(l, &[a, p], ROJO);
    rayo(l, &[p, b], ROJO);
    l.linea_con(p.0, p.1, 270.0, 40.0, ROJO, 1.0, EstiloTrazo::Punteado);
    angulo(l, p, a, (p.0, 20.0), 50.0, "θ₁", TINTA);
    angulo(l, p, (p.0, 250.0), b, 50.0, "θ₂", TINTA);
    txt_n(l, 180.0, 280.0, "n₁ · sen θ₁ = n₂ · sen θ₂", 13.0, TINTA);
}

// ----------------------------------------------------------------- ondas

fn onda_transversal(l: &mut Lapiz) {
    let (y0, a, lambda) = (110.0, 50.0, 160.0);
    l.linea_con(20.0, y0, 400.0, y0, GRIS, 1.0, EstiloTrazo::Discontinuo);
    let pts = curva(
        |x| y0 - a * (std::f32::consts::TAU * (x - 20.0) / lambda).sin(),
        20.0,
        380.0,
        90,
    );
    l.poli(&pts, AZUL, 3.0, None);
    l.flecha(&[(60.0, y0), (60.0, y0 - a)], ROJO, 1.2, true);
    txt(l, 72.0, y0 - a / 2.0, "A", 12.0, ROJO);
    l.linea_con(60.0, y0 - a, 60.0, 30.0, GRIS, 1.0, EstiloTrazo::Punteado);
    l.linea_con(220.0, y0 - a, 220.0, 30.0, GRIS, 1.0, EstiloTrazo::Punteado);
    l.flecha(&[(60.0, 34.0), (220.0, 34.0)], VERDE, 1.2, true);
    txt(l, 140.0, 22.0, "λ (longitud de onda)", 11.0, VERDE);
    txt(l, 220.0, y0 - a - 10.0, "cresta", 11.0, TINTA);
    txt(l, 140.0, y0 + a + 14.0, "valle", 11.0, TINTA);
    l.flecha(&[(320.0, 196.0), (400.0, 196.0)], TINTA, 1.6, false);
    txt(l, 360.0, 212.0, "propagación", 11.0, TINTA);
}

fn onda_longitudinal(l: &mut Lapiz) {
    let (lambda, a) = (120.0, 11.0);
    let mut u = 0.0;
    let mut xs = Vec::new();
    while u <= 380.0 {
        xs.push(20.0 + u - a * (std::f32::consts::TAU * u / lambda).sin());
        u += 7.0;
    }
    for x in &xs {
        l.linea(*x, 60.0, *x, 140.0, AZUL, 2.0);
    }
    l.flecha(&[(20.0, 40.0), (140.0, 40.0)], VERDE, 1.2, true);
    txt(l, 80.0, 28.0, "λ", 13.0, VERDE);
    rotulo(l, (20.0, 140.0), (40.0, 175.0), "compresión", Al::Izq);
    rotulo(l, (80.0, 140.0), (130.0, 195.0), "rarefacción", Al::Izq);
    l.flecha(&[(300.0, 180.0), (390.0, 180.0)], TINTA, 1.6, false);
    txt(l, 345.0, 196.0, "propagación", 11.0, TINTA);
}

// --------------------------------------------------------------- vectores

fn suma_vectores(l: &mut Lapiz) {
    let o = (40.0, 220.0);
    let a = (210.0, 170.0);
    let b = (110.0, 90.0);
    let r = (a.0 + b.0 - o.0, a.1 + b.1 - o.1);
    l.linea_con(a.0, a.1, r.0, r.1, GRIS, 1.2, EstiloTrazo::Discontinuo);
    l.linea_con(b.0, b.1, r.0, r.1, GRIS, 1.2, EstiloTrazo::Discontinuo);
    vector(l, o, a, AZUL, "a");
    vector(l, o, b, VERDE, "b");
    vector(l, o, r, ROJO, "a + b");
    l.circulo(o.0, o.1, 3.5, None, Some(TINTA), 1.0);
}

fn componentes(l: &mut Lapiz) {
    let o = (40.0, 220.0);
    ejes(l, o, (20.0, 300.0), (10.0, 240.0), ("x", "y"));
    let v = (230.0, 80.0);
    l.linea_con(v.0, v.1, v.0, o.1, GRIS, 1.2, EstiloTrazo::Discontinuo);
    l.linea_con(v.0, v.1, o.0, v.1, GRIS, 1.2, EstiloTrazo::Discontinuo);
    l.flecha(&[o, (v.0, o.1)], AZUL, 3.0, false);
    l.flecha(&[o, (o.0, v.1)], VERDE, 3.0, false);
    vector(l, o, v, ROJO, "v");
    txt_n(
        l,
        (o.0 + v.0) / 2.0,
        o.1 + 14.0,
        "vx = v · cos θ",
        12.0,
        AZUL,
    );
    txt_a(
        l,
        o.0 - 8.0,
        (o.1 + v.1) / 2.0,
        "vy = v · sen θ",
        12.0,
        Al::Der,
    );
    l.v.last_mut().unwrap().trazo = VERDE;
    angulo(l, o, (v.0, o.1), v, 46.0, "θ", TINTA);
}

fn tiro_parabolico(l: &mut Lapiz) {
    let o = (30.0, 220.0);
    ejes(l, o, (30.0, 380.0), (20.0, 220.0), ("x", "y"));
    let (alcance, hmax) = (300.0, 130.0);
    let altura =
        |x: f32| o.1 - 4.0 * hmax * (x - o.0) * (alcance - (x - o.0)) / (alcance * alcance);
    l.poli(&curva(altura, o.0, o.0 + alcance, 60), NARANJA, 2.0, None);
    l.v.last_mut().unwrap().estilo = EstiloTrazo::Discontinuo;
    for i in 1..6 {
        let x = o.0 + alcance * i as f32 / 6.0;
        l.circulo(x, altura(x), 5.0, Some(TINTA), Some(NARANJA), 1.2);
    }
    let pend = 4.0 * hmax / alcance;
    vector(l, o, (o.0 + 46.0, o.1 - 46.0 * pend), ROJO, "v₀");
    let cima = (o.0 + alcance / 2.0, o.1 - hmax);
    vector(l, cima, (cima.0 + 56.0, cima.1), ROJO, "vx");
    l.linea_con(
        cima.0,
        cima.1,
        cima.0,
        o.1,
        GRIS,
        1.0,
        EstiloTrazo::Discontinuo,
    );
    txt_a(l, cima.0 + 6.0, o.1 - hmax / 2.0, "h máx", 11.0, Al::Izq);
    l.flecha(
        &[(o.0, o.1 + 22.0), (o.0 + alcance, o.1 + 22.0)],
        TINTA,
        1.0,
        true,
    );
    txt(l, o.0 + alcance / 2.0, o.1 + 34.0, "alcance", 11.0, TINTA);
    angulo(
        l,
        o,
        (o.0 + 60.0, o.1),
        (o.0 + 46.0, o.1 - 46.0 * pend),
        34.0,
        "α",
        TINTA,
    );
}

fn pendulo(l: &mut Lapiz) {
    let p = (150.0, 20.0);
    rayado(l, 90.0, 210.0, 20.0, true);
    let largo = 180.0;
    let t: f32 = 25.0;
    let bola = (
        p.0 + largo * t.to_radians().sin(),
        p.1 + largo * t.to_radians().cos(),
    );
    l.linea_con(
        p.0,
        p.1,
        p.0,
        p.1 + largo + 20.0,
        GRIS,
        1.0,
        EstiloTrazo::Discontinuo,
    );
    l.arco(p.0, p.1, largo, 90.0 - t - 4.0, 90.0 + t + 4.0, GRIS, 1.0);
    l.v.last_mut().unwrap().estilo = EstiloTrazo::Punteado;
    l.linea(p.0, p.1, bola.0, bola.1, TINTA, 1.8);
    angulo(l, p, (p.0, p.1 + 100.0), bola, 48.0, "θ", TINTA);
    txt(
        l,
        (p.0 + bola.0) / 2.0 + 14.0,
        (p.1 + bola.1) / 2.0,
        "L",
        13.0,
        TINTA,
    );
    l.circulo(bola.0, bola.1, 15.0, Some(TINTA), Some(AZUL), G);
    txt_n(l, bola.0, bola.1, "m", 12.0, BLANCO);
    vector(
        l,
        (bola.0, bola.1 + 15.0),
        (bola.0, bola.1 + 75.0),
        ROJO,
        "m·g",
    );
    l.circulo(p.0, p.1, 4.0, None, Some(TINTA), 1.0);
}
