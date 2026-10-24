//! **El mobiliario** en planta (a 1 px = 1 cm, con las medidas de verdad:
//! cama de 160 × 200, inodoro de 40 × 70...) y en isometrica (cajas con
//! sus tres caras en tres tonos), para amueblar plantas y maquetas.

use super::tecnico::{
    BLANCA, BLANCO, CLARO, Caras, Dibujo, FINA, GRIS, HORMIGON, MADERA, TELA, TINTA, caja_iso,
    forma, girar, iso, poner,
};
use super::{Al, Lapiz, WORK_SANS, hex};
use crate::biblioteca::FiguraGuardada;
use crate::elemento::{ColorRgba, EstiloTrazo};

pub(super) const CATEGORIA: &str = "Mobiliario";

const AGUA: ColorRgba = hex(0xdcebf6);
const COJIN: ColorRgba = hex(0xe3eaf4);

pub(super) fn todas(v: &mut Vec<FiguraGuardada>) {
    let lista: [Dibujo; 20] = [
        ("arq-cama-doble", "Cama doble (planta)", |l| {
            cama(l, 160.0, 200.0, 2)
        }),
        ("arq-cama-individual", "Cama individual (planta)", |l| {
            cama(l, 90.0, 190.0, 1)
        }),
        ("arq-sofa", "Sofá (planta)", sofa),
        (
            "arq-mesa-sillas",
            "Mesa con seis sillas (planta)",
            mesa_con_sillas,
        ),
        (
            "arq-mesa-redonda",
            "Mesa redonda con sillas (planta)",
            mesa_redonda,
        ),
        (
            "arq-escritorio",
            "Escritorio con silla (planta)",
            escritorio,
        ),
        ("arq-armario", "Armario (planta)", armario),
        ("arq-inodoro", "Inodoro (planta)", inodoro),
        ("arq-lavabo", "Lavabo (planta)", lavabo),
        ("arq-ducha", "Ducha (planta)", ducha),
        ("arq-banera", "Bañera (planta)", banera),
        ("arq-cocina", "Cocina con encimera (planta)", cocina),
        ("arq-frigorifico", "Frigorífico (planta)", frigorifico),
        ("arq-cama-iso", "Cama (isométrica)", cama_iso),
        ("arq-sofa-iso", "Sofá (isométrica)", sofa_iso),
        ("arq-mesa-iso", "Mesa con sillas (isométrica)", mesa_iso),
        (
            "arq-escritorio-iso",
            "Escritorio (isométrica)",
            escritorio_iso,
        ),
        ("arq-armario-iso", "Armario (isométrica)", armario_iso),
        (
            "arq-frigorifico-iso",
            "Frigorífico (isométrica)",
            frigorifico_iso,
        ),
        ("arq-cocina-iso", "Cocina (isométrica)", cocina_iso),
    ];
    poner(v, CATEGORIA, &lista);
}

// -------------------------------------------------------------------- planta

fn cama(l: &mut Lapiz, w: f32, h: f32, almohadas: usize) {
    l.rect(0.0, 0.0, w, h, Some(TINTA), Some(BLANCO), 2.0, false);
    l.rect(0.0, 0.0, w, 8.0, Some(TINTA), Some(GRIS), 1.25, false);
    let hueco = 8.0;
    let ancho = (w - hueco * (almohadas + 1) as f32) / almohadas as f32;
    for i in 0..almohadas {
        l.rect(
            hueco + (ancho + hueco) * i as f32,
            16.0,
            ancho,
            36.0,
            Some(FINA),
            Some(COJIN),
            1.25,
            true,
        );
    }
    // La sabana doblada en diagonal.
    l.poli(&[(0.0, 72.0), (w, 72.0)], FINA, 1.0, None);
    forma(
        l,
        &[(w * 0.55, 72.0), (w, 72.0), (w, 72.0 + w * 0.45)],
        FINA,
        Some(CLARO),
        1.0,
    );
}

fn sofa(l: &mut Lapiz) {
    let (w, h) = (220.0, 90.0);
    l.rect(0.0, 0.0, w, h, Some(TINTA), Some(COJIN), 2.0, true);
    l.rect(
        0.0,
        0.0,
        w,
        22.0,
        Some(TINTA),
        Some(hex(0xc4d1e3)),
        1.25,
        true,
    );
    l.rect(
        0.0,
        0.0,
        22.0,
        h,
        Some(TINTA),
        Some(hex(0xc4d1e3)),
        1.25,
        true,
    );
    l.rect(
        w - 22.0,
        0.0,
        22.0,
        h,
        Some(TINTA),
        Some(hex(0xc4d1e3)),
        1.25,
        true,
    );
    let ancho = (w - 44.0) / 3.0;
    for i in 0..3 {
        l.rect(
            22.0 + ancho * i as f32,
            22.0,
            ancho,
            h - 26.0,
            Some(FINA),
            Some(COJIN),
            1.0,
            true,
        );
    }
}

/// Una silla vista desde arriba, con el respaldo hacia `grados` (0 = arriba)
/// y su centro en `(cx, cy)`.
fn silla(l: &mut Lapiz, cx: f32, cy: f32, grados: f32) {
    let asiento = [
        (cx - 22.0, cy - 20.0),
        (cx + 22.0, cy - 20.0),
        (cx + 22.0, cy + 22.0),
        (cx - 22.0, cy + 22.0),
    ];
    forma(
        l,
        &girar(&asiento, (cx, cy), grados),
        TINTA,
        Some(BLANCO),
        1.25,
    );
    let respaldo = [
        (cx - 22.0, cy - 26.0),
        (cx + 22.0, cy - 26.0),
        (cx + 22.0, cy - 18.0),
        (cx - 22.0, cy - 18.0),
    ];
    forma(
        l,
        &girar(&respaldo, (cx, cy), grados),
        TINTA,
        Some(GRIS),
        1.25,
    );
}

fn mesa_con_sillas(l: &mut Lapiz) {
    let (x0, y0, w, h) = (50.0, 50.0, 180.0, 90.0);
    for x in [x0 + 45.0, x0 + w - 45.0] {
        silla(l, x, y0 - 12.0, 0.0);
        silla(l, x, y0 + h + 12.0, 180.0);
    }
    silla(l, x0 - 12.0, y0 + h / 2.0, -90.0);
    silla(l, x0 + w + 12.0, y0 + h / 2.0, 90.0);
    l.rect(x0, y0, w, h, Some(TINTA), Some(hex(0xf1e3cf)), 2.0, false);
}

fn mesa_redonda(l: &mut Lapiz) {
    let (c, r) = (110.0, 60.0);
    for i in 0..4 {
        let g = 45.0 + 90.0 * i as f32;
        let a = (g - 90.0f32).to_radians();
        silla(l, c + (r + 14.0) * a.cos(), c + (r + 14.0) * a.sin(), g);
    }
    l.circulo(c, c, r, Some(TINTA), Some(hex(0xf1e3cf)), 2.0);
}

fn escritorio(l: &mut Lapiz) {
    let (w, h) = (140.0, 70.0);
    l.rect(0.0, 0.0, w, h, Some(TINTA), Some(hex(0xf1e3cf)), 2.0, false);
    // El monitor, el teclado y la lampara.
    l.rect(40.0, 8.0, 60.0, 8.0, Some(TINTA), Some(GRIS), 1.25, true);
    l.rect(46.0, 34.0, 48.0, 14.0, Some(FINA), Some(BLANCO), 1.0, true);
    l.circulo(120.0, 18.0, 9.0, Some(FINA), Some(hex(0xfff3c4)), 1.0);
    // La silla de oficina: asiento redondo y respaldo curvo.
    l.circulo(70.0, h + 30.0, 24.0, Some(TINTA), Some(COJIN), 1.5);
    l.arco(70.0, h + 30.0, 30.0, 25.0, 155.0, TINTA, 4.0);
}

fn armario(l: &mut Lapiz) {
    let (w, h) = (150.0, 60.0);
    l.rect(0.0, 0.0, w, h, Some(TINTA), Some(BLANCO), 2.0, false);
    l.linea_con(
        6.0,
        h / 2.0,
        w - 6.0,
        h / 2.0,
        FINA,
        1.0,
        EstiloTrazo::Discontinuo,
    );
    for i in 0..7 {
        let x = 18.0 + 19.0 * i as f32;
        l.poli(
            &[(x - 6.0, h / 2.0 - 18.0), (x + 6.0, h / 2.0 + 18.0)],
            FINA,
            1.25,
            None,
        );
    }
    // Las puertas abatibles, a trazos.
    for (bx, desde, hasta) in [(0.0, 90.0, 0.0), (w, 90.0, 180.0)] {
        l.arco(bx, h, w / 2.0, desde, hasta, FINA, 0.75);
        if let Some(e) = l.v.last_mut() {
            e.estilo = EstiloTrazo::Discontinuo;
        }
    }
}

fn inodoro(l: &mut Lapiz) {
    l.rect(0.0, 0.0, 44.0, 18.0, Some(TINTA), Some(BLANCO), 1.75, true);
    l.elipse(4.0, 14.0, 36.0, 54.0, Some(TINTA), Some(BLANCO), 1.75);
    l.elipse(10.0, 24.0, 24.0, 36.0, Some(FINA), Some(AGUA), 1.0);
}

fn lavabo(l: &mut Lapiz) {
    l.rect(0.0, 0.0, 60.0, 48.0, Some(TINTA), Some(BLANCO), 1.75, true);
    l.elipse(8.0, 12.0, 44.0, 30.0, Some(FINA), Some(AGUA), 1.25);
    l.circulo(30.0, 7.0, 3.5, Some(TINTA), Some(GRIS), 1.0);
    l.circulo(30.0, 27.0, 2.0, Some(FINA), Some(FINA), 1.0);
}

fn ducha(l: &mut Lapiz) {
    let s = 90.0;
    l.rect(0.0, 0.0, s, s, Some(TINTA), Some(BLANCO), 2.0, false);
    l.rect(
        6.0,
        6.0,
        s - 12.0,
        s - 12.0,
        Some(FINA),
        Some(AGUA),
        1.0,
        false,
    );
    l.linea(6.0, 6.0, s - 6.0, s - 6.0, FINA, 0.75);
    l.linea(s - 6.0, 6.0, 6.0, s - 6.0, FINA, 0.75);
    l.circulo(s / 2.0, s / 2.0, 5.0, Some(TINTA), Some(GRIS), 1.0);
    l.circulo(s - 14.0, 14.0, 4.0, Some(TINTA), Some(GRIS), 1.0);
}

fn banera(l: &mut Lapiz) {
    let (w, h) = (170.0, 75.0);
    l.rect(0.0, 0.0, w, h, Some(TINTA), Some(BLANCO), 2.0, true);
    l.rect(
        10.0,
        9.0,
        w - 20.0,
        h - 18.0,
        Some(FINA),
        Some(AGUA),
        1.25,
        true,
    );
    l.circulo(w - 26.0, h / 2.0, 4.0, Some(TINTA), Some(GRIS), 1.0);
    l.circulo(w - 6.0, h / 2.0, 2.5, Some(TINTA), Some(TINTA), 1.0);
}

fn cocina(l: &mut Lapiz) {
    let (w, h) = (300.0, 62.0);
    l.rect(0.0, 0.0, w, h, Some(TINTA), Some(hex(0xf3f4f6)), 2.0, false);
    // El fregadero de dos senos con su grifo.
    for x in [30.0, 82.0] {
        l.rect(x, 10.0, 46.0, 42.0, Some(FINA), Some(AGUA), 1.25, true);
        l.circulo(x + 23.0, 31.0, 2.5, Some(FINA), Some(FINA), 1.0);
    }
    l.circulo(78.0, 6.0, 3.0, Some(TINTA), Some(GRIS), 1.0);
    // La placa de cuatro fuegos.
    l.rect(
        180.0,
        6.0,
        76.0,
        50.0,
        Some(TINTA),
        Some(hex(0x3a3f4b)),
        1.25,
        true,
    );
    for (cx, cy, r) in [
        (198.0, 20.0, 9.0),
        (238.0, 20.0, 11.0),
        (198.0, 43.0, 11.0),
        (238.0, 43.0, 9.0),
    ] {
        l.circulo(cx, cy, r, Some(GRIS), None, 1.25);
        l.circulo(cx, cy, r * 0.5, Some(GRIS), None, 1.0);
    }
    l.linea_con(
        0.0,
        h - 2.0,
        w,
        h - 2.0,
        FINA,
        0.75,
        EstiloTrazo::Discontinuo,
    );
}

fn frigorifico(l: &mut Lapiz) {
    let s = 70.0;
    l.rect(0.0, 0.0, s, s, Some(TINTA), Some(BLANCO), 2.0, false);
    l.linea(0.0, s - 8.0, s, s - 8.0, TINTA, 1.25);
    l.texto(
        s / 2.0,
        22.0,
        "REF",
        14.0,
        WORK_SANS,
        TINTA,
        true,
        Al::Centro,
    );
    l.arco(0.0, s, s, 0.0, 90.0, FINA, 0.75);
    if let Some(e) = l.v.last_mut() {
        e.estilo = EstiloTrazo::Discontinuo;
    }
}

// ---------------------------------------------------------------- isometrica

fn cama_iso(l: &mut Lapiz) {
    let o = (150.0, 70.0);
    caja_iso(l, o, 0.0, 0.0, 0.0, 110.0, 8.0, 70.0, MADERA);
    caja_iso(l, o, 0.0, 8.0, 0.0, 110.0, 150.0, 26.0, MADERA);
    caja_iso(l, o, 4.0, 10.0, 26.0, 102.0, 144.0, 14.0, BLANCA);
    caja_iso(l, o, 10.0, 16.0, 40.0, 40.0, 24.0, 8.0, TELA);
    caja_iso(l, o, 60.0, 16.0, 40.0, 40.0, 24.0, 8.0, TELA);
    // La colcha, que cae por los pies.
    caja_iso(l, o, 2.0, 70.0, 26.0, 106.0, 86.0, 16.0, TELA);
}

fn sofa_iso(l: &mut Lapiz) {
    let o = (100.0, 20.0);
    caja_iso(l, o, 0.0, 0.0, 0.0, 210.0, 85.0, 20.0, TELA);
    caja_iso(l, o, 0.0, 0.0, 20.0, 210.0, 20.0, 46.0, TELA);
    caja_iso(l, o, 0.0, 20.0, 20.0, 20.0, 65.0, 28.0, TELA);
    let ancho = 170.0 / 3.0;
    for i in 0..3 {
        caja_iso(
            l,
            o,
            20.0 + ancho * i as f32,
            20.0,
            20.0,
            ancho,
            65.0,
            16.0,
            BLANCA,
        );
    }
    caja_iso(l, o, 190.0, 20.0, 20.0, 20.0, 65.0, 28.0, TELA);
}

/// Una silla en isometrica; `respaldo_atras` la pone mirando hacia
/// delante (la de detras de la mesa).
fn silla_iso(l: &mut Lapiz, o: (f32, f32), x: f32, y: f32, respaldo_atras: bool) {
    if respaldo_atras {
        caja_iso(l, o, x, y, 45.0, 40.0, 4.0, 42.0, MADERA);
    }
    for (dx, dy) in [(0.0, 0.0), (36.0, 0.0), (0.0, 36.0), (36.0, 36.0)] {
        caja_iso(l, o, x + dx, y + dy, 0.0, 4.0, 4.0, 45.0, MADERA);
    }
    caja_iso(l, o, x, y, 45.0, 40.0, 40.0, 5.0, MADERA);
    if !respaldo_atras {
        caja_iso(l, o, x, y + 36.0, 50.0, 40.0, 4.0, 42.0, MADERA);
    }
}

fn mesa_iso(l: &mut Lapiz) {
    let o = (120.0, 120.0);
    silla_iso(l, o, 40.0, -50.0, true);
    for (dx, dy) in [(0.0, 0.0), (126.0, 0.0), (0.0, 76.0), (126.0, 76.0)] {
        caja_iso(l, o, dx, dy, 0.0, 4.0, 4.0, 72.0, MADERA);
    }
    caja_iso(l, o, 0.0, 0.0, 72.0, 130.0, 80.0, 5.0, MADERA);
    silla_iso(l, o, 40.0, 90.0, false);
}

fn escritorio_iso(l: &mut Lapiz) {
    let o = (100.0, 80.0);
    caja_iso(l, o, 0.0, 0.0, 0.0, 4.0, 70.0, 72.0, MADERA);
    caja_iso(l, o, 100.0, 0.0, 0.0, 40.0, 70.0, 72.0, MADERA);
    let p = |x: f32, y: f32, z: f32| iso(o, x, y, z);
    for z in [24.0, 48.0] {
        let (a, b) = (p(100.0, 70.0, z), p(140.0, 70.0, z));
        l.linea(a.0, a.1, b.0, b.1, TINTA, 1.0);
    }
    for z in [12.0, 36.0, 60.0] {
        let (a, b) = (p(114.0, 70.0, z), p(126.0, 70.0, z));
        l.linea(a.0, a.1, b.0, b.1, TINTA, 2.0);
    }
    caja_iso(l, o, 0.0, 0.0, 72.0, 140.0, 70.0, 4.0, MADERA);
    // El monitor con su pie.
    caja_iso(l, o, 60.0, 14.0, 76.0, 20.0, 12.0, 3.0, HORMIGON);
    caja_iso(l, o, 68.0, 18.0, 79.0, 4.0, 4.0, 12.0, HORMIGON);
    let pantalla = Caras {
        arriba: hex(0x3a3f4b),
        izq: hex(0x4a6a8a),
        der: hex(0x2b2f3a),
    };
    caja_iso(l, o, 40.0, 16.0, 88.0, 60.0, 4.0, 36.0, pantalla);
}

fn armario_iso(l: &mut Lapiz) {
    let o = (70.0, 20.0);
    caja_iso(l, o, 0.0, 0.0, 0.0, 120.0, 60.0, 200.0, MADERA);
    let p = |x: f32, z: f32| iso(o, x, 60.0, z);
    let (a, b) = (p(60.0, 0.0), p(60.0, 200.0));
    l.linea(a.0, a.1, b.0, b.1, TINTA, 1.25);
    for x in [54.0, 66.0] {
        let (a, b) = (p(x, 90.0), p(x, 120.0));
        l.linea(a.0, a.1, b.0, b.1, TINTA, 2.5);
    }
}

fn frigorifico_iso(l: &mut Lapiz) {
    let o = (70.0, 20.0);
    caja_iso(l, o, 0.0, 0.0, 0.0, 70.0, 65.0, 180.0, BLANCA);
    let p = |x: f32, z: f32| iso(o, x, 65.0, z);
    let (a, b) = (p(0.0, 120.0), p(70.0, 120.0));
    l.linea(a.0, a.1, b.0, b.1, TINTA, 1.25);
    for (z0, z1) in [(130.0, 160.0), (70.0, 108.0)] {
        let (a, b) = (p(62.0, z0), p(62.0, z1));
        l.linea(a.0, a.1, b.0, b.1, FINA, 3.0);
    }
}

fn cocina_iso(l: &mut Lapiz) {
    let o = (80.0, 20.0);
    caja_iso(l, o, 0.0, 0.0, 0.0, 240.0, 60.0, 86.0, BLANCA);
    caja_iso(l, o, 0.0, 0.0, 86.0, 240.0, 62.0, 4.0, HORMIGON);
    let p = |x: f32, y: f32, z: f32| iso(o, x, y, z);
    // Las puertas de los muebles bajos.
    for i in 1..4 {
        let x = 60.0 * i as f32;
        let (a, b) = (p(x, 60.0, 4.0), p(x, 60.0, 80.0));
        l.linea(a.0, a.1, b.0, b.1, TINTA, 1.0);
    }
    // El fregadero, hundido en la encimera.
    let z = 90.0;
    forma(
        l,
        &[
            p(20.0, 10.0, z),
            p(80.0, 10.0, z),
            p(80.0, 50.0, z),
            p(20.0, 50.0, z),
        ],
        TINTA,
        Some(AGUA),
        1.25,
    );
    // Los fuegos: circulos en el plano de arriba (elipses tumbadas).
    for (x, y) in [(150.0, 18.0), (190.0, 18.0), (150.0, 44.0), (190.0, 44.0)] {
        let (cx, cy) = p(x, y, z);
        let r = 10.0;
        l.elipse(
            cx - r * 1.2247,
            cy - r * std::f32::consts::FRAC_1_SQRT_2,
            2.0 * r * 1.2247,
            2.0 * r * std::f32::consts::FRAC_1_SQRT_2,
            Some(TINTA),
            Some(hex(0x3a3f4b)),
            1.0,
        );
    }
}
