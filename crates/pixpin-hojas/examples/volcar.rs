//! Vuelca las primeras filas de una hoja como texto (lo que se veria en
//! cada celda) y mide abrir y cargar.
//!
//! ```text
//! cargo run --release -p pixpin-hojas --example volcar -- <libro.xlsx> [hoja] [filas]
//! ```

use std::time::Instant;

use pixpin_hojas::{Libro, Regional, nombre_columna};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let Some(ruta) = a.first() else {
        eprintln!("uso: volcar <libro.xlsx> [hoja] [filas]");
        std::process::exit(2);
    };
    let t = Instant::now();
    let libro = Libro::abrir(ruta.as_ref(), Regional::es_pe()).expect("abrir");
    let t_abrir = t.elapsed().as_secs_f64() * 1000.0;
    let i = a
        .get(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(libro.hoja_activa);
    let filas: u32 = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(30);
    for (k, h) in libro.hojas.iter().enumerate() {
        println!(
            "hoja {k}: {} ({:?}, {:?}) {}",
            h.nombre, h.visibilidad, h.tipo, h.ruta
        );
    }
    let t = Instant::now();
    let mut hoja = libro.abrir_hoja(i);
    hoja.cargar_hasta_fila(filas);
    let t_primera = t.elapsed().as_secs_f64() * 1000.0;
    for f in hoja.filas_entre(0, filas) {
        for c in hoja.celdas_de(f) {
            if let Some(x) = libro.contenido(&hoja, c) {
                println!(
                    "{}{}: {:?} -> «{}» ({:?}, xf {}, formato {:?})",
                    nombre_columna(c.col),
                    f.indice + 1,
                    c.valor,
                    x.mostrado.texto,
                    x.horizontal,
                    c.xf,
                    libro.estilos.xf(c.xf).formato.es_fecha
                );
                if std::env::var_os("VOLCAR_ESTILOS").is_some() {
                    let xf = libro.estilos.xf(c.xf);
                    println!(
                        "    fuente {:?}\n    relleno {:?}\n    borde {:?}\n    alineacion {:?}",
                        libro.estilos.fuente(xf),
                        libro.estilos.relleno(xf),
                        libro.estilos.borde(xf),
                        xf.alineacion
                    );
                }
            }
        }
    }
    let t = Instant::now();
    hoja.cargar_todo();
    let t_todo = t.elapsed().as_secs_f64() * 1000.0;
    println!("vista: {:?}", hoja.vista());
    println!(
        "combinadas: {}, hipervinculos: {}, dibujos: {}",
        hoja.cola.combinadas.len(),
        hoja.cola.hipervinculos.len(),
        hoja.cola.dibujos.len()
    );
    println!(
        "abrir {t_abrir:.1} ms, primeras {filas} filas {t_primera:.1} ms, resto {t_todo:.1} ms; {} filas, {} celdas",
        hoja.filas.len(),
        hoja.celdas.len()
    );
}
