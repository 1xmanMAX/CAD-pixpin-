//! **Ingenieria civil**: secciones (viga y columna armadas, perfil en I),
//! cimentacion y contencion (zapata, muro), terreno (talud, perfil
//! longitudinal, curvas de nivel, simbolos topograficos), obras (puentes,
//! carretera en seccion, tuberias) y estructuras (celosia, apoyos, vigas
//! cargadas y los diagramas de cortantes y momentos).
//!
//! Hormigon en gris claro, acero en rojo, tierra en ocre y agua en azul,
//! como en los apuntes de clase.

use super::tecnico::{
    BLANCO, Dibujo, FINA, GRIS, TINTA, circulo_de_puntos, forma, poner, punta, rayado,
};
use super::{Al, CAVEAT, Lapiz, WORK_SANS, hex};
use crate::biblioteca::FiguraGuardada;
use crate::elemento::{ColorRgba, EstiloTrazo};

pub(super) const CATEGORIA: &str = "Ingeniería civil";

const HORMIGON: ColorRgba = hex(0xe4e6ea);
const ACERO: ColorRgba = hex(0xc0392b);
const PERFIL: ColorRgba = hex(0xb7bfca);
const TIERRA: ColorRgba = hex(0xeadcc4);
const TIERRA_RAYA: ColorRgba = hex(0xb79c78);
const AGUA: ColorRgba = hex(0xcfe3f5);
const AGUA_RAYA: ColorRgba = hex(0x4f8fc9);
const CARGA: ColorRgba = hex(0x2f6fb0);
const ROTULO: ColorRgba = hex(0x5d6b82);
const POSITIVO: ColorRgba = hex(0xd6e4f5);
const NEGATIVO: ColorRgba = hex(0xf6d5d1);

pub(super) fn todas(v: &mut Vec<FiguraGuardada>) {
    let lista: [Dibujo; 21] = [
        ("civil-viga-seccion", "Sección de viga armada", viga_seccion),
        ("civil-perfil-i", "Perfil de acero en I", perfil_i),
        ("civil-columna", "Sección de columna", columna),
        ("civil-zapata", "Zapata aislada (corte)", zapata),
        (
            "civil-muro-contencion",
            "Muro de contención (corte)",
            muro_contencion,
        ),
        ("civil-talud", "Talud (perfil de terreno)", talud),
        (
            "civil-perfil-longitudinal",
            "Perfil longitudinal",
            perfil_longitudinal,
        ),
        ("civil-puente-vigas", "Puente de vigas", puente_vigas),
        ("civil-puente-arco", "Puente en arco", puente_arco),
        ("civil-carretera", "Carretera en sección", carretera),
        (
            "civil-tuberia-seccion",
            "Tubería (sección)",
            tuberia_seccion,
        ),
        (
            "civil-tuberia-perfil",
            "Tubería con pendiente",
            tuberia_perfil,
        ),
        ("civil-curvas-nivel", "Curvas de nivel", curvas_de_nivel),
        (
            "civil-simbolos-topo",
            "Símbolos topográficos",
            simbolos_topograficos,
        ),
        ("civil-celosia", "Armadura (celosía Pratt)", celosia),
        (
            "civil-viga-carga",
            "Viga con carga repartida",
            viga_con_carga,
        ),
        (
            "civil-diagramas",
            "Diagramas de cortantes y momentos",
            diagramas,
        ),
        ("civil-voladizo", "Voladizo con carga puntual", voladizo),
        ("civil-apoyo-fijo", "Apoyo fijo (articulado)", |l| {
            apoyo_fijo(l, 40.0, 10.0);
            rotulo(l, 40.0, 70.0, "Apoyo fijo");
        }),
        ("civil-apoyo-movil", "Apoyo móvil (rodillo)", |l| {
            apoyo_movil(l, 40.0, 10.0);
            rotulo(l, 40.0, 76.0, "Apoyo móvil");
        }),
        ("civil-empotramiento", "Empotramiento", |l| {
            empotramiento(l, 30.0, 0.0, 90.0);
            l.rect(
                30.0,
                39.0,
                80.0,
                12.0,
                Some(TINTA),
                Some(HORMIGON),
                1.5,
                false,
            );
            rotulo(l, 56.0, 100.0, "Empotramiento");
        }),
    ];
    poner(v, CATEGORIA, &lista);
}

fn rotulo(l: &mut Lapiz, x: f32, y: f32, s: &str) {
    l.texto(x, y, s, 12.0, WORK_SANS, ROTULO, true, Al::Centro);
}

/// Una cota horizontal con sus marcas a 45° y el texto encima.
fn cota_h(l: &mut Lapiz, x0: f32, x1: f32, y: f32, s: &str) {
    l.linea(x0 - 6.0, y, x1 + 6.0, y, FINA, 1.0);
    for x in [x0, x1] {
        l.linea(x, y - 8.0, x, y + 8.0, FINA, 0.75);
        l.linea(x - 4.0, y + 4.0, x + 4.0, y - 4.0, TINTA, 1.75);
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

/// Una cota vertical con el texto a su derecha.
fn cota_v(l: &mut Lapiz, x: f32, y0: f32, y1: f32, s: &str) {
    l.linea(x, y0 - 6.0, x, y1 + 6.0, FINA, 1.0);
    for y in [y0, y1] {
        l.linea(x - 8.0, y, x + 8.0, y, FINA, 0.75);
        l.linea(x - 4.0, y + 4.0, x + 4.0, y - 4.0, TINTA, 1.75);
    }
    l.texto(
        x + 8.0,
        (y0 + y1) / 2.0 - 8.0,
        s,
        12.0,
        WORK_SANS,
        TINTA,
        false,
        Al::Izq,
    );
}

/// La superficie del terreno: la linea y sus rayitas por debajo.
fn terreno(l: &mut Lapiz, pts: &[(f32, f32)]) {
    l.poli(pts, TINTA, 2.0, None);
    for w in pts.windows(2) {
        let ((x0, y0), (x1, y1)) = (w[0], w[1]);
        let largo = ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt();
        let n = (largo / 14.0) as usize;
        for i in 0..n {
            let t = (i as f32 + 0.5) / n as f32;
            let (x, y) = (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t);
            l.linea(x, y + 1.0, x - 8.0, y + 10.0, TIERRA_RAYA, 1.0);
        }
    }
}

/// Las olas del agua: unas rayitas horizontales.
fn olas(l: &mut Lapiz, x0: f32, x1: f32, y: f32) {
    let mut x = x0;
    let mut fila = 0;
    while x + 24.0 < x1 {
        let dy = if fila % 2 == 0 { 0.0 } else { 8.0 };
        l.linea(x, y + dy, x + 22.0, y + dy, AGUA_RAYA, 1.25);
        x += 34.0;
        fila += 1;
    }
}

/// Una barra de acero vista en seccion.
fn barra(l: &mut Lapiz, x: f32, y: f32, r: f32) {
    l.circulo(x, y, r, Some(hex(0x7b1f17)), Some(ACERO), 1.0);
}

// ----------------------------------------------------------------- secciones

fn viga_seccion(l: &mut Lapiz) {
    let (b, h) = (150.0, 260.0);
    l.rect(0.0, 0.0, b, h, Some(TINTA), Some(HORMIGON), 2.0, false);
    l.rect(14.0, 14.0, b - 28.0, h - 28.0, Some(ACERO), None, 2.0, true);
    for x in [32.0, b - 32.0] {
        barra(l, x, 30.0, 6.0);
    }
    for x in [32.0, b / 2.0, b - 32.0] {
        barra(l, x, h - 32.0, 9.0);
    }
    l.linea(b - 26.0, 30.0, b + 30.0, 30.0, FINA, 0.75);
    l.texto(
        b + 34.0,
        22.0,
        "2 Ø 12",
        12.0,
        WORK_SANS,
        TINTA,
        false,
        Al::Izq,
    );
    l.linea(b - 22.0, h - 32.0, b + 30.0, h - 32.0, FINA, 0.75);
    l.texto(
        b + 34.0,
        h - 40.0,
        "3 Ø 20",
        12.0,
        WORK_SANS,
        TINTA,
        false,
        Al::Izq,
    );
    l.linea(b - 14.0, h / 2.0, b + 30.0, h / 2.0, FINA, 0.75);
    l.texto(
        b + 34.0,
        h / 2.0 - 8.0,
        "Estribo Ø 8 @ 15",
        12.0,
        WORK_SANS,
        TINTA,
        false,
        Al::Izq,
    );
    cota_h(l, 0.0, b, h + 30.0, "b = 30 cm");
    cota_v(l, -30.0, 0.0, h, "");
    l.texto(
        -36.0,
        h / 2.0 - 8.0,
        "h = 60",
        12.0,
        WORK_SANS,
        TINTA,
        false,
        Al::Der,
    );
}

fn perfil_i(l: &mut Lapiz) {
    let (b, h, ala, alma) = (120.0, 200.0, 14.0, 10.0);
    let (m0, m1) = (b / 2.0 - alma / 2.0, b / 2.0 + alma / 2.0);
    forma(
        l,
        &[
            (0.0, 0.0),
            (b, 0.0),
            (b, ala),
            (m1, ala),
            (m1, h - ala),
            (b, h - ala),
            (b, h),
            (0.0, h),
            (0.0, h - ala),
            (m0, h - ala),
            (m0, ala),
            (0.0, ala),
        ],
        TINTA,
        Some(PERFIL),
        2.0,
    );
    cota_h(l, 0.0, b, h + 30.0, "b");
    cota_v(l, b + 30.0, 0.0, h, "h");
    l.texto(
        b / 2.0,
        -30.0,
        "IPE 200",
        14.0,
        WORK_SANS,
        TINTA,
        true,
        Al::Centro,
    );
    l.linea(m1, h / 2.0, b + 4.0, h / 2.0 - 30.0, FINA, 0.75);
    l.texto(
        b + 8.0,
        h / 2.0 - 46.0,
        "alma",
        12.0,
        WORK_SANS,
        ROTULO,
        false,
        Al::Izq,
    );
    l.linea(b * 0.8, ala / 2.0, b + 4.0, 26.0, FINA, 0.75);
    l.texto(
        b + 8.0,
        22.0,
        "ala",
        12.0,
        WORK_SANS,
        ROTULO,
        false,
        Al::Izq,
    );
}

fn columna(l: &mut Lapiz) {
    let s = 170.0;
    l.rect(0.0, 0.0, s, s, Some(TINTA), Some(HORMIGON), 2.0, false);
    l.rect(14.0, 14.0, s - 28.0, s - 28.0, Some(ACERO), None, 2.0, true);
    for (x, y) in [
        (28.0, 28.0),
        (s / 2.0, 28.0),
        (s - 28.0, 28.0),
        (28.0, s / 2.0),
        (s - 28.0, s / 2.0),
        (28.0, s - 28.0),
        (s / 2.0, s - 28.0),
        (s - 28.0, s - 28.0),
    ] {
        barra(l, x, y, 8.0);
    }
    l.poli(&[(s / 2.0, 28.0), (s / 2.0, s - 28.0)], ACERO, 1.25, None);
    if let Some(e) = l.v.last_mut() {
        e.estilo = EstiloTrazo::Discontinuo;
    }
    cota_h(l, 0.0, s, s + 30.0, "40 cm");
    cota_v(l, s + 30.0, 0.0, s, "40 cm");
    l.texto(
        s / 2.0,
        -30.0,
        "COLUMNA · 8 Ø 16",
        13.0,
        WORK_SANS,
        TINTA,
        true,
        Al::Centro,
    );
}

// ------------------------------------------------------ cimentacion y muros

fn zapata(l: &mut Lapiz) {
    let suelo = 60.0;
    l.rect(-20.0, suelo, 360.0, 260.0, None, Some(TIERRA), 1.0, false);
    rayado(l, -20.0, suelo, 360.0, 260.0, 16.0, TIERRA_RAYA, 0.6);
    // El solado, la zapata y el pedestal de la columna.
    l.rect(
        -6.0,
        280.0,
        332.0,
        12.0,
        Some(TINTA),
        Some(GRIS),
        1.25,
        false,
    );
    l.rect(
        0.0,
        210.0,
        320.0,
        70.0,
        Some(TINTA),
        Some(HORMIGON),
        2.0,
        false,
    );
    l.rect(
        130.0,
        0.0,
        60.0,
        210.0,
        Some(TINTA),
        Some(HORMIGON),
        2.0,
        false,
    );
    // El acero: la parrilla abajo y las barras de la columna con su patilla.
    l.poli(
        &[(14.0, 250.0), (14.0, 268.0), (306.0, 268.0), (306.0, 250.0)],
        ACERO,
        2.5,
        None,
    );
    for x in [36.0, 76.0, 116.0, 160.0, 204.0, 244.0, 284.0] {
        barra(l, x, 262.0, 4.0);
    }
    l.poli(
        &[(142.0, 0.0), (142.0, 264.0), (104.0, 264.0)],
        ACERO,
        2.5,
        None,
    );
    l.poli(
        &[(178.0, 0.0), (178.0, 264.0), (216.0, 264.0)],
        ACERO,
        2.5,
        None,
    );
    terreno(l, &[(-20.0, suelo), (130.0, suelo)]);
    terreno(l, &[(190.0, suelo), (340.0, suelo)]);
    cota_h(l, 0.0, 320.0, 330.0, "B = 2.00 m");
    cota_v(l, 360.0, suelo, 280.0, "Df = 1.40 m");
    l.texto(
        -26.0,
        280.0,
        "Solado",
        12.0,
        WORK_SANS,
        ROTULO,
        false,
        Al::Der,
    );
}

fn muro_contencion(l: &mut Lapiz) {
    // El relleno de detras.
    l.rect(130.0, 30.0, 190.0, 270.0, None, Some(TIERRA), 1.0, false);
    rayado(l, 130.0, 30.0, 190.0, 270.0, 16.0, TIERRA_RAYA, 0.6);
    l.rect(0.0, 220.0, 100.0, 80.0, None, Some(TIERRA), 1.0, false);
    rayado(l, 0.0, 220.0, 100.0, 80.0, 16.0, TIERRA_RAYA, 0.6);
    // El dren de grava junto al trasdos.
    l.rect(
        130.0,
        40.0,
        22.0,
        220.0,
        Some(FINA),
        Some(hex(0xdcdcd4)),
        1.0,
        false,
    );
    for i in 0..18 {
        l.circulo(
            141.0 + if i % 2 == 0 { -4.0 } else { 4.0 },
            50.0 + 12.0 * i as f32,
            3.0,
            Some(FINA),
            None,
            0.75,
        );
    }
    // El muro: pantalla y zapata.
    forma(
        l,
        &[
            (100.0, 0.0),
            (130.0, 0.0),
            (130.0, 260.0),
            (240.0, 260.0),
            (240.0, 300.0),
            (40.0, 300.0),
            (40.0, 260.0),
            (100.0, 260.0),
        ],
        TINTA,
        Some(HORMIGON),
        2.0,
    );
    // El barbacana que saca el agua.
    l.linea(130.0, 236.0, 96.0, 242.0, AGUA_RAYA, 3.0);
    punta(l, 86.0, 244.0, 170.0, 10.0, AGUA_RAYA);
    terreno(l, &[(130.0, 30.0), (320.0, 30.0)]);
    terreno(l, &[(0.0, 220.0), (100.0, 220.0)]);
    // El empuje de la tierra, en triangulo.
    forma(
        l,
        &[(160.0, 30.0), (160.0, 260.0), (240.0, 260.0)],
        ACERO,
        None,
        1.25,
    );
    for i in 1..6 {
        let y = 30.0 + 230.0 * i as f32 / 6.0;
        let x = 160.0 + 80.0 * i as f32 / 6.0;
        l.flecha(&[(x, y), (164.0, y)], ACERO, 1.25, false);
    }
    l.texto(250.0, 150.0, "Empuje", 13.0, CAVEAT, ACERO, true, Al::Izq);
    l.texto(
        280.0,
        5.0,
        "Relleno",
        12.0,
        WORK_SANS,
        ROTULO,
        false,
        Al::Centro,
    );
}

// ------------------------------------------------------------------- terreno

fn talud(l: &mut Lapiz) {
    let sup = [(0.0, 60.0), (90.0, 60.0), (300.0, 200.0), (440.0, 200.0)];
    let mut cuerpo = sup.to_vec();
    cuerpo.extend([(440.0, 290.0), (0.0, 290.0)]);
    forma(l, &cuerpo, TIERRA, Some(TIERRA), 1.0);
    l.linea_con(
        0.0,
        250.0,
        440.0,
        250.0,
        AGUA_RAYA,
        1.5,
        EstiloTrazo::Discontinuo,
    );
    forma(
        l,
        &[(390.0, 236.0), (406.0, 236.0), (398.0, 248.0)],
        AGUA_RAYA,
        Some(BLANCO),
        1.25,
    );
    l.texto(
        412.0,
        230.0,
        "N.F.",
        11.0,
        WORK_SANS,
        AGUA_RAYA,
        true,
        Al::Izq,
    );
    terreno(l, &sup);
    // El triangulo de la pendiente, 1.5 : 1.
    l.poli(
        &[(150.0, 100.0), (195.0, 100.0), (195.0, 130.0)],
        ACERO,
        1.5,
        None,
    );
    l.texto(172.0, 82.0, "1.5", 12.0, WORK_SANS, ACERO, true, Al::Centro);
    l.texto(201.0, 106.0, "1", 12.0, WORK_SANS, ACERO, true, Al::Izq);
    l.texto(
        45.0,
        34.0,
        "Corona",
        12.0,
        WORK_SANS,
        ROTULO,
        false,
        Al::Centro,
    );
    l.texto(
        330.0,
        176.0,
        "Pie del talud",
        12.0,
        WORK_SANS,
        ROTULO,
        false,
        Al::Izq,
    );
}

fn perfil_longitudinal(l: &mut Lapiz) {
    let (x0, x1, y0, y1) = (60.0, 620.0, 20.0, 260.0);
    l.rect(
        x0,
        y0,
        x1 - x0,
        y1 - y0,
        Some(FINA),
        Some(BLANCO),
        1.0,
        false,
    );
    for i in 0..=6 {
        let y = y1 - (y1 - y0) * i as f32 / 6.0;
        l.linea_con(x0, y, x1, y, GRIS, 0.75, EstiloTrazo::Discontinuo);
        l.texto(
            x0 - 8.0,
            y - 7.0,
            &format!("{}", 100 + 5 * i),
            10.0,
            WORK_SANS,
            ROTULO,
            false,
            Al::Der,
        );
    }
    let terreno_pts: Vec<(f32, f32)> = (0..=28)
        .map(|i| {
            let t = i as f32 / 28.0;
            let y = 160.0 - 60.0 * (t * 3.6).sin() + 22.0 * (t * 11.0).sin() - 30.0 * t;
            (x0 + (x1 - x0) * t, y)
        })
        .collect();
    let mut cuerpo = terreno_pts.clone();
    cuerpo.extend([(x1, y1), (x0, y1)]);
    forma(l, &cuerpo, TIERRA, Some(TIERRA), 1.0);
    l.poli(&terreno_pts, TINTA, 2.0, None);
    l.linea_con(x0, 150.0, x1, 120.0, ACERO, 2.0, EstiloTrazo::Discontinuo);
    l.texto(
        x1 - 6.0,
        92.0,
        "Rasante",
        12.0,
        WORK_SANS,
        ACERO,
        true,
        Al::Der,
    );
    l.texto(
        x0 + 10.0,
        y1 - 26.0,
        "Terreno natural",
        12.0,
        WORK_SANS,
        ROTULO,
        false,
        Al::Izq,
    );
    // La banda de abajo con las progresivas.
    l.rect(
        x0,
        y1,
        x1 - x0,
        30.0,
        Some(FINA),
        Some(hex(0xf3f5f8)),
        1.0,
        false,
    );
    l.texto(
        x0 - 8.0,
        y1 + 8.0,
        "Prog.",
        10.0,
        WORK_SANS,
        ROTULO,
        true,
        Al::Der,
    );
    for i in 0..=7 {
        let x = x0 + (x1 - x0) * i as f32 / 7.0;
        l.linea(x, y1, x, y1 + 30.0, FINA, 0.75);
        let al = if i == 7 { Al::Der } else { Al::Izq };
        l.texto(
            x + if i == 7 { -4.0 } else { 4.0 },
            y1 + 9.0,
            &format!("0+{:03}", 20 * i),
            10.0,
            WORK_SANS,
            TINTA,
            false,
            al,
        );
    }
}

// --------------------------------------------------------------------- obras

fn puente_vigas(l: &mut Lapiz) {
    let suelo = [
        (-20.0, 100.0),
        (60.0, 100.0),
        (180.0, 240.0),
        (420.0, 240.0),
        (540.0, 100.0),
        (620.0, 100.0),
    ];
    let mut cuerpo = suelo.to_vec();
    cuerpo.extend([(620.0, 270.0), (-20.0, 270.0)]);
    forma(l, &cuerpo, TIERRA, Some(TIERRA), 1.0);
    forma(
        l,
        &[
            (154.3, 210.0),
            (445.7, 210.0),
            (420.0, 240.0),
            (180.0, 240.0),
        ],
        AGUA_RAYA,
        Some(AGUA),
        1.0,
    );
    olas(l, 190.0, 410.0, 220.0);
    terreno(l, &suelo);
    // Estribos, pilas y tablero.
    forma(
        l,
        &[(0.0, 78.0), (54.0, 78.0), (74.0, 112.0), (0.0, 112.0)],
        TINTA,
        Some(HORMIGON),
        1.5,
    );
    forma(
        l,
        &[(546.0, 78.0), (600.0, 78.0), (600.0, 112.0), (526.0, 112.0)],
        TINTA,
        Some(HORMIGON),
        1.5,
    );
    for x in [200.0, 400.0] {
        forma(
            l,
            &[
                (x - 10.0, 78.0),
                (x + 10.0, 78.0),
                (x + 14.0, 240.0),
                (x - 14.0, 240.0),
            ],
            TINTA,
            Some(HORMIGON),
            1.5,
        );
        l.rect(
            x - 24.0,
            240.0,
            48.0,
            14.0,
            Some(TINTA),
            Some(HORMIGON),
            1.25,
            false,
        );
    }
    l.rect(
        0.0,
        60.0,
        600.0,
        18.0,
        Some(TINTA),
        Some(HORMIGON),
        2.0,
        false,
    );
    // La baranda.
    l.linea(0.0, 40.0, 600.0, 40.0, TINTA, 1.5);
    for i in 0..=20 {
        let x = 30.0 * i as f32;
        l.linea(x, 40.0, x, 60.0, TINTA, 1.0);
    }
}

fn puente_arco(l: &mut Lapiz) {
    let (x0, x1, base, flecha) = (60.0, 540.0, 230.0, 160.0);
    let arco = |r: f32| -> Vec<(f32, f32)> {
        (0..=32)
            .map(|i| {
                let t = i as f32 / 32.0;
                (x0 + (x1 - x0) * t, base - flecha * 4.0 * t * (1.0 - t) - r)
            })
            .collect()
    };
    let suelo = [
        (-20.0, 56.0),
        (40.0, 56.0),
        (70.0, 240.0),
        (530.0, 240.0),
        (560.0, 56.0),
        (620.0, 56.0),
    ];
    let mut cuerpo = suelo.to_vec();
    cuerpo.extend([(620.0, 270.0), (-20.0, 270.0)]);
    forma(l, &cuerpo, TIERRA, Some(TIERRA), 1.0);
    forma(
        l,
        &[(64.0, 214.0), (536.0, 214.0), (530.0, 240.0), (70.0, 240.0)],
        AGUA_RAYA,
        Some(AGUA),
        1.0,
    );
    olas(l, 90.0, 510.0, 222.0);
    terreno(l, &suelo);
    // Los montantes, del arco al tablero.
    for i in 1..12 {
        let t = i as f32 / 12.0;
        let x = x0 + (x1 - x0) * t;
        let y = base - flecha * 4.0 * t * (1.0 - t) - 16.0;
        l.rect(
            x - 4.0,
            56.0,
            8.0,
            (y - 56.0).max(2.0),
            Some(TINTA),
            Some(HORMIGON),
            1.0,
            false,
        );
    }
    let mut anillo = arco(0.0);
    anillo.extend(arco(16.0).into_iter().rev());
    forma(l, &anillo, TINTA, Some(HORMIGON), 1.75);
    l.rect(
        0.0,
        40.0,
        600.0,
        16.0,
        Some(TINTA),
        Some(HORMIGON),
        2.0,
        false,
    );
}

fn carretera(l: &mut Lapiz) {
    let eje = 280.0;
    // La calzada con bombeo del 2 %: sube 7 hacia el eje.
    let capa = |dy: f32| -> Vec<(f32, f32)> {
        vec![
            (60.0, 110.0 + dy),
            (100.0, 107.0 + dy),
            (eje, 100.0 + dy),
            (460.0, 107.0 + dy),
            (500.0, 110.0 + dy),
        ]
    };
    let entre = |a: f32, b: f32| -> Vec<(f32, f32)> {
        let mut v = capa(a);
        v.extend(capa(b).into_iter().rev());
        v
    };
    forma(l, &entre(34.0, 60.0), FINA, Some(hex(0xefe5d2)), 1.0);
    forma(l, &entre(14.0, 34.0), FINA, Some(hex(0xdccaa5)), 1.0);
    forma(l, &entre(0.0, 14.0), TINTA, Some(hex(0x3a3f4b)), 1.25);
    // Las cunetas a los lados y el terreno.
    terreno(
        l,
        &[(0.0, 96.0), (20.0, 110.0), (40.0, 134.0), (60.0, 110.0)],
    );
    terreno(
        l,
        &[
            (500.0, 110.0),
            (520.0, 134.0),
            (540.0, 110.0),
            (560.0, 96.0),
        ],
    );
    l.linea(60.0, 170.0, 500.0, 170.0, TIERRA_RAYA, 1.0);
    l.linea_con(eje, 50.0, eje, 190.0, FINA, 1.0, EstiloTrazo::RayaPunto);
    l.texto(eje, 30.0, "Eje", 12.0, WORK_SANS, ROTULO, true, Al::Centro);
    for (a, b, x) in [(eje - 30.0, 120.0, 180.0), (eje + 30.0, 440.0, 380.0)] {
        l.flecha(&[(a, 88.0), (b, 92.0)], ACERO, 1.25, false);
        l.texto(x, 66.0, "2 %", 12.0, WORK_SANS, ACERO, true, Al::Centro);
    }
    for (y, s) in [
        (107.0, "Carpeta asfáltica"),
        (128.0, "Base"),
        (155.0, "Subbase"),
        (176.0, "Subrasante"),
    ] {
        l.linea(470.0, y, 580.0, y + 30.0, FINA, 0.75);
        l.texto(586.0, y + 22.0, s, 12.0, WORK_SANS, TINTA, false, Al::Izq);
    }
    l.texto(
        80.0,
        186.0,
        "Berma",
        11.0,
        WORK_SANS,
        ROTULO,
        false,
        Al::Centro,
    );
    l.texto(
        30.0,
        140.0,
        "Cuneta",
        11.0,
        WORK_SANS,
        ROTULO,
        false,
        Al::Centro,
    );
}

fn tuberia_seccion(l: &mut Lapiz) {
    let (c, r, e) = (100.0, 80.0, 12.0);
    l.circulo(c, c, r, Some(TINTA), Some(PERFIL), 2.0);
    l.circulo(c, c, r - e, Some(TINTA), Some(BLANCO), 1.5);
    // El agua hasta el tirante `y`.
    let ri = r - e;
    let nivel = c + 22.0;
    let a0 = ((nivel - c) / ri).asin().to_degrees();
    let pts: Vec<(f32, f32)> = (0..=24)
        .map(|i| {
            let a = (a0 + (180.0 - 2.0 * a0) * i as f32 / 24.0).to_radians();
            (c + ri * a.cos(), c + ri * a.sin())
        })
        .collect();
    forma(l, &pts, AGUA_RAYA, Some(AGUA), 1.25);
    l.linea_con(c - ri, c, c + ri, c, FINA, 1.0, EstiloTrazo::RayaPunto);
    cota_v(l, c + r + 24.0, c - r, c + r, "D");
    cota_v(l, c + 20.0, nivel, c + ri, "y");
}

fn tuberia_perfil(l: &mut Lapiz) {
    let (x0, x1) = (40.0, 520.0);
    let (ya, yb) = (90.0, 130.0);
    terreno(l, &[(0.0, 30.0), (560.0, 40.0)]);
    for (x, y) in [(x0, ya), (x1, yb)] {
        l.rect(
            x - 22.0,
            30.0,
            44.0,
            y,
            Some(TINTA),
            Some(HORMIGON),
            1.5,
            false,
        );
        l.circulo(x, 30.0, 4.0, Some(TINTA), Some(TINTA), 1.0);
    }
    forma(
        l,
        &[
            (x0 + 22.0, ya - 14.0),
            (x1 - 22.0, yb - 14.0),
            (x1 - 22.0, yb + 14.0),
            (x0 + 22.0, ya + 14.0),
        ],
        TINTA,
        Some(PERFIL),
        1.5,
    );
    forma(
        l,
        &[
            (x0 + 22.0, ya - 8.0),
            (x1 - 22.0, yb - 8.0),
            (x1 - 22.0, yb + 8.0),
            (x0 + 22.0, ya + 8.0),
        ],
        AGUA_RAYA,
        Some(AGUA),
        1.0,
    );
    let k = (yb - ya) / (x1 - x0);
    l.flecha(
        &[(160.0, ya + k * 120.0), (360.0, ya + k * 320.0)],
        AGUA_RAYA,
        2.0,
        false,
    );
    l.texto(
        280.0,
        150.0,
        "S = 1 %",
        13.0,
        WORK_SANS,
        TINTA,
        true,
        Al::Centro,
    );
    l.texto(
        x0,
        ya + 40.0,
        "Buzón 1",
        11.0,
        WORK_SANS,
        ROTULO,
        false,
        Al::Centro,
    );
    l.texto(
        x1,
        yb + 40.0,
        "Buzón 2",
        11.0,
        WORK_SANS,
        ROTULO,
        false,
        Al::Centro,
    );
}

fn curvas_de_nivel(l: &mut Lapiz) {
    let (cx, cy) = (220.0, 170.0);
    let n = 7;
    for k in 0..n {
        let base = 160.0 - 21.0 * k as f32;
        let pts: Vec<(f32, f32)> = circulo_de_puntos(0.0, 0.0, 1.0, 1.0, 72)
            .into_iter()
            .map(|(x, y)| {
                let a = y.atan2(x);
                let r = base
                    * (1.0
                        + 0.16 * (2.0 * a + 0.6).sin()
                        + 0.08 * (3.0 * a + 1.3 + k as f32 * 0.2).sin()
                        + 0.04 * (5.0 * a).sin());
                (
                    cx + 1.3 * r * x + 6.0 * k as f32,
                    cy + r * y - 3.0 * k as f32,
                )
            })
            .collect();
        let maestra = k % 5 == 0;
        forma(
            l,
            &pts,
            hex(0x9b6a3c),
            None,
            if maestra { 2.25 } else { 1.0 },
        );
        let (lx, ly) = pts[0];
        l.rect(
            lx - 14.0,
            ly - 8.0,
            28.0,
            16.0,
            None,
            Some(BLANCO),
            1.0,
            false,
        );
        l.texto(
            lx,
            ly - 7.0,
            &format!("{}", 100 + 5 * k),
            10.0,
            WORK_SANS,
            hex(0x9b6a3c),
            maestra,
            Al::Centro,
        );
    }
    let (px, py) = (cx + 40.0, cy - 20.0);
    forma(
        l,
        &[(px, py - 8.0), (px + 7.0, py + 5.0), (px - 7.0, py + 5.0)],
        TINTA,
        Some(TINTA),
        1.0,
    );
    l.texto(
        px + 10.0,
        py - 4.0,
        "131.4",
        10.0,
        WORK_SANS,
        TINTA,
        true,
        Al::Izq,
    );
}

fn simbolos_topograficos(l: &mut Lapiz) {
    let (w, fila) = (320.0, 40.0);
    let n = 8;
    l.rect(
        0.0,
        0.0,
        w,
        50.0 + fila * n as f32,
        Some(TINTA),
        Some(BLANCO),
        1.5,
        false,
    );
    l.texto(
        w / 2.0,
        14.0,
        "SÍMBOLOS TOPOGRÁFICOS",
        13.0,
        WORK_SANS,
        TINTA,
        true,
        Al::Centro,
    );
    l.linea(0.0, 40.0, w, 40.0, TINTA, 1.0);
    let nombres = [
        "Vértice geodésico",
        "Banco de nivel (BM)",
        "Estación / estaca",
        "Punto de cota",
        "Poste",
        "Pozo / buzón",
        "Árbol",
        "Cerco / alambrado",
    ];
    for (i, s) in nombres.iter().enumerate() {
        let (x, y) = (40.0, 60.0 + fila * i as f32);
        match i {
            0 => {
                forma(
                    l,
                    &[(x, y - 11.0), (x + 12.0, y + 9.0), (x - 12.0, y + 9.0)],
                    TINTA,
                    Some(BLANCO),
                    1.5,
                );
                l.circulo(x, y + 2.0, 2.5, Some(TINTA), Some(TINTA), 1.0);
            }
            1 => {
                l.rect(
                    x - 10.0,
                    y - 10.0,
                    20.0,
                    20.0,
                    Some(TINTA),
                    Some(BLANCO),
                    1.5,
                    false,
                );
                forma(
                    l,
                    &[
                        (x - 10.0, y + 10.0),
                        (x + 10.0, y - 10.0),
                        (x + 10.0, y + 10.0),
                    ],
                    TINTA,
                    Some(TINTA),
                    1.0,
                );
            }
            2 => {
                l.circulo(x, y, 10.0, Some(TINTA), Some(BLANCO), 1.5);
                l.linea(x - 14.0, y, x + 14.0, y, TINTA, 1.0);
                l.linea(x, y - 14.0, x, y + 14.0, TINTA, 1.0);
            }
            3 => {
                l.linea(x - 6.0, y - 6.0, x + 6.0, y + 6.0, TINTA, 1.5);
                l.linea(x + 6.0, y - 6.0, x - 6.0, y + 6.0, TINTA, 1.5);
                l.texto(
                    x + 10.0,
                    y - 14.0,
                    "125.30",
                    9.0,
                    WORK_SANS,
                    TINTA,
                    false,
                    Al::Izq,
                );
            }
            4 => l.circulo(x, y, 5.0, Some(TINTA), Some(TINTA), 1.0),
            5 => {
                l.circulo(x, y, 11.0, Some(TINTA), Some(GRIS), 1.5);
                l.circulo(x, y, 5.0, Some(TINTA), Some(BLANCO), 1.0);
            }
            6 => {
                l.circulo(x, y, 11.0, Some(hex(0x4f7d3f)), Some(hex(0xd7e8cb)), 1.5);
                l.circulo(x, y, 2.0, Some(hex(0x4f7d3f)), Some(hex(0x4f7d3f)), 1.0);
            }
            _ => {
                l.linea(x - 22.0, y, x + 22.0, y, TINTA, 1.25);
                for dx in [-14.0, 0.0, 14.0] {
                    l.linea(x + dx - 4.0, y - 4.0, x + dx + 4.0, y + 4.0, TINTA, 1.25);
                    l.linea(x + dx + 4.0, y - 4.0, x + dx - 4.0, y + 4.0, TINTA, 1.25);
                }
            }
        }
        l.texto(90.0, y - 8.0, s, 12.0, WORK_SANS, TINTA, false, Al::Izq);
    }
}

// --------------------------------------------------------------- estructuras

/// Un apoyo fijo con la punta en `(x, y)`: triangulo sobre el suelo rayado.
fn apoyo_fijo(l: &mut Lapiz, x: f32, y: f32) {
    forma(
        l,
        &[(x, y), (x + 18.0, y + 30.0), (x - 18.0, y + 30.0)],
        TINTA,
        Some(BLANCO),
        1.75,
    );
    l.circulo(x, y, 4.0, Some(TINTA), Some(BLANCO), 1.5);
    suelo_rayado(l, x - 28.0, x + 28.0, y + 30.0);
}

/// Un apoyo movil: el triangulo sobre dos rodillos.
fn apoyo_movil(l: &mut Lapiz, x: f32, y: f32) {
    forma(
        l,
        &[(x, y), (x + 18.0, y + 28.0), (x - 18.0, y + 28.0)],
        TINTA,
        Some(BLANCO),
        1.75,
    );
    l.circulo(x, y, 4.0, Some(TINTA), Some(BLANCO), 1.5);
    for dx in [-10.0, 10.0] {
        l.circulo(x + dx, y + 34.0, 5.5, Some(TINTA), Some(BLANCO), 1.5);
    }
    suelo_rayado(l, x - 28.0, x + 28.0, y + 40.0);
}

/// La raya del suelo con sus rayitas.
fn suelo_rayado(l: &mut Lapiz, x0: f32, x1: f32, y: f32) {
    l.linea(x0, y, x1, y, TINTA, 2.0);
    let mut x = x0 + 6.0;
    while x <= x1 {
        l.linea(x, y, x - 8.0, y + 9.0, TINTA, 1.0);
        x += 8.0;
    }
}

/// Un empotramiento en una pared vertical en `x`, rayado por la izquierda.
fn empotramiento(l: &mut Lapiz, x: f32, y0: f32, y1: f32) {
    l.linea(x, y0, x, y1, TINTA, 2.5);
    let mut y = y0 + 4.0;
    while y <= y1 {
        l.linea(x, y, x - 10.0, y + 9.0, TINTA, 1.0);
        y += 9.0;
    }
}

/// Unas flechas de carga hacia abajo, de `x0` a `x1`, con la raya de
/// arriba si es repartida.
fn carga_repartida(l: &mut Lapiz, x0: f32, x1: f32, arriba: f32, abajo: f32, n: usize) {
    l.linea(x0, arriba, x1, arriba, CARGA, 1.5);
    for i in 0..=n {
        let x = x0 + (x1 - x0) * i as f32 / n as f32;
        l.flecha(&[(x, arriba), (x, abajo)], CARGA, 1.5, false);
    }
}

fn celosia(l: &mut Lapiz) {
    let (x0, paso, yb, yt) = (40.0, 80.0, 160.0, 80.0);
    let b = |i: usize| (x0 + paso * i as f32, yb);
    let t = |i: usize| (x0 + paso * i as f32, yt);
    let mut barras = vec![(b(0), b(6)), (t(1), t(5)), (b(0), t(1)), (b(6), t(5))];
    for i in 1..=5 {
        barras.push((t(i), b(i)));
    }
    for i in 1..=2 {
        barras.push((t(i), b(i + 1)));
    }
    for i in 4..=5 {
        barras.push((t(i), b(i - 1)));
    }
    for (a, c) in barras {
        l.linea(a.0, a.1, c.0, c.1, TINTA, 3.0);
    }
    for i in 0..=6 {
        let (x, y) = b(i);
        l.circulo(x, y, 4.5, Some(TINTA), Some(BLANCO), 1.5);
        if (1..=5).contains(&i) {
            let (x, y) = t(i);
            l.circulo(x, y, 4.5, Some(TINTA), Some(BLANCO), 1.5);
            l.flecha(&[(x, y - 50.0), (x, y - 8.0)], CARGA, 1.75, false);
            l.texto(x, y - 70.0, "P", 13.0, WORK_SANS, CARGA, true, Al::Centro);
        }
    }
    apoyo_fijo(l, x0, yb + 5.0);
    apoyo_movil(l, x0 + paso * 6.0, yb + 5.0);
}

fn viga_con_carga(l: &mut Lapiz) {
    let (x0, x1) = (40.0, 440.0);
    carga_repartida(l, x0, x1, 40.0, 92.0, 10);
    l.texto(
        (x0 + x1) / 2.0,
        14.0,
        "q = 10 kN/m",
        13.0,
        WORK_SANS,
        CARGA,
        true,
        Al::Centro,
    );
    l.rect(
        x0,
        94.0,
        x1 - x0,
        12.0,
        Some(TINTA),
        Some(HORMIGON),
        1.75,
        false,
    );
    apoyo_fijo(l, x0, 106.0);
    apoyo_movil(l, x1, 106.0);
    cota_h(l, x0, x1, 190.0, "L = 6.00 m");
}

fn diagramas(l: &mut Lapiz) {
    let (x0, x1) = (60.0, 460.0);
    let xm = (x0 + x1) / 2.0;
    carga_repartida(l, x0, x1, 20.0, 62.0, 10);
    l.texto(xm, -4.0, "q", 14.0, WORK_SANS, CARGA, true, Al::Centro);
    l.rect(
        x0,
        64.0,
        x1 - x0,
        10.0,
        Some(TINTA),
        Some(HORMIGON),
        1.75,
        false,
    );
    apoyo_fijo(l, x0, 74.0);
    apoyo_movil(l, x1, 74.0);
    // Cortantes: recta de +qL/2 a -qL/2.
    let yv = 200.0;
    forma(
        l,
        &[(x0, yv), (x0, yv - 50.0), (xm, yv)],
        CARGA,
        Some(POSITIVO),
        1.5,
    );
    forma(
        l,
        &[(xm, yv), (x1, yv + 50.0), (x1, yv)],
        ACERO,
        Some(NEGATIVO),
        1.5,
    );
    l.linea(x0 - 10.0, yv, x1 + 10.0, yv, TINTA, 1.25);
    l.texto(
        x0 - 16.0,
        yv - 10.0,
        "V",
        16.0,
        WORK_SANS,
        TINTA,
        true,
        Al::Der,
    );
    l.texto(
        x0 + 6.0,
        yv - 70.0,
        "+qL/2",
        12.0,
        WORK_SANS,
        CARGA,
        true,
        Al::Izq,
    );
    l.texto(
        x1 - 6.0,
        yv + 54.0,
        "−qL/2",
        12.0,
        WORK_SANS,
        ACERO,
        true,
        Al::Der,
    );
    // Momentos: la parabola, maxima en el centro.
    let ym = 300.0;
    let mut par: Vec<(f32, f32)> = (0..=24)
        .map(|i| {
            let t = i as f32 / 24.0;
            (x0 + (x1 - x0) * t, ym + 70.0 * 4.0 * t * (1.0 - t))
        })
        .collect();
    par.push((x0, ym));
    forma(l, &par, CARGA, Some(POSITIVO), 1.5);
    l.linea(x0 - 10.0, ym, x1 + 10.0, ym, TINTA, 1.25);
    l.texto(
        x0 - 16.0,
        ym - 10.0,
        "M",
        16.0,
        WORK_SANS,
        TINTA,
        true,
        Al::Der,
    );
    l.linea_con(xm, ym, xm, ym + 70.0, CARGA, 1.0, EstiloTrazo::Discontinuo);
    l.texto(
        xm + 6.0,
        ym + 74.0,
        "qL²/8",
        12.0,
        WORK_SANS,
        CARGA,
        true,
        Al::Izq,
    );
    for x in [x0, x1] {
        l.linea_con(x, 120.0, x, ym + 10.0, GRIS, 1.0, EstiloTrazo::Discontinuo);
    }
}

fn voladizo(l: &mut Lapiz) {
    empotramiento(l, 30.0, 40.0, 140.0);
    l.rect(
        30.0,
        84.0,
        340.0,
        12.0,
        Some(TINTA),
        Some(HORMIGON),
        1.75,
        false,
    );
    l.flecha(&[(366.0, 20.0), (366.0, 80.0)], CARGA, 2.5, false);
    l.texto(380.0, 20.0, "P", 16.0, WORK_SANS, CARGA, true, Al::Izq);
    cota_h(l, 30.0, 366.0, 150.0, "L");
    // La deformada, a trazos.
    let def: Vec<(f32, f32)> = (0..=16)
        .map(|i| {
            let t = i as f32 / 16.0;
            (30.0 + 336.0 * t, 90.0 + 40.0 * t * t * (1.5 - 0.5 * t))
        })
        .collect();
    l.poli(&def, ACERO, 1.25, None);
    if let Some(e) = l.v.last_mut() {
        e.estilo = EstiloTrazo::Discontinuo;
    }
}
