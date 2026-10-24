//! **Arquitectura y escala humana** (24-oct-2026): los simbolos de un plano
//! de estudiante (muros con su poche, puertas con su arco de giro, ventanas,
//! escaleras en planta, norte, escala grafica, cotas, niveles, ejes y el
//! cajetin) y las figuras que dan la escala (personas en alzado y en
//! planta, coches, bicicleta y el circulo de giro de una silla de ruedas).
//!
//! Se dibujan a 1 px = 1 cm: una puerta de 90 cm mide 90 y una persona
//! 170, asi todo sale a la misma escala y se puede estampar junto.

use super::tecnico::{
    BLANCO, CLARO, Dibujo, FINA, GRIS, POCHE, TINTA, forma, poner, punta, rayado,
};
use super::{Al, CAVEAT, Lapiz, WORK_SANS, hex};
use crate::biblioteca::FiguraGuardada;
use crate::elemento::{ColorRgba, EstiloTrazo};

pub(super) const CATEGORIA: &str = "Arquitectura";
pub(super) const ESCALA: &str = "Escala humana";

const ROTULO: ColorRgba = hex(0x5d6b82);
const MURO: ColorRgba = hex(0xd5d9e0);
const VIDRIO: ColorRgba = hex(0xd6e7f3);
const SILUETA: ColorRgba = hex(0xaeb7c4);
const SILUETA_BORDE: ColorRgba = hex(0x6f7a8b);

pub(super) fn todas(v: &mut Vec<FiguraGuardada>) {
    let plano: [Dibujo; 18] = [
        ("arq-muro", "Muro (tramo)", muro),
        ("arq-muro-esquina", "Muro en esquina", muro_esquina),
        ("arq-puerta", "Puerta con arco de giro", puerta),
        ("arq-puerta-doble", "Puerta doble", puerta_doble),
        ("arq-puerta-corredera", "Puerta corredera", puerta_corredera),
        ("arq-ventana", "Ventana en muro", ventana),
        (
            "arq-escalera-recta",
            "Escalera recta (planta)",
            escalera_recta,
        ),
        ("arq-escalera-u", "Escalera en U (planta)", escalera_u),
        (
            "arq-escalera-caracol",
            "Escalera de caracol (planta)",
            escalera_caracol,
        ),
        ("arq-norte", "Norte", norte),
        ("arq-escala-grafica", "Escala gráfica", escala_grafica),
        ("arq-cota", "Cota", cota),
        ("arq-cota-cadena", "Cotas en cadena", cotas_en_cadena),
        ("arq-nivel", "Nivel (N.P.T.)", nivel),
        ("arq-corte", "Línea de corte A-A'", linea_de_corte),
        ("arq-ejes", "Ejes estructurales", ejes),
        ("arq-ambiente", "Rótulo de ambiente", rotulo_de_ambiente),
        ("arq-cajetin", "Cajetín de plano", cajetin),
    ];
    let escala: [Dibujo; 8] = [
        ("arq-persona-frente", "Persona en alzado (frente)", |l| {
            persona_frente(l, 0.0, 0.0, 1.0)
        }),
        (
            "arq-persona-caminando",
            "Persona caminando (alzado)",
            persona_caminando,
        ),
        ("arq-grupo", "Grupo de personas (alzado)", grupo),
        ("arq-persona-planta", "Persona en planta", persona_planta),
        ("arq-coche-planta", "Coche en planta", coche_planta),
        ("arq-coche-alzado", "Coche en alzado", coche_alzado),
        ("arq-bicicleta", "Bicicleta (alzado)", bicicleta),
        (
            "arq-giro-silla",
            "Círculo de giro (silla de ruedas)",
            giro_silla,
        ),
    ];
    poner(v, CATEGORIA, &plano);
    poner(v, ESCALA, &escala);
}

// --------------------------------------------------------------------- muros

/// Un tramo de muro cortado: relleno gris, rayado fino y contorno grueso.
pub(super) fn tramo(l: &mut Lapiz, x: f32, y: f32, w: f32, h: f32) {
    l.rect(x, y, w, h, None, Some(MURO), 1.0, false);
    rayado(l, x, y, w, h, 7.0, POCHE, 0.75);
    l.rect(x, y, w, h, Some(TINTA), None, 2.0, false);
}

fn muro(l: &mut Lapiz) {
    tramo(l, 0.0, 0.0, 300.0, 25.0);
}

fn muro_esquina(l: &mut Lapiz) {
    let e = 25.0;
    let pts = [
        (0.0, 0.0),
        (240.0, 0.0),
        (240.0, e),
        (e, e),
        (e, 240.0),
        (0.0, 240.0),
    ];
    forma(l, &pts, TINTA, Some(MURO), 1.0);
    rayado(l, 0.0, 0.0, 240.0, e, 7.0, POCHE, 0.75);
    rayado(l, 0.0, e, e, 240.0 - e, 7.0, POCHE, 0.75);
    forma(l, &pts, TINTA, None, 2.0);
}

/// El muro con un hueco de `ancho` a partir de `x`.
fn muro_con_hueco(l: &mut Lapiz, x: f32, ancho: f32, total: f32) {
    tramo(l, 0.0, 0.0, x, 20.0);
    tramo(l, x + ancho, 0.0, total - x - ancho, 20.0);
}

/// Una hoja de puerta con su bisagra en `(bx, by)`, abriendo hacia abajo.
fn hoja(l: &mut Lapiz, bx: f32, by: f32, largo: f32, a_la_derecha: bool) {
    let x = if a_la_derecha { bx } else { bx - 4.0 };
    l.rect(x, by, 4.0, largo, Some(TINTA), Some(BLANCO), 1.5, false);
    let (desde, hasta) = if a_la_derecha {
        (0.0, 90.0)
    } else {
        (90.0, 180.0)
    };
    l.arco(bx, by, largo, desde, hasta, FINA, 1.0);
}

fn puerta(l: &mut Lapiz) {
    muro_con_hueco(l, 50.0, 90.0, 190.0);
    hoja(l, 50.0, 20.0, 90.0, true);
}

fn puerta_doble(l: &mut Lapiz) {
    muro_con_hueco(l, 50.0, 160.0, 260.0);
    hoja(l, 50.0, 20.0, 80.0, true);
    hoja(l, 210.0, 20.0, 80.0, false);
}

fn puerta_corredera(l: &mut Lapiz) {
    muro_con_hueco(l, 50.0, 100.0, 200.0);
    l.rect(46.0, 4.0, 58.0, 5.0, Some(TINTA), Some(BLANCO), 1.25, false);
    l.rect(
        96.0,
        11.0,
        58.0,
        5.0,
        Some(TINTA),
        Some(BLANCO),
        1.25,
        false,
    );
    l.flecha(&[(70.0, 40.0), (130.0, 40.0)], FINA, 1.25, true);
}

fn ventana(l: &mut Lapiz) {
    muro_con_hueco(l, 50.0, 120.0, 220.0);
    l.rect(
        50.0,
        0.0,
        120.0,
        20.0,
        Some(TINTA),
        Some(BLANCO),
        1.5,
        false,
    );
    l.rect(50.0, 8.0, 120.0, 4.0, Some(FINA), Some(VIDRIO), 1.0, false);
    // El alfeizar, que sobresale por fuera.
    l.linea(44.0, 24.0, 176.0, 24.0, FINA, 1.0);
    l.texto(
        110.0,
        32.0,
        "V-1",
        11.0,
        WORK_SANS,
        ROTULO,
        true,
        Al::Centro,
    );
}

// ----------------------------------------------------------------- escaleras

fn escalera_recta(l: &mut Lapiz) {
    let (w, paso, n) = (100.0, 25.0, 12);
    l.rect(
        0.0,
        0.0,
        w,
        paso * n as f32,
        Some(TINTA),
        Some(BLANCO),
        2.0,
        false,
    );
    for i in 1..n {
        let y = paso * i as f32;
        l.linea(0.0, y, w, y, FINA, 1.0);
    }
    // La linea de corte, quebrada, a media escalera.
    l.poli(
        &[(0.0, 185.0), (44.0, 162.0), (52.0, 176.0), (100.0, 150.0)],
        TINTA,
        1.5,
        None,
    );
    l.circulo(50.0, 288.0, 4.0, Some(TINTA), Some(TINTA), 1.0);
    l.flecha(&[(50.0, 288.0), (50.0, 18.0)], TINTA, 1.5, false);
    l.texto(
        50.0,
        306.0,
        "SUBE",
        11.0,
        WORK_SANS,
        TINTA,
        true,
        Al::Centro,
    );
}

fn escalera_u(l: &mut Lapiz) {
    let (w, paso, n, meseta) = (100.0, 25.0, 8, 100.0);
    let total = meseta + paso * n as f32;
    l.rect(
        0.0,
        0.0,
        2.0 * w + 20.0,
        total,
        Some(TINTA),
        Some(BLANCO),
        2.0,
        false,
    );
    tramo(l, w, meseta, 20.0, total - meseta);
    for i in 0..n {
        let y = meseta + paso * i as f32;
        l.linea(0.0, y, w, y, FINA, 1.0);
        l.linea(w + 20.0, y, 2.0 * w + 20.0, y, FINA, 1.0);
    }
    l.circulo(50.0, total - 12.0, 4.0, Some(TINTA), Some(TINTA), 1.0);
    l.flecha(
        &[
            (50.0, total - 12.0),
            (50.0, 50.0),
            (170.0, 50.0),
            (170.0, total - 14.0),
        ],
        TINTA,
        1.5,
        false,
    );
    l.texto(
        50.0,
        total + 8.0,
        "SUBE",
        11.0,
        WORK_SANS,
        TINTA,
        true,
        Al::Centro,
    );
}

fn escalera_caracol(l: &mut Lapiz) {
    let (c, r) = (110.0, 110.0);
    l.circulo(c, c, r, Some(TINTA), Some(BLANCO), 2.0);
    for i in 0..14 {
        let a = (i as f32 * 360.0 / 14.0).to_radians();
        l.linea(
            c + 14.0 * a.cos(),
            c + 14.0 * a.sin(),
            c + r * a.cos(),
            c + r * a.sin(),
            FINA,
            1.0,
        );
    }
    l.circulo(c, c, 14.0, Some(TINTA), Some(GRIS), 1.5);
    let (desde, hasta) = (100.0, 390.0);
    l.arco(c, c, 70.0, desde, hasta, TINTA, 1.5);
    let a = hasta.to_radians();
    punta(
        l,
        c + 70.0 * a.cos(),
        c + 70.0 * a.sin(),
        hasta + 90.0,
        12.0,
        TINTA,
    );
    let a0 = desde.to_radians();
    l.circulo(
        c + 70.0 * a0.cos(),
        c + 70.0 * a0.sin(),
        4.0,
        Some(TINTA),
        Some(TINTA),
        1.0,
    );
    l.texto(
        c,
        2.0 * r + 10.0,
        "SUBE",
        11.0,
        WORK_SANS,
        TINTA,
        true,
        Al::Centro,
    );
}

// ------------------------------------------------------------ rotulos y cotas

fn norte(l: &mut Lapiz) {
    let (cx, cy) = (50.0, 80.0);
    l.circulo(cx, cy, 44.0, Some(TINTA), Some(BLANCO), 1.5);
    forma(
        l,
        &[(cx, 26.0), (cx + 20.0, 116.0), (cx, 100.0)],
        TINTA,
        Some(BLANCO),
        1.5,
    );
    forma(
        l,
        &[(cx, 26.0), (cx, 100.0), (cx - 20.0, 116.0)],
        TINTA,
        Some(TINTA),
        1.5,
    );
    l.texto(cx, 0.0, "N", 20.0, WORK_SANS, TINTA, true, Al::Centro);
}

fn escala_grafica(l: &mut Lapiz) {
    let (x0, y0, paso, alto) = (10.0, 28.0, 50.0, 10.0);
    l.texto(
        x0,
        0.0,
        "ESCALA 1:100",
        11.0,
        WORK_SANS,
        TINTA,
        true,
        Al::Izq,
    );
    // El primer metro partido en decimos de 20 cm, a la izquierda del cero.
    for i in 0..5 {
        let x = x0 + 10.0 * i as f32;
        let relleno = if i % 2 == 0 { TINTA } else { BLANCO };
        l.rect(x, y0, 10.0, alto, Some(TINTA), Some(relleno), 1.0, false);
    }
    for i in 0..5 {
        let x = x0 + paso + paso * i as f32;
        let relleno = if i % 2 == 0 { BLANCO } else { TINTA };
        l.rect(x, y0, paso, alto, Some(TINTA), Some(relleno), 1.0, false);
    }
    for (i, s) in ["1", "0", "1", "2", "3", "4", "5 m"].iter().enumerate() {
        let x = x0 + paso * i as f32;
        l.texto(
            x,
            y0 + alto + 6.0,
            s,
            10.0,
            WORK_SANS,
            TINTA,
            false,
            Al::Centro,
        );
    }
}

/// Una cota horizontal de `x0` a `x1` a la altura `y`, con sus lineas de
/// referencia desde `y_ref` y sus marcas a 45°.
fn tramo_de_cota(l: &mut Lapiz, x0: f32, x1: f32, y: f32, y_ref: f32, s: &str) {
    l.linea(x0 - 8.0, y, x1 + 8.0, y, TINTA, 1.0);
    for x in [x0, x1] {
        l.linea(x, y_ref, x, y + 8.0, FINA, 0.75);
        l.linea(x - 5.0, y + 5.0, x + 5.0, y - 5.0, TINTA, 2.0);
    }
    l.texto(
        (x0 + x1) / 2.0,
        y - 18.0,
        s,
        12.0,
        WORK_SANS,
        TINTA,
        false,
        Al::Centro,
    );
}

fn cota(l: &mut Lapiz) {
    tramo_de_cota(l, 10.0, 360.0, 30.0, 70.0, "3.50");
}

fn cotas_en_cadena(l: &mut Lapiz) {
    let xs = [10.0, 130.0, 220.0, 400.0];
    let textos = ["1.20", "0.90", "1.80"];
    for (i, s) in textos.iter().enumerate() {
        tramo_de_cota(l, xs[i], xs[i + 1], 30.0, 74.0, s);
    }
    tramo_de_cota(l, xs[0], xs[3], 0.0 + 80.0 + 30.0, 140.0, "3.90");
}

fn nivel(l: &mut Lapiz) {
    l.linea(0.0, 34.0, 150.0, 34.0, TINTA, 1.25);
    forma(
        l,
        &[(20.0, 18.0), (44.0, 18.0), (32.0, 34.0)],
        TINTA,
        Some(BLANCO),
        1.25,
    );
    forma(
        l,
        &[(32.0, 18.0), (44.0, 18.0), (32.0, 34.0)],
        TINTA,
        Some(TINTA),
        1.0,
    );
    l.texto(
        52.0,
        0.0,
        "N.P.T. +0.00",
        12.0,
        WORK_SANS,
        TINTA,
        true,
        Al::Izq,
    );
}

fn linea_de_corte(l: &mut Lapiz) {
    l.linea_con(38.0, 30.0, 362.0, 30.0, TINTA, 1.5, EstiloTrazo::RayaPunto);
    for (x, s) in [(20.0, "A"), (380.0, "A'")] {
        l.flecha(&[(x, 48.0), (x, 82.0)], TINTA, 2.0, false);
        l.circulo(x, 30.0, 18.0, Some(TINTA), Some(BLANCO), 1.5);
        l.texto(x, 21.0, s, 14.0, WORK_SANS, TINTA, true, Al::Centro);
    }
}

fn ejes(l: &mut Lapiz) {
    let (x0, y0, dx, dy) = (70.0, 70.0, 130.0, 110.0);
    let r = 15.0;
    for (i, s) in ["A", "B", "C"].iter().enumerate() {
        let x = x0 + dx * i as f32;
        l.linea_con(
            x,
            r * 2.0,
            x,
            y0 + dy * 2.0 + 30.0,
            FINA,
            1.0,
            EstiloTrazo::RayaPunto,
        );
        l.circulo(x, r, r, Some(TINTA), Some(BLANCO), 1.5);
        l.texto(x, r - 9.0, s, 13.0, WORK_SANS, TINTA, true, Al::Centro);
    }
    for (i, s) in ["1", "2", "3"].iter().enumerate() {
        let y = y0 + dy * i as f32;
        l.linea_con(
            r * 2.0,
            y,
            x0 + dx * 2.0 + 30.0,
            y,
            FINA,
            1.0,
            EstiloTrazo::RayaPunto,
        );
        l.circulo(r, y, r, Some(TINTA), Some(BLANCO), 1.5);
        l.texto(r, y - 9.0, s, 13.0, WORK_SANS, TINTA, true, Al::Centro);
    }
    // Los pilares en los cruces.
    for i in 0..3 {
        for j in 0..3 {
            let (x, y) = (x0 + dx * i as f32, y0 + dy * j as f32);
            l.rect(
                x - 12.0,
                y - 12.0,
                24.0,
                24.0,
                Some(TINTA),
                Some(POCHE),
                1.5,
                false,
            );
        }
    }
}

fn rotulo_de_ambiente(l: &mut Lapiz) {
    l.texto(
        90.0,
        0.0,
        "DORMITORIO",
        16.0,
        WORK_SANS,
        TINTA,
        true,
        Al::Centro,
    );
    l.linea(10.0, 26.0, 170.0, 26.0, TINTA, 1.0);
    l.texto(
        90.0,
        32.0,
        "A = 12.50 m²",
        12.0,
        WORK_SANS,
        ROTULO,
        false,
        Al::Centro,
    );
    l.texto(
        90.0,
        52.0,
        "N.P.T. +0.15",
        11.0,
        WORK_SANS,
        ROTULO,
        false,
        Al::Centro,
    );
}

fn cajetin(l: &mut Lapiz) {
    let celdas: [(f32, f32, f32, f32, &str, &str); 8] = [
        (0.0, 0.0, 260.0, 50.0, "PROYECTO", ""),
        (260.0, 0.0, 90.0, 50.0, "ESCALA", "1:100"),
        (350.0, 0.0, 90.0, 100.0, "LÁMINA", "A-01"),
        (0.0, 50.0, 260.0, 50.0, "PLANO", ""),
        (260.0, 50.0, 90.0, 50.0, "FECHA", ""),
        (0.0, 100.0, 175.0, 50.0, "ALUMNO/A", ""),
        (175.0, 100.0, 175.0, 50.0, "PROFESOR/A", ""),
        (350.0, 100.0, 90.0, 50.0, "CURSO", ""),
    ];
    l.rect(
        0.0,
        -40.0,
        440.0,
        40.0,
        Some(TINTA),
        Some(hex(0xeef1f6)),
        1.0,
        false,
    );
    l.texto(
        220.0,
        -31.0,
        "FACULTAD DE ARQUITECTURA",
        13.0,
        WORK_SANS,
        TINTA,
        true,
        Al::Centro,
    );
    for (x, y, w, h, rotulo, valor) in celdas {
        l.rect(x, y, w, h, Some(TINTA), Some(BLANCO), 1.0, false);
        l.texto(
            x + 6.0,
            y + 5.0,
            rotulo,
            8.0,
            WORK_SANS,
            ROTULO,
            true,
            Al::Izq,
        );
        if !valor.is_empty() {
            let tam = if h > 60.0 { 22.0 } else { 14.0 };
            l.texto(
                x + w / 2.0,
                y + h / 2.0 - tam * 0.4,
                valor,
                tam,
                WORK_SANS,
                TINTA,
                true,
                Al::Centro,
            );
        }
    }
    l.rect(0.0, -40.0, 440.0, 190.0, Some(TINTA), None, 2.0, false);
}

// ------------------------------------------------------------- escala humana

/// Una persona de frente de 170 de alto (por `k`), desde `(ox, oy)`.
fn persona_frente(l: &mut Lapiz, ox: f32, oy: f32, k: f32) {
    let p = |x: f32, y: f32| (ox + x * k, oy + y * k);
    let cuerpo: Vec<(f32, f32)> = [
        (10.0, 24.0),
        (40.0, 24.0),
        (46.0, 30.0),
        (49.0, 84.0),
        (44.0, 85.0),
        (39.0, 42.0),
        (38.0, 94.0),
        (37.0, 170.0),
        (28.0, 170.0),
        (25.0, 104.0),
        (22.0, 170.0),
        (13.0, 170.0),
        (12.0, 94.0),
        (11.0, 42.0),
        (6.0, 85.0),
        (1.0, 84.0),
        (4.0, 30.0),
    ]
    .iter()
    .map(|(x, y)| p(*x, *y))
    .collect();
    forma(l, &cuerpo, SILUETA_BORDE, Some(SILUETA), 1.0);
    let (cx, cy) = p(25.0, 11.0);
    l.circulo(cx, cy, 10.0 * k, Some(SILUETA_BORDE), Some(SILUETA), 1.0);
}

fn persona_caminando(l: &mut Lapiz) {
    l.poli(
        &[(28.0, 30.0), (18.0, 60.0), (14.0, 84.0)],
        SILUETA,
        7.0,
        None,
    );
    forma(
        l,
        &[
            (24.0, 24.0),
            (36.0, 24.0),
            (40.0, 40.0),
            (39.0, 90.0),
            (48.0, 128.0),
            (56.0, 164.0),
            (60.0, 170.0),
            (46.0, 170.0),
            (42.0, 132.0),
            (30.0, 106.0),
            (22.0, 134.0),
            (12.0, 170.0),
            (2.0, 170.0),
            (4.0, 164.0),
            (16.0, 128.0),
            (22.0, 92.0),
            (21.0, 40.0),
        ],
        SILUETA_BORDE,
        Some(SILUETA),
        1.0,
    );
    l.poli(
        &[(33.0, 30.0), (42.0, 62.0), (50.0, 82.0)],
        SILUETA_BORDE,
        7.0,
        None,
    );
    l.circulo(31.0, 11.0, 10.0, Some(SILUETA_BORDE), Some(SILUETA), 1.0);
}

fn grupo(l: &mut Lapiz) {
    persona_frente(l, 0.0, 0.0, 1.0);
    persona_frente(l, 56.0, 6.0, 0.965);
    persona_frente(l, 112.0, 70.0, 0.6);
    l.linea(-10.0, 170.0, 150.0, 170.0, FINA, 1.0);
}

fn persona_planta(l: &mut Lapiz) {
    l.elipse(
        0.0,
        6.0,
        56.0,
        26.0,
        Some(SILUETA_BORDE),
        Some(SILUETA),
        1.0,
    );
    l.circulo(28.0, 19.0, 11.0, Some(SILUETA_BORDE), Some(BLANCO), 1.25);
    forma(
        l,
        &[(24.0, 30.0), (28.0, 38.0), (32.0, 30.0)],
        SILUETA_BORDE,
        Some(SILUETA_BORDE),
        1.0,
    );
}

fn coche_planta(l: &mut Lapiz) {
    l.rect(300.0, -8.0, 16.0, 10.0, Some(TINTA), Some(GRIS), 1.0, true);
    l.rect(300.0, 178.0, 16.0, 10.0, Some(TINTA), Some(GRIS), 1.0, true);
    l.rect(0.0, 0.0, 450.0, 180.0, Some(TINTA), Some(CLARO), 2.0, true);
    forma(
        l,
        &[(298.0, 24.0), (346.0, 38.0), (346.0, 142.0), (298.0, 156.0)],
        TINTA,
        Some(VIDRIO),
        1.25,
    );
    forma(
        l,
        &[(160.0, 30.0), (130.0, 42.0), (130.0, 138.0), (160.0, 150.0)],
        TINTA,
        Some(VIDRIO),
        1.25,
    );
    l.rect(
        160.0,
        30.0,
        138.0,
        120.0,
        Some(TINTA),
        Some(BLANCO),
        1.25,
        true,
    );
    // El capo con sus dos nervios y los faros.
    l.linea(360.0, 40.0, 440.0, 46.0, FINA, 0.75);
    l.linea(360.0, 140.0, 440.0, 134.0, FINA, 0.75);
    for y in [18.0, 132.0] {
        l.rect(
            440.0,
            y,
            7.0,
            30.0,
            Some(TINTA),
            Some(hex(0xfff3c4)),
            1.0,
            true,
        );
        l.rect(
            3.0,
            y,
            6.0,
            30.0,
            Some(TINTA),
            Some(hex(0xf2b3ad)),
            1.0,
            true,
        );
    }
}

fn coche_alzado(l: &mut Lapiz) {
    forma(
        l,
        &[
            (0.0, 112.0),
            (8.0, 82.0),
            (80.0, 72.0),
            (140.0, 32.0),
            (290.0, 28.0),
            (360.0, 70.0),
            (436.0, 82.0),
            (450.0, 108.0),
            (446.0, 126.0),
            (2.0, 126.0),
        ],
        TINTA,
        Some(CLARO),
        2.0,
    );
    forma(
        l,
        &[(150.0, 40.0), (222.0, 38.0), (222.0, 72.0), (110.0, 74.0)],
        TINTA,
        Some(VIDRIO),
        1.25,
    );
    forma(
        l,
        &[(234.0, 38.0), (288.0, 38.0), (342.0, 72.0), (234.0, 72.0)],
        TINTA,
        Some(VIDRIO),
        1.25,
    );
    l.linea(228.0, 36.0, 228.0, 118.0, FINA, 1.0);
    l.linea(110.0, 96.0, 400.0, 96.0, FINA, 0.75);
    for x in [95.0, 358.0] {
        l.circulo(x, 124.0, 32.0, Some(TINTA), Some(hex(0x3a3f4b)), 1.5);
        l.circulo(x, 124.0, 14.0, Some(TINTA), Some(GRIS), 1.0);
    }
    l.linea(-10.0, 156.0, 460.0, 156.0, FINA, 1.0);
}

fn bicicleta(l: &mut Lapiz) {
    for x in [40.0, 166.0] {
        l.circulo(x, 80.0, 36.0, Some(TINTA), None, 2.5);
        l.circulo(x, 80.0, 3.0, Some(TINTA), Some(TINTA), 1.0);
    }
    let c = TINTA;
    l.poli(
        &[(40.0, 80.0), (88.0, 80.0), (76.0, 30.0), (40.0, 80.0)],
        c,
        2.0,
        None,
    );
    l.poli(&[(88.0, 80.0), (142.0, 34.0), (76.0, 34.0)], c, 2.0, None);
    l.poli(&[(142.0, 34.0), (166.0, 80.0)], c, 2.0, None);
    l.poli(&[(142.0, 34.0), (140.0, 20.0), (154.0, 18.0)], c, 2.0, None);
    l.poli(&[(64.0, 22.0), (84.0, 22.0)], c, 4.0, None);
    l.linea(74.0, 22.0, 76.0, 30.0, c, 2.0);
    l.circulo(88.0, 80.0, 8.0, Some(c), Some(BLANCO), 1.5);
}

fn giro_silla(l: &mut Lapiz) {
    let c = 75.0;
    l.circulo(c, c, 75.0, Some(hex(0x3f6db3)), Some(hex(0xeef3fb)), 1.5);
    if let Some(e) = l.v.last_mut() {
        e.estilo = EstiloTrazo::Discontinuo;
    }
    // La silla vista desde arriba: asiento, ruedas grandes y reposapies.
    l.rect(
        c - 25.0,
        c - 22.0,
        50.0,
        44.0,
        Some(TINTA),
        Some(BLANCO),
        1.5,
        true,
    );
    l.rect(
        c - 33.0,
        c - 30.0,
        7.0,
        56.0,
        Some(TINTA),
        Some(GRIS),
        1.25,
        true,
    );
    l.rect(
        c + 26.0,
        c - 30.0,
        7.0,
        56.0,
        Some(TINTA),
        Some(GRIS),
        1.25,
        true,
    );
    l.linea(c - 25.0, c - 26.0, c + 25.0, c - 26.0, TINTA, 2.0);
    l.rect(
        c - 16.0,
        c + 26.0,
        32.0,
        10.0,
        Some(TINTA),
        Some(GRIS),
        1.0,
        true,
    );
    l.texto(
        c,
        2.0 * c + 8.0,
        "Ø 1.50 m",
        16.0,
        CAVEAT,
        hex(0x3f6db3),
        true,
        Al::Centro,
    );
}
