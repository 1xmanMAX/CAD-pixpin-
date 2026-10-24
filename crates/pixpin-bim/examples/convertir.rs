//! cargo run --release -p pixpin-bim --example convertir -- modelo.ifc [salida.px3d]
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let t = std::time::Instant::now();
    match pixpin_bim::convertir_fichero(std::path::Path::new(&a[1])) {
        Ok(m) => {
            println!(
                "{} ms · {} vertices · {} triangulos opacos · {} transparentes · {} aristas · {} elementos · caja {:?} · origen {:?}",
                t.elapsed().as_millis(),
                m.vertices.len(),
                m.opacos.len() / 3,
                m.transparentes.len() / 3,
                m.aristas.len() / 2,
                m.elementos.len(),
                m.caja,
                m.origen
            );
            let mut tipos: std::collections::BTreeMap<&str, usize> = Default::default();
            for e in &m.elementos {
                *tipos.entry(&e.tipo).or_default() += 1;
            }
            println!("{tipos:?}");
            for (k, n) in m.niveles.iter().enumerate() {
                let de: Vec<_> = m
                    .elementos
                    .iter()
                    .filter(|e| e.nivel == Some(k as u32))
                    .collect();
                let v: f32 = de.iter().map(|e| e.medidas.volumen).sum();
                println!(
                    "nivel {} ({:.2}): {} elementos, {v:.2} m3",
                    n.nombre,
                    n.cota,
                    de.len()
                );
            }
            let sin = m.elementos.iter().filter(|e| e.nivel.is_none()).count();
            let cerrados = m.elementos.iter().filter(|e| e.medidas.cerrado).count();
            println!(
                "sin nivel: {sin} · cerrados: {cerrados} de {}",
                m.elementos.len()
            );
            for e in m.elementos.iter().take(4) {
                println!("  {} {} {:?}", e.tipo, e.nombre, e.medidas);
            }
            if let Some(s) = a.get(2) {
                std::fs::write(s, m.a_bytes()).unwrap();
            }
        }
        Err(e) => println!("error: {e}"),
    }
}
