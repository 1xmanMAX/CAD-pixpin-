//! Mide por separado descomprimir la hoja y leerla, para saber donde se va
//! el tiempo de la carga completa.
//!
//! ```text
//! cargo run --release -p pixpin-hojas --example medir_carga -- <libro.xlsx> [hoja]
//! ```

use std::io::Read;
use std::time::Instant;

use pixpin_hojas::{Libro, Regional};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ruta = a.first().expect("libro");
    let libro = Libro::abrir(ruta.as_ref(), Regional::es_pe()).expect("abrir");
    let i = a
        .get(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(libro.hoja_activa);
    let bytes = std::fs::read(ruta).expect("leer");
    let p = pixpin_hojas::paquete::Paquete::nuevo(bytes.into()).expect("zip");
    let t = Instant::now();
    let mut v = Vec::new();
    p.lector(&libro.hojas[i].ruta)
        .expect("lector")
        .read_to_end(&mut v)
        .expect("inflar");
    let t_inflar = t.elapsed().as_secs_f64() * 1000.0;
    // Solo el lector de XML, sin hacer nada con lo que lee.
    let t = Instant::now();
    let mut pos = 0;
    let mut n = 0u64;
    while let pixpin_hojas::xml::Paso::Evento(_) = pixpin_hojas::xml::siguiente(&v, &mut pos, true)
    {
        n += 1;
    }
    let t_xml = t.elapsed().as_secs_f64() * 1000.0;
    let t = Instant::now();
    let mut h = libro.abrir_hoja(i);
    h.cargar_todo();
    let t_todo = t.elapsed().as_secs_f64() * 1000.0;
    println!(
        "XML {} MB: inflar {t_inflar:.0} ms, recorrer {n} eventos {t_xml:.0} ms, carga completa {t_todo:.0} ms ({} celdas)",
        v.len() / (1024 * 1024),
        h.celdas.len()
    );
}
