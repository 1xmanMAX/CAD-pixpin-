//! **La vegetacion** de los planos y alzados: arboles en planta (copa en
//! feston, de ramas, con sombra, conifera, esquematico y palmera), en
//! alzado, en isometrica y en corte con sus raices; arbustos, seto y cesped.
//! Verdes suaves con trazo verde oscuro, para que se lean en blanco y negro
//! y en color.

use super::tecnico::{Dibujo, TINTA, copa, forma, girar, iso, poner, rayado};
use super::{Lapiz, hex};
use crate::biblioteca::FiguraGuardada;
use crate::elemento::{ColorRgba, EstiloTrazo};

pub(super) const CATEGORIA: &str = "Vegetación";

const VERDE: ColorRgba = hex(0x4f7d3f);
const HOJA: ColorRgba = hex(0xd7e8cb);
const HOJA_MEDIA: ColorRgba = hex(0xa9cc93);
const HOJA_OSCURA: ColorRgba = hex(0x6f9e5a);
const TRONCO: ColorRgba = hex(0x7d5f45);
const SOMBRA: ColorRgba = hex(0xd5d9df);
const TIERRA: ColorRgba = hex(0xe9dcc8);
const TIERRA_RAYA: ColorRgba = hex(0xb79c78);

pub(super) fn todas(v: &mut Vec<FiguraGuardada>) {
    let lista: [Dibujo; 17] = [
        (
            "arq-arbol-planta-copa",
            "Árbol en planta (copa)",
            arbol_planta_copa,
        ),
        (
            "arq-arbol-planta-ramas",
            "Árbol en planta (ramas)",
            arbol_planta_ramas,
        ),
        (
            "arq-arbol-planta-sombra",
            "Árbol en planta (con sombra)",
            arbol_planta_sombra,
        ),
        (
            "arq-arbol-planta-conifera",
            "Conífera en planta",
            conifera_planta,
        ),
        (
            "arq-arbol-planta-simple",
            "Árbol en planta (esquemático)",
            arbol_planta_simple,
        ),
        ("arq-palmera-planta", "Palmera en planta", palmera_planta),
        (
            "arq-arbol-alzado",
            "Árbol en alzado (frondoso)",
            arbol_alzado,
        ),
        ("arq-conifera-alzado", "Conífera en alzado", conifera_alzado),
        (
            "arq-arbol-alzado-ramas",
            "Árbol en alzado (ramas)",
            arbol_alzado_ramas,
        ),
        ("arq-palmera-alzado", "Palmera en alzado", palmera_alzado),
        ("arq-arbol-iso", "Árbol en isométrica", arbol_iso),
        (
            "arq-arbol-corte",
            "Árbol en corte (con raíces)",
            arbol_corte,
        ),
        ("arq-arbustos-planta", "Arbustos en planta", arbustos_planta),
        ("arq-arbusto-alzado", "Arbusto en alzado", arbusto_alzado),
        ("arq-seto-planta", "Seto en planta", seto_planta),
        ("arq-cesped-planta", "Césped (planta)", cesped_planta),
        ("arq-cesped-alzado", "Césped (alzado)", cesped_alzado),
    ];
    poner(v, CATEGORIA, &lista);
}

/// Un azar fijo (el mismo dibujo siempre).
struct Azar(u32);

impl Azar {
    /// De 0 a 1.
    fn sig(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }
}

// -------------------------------------------------------------------- planta

fn arbol_planta_copa(l: &mut Lapiz) {
    copa(l, 100.0, 100.0, 100.0, 100.0, 9, VERDE, Some(HOJA), 1.75);
    copa(l, 100.0, 100.0, 64.0, 64.0, 7, HOJA_OSCURA, None, 1.0);
    l.circulo(100.0, 100.0, 4.0, Some(TRONCO), Some(TRONCO), 1.0);
}

fn arbol_planta_ramas(l: &mut Lapiz) {
    let (c, r) = (100.0, 100.0);
    l.circulo(c, c, r, Some(VERDE), Some(hex(0xf1f7ec)), 1.25);
    let p = |a: f32, d: f32| {
        let a = a.to_radians();
        (c + d * a.cos(), c + d * a.sin())
    };
    for i in 0..8 {
        let a = 22.5 + 45.0 * i as f32;
        let (x, y) = p(a, r * 0.86);
        l.linea(c, c, x, y, TRONCO, 1.5);
        let medio = p(a, r * 0.45);
        for lado in [-28.0, 28.0] {
            let fin = p(a + lado, r * 0.78);
            l.linea(medio.0, medio.1, fin.0, fin.1, TRONCO, 1.0);
        }
    }
    l.circulo(c, c, 6.0, Some(TRONCO), Some(TRONCO), 1.0);
}

fn arbol_planta_sombra(l: &mut Lapiz) {
    copa(l, 122.0, 118.0, 92.0, 92.0, 8, SOMBRA, Some(SOMBRA), 1.0);
    copa(
        l,
        100.0,
        100.0,
        92.0,
        92.0,
        8,
        VERDE,
        Some(HOJA_MEDIA),
        1.75,
    );
    // Los brillos de la copa, arriba a la izquierda.
    l.arco(100.0, 100.0, 62.0, 190.0, 260.0, BLANCO_VERDE, 3.0);
    l.arco(100.0, 100.0, 40.0, 200.0, 250.0, BLANCO_VERDE, 2.5);
    l.circulo(100.0, 100.0, 4.0, Some(TRONCO), Some(TRONCO), 1.0);
}

const BLANCO_VERDE: ColorRgba = hex(0xeef6e8);

fn conifera_planta(l: &mut Lapiz) {
    let estrella = |r: f32, n: usize| -> Vec<(f32, f32)> {
        (0..2 * n)
            .map(|i| {
                let a = std::f32::consts::PI * i as f32 / n as f32;
                let d = if i % 2 == 0 { r } else { r * 0.62 };
                (90.0 + d * a.cos(), 90.0 + d * a.sin())
            })
            .collect()
    };
    forma(l, &estrella(90.0, 16), VERDE, Some(HOJA), 1.5);
    forma(
        l,
        &girar(&estrella(58.0, 12), (90.0, 90.0), 8.0),
        HOJA_OSCURA,
        Some(HOJA_MEDIA),
        1.0,
    );
    for i in 0..16 {
        let a = (360.0 / 16.0 * i as f32).to_radians();
        l.linea(
            90.0,
            90.0,
            90.0 + 80.0 * a.cos(),
            90.0 + 80.0 * a.sin(),
            HOJA_OSCURA,
            0.75,
        );
    }
    l.circulo(90.0, 90.0, 4.0, Some(TRONCO), Some(TRONCO), 1.0);
}

fn arbol_planta_simple(l: &mut Lapiz) {
    l.circulo(80.0, 80.0, 80.0, Some(VERDE), Some(BLANCO_VERDE), 1.5);
    l.linea_con(0.0, 80.0, 160.0, 80.0, VERDE, 0.75, EstiloTrazo::RayaPunto);
    l.linea_con(80.0, 0.0, 80.0, 160.0, VERDE, 0.75, EstiloTrazo::RayaPunto);
    l.circulo(80.0, 80.0, 10.0, Some(TRONCO), Some(hex(0xe9d9c8)), 1.5);
}

/// Una hoja alargada (de palmera) de largo `largo` desde `(x, y)` hacia
/// `grados`, con su nervio.
fn fronda(l: &mut Lapiz, x: f32, y: f32, largo: f32, ancho: f32, grados: f32, curva: f32) {
    let n = 12;
    let mut arriba = Vec::new();
    let mut abajo = Vec::new();
    let mut nervio = Vec::new();
    for i in 0..=n {
        let t = i as f32 / n as f32;
        let w = ancho * (std::f32::consts::PI * t).sin() * (1.0 - 0.3 * t);
        let flecha = curva * t * t;
        arriba.push((x + largo * t, y - w + flecha));
        abajo.push((x + largo * t, y + w + flecha));
        nervio.push((x + largo * t, y + flecha));
    }
    abajo.reverse();
    arriba.extend(abajo);
    forma(
        l,
        &girar(&arriba, (x, y), grados),
        VERDE,
        Some(HOJA_MEDIA),
        1.25,
    );
    l.poli(&girar(&nervio, (x, y), grados), VERDE, 1.0, None);
}

fn palmera_planta(l: &mut Lapiz) {
    for i in 0..9 {
        fronda(l, 100.0, 100.0, 96.0, 14.0, 40.0 * i as f32 + 8.0, 10.0);
    }
    l.circulo(100.0, 100.0, 9.0, Some(TRONCO), Some(hex(0xc9a57f)), 1.5);
}

// -------------------------------------------------------------------- alzado

fn tronco(l: &mut Lapiz, x: f32, base: f32, arriba: f32, ancho: f32) {
    forma(
        l,
        &[
            (x - ancho, base),
            (x - ancho * 0.5, base - 12.0),
            (x - ancho * 0.45, arriba),
            (x + ancho * 0.45, arriba),
            (x + ancho * 0.5, base - 12.0),
            (x + ancho, base),
        ],
        TRONCO,
        Some(hex(0xa7835f)),
        1.5,
    );
}

fn arbol_alzado(l: &mut Lapiz) {
    tronco(l, 110.0, 320.0, 150.0, 12.0);
    l.poli(&[(110.0, 210.0), (78.0, 170.0)], TRONCO, 4.0, None);
    l.poli(&[(110.0, 190.0), (146.0, 150.0)], TRONCO, 4.0, None);
    copa(l, 110.0, 120.0, 105.0, 95.0, 11, VERDE, Some(HOJA), 1.75);
    copa(l, 70.0, 140.0, 38.0, 32.0, 6, HOJA_OSCURA, None, 1.0);
    copa(l, 146.0, 104.0, 40.0, 34.0, 6, HOJA_OSCURA, None, 1.0);
    copa(l, 104.0, 70.0, 34.0, 28.0, 6, HOJA_OSCURA, None, 1.0);
    l.linea(20.0, 320.0, 200.0, 320.0, TINTA, 1.5);
}

fn conifera_alzado(l: &mut Lapiz) {
    l.rect(
        52.0,
        260.0,
        16.0,
        40.0,
        Some(TRONCO),
        Some(hex(0xa7835f)),
        1.5,
        false,
    );
    let pisos = [
        (0.0, 120.0, 60.0),
        (70.0, 200.0, 50.0),
        (140.0, 270.0, 40.0),
    ];
    for (i, (arriba, abajo, medio)) in pisos.iter().rev().enumerate() {
        let relleno = if i % 2 == 0 { HOJA } else { HOJA_MEDIA };
        forma(
            l,
            &[
                (60.0, *arriba),
                (60.0 + medio, *abajo),
                (60.0 - medio, *abajo),
            ],
            VERDE,
            Some(relleno),
            1.5,
        );
    }
    l.linea(0.0, 300.0, 120.0, 300.0, TINTA, 1.5);
}

/// Una rama y sus hijas, hasta `nivel` 0.
fn rama(l: &mut Lapiz, x: f32, y: f32, grados: f32, largo: f32, nivel: u32) {
    let a = grados.to_radians();
    let (x2, y2) = (x + largo * a.cos(), y + largo * a.sin());
    l.linea(x, y, x2, y2, TRONCO, 0.8 + nivel as f32 * 0.9);
    if nivel > 0 {
        rama(l, x2, y2, grados - 26.0, largo * 0.72, nivel - 1);
        rama(l, x2, y2, grados + 24.0, largo * 0.7, nivel - 1);
    }
}

fn arbol_alzado_ramas(l: &mut Lapiz) {
    l.elipse(
        10.0,
        0.0,
        200.0,
        190.0,
        Some(HOJA_OSCURA),
        Some(BLANCO_VERDE),
        1.0,
    );
    if let Some(e) = l.v.last_mut() {
        e.estilo = EstiloTrazo::Discontinuo;
    }
    l.linea(110.0, 300.0, 110.0, 190.0, TRONCO, 5.0);
    rama(l, 110.0, 190.0, -90.0, 56.0, 4);
    l.linea(30.0, 300.0, 190.0, 300.0, TINTA, 1.5);
}

fn palmera_alzado(l: &mut Lapiz) {
    // El tronco curvo, con sus anillos.
    let tronco: Vec<(f32, f32)> = (0..=10)
        .map(|i| {
            let t = i as f32 / 10.0;
            (120.0 - 20.0 * (t * 1.6).sin() + 6.0 * t, 330.0 - 250.0 * t)
        })
        .collect();
    l.poli(&tronco, TRONCO, 9.0, None);
    for (x, y) in tronco.iter().skip(1).take(8) {
        l.linea(x - 5.0, *y, x + 5.0, y + 2.0, hex(0x5e4633), 1.0);
    }
    let (cx, cy) = tronco[10];
    for (g, c) in [
        (200.0, 50.0),
        (225.0, 30.0),
        (250.0, 14.0),
        (290.0, 14.0),
        (315.0, 30.0),
        (340.0, 50.0),
        (160.0, 60.0),
        (20.0, 60.0),
    ] {
        // La hoja cae hacia abajo a los dos lados.
        let c = if (90.0..270.0).contains(&g) { -c } else { c };
        fronda(l, cx, cy, 100.0, 11.0, g, c);
    }
    l.circulo(cx, cy, 6.0, Some(TRONCO), Some(hex(0xc9a57f)), 1.0);
    l.linea(40.0, 330.0, 200.0, 330.0, TINTA, 1.5);
}

// --------------------------------------------------------- isometrica y corte

fn arbol_iso(l: &mut Lapiz) {
    let o = (120.0, 200.0);
    // El alcorque de cesped, un rombo en el suelo.
    let p = |x: f32, y: f32| iso(o, x, y, 0.0);
    forma(
        l,
        &[
            p(-60.0, -60.0),
            p(60.0, -60.0),
            p(60.0, 60.0),
            p(-60.0, 60.0),
        ],
        VERDE,
        Some(HOJA),
        1.25,
    );
    l.elipse(
        o.0 - 34.0,
        o.1 - 10.0,
        90.0,
        30.0,
        None,
        Some(HOJA_OSCURA),
        1.0,
    );
    // El tronco: un cilindro chico.
    l.rect(
        o.0 - 7.0,
        o.1 - 90.0,
        14.0,
        90.0,
        Some(TRONCO),
        Some(hex(0xa7835f)),
        1.25,
        false,
    );
    l.elipse(
        o.0 - 7.0,
        o.1 - 4.0,
        14.0,
        8.0,
        Some(TRONCO),
        Some(hex(0x8a6a4f)),
        1.0,
    );
    // La copa: una esfera con su lado en sombra.
    let (cx, cy, r) = (o.0, o.1 - 140.0, 70.0);
    l.circulo(cx, cy, r, Some(VERDE), Some(HOJA_MEDIA), 1.75);
    let mut sombra: Vec<(f32, f32)> = (0..=18)
        .map(|i| {
            let a = (-30.0 + 180.0 * i as f32 / 18.0f32).to_radians();
            (cx + r * a.cos(), cy + r * a.sin())
        })
        .collect();
    sombra.extend((0..=18).rev().map(|i| {
        let a = (-30.0 + 180.0 * i as f32 / 18.0f32).to_radians();
        (
            cx + 12.0 + r * 0.78 * a.cos(),
            cy - 8.0 + r * 0.82 * a.sin(),
        )
    }));
    forma(l, &sombra, HOJA_OSCURA, Some(HOJA_OSCURA), 1.0);
    l.arco(cx, cy, r * 0.62, 200.0, 250.0, BLANCO_VERDE, 4.0);
}

fn arbol_corte(l: &mut Lapiz) {
    let suelo = 240.0;
    l.rect(0.0, suelo, 240.0, 110.0, None, Some(TIERRA), 1.0, false);
    rayado(l, 0.0, suelo, 240.0, 110.0, 12.0, TIERRA_RAYA, 0.75);
    l.linea(0.0, suelo, 240.0, suelo, TINTA, 2.5);
    // Las raices, a trazos bajo tierra.
    for (dx, dy) in [
        (-80.0, 40.0),
        (-50.0, 80.0),
        (-10.0, 100.0),
        (30.0, 90.0),
        (70.0, 60.0),
        (90.0, 26.0),
    ] {
        let pts = [
            (120.0, suelo),
            (120.0 + dx * 0.5, suelo + dy * 0.6),
            (120.0 + dx, suelo + dy),
        ];
        l.poli(&pts, TRONCO, 2.0, None);
        if let Some(e) = l.v.last_mut() {
            e.estilo = EstiloTrazo::Discontinuo;
        }
    }
    tronco(l, 120.0, suelo, 120.0, 13.0);
    copa(l, 120.0, 100.0, 100.0, 90.0, 11, VERDE, Some(HOJA), 1.75);
    copa(l, 90.0, 120.0, 36.0, 30.0, 6, HOJA_OSCURA, None, 1.0);
    copa(l, 150.0, 80.0, 38.0, 32.0, 6, HOJA_OSCURA, None, 1.0);
}

// ------------------------------------------------------- arbustos y cesped

fn arbustos_planta(l: &mut Lapiz) {
    for (cx, cy, r, relleno) in [
        (60.0, 70.0, 56.0, HOJA),
        (150.0, 52.0, 46.0, HOJA_MEDIA),
        (130.0, 130.0, 40.0, HOJA),
    ] {
        copa(l, cx, cy, r, r, 8, VERDE, Some(relleno), 1.5);
        l.circulo(cx, cy, 2.5, Some(VERDE), Some(VERDE), 1.0);
    }
}

fn arbusto_alzado(l: &mut Lapiz) {
    copa(l, 70.0, 74.0, 66.0, 50.0, 9, VERDE, Some(HOJA), 1.5);
    copa(l, 150.0, 82.0, 54.0, 42.0, 8, VERDE, Some(HOJA_MEDIA), 1.5);
    l.linea(0.0, 122.0, 220.0, 122.0, TINTA, 1.5);
}

fn seto_planta(l: &mut Lapiz) {
    let (largo, ancho, n) = (320.0, 50.0, 16);
    let paso = largo / n as f32;
    let r = paso / 2.0;
    // El relleno: el cuerpo, los festones y las puntas redondas.
    l.rect(0.0, 0.0, largo, ancho, None, Some(HOJA_MEDIA), 1.0, false);
    for i in 0..n {
        let cx = paso * (i as f32 + 0.5);
        l.circulo(cx, 0.0, r, None, Some(HOJA_MEDIA), 1.0);
        l.circulo(cx, ancho, r, None, Some(HOJA_MEDIA), 1.0);
    }
    l.circulo(0.0, ancho / 2.0, ancho / 2.0, None, Some(HOJA_MEDIA), 1.0);
    l.circulo(largo, ancho / 2.0, ancho / 2.0, None, Some(HOJA_MEDIA), 1.0);
    // El borde.
    for i in 0..n {
        let cx = paso * (i as f32 + 0.5);
        l.arco(cx, 0.0, r, 180.0, 360.0, VERDE, 1.5);
        l.arco(cx, ancho, r, 0.0, 180.0, VERDE, 1.5);
    }
    l.arco(0.0, ancho / 2.0, ancho / 2.0, 90.0, 270.0, VERDE, 1.5);
    l.arco(largo, ancho / 2.0, ancho / 2.0, -90.0, 90.0, VERDE, 1.5);
    let mut a = Azar(7);
    for _ in 0..40 {
        let (x, y) = (a.sig() * largo, 10.0 + a.sig() * (ancho - 20.0));
        l.arco(x, y, 5.0, 180.0, 330.0, HOJA_OSCURA, 1.0);
    }
}

/// Una mata de hierba: tres o cuatro hojas desde `(x, y)`.
fn mata(l: &mut Lapiz, x: f32, y: f32, alto: f32, c: ColorRgba) {
    for (dx, k) in [(-5.0, 0.7), (-1.0, 1.0), (4.0, 0.8)] {
        l.poli(
            &[
                (x, y),
                (x + dx * 0.4, y - alto * k * 0.5),
                (x + dx, y - alto * k),
            ],
            c,
            1.0,
            None,
        );
    }
}

fn cesped_planta(l: &mut Lapiz) {
    let (w, h) = (260.0, 160.0);
    l.rect(
        0.0,
        0.0,
        w,
        h,
        Some(VERDE),
        Some(hex(0xeef6e8)),
        1.25,
        false,
    );
    if let Some(e) = l.v.last_mut() {
        e.estilo = EstiloTrazo::Discontinuo;
    }
    let mut a = Azar(42);
    let (cols, filas) = (9, 6);
    for f in 0..filas {
        for c in 0..cols {
            let x = 18.0 + (w - 36.0) * c as f32 / (cols - 1) as f32 + (a.sig() - 0.5) * 14.0;
            let y = 26.0 + (h - 40.0) * f as f32 / (filas - 1) as f32 + (a.sig() - 0.5) * 10.0;
            mata(
                l,
                x,
                y,
                9.0 + a.sig() * 5.0,
                if (c + f) % 3 == 0 { HOJA_OSCURA } else { VERDE },
            );
        }
    }
}

fn cesped_alzado(l: &mut Lapiz) {
    let w = 300.0;
    let mut a = Azar(3);
    let mut x = 4.0;
    while x < w - 4.0 {
        mata(
            l,
            x,
            40.0,
            14.0 + a.sig() * 14.0,
            if a.sig() > 0.6 { HOJA_OSCURA } else { VERDE },
        );
        x += 8.0 + a.sig() * 6.0;
    }
    l.linea(0.0, 40.0, w, 40.0, TINTA, 1.5);
}
