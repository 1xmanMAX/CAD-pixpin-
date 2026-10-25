//! **Un plano de prueba** para usar la app a mano: habitaciones con muros,
//! piezas de la biblioteca, sombreados, cotas, textos y, si se pide, muchos
//! objetos de relleno para medir el rendimiento.
//!
//! ```text
//! cargo run -p cad-nucleo --release --example plano_de_prueba -- prueba.dwg 100000
//! ```

use cad_nucleo::especialidad::{arco, circulo, linea, polilinea, texto};
use cad_nucleo::{Dibujo, Sesion};

fn main() {
    let mut args = std::env::args().skip(1);
    let ruta = args.next().unwrap_or_else(|| "prueba.dxf".into());
    let relleno: usize = args.next().and_then(|n| n.parse().ok()).unwrap_or(0);
    let mut s = Sesion::nueva(Dibujo::nuevo());
    // Una fila de habitaciones de 4 x 3 m, con lo de cada una.
    for i in 0..4 {
        let x = i as f64 * 4500.0;
        for o in [
            "rec".to_string(),
            format!("{x},0"),
            format!("{},3000", x + 4000.0),
        ] {
            s.escribir(&o);
        }
        for o in ["bib", "inodoro", &format!("{},2900", x + 600.0), "", ""] {
            s.escribir(o);
        }
        s.escape();
        for o in ["so", &format!("{},1500", x + 2000.0)] {
            s.escribir(o);
        }
        s.escape();
        for o in [
            "dli".to_string(),
            format!("{x},0"),
            format!("{},0", x + 4000.0),
            format!("{},-600", x + 2000.0),
        ] {
            s.escribir(&o);
        }
        s.escape();
        for o in [
            "t".to_string(),
            format!("{},1200", x + 1200.0),
            "200".into(),
            format!("Dormitorio {}", i + 1),
        ] {
            s.escribir(&o);
        }
        s.escape();
    }
    // Un texto de parrafo.
    for o in [
        "mt",
        "0,4000",
        "3000,3500",
        "Notas generales",
        "Muros de 15 cm",
        "",
    ] {
        s.escribir(o);
    }
    s.escape();
    // El relleno, en una cuadricula debajo.
    if relleno > 0 {
        let col = 500usize;
        s.dibujo.editar("RELLENO", |t| {
            for i in 0..relleno {
                let (x, y) = ((i % col) as f64 * 100.0, -5000.0 - (i / col) as f64 * 100.0);
                let e = match i % 10 {
                    0..=5 => linea([x, y], [x + 70.0, y + (i % 7) as f64 * 10.0]),
                    6 => circulo([x + 50.0, y + 50.0], 30.0),
                    7 => arco([x + 50.0, y + 50.0], 40.0, 0.3, 2.5),
                    8 => polilinea(
                        &[[x, y], [x + 60.0, y], [x + 60.0, y + 40.0], [x, y + 40.0]],
                        true,
                    ),
                    _ => texto(&format!("T{i}"), [x, y], 20.0, 0.0, false),
                };
                t.anadir(e);
            }
        });
    }
    for l in s.registro.iter().filter(|l| !l.starts_with("Orden")) {
        println!("{l}");
    }
    let n = s.dibujo.doc.model_space_entities().count();
    match s.dibujo.guardar_como(std::path::Path::new(&ruta)) {
        Ok(()) => println!("{n} objetos en {ruta}"),
        Err(e) => eprintln!("No se pudo guardar: {e}"),
    }
}
