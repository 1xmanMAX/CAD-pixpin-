//! **La cota de una foto medida en el movil, exportada a PDF** y abierta con
//! el lector de Windows: el numero dice centimetros, va a lo largo de la raya
//! y lleva su halo, como en el telefono (queja del usuario, 27-sep-2026).
//!
//! El lienzo es inventado, con la misma forma que los del movil: la escala
//! arriba del todo y las cotas como `pixpin-measure` con su `fontSize`.
//! Deja la muestra en `target/muestras-de-cota/` para mirarla.

use pixpin_motor2d::exportar::{self, Alcance};
use pixpin_motor2d::pintado::Orden;
use pixpin_pdf::escribir::de_hojas;

/// Un plano sobre fondo oscuro (la foto), una cota vertical larga de 300 px
/// que con 5/300 cm por pixel mide 5 cm, una corta como las del usuario y
/// una linea redonda larga que en el movil sale recta.
const PLANO: &str = r##"{"type":"excalidraw","backgroundColor":"#ffffff",
    "escala":{"decimales":2,"unidad":"cm","unidadesPorPixel":0.016666666666666666},
    "elements":[
      {"type":"rectangle","id":"foto","x":0,"y":0,"width":240,"height":400,
       "strokeColor":"#1e1e1e","backgroundColor":"#1e1e1e","fillStyle":"solid",
       "strokeWidth":1,"roughness":0,"opacity":100,"seed":1},
      {"type":"pixpin-measure","id":"larga","x":60,"y":50,"width":0,"height":300,
       "points":[{"x":0.0,"y":0.0},{"x":0.0,"y":300.0}],"strokeColor":"#0edeff",
       "strokeWidth":2.5,"roughness":1,"opacity":100,"seed":42,"fontFamily":5,"fontSize":20.0},
      {"type":"pixpin-measure","id":"corta","x":170,"y":180,"width":0.6955,"height":36.6305,
       "points":[{"x":0.0,"y":0.0},{"x":-0.6955,"y":36.6305}],"strokeColor":"#0edeff",
       "strokeWidth":2.5,"roughness":1,"opacity":100,"seed":7,"fontFamily":5,"fontSize":9.0},
      {"type":"line","id":"linea","x":120,"y":10,"width":3,"height":380,"roundness":{"type":3},
       "points":[{"x":0.0,"y":0.0},{"x":3.0,"y":380.0}],"strokeColor":"#ff6600",
       "strokeWidth":2.5,"roughness":1,"opacity":100,"seed":950731993}
    ]}"##;

fn escena() -> pixpin_motor2d::Escena {
    pixpin_motor2d::excalidraw::a_escena(&pixpin_motor2d::excalidraw::leer(PLANO).expect("se lee"))
}

fn carpeta() -> std::path::PathBuf {
    let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/muestras-de-cota");
    std::fs::create_dir_all(&d).expect("carpeta de muestras");
    d
}

#[test]
fn las_cotas_del_plano_salen_como_rotulos_girados_con_halo_en_centimetros() {
    let hojas = exportar::hojas(&escena(), Alcance::Todo, &[], None);
    let rotulos: Vec<(String, f32, [f32; 3])> = hojas[0]
        .ordenes
        .iter()
        .filter_map(|o| match o {
            Orden::Rotulo {
                texto,
                angulo,
                halo,
                ..
            } => Some((
                texto.clone(),
                angulo.to_degrees().rem_euclid(360.0),
                [halo.r, halo.g, halo.b],
            )),
            _ => None,
        })
        .collect();
    assert_eq!(rotulos.len(), 2, "{rotulos:?}");
    assert_eq!(rotulos[0].0, "5,00 cm · -90°");
    assert!(
        (rotulos[0].1 - 90.0).abs() < 0.1,
        "a lo largo de la vertical: {}",
        rotulos[0].1
    );
    assert!(
        (rotulos[1].1 - 271.09).abs() < 0.1,
        "volteada para no ir boca abajo: {}",
        rotulos[1].1
    );
    // La tinta adaptada al papel blanco es oscura: halo blanco.
    assert_eq!(rotulos[0].2, [1.0, 1.0, 1.0]);
}

#[test]
fn en_el_pdf_el_numero_lleva_su_halo_trazado_debajo_y_va_girado() {
    let hojas = exportar::hojas(&escena(), Alcance::Todo, &[], None);
    let bytes = de_hojas(
        &hojas,
        Some(pixpin_motor2d::ColorRgba::opaco(1.0, 1.0, 1.0)),
        &|_| None,
    )
    .expect("una hoja");
    let ruta = std::env::temp_dir().join(format!("pixpin-cota-{}.pdf", std::process::id()));
    std::fs::write(&ruta, &bytes).expect("escribir");
    let doc = pixpin_pdf::Documento::abrir(&ruta).expect("Windows lo abre");
    let p = doc.renderizar(0, 900).expect("pagina");
    let png = exportar::png_rgba(p.ancho, p.alto, &p.pixeles);
    std::fs::write(carpeta().join("cota_pdf.png"), png).expect("muestra");

    let px = |x: u32, y: u32| {
        let i = ((y * p.ancho + x) * 4) as usize;
        [p.pixeles[i], p.pixeles[i + 1], p.pixeles[i + 2]]
    };
    let caja_de = |cumple: &dyn Fn([u8; 3]) -> bool, x0: u32, x1: u32, y0: u32, y1: u32| {
        let mut c: Option<(u32, u32, u32, u32)> = None;
        for y in y0..y1 {
            for x in x0..x1 {
                if cumple(px(x, y)) {
                    c = Some(match c {
                        None => (x, y, x, y),
                        Some((a, b, cc, d)) => (a.min(x), b.min(y), cc.max(x), d.max(y)),
                    });
                }
            }
        }
        c
    };
    // La foto oscura: donde esta en la pagina.
    let (fx0, fy0, fx1, fy1) = caja_de(
        &|c| c[0] < 60 && c[1] < 60 && c[2] < 60,
        0,
        p.ancho,
        0,
        p.alto,
    )
    .expect("la foto");
    // Dentro de ella, blanco solo puede ser el halo de un numero: sin el, el
    // numero se perderia sobre lo de debajo. Se mira la mitad izquierda, la
    // de la cota larga: su halo es un renglon PUESTO DE PIE, mas alto que
    // ancho, porque el numero va a lo largo de la raya vertical.
    let blanco = |c: [u8; 3]| c[0] > 230 && c[1] > 230 && c[2] > 230;
    let (hx0, hy0, hx1, hy1) = caja_de(&blanco, fx0 + 2, (fx0 + fx1) / 2, fy0 + 2, fy1 - 2)
        .expect("hay halo sobre la foto");
    let (ancho, alto) = (hx1 - hx0, hy1 - hy0);
    assert!(
        alto > ancho * 2,
        "el numero va a lo largo de la cota: {ancho} x {alto}"
    );
}
