//! **Los simbolos de storyboard**: siluetas, flechas de movimiento de
//! camara, camaras, soportes, luces, sonido y marcos de encuadre. Trazo de
//! carbon con relleno claro y los detalles (objetivo, piloto de grabar,
//! flechas) en rojo, para que se lean encima de cualquier boceto.

use super::{Al, Lapiz, WORK_SANS, hex};
use crate::biblioteca::FiguraGuardada;
use crate::elemento::{ColorRgba, EstiloTrazo};

const CATEGORIA: &str = "Storyboard";
const TINTA: ColorRgba = hex(0x2b2f3a);
const CLARO: ColorRgba = hex(0xeef1f6);
const ROJO: ColorRgba = hex(0xd1453b);
const ROSA: ColorRgba = hex(0xfbe3e1);
const BLANCO: ColorRgba = hex(0xffffff);
const G: f32 = 2.0;

/// Una figura de la lista: su clave, su nombre y como se dibuja.
type DeLaLista = (&'static str, &'static str, fn(&mut Lapiz));

pub(super) fn todas(v: &mut Vec<FiguraGuardada>) {
    let lista: [DeLaLista; 39] = [
        ("persona-frente", "Persona de frente", persona_frente),
        ("persona-lado", "Persona de lado", persona_lado),
        (
            "persona-arriba",
            "Persona vista desde arriba",
            persona_arriba,
        ),
        ("flecha-recta", "Flecha", flecha_recta),
        ("flecha-diagonal", "Flecha diagonal", flecha_diagonal),
        ("flecha-arriba", "Flecha hacia arriba", flecha_arriba),
        ("flecha-doble", "Flecha doble", flecha_doble),
        ("flecha-curva", "Flecha curva", flecha_curva),
        ("flecha-giro", "Giro", flecha_giro),
        ("flecha-trayectoria", "Trayectoria", trayectoria),
        ("zoom-acercar", "Zoom: acercar", |l| zoom(l, true)),
        ("zoom-alejar", "Zoom: alejar", |l| zoom(l, false)),
        ("paneo-derecha", "Paneo a la derecha", |l| paneo(l, true)),
        ("paneo-izquierda", "Paneo a la izquierda", |l| {
            paneo(l, false)
        }),
        ("inclinar-arriba", "Inclinar hacia arriba", |l| {
            inclinar(l, true)
        }),
        ("inclinar-abajo", "Inclinar hacia abajo", |l| {
            inclinar(l, false)
        }),
        ("camara-cine", "Cámara de cine", camara_cine),
        ("camara-video", "Cámara de vídeo", camara_video),
        ("camara-tripode", "Cámara en trípode", camara_tripode),
        ("camara-reflex", "Cámara réflex", camara_reflex),
        ("camara-accion", "Cámara de acción", camara_accion),
        ("movil", "Móvil", movil),
        ("camara-360", "Cámara 360°", camara_360),
        ("camara-arriba", "Cámara vista desde arriba", camara_arriba),
        ("dron", "Dron", dron),
        ("dron-arriba", "Dron visto desde arriba", dron_arriba),
        ("grua", "Grúa", grua),
        ("rieles", "Rieles (dolly)", rieles),
        ("foco", "Foco", foco),
        ("foco-arriba", "Foco visto desde arriba", foco_arriba),
        ("softbox", "Softbox", softbox),
        ("panel-led", "Panel LED", panel_led),
        ("reflector", "Paraguas reflector", reflector),
        ("microfono", "Micrófono de mano", microfono),
        ("boom", "Micrófono boom", boom),
        ("solapa", "Micrófono de solapa", solapa),
        ("claqueta", "Claqueta", claqueta),
        ("marco-16-9", "Marco 16:9", |l| marco(l, 16.0 / 9.0, "16:9")),
        ("marco-4-3", "Marco 4:3", |l| marco(l, 4.0 / 3.0, "4:3")),
    ];
    for (clave, nombre, dibujar) in lista {
        let mut l = Lapiz::default();
        dibujar(&mut l);
        v.push(l.figura(clave, CATEGORIA, nombre));
    }
    let mut l = Lapiz::default();
    marco(&mut l, 2.39, "2.39:1");
    v.push(l.figura("marco-cinemascope", CATEGORIA, "Marco cinemascope"));
}

/// Una forma cerrada con relleno.
fn forma(l: &mut Lapiz, pts: &[(f32, f32)], trazo: ColorRgba, relleno: ColorRgba) {
    let mut v = pts.to_vec();
    if v.first() != v.last() {
        v.push(v[0]);
    }
    l.poli(&v, trazo, G, Some(relleno));
}

/// Gira unos puntos alrededor de `c`.
fn girar(pts: &[(f32, f32)], c: (f32, f32), grados: f32) -> Vec<(f32, f32)> {
    let (s, co) = grados.to_radians().sin_cos();
    pts.iter()
        .map(|(x, y)| {
            let (dx, dy) = (x - c.0, y - c.1);
            (c.0 + dx * co - dy * s, c.1 + dx * s + dy * co)
        })
        .collect()
}

// ------------------------------------------------------------------ personas

fn persona_frente(l: &mut Lapiz) {
    forma(
        l,
        &[
            (36.0, 40.0),
            (64.0, 40.0),
            (76.0, 48.0),
            (82.0, 104.0),
            (74.0, 106.0),
            (68.0, 62.0),
            (66.0, 124.0),
            (64.0, 196.0),
            (53.0, 196.0),
            (50.0, 132.0),
            (47.0, 196.0),
            (36.0, 196.0),
            (34.0, 124.0),
            (32.0, 62.0),
            (26.0, 106.0),
            (18.0, 104.0),
            (24.0, 48.0),
        ],
        TINTA,
        CLARO,
    );
    l.circulo(50.0, 20.0, 14.0, Some(TINTA), Some(CLARO), G);
}

fn persona_lado(l: &mut Lapiz) {
    forma(
        l,
        &[
            (42.0, 40.0),
            (58.0, 40.0),
            (63.0, 64.0),
            (60.0, 124.0),
            (66.0, 192.0),
            (72.0, 196.0),
            (52.0, 196.0),
            (51.0, 134.0),
            (47.0, 196.0),
            (36.0, 196.0),
            (41.0, 124.0),
            (38.0, 64.0),
        ],
        TINTA,
        CLARO,
    );
    l.poli(&[(52.0, 48.0), (56.0, 84.0), (62.0, 108.0)], TINTA, G, None);
    l.circulo(50.0, 20.0, 14.0, Some(TINTA), Some(CLARO), G);
    l.poli(&[(63.0, 16.0), (68.0, 22.0), (63.0, 24.0)], TINTA, G, None);
}

fn persona_arriba(l: &mut Lapiz) {
    l.elipse(0.0, 20.0, 110.0, 44.0, Some(TINTA), Some(CLARO), G);
    l.circulo(55.0, 42.0, 17.0, Some(TINTA), Some(BLANCO), G);
    forma(l, &[(49.0, 59.0), (55.0, 72.0), (61.0, 59.0)], ROJO, ROJO);
}

// ------------------------------------------------------------------- flechas

const FLECHA: [(f32, f32); 7] = [
    (0.0, 22.0),
    (78.0, 22.0),
    (78.0, 0.0),
    (124.0, 37.0),
    (78.0, 74.0),
    (78.0, 52.0),
    (0.0, 52.0),
];

fn flecha_recta(l: &mut Lapiz) {
    forma(l, &FLECHA, ROJO, ROSA);
}

fn flecha_diagonal(l: &mut Lapiz) {
    forma(l, &girar(&FLECHA, (62.0, 37.0), 40.0), ROJO, ROSA);
}

fn flecha_arriba(l: &mut Lapiz) {
    forma(l, &girar(&FLECHA, (62.0, 37.0), -90.0), ROJO, ROSA);
}

fn flecha_doble(l: &mut Lapiz) {
    forma(
        l,
        &[
            (0.0, 37.0),
            (40.0, 0.0),
            (40.0, 22.0),
            (100.0, 22.0),
            (100.0, 0.0),
            (140.0, 37.0),
            (100.0, 74.0),
            (100.0, 52.0),
            (40.0, 52.0),
            (40.0, 74.0),
        ],
        ROJO,
        ROSA,
    );
}

/// Una flecha gruesa que sigue un arco: el borde de fuera, la punta y el
/// borde de dentro, en una sola forma.
fn flecha_en_arco(l: &mut Lapiz, cx: f32, cy: f32, r: f32, desde: f32, hasta: f32) {
    let ancho = 14.0;
    let tramos = (((hasta - desde).abs() / 6.0).ceil() as usize).max(2);
    let punto = |a: f32, rr: f32| {
        let a = a.to_radians();
        (cx + rr * a.cos(), cy + rr * a.sin())
    };
    let mut pts: Vec<(f32, f32)> = (0..=tramos)
        .map(|i| {
            punto(
                desde + (hasta - desde) * i as f32 / tramos as f32,
                r + ancho,
            )
        })
        .collect();
    let signo = (hasta - desde).signum();
    pts.push(punto(hasta, r + ancho + 14.0));
    pts.push(punto(hasta + signo * 22.0, r));
    pts.push(punto(hasta, r - ancho - 14.0));
    pts.extend((0..=tramos).rev().map(|i| {
        punto(
            desde + (hasta - desde) * i as f32 / tramos as f32,
            r - ancho,
        )
    }));
    forma(l, &pts, ROJO, ROSA);
}

fn flecha_curva(l: &mut Lapiz) {
    flecha_en_arco(l, 110.0, 110.0, 80.0, 180.0, 255.0);
}

fn flecha_giro(l: &mut Lapiz) {
    flecha_en_arco(l, 70.0, 70.0, 48.0, 330.0, 90.0 + 360.0 - 30.0);
}

fn trayectoria(l: &mut Lapiz) {
    let pts: Vec<(f32, f32)> = (0..=24)
        .map(|i| {
            let t = i as f32 / 24.0;
            (t * 170.0, 40.0 - 32.0 * (t * std::f32::consts::TAU).sin())
        })
        .collect();
    l.flecha(&pts, ROJO, 3.0, false);
    if let Some(e) = l.v.last_mut() {
        e.estilo = EstiloTrazo::Discontinuo;
    }
    l.circulo(0.0, 40.0, 6.0, None, Some(ROJO), 1.0);
}

fn zoom(l: &mut Lapiz, acercar: bool) {
    let c = 60.0;
    for (dx, dy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let (fuera, dentro) = (
            (c + dx * 54.0, c + dy * 54.0),
            (c + dx * 14.0, c + dy * 14.0),
        );
        let (a, b) = if acercar {
            (fuera, dentro)
        } else {
            (dentro, fuera)
        };
        l.flecha(&[a, b], ROJO, 3.0, false);
    }
    l.rect(c - 30.0, c - 22.0, 60.0, 44.0, Some(TINTA), None, 1.5, true);
    if let Some(e) = l.v.last_mut() {
        e.estilo = EstiloTrazo::Discontinuo;
    }
}

fn paneo(l: &mut Lapiz, derecha: bool) {
    for y in [10.0, 50.0] {
        let (a, b) = if derecha { (0.0, 150.0) } else { (150.0, 0.0) };
        l.flecha(&[(a, y), (b, y)], ROJO, 3.0, false);
    }
    l.rect(55.0, 18.0, 40.0, 24.0, Some(TINTA), Some(CLARO), 1.5, true);
}

fn inclinar(l: &mut Lapiz, arriba: bool) {
    l.arco_eliptico(20.0, 70.0, 16.0, 62.0, -60.0, 60.0, TINTA, G);
    l.arco_eliptico(120.0, 70.0, 16.0, 62.0, 120.0, 240.0, TINTA, G);
    let (a, b) = if arriba { (120.0, 20.0) } else { (20.0, 120.0) };
    l.flecha(&[(70.0, a), (70.0, b)], ROJO, 3.0, false);
}

// ------------------------------------------------------------------- camaras

/// El objetivo de perfil: un cono que se abre hacia delante.
fn objetivo(l: &mut Lapiz, x: f32, cy: f32, largo: f32, alto: f32) {
    forma(
        l,
        &[
            (x, cy - alto * 0.32),
            (x + largo, cy - alto / 2.0),
            (x + largo, cy + alto / 2.0),
            (x, cy + alto * 0.32),
        ],
        TINTA,
        CLARO,
    );
    l.linea(
        x + largo,
        cy - alto / 2.0,
        x + largo,
        cy + alto / 2.0,
        ROJO,
        3.0,
    );
}

fn piloto(l: &mut Lapiz, x: f32, y: f32) {
    l.circulo(x, y, 4.5, None, Some(ROJO), 1.0);
}

fn camara_cine(l: &mut Lapiz) {
    l.circulo(42.0, 30.0, 26.0, Some(TINTA), Some(CLARO), G);
    l.circulo(42.0, 30.0, 6.0, Some(TINTA), None, 1.5);
    l.circulo(100.0, 30.0, 26.0, Some(TINTA), Some(CLARO), G);
    l.circulo(100.0, 30.0, 6.0, Some(TINTA), None, 1.5);
    l.rect(14.0, 56.0, 120.0, 64.0, Some(TINTA), Some(CLARO), G, true);
    objetivo(l, 134.0, 88.0, 34.0, 48.0);
    piloto(l, 30.0, 70.0);
}

fn camara_video(l: &mut Lapiz) {
    l.rect(30.0, 0.0, 70.0, 14.0, Some(TINTA), Some(CLARO), G, true);
    l.linea(40.0, 14.0, 40.0, 24.0, TINTA, G);
    l.linea(90.0, 14.0, 90.0, 24.0, TINTA, G);
    l.rect(10.0, 24.0, 116.0, 58.0, Some(TINTA), Some(CLARO), G, true);
    l.rect(0.0, 34.0, 14.0, 26.0, Some(TINTA), Some(BLANCO), G, true);
    objetivo(l, 126.0, 53.0, 30.0, 44.0);
    piloto(l, 112.0, 36.0);
}

fn camara_tripode(l: &mut Lapiz) {
    l.rect(20.0, 0.0, 90.0, 48.0, Some(TINTA), Some(CLARO), G, true);
    objetivo(l, 110.0, 24.0, 26.0, 36.0);
    piloto(l, 32.0, 12.0);
    l.rect(55.0, 48.0, 20.0, 10.0, Some(TINTA), Some(TINTA), G, false);
    l.linea(65.0, 58.0, 20.0, 170.0, TINTA, G);
    l.linea(65.0, 58.0, 110.0, 170.0, TINTA, G);
    l.linea(65.0, 58.0, 65.0, 168.0, TINTA, G);
    l.linea(40.0, 120.0, 90.0, 120.0, TINTA, 1.5);
}

fn camara_reflex(l: &mut Lapiz) {
    forma(
        l,
        &[
            (0.0, 22.0),
            (40.0, 22.0),
            (50.0, 4.0),
            (90.0, 4.0),
            (100.0, 22.0),
            (150.0, 22.0),
            (150.0, 100.0),
            (0.0, 100.0),
        ],
        TINTA,
        CLARO,
    );
    l.rect(0.0, 22.0, 24.0, 78.0, Some(TINTA), Some(BLANCO), G, true);
    l.circulo(75.0, 62.0, 32.0, Some(TINTA), Some(BLANCO), G);
    l.circulo(75.0, 62.0, 21.0, Some(TINTA), Some(CLARO), G);
    l.circulo(75.0, 62.0, 9.0, Some(ROJO), Some(ROSA), G);
    l.rect(118.0, 32.0, 20.0, 10.0, Some(TINTA), None, 1.5, true);
    piloto(l, 12.0, 34.0);
}

fn camara_accion(l: &mut Lapiz) {
    l.rect(0.0, 0.0, 110.0, 80.0, Some(TINTA), Some(CLARO), G, true);
    l.circulo(64.0, 42.0, 26.0, Some(TINTA), Some(BLANCO), G);
    l.circulo(64.0, 42.0, 13.0, Some(ROJO), Some(ROSA), G);
    l.rect(10.0, 10.0, 26.0, 16.0, Some(TINTA), Some(BLANCO), 1.5, true);
    l.rect(80.0, -8.0, 20.0, 8.0, Some(TINTA), Some(ROJO), 1.5, true);
}

fn movil(l: &mut Lapiz) {
    l.rect(0.0, 0.0, 74.0, 148.0, Some(TINTA), Some(CLARO), G, true);
    l.rect(8.0, 18.0, 58.0, 108.0, Some(TINTA), Some(BLANCO), 1.5, true);
    l.circulo(37.0, 9.0, 3.5, None, Some(TINTA), 1.0);
    l.linea(28.0, 137.0, 46.0, 137.0, TINTA, 2.5);
    l.circulo(20.0, 30.0, 5.0, Some(ROJO), Some(ROSA), 1.5);
}

fn camara_360(l: &mut Lapiz) {
    l.circulo(55.0, 50.0, 46.0, Some(TINTA), Some(CLARO), G);
    l.circulo(30.0, 50.0, 16.0, Some(TINTA), Some(BLANCO), G);
    l.circulo(30.0, 50.0, 7.0, Some(ROJO), Some(ROSA), 1.5);
    l.circulo(80.0, 50.0, 16.0, Some(TINTA), Some(BLANCO), G);
    l.circulo(80.0, 50.0, 7.0, Some(ROJO), Some(ROSA), 1.5);
    forma(
        l,
        &[(42.0, 94.0), (68.0, 94.0), (74.0, 112.0), (36.0, 112.0)],
        TINTA,
        CLARO,
    );
    l.linea(55.0, 112.0, 55.0, 150.0, TINTA, G);
}

/// La camara vista desde arriba con su campo de vision a trazos: lo que se
/// usa en una planta de rodaje.
fn camara_arriba(l: &mut Lapiz) {
    l.rect(0.0, 30.0, 70.0, 44.0, Some(TINTA), Some(CLARO), G, true);
    forma(
        l,
        &[(70.0, 40.0), (96.0, 32.0), (96.0, 72.0), (70.0, 64.0)],
        TINTA,
        BLANCO,
    );
    l.linea_con(96.0, 32.0, 190.0, 0.0, ROJO, 1.5, EstiloTrazo::Discontinuo);
    l.linea_con(
        96.0,
        72.0,
        190.0,
        104.0,
        ROJO,
        1.5,
        EstiloTrazo::Discontinuo,
    );
    l.arco(96.0, 52.0, 94.0, -18.0, 18.0, ROJO, 1.5);
    piloto(l, 12.0, 42.0);
}

// ------------------------------------------------------------------ soportes

fn dron(l: &mut Lapiz) {
    l.rect(60.0, 30.0, 60.0, 26.0, Some(TINTA), Some(CLARO), G, true);
    l.linea(10.0, 26.0, 60.0, 38.0, TINTA, G);
    l.linea(170.0, 26.0, 120.0, 38.0, TINTA, G);
    for x in [10.0, 170.0] {
        l.linea(x, 18.0, x, 26.0, TINTA, G);
        l.elipse(x - 34.0, 12.0, 68.0, 8.0, Some(TINTA), Some(CLARO), 1.5);
    }
    l.linea(76.0, 56.0, 68.0, 70.0, TINTA, G);
    l.linea(104.0, 56.0, 112.0, 70.0, TINTA, G);
    l.circulo(90.0, 66.0, 9.0, Some(TINTA), Some(BLANCO), G);
    l.circulo(90.0, 66.0, 3.5, None, Some(ROJO), 1.0);
}

fn dron_arriba(l: &mut Lapiz) {
    let c = 80.0;
    for (dx, dy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let (x, y) = (c + dx * 52.0, c + dy * 52.0);
        l.linea(c, c, x, y, TINTA, 4.0);
        l.circulo(x, y, 26.0, Some(TINTA), Some(CLARO), 1.5);
        l.circulo(x, y, 4.0, None, Some(TINTA), 1.0);
    }
    l.rect(
        c - 22.0,
        c - 26.0,
        44.0,
        52.0,
        Some(TINTA),
        Some(BLANCO),
        G,
        true,
    );
    forma(
        l,
        &[(c - 7.0, c - 26.0), (c, c - 36.0), (c + 7.0, c - 26.0)],
        ROJO,
        ROJO,
    );
}

fn grua(l: &mut Lapiz) {
    l.rect(30.0, 150.0, 80.0, 20.0, Some(TINTA), Some(CLARO), G, true);
    for x in [42.0, 98.0] {
        l.circulo(x, 178.0, 10.0, Some(TINTA), Some(BLANCO), G);
    }
    l.linea(70.0, 150.0, 70.0, 110.0, TINTA, 4.0);
    l.linea(30.0, 132.0, 200.0, 20.0, TINTA, 4.0);
    l.rect(20.0, 126.0, 22.0, 20.0, Some(TINTA), Some(TINTA), G, false);
    l.linea(200.0, 20.0, 200.0, 34.0, TINTA, G);
    l.rect(186.0, 34.0, 36.0, 24.0, Some(TINTA), Some(CLARO), G, true);
    objetivo(l, 222.0, 46.0, 14.0, 20.0);
}

fn rieles(l: &mut Lapiz) {
    l.linea(0.0, 10.0, 220.0, 10.0, TINTA, 3.0);
    l.linea(0.0, 50.0, 220.0, 50.0, TINTA, 3.0);
    for i in 0..9 {
        let x = 8.0 + i as f32 * 25.5;
        l.rect(x, 2.0, 8.0, 56.0, Some(TINTA), Some(CLARO), 1.5, false);
    }
}

// --------------------------------------------------------------------- luces

/// El pie de una luz: tubo y tres patas.
fn pie(l: &mut Lapiz, x: f32, y: f32, alto: f32) {
    l.linea(x, y, x, y + alto, TINTA, G);
    l.linea(x, y + alto, x - 30.0, y + alto + 24.0, TINTA, G);
    l.linea(x, y + alto, x + 30.0, y + alto + 24.0, TINTA, G);
    l.linea(x, y + alto, x, y + alto + 24.0, TINTA, G);
}

fn foco(l: &mut Lapiz) {
    l.rect(30.0, 20.0, 60.0, 56.0, Some(TINTA), Some(CLARO), G, true);
    l.linea(90.0, 20.0, 118.0, 0.0, TINTA, G);
    l.linea(90.0, 76.0, 118.0, 96.0, TINTA, G);
    l.linea(90.0, 30.0, 90.0, 66.0, ROJO, 4.0);
    l.linea_con(
        124.0,
        48.0,
        160.0,
        48.0,
        ROJO,
        1.5,
        EstiloTrazo::Discontinuo,
    );
    l.linea_con(122.0, 20.0, 156.0, 4.0, ROJO, 1.5, EstiloTrazo::Discontinuo);
    l.linea_con(
        122.0,
        76.0,
        156.0,
        92.0,
        ROJO,
        1.5,
        EstiloTrazo::Discontinuo,
    );
    pie(l, 60.0, 76.0, 80.0);
}

fn foco_arriba(l: &mut Lapiz) {
    l.rect(0.0, 22.0, 50.0, 40.0, Some(TINTA), Some(CLARO), G, true);
    l.linea(50.0, 22.0, 70.0, 6.0, TINTA, G);
    l.linea(50.0, 62.0, 70.0, 78.0, TINTA, G);
    forma(
        l,
        &[(54.0, 30.0), (180.0, 0.0), (180.0, 84.0), (54.0, 54.0)],
        hex(0xf0b74b),
        hex(0xfdf3dc),
    );
}

fn softbox(l: &mut Lapiz) {
    forma(
        l,
        &[(0.0, 0.0), (110.0, 0.0), (80.0, 60.0), (30.0, 60.0)],
        TINTA,
        CLARO,
    );
    l.rect(8.0, 6.0, 94.0, 10.0, None, Some(hex(0xfdf3dc)), 1.0, false);
    l.rect(40.0, 60.0, 30.0, 14.0, Some(TINTA), Some(TINTA), G, false);
    pie(l, 55.0, 74.0, 76.0);
}

fn panel_led(l: &mut Lapiz) {
    l.rect(0.0, 0.0, 110.0, 76.0, Some(TINTA), Some(CLARO), G, true);
    for f in 0..4 {
        for c in 0..6 {
            l.circulo(
                14.0 + c as f32 * 16.5,
                14.0 + f as f32 * 16.0,
                3.0,
                None,
                Some(hex(0xf0b74b)),
                1.0,
            );
        }
    }
    l.linea(55.0, 76.0, 55.0, 88.0, TINTA, G);
    pie(l, 55.0, 88.0, 70.0);
}

fn reflector(l: &mut Lapiz) {
    let pts: Vec<(f32, f32)> = (0..=18)
        .map(|i| {
            let a = (200.0 + 140.0 * i as f32 / 18.0f32).to_radians();
            (70.0 + 70.0 * a.cos(), 70.0 + 60.0 * a.sin())
        })
        .collect();
    let mut cerrada = pts.clone();
    cerrada.push(pts[0]);
    l.poli(&cerrada, TINTA, G, Some(CLARO));
    for k in 1..4 {
        let a = (200.0 + 35.0 * k as f32).to_radians();
        l.linea(
            70.0,
            70.0,
            70.0 + 70.0 * a.cos(),
            70.0 + 60.0 * a.sin(),
            TINTA,
            1.25,
        );
    }
    l.linea(70.0, 10.0, 70.0, 80.0, TINTA, G);
    l.rect(
        60.0,
        74.0,
        20.0,
        14.0,
        Some(TINTA),
        Some(hex(0xfdf3dc)),
        G,
        true,
    );
    pie(l, 70.0, 88.0, 70.0);
}

// --------------------------------------------------------------------- sonido

fn microfono(l: &mut Lapiz) {
    l.circulo(30.0, 26.0, 24.0, Some(TINTA), Some(CLARO), G);
    for k in -1..=1 {
        let y = 26.0 + k as f32 * 9.0;
        l.linea(12.0, y, 48.0, y, TINTA, 1.25);
    }
    l.rect(22.0, 50.0, 16.0, 10.0, Some(TINTA), Some(TINTA), G, false);
    forma(
        l,
        &[(20.0, 60.0), (40.0, 60.0), (36.0, 130.0), (24.0, 130.0)],
        TINTA,
        CLARO,
    );
    piloto(l, 30.0, 74.0);
}

fn boom(l: &mut Lapiz) {
    l.linea(0.0, 120.0, 150.0, 40.0, TINTA, 3.5);
    let cuerpo = [
        (140.0, 34.0),
        (210.0, 34.0),
        (220.0, 44.0),
        (210.0, 54.0),
        (140.0, 54.0),
    ];
    forma(l, &girar(&cuerpo, (150.0, 40.0), 0.0), TINTA, CLARO);
    for k in 0..5 {
        let x = 150.0 + k as f32 * 12.0;
        l.linea(x, 36.0, x, 52.0, TINTA, 1.0);
    }
    l.linea(150.0, 54.0, 150.0, 64.0, TINTA, G);
    l.rect(138.0, 64.0, 24.0, 8.0, Some(TINTA), Some(ROJO), 1.5, true);
}

fn solapa(l: &mut Lapiz) {
    l.rect(0.0, 0.0, 26.0, 40.0, Some(TINTA), Some(CLARO), G, true);
    l.rect(6.0, 40.0, 14.0, 12.0, Some(TINTA), Some(TINTA), G, false);
    l.rect(30.0, 4.0, 20.0, 30.0, Some(TINTA), Some(BLANCO), G, true);
    let cable: Vec<(f32, f32)> = (0..=20)
        .map(|i| {
            let t = i as f32 / 20.0;
            (13.0 + 40.0 * (t * 3.0).sin() * t, 52.0 + t * 110.0)
        })
        .collect();
    l.poli(&cable, TINTA, G, None);
    l.rect(30.0, 160.0, 40.0, 26.0, Some(TINTA), Some(CLARO), G, true);
    piloto(l, 60.0, 170.0);
}

fn claqueta(l: &mut Lapiz) {
    let palo = [(0.0, 30.0), (160.0, 6.0), (164.0, 30.0), (4.0, 54.0)];
    forma(l, &palo, TINTA, TINTA);
    for k in 0..5 {
        let x = 12.0 + k as f32 * 32.0;
        forma(
            l,
            &[
                (x, 28.0 - k as f32 * 4.8),
                (x + 16.0, 25.6 - k as f32 * 4.8),
                (x + 20.0, 49.0 - k as f32 * 4.8),
                (x + 4.0, 51.4 - k as f32 * 4.8),
            ],
            BLANCO,
            BLANCO,
        );
    }
    l.rect(4.0, 60.0, 160.0, 100.0, Some(TINTA), Some(TINTA), G, true);
    l.rect(4.0, 60.0, 160.0, 22.0, Some(TINTA), Some(BLANCO), G, false);
    for k in 0..5 {
        let x = 4.0 + k as f32 * 32.0;
        forma(
            l,
            &[
                (x + 6.0, 60.0),
                (x + 22.0, 60.0),
                (x + 16.0, 82.0),
                (x, 82.0),
            ],
            TINTA,
            TINTA,
        );
    }
    for (i, s) in ["ESCENA", "TOMA", "ROLLO"].iter().enumerate() {
        let x = 12.0 + i as f32 * 52.0;
        l.texto(x, 92.0, s, 9.0, WORK_SANS, BLANCO, true, Al::Izq);
        l.linea(x, 122.0, x + 42.0, 122.0, BLANCO, 1.0);
    }
    l.linea(4.0, 130.0, 164.0, 130.0, BLANCO, 1.0);
    l.texto(
        12.0,
        136.0,
        "DIRECCIÓN",
        9.0,
        WORK_SANS,
        BLANCO,
        true,
        Al::Izq,
    );
}

// --------------------------------------------------------------------- marcos

/// Un encuadre con sus tercios a puntos, la zona segura a trazos y la
/// proporcion escrita debajo.
fn marco(l: &mut Lapiz, proporcion: f32, rotulo: &str) {
    let w = 320.0;
    let h = w / proporcion;
    l.rect(0.0, 0.0, w, h, Some(TINTA), Some(BLANCO), 2.5, false);
    for k in 1..3 {
        let (x, y) = (w * k as f32 / 3.0, h * k as f32 / 3.0);
        l.linea_con(x, 0.0, x, h, hex(0xb4c0d3), 1.0, EstiloTrazo::Punteado);
        l.linea_con(0.0, y, w, y, hex(0xb4c0d3), 1.0, EstiloTrazo::Punteado);
    }
    l.rect(
        w * 0.05,
        h * 0.05,
        w * 0.9,
        h * 0.9,
        Some(ROJO),
        None,
        1.0,
        false,
    );
    if let Some(e) = l.v.last_mut() {
        e.estilo = EstiloTrazo::Discontinuo;
        e.opacidad = 0.6;
    }
    let (cx, cy) = (w / 2.0, h / 2.0);
    l.linea(cx - 8.0, cy, cx + 8.0, cy, ROJO, 1.25);
    l.linea(cx, cy - 8.0, cx, cy + 8.0, ROJO, 1.25);
    l.texto(w, h + 6.0, rotulo, 11.0, WORK_SANS, TINTA, true, Al::Der);
}
