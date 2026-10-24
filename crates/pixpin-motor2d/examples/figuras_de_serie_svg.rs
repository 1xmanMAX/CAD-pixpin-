//! Escribe cada figura de serie como SVG (y una pagina con todas) en la
//! carpeta que se le pase, para mirarlas sin abrir la app. Con
//! `--alargadas`, en vez del catalogo, unas cuantas alargadas y partidas.
//!
//! `cargo run -p pixpin-motor2d --example figuras_de_serie_svg -- <carpeta> [--alargadas]`

use pixpin_motor2d::biblioteca::FiguraGuardada;
use pixpin_motor2d::elemento::ColorRgba;
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::exportar::{self, Alcance};
use pixpin_motor2d::exportar_svg::{self, OpcionesSvg};
use pixpin_motor2d::figuras_de_serie::{self, Intervalo};

fn alargadas() -> Vec<FiguraGuardada> {
    let iv = |d, h, k| Intervalo {
        desde: Some(d),
        hasta: Some(h),
        de_a: k,
        ..Default::default()
    };
    let mut v = Vec::new();
    let mut con =
        |clave: &str, i: Intervalo| v.extend(figuras_de_serie::alargar(clave, &i).unwrap());
    con("lista-de-tareas", iv(1, 60, None));
    con("lista-numerada", iv(1, 40, Some(10)));
    con(
        "cronograma",
        Intervalo {
            semanas: Some((13, 24)),
            ..iv(19, 36, None)
        },
    );
    con(
        "cronograma",
        Intervalo {
            semanas: Some((1, 26)),
            ..Default::default()
        },
    );
    con(
        "planificador-mensual",
        Intervalo {
            mes: Some((2026, 11)),
            meses: Some(2),
            ..Default::default()
        },
    );
    con(
        "habitos",
        Intervalo {
            mes: Some((2028, 2)),
            ..Default::default()
        },
    );
    con("habitos", iv(1, 15, None));
    con("storyboard-4", iv(1, 12, Some(4)));
    con("storyboard-6", iv(7, 18, None));
    con("asistencia", iv(1, 45, None));
    con("metas", iv(1, 30, None));
    con("receta", iv(1, 24, None));
    con("itinerario", iv(1, 5, None));
    con("planificador-diario", iv(5, 23, None));
    con("semestre", iv(7, 23, None));
    v
}

fn main() {
    // `--lienzo <fichero>`: la lista de tareas del 1 al 40 de 10 en 10, en
    // fila, como la deja el asistente, para abrirla en la app.
    if let Some(i) = std::env::args().position(|a| a == "--lienzo") {
        let destino = std::env::args().nth(i + 1).expect("fichero");
        let iv = Intervalo {
            desde: Some(1),
            hasta: Some(40),
            de_a: Some(10),
            ..Default::default()
        };
        let mut escena = Escena::nueva();
        let mut x = 0.0;
        for f in figuras_de_serie::alargar("lista-de-tareas", &iv).unwrap() {
            let (w, h) = f.tamano();
            pixpin_motor2d::biblioteca::estampar_en_escena(
                &mut escena,
                &f,
                pixpin_motor2d::vector::Punto2::nuevo(x + w / 2.0, h / 2.0),
            );
            x += w + 60.0;
        }
        let vacio =
            pixpin_motor2d::excalidraw::leer(r#"{"type":"excalidraw","version":2,"elements":[]}"#)
                .unwrap();
        std::fs::write(
            destino,
            pixpin_motor2d::excalidraw::escribir(&pixpin_motor2d::excalidraw::con_escena(
                &vacio, &escena,
            )),
        )
        .unwrap();
        return;
    }
    let dir = std::path::PathBuf::from(std::env::args().nth(1).expect("carpeta"));
    let figuras = if std::env::args().any(|a| a == "--alargadas") {
        alargadas()
    } else {
        figuras_de_serie::catalogo()
    };
    std::fs::create_dir_all(&dir).unwrap();
    let mut pagina = String::from(
        "<html><body style='background:#2a2d34;font:12px sans-serif;color:#ddd;margin:0'><div style='display:flex;flex-wrap:wrap;gap:12px;padding:12px'>",
    );
    for (i, f) in figuras.iter().enumerate() {
        let mut escena = Escena::nueva();
        for e in &f.elementos {
            escena.anadir(e.clone());
        }
        let hojas = exportar::hojas(&escena, Alcance::Todo, &[], None);
        let Some(hoja) = hojas.first() else { continue };
        let svg = exportar_svg::svg(
            hoja,
            OpcionesSvg {
                fondo: Some(ColorRgba::opaco(1.0, 1.0, 1.0)),
                marcos: false,
            },
            &|_| None,
        );
        let nombre = format!("{i:03}-{}.svg", f.id);
        std::fs::write(dir.join(&nombre), &svg).unwrap();
        pagina.push_str(&format!(
            "<div style='width:260px'><div style='background:#fff;height:300px;display:flex;align-items:center;justify-content:center'><img src='{nombre}' style='max-width:250px;max-height:290px'></div><div>{} ({} piezas)</div></div>",
            f.nombre,
            f.elementos.len()
        ));
    }
    pagina.push_str("</div></body></html>");
    std::fs::write(dir.join("todas.html"), pagina).unwrap();
}
