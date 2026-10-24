//! **Mide el «Exportar» del lector de PDF**, antes y despues: el PDF con
//! cada hoja pintada (lo de antes) contra el PDF original con la tinta en
//! vectores encima (`con_anotaciones`). Lee la tinta de
//! `<pdf>.pixpin-anotado/hoja-N.excalidraw`, como el lector, y dice si una
//! palabra se sigue encontrando en el resultado. Deja el resultado y la
//! primera hoja anotada pintada por Windows junto al PDF.
//!
//! `cargo run --release -p pixpin-pdf --example medir_exportar_anotado -- <pdf> <palabra> [espacios]`

use std::collections::HashMap;
use std::time::Instant;

use pixpin_motor2d::exportar::Hoja;
use pixpin_motor2d::pintado::{self, Orden};
use pixpin_pdf::con_anotaciones::{self, Anotaciones, Marcador};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let ruta = std::path::PathBuf::from(&args[0]);
    let palabra = args.get(1).cloned().unwrap_or_default();
    let espacios: u8 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let bytes = std::fs::read(&ruta).expect("leer");
    let doc = pixpin_pdf::Documento::abrir(&ruta).expect("abrir");
    let medidas = doc.medidas();
    let n = medidas.len();
    let altos: Vec<f32> = medidas
        .iter()
        .map(|(w, h)| 1400.0 * h / w.max(1.0))
        .collect();
    let carpeta = ruta.with_file_name(format!(
        "{}.pixpin-anotado",
        ruta.file_name().unwrap().to_string_lossy()
    ));
    let mut tinta: HashMap<usize, Vec<Orden>> = HashMap::new();
    for i in 0..n {
        let texto = std::fs::read_to_string(carpeta.join(format!("hoja-{}.excalidraw", i + 1)))
            .unwrap_or_default();
        if let Ok(e) = pixpin_motor2d::excalidraw::leer(&texto)
            .map(|l| pixpin_motor2d::excalidraw::a_escena(&l))
        {
            let o = pintado::ordenes_de_escena(&e);
            if !o.is_empty() {
                tinta.insert(i, o);
            }
        }
    }
    let m = 1400.0 * 0.75;
    let (izq, der) = (
        if espacios & 1 != 0 { m } else { 0.0 },
        if espacios & 2 != 0 { m } else { 0.0 },
    );
    let marcadores = [Marcador {
        titulo: "✅ Hoja 1".into(),
        pagina: 0,
        alto: 0.0,
    }];
    println!(
        "{}: {n} hojas, {} bytes, {} hojas con tinta",
        ruta.display(),
        bytes.len(),
        tinta.len()
    );

    // Ahora: el original con la tinta encima.
    let t = Instant::now();
    let segoe = pixpin_pdf::letra::del_sistema("Segoe UI");
    let de_la_hoja = |i: usize| tinta.get(&i).cloned();
    let nuevo = con_anotaciones::hacer(
        &bytes,
        &Anotaciones {
            tinta: &de_la_hoja,
            izquierda: izq,
            derecha: der,
            marcadores: &marcadores,
            imagenes: &|_| None,
            letra: segoe.as_ref(),
        },
    )
    .expect("con anotaciones");
    let tiempo_nuevo = t.elapsed();
    let salida = ruta.with_extension("anotado.pdf");
    std::fs::write(&salida, &nuevo).unwrap();
    let hallada = (0..n)
        .filter(|&i| {
            pixpin_pdf::plano::de_bytes(&nuevo, i)
                .is_some_and(|p| p.textos.iter().any(|x| x.texto.contains(&palabra)))
        })
        .count();
    println!(
        "  con anotaciones: {} bytes (+{} sobre el original) en {:?}; '{palabra}' en {hallada} hojas",
        nuevo.len(),
        nuevo.len() - bytes.len(),
        tiempo_nuevo
    );
    // La primera hoja con tinta, pintada por Windows desde el resultado.
    if let Some(&i) = tinta.keys().min() {
        let otro = pixpin_pdf::Documento::abrir(&salida).expect("el resultado abre");
        let img = otro.renderizar(i as u32, 1200).expect("pinta");
        let png = salida.with_extension(format!("hoja-{}.png", i + 1));
        std::fs::write(&png, pixpin_codec::codificar_png(&img).unwrap()).unwrap();
        println!("  muestra: {}", png.display());
    }

    // Antes: cada hoja pintada a 1400 con la tinta encima.
    if std::env::var_os("SIN_ANTES").is_none() {
        let t = Instant::now();
        let hojas: Vec<Hoja> = (0..n)
            .map(|i| {
                let mut ordenes = vec![Orden::Imagen {
                    id_objeto: i as u64 + 1,
                    x: 0.0,
                    y: 0.0,
                    ancho: 1400.0,
                    alto: altos[i],
                    opacidad: 1.0,
                    recorte: None,
                    angulo: 0.0,
                }];
                ordenes.extend(tinta.get(&i).cloned().unwrap_or_default());
                Hoja {
                    nombre: String::new(),
                    caja: (-izq, 0.0, 1400.0 + der, altos[i]),
                    ordenes,
                    marcos: Vec::new(),
                    granos: Vec::new(),
                    grafitos: Vec::new(),
                }
            })
            .collect();
        let viejo = pixpin_pdf::escribir::de_hojas_con_indice(
            &hojas,
            None,
            &|id| {
                let img = doc.renderizar((id - 1) as u32, 1400).ok()?;
                Some(pixpin_pdf::escribir::Pixeles {
                    ancho: img.ancho,
                    alto: img.alto,
                    rgba: img.pixeles,
                })
            },
            None,
            &[],
        )
        .expect("pintado");
        let hallada = (0..n)
            .filter(|&i| {
                pixpin_pdf::plano::de_bytes(&viejo, i)
                    .is_some_and(|p| p.textos.iter().any(|x| x.texto.contains(&palabra)))
            })
            .count();
        println!(
            "  pintado (antes): {} bytes en {:?}; '{palabra}' en {hallada} hojas",
            viejo.len(),
            t.elapsed()
        );
    }
}
