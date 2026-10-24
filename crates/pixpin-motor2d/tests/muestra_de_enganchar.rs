//! **La rejilla, el lazo y las seis marcas del iman, en PNG que se miran.**
//!
//! Hay cosas de este grupo que ningun `assert_eq` comprueba de verdad:
//!
//! - que la rejilla sea **finisima**. Una que se ve tanto como el dibujo
//!   compite con el y cansa a los dos minutos, y eso no es un numero: se
//!   mira. Lo que si se puede medir de ella, y se mide aqui, es que al
//!   alejarse **no se cierre en una malla**, que es el fallo que no parece
//!   un fallo —se ve como que el lienzo se ha ensuciado—;
//! - que las seis marcas del iman **se distingan entre si**. Aparecen a un
//!   par de pixeles unas de otras cuando una raya toca una esquina, y si dos
//!   se parecen, la queja «engancha donde no quiero» no se puede ni
//!   diagnosticar.
//!
//! Los PNG se escriben en `target/muestras-grupo-b/`. Para rehacerlos:
//! `cargo test -p pixpin-motor2d --test muestra_de_enganchar -- --nocapture`.
//!
//! El codificador PNG de aqui abajo es de juguete a proposito —bloques sin
//! comprimir— para no meterle una dependencia al motor por una muestra.

use std::path::PathBuf;

use pixpin_motor2d::cuadricula::{Cuadricula, EstiloRejilla};
use pixpin_motor2d::enganche::{Anclaje, TipoAnclaje, pista};
use pixpin_motor2d::lazo::Lazo;
use pixpin_motor2d::pintado::Orden;
use pixpin_motor2d::vector::Punto2;

const ANCHO: u32 = 420;
const ALTO: u32 = 220;

const PAPEL: [u8; 4] = [255, 255, 255, 255];

fn lienzo() -> Vec<u8> {
    PAPEL
        .iter()
        .copied()
        .cycle()
        .take((ANCHO * ALTO * 4) as usize)
        .collect()
}

fn punto(v: &mut [u8], x: i32, y: i32, c: [u8; 4]) {
    if x < 0 || y < 0 || x >= ANCHO as i32 || y >= ALTO as i32 {
        return;
    }
    let i = ((y as u32 * ANCHO + x as u32) * 4) as usize;
    // Mezcla sobre lo que haya: la rejilla es casi transparente y sin
    // mezclar saldria negra, que es justo lo contrario de lo que se quiere
    // demostrar.
    let a = c[3] as f32 / 255.0;
    for k in 0..3 {
        v[i + k] = (c[k] as f32 * a + v[i + k] as f32 * (1.0 - a)) as u8;
    }
    v[i + 3] = 255;
}

/// Una raya de Bresenham, con grosor de un pixel. Basta: lo que se mira aqui
/// es la forma y la densidad, no el antialiasing.
fn raya(v: &mut [u8], a: Punto2, b: Punto2, c: [u8; 4]) {
    let (mut x0, mut y0) = (a.x.round() as i32, a.y.round() as i32);
    let (x1, y1) = (b.x.round() as i32, b.y.round() as i32);
    let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
    let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
    let mut err = dx + dy;
    loop {
        punto(v, x0, y0, c);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

/// Pinta las ordenes que este grupo produce. Solo las dos que usa.
fn pintar(v: &mut [u8], ordenes: &[Orden]) {
    for o in ordenes {
        match o {
            Orden::Polilinea { puntos, color, .. } => {
                let c = [
                    (color.r * 255.0) as u8,
                    (color.g * 255.0) as u8,
                    (color.b * 255.0) as u8,
                    (color.a * 255.0) as u8,
                ];
                for par in puntos.windows(2) {
                    raya(v, par[0], par[1], c);
                }
            }
            Orden::Relleno { puntos, color } => {
                let c = [
                    (color.r * 255.0) as u8,
                    (color.g * 255.0) as u8,
                    (color.b * 255.0) as u8,
                    (color.a * 255.0) as u8,
                ];
                let x0 = puntos.iter().map(|p| p.x).fold(f32::MAX, f32::min).round() as i32;
                let x1 = puntos.iter().map(|p| p.x).fold(f32::MIN, f32::max).round() as i32;
                let y0 = puntos.iter().map(|p| p.y).fold(f32::MAX, f32::min).round() as i32;
                let y1 = puntos.iter().map(|p| p.y).fold(f32::MIN, f32::max).round() as i32;
                for y in y0..=y1 {
                    for x in x0..=x1 {
                        if dentro(puntos, x as f32 + 0.5, y as f32 + 0.5) {
                            punto(v, x, y, c);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn dentro(poligono: &[Punto2], px: f32, py: f32) -> bool {
    let mut d = false;
    let mut j = poligono.len() - 1;
    for i in 0..poligono.len() {
        let (a, b) = (poligono[i], poligono[j]);
        if (a.y > py) != (b.y > py) && px < (b.x - a.x) * (py - a.y) / (b.y - a.y) + a.x {
            d = !d;
        }
        j = i;
    }
    d
}

fn png(ancho: u32, alto: u32, rgba: &[u8]) -> Vec<u8> {
    fn crc32(datos: &[u8]) -> u32 {
        let mut c = 0xFFFF_FFFFu32;
        for &b in datos {
            c ^= b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
        }
        !c
    }
    fn trozo(salida: &mut Vec<u8>, tipo: &[u8; 4], datos: &[u8]) {
        salida.extend_from_slice(&(datos.len() as u32).to_be_bytes());
        let mut con_tipo = tipo.to_vec();
        con_tipo.extend_from_slice(datos);
        salida.extend_from_slice(&con_tipo);
        salida.extend_from_slice(&crc32(&con_tipo).to_be_bytes());
    }

    // Los datos del PNG llevan un byte de filtro por fila (0 = ninguno).
    let mut crudo = Vec::with_capacity((alto * (1 + ancho * 4)) as usize);
    for y in 0..alto {
        crudo.push(0);
        let desde = (y * ancho * 4) as usize;
        crudo.extend_from_slice(&rgba[desde..desde + (ancho * 4) as usize]);
    }
    // zlib con bloques sin comprimir: cabecera 0x78 0x01 y Adler-32 al final.
    let mut z = vec![0x78, 0x01];
    for (i, datos) in crudo.chunks(65_535).enumerate() {
        let ultimo = (i + 1) * 65_535 >= crudo.len();
        z.push(u8::from(ultimo));
        z.extend_from_slice(&(datos.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(datos.len() as u16)).to_le_bytes());
        z.extend_from_slice(datos);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &crudo {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());

    let mut salida = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&ancho.to_be_bytes());
    ihdr.extend_from_slice(&alto.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    trozo(&mut salida, b"IHDR", &ihdr);
    trozo(&mut salida, b"IDAT", &z);
    trozo(&mut salida, b"IEND", &[]);
    salida
}

fn carpeta() -> PathBuf {
    // `CARGO_TARGET_DIR` puede estar en cualquier parte; se pregunta por el
    // sitio del ejecutable de la prueba, que siempre esta dentro de el.
    let mut p = std::env::current_exe().expect("el ejecutable de la prueba");
    p.pop();
    p.pop();
    p.pop();
    p.push("muestras-grupo-b");
    std::fs::create_dir_all(&p).expect("crear la carpeta de muestras");
    p
}

fn guardar(nombre: &str, pixeles: &[u8]) {
    let ruta = carpeta().join(nombre);
    std::fs::write(&ruta, png(ANCHO, ALTO, pixeles)).expect("escribir el PNG");
    println!("muestra: {}", ruta.display());
}

/// Cuanta tinta hay: pixeles que no son papel blanco.
fn manchados(v: &[u8]) -> usize {
    v.chunks(4)
        .filter(|p| p[0] != 255 || p[1] != 255 || p[2] != 255)
        .count()
}

#[test]
fn la_rejilla_se_ve_pero_no_tapa_el_dibujo_y_al_alejar_no_se_cierra() {
    let vista = (0.0, 0.0, ANCHO as f32, ALTO as f32);

    let cuadros = Cuadricula {
        estilo: EstiloRejilla::Lineas,
        paso: 20.0,
        imanta: false,
    };
    let mut v = lienzo();
    pintar(&mut v, &cuadros.ordenes(vista, 1.0));
    guardar("rejilla-cuadros.png", &v);
    let tinta_cuadros = manchados(&v);

    let puntos = Cuadricula {
        estilo: EstiloRejilla::Puntos,
        ..cuadros
    };
    let mut v = lienzo();
    pintar(&mut v, &puntos.ordenes(vista, 1.0));
    guardar("rejilla-puntos.png", &v);
    let tinta_puntos = manchados(&v);

    // **La de puntos dice lo mismo ensuciando la mitad**, que es su razon de
    // ser y no una diferencia de adorno.
    assert!(
        tinta_puntos * 4 < tinta_cuadros,
        "la de puntos ensucia {tinta_puntos} y la de cuadros {tinta_cuadros}: no compensa"
    );

    // Y ninguna de las dos puede tapar el papel. El limite es generoso a
    // proposito —una rejilla legible mancha alrededor de un diez por ciento—
    // y lo que caza es el fallo de verdad: que al alejarse se cierre.
    let total = (ANCHO * ALTO) as usize;
    assert!(
        tinta_cuadros * 5 < total,
        "la rejilla tapa {tinta_cuadros} de {total} pixeles"
    );

    // Al diez por ciento de aumento, el paso se dobla hasta que se vea: la
    // mancha NO puede crecer respecto al 100 %.
    let mut lejos = lienzo();
    pintar(&mut lejos, &cuadros.ordenes(vista, 0.1));
    guardar("rejilla-cuadros-alejada.png", &lejos);
    assert!(
        manchados(&lejos) <= tinta_cuadros,
        "alejandose se cerro en una malla: {} pixeles contra {tinta_cuadros}",
        manchados(&lejos)
    );
}

#[test]
fn el_lazo_se_ve_como_un_area_y_no_como_un_garabato() {
    // Un contorno abierto no se lee como «esto es lo que vas a coger». La
    // pista se cierra sola y lleva un velo tenue por dentro; las dos cosas
    // juntas son lo que convierte un trazo en una zona.
    let mut l = Lazo::empezar(Punto2::nuevo(60.0, 40.0));
    for p in [
        (200.0, 30.0),
        (340.0, 70.0),
        (300.0, 180.0),
        (150.0, 190.0),
        (70.0, 120.0),
    ] {
        l.mover(Punto2::nuevo(p.0, p.1));
    }
    let mut v = lienzo();
    let mut ordenes = Vec::new();
    ordenes.extend(l.relleno());
    ordenes.push(l.orden(1.0));
    pintar(&mut v, &ordenes);
    guardar("lazo.png", &v);

    // El velo tiene que cubrir de verdad el interior: sin el, el lazo son
    // cinco rayas sueltas.
    let dentro = manchados(&v);
    assert!(
        dentro > 15_000,
        "el interior del lazo apenas se ve: {dentro} pixeles"
    );
}

#[test]
fn las_seis_marcas_del_iman_se_distinguen_de_un_vistazo() {
    // Aparecen a un par de pixeles unas de otras cuando una raya toca una
    // esquina. Si dos se parecen, la queja «engancha donde no quiero» no se
    // puede ni diagnosticar.
    let tipos = [
        TipoAnclaje::Esquina,
        TipoAnclaje::Medio,
        TipoAnclaje::Centro,
        TipoAnclaje::Interseccion,
        TipoAnclaje::Eje,
        TipoAnclaje::Borde,
    ];
    let mut v = lienzo();
    let mut huellas: Vec<Vec<bool>> = Vec::new();
    for (i, t) in tipos.iter().enumerate() {
        let centro = Punto2::nuevo(50.0 + i as f32 * 60.0, 110.0);
        let a = Anclaje {
            punto: centro,
            tipo: *t,
            id: 1,
        };
        // A escala 3 para que la forma se lea en el PNG; en pantalla la marca
        // mide nueve pixeles.
        let orden = pista(&a, 1.0 / 3.0);
        // Cada marca se pinta ademas sola y **en el mismo sitio**, para poder
        // comparar siluetas y no solo cuanta tinta gastan. Contar pixeles no
        // vale: un aspa y una cruz gastan exactamente los mismos y son dos
        // marcas distintas.
        let mut solo = lienzo();
        let centrada = pista(
            &Anclaje {
                punto: Punto2::nuevo(ANCHO as f32 / 2.0, ALTO as f32 / 2.0),
                tipo: *t,
                id: 1,
            },
            1.0 / 3.0,
        );
        pintar(&mut solo, &[centrada]);
        huellas.push(
            solo.chunks(4)
                .map(|p| p[0] != 255 || p[1] != 255 || p[2] != 255)
                .collect(),
        );
        pintar(&mut v, &[orden]);
    }
    guardar("marcas-del-iman.png", &v);

    // Todas tienen que dejar huella —una marca invisible es un iman magico—
    // y ninguna pareja puede dejar la misma silueta.
    for (t, h) in tipos.iter().zip(&huellas) {
        assert!(
            h.iter().filter(|x| **x).count() > 8,
            "la marca de {t:?} no se ve"
        );
    }
    for i in 0..huellas.len() {
        for j in (i + 1)..huellas.len() {
            assert_ne!(
                huellas[i], huellas[j],
                "{:?} y {:?} se pintan con la misma silueta",
                tipos[i], tipos[j]
            );
        }
    }
}
