//! **Medicina**: siluetas del cuerpo para marcar (de frente y de
//! espaldas), organos esquematicos con sus partes rotuladas (corazon,
//! pulmones, rinon, estomago, cerebro, ojo), el esqueleto, una vertebra,
//! un trazado de ECG y dos hojas: la historia clinica y la nota SOAP.

use super::super::FRAUNCES;
use super::super::plantillas::{Pag, Tema, W};
use super::*;
use crate::elemento::EstiloTrazo;

const CATEGORIA: &str = "Medicina";
const PREFIJO: &str = "salud-";
const PIEL: ColorRgba = hex(0xf6e3d5);
const PIEL_BORDE: ColorRgba = hex(0x9a6a52);
const HUESO: ColorRgba = hex(0xf4efe4);
const MUSCULO: ColorRgba = hex(0xf3c6c0);
const ROJO_OSCURO: ColorRgba = hex(0xa63a3a);
const AZUL_VENA: ColorRgba = hex(0x4a78c2);
const AZUL_CLARO: ColorRgba = hex(0xc9dcf5);
const ROJO_CLARO: ColorRgba = hex(0xf7c9c9);
const ORGANO: ColorRgba = hex(0xf6c1c7);
const ORGANO_BORDE: ColorRgba = hex(0xc0566a);

/// El color de las hojas de medicina.
const MEDICINA: Tema = Tema {
    categoria: CATEGORIA,
    acento: hex(0xc94b4b),
    suave: hex(0xfbe9e9),
    titulo: FRAUNCES,
};
const M: f32 = 48.0;
const Y0: f32 = 128.0;

pub(super) fn todas(v: &mut Vec<FiguraGuardada>) {
    let lista: [Dibujo; 11] = [
        (
            "silueta-frontal",
            "Silueta corporal (vista anterior)",
            |l| silueta(l, true),
        ),
        (
            "silueta-posterior",
            "Silueta corporal (vista posterior)",
            |l| silueta(l, false),
        ),
        ("corazon", "Corazón con sus cavidades", corazon),
        ("pulmones", "Pulmones", pulmones),
        ("esqueleto", "Esqueleto simplificado", esqueleto),
        ("vertebra", "Vértebra (vista superior)", vertebra),
        ("ojo", "Ojo en corte", ojo),
        ("ecg", "Electrocardiograma (ECG)", ecg),
        ("rinon", "Riñón en corte", rinon),
        ("estomago", "Estómago", estomago),
        ("encefalo", "Encéfalo (vista lateral)", encefalo),
    ];
    meter(v, PREFIJO, CATEGORIA, &lista);
    v.push(historia_clinica());
    v.push(nota_soap());
}

// -------------------------------------------------------------- siluetas

/// La mitad derecha (de quien mira) de la silueta, del cuello a la
/// entrepierna, con el eje en `x = 0`.
const MITAD: [(f32, f32); 30] = [
    (12.0, 62.0),
    (24.0, 74.0),
    (56.0, 84.0),
    (66.0, 98.0),
    (70.0, 150.0),
    (74.0, 206.0),
    (80.0, 262.0),
    (86.0, 286.0),
    (82.0, 300.0),
    (72.0, 302.0),
    (66.0, 290.0),
    (62.0, 262.0),
    (56.0, 206.0),
    (50.0, 156.0),
    (44.0, 122.0),
    (42.0, 170.0),
    (38.0, 206.0),
    (46.0, 244.0),
    (48.0, 272.0),
    (44.0, 340.0),
    (40.0, 404.0),
    (36.0, 446.0),
    (48.0, 462.0),
    (44.0, 470.0),
    (14.0, 470.0),
    (10.0, 446.0),
    (12.0, 404.0),
    (14.0, 340.0),
    (6.0, 278.0),
    (0.0, 270.0),
];

fn silueta(l: &mut Lapiz, de_frente: bool) {
    let cx = 110.0;
    let derecha: Vec<(f32, f32)> = MITAD.iter().map(|(x, y)| (cx + x, *y)).collect();
    let mut cuerpo = derecha.clone();
    cuerpo.extend(espejo(&derecha, cx));
    forma(l, &suavizar(&cuerpo, 2), PIEL_BORDE, Some(PIEL), G);
    l.elipse(cx - 24.0, 4.0, 48.0, 60.0, Some(PIEL_BORDE), Some(PIEL), G);
    if de_frente {
        l.arco_eliptico(cx - 22.0, 82.0, 18.0, 5.0, 200.0, 340.0, PIEL_BORDE, 1.2);
        l.arco_eliptico(cx + 22.0, 82.0, 18.0, 5.0, 200.0, 340.0, PIEL_BORDE, 1.2);
        l.circulo(cx, 214.0, 2.5, None, Some(PIEL_BORDE), 1.0);
    } else {
        l.linea_con(
            cx,
            70.0,
            cx,
            252.0,
            PIEL_BORDE,
            1.2,
            EstiloTrazo::Discontinuo,
        );
        l.arco_eliptico(cx - 24.0, 118.0, 14.0, 20.0, 100.0, 260.0, PIEL_BORDE, 1.2);
        l.arco_eliptico(cx + 24.0, 118.0, 14.0, 20.0, -80.0, 80.0, PIEL_BORDE, 1.2);
        l.linea(cx, 256.0, cx, 274.0, PIEL_BORDE, 1.2);
    }
    let (izq, der) = if de_frente { ("D", "I") } else { ("I", "D") };
    txt_n(l, 8.0, 160.0, izq, 14.0, GRIS);
    txt_n(l, 2.0 * cx - 8.0, 160.0, der, 14.0, GRIS);
    txt(
        l,
        cx,
        492.0,
        if de_frente {
            "vista anterior"
        } else {
            "vista posterior"
        },
        11.0,
        GRIS,
    );
}

// --------------------------------------------------------------- corazon

fn corazon(l: &mut Lapiz) {
    let musculo = [
        (40.0, 84.0),
        (150.0, 78.0),
        (170.0, 80.0),
        (286.0, 84.0),
        (294.0, 180.0),
        (256.0, 264.0),
        (160.0, 332.0),
        (64.0, 264.0),
        (28.0, 180.0),
    ];
    blanda(l, &musculo, ROJO_OSCURO, Some(MUSCULO), 2.5);
    // Los vasos, saliendo por arriba y por los lados.
    l.linea(80.0, 6.0, 80.0, 96.0, AZUL_VENA, 16.0);
    l.linea(0.0, 150.0, 48.0, 150.0, AZUL_VENA, 14.0);
    l.poli(
        &[(152.0, 210.0), (152.0, 50.0), (106.0, 50.0)],
        AZUL_VENA,
        12.0,
        None,
    );
    l.poli(
        &[
            (168.0, 210.0),
            (168.0, 38.0),
            (196.0, 16.0),
            (236.0, 18.0),
            (254.0, 46.0),
            (254.0, 86.0),
        ],
        ROJO,
        12.0,
        None,
    );
    l.linea(326.0, 112.0, 280.0, 112.0, ROJO, 9.0);
    l.linea(326.0, 150.0, 280.0, 150.0, ROJO, 9.0);
    // Las cuatro cavidades.
    l.rect(
        44.0,
        92.0,
        100.0,
        80.0,
        Some(AZUL_VENA),
        Some(AZUL_CLARO),
        G,
        true,
    );
    l.rect(
        176.0,
        92.0,
        104.0,
        80.0,
        Some(ROJO),
        Some(ROJO_CLARO),
        G,
        true,
    );
    forma(
        l,
        &[(44.0, 182.0), (154.0, 182.0), (154.0, 306.0), (70.0, 256.0)],
        AZUL_VENA,
        Some(AZUL_CLARO),
        G,
    );
    forma(
        l,
        &[
            (166.0, 182.0),
            (280.0, 182.0),
            (252.0, 256.0),
            (166.0, 306.0),
        ],
        ROJO,
        Some(ROJO_CLARO),
        G,
    );
    forma(
        l,
        &[(80.0, 168.0), (118.0, 168.0), (99.0, 194.0)],
        TINTA,
        Some(BLANCO),
        1.2,
    );
    forma(
        l,
        &[(208.0, 168.0), (248.0, 168.0), (228.0, 194.0)],
        TINTA,
        Some(BLANCO),
        1.2,
    );
    txt_n(l, 94.0, 128.0, "AD", 16.0, AZUL_VENA);
    txt_n(l, 228.0, 128.0, "AI", 16.0, ROJO);
    txt_n(l, 104.0, 230.0, "VD", 16.0, AZUL_VENA);
    txt_n(l, 214.0, 230.0, "VI", 16.0, ROJO);
    // Por donde va la sangre.
    l.flecha(&[(62.0, 20.0), (62.0, 56.0)], AZUL_VENA, 1.5, false);
    l.flecha(&[(4.0, 166.0), (34.0, 166.0)], AZUL_VENA, 1.5, false);
    l.flecha(&[(136.0, 36.0), (110.0, 36.0)], AZUL_VENA, 1.5, false);
    l.flecha(&[(272.0, 50.0), (272.0, 86.0)], ROJO, 1.5, false);
    l.flecha(&[(320.0, 130.0), (290.0, 130.0)], ROJO, 1.5, false);
    rotulos_izquierda(l, 30.0, 4.0, 0.0, &[((72.0, 24.0), "Vena cava superior")]);
    rotulo(
        l,
        (4.0, 150.0),
        (-6.0, 206.0),
        "Vena cava inferior",
        Al::Der,
    );
    rotulo(l, (108.0, 50.0), (24.0, 70.0), "Arteria pulmonar", Al::Der);
    rotulo(
        l,
        (99.0, 184.0),
        (-6.0, 290.0),
        "Válvula tricúspide",
        Al::Der,
    );
    rotulo(l, (236.0, 18.0), (340.0, 12.0), "Aorta", Al::Izq);
    rotulo(
        l,
        (322.0, 112.0),
        (340.0, 96.0),
        "Venas pulmonares",
        Al::Izq,
    );
    rotulo(l, (228.0, 184.0), (340.0, 204.0), "Válvula mitral", Al::Izq);
    rotulo(l, (160.0, 260.0), (340.0, 280.0), "Tabique", Al::Izq);
    txt(
        l,
        160.0,
        352.0,
        "AD: aurícula derecha · AI: aurícula izquierda",
        11.0,
        TINTA,
    );
    txt(
        l,
        160.0,
        370.0,
        "VD: ventrículo derecho · VI: ventrículo izquierdo",
        11.0,
        TINTA,
    );
}

// -------------------------------------------------------------- pulmones

fn pulmones(l: &mut Lapiz) {
    l.arco_eliptico(200.0, 334.0, 186.0, 26.0, 180.0, 360.0, ROJO_OSCURO, 3.0);
    let derecho = [
        (118.0, 64.0),
        (96.0, 72.0),
        (70.0, 110.0),
        (52.0, 170.0),
        (42.0, 240.0),
        (48.0, 292.0),
        (90.0, 304.0),
        (140.0, 298.0),
        (150.0, 270.0),
        (146.0, 150.0),
        (138.0, 92.0),
    ];
    let izquierdo = [
        (282.0, 64.0),
        (304.0, 72.0),
        (330.0, 110.0),
        (348.0, 170.0),
        (358.0, 240.0),
        (352.0, 292.0),
        (310.0, 304.0),
        (270.0, 298.0),
        (256.0, 278.0),
        (272.0, 250.0),
        (262.0, 222.0),
        (254.0, 150.0),
        (262.0, 92.0),
    ];
    blanda(l, &derecho, ORGANO_BORDE, Some(ORGANO), G);
    blanda(l, &izquierdo, ORGANO_BORDE, Some(ORGANO), G);
    // Las cisuras: dos en el derecho (tres lobulos), una en el izquierdo.
    l.linea_con(
        66.0,
        150.0,
        144.0,
        276.0,
        ORGANO_BORDE,
        1.5,
        EstiloTrazo::Discontinuo,
    );
    l.linea_con(
        104.0,
        212.0,
        148.0,
        212.0,
        ORGANO_BORDE,
        1.5,
        EstiloTrazo::Discontinuo,
    );
    l.linea_con(
        338.0,
        140.0,
        262.0,
        262.0,
        ORGANO_BORDE,
        1.5,
        EstiloTrazo::Discontinuo,
    );
    // La traquea con sus anillos y los bronquios.
    l.rect(190.0, 0.0, 20.0, 112.0, Some(TINTA), Some(CLARO), 1.5, true);
    for i in 1..9 {
        let y = 12.0 * i as f32;
        l.linea(190.0, y, 210.0, y, GRIS, 1.0);
    }
    let bronquio = hex(0x8a94a6);
    l.poli(
        &[(200.0, 108.0), (150.0, 150.0), (124.0, 176.0)],
        bronquio,
        8.0,
        None,
    );
    l.poli(
        &[(200.0, 108.0), (250.0, 150.0), (276.0, 176.0)],
        bronquio,
        8.0,
        None,
    );
    for (a, b) in [
        ((150.0, 150.0), (110.0, 120.0)),
        ((124.0, 176.0), (96.0, 230.0)),
        ((124.0, 176.0), (80.0, 180.0)),
        ((250.0, 150.0), (292.0, 120.0)),
        ((276.0, 176.0), (306.0, 232.0)),
        ((276.0, 176.0), (324.0, 184.0)),
    ] {
        l.linea(a.0, a.1, b.0, b.1, bronquio, 3.0);
    }
    rotulos_izquierda(
        l,
        24.0,
        90.0,
        56.0,
        &[
            ((96.0, 110.0), "Lóbulo superior"),
            ((60.0, 190.0), "Pulmón derecho"),
            ((126.0, 240.0), "Lóbulo medio"),
            ((80.0, 280.0), "Lóbulo inferior"),
        ],
    );
    rotulos_derecha(
        l,
        380.0,
        30.0,
        64.0,
        &[
            ((210.0, 40.0), "Tráquea"),
            ((236.0, 138.0), "Bronquio principal"),
            ((330.0, 200.0), "Pulmón izquierdo"),
            ((350.0, 318.0), "Diafragma"),
        ],
    );
}

// ----------------------------------------------------------------- huesos

/// Un hueso largo de `a` a `b`: borde oscuro, cuerpo claro y una cabeza
/// en cada punta.
fn hueso(l: &mut Lapiz, a: (f32, f32), b: (f32, f32), g: f32) {
    l.linea(a.0, a.1, b.0, b.1, TINTA, g + 3.0);
    for p in [a, b] {
        l.circulo(p.0, p.1, g * 0.8, Some(TINTA), Some(HUESO), 1.5);
    }
    l.linea(a.0, a.1, b.0, b.1, HUESO, g);
}

fn esqueleto(l: &mut Lapiz) {
    let cx = 100.0;
    // Columna y costillas.
    for i in 0..15 {
        l.rect(
            cx - 5.0,
            64.0 + 13.0 * i as f32,
            10.0,
            10.0,
            Some(TINTA),
            Some(HUESO),
            1.2,
            true,
        );
    }
    for i in 0..7 {
        let i = i as f32;
        l.arco_eliptico(
            cx,
            98.0 + 15.0 * i,
            44.0 - 1.5 * i,
            18.0,
            200.0,
            340.0,
            TINTA,
            5.5,
        );
        l.arco_eliptico(
            cx,
            98.0 + 15.0 * i,
            44.0 - 1.5 * i,
            18.0,
            200.0,
            340.0,
            HUESO,
            3.0,
        );
    }
    l.rect(
        cx - 4.0,
        84.0,
        8.0,
        74.0,
        Some(TINTA),
        Some(HUESO),
        1.2,
        true,
    );
    // Pelvis.
    blanda(
        l,
        &[
            (62.0, 246.0),
            (138.0, 246.0),
            (134.0, 282.0),
            (112.0, 296.0),
            (100.0, 284.0),
            (88.0, 296.0),
            (66.0, 282.0),
        ],
        TINTA,
        Some(HUESO),
        1.5,
    );
    l.circulo(cx, 268.0, 8.0, Some(TINTA), Some(BLANCO), 1.2);
    // Brazos y piernas, a los dos lados.
    for s in [-1.0f32, 1.0] {
        let x = |d: f32| cx + s * d;
        hueso(l, (x(6.0), 80.0), (x(40.0), 86.0), 4.0);
        hueso(l, (x(46.0), 90.0), (x(52.0), 170.0), 6.0);
        hueso(l, (x(50.0), 176.0), (x(48.0), 250.0), 3.5);
        hueso(l, (x(58.0), 176.0), (x(58.0), 250.0), 3.5);
        l.elipse(
            x(54.0) - 9.0,
            256.0,
            18.0,
            26.0,
            Some(TINTA),
            Some(HUESO),
            1.5,
        );
        hueso(l, (x(22.0), 290.0), (x(18.0), 380.0), 7.0);
        l.circulo(x(18.0), 388.0, 6.0, Some(TINTA), Some(HUESO), 1.5);
        hueso(l, (x(16.0), 398.0), (x(16.0), 458.0), 5.0);
        hueso(l, (x(26.0), 400.0), (x(26.0), 456.0), 3.0);
        l.elipse(
            x(20.0) - 13.0 + s * 4.0,
            462.0,
            26.0,
            10.0,
            Some(TINTA),
            Some(HUESO),
            1.5,
        );
    }
    // El craneo, encima.
    l.elipse(cx - 26.0, 0.0, 52.0, 56.0, Some(TINTA), Some(HUESO), G);
    forma(
        l,
        &[
            (cx - 16.0, 44.0),
            (cx + 16.0, 44.0),
            (cx + 12.0, 64.0),
            (cx - 12.0, 64.0),
        ],
        TINTA,
        Some(HUESO),
        1.5,
    );
    l.circulo(cx - 10.0, 26.0, 6.0, None, Some(TINTA), 1.0);
    l.circulo(cx + 10.0, 26.0, 6.0, None, Some(TINTA), 1.0);
    rotulos_derecha(
        l,
        200.0,
        14.0,
        42.0,
        &[
            ((122.0, 18.0), "Cráneo"),
            ((126.0, 84.0), "Clavícula"),
            ((104.0, 120.0), "Esternón"),
            ((138.0, 128.0), "Costillas"),
            ((150.0, 140.0), "Húmero"),
            ((100.0, 220.0), "Columna vertebral"),
            ((156.0, 220.0), "Radio y cúbito"),
            ((132.0, 262.0), "Pelvis"),
            ((120.0, 340.0), "Fémur"),
            ((124.0, 388.0), "Rótula"),
            ((122.0, 430.0), "Tibia y peroné"),
        ],
    );
}

fn vertebra(l: &mut Lapiz) {
    blanda(
        l,
        &[(106.0, 98.0), (40.0, 80.0), (30.0, 98.0), (100.0, 128.0)],
        TINTA,
        Some(HUESO),
        1.8,
    );
    blanda(
        l,
        &[(174.0, 98.0), (240.0, 80.0), (250.0, 98.0), (180.0, 128.0)],
        TINTA,
        Some(HUESO),
        1.8,
    );
    blanda(
        l,
        &[(126.0, 76.0), (140.0, 8.0), (154.0, 76.0)],
        TINTA,
        Some(HUESO),
        1.8,
    );
    blanda(
        l,
        &[
            (94.0, 150.0),
            (100.0, 98.0),
            (120.0, 70.0),
            (140.0, 62.0),
            (160.0, 70.0),
            (180.0, 98.0),
            (186.0, 150.0),
        ],
        TINTA,
        Some(HUESO),
        1.8,
    );
    l.elipse(60.0, 138.0, 160.0, 96.0, Some(TINTA), Some(HUESO), G);
    l.elipse(84.0, 156.0, 112.0, 62.0, Some(hex(0xd8cdb6)), None, 1.0);
    l.v.last_mut().unwrap().estilo = EstiloTrazo::Discontinuo;
    l.elipse(112.0, 88.0, 56.0, 48.0, Some(TINTA), Some(BLANCO), G);
    txt(l, 140.0, 254.0, "ANTERIOR", 9.0, GRIS);
    txt(l, 60.0, 14.0, "POSTERIOR", 9.0, GRIS);
    rotulos_derecha(
        l,
        270.0,
        20.0,
        40.0,
        &[
            ((150.0, 30.0), "Apófisis espinosa"),
            ((166.0, 78.0), "Lámina"),
            ((236.0, 92.0), "Apófisis transversa"),
            ((184.0, 132.0), "Pedículo"),
            ((150.0, 112.0), "Agujero vertebral"),
            ((190.0, 196.0), "Cuerpo vertebral"),
        ],
    );
}

// ------------------------------------------------------------------- ojo

fn ojo(l: &mut Lapiz) {
    let (cx, cy, r) = (170.0, 150.0, 100.0);
    // El nervio optico, por detras.
    l.rect(
        256.0,
        134.0,
        80.0,
        32.0,
        Some(TINTA),
        Some(hex(0xf6dc8c)),
        1.5,
        false,
    );
    let punto = |a: f32, rr: f32, c: (f32, f32)| {
        (
            c.0 + rr * a.to_radians().cos(),
            c.1 + rr * a.to_radians().sin(),
        )
    };
    // La cornea: el arco de delante que se junta con la esclerotica.
    let (cc, rc) = ((104.1, cy), 54.1);
    let esclera: Vec<(f32, f32)> = (0..=40)
        .map(|i| punto(-150.0 + 300.0 * i as f32 / 40.0, r, (cx, cy)))
        .collect();
    let cornea: Vec<(f32, f32)> = (0..=18)
        .map(|i| punto(112.5 + 135.0 * i as f32 / 18.0, rc, cc))
        .collect();
    let mut globo = esclera.clone();
    globo.extend(&cornea);
    forma(l, &globo, hex(0xf4f8fb), Some(hex(0xf4f8fb)), 1.0);
    l.poli(&esclera, hex(0x8a94a6), 5.0, None);
    l.poli(&cornea, AZUL, 2.0, None);
    l.arco(cx, cy, 93.0, -146.0, 146.0, hex(0x9c4f4f), 3.0);
    l.arco(cx, cy, 88.0, -140.0, 140.0, hex(0xd88a3a), 3.0);
    l.elipse(
        87.0,
        118.0,
        28.0,
        64.0,
        Some(AZUL),
        Some(hex(0xcfe6f7)),
        1.5,
    );
    l.linea(86.0, 98.0, 86.0, 134.0, hex(0x6b8e5a), 6.0);
    l.linea(86.0, 166.0, 86.0, 202.0, hex(0x6b8e5a), 6.0);
    let fovea = punto(-8.0, 88.0, (cx, cy));
    l.circulo(fovea.0, fovea.1, 3.5, None, Some(ROJO_OSCURO), 1.0);
    l.flecha(&[(0.0, 150.0), (40.0, 150.0)], AMARILLO, 3.0, false);
    txt(l, 16.0, 136.0, "luz", 11.0, GRIS);
    rotulo(l, (54.0, 128.0), (40.0, 40.0), "Córnea", Al::Centro);
    rotulo(l, (86.0, 110.0), (96.0, 14.0), "Iris", Al::Centro);
    rotulo(l, (104.0, 130.0), (150.0, 30.0), "Cristalino", Al::Centro);
    rotulo(l, (220.0, 63.0), (232.0, 22.0), "Esclerótica", Al::Centro);
    rotulo(l, (250.0, 96.0), (300.0, 40.0), "Coroides", Al::Centro);
    rotulo(l, (254.0, 116.0), (354.0, 96.0), "Retina", Al::Izq);
    rotulo(l, fovea, (354.0, 124.0), "Fóvea", Al::Izq);
    rotulo(l, (330.0, 160.0), (354.0, 190.0), "Nervio óptico", Al::Izq);
    rotulo(l, (86.0, 150.0), (40.0, 270.0), "Pupila", Al::Centro);
    rotulo(l, (70.0, 176.0), (100.0, 290.0), "Humor acuoso", Al::Centro);
    rotulo(
        l,
        (180.0, 190.0),
        (190.0, 280.0),
        "Humor vítreo",
        Al::Centro,
    );
}

// ------------------------------------------------------------------- ecg

fn ecg(l: &mut Lapiz) {
    let (w, h) = (480.0, 170.0);
    let fina = hex(0xf7d5d5);
    let gruesa = hex(0xeaa5a5);
    l.rect(0.0, 0.0, w, h, None, Some(hex(0xfff7f7)), 1.0, false);
    for i in 0..=(w / 8.0) as usize {
        let x = 8.0 * i as f32;
        l.linea(
            x,
            0.0,
            x,
            h,
            if i % 5 == 0 { gruesa } else { fina },
            if i % 5 == 0 { 1.2 } else { 0.8 },
        );
    }
    for j in 0..=(h / 8.0) as usize {
        let y = 8.0 * j as f32;
        l.linea(
            0.0,
            y,
            w,
            y,
            if j % 5 == 0 { gruesa } else { fina },
            if j % 5 == 0 { 1.2 } else { 0.8 },
        );
    }
    // Un latido: P, QRS y T.
    let latido = [
        (0.0, 0.0),
        (20.0, 0.0),
        (28.0, -8.0),
        (36.0, -10.0),
        (44.0, -8.0),
        (52.0, 0.0),
        (62.0, 0.0),
        (66.0, 7.0),
        (72.0, -62.0),
        (78.0, 18.0),
        (84.0, 0.0),
        (100.0, 0.0),
        (112.0, -14.0),
        (124.0, -18.0),
        (136.0, -14.0),
        (146.0, 0.0),
    ];
    let base = 112.0;
    let mut trazo = Vec::new();
    for k in 0..3 {
        trazo.extend(latido.iter().map(|(x, y)| (x + 160.0 * k as f32, base + y)));
    }
    trazo.push((w, base));
    l.poli(&trazo, TINTA, 2.0, None);
    txt_n(l, 36.0, base - 22.0, "P", 12.0, ROJO_OSCURO);
    txt_n(l, 62.0, base + 16.0, "Q", 12.0, ROJO_OSCURO);
    txt_n(l, 72.0, base - 72.0, "R", 12.0, ROJO_OSCURO);
    txt_n(l, 84.0, base + 26.0, "S", 12.0, ROJO_OSCURO);
    txt_n(l, 124.0, base - 30.0, "T", 12.0, ROJO_OSCURO);
    l.flecha(&[(72.0, 30.0), (232.0, 30.0)], AZUL, 1.2, true);
    txt(l, 152.0, 20.0, "intervalo R-R", 11.0, AZUL);
    txt_a(l, w - 8.0, h - 12.0, "25 mm/s · 10 mm/mV", 10.0, Al::Der);
}

// --------------------------------------------------------------- organos

fn rinon(l: &mut Lapiz) {
    l.linea(76.0, 120.0, 0.0, 108.0, ROJO, 7.0);
    l.linea(76.0, 140.0, 0.0, 146.0, AZUL_VENA, 8.0);
    l.poli(
        &[(64.0, 160.0), (52.0, 210.0), (40.0, 270.0)],
        hex(0xd9b44a),
        8.0,
        None,
    );
    let borde = [
        (100.0, 10.0),
        (160.0, 20.0),
        (194.0, 80.0),
        (194.0, 180.0),
        (160.0, 252.0),
        (100.0, 264.0),
        (56.0, 232.0),
        (40.0, 170.0),
        (76.0, 146.0),
        (76.0, 112.0),
        (40.0, 84.0),
        (56.0, 30.0),
    ];
    blanda(l, &borde, ROJO_OSCURO, Some(hex(0xe89a8c)), 2.5);
    let centro = (106.0, 137.0);
    for g in [-75.0f32, -45.0, -15.0, 15.0, 45.0, 75.0] {
        let p = |a: f32, rr: f32| {
            (
                centro.0 + rr * a.to_radians().cos(),
                centro.1 + rr * a.to_radians().sin(),
            )
        };
        forma(
            l,
            &[p(g, 36.0), p(g - 12.0, 74.0), p(g + 12.0, 74.0)],
            ROJO_OSCURO,
            Some(hex(0xc65a52)),
            1.2,
        );
    }
    blanda(
        l,
        &[
            (70.0, 112.0),
            (110.0, 116.0),
            (124.0, 138.0),
            (110.0, 158.0),
            (70.0, 166.0),
            (60.0, 140.0),
        ],
        hex(0xb08a2e),
        Some(hex(0xf6dc8c)),
        1.5,
    );
    rotulos_derecha(
        l,
        230.0,
        30.0,
        50.0,
        &[
            ((150.0, 26.0), "Cápsula"),
            ((178.0, 92.0), "Corteza"),
            ((166.0, 140.0), "Médula (pirámide)"),
            ((112.0, 150.0), "Pelvis renal"),
        ],
    );
    rotulos_izquierda(
        l,
        -14.0,
        70.0,
        80.0,
        &[
            ((10.0, 109.0), "Arteria renal"),
            ((10.0, 146.0), "Vena renal"),
            ((46.0, 244.0), "Uréter"),
        ],
    );
}

fn estomago(l: &mut Lapiz) {
    l.rect(
        64.0,
        0.0,
        24.0,
        50.0,
        Some(ORGANO_BORDE),
        Some(hex(0xf2d0c4)),
        G,
        false,
    );
    let borde = [
        (64.0, 40.0),
        (100.0, 30.0),
        (150.0, 26.0),
        (200.0, 54.0),
        (226.0, 116.0),
        (214.0, 196.0),
        (164.0, 244.0),
        (96.0, 254.0),
        (44.0, 238.0),
        (26.0, 214.0),
        (40.0, 198.0),
        (80.0, 206.0),
        (124.0, 192.0),
        (150.0, 156.0),
        (148.0, 112.0),
        (118.0, 84.0),
        (90.0, 80.0),
        (70.0, 66.0),
    ];
    blanda(l, &borde, ORGANO_BORDE, Some(ORGANO), 2.5);
    for k in 0..4 {
        let y = 120.0 + 26.0 * k as f32;
        let pliegue = curva(
            |x| y + 4.0 * (x / 9.0).sin() + (x - 160.0) * 0.4,
            168.0,
            204.0 - 6.0 * k as f32,
            16,
        );
        l.poli(&pliegue, ORGANO_BORDE, 1.2, None);
    }
    l.poli(
        &[(32.0, 206.0), (10.0, 222.0), (8.0, 262.0)],
        ORGANO_BORDE,
        16.0,
        None,
    );
    l.poli(
        &[(32.0, 206.0), (10.0, 222.0), (8.0, 262.0)],
        hex(0xf2d0c4),
        11.0,
        None,
    );
    rotulos_derecha(
        l,
        250.0,
        10.0,
        38.0,
        &[
            ((88.0, 20.0), "Esófago"),
            ((170.0, 44.0), "Fundus"),
            ((200.0, 130.0), "Cuerpo"),
            ((210.0, 200.0), "Curvatura mayor"),
            ((140.0, 140.0), "Curvatura menor"),
            ((36.0, 208.0), "Píloro"),
            ((10.0, 250.0), "Duodeno"),
        ],
    );
}

fn encefalo(l: &mut Lapiz) {
    forma(
        l,
        &[
            (188.0, 166.0),
            (214.0, 166.0),
            (210.0, 250.0),
            (194.0, 250.0),
        ],
        TINTA,
        Some(hex(0xe8c8b8)),
        1.8,
    );
    blanda(
        l,
        &[
            (206.0, 168.0),
            (260.0, 156.0),
            (296.0, 168.0),
            (290.0, 208.0),
            (244.0, 220.0),
            (206.0, 204.0),
        ],
        TINTA,
        Some(hex(0xd9a78f)),
        1.8,
    );
    for k in 0..4 {
        let y = 176.0 + 9.0 * k as f32;
        l.arco_eliptico(
            250.0,
            y + 30.0,
            36.0 - 3.0 * k as f32,
            26.0,
            205.0,
            335.0,
            hex(0xa86a52),
            1.0,
        );
    }
    let borde = [
        (30.0, 120.0),
        (40.0, 60.0),
        (100.0, 20.0),
        (190.0, 12.0),
        (264.0, 38.0),
        (304.0, 100.0),
        (298.0, 150.0),
        (252.0, 172.0),
        (200.0, 170.0),
        (150.0, 188.0),
        (90.0, 186.0),
        (40.0, 160.0),
    ];
    blanda(l, &borde, TINTA, Some(hex(0xf5d6cf)), 2.5);
    // Las cisuras y algunas circunvoluciones.
    l.poli(
        &bezier((168.0, 14.0), (150.0, 70.0), (142.0, 124.0), 10),
        TINTA,
        2.5,
        None,
    );
    l.poli(
        &bezier((92.0, 136.0), (150.0, 112.0), (214.0, 116.0), 10),
        TINTA,
        2.5,
        None,
    );
    l.poli(
        &bezier((262.0, 40.0), (260.0, 80.0), (282.0, 118.0), 8),
        TINTA,
        1.8,
        None,
    );
    for (a, c, b) in [
        ((60.0, 70.0), (80.0, 90.0), (70.0, 120.0)),
        ((110.0, 40.0), (130.0, 70.0), (110.0, 100.0)),
        ((200.0, 30.0), (190.0, 70.0), (210.0, 100.0)),
        ((230.0, 50.0), (240.0, 80.0), (220.0, 104.0)),
        ((140.0, 150.0), (170.0, 140.0), (200.0, 152.0)),
    ] {
        l.poli(&bezier(a, c, b, 8), hex(0xc08a7a), 1.2, None);
    }
    txt_n(l, 90.0, 80.0, "Frontal", 12.0, hex(0x3f6db3));
    txt_n(l, 218.0, 70.0, "Parietal", 12.0, hex(0x2c8a7f));
    txt_n(l, 160.0, 160.0, "Temporal", 12.0, hex(0xc0692b));
    txt_n(l, 286.0, 122.0, "Occ.", 11.0, MORADO);
    rotulos_derecha(
        l,
        330.0,
        20.0,
        46.0,
        &[
            ((160.0, 50.0), "Cisura de Rolando"),
            ((190.0, 117.0), "Cisura de Silvio"),
            ((298.0, 130.0), "Lóbulo occipital"),
            ((270.0, 196.0), "Cerebelo"),
            ((206.0, 236.0), "Tronco encefálico"),
        ],
    );
}

// ----------------------------------------------------------------- hojas

fn historia_clinica() -> FiguraGuardada {
    let mut p = Pag::nueva(MEDICINA, "Historia clínica", &["Fecha", "N.º historia"]);
    let ancho = W - 2.0 * M;
    let mitad = ancho / 2.0 - 14.0;
    let x2 = M + mitad + 28.0;
    let y = p.seccion(M, Y0, ancho, "Datos del paciente");
    p.datos(
        M,
        y,
        mitad,
        &[
            "Nombre y apellidos:",
            "Fecha de nacimiento:",
            "Sexo:",
            "DNI / documento:",
        ],
        28.0,
    );
    let y = p.datos(
        x2,
        y,
        mitad,
        &[
            "Teléfono:",
            "Dirección:",
            "Seguro médico:",
            "Contacto de emergencia:",
        ],
        28.0,
    );
    let y = p.seccion(M, y + 22.0, ancho, "Motivo de consulta");
    let y = p.renglones(M, y, ancho, 2, 28.0);
    let y = p.seccion(M, y + 22.0, ancho, "Enfermedad actual");
    let y = p.renglones(M, y, ancho, 3, 28.0);
    let y = p.seccion(M, y + 22.0, ancho, "Antecedentes");
    p.casillas(
        M,
        y,
        mitad,
        5,
        28.0,
        &[
            "Alergias:",
            "Hipertensión arterial",
            "Diabetes",
            "Cardiopatía",
            "Cirugías previas:",
        ],
    );
    let y = p.datos(
        x2,
        y,
        mitad,
        &[
            "Medicación habitual:",
            "Antecedentes familiares:",
            "Tabaco / alcohol:",
            "Vacunas:",
            "Otros:",
        ],
        28.0,
    );
    let y = p.seccion(M, y + 22.0, ancho, "Constantes vitales");
    let cols = [
        ("TA (mmHg)", 1.0),
        ("FC (lpm)", 1.0),
        ("FR (rpm)", 1.0),
        ("T.ª (°C)", 1.0),
        ("SpO₂ (%)", 1.0),
        ("Peso (kg)", 1.0),
        ("Talla (cm)", 1.0),
    ];
    let y = p.tabla(M, y + 8.0, ancho, &cols, 1, 34.0);
    let y = p.seccion(M, y + 22.0, ancho, "Exploración física");
    let y = p.renglones(M, y, ancho, 3, 28.0);
    let y = p.seccion(M, y + 22.0, mitad, "Diagnóstico");
    p.renglones(M, y, mitad, 3, 28.0);
    p.seccion(x2, y - 20.0, mitad, "Plan / tratamiento");
    let y = p.renglones(x2, y, mitad, 3, 28.0);
    p.datos(x2, y + 6.0, mitad, &["Médico y firma:"], 30.0);
    p.fin(&format!("{PREFIJO}historia-clinica"), "Historia clínica")
}

fn nota_soap() -> FiguraGuardada {
    let mut p = Pag::nueva(MEDICINA, "Nota SOAP", &["Fecha", "Paciente"]);
    let ancho = W - 2.0 * M;
    let partes = [
        (
            "S",
            "Subjetivo",
            "Lo que cuenta el paciente: síntomas, desde cuándo, cómo, qué lo alivia.",
        ),
        (
            "O",
            "Objetivo",
            "Lo que se observa y se mide: constantes, exploración, pruebas.",
        ),
        (
            "A",
            "Análisis",
            "La valoración: diagnóstico o diagnósticos diferenciales.",
        ),
        (
            "P",
            "Plan",
            "Tratamiento, pruebas pedidas, educación y seguimiento.",
        ),
    ];
    let mut y = Y0;
    for (letra, titulo, ayuda) in partes {
        let (acento, suave) = (p.t.acento, p.t.suave);
        p.l.rect(M, y, ancho, 214.0, None, Some(suave), 1.0, true);
        p.l.circulo(M + 30.0, y + 30.0, 20.0, None, Some(acento), 1.0);
        txt_n(&mut p.l, M + 30.0, y + 31.0, letra, 20.0, BLANCO);
        p.l.texto(
            M + 62.0,
            y + 12.0,
            titulo,
            15.0,
            WORK_SANS,
            acento,
            true,
            Al::Izq,
        );
        p.l.texto(
            M + 62.0,
            y + 34.0,
            ayuda,
            10.5,
            WORK_SANS,
            GRIS,
            false,
            Al::Izq,
        );
        if letra == "O" {
            let cols = [
                ("TA", 1.0),
                ("FC", 1.0),
                ("FR", 1.0),
                ("T.ª", 1.0),
                ("SpO₂", 1.0),
            ];
            let fin = p.tabla(M + 20.0, y + 62.0, ancho - 40.0, &cols, 1, 30.0);
            p.renglones(M + 20.0, fin, ancho - 40.0, 3, 28.0);
        } else {
            p.renglones(M + 20.0, y + 56.0, ancho - 40.0, 5, 28.0);
        }
        y += 236.0;
    }
    p.fin(&format!("{PREFIJO}nota-soap"), "Nota SOAP")
}
