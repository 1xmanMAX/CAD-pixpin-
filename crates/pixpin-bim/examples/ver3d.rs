//! Prueba a mano: `cargo run -p pixpin-bim --release --example ver3d -- <modelo.ifc|.px3d> [segundos]`.
fn main() {
    let ruta = std::path::PathBuf::from(std::env::args().nth(1).expect("falta el modelo"));
    let segundos: u64 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let (tx, rx) = std::sync::mpsc::channel();
    let r2 = ruta.clone();
    std::thread::spawn(move || {
        let m = if r2.extension().is_some_and(|e| e == "px3d") {
            std::fs::read(&r2)
                .ok()
                .and_then(|b| pixpin_cad::modelo3d::Modelo3d::de_bytes(&b))
                .ok_or_else(|| "cache rota".to_string())
        } else {
            pixpin_bim::convertir_fichero(&r2)
        };
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
    if let Err(e) = pixpin_cad::ventana3d::ver(&titulo, rx, Default::default()) {
        eprintln!("error: {e}");
    }
}
