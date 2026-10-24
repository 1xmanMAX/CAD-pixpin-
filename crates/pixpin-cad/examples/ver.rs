//! Prueba a mano del visor: `cargo run -p pixpin-cad --release --example ver -- <plano.dwg> [segundos]`.
fn main() {
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .try_init();
    let ruta = std::path::PathBuf::from(std::env::args().nth(1).expect("falta el plano"));
    let segundos: u64 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let (tx, rx) = std::sync::mpsc::channel();
    let r2 = ruta.clone();
    std::thread::spawn(move || {
        let t = std::time::Instant::now();
        let m = pixpin_cad::convertir::convertir_fichero(&r2).map(|(m, _)| m);
        eprintln!("convertido en {:?}", t.elapsed());
        let _ = tx.send(m);
    });
    if segundos > 0 {
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(segundos));
            std::process::exit(0);
        });
    }
    let titulo = ruta
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    if let Err(e) = pixpin_cad::ventana::ver(&titulo, rx, Default::default(), Default::default()) {
        eprintln!("error: {e}");
    }
}
