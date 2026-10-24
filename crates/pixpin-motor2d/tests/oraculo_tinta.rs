//! La tinta de Rust contra la de Excalidraw ejecutada de verdad.
//!
//! `tests/oraculo/tinta.json` lo escribe `herramientas/oraculo-excalidraw`
//! con las funciones originales. Si esto se pone rojo tras regenerar, es que
//! Excalidraw cambio algo que aun no hemos portado; si se pone rojo sin
//! regenerar, es que el porte se rompio.

use pixpin_motor2d::tinta::freehand::{self, Entrada, Opciones};
use serde::Deserialize;

const TOLERANCIA: f64 = 0.01;

#[derive(Deserialize)]
struct Fichero {
    casos: Vec<Caso>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct Caso {
    nombre: String,
    puntos: Vec<[f64; 2]>,
    presiones: Vec<f64>,
    grosor: f64,
    variabilidad: String,
    streamline: f64,
    contorno: Vec<[f64; 2]>,
    svg: String,
}

fn casos(variabilidad: &str) -> Vec<Caso> {
    let texto = include_str!("oraculo/tinta.json");
    let f: Fichero = serde_json::from_str(texto).expect("tinta.json se lee");
    f.casos
        .into_iter()
        .filter(|c| c.variabilidad == variabilidad)
        .collect()
}

fn comparar(nombre: &str, esperado: &[[f64; 2]], obtenido: &[[f64; 2]]) {
    assert_eq!(
        esperado.len(),
        obtenido.len(),
        "{nombre}: {} puntos en Excalidraw, {} en Rust",
        esperado.len(),
        obtenido.len()
    );
    for (i, (e, o)) in esperado.iter().zip(obtenido).enumerate() {
        let d = (e[0] - o[0]).abs().max((e[1] - o[1]).abs());
        assert!(
            d <= TOLERANCIA,
            "{nombre}: el punto {i} difiere {d}: Excalidraw {e:?}, Rust {o:?}"
        );
    }
}

#[test]
fn la_pluma_variable_coincide_con_excalidraw_en_todos_los_casos() {
    let casos = casos("variable");
    assert_eq!(
        casos.len(),
        12,
        "el oraculo tiene que traer los 12 casos variables"
    );
    for c in casos {
        let simular = c.presiones.is_empty();
        let entrada: Vec<Entrada> = c
            .puntos
            .iter()
            .enumerate()
            .map(|(i, p)| Entrada {
                x: p[0],
                y: p[1],
                presion: if simular { None } else { Some(c.presiones[i]) },
            })
            .collect();
        let o = Opciones {
            size: c.grosor * 4.25,
            thinning: 0.6,
            smoothing: 0.5,
            streamline: c.streamline,
            simular_presion: simular,
            last: true,
            easing: freehand::seno,
        };
        comparar(&c.nombre, &c.contorno, &freehand::contorno(&entrada, &o));
    }
}

#[test]
fn la_pluma_constante_coincide_con_excalidraw_en_todos_los_casos() {
    let casos = casos("constant");
    assert_eq!(
        casos.len(),
        9,
        "el oraculo tiene que traer los 9 casos constantes"
    );
    for c in casos {
        let obtenido =
            pixpin_motor2d::tinta::laser::contorno(&c.puntos, c.grosor * 1.4, c.streamline);
        comparar(&c.nombre, &c.contorno, &obtenido);
    }
}
