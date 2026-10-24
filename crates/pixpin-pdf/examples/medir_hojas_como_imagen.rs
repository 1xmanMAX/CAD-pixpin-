//! **Cuanto pesa una hoja de PDF como imagen** para la pagina web con el
//! interruptor «Texto buscable» quitado (Android v0.98.1 la manda en WebP;
//! Windows no trae codificador WebP y el del crate `image` solo hace WebP sin
//! perdida). Compara JPEG a 72 y 82 con WebP sin perdida, y lo que tarda.
//!
//! `cargo run -p pixpin-pdf --release --example medir_hojas_como_imagen -- <pdf> [hojas] [ancho]`

use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let ruta = std::path::PathBuf::from(args.next().expect("falta el PDF"));
    let hojas: u32 = args.next().and_then(|a| a.parse().ok()).unwrap_or(3);
    let ancho: u32 = args.next().and_then(|a| a.parse().ok()).unwrap_or(1600);
    let doc = pixpin_pdf::Documento::abrir(&ruta).expect("abre");
    let tmp = std::env::temp_dir().join("pixpin-medir-hoja.webp");
    let (mut j72, mut j82, mut wl) = (0usize, 0usize, 0usize);
    let (mut tj, mut tw) = (0u128, 0u128);
    let n = hojas.min(doc.medidas().len() as u32);
    for i in 0..n {
        let img = doc.renderizar(i, ancho).expect("pinta");
        let t = Instant::now();
        j72 += pixpin_codec::imagen::codificar_jpg(&img, 72).unwrap().len();
        tj += t.elapsed().as_millis();
        j82 += pixpin_codec::imagen::codificar_jpg(&img, 82).unwrap().len();
        let t = Instant::now();
        pixpin_codec::imagen::guardar(&img, &tmp, pixpin_codec::imagen::FormatoImagen::Webp)
            .unwrap();
        tw += t.elapsed().as_millis();
        wl += std::fs::metadata(&tmp).unwrap().len() as usize;
    }
    let _ = std::fs::remove_file(&tmp);
    println!(
        "{} ({n} hojas a {ancho}px): JPEG72 {} KB ({} ms), JPEG82 {} KB, WebP sin perdida {} KB ({} ms)",
        ruta.display(),
        j72 / 1024,
        tj,
        j82 / 1024,
        wl / 1024,
        tw
    );
}
