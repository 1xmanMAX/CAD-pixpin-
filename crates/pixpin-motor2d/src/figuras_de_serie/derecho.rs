//! **Derecho**: esquemas para estudiar (fases del proceso civil y del
//! penal, piramide de Kelsen, poderes del Estado, mapa de articulos, linea
//! de tiempo de un caso, silogismo juridico, partes del proceso), hojas
//! para trabajar casos (IRAC, ficha de sentencia, cuadro comparativo) y
//! simbolos (balanza, mazo y codigo).
//!
//! Azul marino con dorado, el aire de los libros de leyes.

use super::tecnico::{BLANCO, Dibujo, caja_con_texto, forma, girar, poner, punta};
use super::{Al, FRAUNCES, Lapiz, WORK_SANS, hex};
use crate::biblioteca::FiguraGuardada;
use crate::elemento::{ColorRgba, EstiloTrazo};

pub(super) const CATEGORIA: &str = "Derecho";

const MARINO: ColorRgba = hex(0x1f3a5f);
const MARINO_MEDIO: ColorRgba = hex(0x3d5f8c);
const DORADO: ColorRgba = hex(0xb8892d);
const DORADO_CLARO: ColorRgba = hex(0xf3e6c8);
const SUAVE: ColorRgba = hex(0xeaf0f8);
const CREMA: ColorRgba = hex(0xfbf7ee);
const ROTULO: ColorRgba = hex(0x5d6b82);
const RAYA: ColorRgba = hex(0xcfd7e3);

pub(super) fn todas(v: &mut Vec<FiguraGuardada>) {
    let lista: [Dibujo; 14] = [
        (
            "der-proceso-civil",
            "Proceso judicial (fases)",
            proceso_civil,
        ),
        ("der-proceso-penal", "Proceso penal (etapas)", proceso_penal),
        ("der-kelsen", "Pirámide de Kelsen", kelsen),
        ("der-poderes", "Poderes del Estado", poderes),
        (
            "der-mapa-articulos",
            "Mapa conceptual de artículos",
            mapa_de_articulos,
        ),
        (
            "der-linea-tiempo",
            "Línea de tiempo del caso",
            linea_de_tiempo,
        ),
        ("der-irac", "Análisis de caso (IRAC)", irac),
        (
            "der-ficha-sentencia",
            "Ficha de análisis de sentencia",
            ficha_de_sentencia,
        ),
        (
            "der-cuadro-comparativo",
            "Cuadro comparativo",
            cuadro_comparativo,
        ),
        ("der-silogismo", "Silogismo jurídico", silogismo),
        ("der-partes", "Partes del proceso", partes),
        ("der-balanza", "Balanza de la justicia", balanza),
        ("der-mazo", "Mazo de juez", mazo),
        ("der-codigo", "Código (libro de leyes)", codigo),
    ];
    poner(v, CATEGORIA, &lista);
}

fn titulo(l: &mut Lapiz, x: f32, y: f32, s: &str, al: Al) {
    l.texto(x, y, s, 22.0, FRAUNCES, MARINO, true, al);
}

/// Una flecha recta de esquema con su punta rellena.
fn flecha(l: &mut Lapiz, a: (f32, f32), b: (f32, f32), c: ColorRgba) {
    l.linea(a.0, a.1, b.0, b.1, c, 1.75);
    let g = (b.1 - a.1).atan2(b.0 - a.0).to_degrees();
    punta(l, b.0, b.1, g, 10.0, c);
}

/// Renglones para escribir, de `x0` a `x1`, desde `y` cada `paso`.
fn renglones(l: &mut Lapiz, x0: f32, x1: f32, y: f32, n: usize, paso: f32) {
    for i in 0..n {
        let y = y + paso * i as f32;
        l.linea(x0, y, x1, y, RAYA, 1.0);
    }
}

// ----------------------------------------------------------------- esquemas

fn proceso_civil(l: &mut Lapiz) {
    let fases = [
        ("Demanda", "Pretensión y\npruebas iniciales"),
        ("Contestación", "Defensa y\nexcepciones"),
        ("Saneamiento", "Audiencia previa\ny conciliación"),
        ("Prueba", "Actuación de\nmedios probatorios"),
        ("Alegatos", "Conclusiones\nde las partes"),
        ("Sentencia", "Decisión del\njuez"),
        ("Recursos", "Apelación,\ncasación"),
    ];
    let (w, h, hueco, y0) = (124.0, 70.0, 30.0, 60.0);
    titulo(l, 0.0, 0.0, "Proceso judicial", Al::Izq);
    for (i, (fase, que)) in fases.iter().enumerate() {
        let x = (w + hueco) * i as f32;
        let ultima = i == fases.len() - 1;
        let (relleno, tinta) = if ultima {
            (CREMA, MARINO)
        } else if i == 5 {
            (MARINO, BLANCO)
        } else {
            (SUAVE, MARINO)
        };
        caja_con_texto(l, x, y0, w, h, fase, 14.0, MARINO, relleno, tinta);
        if ultima
            && let Some(e) =
                l.v.iter_mut()
                    .rev()
                    .find(|e| matches!(e.figura, crate::elemento::Figura::Rectangulo))
        {
            e.estilo = EstiloTrazo::Discontinuo;
        }
        l.circulo(x + 14.0, y0, 12.0, Some(DORADO), Some(DORADO), 1.0);
        l.texto(
            x + 14.0,
            y0 - 8.0,
            &format!("{}", i + 1),
            11.0,
            WORK_SANS,
            BLANCO,
            true,
            Al::Centro,
        );
        l.texto(
            x + w / 2.0,
            y0 + h + 10.0,
            que,
            11.0,
            WORK_SANS,
            ROTULO,
            false,
            Al::Centro,
        );
        if !ultima {
            flecha(
                l,
                (x + w + 3.0, y0 + h / 2.0),
                (x + w + hueco - 3.0, y0 + h / 2.0),
                DORADO,
            );
        }
    }
}

fn proceso_penal(l: &mut Lapiz) {
    let etapas = [
        "Denuncia",
        "Investigación\npreparatoria",
        "Etapa\nintermedia",
        "Juicio\noral",
        "Sentencia y\nejecución",
    ];
    let (w, h, pico, y0) = (150.0, 76.0, 22.0, 50.0);
    titulo(l, 0.0, 0.0, "Proceso penal", Al::Izq);
    let tonos = [
        hex(0x5d7fae),
        hex(0x4d6f9e),
        MARINO_MEDIO,
        hex(0x2d4d78),
        MARINO,
    ];
    for (i, s) in etapas.iter().enumerate() {
        let x = (w + 6.0) * i as f32;
        let entrada = if i == 0 { 0.0 } else { pico };
        forma(
            l,
            &[
                (x, y0),
                (x + w, y0),
                (x + w + pico, y0 + h / 2.0),
                (x + w, y0 + h),
                (x, y0 + h),
                (x + entrada, y0 + h / 2.0),
            ],
            MARINO,
            Some(tonos[i]),
            1.5,
        );
        let lineas = s.lines().count() as f32;
        let cx = x + (w + entrada + pico * 0.5) / 2.0;
        l.texto(
            cx,
            y0 + h / 2.0 - 8.0 * lineas,
            s,
            13.0,
            WORK_SANS,
            BLANCO,
            true,
            Al::Centro,
        );
    }
}

fn kelsen(l: &mut Lapiz) {
    let (base, alto, cx) = (480.0, 400.0, 240.0);
    let ys = [0.0, 110.0, 180.0, 250.0, 320.0, 400.0];
    let niveles = [
        ("I", "Constitución"),
        ("II", "Tratados internacionales (DD. HH.)"),
        ("III", "Leyes orgánicas y ordinarias"),
        ("IV", "Decretos y reglamentos"),
        ("V", "Resoluciones, sentencias y actos"),
    ];
    let tonos = [
        DORADO,
        hex(0x2d4d78),
        MARINO_MEDIO,
        hex(0x5d7fae),
        hex(0x8aa6cb),
    ];
    let ancho = |y: f32| base * y / alto;
    for (i, (num, nombre)) in niveles.iter().enumerate() {
        let (a, b) = (ys[i], ys[i + 1]);
        let (wa, wb) = (ancho(a), ancho(b));
        forma(
            l,
            &[
                (cx - wa / 2.0, a),
                (cx + wa / 2.0, a),
                (cx + wb / 2.0, b),
                (cx - wb / 2.0, b),
            ],
            MARINO,
            Some(tonos[i]),
            1.5,
        );
        let ym = if i == 0 { 76.0 } else { (a + b) / 2.0 };
        l.texto(cx, ym - 10.0, num, 16.0, FRAUNCES, BLANCO, true, Al::Centro);
        let borde = cx + ancho(ym) / 2.0;
        l.linea(borde + 6.0, ym, 520.0, ym, ROTULO, 1.0);
        l.circulo(borde + 6.0, ym, 2.5, Some(ROTULO), Some(ROTULO), 1.0);
        l.texto(
            528.0,
            ym - 9.0,
            nombre,
            14.0,
            WORK_SANS,
            MARINO,
            i == 0,
            Al::Izq,
        );
    }
    l.texto(
        cx,
        alto + 16.0,
        "Pirámide de Kelsen",
        20.0,
        FRAUNCES,
        MARINO,
        true,
        Al::Centro,
    );
}

fn poderes(l: &mut Lapiz) {
    let (w, h) = (200.0, 56.0);
    let xs = [0.0, 250.0, 500.0];
    let centro = xs[1] + w / 2.0;
    caja_con_texto(
        l,
        xs[1],
        0.0,
        w,
        h,
        "ESTADO\nSoberanía popular",
        15.0,
        DORADO,
        DORADO_CLARO,
        MARINO,
    );
    l.linea(centro, h, centro, 90.0, MARINO, 1.75);
    l.linea(xs[0] + w / 2.0, 90.0, xs[2] + w / 2.0, 90.0, MARINO, 1.75);
    let ramas = [
        (
            "Poder Legislativo",
            ["Congreso / Parlamento", "Elabora y aprueba\nlas leyes"],
        ),
        (
            "Poder Ejecutivo",
            ["Presidente y Gobierno", "Ministerios y\nadministración"],
        ),
        ("Poder Judicial", ["Tribunal Supremo", "Cortes y juzgados"]),
    ];
    for (x, (poder, hijos)) in xs.iter().zip(ramas) {
        let cxr = x + w / 2.0;
        flecha(l, (cxr, 90.0), (cxr, 116.0), MARINO);
        caja_con_texto(l, *x, 118.0, w, h, poder, 15.0, MARINO, MARINO, BLANCO);
        for (j, hijo) in hijos.iter().enumerate() {
            let y = 210.0 + 76.0 * j as f32;
            l.linea(x + 18.0, 118.0 + h, x + 18.0, y + 26.0, MARINO_MEDIO, 1.25);
            l.linea(x + 18.0, y + 26.0, x + 34.0, y + 26.0, MARINO_MEDIO, 1.25);
            caja_con_texto(
                l,
                x + 34.0,
                y,
                w - 34.0,
                52.0,
                hijo,
                12.0,
                MARINO_MEDIO,
                SUAVE,
                MARINO,
            );
        }
    }
    caja_con_texto(
        l,
        120.0,
        380.0,
        460.0,
        46.0,
        "Organismos constitucionales autónomos",
        13.0,
        ROTULO,
        BLANCO,
        ROTULO,
    );
    if let Some(e) =
        l.v.iter_mut()
            .rev()
            .find(|e| matches!(e.figura, crate::elemento::Figura::Rectangulo))
    {
        e.estilo = EstiloTrazo::Discontinuo;
    }
}

fn mapa_de_articulos(l: &mut Lapiz) {
    let (cx, cy, rx, ry) = (380.0, 250.0, 280.0, 180.0);
    let articulos = [
        ("Art. 1.º\nObjeto", "establece"),
        ("Art. 2.º\nÁmbito de aplicación", "delimita"),
        ("Art. 3.º\nDefiniciones", "define"),
        ("Art. 4.º\nDerechos", "reconoce"),
        ("Art. 5.º\nObligaciones", "impone"),
        ("Art. 6.º\nSanciones", "prevé"),
    ];
    let (w, h) = (160.0, 56.0);
    let puntos: Vec<(f32, f32)> = (0..articulos.len())
        .map(|i| {
            let a = (-90.0 + 60.0 * i as f32).to_radians();
            (cx + rx * a.cos(), cy + ry * a.sin())
        })
        .collect();
    for ((px, py), (_, verbo)) in puntos.iter().zip(articulos) {
        l.linea(cx, cy, *px, *py, MARINO_MEDIO, 1.5);
        let (mx, my) = ((cx + px) / 2.0, (cy + py) / 2.0);
        let ancho = 12.0 + 7.0 * verbo.chars().count() as f32;
        l.rect(
            mx - ancho / 2.0,
            my - 10.0,
            ancho,
            20.0,
            None,
            Some(BLANCO),
            1.0,
            true,
        );
        l.texto(
            mx,
            my - 8.0,
            verbo,
            11.0,
            WORK_SANS,
            DORADO,
            true,
            Al::Centro,
        );
    }
    for ((px, py), (art, _)) in puntos.iter().zip(articulos) {
        caja_con_texto(
            l,
            px - w / 2.0,
            py - h / 2.0,
            w,
            h,
            art,
            13.0,
            MARINO_MEDIO,
            SUAVE,
            MARINO,
        );
    }
    l.elipse(
        cx - 100.0,
        cy - 40.0,
        200.0,
        80.0,
        Some(DORADO),
        Some(MARINO),
        2.5,
    );
    l.texto(
        cx,
        cy - 22.0,
        "LEY / CÓDIGO",
        17.0,
        FRAUNCES,
        BLANCO,
        true,
        Al::Centro,
    );
    l.texto(
        cx,
        cy + 2.0,
        "Título preliminar",
        11.0,
        WORK_SANS,
        DORADO_CLARO,
        false,
        Al::Centro,
    );
}

fn linea_de_tiempo(l: &mut Lapiz) {
    let eventos = [
        "Hechos",
        "Denuncia /\ndemanda",
        "Admisión",
        "Audiencia",
        "Sentencia",
        "Apelación",
    ];
    let (x0, paso, y) = (70.0, 150.0, 200.0);
    titulo(l, 0.0, 0.0, "Línea de tiempo del caso", Al::Izq);
    l.linea(0.0, y, x0 + paso * 5.0 + 70.0, y, MARINO, 3.0);
    punta(l, x0 + paso * 5.0 + 84.0, y, 0.0, 16.0, MARINO);
    let (w, h) = (128.0, 64.0);
    for (i, s) in eventos.iter().enumerate() {
        let x = x0 + paso * i as f32;
        let arriba = i % 2 == 0;
        let (ty, sy0, sy1) = if arriba {
            (y - 40.0 - h, y - 40.0, y - 8.0)
        } else {
            (y + 40.0, y + 8.0, y + 40.0)
        };
        l.linea(x, sy0, x, sy1, DORADO, 1.5);
        l.circulo(x, y, 8.0, Some(MARINO), Some(DORADO), 2.0);
        l.rect(
            x - w / 2.0,
            ty,
            w,
            h,
            Some(MARINO_MEDIO),
            Some(SUAVE),
            1.25,
            true,
        );
        l.texto(
            x,
            ty + 6.0,
            "dd / mm / aaaa",
            10.0,
            WORK_SANS,
            ROTULO,
            false,
            Al::Centro,
        );
        l.texto(x, ty + 22.0, s, 13.0, WORK_SANS, MARINO, true, Al::Centro);
    }
}

fn silogismo(l: &mut Lapiz) {
    let w = 420.0;
    let filas = [
        (
            "PREMISA MAYOR",
            "La norma jurídica aplicable",
            SUAVE,
            MARINO,
        ),
        (
            "PREMISA MENOR",
            "Los hechos probados del caso",
            SUAVE,
            MARINO,
        ),
        (
            "CONCLUSIÓN",
            "El fallo: la consecuencia jurídica",
            MARINO,
            BLANCO,
        ),
    ];
    titulo(l, w / 2.0, 0.0, "Silogismo jurídico", Al::Centro);
    for (i, (rot, que, relleno, tinta)) in filas.iter().enumerate() {
        let y = 50.0 + 110.0 * i as f32 + if i == 2 { 20.0 } else { 0.0 };
        l.rect(0.0, y, w, 70.0, Some(MARINO), Some(*relleno), 1.75, true);
        l.texto(
            w / 2.0,
            y + 12.0,
            rot,
            12.0,
            WORK_SANS,
            if i == 2 { DORADO_CLARO } else { DORADO },
            true,
            Al::Centro,
        );
        l.texto(
            w / 2.0,
            y + 34.0,
            que,
            15.0,
            WORK_SANS,
            *tinta,
            false,
            Al::Centro,
        );
    }
    l.texto(
        w / 2.0,
        128.0,
        "+",
        22.0,
        WORK_SANS,
        DORADO,
        true,
        Al::Centro,
    );
    l.linea(40.0, 256.0, w - 40.0, 256.0, MARINO, 2.0);
    // El «por tanto» (∴), con tres puntos: no todas las fuentes lo tienen.
    l.rect(
        w / 2.0 - 24.0,
        240.0,
        48.0,
        30.0,
        None,
        Some(BLANCO),
        1.0,
        false,
    );
    for (dx, dy) in [(0.0, -7.0), (-8.0, 7.0), (8.0, 7.0)] {
        l.circulo(
            w / 2.0 + dx,
            256.0 + dy,
            3.5,
            Some(DORADO),
            Some(DORADO),
            1.0,
        );
    }
}

fn partes(l: &mut Lapiz) {
    let juez = (260.0, 50.0);
    let (dte, ddo) = ((60.0, 280.0), (460.0, 280.0));
    let r = 48.0;
    let hacia = |a: (f32, f32), b: (f32, f32), d: f32| {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let n = (dx * dx + dy * dy).sqrt();
        (a.0 + dx / n * d, a.1 + dy / n * d)
    };
    flecha(
        l,
        hacia(dte, juez, r + 6.0),
        hacia(juez, dte, r + 6.0),
        MARINO_MEDIO,
    );
    flecha(
        l,
        hacia(ddo, juez, r + 6.0),
        hacia(juez, ddo, r + 6.0),
        MARINO_MEDIO,
    );
    l.linea_con(
        dte.0 + r + 6.0,
        dte.1,
        ddo.0 - r - 6.0,
        ddo.1,
        DORADO,
        2.0,
        EstiloTrazo::Discontinuo,
    );
    l.texto(
        126.0,
        150.0,
        "pretensión",
        12.0,
        WORK_SANS,
        MARINO_MEDIO,
        true,
        Al::Der,
    );
    l.texto(
        394.0,
        150.0,
        "defensa",
        12.0,
        WORK_SANS,
        MARINO_MEDIO,
        true,
        Al::Izq,
    );
    l.texto(
        260.0,
        288.0,
        "conflicto (litis)",
        12.0,
        WORK_SANS,
        DORADO,
        true,
        Al::Centro,
    );
    for ((x, y), s, relleno, tinta) in [
        (juez, "Juez", MARINO, BLANCO),
        (dte, "Demandante", SUAVE, MARINO),
        (ddo, "Demandado", SUAVE, MARINO),
    ] {
        l.circulo(x, y, r, Some(MARINO), Some(relleno), 2.0);
        l.texto(x, y - 9.0, s, 14.0, WORK_SANS, tinta, true, Al::Centro);
    }
}

// --------------------------------------------------------------------- hojas

/// Una hoja con titulo, para casos: devuelve el lapiz y donde sigue.
fn hoja(w: f32, h: f32, t: &str) -> (Lapiz, f32) {
    let mut l = Lapiz::default();
    l.rect(
        0.0,
        0.0,
        w,
        h,
        Some(hex(0xdde3ec)),
        Some(BLANCO),
        1.0,
        false,
    );
    l.rect(0.0, 0.0, w, 10.0, None, Some(MARINO), 1.0, false);
    l.texto(36.0, 34.0, t, 24.0, FRAUNCES, MARINO, true, Al::Izq);
    l.linea(36.0, 76.0, w - 36.0, 76.0, DORADO, 2.0);
    (l, 92.0)
}

fn irac(l: &mut Lapiz) {
    let w = 640.0;
    let (mut p, mut y) = hoja(w, 940.0, "Análisis de caso · IRAC");
    p.texto(36.0, y, "CASO", 10.0, WORK_SANS, ROTULO, true, Al::Izq);
    p.linea(80.0, y + 14.0, w - 36.0, y + 14.0, RAYA, 1.0);
    y += 40.0;
    let secciones = [
        (
            "H",
            "Hechos",
            "¿Qué pasó? Partes, fechas y hechos relevantes.",
            4,
        ),
        (
            "I",
            "Cuestión jurídica",
            "¿Qué problema de derecho debe resolverse?",
            3,
        ),
        ("R", "Regla", "Norma, artículo o precedente aplicable.", 3),
        (
            "A",
            "Análisis",
            "Aplicación de la regla a los hechos; argumentos de cada parte.",
            5,
        ),
        ("C", "Conclusión", "Cómo se resuelve y por qué.", 3),
    ];
    for (letra, nombre, pista, n) in secciones {
        p.circulo(54.0, y + 16.0, 16.0, Some(MARINO), Some(MARINO), 1.0);
        p.texto(
            54.0,
            y + 5.0,
            letra,
            16.0,
            FRAUNCES,
            DORADO_CLARO,
            true,
            Al::Centro,
        );
        p.texto(82.0, y, nombre, 16.0, WORK_SANS, MARINO, true, Al::Izq);
        p.texto(
            82.0,
            y + 21.0,
            pista,
            11.0,
            WORK_SANS,
            ROTULO,
            false,
            Al::Izq,
        );
        renglones(&mut p, 82.0, w - 36.0, y + 64.0, n, 28.0);
        y += 64.0 + 28.0 * n as f32 + 8.0;
    }
    l.v.extend(p.v);
}

fn ficha_de_sentencia(l: &mut Lapiz) {
    let w = 640.0;
    let (mut p, mut y) = hoja(w, 1000.0, "Ficha de sentencia");
    let campos = [
        ("TRIBUNAL", "EXPEDIENTE N.º"),
        ("FECHA", "PONENTE / MAGISTRADO"),
        ("DEMANDANTE", "DEMANDADO"),
        ("MATERIA", "SENTIDO DEL FALLO"),
    ];
    let mitad = w / 2.0;
    for (a, b) in campos {
        for (x, s) in [(36.0, a), (mitad + 10.0, b)] {
            p.rect(x, y, mitad - 46.0, 44.0, Some(RAYA), Some(CREMA), 1.0, true);
            p.texto(x + 8.0, y + 6.0, s, 9.0, WORK_SANS, DORADO, true, Al::Izq);
        }
        y += 52.0;
    }
    y += 8.0;
    for (s, n) in [
        ("Antecedentes", 3),
        ("Fundamentos jurídicos", 4),
        ("Fallo / decisión", 2),
        ("Ratio decidendi", 2),
        ("Obiter dicta", 2),
        ("Comentario crítico", 3),
    ] {
        p.rect(36.0, y + 2.0, 4.0, 18.0, None, Some(DORADO), 1.0, false);
        p.texto(48.0, y, s, 15.0, WORK_SANS, MARINO, true, Al::Izq);
        renglones(&mut p, 48.0, w - 36.0, y + 50.0, n, 28.0);
        y += 50.0 + 28.0 * n as f32;
    }
    l.v.extend(p.v);
}

fn cuadro_comparativo(l: &mut Lapiz) {
    let (w0, wc, n) = (170.0, 250.0, 2);
    let w = w0 + wc * n as f32;
    let filas = [
        "Concepto",
        "Naturaleza jurídica",
        "Base legal",
        "Requisitos",
        "Efectos",
        "Ejemplo",
    ];
    let (hc, hf) = (46.0, 70.0);
    titulo(l, w / 2.0, 0.0, "Cuadro comparativo", Al::Centro);
    let y0 = 44.0;
    l.rect(0.0, y0, w, hc, Some(MARINO), Some(MARINO), 1.5, false);
    l.texto(
        w0 / 2.0,
        y0 + 14.0,
        "CRITERIO",
        12.0,
        WORK_SANS,
        DORADO_CLARO,
        true,
        Al::Centro,
    );
    for (i, s) in ["Institución A", "Institución B"].iter().enumerate() {
        l.texto(
            w0 + wc * i as f32 + wc / 2.0,
            y0 + 13.0,
            s,
            14.0,
            WORK_SANS,
            BLANCO,
            true,
            Al::Centro,
        );
    }
    for (i, s) in filas.iter().enumerate() {
        let y = y0 + hc + hf * i as f32;
        let fondo = if i % 2 == 0 { BLANCO } else { hex(0xf6f8fb) };
        l.rect(0.0, y, w, hf, Some(RAYA), Some(fondo), 1.0, false);
        l.rect(0.0, y, w0, hf, Some(RAYA), Some(SUAVE), 1.0, false);
        l.texto(
            14.0,
            y + hf / 2.0 - 9.0,
            s,
            13.0,
            WORK_SANS,
            MARINO,
            true,
            Al::Izq,
        );
    }
    for i in 1..n {
        let x = w0 + wc * i as f32;
        l.linea(x, y0 + hc, x, y0 + hc + hf * filas.len() as f32, RAYA, 1.0);
    }
    l.rect(
        0.0,
        y0,
        w,
        hc + hf * filas.len() as f32,
        Some(MARINO),
        None,
        1.75,
        false,
    );
}

// ------------------------------------------------------------------ simbolos

fn balanza(l: &mut Lapiz) {
    let cx = 160.0;
    // El pie y el mastil.
    forma(
        l,
        &[
            (cx - 70.0, 300.0),
            (cx + 70.0, 300.0),
            (cx + 54.0, 284.0),
            (cx - 54.0, 284.0),
        ],
        DORADO,
        Some(DORADO_CLARO),
        2.0,
    );
    l.rect(
        cx - 7.0,
        50.0,
        14.0,
        234.0,
        Some(DORADO),
        Some(DORADO_CLARO),
        2.0,
        false,
    );
    l.circulo(cx, 36.0, 14.0, Some(DORADO), Some(DORADO_CLARO), 2.0);
    // La cruz, un poco inclinada.
    let cruz = girar(
        &[
            (cx - 130.0, 70.0),
            (cx + 130.0, 70.0),
            (cx + 130.0, 80.0),
            (cx - 130.0, 80.0),
        ],
        (cx, 75.0),
        -4.0,
    );
    forma(l, &cruz, DORADO, Some(DORADO_CLARO), 2.0);
    for (i, k) in [(0, -1.0), (1, 1.0)] {
        let (px, py) = if i == 0 { cruz[0] } else { cruz[1] };
        let (px, py) = (px + 4.0 * -k, py + 5.0);
        let fondo = py + 120.0;
        l.linea(px, py, px - 46.0, fondo, DORADO, 1.25);
        l.linea(px, py, px + 46.0, fondo, DORADO, 1.25);
        let plato: Vec<(f32, f32)> = (0..=16)
            .map(|j| {
                let a = std::f32::consts::PI * j as f32 / 16.0;
                (px + 54.0 * a.cos(), fondo + 26.0 * a.sin())
            })
            .collect();
        forma(l, &plato, DORADO, Some(DORADO_CLARO), 2.0);
        l.circulo(px, py, 4.0, Some(DORADO), Some(DORADO), 1.0);
    }
}

fn mazo(l: &mut Lapiz) {
    // La base, donde golpea.
    l.elipse(
        150.0,
        214.0,
        170.0,
        40.0,
        Some(MARINO),
        Some(hex(0x6b4a33)),
        2.0,
    );
    l.rect(
        150.0,
        214.0,
        170.0,
        18.0,
        None,
        Some(hex(0x6b4a33)),
        1.0,
        false,
    );
    l.elipse(
        150.0,
        200.0,
        170.0,
        36.0,
        Some(MARINO),
        Some(hex(0x8a6446)),
        2.0,
    );
    let centro = (150.0, 120.0);
    let madera = hex(0x8a6446);
    let oscura = hex(0x6b4a33);
    // El mango, inclinado.
    let mango = girar(
        &[
            (150.0, 112.0),
            (330.0, 112.0),
            (336.0, 116.0),
            (336.0, 124.0),
            (330.0, 128.0),
            (150.0, 128.0),
        ],
        centro,
        -28.0,
    );
    forma(l, &mango, MARINO, Some(madera), 2.0);
    // La cabeza, con sus dos anillos dorados.
    let cabeza = girar(
        &[(110.0, 60.0), (190.0, 60.0), (190.0, 180.0), (110.0, 180.0)],
        centro,
        -28.0,
    );
    forma(l, &cabeza, MARINO, Some(oscura), 2.0);
    for (y0, y1) in [(72.0, 84.0), (156.0, 168.0)] {
        let anillo = girar(
            &[(110.0, y0), (190.0, y0), (190.0, y1), (110.0, y1)],
            centro,
            -28.0,
        );
        forma(l, &anillo, MARINO, Some(DORADO), 1.25);
    }
}

fn codigo(l: &mut Lapiz) {
    let (w, h) = (210.0, 280.0);
    // Las hojas, que asoman por la derecha y por abajo.
    l.rect(
        14.0,
        10.0,
        w,
        h,
        Some(hex(0xc9c2b0)),
        Some(CREMA),
        1.25,
        true,
    );
    for i in 0..4 {
        let d = 3.0 * i as f32;
        l.linea(
            w + 10.0 - d,
            16.0 + d,
            w + 10.0 - d,
            h + 4.0 - d,
            hex(0xc9c2b0),
            0.75,
        );
    }
    l.rect(0.0, 0.0, w, h, Some(MARINO), Some(MARINO), 2.0, true);
    l.rect(0.0, 0.0, 24.0, h, None, Some(hex(0x16304f)), 1.0, true);
    l.rect(
        40.0,
        26.0,
        w - 64.0,
        h - 52.0,
        Some(DORADO),
        None,
        1.5,
        false,
    );
    l.texto(
        24.0 + (w - 24.0) / 2.0,
        54.0,
        "CÓDIGO",
        20.0,
        FRAUNCES,
        DORADO_CLARO,
        true,
        Al::Centro,
    );
    l.texto(
        24.0 + (w - 24.0) / 2.0,
        82.0,
        "CIVIL",
        15.0,
        FRAUNCES,
        DORADO_CLARO,
        false,
        Al::Centro,
    );
    l.texto(
        24.0 + (w - 24.0) / 2.0,
        126.0,
        "§",
        28.0,
        FRAUNCES,
        DORADO,
        true,
        Al::Centro,
    );
}
