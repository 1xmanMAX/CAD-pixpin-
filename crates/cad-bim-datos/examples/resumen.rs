//! **El resumen de un modelo** en la consola: `cargo run --release -p
//! cad-bim-datos --example resumen -- modelo.rvt`.

fn main() {
    let Some(ruta) = std::env::args().nth(1) else {
        eprintln!("uso: resumen <modelo.ifc|.rvt>");
        return;
    };
    let t = std::time::Instant::now();
    let m = match cad_bim_datos::leer(std::path::Path::new(&ruta), Default::default(), &mut |a| {
        eprintln!("… {a}")
    }) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {e}");
            return;
        }
    };
    println!("leido en {:.2} s", t.elapsed().as_secs_f64());
    let a = &m.archivo;
    println!(
        "{} | {:?} | {:?} | por {:?} | {:?} | central {:?} | miniatura {:?} bytes",
        a.formato.nombre(),
        a.version,
        a.compilacion,
        a.guardado_por,
        a.trabajo_compartido,
        a.ruta_central,
        a.vista_previa.as_ref().map(Vec::len)
    );
    println!("proyecto {:?}", m.proyecto);
    println!("niveles {}:", m.niveles.len());
    for n in &m.niveles {
        println!("  {} {:?} ({} elementos)", n.nombre, n.cota, n.elementos);
    }
    println!("categorias:");
    for c in &m.categorias {
        println!("  {} {} {:?}", c.nombre, c.elementos, c.clases);
    }
    println!(
        "elementos {} | materiales {} | espacios {} | tipos {}",
        m.elementos.len(),
        m.materiales.len(),
        m.espacios.len(),
        m.tipos.len()
    );
    let props: usize = m
        .elementos
        .iter()
        .flat_map(|e| &e.conjuntos)
        .map(|c| c.propiedades.len())
        .sum();
    println!("propiedades en total {props}");
    for e in m.elementos.iter().take(2) {
        println!("{e:#?}");
    }
    for s in m.espacios.iter().take(3) {
        println!("espacio {s:?}");
    }
    for a in &m.avisos {
        println!("aviso: {a}");
    }
}
