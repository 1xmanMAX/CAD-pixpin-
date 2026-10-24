//! **Biologia de secundaria**: las celulas (animal, vegetal y bacteria),
//! el ADN y la neurona, con sus partes rotuladas al lado para estudiarlas
//! o borrar los nombres y preguntarlos.

use super::*;
use crate::elemento::EstiloTrazo;

const CATEGORIA: &str = "Biología";
const PREFIJO: &str = "ciencia-";
const CITOPLASMA: ColorRgba = hex(0xfdf0e6);
const MEMBRANA: ColorRgba = hex(0xc87a5a);
const NUCLEO: ColorRgba = hex(0xd8c7f0);
const MITOCONDRIA: ColorRgba = hex(0xf7b98e);
const RETICULO: ColorRgba = hex(0x6fa8dc);
const GOLGI: ColorRgba = hex(0xe8a33d);
const CLOROPLASTO: ColorRgba = hex(0x86c06c);
const VACUOLA: ColorRgba = hex(0xd6ecf7);
const MIELINA: ColorRgba = hex(0xf6dc8c);

pub(super) fn todas(v: &mut Vec<FiguraGuardada>) {
    let lista: [Dibujo; 5] = [
        ("celula-animal", "Célula animal", celula_animal),
        ("celula-vegetal", "Célula vegetal", celula_vegetal),
        ("bacteria", "Célula procariota (bacteria)", bacteria),
        ("adn", "ADN (doble hélice)", adn),
        ("neurona", "Neurona", neurona),
    ];
    meter(v, PREFIJO, CATEGORIA, &lista);
}

/// Una mitocondria con sus crestas, tumbada `giro` grados.
fn mitocondria(l: &mut Lapiz, c: (f32, f32), giro: f32) {
    let caja: Vec<(f32, f32)> = (0..24)
        .map(|i| {
            let a = (i as f32 * 15.0).to_radians();
            (c.0 + 24.0 * a.cos(), c.1 + 11.0 * a.sin())
        })
        .collect();
    forma(
        l,
        &girar(&caja, c, giro),
        hex(0xb5603a),
        Some(MITOCONDRIA),
        1.5,
    );
    let crestas = zigzag((c.0 - 18.0, c.1), (c.0 + 18.0, c.1), 4, 6.0, 2.0);
    l.poli(&girar(&crestas, c, giro), hex(0xb5603a), 1.2, None);
}

/// El nucleo con su nucleolo.
fn nucleo(l: &mut Lapiz, c: (f32, f32), r: f32) {
    l.circulo(c.0, c.1, r, Some(hex(0x7a5bb0)), Some(NUCLEO), G);
    l.circulo(
        c.0 + r * 0.2,
        c.1 - r * 0.1,
        r * 0.32,
        None,
        Some(hex(0x8e6cc6)),
        1.0,
    );
}

/// Unos ribosomas sueltos.
fn ribosomas(l: &mut Lapiz, pts: &[(f32, f32)]) {
    for p in pts {
        l.circulo(p.0, p.1, 2.2, None, Some(hex(0x5b4a8a)), 1.0);
    }
}

/// El aparato de Golgi: unas cisternas curvas apiladas.
fn golgi(l: &mut Lapiz, c: (f32, f32)) {
    for i in 0..4 {
        let r = 26.0 + 7.0 * i as f32;
        l.arco(c.0, c.1 + r, r, 240.0, 300.0, GOLGI, 4.0);
    }
    l.circulo(c.0 + 22.0, c.1 + 6.0, 3.5, Some(GOLGI), None, 1.5);
    l.circulo(c.0 - 22.0, c.1 + 8.0, 3.0, Some(GOLGI), None, 1.5);
}

fn celula_animal(l: &mut Lapiz) {
    let borde = [
        (40.0, 60.0),
        (130.0, 14.0),
        (240.0, 30.0),
        (310.0, 110.0),
        (290.0, 210.0),
        (180.0, 250.0),
        (70.0, 230.0),
        (16.0, 150.0),
    ];
    blanda(l, &borde, MEMBRANA, Some(CITOPLASMA), 3.0);
    // El reticulo: ondas alrededor del nucleo.
    for i in 0..3 {
        let r = 56.0 + 9.0 * i as f32;
        let pts: Vec<(f32, f32)> = (0..=30)
            .map(|k| {
                let a = (110.0 + 120.0 * k as f32 / 30.0).to_radians();
                let rr = r + 3.0 * (k as f32 * 1.6).sin();
                (160.0 + rr * a.cos(), 130.0 + rr * a.sin())
            })
            .collect();
        l.poli(&pts, RETICULO, 2.0, None);
    }
    nucleo(l, (160.0, 130.0), 42.0);
    mitocondria(l, (240.0, 80.0), 30.0);
    mitocondria(l, (230.0, 196.0), -20.0);
    mitocondria(l, (80.0, 90.0), -35.0);
    golgi(l, (250.0, 140.0));
    l.circulo(
        100.0,
        200.0,
        11.0,
        Some(hex(0x9c4f8f)),
        Some(hex(0xf0c9e6)),
        1.5,
    );
    ribosomas(
        l,
        &[
            (120.0, 60.0),
            (190.0, 54.0),
            (60.0, 160.0),
            (150.0, 214.0),
            (275.0, 110.0),
            (205.0, 225.0),
            (110.0, 175.0),
        ],
    );
    rotulos_derecha(
        l,
        350.0,
        30.0,
        38.0,
        &[
            ((296.0, 72.0), "Membrana plasmática"),
            ((240.0, 80.0), "Mitocondria"),
            ((275.0, 110.0), "Ribosomas"),
            ((198.0, 118.0), "Núcleo"),
            ((166.0, 126.0), "Nucléolo"),
            ((262.0, 140.0), "Aparato de Golgi"),
        ],
    );
    rotulos_izquierda(
        l,
        -16.0,
        90.0,
        56.0,
        &[
            ((60.0, 100.0), "Citoplasma"),
            ((95.0, 130.0), "Retículo endoplasmático"),
            ((96.0, 200.0), "Lisosoma"),
        ],
    );
}

fn celula_vegetal(l: &mut Lapiz) {
    l.rect(
        10.0,
        10.0,
        320.0,
        230.0,
        Some(hex(0x4f8a3a)),
        Some(hex(0xcfe6b8)),
        5.0,
        true,
    );
    l.rect(
        20.0,
        20.0,
        300.0,
        210.0,
        Some(hex(0x7aa860)),
        Some(hex(0xf3f8e8)),
        1.5,
        true,
    );
    l.rect(
        110.0,
        50.0,
        190.0,
        150.0,
        Some(hex(0x6aa6c8)),
        Some(VACUOLA),
        1.5,
        true,
    );
    nucleo(l, (64.0, 70.0), 30.0);
    for (c, g) in [
        ((60.0, 170.0), 20.0),
        ((150.0, 214.0), 0.0),
        ((250.0, 214.0), -10.0),
        ((306.0, 120.0), 90.0),
        ((200.0, 34.0), 0.0),
    ] {
        let caja: Vec<(f32, f32)> = (0..24)
            .map(|i| {
                let a = (i as f32 * 15.0).to_radians();
                (c.0 + 18.0 * a.cos(), c.1 + 8.0 * a.sin())
            })
            .collect();
        forma(
            l,
            &girar(&caja, c, g),
            hex(0x3f7d2e),
            Some(CLOROPLASTO),
            1.5,
        );
        for k in [-8.0, 0.0, 8.0] {
            let p = girar(&[(c.0 + k, c.1 - 4.0), (c.0 + k, c.1 + 4.0)], c, g);
            l.linea(p[0].0, p[0].1, p[1].0, p[1].1, hex(0x3f7d2e), 1.5);
        }
    }
    mitocondria(l, (60.0, 120.0), 70.0);
    rotulos_derecha(
        l,
        370.0,
        20.0,
        52.0,
        &[
            ((328.0, 40.0), "Pared celular"),
            ((319.0, 76.0), "Membrana plasmática"),
            ((306.0, 128.0), "Cloroplasto"),
            ((292.0, 176.0), "Vacuola"),
        ],
    );
    rotulos_izquierda(
        l,
        -14.0,
        64.0,
        66.0,
        &[
            ((38.0, 66.0), "Núcleo"),
            ((56.0, 122.0), "Mitocondria"),
            ((40.0, 212.0), "Citoplasma"),
        ],
    );
}

fn bacteria(l: &mut Lapiz) {
    // El flagelo, por detras.
    let flagelo = curva(
        |x| 110.0 + 12.0 * ((x - 250.0) / 14.0).sin(),
        250.0,
        350.0,
        40,
    );
    l.poli(&flagelo, hex(0x8a6a4a), 2.0, None);
    for (a, b) in [
        ((70.0, 56.0), (60.0, 36.0)),
        ((120.0, 50.0), (122.0, 28.0)),
        ((180.0, 52.0), (190.0, 30.0)),
        ((90.0, 168.0), (80.0, 188.0)),
        ((170.0, 170.0), (176.0, 192.0)),
    ] {
        l.linea(a.0, a.1, b.0, b.1, hex(0x8a6a4a), 1.5);
    }
    l.rect(
        30.0,
        46.0,
        230.0,
        130.0,
        Some(hex(0xb7a5d9)),
        Some(hex(0xf1ecfa)),
        2.0,
        true,
    );
    l.v.last_mut().unwrap().estilo = EstiloTrazo::Discontinuo;
    l.rect(
        40.0,
        56.0,
        210.0,
        110.0,
        Some(hex(0x7a5bb0)),
        Some(hex(0xe8f1d8)),
        3.0,
        true,
    );
    l.rect(
        46.0,
        62.0,
        198.0,
        98.0,
        Some(hex(0xa6c48a)),
        None,
        1.2,
        true,
    );
    let adn: Vec<(f32, f32)> = (0..=80)
        .map(|i| {
            let t = i as f32 / 80.0 * std::f32::consts::TAU * 3.0;
            (
                145.0 + (40.0 + 8.0 * (t * 3.0).sin()) * (t / 3.0).cos(),
                110.0 + (22.0 + 6.0 * (t * 2.0).cos()) * (t / 3.0).sin(),
            )
        })
        .collect();
    l.poli(&adn, hex(0x7a5bb0), 1.5, None);
    l.circulo(214.0, 136.0, 9.0, Some(hex(0xc0692b)), None, 1.5);
    ribosomas(
        l,
        &[
            (70.0, 80.0),
            (80.0, 140.0),
            (200.0, 80.0),
            (220.0, 100.0),
            (110.0, 150.0),
            (180.0, 146.0),
        ],
    );
    rotulo(l, (40.0, 120.0), (14.0, 220.0), "Pared celular", Al::Izq);
    rotulo(l, (30.0, 70.0), (10.0, 20.0), "Cápsula", Al::Izq);
    rotulo(
        l,
        (145.0, 110.0),
        (130.0, 240.0),
        "Nucleoide (ADN)",
        Al::Izq,
    );
    rotulo(l, (214.0, 136.0), (240.0, 220.0), "Plásmido", Al::Izq);
    rotulo(l, (200.0, 80.0), (220.0, 14.0), "Ribosomas", Al::Izq);
    rotulo(l, (330.0, 104.0), (320.0, 160.0), "Flagelo", Al::Izq);
    rotulo(l, (122.0, 28.0), (100.0, 4.0), "Pili", Al::Der);
}

fn adn(l: &mut Lapiz) {
    let (y0, amp, periodo) = (90.0, 60.0, 160.0);
    let hebra = |x: f32, fase: f32| y0 + amp * (std::f32::consts::TAU * x / periodo + fase).sin();
    let pares = [
        (ROJO, AZUL, "A", "T"),
        (VERDE, NARANJA, "G", "C"),
        (AZUL, ROJO, "T", "A"),
        (NARANJA, VERDE, "C", "G"),
    ];
    let mut x = 14.0;
    let mut i = 0;
    while x < 400.0 {
        let (a, b) = (hebra(x, 0.0), hebra(x, std::f32::consts::PI));
        let m = (a + b) / 2.0;
        let (ca, cb, _, _) = pares[i % pares.len()];
        l.linea(x, a, x, m, ca, 4.0);
        l.linea(x, m, x, b, cb, 4.0);
        x += 13.0;
        i += 1;
    }
    l.poli(
        &curva(|x| hebra(x, 0.0), 0.0, 410.0, 120),
        hex(0x4a4f5a),
        6.0,
        None,
    );
    l.poli(
        &curva(|x| hebra(x, std::f32::consts::PI), 0.0, 410.0, 120),
        hex(0x8a94a6),
        6.0,
        None,
    );
    for (k, (ca, cb, sa, sb)) in pares.iter().enumerate() {
        let x = 20.0 + 100.0 * k as f32;
        l.rect(x, 184.0, 16.0, 12.0, None, Some(*ca), 1.0, false);
        l.rect(x + 16.0, 184.0, 16.0, 12.0, None, Some(*cb), 1.0, false);
        txt_a(l, x + 38.0, 190.0, &format!("{sa}–{sb}"), 11.0, Al::Izq);
    }
    txt(
        l,
        205.0,
        222.0,
        "A: adenina · T: timina · G: guanina · C: citosina",
        11.0,
        GRIS,
    );
    rotulo(
        l,
        (40.0, hebra(40.0, 0.0)),
        (60.0, 6.0),
        "esqueleto de azúcar y fosfato",
        Al::Izq,
    );
}

fn neurona(l: &mut Lapiz) {
    let soma = (80.0, 120.0);
    // Las dendritas: ramas que salen del soma.
    for g in [150.0f32, 190.0, 225.0, 260.0, 300.0, 110.0] {
        let a = g.to_radians();
        let p0 = (soma.0 + 26.0 * a.cos(), soma.1 + 26.0 * a.sin());
        let p1 = (soma.0 + 62.0 * a.cos(), soma.1 + 62.0 * a.sin());
        l.poli(&[p0, p1], hex(0x9c4f8f), 3.0, None);
        for d in [-28.0f32, 28.0] {
            let b = (g + d).to_radians();
            l.poli(
                &[p1, (p1.0 + 24.0 * b.cos(), p1.1 + 24.0 * b.sin())],
                hex(0x9c4f8f),
                1.8,
                None,
            );
        }
    }
    blanda(
        l,
        &[
            (52.0, 100.0),
            (80.0, 88.0),
            (112.0, 104.0),
            (116.0, 132.0),
            (90.0, 152.0),
            (58.0, 142.0),
        ],
        hex(0x9c4f8f),
        Some(hex(0xf0c9e6)),
        G,
    );
    nucleo(l, (82.0, 120.0), 13.0);
    // El axon con su mielina a trozos.
    l.linea(112.0, 120.0, 420.0, 120.0, hex(0x9c4f8f), 3.0);
    let mut x = 140.0;
    while x < 380.0 {
        l.rect(
            x,
            110.0,
            42.0,
            20.0,
            Some(hex(0xc9a640)),
            Some(MIELINA),
            1.5,
            true,
        );
        x += 52.0;
    }
    // Los terminales.
    for d in [-30.0f32, -12.0, 8.0, 26.0] {
        let fin = (470.0, 120.0 + d * 1.6);
        l.poli(
            &[(420.0, 120.0), (444.0, 120.0 + d * 0.8), fin],
            hex(0x9c4f8f),
            2.0,
            None,
        );
        l.circulo(
            fin.0 + 4.0,
            fin.1,
            5.0,
            Some(hex(0x9c4f8f)),
            Some(hex(0xf0c9e6)),
            1.5,
        );
    }
    l.flecha(&[(200.0, 160.0), (300.0, 160.0)], ROJO, 1.5, false);
    txt(l, 250.0, 174.0, "impulso nervioso", 10.0, ROJO);
    rotulo(l, (30.0, 90.0), (14.0, 14.0), "Dendritas", Al::Izq);
    rotulo(l, (100.0, 140.0), (60.0, 220.0), "Soma", Al::Izq);
    rotulo(l, (82.0, 120.0), (120.0, 200.0), "Núcleo", Al::Izq);
    rotulo(
        l,
        (161.0, 110.0),
        (150.0, 40.0),
        "Vaina de mielina",
        Al::Izq,
    );
    rotulo(l, (187.0, 120.0), (230.0, 70.0), "Nodo de Ranvier", Al::Izq);
    rotulo(l, (400.0, 120.0), (370.0, 40.0), "Axón", Al::Izq);
    rotulo(l, (474.0, 72.0), (440.0, 20.0), "Terminal axónico", Al::Izq);
}
