//! **Mide la exportacion de un PDF anotado**: cuantas hojas se leen como
//! lineas (la pagina web sin fotos), cuantos textos salen y si una palabra se
//! encuentra entera en un solo rotulo (lo que busca Ctrl+F del navegador).
//!
//! `cargo run --release -p pixpin-pdf --example medir_anotado -- <pdf> [palabra]`

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let ruta = std::path::PathBuf::from(&args[0]);
    let palabra = args.get(1).cloned().unwrap_or_default();
    let bytes = std::fs::read(&ruta).expect("leer");
    let n = pixpin_pdf::union::contar_paginas(&bytes).unwrap_or(0) as usize;
    if let Ok(v) = std::env::var("VOLCAR") {
        volcar(&bytes, v.parse().unwrap());
        return;
    }
    println!("{}: {} bytes, {n} paginas", ruta.display(), bytes.len());
    let t = std::time::Instant::now();
    let (mut lineas, mut textos, mut sin, mut hallada) = (0, 0, 0, 0);
    let cuales: Vec<usize> = (0..n).collect();
    pixpin_pdf::plano::con_cada(&bytes, &cuales, |i, p| {
        let Some(p) = p else {
            println!("  hoja {i}: no se lee");
            return;
        };
        if p.se_manda_como_lineas() {
            lineas += 1;
        } else {
            sin += 1;
            if sin < 6 {
                println!(
                    "  hoja {i}: foto (sin entender {}, puntos {}, textos {}, cortado {})",
                    p.sin_entender,
                    p.puntos(),
                    p.textos.len(),
                    p.cortado
                );
            }
        }
        textos += p.textos.len();
        if !palabra.is_empty() && p.textos.iter().any(|x| x.texto.contains(&palabra)) {
            hallada += 1;
        }
    });
    println!(
        "  {lineas} como lineas, {sin} como foto, {textos} textos, '{palabra}' entera en {hallada} hojas; {:?}",
        t.elapsed()
    );
    let t = std::time::Instant::now();
    let web: usize = pixpin_pdf::plano_web::de_paginas(&bytes, &cuales, 800.0)
        .iter()
        .flatten()
        .map(String::len)
        .sum();
    println!("  paquetes de la web: {} bytes en {:?}", web, t.elapsed());
}

#[allow(dead_code)]
fn volcar(bytes: &[u8], i: usize) {
    let p = pixpin_pdf::plano::de_bytes(bytes, i).expect("hoja");
    for t in p.textos.iter().take(60) {
        println!(
            "    [{:.1},{:.1} a={:.2} w={:.2}] {:?}",
            t.x, t.y, t.a, t.ancho, t.texto
        );
    }
}
