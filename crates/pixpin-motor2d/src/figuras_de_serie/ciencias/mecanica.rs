//! **Ingenieria mecanica**: engranajes, rodamiento, resorte, correa,
//! perno y tuerca, piston y biela; vigas con sus cargas y apoyos, fuerzas
//! en un nudo; y lo del dibujo tecnico: vistas ortogonales acotadas,
//! simbolos de soldadura, tolerancias y marcos de control geometrico.
//! Acero en gris azulado, cargas en rojo y reacciones en verde.

use super::*;
use crate::elemento::EstiloTrazo;

const CATEGORIA: &str = "Mecánica";
const PREFIJO: &str = "ciencia-";
const ACERO: ColorRgba = hex(0xc9d1dc);
const ACERO_OSCURO: ColorRgba = hex(0x8a96a8);
const LINEA: ColorRgba = hex(0x1f3550);

pub(super) fn todas(v: &mut Vec<FiguraGuardada>) {
    let lista: [Dibujo; 15] = [
        ("engranaje", "Engranaje recto", |l| {
            engranaje(l, (100.0, 100.0), 80.0, 18, 0.0, ACERO)
        }),
        ("par-engranajes", "Par de engranajes", par_engranajes),
        ("rodamiento", "Rodamiento de bolas", rodamiento),
        ("resorte", "Resorte (ley de Hooke)", resorte),
        ("correa", "Transmisión por correa", correa),
        ("perno-tuerca", "Perno y tuerca", perno_tuerca),
        ("piston-biela", "Pistón, biela y cigüeñal", piston_biela),
        ("viga-voladizo", "Viga empotrada (voladizo)", viga_voladizo),
        ("viga-apoyada", "Viga simplemente apoyada", viga_apoyada),
        (
            "fuerzas-nudo",
            "Diagrama de fuerzas en un nudo",
            fuerzas_nudo,
        ),
        ("vistas-ortogonales", "Vistas ortogonales acotadas", vistas),
        (
            "soldadura-angulo",
            "Símbolo de soldadura en ángulo",
            |l| soldadura(l, false),
        ),
        ("soldadura-v", "Símbolo de soldadura a tope en V", |l| {
            soldadura(l, true)
        }),
        ("tolerancia", "Cotas con tolerancia y ajuste", tolerancias),
        (
            "control-geometrico",
            "Marcos de tolerancia geométrica",
            control_geometrico,
        ),
    ];
    meter(v, PREFIJO, CATEGORIA, &lista);
}

// ------------------------------------------------------------ elementos

/// **Un engranaje** de `dientes` dientes en `c`, con su radio primitivo
/// `r`, girado `giro` grados: el contorno, el cubo, el agujero con su
/// chavetero y la circunferencia primitiva a raya y punto.
fn engranaje(l: &mut Lapiz, c: (f32, f32), r: f32, dientes: usize, giro: f32, relleno: ColorRgba) {
    let paso = 360.0 / dientes as f32;
    let modulo = 2.0 * r / dientes as f32;
    let (fuera, dentro) = (r + modulo, r - 1.25 * modulo);
    let p = |a: f32, rr: f32| {
        (
            c.0 + rr * a.to_radians().cos(),
            c.1 + rr * a.to_radians().sin(),
        )
    };
    let mut v = Vec::new();
    for i in 0..dientes {
        let a = giro + paso * i as f32;
        v.push(p(a - 0.5 * paso, dentro));
        v.push(p(a - 0.27 * paso, dentro));
        v.push(p(a - 0.16 * paso, fuera));
        v.push(p(a + 0.16 * paso, fuera));
        v.push(p(a + 0.27 * paso, dentro));
    }
    forma(l, &v, LINEA, Some(relleno), 1.8);
    l.arco(c.0, c.1, r, 0.0, 360.0, LINEA, 0.8);
    l.v.last_mut().unwrap().estilo = EstiloTrazo::RayaPunto;
    l.circulo(c.0, c.1, r * 0.36, Some(LINEA), Some(ACERO_OSCURO), 1.5);
    let agujero = r * 0.16;
    l.circulo(c.0, c.1, agujero, Some(LINEA), Some(BLANCO), 1.5);
    l.rect(
        c.0 - agujero * 0.3,
        c.1 - agujero - agujero * 0.35,
        agujero * 0.6,
        agujero * 0.5,
        Some(LINEA),
        Some(BLANCO),
        1.2,
        false,
    );
}

fn par_engranajes(l: &mut Lapiz) {
    let (c1, c2) = ((100.0, 110.0), (228.0, 110.0));
    engranaje(l, c1, 80.0, 20, 0.0, ACERO);
    engranaje(l, c2, 48.0, 12, 15.0, hex(0xe8d6a8));
    l.flecha(
        &bezier((40.0, 20.0), (100.0, -4.0), (160.0, 20.0), 12),
        ROJO,
        1.8,
        false,
    );
    l.flecha(
        &bezier((252.0, 48.0), (226.0, 36.0), (200.0, 48.0), 10),
        ROJO,
        1.8,
        false,
    );
    txt(l, c1.0, 222.0, "z₁ = 20", 12.0, TINTA);
    txt(l, c2.0, 222.0, "z₂ = 12", 12.0, TINTA);
    txt_n(l, 163.0, 246.0, "i = z₁ / z₂ = n₂ / n₁", 12.0, TINTA);
}

fn rodamiento(l: &mut Lapiz) {
    let c = (110.0, 110.0);
    l.circulo(c.0, c.1, 96.0, Some(LINEA), Some(ACERO), G);
    l.circulo(c.0, c.1, 78.0, Some(LINEA), Some(BLANCO), 1.5);
    l.circulo(c.0, c.1, 62.0, Some(hex(0xb89a52)), None, 3.0);
    l.circulo(c.0, c.1, 48.0, Some(LINEA), Some(ACERO), 1.5);
    l.circulo(c.0, c.1, 30.0, Some(LINEA), Some(BLANCO), 1.5);
    for i in 0..10 {
        let a = (36.0 * i as f32).to_radians();
        let (x, y) = (c.0 + 63.0 * a.cos(), c.1 + 63.0 * a.sin());
        l.circulo(x, y, 13.0, Some(LINEA), Some(ACERO_OSCURO), 1.5);
        l.circulo(x - 4.0, y - 4.0, 3.0, None, Some(BLANCO), 1.0);
    }
    l.linea_con(
        c.0 - 106.0,
        c.1,
        c.0 + 106.0,
        c.1,
        GRIS,
        0.8,
        EstiloTrazo::RayaPunto,
    );
    l.linea_con(
        c.0,
        c.1 - 106.0,
        c.0,
        c.1 + 106.0,
        GRIS,
        0.8,
        EstiloTrazo::RayaPunto,
    );
    rotulos_derecha(
        l,
        236.0,
        24.0,
        44.0,
        &[
            ((190.0, 70.0), "Aro exterior"),
            ((161.0, 74.0), "Bola"),
            ((166.0, 140.0), "Jaula"),
            ((150.0, 130.0), "Aro interior"),
            ((130.0, 110.0), "Agujero (eje)"),
        ],
    );
}

fn resorte(l: &mut Lapiz) {
    rayado(l, 20.0, 260.0, 14.0, true);
    // En reposo y estirado.
    l.poli(
        &zigzag((70.0, 14.0), (70.0, 130.0), 7, 16.0, 12.0),
        LINEA,
        G,
        None,
    );
    l.rect(46.0, 130.0, 48.0, 40.0, Some(LINEA), Some(ACERO), G, true);
    txt_n(l, 70.0, 150.0, "m", 14.0, TINTA);
    l.poli(
        &zigzag((200.0, 14.0), (200.0, 190.0), 7, 12.0, 12.0),
        LINEA,
        G,
        None,
    );
    l.rect(176.0, 190.0, 48.0, 40.0, Some(LINEA), Some(ACERO), G, true);
    txt_n(l, 200.0, 210.0, "m", 14.0, TINTA);
    l.linea_con(
        94.0,
        130.0,
        176.0,
        130.0,
        GRIS,
        1.0,
        EstiloTrazo::Discontinuo,
    );
    l.flecha(&[(240.0, 130.0), (240.0, 190.0)], TINTA, 1.2, true);
    l.linea_con(
        224.0,
        190.0,
        248.0,
        190.0,
        GRIS,
        1.0,
        EstiloTrazo::Discontinuo,
    );
    l.linea_con(
        224.0,
        130.0,
        248.0,
        130.0,
        GRIS,
        1.0,
        EstiloTrazo::Discontinuo,
    );
    txt_a(l, 246.0, 160.0, "x", 13.0, Al::Izq);
    vector(l, (200.0, 230.0), (200.0, 276.0), ROJO, "F");
    txt_a(l, 92.0, 70.0, "k", 13.0, Al::Izq);
    txt_n(l, 130.0, 300.0, "F = −k · x", 14.0, TINTA);
}

fn correa(l: &mut Lapiz) {
    let (c1, r1, c2, r2): ((f32, f32), f32, (f32, f32), f32) =
        ((90.0, 100.0), 70.0, (320.0, 100.0), 36.0);
    let d = c2.0 - c1.0;
    let phi = ((r1 - r2) / d).asin();
    let a = 90.0 - phi.to_degrees();
    let p = |c: (f32, f32), r: f32, g: f32| {
        (
            c.0 + r * g.to_radians().cos(),
            c.1 + r * g.to_radians().sin(),
        )
    };
    for (c, r) in [(c1, r1), (c2, r2)] {
        l.circulo(c.0, c.1, r, Some(LINEA), Some(ACERO), G);
        l.circulo(c.0, c.1, r * 0.7, Some(ACERO_OSCURO), None, 1.2);
        l.circulo(c.0, c.1, r * 0.2, Some(LINEA), Some(BLANCO), 1.5);
    }
    let correa = hex(0x3a3f4a);
    let (t1, t2) = (p(c1, r1 + 3.0, -a), p(c2, r2 + 3.0, -a));
    let (b1, b2) = (p(c1, r1 + 3.0, a), p(c2, r2 + 3.0, a));
    l.linea(t1.0, t1.1, t2.0, t2.1, correa, 5.0);
    l.linea(b1.0, b1.1, b2.0, b2.1, correa, 5.0);
    l.arco(c1.0, c1.1, r1 + 3.0, a, 360.0 - a, correa, 5.0);
    l.arco(c2.0, c2.1, r2 + 3.0, -a, a, correa, 5.0);
    l.flecha(&[(190.0, 16.0), (240.0, 22.0)], ROJO, 1.8, false);
    txt(l, c1.0, 196.0, "conductora (d₁, n₁)", 11.0, TINTA);
    txt(l, c2.0, 196.0, "conducida (d₂, n₂)", 11.0, TINTA);
    txt_n(l, 205.0, 222.0, "n₁ · d₁ = n₂ · d₂", 12.0, TINTA);
}

fn perno_tuerca(l: &mut Lapiz) {
    // De lado: cabeza, arandela, vastago roscado y tuerca.
    l.rect(10.0, 30.0, 34.0, 90.0, Some(LINEA), Some(ACERO), G, false);
    l.linea(10.0, 52.0, 44.0, 52.0, LINEA, 1.2);
    l.linea(10.0, 98.0, 44.0, 98.0, LINEA, 1.2);
    l.rect(44.0, 55.0, 216.0, 40.0, Some(LINEA), Some(ACERO), G, false);
    let mut x = 120.0;
    while x < 256.0 {
        l.linea(x, 55.0, x + 6.0, 95.0, LINEA, 1.0);
        x += 8.0;
    }
    l.linea_con(
        120.0,
        59.0,
        260.0,
        59.0,
        LINEA,
        0.8,
        EstiloTrazo::Discontinuo,
    );
    l.linea_con(
        120.0,
        91.0,
        260.0,
        91.0,
        LINEA,
        0.8,
        EstiloTrazo::Discontinuo,
    );
    l.rect(
        150.0,
        34.0,
        8.0,
        82.0,
        Some(LINEA),
        Some(ACERO_OSCURO),
        1.5,
        false,
    );
    l.rect(158.0, 30.0, 34.0, 90.0, Some(LINEA), Some(ACERO), G, false);
    l.linea(158.0, 52.0, 192.0, 52.0, LINEA, 1.2);
    l.linea(158.0, 98.0, 192.0, 98.0, LINEA, 1.2);
    l.linea_con(0.0, 75.0, 270.0, 75.0, GRIS, 0.8, EstiloTrazo::RayaPunto);
    // La tuerca de frente.
    let c = (384.0, 75.0);
    let hexagono: Vec<(f32, f32)> = (0..6)
        .map(|i| {
            (
                c.0 + 52.0 * (60.0 * i as f32).to_radians().cos(),
                c.1 + 52.0 * (60.0 * i as f32).to_radians().sin(),
            )
        })
        .collect();
    forma(l, &hexagono, LINEA, Some(ACERO), G);
    l.circulo(c.0, c.1, 44.0, Some(ACERO_OSCURO), None, 1.0);
    l.circulo(c.0, c.1, 20.0, Some(LINEA), Some(BLANCO), 1.5);
    l.arco(c.0, c.1, 23.0, 0.0, 270.0, LINEA, 1.0);
    cota_h(l, 44.0, 150.0, 150.0, 95.0, "l");
    cota_v(l, 55.0, 95.0, 286.0, 260.0, "d (M20)");
    cota_h(l, c.0 - 45.0, c.0 + 45.0, 150.0, 120.0, "s (llave)");
    rotulo(l, (27.0, 30.0), (20.0, 4.0), "cabeza", Al::Izq);
    rotulo(l, (80.0, 55.0), (70.0, 26.0), "vástago", Al::Izq);
    rotulo(l, (154.0, 34.0), (150.0, 4.0), "arandela", Al::Der);
    rotulo(l, (175.0, 30.0), (196.0, 4.0), "tuerca", Al::Izq);
    rotulo(l, (236.0, 55.0), (244.0, 26.0), "rosca", Al::Izq);
}

fn piston_biela(l: &mut Lapiz) {
    let ciguenal = (110.0, 280.0);
    let manivela = 50.0;
    let a: f32 = -60.0;
    let muneca = (
        ciguenal.0 + manivela * a.to_radians().cos(),
        ciguenal.1 + manivela * a.to_radians().sin(),
    );
    let largo = 150.0;
    let bulon = (
        ciguenal.0,
        muneca.1 - (largo * largo - (muneca.0 - ciguenal.0).powi(2)).sqrt(),
    );
    // El cilindro.
    l.rect(
        40.0,
        10.0,
        12.0,
        210.0,
        None,
        Some(ACERO_OSCURO),
        1.0,
        false,
    );
    l.rect(
        168.0,
        10.0,
        12.0,
        210.0,
        None,
        Some(ACERO_OSCURO),
        1.0,
        false,
    );
    l.rect(40.0, 0.0, 140.0, 12.0, None, Some(ACERO_OSCURO), 1.0, false);
    // El ciguenal, la biela y el piston.
    l.circulo(
        ciguenal.0,
        ciguenal.1,
        manivela + 14.0,
        Some(LINEA),
        Some(ACERO),
        G,
    );
    l.circulo(ciguenal.0, ciguenal.1, manivela, Some(GRIS), None, 1.0);
    l.v.last_mut().unwrap().estilo = EstiloTrazo::Discontinuo;
    l.circulo(
        ciguenal.0,
        ciguenal.1,
        10.0,
        Some(LINEA),
        Some(ACERO_OSCURO),
        1.5,
    );
    l.linea(bulon.0, bulon.1, muneca.0, muneca.1, LINEA, 16.0);
    l.linea(bulon.0, bulon.1, muneca.0, muneca.1, ACERO, 11.0);
    l.circulo(
        muneca.0,
        muneca.1,
        11.0,
        Some(LINEA),
        Some(ACERO_OSCURO),
        1.5,
    );
    l.rect(
        54.0,
        bulon.1 - 40.0,
        112.0,
        74.0,
        Some(LINEA),
        Some(hex(0xdfe4eb)),
        G,
        false,
    );
    for k in 0..3 {
        let y = bulon.1 - 32.0 + 8.0 * k as f32;
        l.linea(54.0, y, 166.0, y, LINEA, 1.2);
    }
    l.circulo(bulon.0, bulon.1, 9.0, Some(LINEA), Some(ACERO_OSCURO), 1.5);
    // Los puntos muertos y la carrera.
    let pms = ciguenal.1 - manivela - largo - 40.0;
    let pmi = ciguenal.1 + manivela - largo - 40.0;
    l.linea_con(186.0, pms, 236.0, pms, GRIS, 1.0, EstiloTrazo::Discontinuo);
    l.linea_con(186.0, pmi, 236.0, pmi, GRIS, 1.0, EstiloTrazo::Discontinuo);
    txt_a(l, 242.0, pms, "PMS", 11.0, Al::Izq);
    txt_a(l, 242.0, pmi, "PMI", 11.0, Al::Izq);
    l.flecha(&[(214.0, pms), (214.0, pmi)], TINTA, 1.0, true);
    txt_a(l, 220.0, (pms + pmi) / 2.0, "carrera", 11.0, Al::Izq);
    rotulo(
        l,
        (160.0, bulon.1 + 20.0),
        (290.0, 190.0),
        "pistón",
        Al::Izq,
    );
    rotulo(
        l,
        ((bulon.0 + muneca.0) / 2.0, (bulon.1 + muneca.1) / 2.0),
        (290.0, 236.0),
        "biela",
        Al::Izq,
    );
    rotulo(
        l,
        (ciguenal.0 + 50.0, ciguenal.1 + 30.0),
        (290.0, 300.0),
        "cigüeñal",
        Al::Izq,
    );
}

// ---------------------------------------------------------------- vigas

/// Unas flechas de carga repartida de `x0` a `x1` que caen en `y`.
fn repartida(l: &mut Lapiz, x0: f32, x1: f32, y: f32, n: usize, s: &str) {
    l.linea(x0, y - 40.0, x1, y - 40.0, ROJO, 1.5);
    for i in 0..=n {
        let x = x0 + (x1 - x0) * i as f32 / n as f32;
        l.flecha(&[(x, y - 40.0), (x, y - 2.0)], ROJO, 1.4, false);
    }
    txt_n(l, (x0 + x1) / 2.0, y - 54.0, s, 13.0, ROJO);
}

fn viga_voladizo(l: &mut Lapiz) {
    l.rect(20.0, 30.0, 20.0, 150.0, None, Some(GRIS_CLARO), 1.0, false);
    let mut y = 34.0;
    while y < 176.0 {
        l.linea(20.0, y + 10.0, 40.0, y, GRIS, 1.0);
        y += 10.0;
    }
    l.linea(40.0, 30.0, 40.0, 180.0, TINTA, 2.5);
    l.rect(40.0, 95.0, 300.0, 20.0, Some(LINEA), Some(ACERO), G, false);
    let flecha = curva(
        |x| 105.0 + 34.0 * ((x - 40.0) / 300.0).powi(2),
        40.0,
        340.0,
        30,
    );
    l.poli(&flecha, AZUL, 1.2, None);
    l.v.last_mut().unwrap().estilo = EstiloTrazo::Discontinuo;
    vector(l, (340.0, 30.0), (340.0, 93.0), ROJO, "");
    txt_n(l, 340.0, 20.0, "P", 13.0, ROJO);
    vector(l, (60.0, 170.0), (60.0, 118.0), VERDE, "");
    txt_n(l, 74.0, 176.0, "R", 13.0, VERDE);
    l.flecha(
        &bezier((70.0, 66.0), (52.0, 50.0), (60.0, 82.0), 8),
        VERDE,
        1.8,
        false,
    );
    txt_n(l, 82.0, 60.0, "M", 13.0, VERDE);
    cota_h(l, 40.0, 340.0, 210.0, 115.0, "L");
    txt(l, 300.0, 156.0, "flecha", 10.0, AZUL);
}

/// Un apoyo fijo (triangulo) o movil (triangulo con rodillos) bajo `x`.
fn apoyo(l: &mut Lapiz, x: f32, y: f32, movil: bool) {
    forma(
        l,
        &[(x, y), (x - 16.0, y + 26.0), (x + 16.0, y + 26.0)],
        LINEA,
        Some(ACERO),
        1.8,
    );
    let suelo = if movil {
        for dx in [-10.0, 0.0, 10.0] {
            l.circulo(x + dx, y + 31.0, 4.5, Some(LINEA), Some(BLANCO), 1.2);
        }
        y + 36.0
    } else {
        y + 26.0
    };
    rayado(l, x - 24.0, x + 24.0, suelo, false);
}

fn viga_apoyada(l: &mut Lapiz) {
    l.rect(30.0, 96.0, 320.0, 16.0, Some(LINEA), Some(ACERO), G, false);
    apoyo(l, 50.0, 112.0, false);
    apoyo(l, 330.0, 112.0, true);
    repartida(l, 30.0, 350.0, 94.0, 10, "q (kN/m)");
    vector(l, (50.0, 200.0), (50.0, 152.0), VERDE, "");
    txt_n(l, 66.0, 196.0, "RA", 12.0, VERDE);
    vector(l, (330.0, 210.0), (330.0, 162.0), VERDE, "");
    txt_n(l, 346.0, 206.0, "RB", 12.0, VERDE);
    cota_h(l, 50.0, 330.0, 236.0, 220.0, "L");
    txt_a(l, 30.0, 20.0, "RA = RB = q · L / 2", 11.0, Al::Izq);
}

fn fuerzas_nudo(l: &mut Lapiz) {
    let o = (120.0, 130.0);
    // F1 y F2 dadas; F3 cierra el equilibrio.
    let f1 = (
        100.0 * 20.0f32.to_radians().cos(),
        -100.0 * 20.0f32.to_radians().sin(),
    );
    let f2 = (
        -80.0 * 50.0f32.to_radians().cos(),
        -80.0 * 50.0f32.to_radians().sin(),
    );
    let f3 = (-(f1.0 + f2.0), -(f1.1 + f2.1));
    let fuerzas = [(f1, "F₁", ROJO), (f2, "F₂", AZUL), (f3, "F₃", VERDE)];
    for (d, s, c) in fuerzas {
        vector(l, o, (o.0 + d.0, o.1 + d.1), c, s);
    }
    l.circulo(o.0, o.1, 6.0, Some(TINTA), Some(BLANCO), G);
    // El poligono de fuerzas, cerrado.
    let mut p = (260.0, 170.0);
    for (d, s, c) in fuerzas {
        let q = (p.0 + d.0 * 0.8, p.1 + d.1 * 0.8);
        l.flecha(&[p, q], c, 2.0, false);
        txt(
            l,
            (p.0 + q.0) / 2.0 + 10.0,
            (p.1 + q.1) / 2.0 - 8.0,
            s,
            11.0,
            c,
        );
        p = q;
    }
    txt(l, 300.0, 210.0, "polígono cerrado", 10.0, GRIS);
    txt_n(l, 200.0, 250.0, "ΣFx = 0 · ΣFy = 0", 13.0, TINTA);
}

// ----------------------------------------------------- dibujo tecnico

fn vistas(l: &mut Lapiz) {
    let (x0, y0) = (50.0, 50.0);
    let lado = |l: &mut Lapiz, pts: &[(f32, f32)]| forma(l, pts, LINEA, Some(hex(0xf2f5f9)), 2.0);
    // Alzado: la L de frente.
    lado(
        l,
        &[
            (x0, y0),
            (x0 + 50.0, y0),
            (x0 + 50.0, y0 + 50.0),
            (x0 + 120.0, y0 + 50.0),
            (x0 + 120.0, y0 + 80.0),
            (x0, y0 + 80.0),
        ],
    );
    // Perfil izquierdo, a la derecha (sistema europeo).
    let px = x0 + 180.0;
    lado(
        l,
        &[
            (px, y0),
            (px + 60.0, y0),
            (px + 60.0, y0 + 80.0),
            (px, y0 + 80.0),
        ],
    );
    l.linea_con(
        px,
        y0 + 50.0,
        px + 60.0,
        y0 + 50.0,
        LINEA,
        1.2,
        EstiloTrazo::Discontinuo,
    );
    // Planta, debajo.
    let py = y0 + 130.0;
    lado(
        l,
        &[
            (x0, py),
            (x0 + 120.0, py),
            (x0 + 120.0, py + 60.0),
            (x0, py + 60.0),
        ],
    );
    l.linea(x0 + 50.0, py, x0 + 50.0, py + 60.0, LINEA, 1.5);
    // Lineas de referencia entre vistas.
    for x in [x0, x0 + 50.0, x0 + 120.0] {
        l.linea_con(
            x,
            y0 + 84.0,
            x,
            py - 4.0,
            GRIS_CLARO,
            0.8,
            EstiloTrazo::Discontinuo,
        );
    }
    for y in [y0, y0 + 50.0, y0 + 80.0] {
        l.linea_con(
            x0 + 124.0,
            y,
            px - 4.0,
            y,
            GRIS_CLARO,
            0.8,
            EstiloTrazo::Discontinuo,
        );
    }
    cota_h(l, x0, x0 + 120.0, y0 - 18.0, y0 + 50.0, "120");
    cota_h(l, x0, x0 + 50.0, y0 - 38.0, y0, "50");
    cota_v(l, y0, y0 + 80.0, x0 - 20.0, x0, "80");
    cota_v(l, y0 + 50.0, y0 + 80.0, x0 + 140.0, x0 + 120.0, "30");
    cota_h(l, px, px + 60.0, y0 - 18.0, y0, "60");
    cota_v(l, py, py + 60.0, x0 - 20.0, x0, "60");
    txt(l, x0 + 60.0, y0 + 98.0, "ALZADO", 9.0, GRIS);
    txt(l, px + 30.0, y0 + 98.0, "PERFIL IZQUIERDO", 9.0, GRIS);
    txt(l, x0 + 60.0, py + 76.0, "PLANTA", 9.0, GRIS);
    txt(l, px + 30.0, py + 40.0, "cotas en mm", 10.0, GRIS);
}

/// **Un simbolo de soldadura** (ISO 2553): la flecha a la union, la linea
/// de referencia llena y la de identificacion a trazos, el simbolo (en
/// angulo o en V), la medida, la longitud y la cola con el proceso.
fn soldadura(l: &mut Lapiz, en_v: bool) {
    // Las dos chapas.
    if en_v {
        l.rect(
            10.0,
            150.0,
            90.0,
            20.0,
            Some(LINEA),
            Some(ACERO),
            1.5,
            false,
        );
        l.rect(
            110.0,
            150.0,
            90.0,
            20.0,
            Some(LINEA),
            Some(ACERO),
            1.5,
            false,
        );
        forma(
            l,
            &[
                (92.0, 150.0),
                (118.0, 150.0),
                (110.0, 170.0),
                (100.0, 170.0),
            ],
            LINEA,
            Some(ACERO_OSCURO),
            1.2,
        );
    } else {
        l.rect(
            10.0,
            160.0,
            160.0,
            18.0,
            Some(LINEA),
            Some(ACERO),
            1.5,
            false,
        );
        l.rect(
            80.0,
            100.0,
            18.0,
            60.0,
            Some(LINEA),
            Some(ACERO),
            1.5,
            false,
        );
        forma(
            l,
            &[(98.0, 160.0), (98.0, 142.0), (116.0, 160.0)],
            LINEA,
            Some(ACERO_OSCURO),
            1.2,
        );
    }
    let union = if en_v { (105.0, 150.0) } else { (104.0, 152.0) };
    let codo = (160.0, 80.0);
    l.flecha(&[codo, union], LINEA, 1.5, false);
    l.linea(codo.0, codo.1, 330.0, codo.1, LINEA, 1.5);
    l.linea_con(
        codo.0,
        codo.1 + 6.0,
        330.0,
        codo.1 + 6.0,
        LINEA,
        1.2,
        EstiloTrazo::Discontinuo,
    );
    l.poli(&[(330.0, codo.1), (346.0, codo.1 - 14.0)], LINEA, 1.5, None);
    l.poli(&[(330.0, codo.1), (346.0, codo.1 + 14.0)], LINEA, 1.5, None);
    txt_a(l, 350.0, codo.1, "111", 11.0, Al::Izq);
    let sx = 220.0;
    if en_v {
        l.poli(
            &[
                (sx - 10.0, codo.1 - 18.0),
                (sx, codo.1),
                (sx + 10.0, codo.1 - 18.0),
            ],
            LINEA,
            1.8,
            None,
        );
        txt_a(l, sx - 16.0, codo.1 - 12.0, "s8", 11.0, Al::Der);
        txt_a(l, sx + 16.0, codo.1 - 12.0, "200", 11.0, Al::Izq);
    } else {
        forma(
            l,
            &[
                (sx - 10.0, codo.1),
                (sx - 10.0, codo.1 - 18.0),
                (sx + 8.0, codo.1),
            ],
            LINEA,
            None,
            1.8,
        );
        txt_a(l, sx - 16.0, codo.1 - 10.0, "a5", 11.0, Al::Der);
        txt_a(l, sx + 14.0, codo.1 - 10.0, "4 × 50 (100)", 11.0, Al::Izq);
        l.circulo(codo.0, codo.1, 6.0, Some(LINEA), None, 1.5);
    }
    rotulo(
        l,
        (260.0, codo.1),
        (280.0, 130.0),
        "línea de referencia",
        Al::Izq,
    );
    rotulo(
        l,
        (290.0, codo.1 + 6.0),
        (300.0, 160.0),
        "línea de identificación",
        Al::Izq,
    );
    rotulo(
        l,
        (346.0, codo.1 - 10.0),
        (356.0, 30.0),
        "proceso (111: arco)",
        Al::Izq,
    );
    if !en_v {
        rotulo(l, (156.0, 76.0), (120.0, 30.0), "todo alrededor", Al::Der);
    }
}

fn tolerancias(l: &mut Lapiz) {
    // Un eje escalonado con sus diametros tolerados.
    l.rect(40.0, 60.0, 140.0, 80.0, Some(LINEA), Some(ACERO), G, false);
    l.rect(180.0, 75.0, 140.0, 50.0, Some(LINEA), Some(ACERO), G, false);
    l.linea_con(20.0, 100.0, 340.0, 100.0, GRIS, 0.8, EstiloTrazo::RayaPunto);
    cota_v(l, 60.0, 140.0, 20.0, 40.0, "Ø40 ±0,05");
    cota_v(l, 75.0, 125.0, 344.0, 320.0, "Ø25 g6");
    cota_h(l, 40.0, 180.0, 36.0, 60.0, "60 +0,1/0");
    cota_h(l, 180.0, 320.0, 50.0, 75.0, "80 ±0,2");
    txt_a(
        l,
        40.0,
        172.0,
        "Ajuste: Ø25 H7/g6 (agujero H7, eje g6)",
        11.0,
        Al::Izq,
    );
    txt_a(
        l,
        40.0,
        192.0,
        "Medida máx. = nominal + desv. sup. · mín. = nominal + desv. inf.",
        10.0,
        Al::Izq,
    );
    l.v.last_mut().unwrap().trazo = GRIS;
}

/// Lo que va en la primera casilla de un marco de control geometrico.
#[derive(Clone, Copy)]
enum Caracteristica {
    Planitud,
    Perpendicularidad,
    Paralelismo,
    Posicion,
    Circularidad,
}

fn simbolo(l: &mut Lapiz, c: Caracteristica, cx: f32, cy: f32) {
    match c {
        Caracteristica::Planitud => forma(
            l,
            &[
                (cx - 9.0, cy + 5.0),
                (cx - 4.0, cy - 5.0),
                (cx + 9.0, cy - 5.0),
                (cx + 4.0, cy + 5.0),
            ],
            LINEA,
            None,
            1.5,
        ),
        Caracteristica::Perpendicularidad => {
            l.linea(cx - 9.0, cy + 7.0, cx + 9.0, cy + 7.0, LINEA, 1.5);
            l.linea(cx, cy + 7.0, cx, cy - 8.0, LINEA, 1.5);
        }
        Caracteristica::Paralelismo => {
            l.linea(cx - 7.0, cy + 7.0, cx - 1.0, cy - 7.0, LINEA, 1.5);
            l.linea(cx + 1.0, cy + 7.0, cx + 7.0, cy - 7.0, LINEA, 1.5);
        }
        Caracteristica::Posicion => {
            l.circulo(cx, cy, 6.0, Some(LINEA), None, 1.5);
            l.linea(cx - 9.0, cy, cx + 9.0, cy, LINEA, 1.5);
            l.linea(cx, cy - 9.0, cx, cy + 9.0, LINEA, 1.5);
        }
        Caracteristica::Circularidad => l.circulo(cx, cy, 7.0, Some(LINEA), None, 1.5),
    }
}

/// Un marco: simbolo, tolerancia y referencias.
fn marco(l: &mut Lapiz, x: f32, y: f32, c: Caracteristica, tol: &str, refs: &[&str]) {
    let h = 26.0;
    let mut anchos = vec![30.0, 18.0 + 7.5 * tol.chars().count() as f32];
    anchos.extend(refs.iter().map(|_| 26.0));
    let total: f32 = anchos.iter().sum();
    l.rect(x, y, total, h, Some(LINEA), Some(BLANCO), 1.5, false);
    let mut cx = x;
    for (i, w) in anchos.iter().enumerate() {
        if i > 0 {
            l.linea(cx, y, cx, y + h, LINEA, 1.2);
        }
        let m = cx + w / 2.0;
        match i {
            0 => simbolo(l, c, m, y + h / 2.0),
            1 => txt(l, m, y + h / 2.0, tol, 12.0, TINTA),
            _ => txt_n(l, m, y + h / 2.0, refs[i - 2], 12.0, TINTA),
        }
        cx += w;
    }
}

fn control_geometrico(l: &mut Lapiz) {
    let filas = [
        (Caracteristica::Planitud, "0,05", &[][..], "planitud"),
        (
            Caracteristica::Circularidad,
            "0,02",
            &[][..],
            "circularidad",
        ),
        (
            Caracteristica::Paralelismo,
            "0,1",
            &["A"][..],
            "paralelismo",
        ),
        (
            Caracteristica::Perpendicularidad,
            "0,02",
            &["A"][..],
            "perpendicularidad",
        ),
        (
            Caracteristica::Posicion,
            "Ø0,1",
            &["A", "B"][..],
            "posición",
        ),
    ];
    for (i, (c, tol, refs, nombre)) in filas.iter().enumerate() {
        let y = 10.0 + 40.0 * i as f32;
        marco(l, 10.0, y, *c, tol, refs);
        txt_a(l, 190.0, y + 13.0, nombre, 11.0, Al::Izq);
    }
    // La referencia A: el triangulo lleno sobre su superficie.
    let y = 230.0;
    l.rect(
        10.0,
        y + 30.0,
        160.0,
        24.0,
        Some(LINEA),
        Some(ACERO),
        1.5,
        false,
    );
    forma(
        l,
        &[(80.0, y + 30.0), (70.0, y + 18.0), (90.0, y + 18.0)],
        LINEA,
        Some(LINEA),
        1.2,
    );
    l.linea(80.0, y + 18.0, 80.0, y + 6.0, LINEA, 1.2);
    l.rect(
        68.0,
        y - 18.0,
        24.0,
        24.0,
        Some(LINEA),
        Some(BLANCO),
        1.5,
        false,
    );
    txt_n(l, 80.0, y - 6.0, "A", 12.0, TINTA);
    txt_a(l, 190.0, y + 10.0, "elemento de referencia", 11.0, Al::Izq);
}
