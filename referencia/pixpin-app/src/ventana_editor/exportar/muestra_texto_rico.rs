//! **La columna «RESOLUCION» de una lamina, rehecha con texto rico** y
//! exportada a PNG por el mismo camino que el menu del editor (`a_imagen`),
//! al lado del original para compararlas a ojo (15-oct-2026).
//!
//! Necesita GPU: `PIXPIN_BANCO_TEXTO=<carpeta> cargo test -p pixpin --bin
//! pixpinmax muestra_resolucion_con_texto_rico -- --ignored --nocapture`.
//! El original se lee de `PIXPIN_LAMINA` (la lamina entera: se recorta la
//! columna de la derecha).

use pixpin_codec::ImagenRgba;
use pixpin_motor2d::elemento::{ColorRgba, Elemento, Figura};
use pixpin_motor2d::{Escena, texto_rico};

use super::Lienzo;

/// La columna, linea a linea, como la escribiria el asistente.
pub(crate) const RESOLUCION: &str = concat!(
    "Nos piden $m\\sphericalangle COD = \\color{rojo}{X}$\n",
    "•  *Se observa que* $\\overline{AB}$ *es diámetro:*\n",
    "\\centro $m\\sphericalangle AOB=90^\\circ$\n",
    "•  *Si* $\\color{rojo}{m\\sphericalangle AOC=a}$ *y* $\\color{rojo}{m\\sphericalangle BOD=b}$*:*\n",
    "\\centro $X=90^\\circ - \\color{rojo}{(a + b)}$\n",
    "•  *Además AOB es un cuadrante:*\n",
    "\\centro $\\color{verde}{m\\sphericalangle BPD=45^\\circ}$\n",
    "•  *Por ∢ inscrito:*\n",
    "\\centro $\\color{verde}{m\\overset{\\frown}{AC}=2a}$\n",
    "\\centro $\\color{verde}{m\\overset{\\frown}{BD}=2b}$\n",
    "•  *Por ∢ interior:*\n",
    "\\centro $\\color{#2f3fbf}{45^\\circ=\\frac{\\mathbf{2a+2b}}{\\mathbf{2}}}$\n",
    "\\centro $\\color{rojo}{a + b = 45^\\circ}$\n",
    "•  *Reemplazando en X:*\n",
    "\\centro $X=90^\\circ - \\color{rojo}{(a + b)}=90^\\circ- \\color{rojo}{45^\\circ}$\n",
    "\\centro $\\color{rojo}{\\therefore X = 45^\\circ}$",
);

fn texto_rico_en(fuente: &str, x: f32, y: f32, tam: f32, familia: &str) -> Elemento {
    let mut e = Elemento {
        figura: Figura::Texto {
            texto: String::new(),
            tam,
            familia: familia.into(),
        },
        x,
        y,
        trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
        ..Default::default()
    };
    texto_rico::poner_fuente(&mut e, fuente);
    e
}

/// La escena de la columna: el titulo, el cuerpo y, aparte, el «θ=90°» en
/// su recuadro (el de la columna de teoremas).
pub(crate) fn escena_de_la_resolucion() -> Escena {
    let mut escena = Escena::nueva();
    escena.fondo = ColorRgba::opaco(1.0, 1.0, 1.0);
    let familia = "Calibri";
    escena.anadir(texto_rico_en("RESOLUCIÓN:", 6.0, 0.0, 20.0, familia));
    escena.anadir(texto_rico_en(RESOLUCION, 6.0, 56.0, 20.0, familia));
    let mut caja = texto_rico_en(
        "$\\color{verde}{\\boxed{\\color{negro}{\\theta=90^\\circ}}}$",
        60.0,
        560.0,
        20.0,
        familia,
    );
    caja.extras.alineacion = None;
    escena.anadir(caja);
    escena
}

/// Pega dos imagenes una al lado de la otra (la segunda al alto de la
/// primera), sobre blanco.
fn lado_a_lado(a: &ImagenRgba, b: &ImagenRgba) -> ImagenRgba {
    let alto = a.alto.max(b.alto);
    let ancho = a.ancho + 12 + b.ancho;
    let mut px = vec![255u8; (ancho * alto * 4) as usize];
    for (img, x0) in [(a, 0u32), (b, a.ancho + 12)] {
        for y in 0..img.alto {
            for x in 0..img.ancho {
                let s = ((y * img.ancho + x) * 4) as usize;
                let d = ((y * ancho + x0 + x) * 4) as usize;
                // Sobre blanco (la exportada puede traer alfa).
                let al = img.pixeles[s + 3] as u32;
                for k in 0..3 {
                    px[d + k] = ((img.pixeles[s + k] as u32 * al + 255 * (255 - al)) / 255) as u8;
                }
                px[d + 3] = 255;
            }
        }
    }
    ImagenRgba {
        ancho,
        alto,
        pixeles: px,
    }
}

fn recortar(img: &ImagenRgba, (x0, y0, x1, y1): (u32, u32, u32, u32)) -> ImagenRgba {
    let (x1, y1) = (x1.min(img.ancho), y1.min(img.alto));
    let (w, h) = (x1 - x0, y1 - y0);
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    for y in y0..y1 {
        let s = ((y * img.ancho + x0) * 4) as usize;
        px.extend_from_slice(&img.pixeles[s..s + (w * 4) as usize]);
    }
    ImagenRgba {
        ancho: w,
        alto: h,
        pixeles: px,
    }
}

#[test]
#[ignore = "necesita GPU; genera PNG para compararlos a ojo"]
fn muestra_resolucion_con_texto_rico() {
    pixpin_motor2d::texto::instalar_medidor(crate::dibujo::pintar::medir_para_el_motor);
    let carpeta = std::env::var_os("PIXPIN_BANCO_TEXTO")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&carpeta).unwrap();
    let ahora = std::time::Instant::now();
    let escena = escena_de_la_resolucion();
    println!(
        "componer y medir la columna: {:.1} ms",
        ahora.elapsed().as_secs_f64() * 1000.0
    );
    let cuerpo = escena.elementos[1].clone();
    let ahora = std::time::Instant::now();
    let n = pixpin_motor2d::pintado::ordenes(&cuerpo).len();
    println!(
        "ordenes de pintado del cuerpo ({n}): {:.2} ms",
        ahora.elapsed().as_secs_f64() * 1000.0
    );
    let lienzo = Lienzo {
        escena: &escena,
        seleccion: &[],
        papel: None,
        fotos: &|_| None,
        nombre: "resolucion".into(),
    };
    let hojas = pixpin_motor2d::exportar::hojas(
        &escena,
        pixpin_motor2d::exportar::Alcance::Todo,
        &[],
        None,
    );
    let ahora = std::time::Instant::now();
    let escala: f32 = std::env::var("PIXPIN_ESCALA")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.0);
    let propia = super::a_imagen(&hojas[0], escala, Some(escena.fondo), &lienzo).unwrap();
    println!(
        "exportar a PNG: {} ms ({}x{})",
        ahora.elapsed().as_millis(),
        propia.ancho,
        propia.alto
    );
    std::fs::write(
        carpeta.join("resolucion-propia.png"),
        pixpin_codec::imagen::codificar_png(&propia).unwrap(),
    )
    .unwrap();
    // El SVG por el mismo camino que el menu.
    let svg = super::svg_de(&hojas[0], Some(escena.fondo), false, &lienzo);
    std::fs::write(carpeta.join("resolucion-propia.svg"), &svg).unwrap();
    assert!(
        svg.contains(">X<") && !svg.contains("sphericalangle"),
        "el SVG lleva lo compuesto"
    );
    // Y el PDF, por el mismo escritor que el menu (con la letra del sistema).
    let segoe = pixpin_pdf::letra::del_sistema("Segoe UI");
    let pdf = pixpin_pdf::escribir::de_hojas_con_letra(
        &hojas,
        Some(escena.fondo),
        &|_| None,
        segoe.as_ref(),
    )
    .expect("el PDF sale");
    assert!(pdf.starts_with(b"%PDF"));
    std::fs::write(carpeta.join("resolucion-propia.pdf"), &pdf).unwrap();
    if let Some(lamina) = std::env::var_os("PIXPIN_LAMINA") {
        let original = pixpin_codec::imagen::cargar(std::path::Path::new(&lamina)).unwrap();
        // La columna de la derecha, de «RESOLUCION» al «∴ X = 45°».
        let k = original.ancho as f32 / 1052.0;
        let r = |v: f32| (v * k) as u32;
        let columna = recortar(&original, (r(758.0), r(78.0), r(1052.0), r(545.0)));
        let comparar = lado_a_lado(&columna, &propia);
        std::fs::write(
            carpeta.join("resolucion-comparar.png"),
            pixpin_codec::imagen::codificar_png(&comparar).unwrap(),
        )
        .unwrap();
        println!(
            "comparar: {}",
            carpeta.join("resolucion-comparar.png").display()
        );
    }
}
