//! **Quimica de secundaria**: el material de laboratorio (vidrio claro con
//! su liquido y sus marcas), modelos de atomos y moleculas, la casilla de
//! un elemento y la tabla periodica entera, enlaces, la grafica de
//! calentamiento y la escala de pH.

use super::*;
use crate::elemento::EstiloTrazo;

const CATEGORIA: &str = "Química";
const PREFIJO: &str = "ciencia-";
const VIDRIO: ColorRgba = hex(0x5a7a94);
const VIDRIO_SUAVE: ColorRgba = hex(0xeef6fb);
const LIQUIDO: ColorRgba = hex(0x9fd0ee);
const LIQUIDO_VERDE: ColorRgba = hex(0xa8e0b0);
const LIQUIDO_ROSA: ColorRgba = hex(0xf5b8cf);
const LIQUIDO_AMBAR: ColorRgba = hex(0xf6d38a);
const METAL: ColorRgba = hex(0x9aa6b6);

pub(super) fn todas(v: &mut Vec<FiguraGuardada>) {
    let lista: [Dibujo; 19] = [
        ("erlenmeyer", "Matraz Erlenmeyer", |l| {
            erlenmeyer(l, 0.0, 0.0, 1.0, LIQUIDO)
        }),
        (
            "vaso-precipitados",
            "Vaso de precipitados",
            vaso_precipitados,
        ),
        ("probeta", "Probeta", probeta),
        ("tubo-ensayo", "Tubo de ensayo", |l| {
            tubo(l, 20.0, 10.0, LIQUIDO)
        }),
        ("gradilla", "Gradilla con tubos de ensayo", gradilla),
        ("mechero-bunsen", "Mechero Bunsen", mechero),
        ("bureta", "Bureta en soporte (valoración)", bureta),
        ("embudo", "Embudo", embudo),
        ("balon", "Matraz de fondo redondo", balon),
        ("modelo-bohr", "Modelo atómico de Bohr", bohr),
        ("molecula-agua", "Molécula de agua (H₂O)", agua),
        ("molecula-co2", "Molécula de dióxido de carbono (CO₂)", co2),
        (
            "casilla-elemento",
            "Casilla de un elemento",
            casilla_rotulada,
        ),
        (
            "casilla-en-blanco",
            "Casilla de elemento en blanco",
            casilla_en_blanco,
        ),
        ("tabla-periodica", "Tabla periódica", tabla_periodica),
        ("enlace-ionico", "Enlace iónico (NaCl)", enlace_ionico),
        (
            "enlace-covalente",
            "Enlace covalente (H₂)",
            enlace_covalente,
        ),
        (
            "curva-calentamiento",
            "Gráfica de calentamiento del agua",
            calentamiento,
        ),
        ("escala-ph", "Escala de pH", escala_ph),
    ];
    meter(v, PREFIJO, CATEGORIA, &lista);
}

// ---------------------------------------------------------------- vidrio

/// Las rayitas de medida de un recipiente: `n` marcas de `y_abajo` a
/// `y_arriba` y el numero (`i * unidad`) en las de cada `cada`.
fn marcas(
    l: &mut Lapiz,
    x: f32,
    (y_abajo, y_arriba): (f32, f32),
    n: usize,
    cada: usize,
    unidad: u32,
) {
    for i in 1..=n {
        let y = y_abajo - (y_abajo - y_arriba) * i as f32 / n as f32;
        let larga = i % cada == 0;
        l.linea(x, y, x + if larga { 14.0 } else { 8.0 }, y, VIDRIO, 1.0);
        if larga {
            txt_a(
                l,
                x + 17.0,
                y,
                &(i as u32 * unidad).to_string(),
                8.0,
                Al::Izq,
            );
        }
    }
}

/// Un Erlenmeyer en `(x, y)` de `s` veces su tamano.
fn erlenmeyer(l: &mut Lapiz, x: f32, y: f32, s: f32, liquido: ColorRgba) {
    let p = |a: f32, b: f32| (x + a * s, y + b * s);
    let cuerpo = [
        p(52.0, 14.0),
        p(52.0, 70.0),
        p(6.0, 196.0),
        p(10.0, 202.0),
        p(140.0, 202.0),
        p(144.0, 196.0),
        p(98.0, 70.0),
        p(98.0, 14.0),
    ];
    forma(l, &cuerpo, VIDRIO, Some(VIDRIO_SUAVE), G);
    let lado = |yy: f32| 52.0 - (yy - 70.0) * 46.0 / 126.0;
    let nivel = 140.0;
    let liq = [
        p(lado(nivel), nivel),
        p(6.0, 196.0),
        p(10.0, 202.0),
        p(140.0, 202.0),
        p(144.0, 196.0),
        p(150.0 - lado(nivel), nivel),
    ];
    forma(l, &liq, liquido, Some(liquido), 1.0);
    forma(l, &cuerpo, VIDRIO, None, G);
    l.rect(
        x + 46.0 * s,
        y + 6.0 * s,
        58.0 * s,
        9.0 * s,
        Some(VIDRIO),
        Some(VIDRIO_SUAVE),
        1.5,
        true,
    );
    if s >= 1.0 {
        for (i, ml) in ["50", "100", "150"].iter().enumerate() {
            let yy = 180.0 - 24.0 * i as f32;
            l.linea(x + 62.0, y + yy, x + 78.0, y + yy, VIDRIO, 1.0);
            txt_a(l, x + 82.0, y + yy, ml, 8.0, Al::Izq);
        }
        txt(l, x + 75.0, y + 92.0, "250 mL", 9.0, VIDRIO);
    }
}

fn vaso_precipitados(l: &mut Lapiz) {
    let cuerpo = [
        (12.0, 10.0),
        (22.0, 20.0),
        (22.0, 196.0),
        (30.0, 204.0),
        (150.0, 204.0),
        (158.0, 196.0),
        (158.0, 14.0),
    ];
    forma(l, &cuerpo, VIDRIO_SUAVE, Some(VIDRIO_SUAVE), 1.0);
    forma(
        l,
        &[
            (22.0, 110.0),
            (22.0, 196.0),
            (30.0, 204.0),
            (150.0, 204.0),
            (158.0, 196.0),
            (158.0, 110.0),
        ],
        LIQUIDO,
        Some(LIQUIDO),
        1.0,
    );
    l.poli(&cuerpo, VIDRIO, G, None);
    marcas(l, 30.0, (196.0, 40.0), 8, 2, 25);
    txt(l, 112.0, 70.0, "250 mL", 10.0, VIDRIO);
}

fn probeta(l: &mut Lapiz) {
    let tubo = [
        (34.0, 12.0),
        (40.0, 18.0),
        (40.0, 290.0),
        (80.0, 290.0),
        (80.0, 12.0),
    ];
    l.rect(
        30.0,
        290.0,
        60.0,
        8.0,
        Some(VIDRIO),
        Some(VIDRIO_SUAVE),
        1.5,
        true,
    );
    l.rect(
        12.0,
        298.0,
        96.0,
        10.0,
        Some(VIDRIO),
        Some(VIDRIO_SUAVE),
        1.5,
        true,
    );
    forma(l, &tubo, VIDRIO_SUAVE, Some(VIDRIO_SUAVE), 1.0);
    l.rect(41.0, 122.0, 38.0, 167.0, None, Some(LIQUIDO), 1.0, false);
    l.arco_eliptico(60.0, 122.0, 19.0, 5.0, 0.0, 180.0, hex(0x3d8cc4), 1.5);
    l.poli(&tubo, VIDRIO, G, None);
    marcas(l, 41.0, (280.0, 40.0), 24, 4, 25);
    rotulo(l, (60.0, 127.0), (100.0, 150.0), "menisco", Al::Izq);
}

/// Un tubo de ensayo con su liquido.
fn tubo(l: &mut Lapiz, x: f32, y: f32, liquido: ColorRgba) {
    let (w, h) = (26.0, 150.0);
    let r = w / 2.0;
    let fondo: Vec<(f32, f32)> = (0..=12)
        .map(|i| {
            let a = (180.0 - 15.0 * i as f32).to_radians();
            (x + r + r * a.cos(), y + h + r * a.sin())
        })
        .collect();
    let mut v = vec![(x, y), (x, y + h)];
    v.extend(&fondo);
    v.push((x + w, y));
    forma(l, &v, VIDRIO_SUAVE, Some(VIDRIO_SUAVE), 1.0);
    let mut liq = vec![(x + 1.0, y + 80.0), (x + 1.0, y + h)];
    liq.extend(&fondo);
    liq.push((x + w - 1.0, y + 80.0));
    forma(l, &liq, liquido, Some(liquido), 1.0);
    l.poli(&v, VIDRIO, G, None);
    l.arco_eliptico(x + r, y, r + 2.0, 3.0, 0.0, 360.0, VIDRIO, 1.5);
}

fn gradilla(l: &mut Lapiz) {
    let (borde, madera, oscura) = (hex(0x8a6a4a), hex(0xd9b48a), hex(0xc89d70));
    for (i, c) in [LIQUIDO, LIQUIDO_VERDE, LIQUIDO_ROSA, LIQUIDO_AMBAR]
        .iter()
        .enumerate()
    {
        tubo(l, 34.0 + 52.0 * i as f32, 10.0, *c);
    }
    l.rect(
        10.0,
        70.0,
        230.0,
        14.0,
        Some(borde),
        Some(madera),
        1.5,
        true,
    );
    l.rect(
        10.0,
        168.0,
        230.0,
        12.0,
        Some(borde),
        Some(madera),
        1.5,
        true,
    );
    l.rect(
        14.0,
        84.0,
        10.0,
        96.0,
        Some(borde),
        Some(oscura),
        1.5,
        false,
    );
    l.rect(
        226.0,
        84.0,
        10.0,
        96.0,
        Some(borde),
        Some(oscura),
        1.5,
        false,
    );
    l.rect(
        4.0,
        180.0,
        242.0,
        10.0,
        Some(borde),
        Some(oscura),
        1.5,
        true,
    );
}

fn mechero(l: &mut Lapiz) {
    // La llama: el cono de fuera y el de dentro.
    let mut fuera = bezier((58.0, 92.0), (40.0, 40.0), (70.0, 0.0), 10);
    fuera.extend(bezier((70.0, 0.0), (100.0, 40.0), (82.0, 92.0), 10));
    forma(l, &fuera, hex(0x5b8fd6), Some(hex(0xcfe0f7)), 1.5);
    let mut dentro = bezier((62.0, 92.0), (56.0, 60.0), (70.0, 40.0), 8);
    dentro.extend(bezier((70.0, 40.0), (84.0, 60.0), (78.0, 92.0), 8));
    forma(l, &dentro, AZUL, Some(hex(0x8fb6ea)), 1.2);
    l.rect(
        58.0,
        92.0,
        24.0,
        118.0,
        Some(TINTA),
        Some(hex(0xc9d1dc)),
        G,
        false,
    );
    l.rect(54.0, 168.0, 32.0, 16.0, Some(TINTA), Some(METAL), G, true);
    l.rect(64.0, 172.0, 12.0, 8.0, None, Some(TINTA), 1.0, false);
    l.rect(82.0, 196.0, 56.0, 9.0, Some(TINTA), Some(METAL), 1.5, true);
    forma(
        l,
        &[(20.0, 226.0), (120.0, 226.0), (106.0, 210.0), (34.0, 210.0)],
        TINTA,
        Some(GRIS),
        G,
    );
    rotulos_derecha(
        l,
        160.0,
        30.0,
        44.0,
        &[
            ((88.0, 30.0), "llama"),
            ((76.0, 130.0), "tubo"),
            ((86.0, 176.0), "regulador de aire"),
            ((136.0, 200.0), "entrada de gas"),
            ((112.0, 218.0), "base"),
        ],
    );
}

fn bureta(l: &mut Lapiz) {
    l.rect(
        10.0,
        330.0,
        150.0,
        14.0,
        Some(TINTA),
        Some(METAL),
        1.5,
        true,
    );
    l.linea(34.0, 6.0, 34.0, 330.0, GRIS, 5.0);
    l.linea(34.0, 70.0, 90.0, 70.0, TINTA, 3.0);
    l.rect(84.0, 64.0, 18.0, 12.0, Some(TINTA), Some(METAL), 1.5, true);
    l.rect(
        86.0,
        20.0,
        14.0,
        230.0,
        Some(VIDRIO),
        Some(VIDRIO_SUAVE),
        1.5,
        false,
    );
    l.rect(
        87.0,
        54.0,
        12.0,
        195.0,
        None,
        Some(LIQUIDO_ROSA),
        1.0,
        false,
    );
    l.rect(86.0, 20.0, 14.0, 230.0, Some(VIDRIO), None, 1.5, false);
    for i in 0..10 {
        let y = 40.0 + 20.0 * i as f32;
        l.linea(86.0, y, 92.0, y, VIDRIO, 1.0);
        txt_a(l, 82.0, y, &(i * 5).to_string(), 7.0, Al::Der);
    }
    l.rect(80.0, 250.0, 26.0, 10.0, Some(TINTA), Some(METAL), 1.5, true);
    l.linea(106.0, 255.0, 120.0, 247.0, TINTA, 3.0);
    forma(
        l,
        &[(89.0, 260.0), (92.0, 282.0), (94.0, 282.0), (97.0, 260.0)],
        VIDRIO,
        Some(VIDRIO_SUAVE),
        1.2,
    );
    l.circulo(93.0, 290.0, 2.5, None, Some(LIQUIDO_ROSA), 1.0);
    erlenmeyer(l, 75.0, 282.0, 0.24, LIQUIDO_ROSA);
    rotulos_derecha(
        l,
        150.0,
        40.0,
        56.0,
        &[
            ((102.0, 70.0), "pinza"),
            ((100.0, 140.0), "bureta"),
            ((120.0, 247.0), "llave"),
            ((106.0, 316.0), "matraz"),
            ((36.0, 300.0), "soporte universal"),
        ],
    );
}

fn embudo(l: &mut Lapiz) {
    let cono = [
        (10.0, 14.0),
        (120.0, 14.0),
        (72.0, 86.0),
        (72.0, 160.0),
        (66.0, 166.0),
        (58.0, 160.0),
        (58.0, 86.0),
    ];
    forma(l, &cono, VIDRIO, Some(VIDRIO_SUAVE), G);
    forma(
        l,
        &[(24.0, 30.0), (106.0, 30.0), (65.0, 80.0)],
        hex(0xb9c4d0),
        Some(BLANCO),
        1.2,
    );
    l.v.last_mut().unwrap().estilo = EstiloTrazo::Discontinuo;
    l.arco_eliptico(65.0, 14.0, 55.0, 6.0, 0.0, 360.0, VIDRIO, 1.5);
    for (i, y) in [180.0, 196.0].iter().enumerate() {
        l.circulo(65.0, *y, 3.0 - i as f32 * 0.5, None, Some(LIQUIDO), 1.0);
    }
    rotulo(l, (65.0, 56.0), (140.0, 60.0), "papel de filtro", Al::Izq);
}

fn balon(l: &mut Lapiz) {
    let (cx, cy, r) = (80.0, 130.0, 60.0);
    let cuello: f32 = 13.0;
    let a0 = (cuello / r).asin().to_degrees();
    let mut v = vec![(cx - cuello, 12.0)];
    v.extend((0..=40).map(|i| {
        let a = (-90.0 - a0 - (360.0 - 2.0 * a0) * i as f32 / 40.0).to_radians();
        (cx + r * a.cos(), cy + r * a.sin())
    }));
    v.push((cx + cuello, 12.0));
    forma(l, &v, VIDRIO_SUAVE, Some(VIDRIO_SUAVE), 1.0);
    let s = (10.0 / r).asin().to_degrees();
    let liq: Vec<(f32, f32)> = (0..=24)
        .map(|i| {
            let a = (s + (180.0 - 2.0 * s) * i as f32 / 24.0).to_radians();
            (cx + (r - 1.0) * a.cos(), cy + (r - 1.0) * a.sin())
        })
        .collect();
    forma(l, &liq, LIQUIDO_VERDE, Some(LIQUIDO_VERDE), 1.0);
    l.poli(&v, VIDRIO, G, None);
    l.rect(
        cx - cuello - 5.0,
        6.0,
        2.0 * cuello + 10.0,
        9.0,
        Some(VIDRIO),
        Some(VIDRIO_SUAVE),
        1.5,
        true,
    );
}

// ------------------------------------------------------ atomos y moleculas

fn bohr(l: &mut Lapiz) {
    let (cx, cy) = (140.0, 140.0);
    let capas = [(48.0, 2), (82.0, 8), (116.0, 1)];
    for (r, _) in capas {
        l.circulo(cx, cy, r, Some(GRIS), None, 1.2);
    }
    l.circulo(cx, cy, 26.0, Some(ROJO), Some(ROSA), G);
    txt_n(l, cx, cy - 6.0, "11 p⁺", 11.0, ROJO);
    txt_n(l, cx, cy + 9.0, "12 n", 11.0, TINTA);
    for (r, n) in capas {
        for i in 0..n {
            let a = (-90.0 + 360.0 * i as f32 / n as f32 + r).to_radians();
            let (x, y) = (cx + r * a.cos(), cy + r * a.sin());
            l.circulo(x, y, 6.5, Some(AZUL), Some(AZUL_SUAVE), 1.5);
            txt_n(l, x, y, "−", 9.0, AZUL);
        }
    }
    txt_n(
        l,
        cx,
        cy + 140.0,
        "Sodio (Na) · Z = 11 · 2 – 8 – 1",
        13.0,
        TINTA,
    );
    rotulo(
        l,
        (cx + 18.0, cy - 18.0),
        (cx + 130.0, cy - 132.0),
        "núcleo",
        Al::Izq,
    );
    rotulo(
        l,
        (
            cx + 82.0 * std::f32::consts::FRAC_1_SQRT_2,
            cy + 82.0 * std::f32::consts::FRAC_1_SQRT_2,
        ),
        (cx + 140.0, cy + 110.0),
        "órbita (capa)",
        Al::Izq,
    );
}

/// Un atomo como bola con su brillo y su simbolo.
fn bola(
    l: &mut Lapiz,
    (cx, cy): (f32, f32),
    r: f32,
    relleno: ColorRgba,
    s: &str,
    letra: ColorRgba,
) {
    l.circulo(cx, cy, r, Some(TINTA), Some(relleno), G);
    l.circulo(
        cx - r * 0.42,
        cy - r * 0.42,
        r * 0.18,
        None,
        Some(BLANCO),
        1.0,
    );
    txt_n(l, cx, cy + 1.0, s, r * 0.7, letra);
}

fn agua(l: &mut Lapiz) {
    let o = (130.0, 64.0);
    let d = 70.0;
    let a = 37.75f32.to_radians();
    let h1 = (o.0 + d * a.cos(), o.1 + d * a.sin());
    let h2 = (2.0 * o.0 - h1.0, h1.1);
    l.linea(o.0, o.1, h1.0, h1.1, GRIS, 7.0);
    l.linea(o.0, o.1, h2.0, h2.1, GRIS, 7.0);
    angulo(l, o, h1, h2, 52.0, "", TINTA);
    bola(l, o, 38.0, hex(0xe0524a), "O", BLANCO);
    bola(l, h1, 24.0, hex(0xf2f4f7), "H", TINTA);
    bola(l, h2, 24.0, hex(0xf2f4f7), "H", TINTA);
    txt(l, o.0, o.1 + 70.0, "104,5°", 11.0, TINTA);
    txt(l, o.0 + 50.0, 16.0, "δ−", 12.0, ROJO);
    txt(l, h1.0 + 30.0, h1.1 + 18.0, "δ+", 12.0, AZUL);
    txt(l, h2.0 - 30.0, h2.1 + 18.0, "δ+", 12.0, AZUL);
}

fn co2(l: &mut Lapiz) {
    for y in [42.0, 58.0] {
        l.linea(60.0, y, 260.0, y, GRIS, 5.0);
    }
    bola(l, (160.0, 50.0), 28.0, hex(0x4a4f5a), "C", BLANCO);
    bola(l, (60.0, 50.0), 32.0, hex(0xe0524a), "O", BLANCO);
    bola(l, (260.0, 50.0), 32.0, hex(0xe0524a), "O", BLANCO);
    txt_n(l, 160.0, 112.0, "O = C = O", 14.0, TINTA);
    txt(l, 160.0, 132.0, "lineal · 180°", 11.0, GRIS);
}

// ---------------------------------------------------------------- elementos

/// La casilla de un elemento: numero, masa, simbolo, nombre, configuracion
/// y electronegatividad.
fn casilla(l: &mut Lapiz, x: f32, y: f32, datos: [&str; 6]) {
    let [z, masa, simbolo, nombre, config, en] = datos;
    l.rect(
        x,
        y,
        150.0,
        170.0,
        Some(TINTA),
        Some(hex(0xe9f3e3)),
        G,
        true,
    );
    l.texto(x + 12.0, y + 10.0, z, 16.0, WORK_SANS, TINTA, true, Al::Izq);
    txt_a(l, x + 138.0, y + 20.0, masa, 11.0, Al::Der);
    txt_n(l, x + 75.0, y + 76.0, simbolo, 28.0, TINTA);
    txt(l, x + 75.0, y + 116.0, nombre, 13.0, TINTA);
    txt(l, x + 75.0, y + 138.0, config, 10.0, GRIS);
    txt_a(l, x + 12.0, y + 156.0, en, 9.0, Al::Izq);
}

fn casilla_rotulada(l: &mut Lapiz) {
    casilla(
        l,
        10.0,
        10.0,
        ["6", "12,011", "C", "Carbono", "[He] 2s² 2p²", "2,55"],
    );
    rotulos_izquierda(
        l,
        -20.0,
        30.0,
        136.0,
        &[
            ((22.0, 30.0), "número atómico (Z)"),
            ((26.0, 166.0), "electronegatividad"),
        ],
    );
    rotulos_derecha(
        l,
        200.0,
        30.0,
        38.0,
        &[
            ((148.0, 30.0), "masa atómica (u)"),
            ((104.0, 86.0), "símbolo"),
            ((128.0, 126.0), "nombre"),
            ((148.0, 148.0), "configuración electrónica"),
        ],
    );
}

fn casilla_en_blanco(l: &mut Lapiz) {
    casilla(
        l,
        0.0,
        0.0,
        ["Z", "masa", "X", "Nombre", "configuración", "EN"],
    );
}

/// Los 118 elementos por su numero atomico.
const SIMBOLOS: &str = "H He Li Be B C N O F Ne Na Mg Al Si P S Cl Ar K Ca Sc Ti V Cr Mn Fe Co Ni Cu Zn Ga Ge As Se Br Kr \
     Rb Sr Y Zr Nb Mo Tc Ru Rh Pd Ag Cd In Sn Sb Te I Xe Cs Ba La Ce Pr Nd Pm Sm Eu Gd Tb Dy Ho Er Tm Yb Lu Hf Ta W Re Os Ir Pt \
     Au Hg Tl Pb Bi Po At Rn Fr Ra Ac Th Pa U Np Pu Am Cm Bk Cf Es Fm Md No Lr Rf Db Sg Bh Hs Mt Ds Rg Cn Nh Fl Mc Lv Ts Og";

/// Fila y columna (desde 1) de un elemento; los lantanidos y actinidos en
/// las filas 9 y 10, aparte.
fn sitio(z: u32) -> (u32, u32) {
    match z {
        1 => (1, 1),
        2 => (1, 18),
        3..=4 => (2, z - 2),
        5..=10 => (2, z + 8),
        11..=12 => (3, z - 10),
        13..=18 => (3, z),
        19..=36 => (4, z - 18),
        37..=54 => (5, z - 36),
        55..=56 => (6, z - 54),
        57..=71 => (9, z - 54),
        72..=86 => (6, z - 68),
        87..=88 => (7, z - 86),
        89..=103 => (10, z - 86),
        _ => (7, z - 100),
    }
}

/// La familia de un elemento (su sitio en [`FAMILIAS`]), para el color.
fn familia(z: u32) -> usize {
    match z {
        3 | 11 | 19 | 37 | 55 | 87 => 0,
        4 | 12 | 20 | 38 | 56 | 88 => 1,
        21..=30 | 39..=48 | 72..=80 | 104..=112 => 2,
        13 | 31 | 49 | 50 | 81..=84 | 113..=116 => 3,
        5 | 14 | 32 | 33 | 51 | 52 => 4,
        1 | 6 | 7 | 8 | 15 | 16 | 34 => 5,
        9 | 17 | 35 | 53 | 85 | 117 => 6,
        2 | 10 | 18 | 36 | 54 | 86 | 118 => 7,
        57..=71 => 8,
        _ => 9,
    }
}

const FAMILIAS: [(&str, u32); 10] = [
    ("Alcalinos", 0xf6b8b0),
    ("Alcalinotérreos", 0xf9d5a7),
    ("Metales de transición", 0xf3e3a1),
    ("Otros metales", 0xd3e0c4),
    ("Metaloides", 0xbfe3d0),
    ("No metales", 0xb9dcf2),
    ("Halógenos", 0xc9d0f5),
    ("Gases nobles", 0xe0c8f0),
    ("Lantánidos", 0xf5c6de),
    ("Actínidos", 0xe8c3c3),
];

fn tabla_periodica(l: &mut Lapiz) {
    let (x0, y0, c) = (30.0, 30.0, 38.0);
    let celda = |fila: u32, col: u32| {
        let extra = if fila >= 9 { 16.0 } else { 0.0 };
        (
            x0 + c * (col - 1) as f32,
            y0 + c * (fila - 1) as f32 + extra,
        )
    };
    let borde = hex(0x8a94a6);
    for g in 1..=18 {
        txt(
            l,
            x0 + c * (g as f32 - 0.5),
            y0 - 10.0,
            &g.to_string(),
            9.0,
            GRIS,
        );
    }
    for p in 1..=7 {
        txt(
            l,
            x0 - 12.0,
            y0 + c * (p as f32 - 0.5),
            &p.to_string(),
            9.0,
            GRIS,
        );
    }
    for (i, s) in SIMBOLOS.split_whitespace().enumerate() {
        let z = i as u32 + 1;
        let (fila, col) = sitio(z);
        let (x, y) = celda(fila, col);
        l.rect(
            x + 1.0,
            y + 1.0,
            c - 2.0,
            c - 2.0,
            Some(borde),
            Some(hex(FAMILIAS[familia(z)].1)),
            0.8,
            false,
        );
        l.texto(
            x + 4.0,
            y + 3.0,
            &z.to_string(),
            7.0,
            WORK_SANS,
            TINTA,
            false,
            Al::Izq,
        );
        txt_n(l, x + c / 2.0, y + c / 2.0 + 4.0, s, 13.0, TINTA);
    }
    // El hueco de los lantanidos y los actinidos en el grupo 3.
    for (fila, s, f) in [(6, "57–71", 8), (7, "89–103", 9)] {
        let (x, y) = celda(fila, 3);
        l.rect(
            x + 1.0,
            y + 1.0,
            c - 2.0,
            c - 2.0,
            Some(borde),
            Some(hex(FAMILIAS[f].1)),
            0.8,
            false,
        );
        txt(l, x + c / 2.0, y + c / 2.0, s, 7.5, TINTA);
    }
    // La leyenda, en el hueco de arriba de los metales de transicion.
    for (i, (nombre, color)) in FAMILIAS.iter().enumerate() {
        let (col, fila) = (i % 2, i / 2);
        let (x, y) = (
            x0 + c * 2.0 + 16.0 + 150.0 * col as f32,
            y0 + 6.0 + 17.0 * fila as f32,
        );
        l.rect(x, y, 12.0, 12.0, Some(borde), Some(hex(*color)), 0.8, false);
        txt_a(l, x + 17.0, y + 6.0, nombre, 9.5, Al::Izq);
    }
}

// ----------------------------------------------------------------- enlaces

/// Electrones repartidos en un anillo.
fn electrones(l: &mut Lapiz, (cx, cy): (f32, f32), r: f32, n: usize, desde: f32, c: ColorRgba) {
    for i in 0..n {
        let a = (desde + 360.0 * i as f32 / n as f32).to_radians();
        l.circulo(cx + r * a.cos(), cy + r * a.sin(), 4.5, None, Some(c), 1.0);
    }
}

fn enlace_ionico(l: &mut Lapiz) {
    let (crema, menta) = (hex(0xfdeee0), hex(0xe5f4e6));
    l.circulo(60.0, 70.0, 46.0, Some(GRIS), Some(crema), 1.5);
    txt_n(l, 60.0, 70.0, "Na", 16.0, TINTA);
    electrones(l, (60.0, 70.0), 46.0, 1, 0.0, ROJO);
    l.circulo(240.0, 70.0, 52.0, Some(GRIS), Some(menta), 1.5);
    txt_n(l, 240.0, 70.0, "Cl", 16.0, TINTA);
    electrones(l, (240.0, 70.0), 52.0, 7, 0.0, AZUL);
    l.flecha(
        &bezier((112.0, 66.0), (150.0, 22.0), (184.0, 60.0), 12),
        ROJO,
        1.8,
        false,
    );
    txt(l, 150.0, 26.0, "cede 1 e⁻", 11.0, ROJO);
    l.flecha(&[(150.0, 136.0), (150.0, 166.0)], TINTA, 2.0, false);
    l.circulo(90.0, 216.0, 36.0, Some(ROJO), Some(crema), G);
    txt_n(l, 90.0, 216.0, "Na⁺", 15.0, ROJO);
    l.circulo(210.0, 216.0, 46.0, Some(AZUL), Some(menta), G);
    txt_n(l, 210.0, 216.0, "Cl⁻", 15.0, AZUL);
    electrones(l, (210.0, 216.0), 46.0, 8, 22.5, AZUL);
    txt(
        l,
        150.0,
        282.0,
        "atracción entre iones de carga opuesta",
        11.0,
        TINTA,
    );
}

fn enlace_covalente(l: &mut Lapiz) {
    l.circulo(80.0, 70.0, 56.0, Some(GRIS), Some(AZUL_SUAVE), 1.5);
    l.circulo(160.0, 70.0, 56.0, Some(GRIS), Some(AZUL_SUAVE), 1.5);
    l.arco(80.0, 70.0, 56.0, -46.0, 46.0, GRIS, 1.5);
    txt_n(l, 60.0, 70.0, "H", 18.0, TINTA);
    txt_n(l, 180.0, 70.0, "H", 18.0, TINTA);
    l.circulo(120.0, 58.0, 5.0, None, Some(ROJO), 1.0);
    l.circulo(120.0, 82.0, 5.0, None, Some(AZUL), 1.0);
    rotulo(
        l,
        (120.0, 82.0),
        (150.0, 150.0),
        "par de electrones compartido",
        Al::Izq,
    );
    txt_n(l, 120.0, 176.0, "H — H", 15.0, TINTA);
}

// ---------------------------------------------------------------- graficas

fn calentamiento(l: &mut Lapiz) {
    let o = (50.0, 240.0);
    cuadricula(l, o.0, 20.0, o.0 + 360.0, o.1, 30.0);
    ejes(l, o, (o.0, o.0 + 380.0), (10.0, o.1), ("tiempo", "T (°C)"));
    l.linea_con(
        o.0,
        190.0,
        100.0,
        190.0,
        GRIS,
        1.0,
        EstiloTrazo::Discontinuo,
    );
    l.linea_con(o.0, 90.0, 240.0, 90.0, GRIS, 1.0, EstiloTrazo::Discontinuo);
    txt_a(l, o.0 - 6.0, 190.0, "0", 10.0, Al::Der);
    txt_a(l, o.0 - 6.0, 90.0, "100", 10.0, Al::Der);
    l.poli(
        &[
            (50.0, 230.0),
            (100.0, 190.0),
            (170.0, 190.0),
            (240.0, 90.0),
            (350.0, 90.0),
            (400.0, 40.0),
        ],
        ROJO,
        3.0,
        None,
    );
    txt(l, 96.0, 222.0, "sólido", 10.0, AZUL);
    txt(l, 135.0, 204.0, "fusión", 10.0, MORADO);
    txt(l, 186.0, 136.0, "líquido", 10.0, AZUL);
    txt(l, 295.0, 104.0, "ebullición", 10.0, MORADO);
    txt(l, 360.0, 50.0, "gas", 10.0, AZUL);
}

/// Los colores del indicador universal, del 0 al 14.
const PH: [u32; 15] = [
    0xd7263d, 0xe63b2e, 0xf26b21, 0xf7941d, 0xfcc30b, 0xf2e30b, 0xb5d335, 0x5fbb46, 0x2fa96e,
    0x1c9c9a, 0x2a7fc1, 0x3a5ba9, 0x4b3f99, 0x5c2d91, 0x6a1f7f,
];

fn escala_ph(l: &mut Lapiz) {
    let (x0, y, w) = (10.0, 90.0, 36.0);
    for (i, c) in PH.iter().enumerate() {
        let x = x0 + w * i as f32;
        l.rect(x, y, w, 40.0, None, Some(hex(*c)), 1.0, false);
        txt_n(
            l,
            x + w / 2.0,
            y + 20.0,
            &i.to_string(),
            13.0,
            if (4..=8).contains(&i) { TINTA } else { BLANCO },
        );
    }
    l.rect(x0, y, w * 15.0, 40.0, Some(TINTA), None, 1.5, false);
    let ejemplos = [
        (1.5, "jugo gástrico"),
        (2.0, "limón"),
        (5.0, "café"),
        (7.0, "agua pura"),
        (7.4, "sangre"),
        (10.0, "jabón"),
        (13.0, "lejía"),
    ];
    for (i, (ph, s)) in ejemplos.iter().enumerate() {
        let x = x0 + w * (ph + 0.5);
        let alto = if i % 2 == 0 { 66.0 } else { 36.0 };
        l.linea(x, y, x, y - alto + 10.0, GRIS, 1.0);
        txt(l, x, y - alto, s, 10.0, TINTA);
    }
    l.flecha(&[(x0 + w * 6.5, 150.0), (x0, 150.0)], ROJO, 1.6, false);
    l.flecha(
        &[(x0 + w * 8.5, 150.0), (x0 + w * 15.0, 150.0)],
        MORADO,
        1.6,
        false,
    );
    txt(l, x0 + w * 3.0, 166.0, "ácido", 12.0, ROJO);
    txt_n(l, x0 + w * 7.5, 150.0, "neutro", 12.0, VERDE);
    txt(l, x0 + w * 12.0, 166.0, "básico (alcalino)", 12.0, MORADO);
}
