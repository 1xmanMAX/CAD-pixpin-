//! Medir un plano: `cargo run -p pixpin-cad --release --example medir -- <plano.dwg>`.
//! Dice su caja, la de lo que importa y los tramos que quedan lejos.
fn main() {
    let ruta = std::path::PathBuf::from(std::env::args().nth(1).expect("falta el plano"));
    let t = std::time::Instant::now();
    let (m, c) = pixpin_cad::convertir::convertir_fichero(&ruta).expect("no se pudo leer");
    println!(
        "convertido en {:?} · entidades {} · vertices {}",
        t.elapsed(),
        c.entidades,
        m.vertices.len()
    );
    println!("caja {:?} · origen {:?}", m.caja, m.origen);
    println!("caja util {:?}", m.caja_util());
    println!("por tipo {:?}", c.vertices_por_tipo);
    let u = m.caja_util();
    let (ux, uy) = ((u[2] - u[0]).max(1e-6), (u[3] - u[1]).max(1e-6));
    let lejos: Vec<_> = m
        .tramos_lineas
        .iter()
        .chain(m.tramos_triangulos.iter())
        .filter(|t| {
            t.caja[0] < u[0] - ux
                || t.caja[2] > u[2] + ux
                || t.caja[1] < u[1] - uy
                || t.caja[3] > u[3] + uy
        })
        .take(10)
        .collect();
    println!("tramos lejos de lo util: {}", lejos.len());
    for t in lejos {
        println!("  {:?} tamano {}", t.caja, t.tamano);
    }
}
