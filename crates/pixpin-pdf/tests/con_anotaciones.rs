//! **El PDF con lo anotado, que sigue siendo el PDF** (`PdfConAnotaciones`
//! del movil, v0.97.0): el original tal cual al principio del fichero, y
//! detras una revision con la tinta en vectores en su capa, los margenes del
//! lector y los marcadores en el indice. Se comprueba con PDF inventados aqui
//! (texto, caminos y una imagen) y con lo que dibuja Windows.

use pixpin_motor2d::ColorRgba;
use pixpin_motor2d::pintado::Orden;
use pixpin_motor2d::vector::Punto2;
use pixpin_pdf::con_anotaciones::{self, Anotaciones, Marcador};

/// Un PDF de una o varias hojas con un texto, una raya y una imagen de 2x2
/// sin comprimir. `giro` va en `/Rotate` de cada hoja; `indice_en_flujo`
/// escribe el indice como flujo (PDF 1.5), que es lo que traen los de Word.
fn pdf_inventado(hojas: usize, giro: i32, indice_en_flujo: bool) -> Vec<u8> {
    let mut objetos: Vec<Vec<u8>> = Vec::new();
    // 1 catalogo, 2 arbol, 3 letra, 4 imagen, luego por hoja: pagina y contenido.
    let kids: Vec<String> = (0..hojas).map(|i| format!("{} 0 R", 5 + 2 * i)).collect();
    objetos.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    objetos.push(
        format!(
            "<< /Type /Pages /Kids [{}] /Count {hojas} >>",
            kids.join(" ")
        )
        .into_bytes(),
    );
    objetos.push(
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
            .to_vec(),
    );
    let mut imagen = b"<< /Type /XObject /Subtype /Image /Width 2 /Height 2 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length 12 >>\nstream\n".to_vec();
    imagen.extend_from_slice(&[0, 0, 255, 0, 255, 0, 255, 255, 0, 0, 0, 0]);
    imagen.extend_from_slice(b"\nendstream");
    objetos.push(imagen);
    for i in 0..hojas {
        let contenido = format!(
            "BT /F1 24 Tf 72 720 Td (Hola mundo hoja {}) Tj ET\n2 w 72 500 m 500 500 l S\nq 100 0 0 100 400 600 cm /Im1 Do Q\n",
            i + 1
        );
        objetos.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Rotate {giro} /Contents {} 0 R /Resources << /Font << /F1 3 0 R >> /XObject << /Im1 4 0 R >> >> >>",
                6 + 2 * i
            )
            .into_bytes(),
        );
        let mut f = format!("<< /Length {} >>\nstream\n", contenido.len()).into_bytes();
        f.extend_from_slice(contenido.as_bytes());
        f.extend_from_slice(b"\nendstream");
        objetos.push(f);
    }
    let mut s = b"%PDF-1.5\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut donde = Vec::new();
    for (i, o) in objetos.iter().enumerate() {
        donde.push(s.len());
        s.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        s.extend_from_slice(o);
        s.extend_from_slice(b"\nendobj\n");
    }
    let total = objetos.len() + 1;
    if indice_en_flujo {
        let propio = total;
        let inicio = s.len();
        let mut datos = vec![0u8, 0, 0, 0, 0, 0xff, 0xff];
        for d in donde.iter().chain(std::iter::once(&inicio)) {
            datos.push(1);
            datos.extend_from_slice(&(*d as u32).to_be_bytes());
            datos.extend_from_slice(&[0, 0]);
        }
        s.extend_from_slice(
            format!(
                "{propio} 0 obj\n<< /Type /XRef /Size {} /Root 1 0 R /W [1 4 2] /Length {} >>\nstream\n",
                propio + 1,
                datos.len()
            )
            .as_bytes(),
        );
        s.extend_from_slice(&datos);
        s.extend_from_slice(
            format!("\nendstream\nendobj\nstartxref\n{inicio}\n%%EOF\n").as_bytes(),
        );
    } else {
        let xref = s.len();
        s.extend_from_slice(format!("xref\n0 {total}\n0000000000 65535 f \n").as_bytes());
        for d in donde {
            s.extend_from_slice(format!("{d:010} 00000 n \n").as_bytes());
        }
        s.extend_from_slice(
            format!("trailer\n<< /Size {total} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n")
                .as_bytes(),
        );
    }
    s
}

fn rojo() -> ColorRgba {
    ColorRgba::opaco(1.0, 0.0, 0.0)
}

/// Un cuadrado relleno en unidades de la hoja (1400 de ancho).
fn cuadrado(x: f32, y: f32, lado: f32, color: ColorRgba) -> Orden {
    Orden::Relleno {
        puntos: vec![
            Punto2::nuevo(x, y),
            Punto2::nuevo(x + lado, y),
            Punto2::nuevo(x + lado, y + lado),
            Punto2::nuevo(x, y + lado),
        ],
        color,
    }
}

fn trazo_de_resaltador() -> Orden {
    Orden::Polilinea {
        puntos: vec![Punto2::nuevo(100.0, 300.0), Punto2::nuevo(900.0, 320.0)],
        color: ColorRgba {
            r: 1.0,
            g: 0.9,
            b: 0.0,
            a: 0.4,
        },
        grosor: 30.0,
        estilo: pixpin_motor2d::EstiloTrazo::Solido,
    }
}

fn sin_imagenes(_: u64) -> Option<pixpin_pdf::escribir::Pixeles> {
    None
}

fn anotaciones<'a>(
    tinta: &'a dyn Fn(usize) -> Option<Vec<Orden>>,
    marcadores: &'a [Marcador],
) -> Anotaciones<'a> {
    Anotaciones {
        tinta,
        izquierda: 0.0,
        derecha: 0.0,
        marcadores,
        imagenes: &sin_imagenes,
        letra: None,
    }
}

fn texto_de(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn fichero(nombre: &str, bytes: &[u8]) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("pixpin-con-anotaciones-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("carpeta temporal");
    let ruta = dir.join(nombre);
    std::fs::write(&ruta, bytes).expect("escribir el pdf");
    ruta
}

fn pixel(img: &pixpin_codec::imagen::ImagenRgba, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * img.ancho + x) * 4) as usize;
    [
        img.pixeles[i],
        img.pixeles[i + 1],
        img.pixeles[i + 2],
        img.pixeles[i + 3],
    ]
}

fn es_rojo(p: [u8; 4]) -> bool {
    p[0] > 200 && p[1] < 60 && p[2] < 60
}

#[test]
fn el_pdf_con_anotaciones_es_el_mismo_pdf_mas_margenes_tinta_y_marcadores() {
    let original = pdf_inventado(2, 0, false);
    let tinta = |i: usize| {
        (i == 0).then(|| {
            vec![
                cuadrado(100.0, 100.0, 200.0, rojo()),
                trazo_de_resaltador(),
                cuadrado(-600.0, 400.0, 200.0, rojo()),
            ]
        })
    };
    let marcadores = [
        Marcador {
            titulo: "⭐ Hoja 2".into(),
            pagina: 1,
            alto: 0.25,
        },
        // Caso negativo: una hoja que no hay no entra en el indice.
        Marcador {
            titulo: "🔖 Hoja 9".into(),
            pagina: 8,
            alto: 0.0,
        },
    ];
    let mut a = anotaciones(&tinta, &marcadores);
    a.izquierda = 1050.0;
    let hecho = con_anotaciones::hacer(&original, &a).expect("sale el PDF");

    // El original, intacto al principio: se anade, no se reescribe.
    assert_eq!(
        &hecho[..original.len()],
        &original[..],
        "el original no esta entero"
    );
    let t = texto_de(&hecho[original.len()..]);
    // La tinta va **dentro del contenido de la hoja**, al final y en su capa
    // (Android v0.98.2): el lector de PDF de Android no pinta anotaciones y
    // el PDF salia «limpio». La capa sigue en el panel de capas.
    assert!(
        !t.contains("/Subtype /Stamp") && !t.contains("/Annots"),
        "sigue yendo como anotacion: {t}"
    );
    assert!(
        t.contains("/OC /PxOC") && t.contains(" BDC\n/PxT") && t.contains(" Do\nEMC\nQ"),
        "{t}"
    );
    assert!(t.contains("/OCProperties"), "sin capa en el catalogo");
    assert!(t.contains("/Type /OCG"));
    // Como vectores: un formulario con caminos, ni una imagen.
    assert!(t.contains("/Subtype /Form"));
    assert!(!t.contains("/Subtype /Image"), "la tinta no va como foto");
    // La hoja ensanchada a la izquierda (3/4 de 612 pt) y la derecha igual.
    assert!(t.contains("/MediaBox [-459 0 612 792]"), "{t}");
    assert!(t.contains("/CropBox [-459 0 612 792]"));
    // Solo la hoja uno llevaba tinta; las dos se ensanchan.
    assert_eq!(t.matches(" BDC\n").count(), 1);
    assert_eq!(t.matches("/MediaBox [-459 0 612 792]").count(), 2);
    // El indice, con el marcador valido y su emoticono en UTF-16.
    assert!(t.contains("/Type /Outlines"));
    assert!(t.contains("/Count 1"), "{t}");
    assert!(t.contains("<FEFF2B50"), "el titulo no lleva la estrella");
    // Pesa lo que la tinta, no lo que las hojas.
    assert!(
        hecho.len() - original.len() < 12_000,
        "anade {} bytes",
        hecho.len() - original.len()
    );

    // El texto se sigue leyendo (buscar en el PDF lo encuentra).
    let plano = pixpin_pdf::plano::de_bytes(&hecho, 0).expect("se lee");
    assert!(
        plano
            .textos
            .iter()
            .any(|x| x.texto.contains("Hola mundo hoja 1"))
    );
    assert_eq!(pixpin_pdf::union::contar_paginas(&hecho), Some(2));
}

#[test]
fn sin_nada_anotado_sale_el_pdf_limpio_byte_a_byte() {
    let original = pdf_inventado(1, 0, false);
    let nada = |_: usize| None;
    let hecho = con_anotaciones::hacer(&original, &anotaciones(&nada, &[])).expect("sale");
    assert_eq!(hecho, original);
    // Y una tinta vacia es lo mismo que ninguna.
    let vacia = |_: usize| Some(Vec::new());
    assert_eq!(
        con_anotaciones::hacer(&original, &anotaciones(&vacia, &[])).unwrap(),
        original
    );
}

#[test]
fn lo_que_no_es_un_pdf_o_esta_cifrado_no_se_anota() {
    let tinta = |_: usize| Some(vec![cuadrado(0.0, 0.0, 10.0, rojo())]);
    assert!(con_anotaciones::hacer(b"hola", &anotaciones(&tinta, &[])).is_none());
    let original = texto_de(&pdf_inventado(1, 0, false));
    // Un trailer con /Encrypt: cada cadena y cada flujo irian cifrados.
    let cifrado = original.replace(
        "/Root 1 0 R >>",
        "/Root 1 0 R /Encrypt << /Filter /Standard >> >>",
    );
    assert!(con_anotaciones::hacer(cifrado.as_bytes(), &anotaciones(&tinta, &[])).is_none());
}

#[test]
fn con_el_indice_en_flujo_la_revision_tambien_va_en_flujo() {
    let original = pdf_inventado(1, 0, true);
    let tinta = |_: usize| Some(vec![cuadrado(100.0, 100.0, 200.0, rojo())]);
    let hecho = con_anotaciones::hacer(&original, &anotaciones(&tinta, &[])).expect("sale");
    let t = texto_de(&hecho[original.len()..]);
    assert!(
        t.contains("/Type /XRef"),
        "mezclar indices es lo que algunos lectores no perdonan"
    );
    assert!(!t.contains("\ntrailer"));
    assert!(pixpin_pdf::plano::de_bytes(&hecho, 0).is_some());
}

#[test]
fn lo_anotado_se_quita_cortando_como_lo_cocido_del_movil() {
    // La capa se llama «PixPin · hoja N», como la del movil: un PDF que
    // vuelva al lector se limpia igual que uno de un proyecto del movil.
    let original = pdf_inventado(1, 0, false);
    let tinta = |_: usize| Some(vec![cuadrado(100.0, 100.0, 200.0, rojo())]);
    let hecho = con_anotaciones::hacer(&original, &anotaciones(&tinta, &[])).expect("sale");
    assert_eq!(
        pixpin_pdf::cocido::largo_sin_lo_cocido(&hecho),
        Some(original.len())
    );
}

#[test]
fn windows_pinta_la_tinta_donde_se_puso_y_la_hoja_ensanchada() {
    let original = pdf_inventado(1, 0, false);
    // Un cuadrado arriba a la izquierda de la hoja y otro en el margen.
    let tinta = |_: usize| {
        Some(vec![
            cuadrado(100.0, 100.0, 200.0, rojo()),
            cuadrado(-800.0, 1000.0, 300.0, rojo()),
        ])
    };
    let mut a = anotaciones(&tinta, &[]);
    a.izquierda = 1050.0;
    let hecho = con_anotaciones::hacer(&original, &a).expect("sale");
    let ruta = fichero("ensanchado.pdf", &hecho);
    let doc = pixpin_pdf::Documento::abrir(&ruta).expect("Windows lo abre");
    // Windows mide en pixeles de 96 ppp: un punto son 4/3.
    let (w, h) = doc.medidas()[0];
    assert!((w * 0.75 - (612.0 + 459.0)).abs() < 1.0, "ancho {w}");
    assert!((h * 0.75 - 792.0).abs() < 1.0);
    // Pintada a 2450 px: una unidad de la hoja es 1 px (2450 = 1400 + 1050).
    let img = doc.renderizar(0, 2450).expect("pinta");
    let _ = std::fs::write(
        ruta.with_extension("png"),
        pixpin_codec::codificar_png(&img).unwrap(),
    );
    assert!(
        es_rojo(pixel(&img, 1050 + 200, 200)),
        "la tinta de la hoja: {:?}",
        pixel(&img, 1250, 200)
    );
    assert!(
        es_rojo(pixel(&img, 1050 - 650, 1150)),
        "la del margen: {:?}",
        pixel(&img, 400, 1150)
    );
    // Caso negativo: fuera de los cuadrados no hay rojo.
    assert!(!es_rojo(pixel(&img, 1050 + 700, 1300)));
}

#[test]
fn en_una_hoja_girada_la_tinta_cae_donde_se_ve() {
    let original = pdf_inventado(1, 90, false);
    // La hoja se ve apaisada: 792 x 612. Un cuadrado arriba a la izquierda.
    let tinta = |_: usize| Some(vec![cuadrado(100.0, 100.0, 200.0, rojo())]);
    let mut a = anotaciones(&tinta, &[]);
    // Los margenes no se ponen en una hoja girada (el movil tampoco).
    a.derecha = 1050.0;
    let hecho = con_anotaciones::hacer(&original, &a).expect("sale");
    let ruta = fichero("girada.pdf", &hecho);
    let doc = pixpin_pdf::Documento::abrir(&ruta).expect("abre");
    let (w, h) = doc.medidas()[0];
    assert!(
        (w * 0.75 - 792.0).abs() < 1.0 && (h * 0.75 - 612.0).abs() < 1.0,
        "{w}x{h}"
    );
    let img = doc.renderizar(0, 1400).expect("pinta");
    let _ = std::fs::write(
        ruta.with_extension("png"),
        pixpin_codec::codificar_png(&img).unwrap(),
    );
    assert!(
        es_rojo(pixel(&img, 200, 200)),
        "{:?}",
        pixel(&img, 200, 200)
    );
    assert!(!es_rojo(pixel(&img, 1200, 200)), "ni a la derecha");
    assert!(!es_rojo(pixel(&img, 200, 900)), "ni abajo");
}

/// **La tinta, dentro del contenido de la hoja** (Android v0.98.2): la hoja
/// conserva sus flujos y sus recursos (la letra y la imagen se siguen
/// pintando) y Windows la pinta sin mirar anotaciones.
#[test]
fn la_tinta_va_al_final_del_contenido_y_la_hoja_sigue_con_lo_suyo() {
    let original = pdf_inventado(1, 0, false);
    let tinta = |_: usize| Some(vec![cuadrado(100.0, 100.0, 200.0, rojo())]);
    let hecho = con_anotaciones::hacer(&original, &anotaciones(&tinta, &[])).expect("sale");
    let t = texto_de(&hecho[original.len()..]);
    // Su contenido (el 6) entre nuestro `q` y lo nuestro.
    let contenidos = t
        .split("/Contents [")
        .nth(1)
        .and_then(|r| r.split(']').next())
        .expect("lista de contenidos");
    let refs: Vec<&str> = contenidos
        .split(" R")
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .collect();
    assert_eq!(refs.len(), 3, "{contenidos}");
    assert_eq!(refs[1], "6 0", "el suyo en medio: {contenidos}");
    // Sus recursos siguen, con los nuestros al lado.
    assert!(t.contains("/F1 3 0 R") && t.contains("/Im1 4 0 R"), "{t}");
    assert!(t.contains("/Properties <</PxOC"), "{t}");
    // Windows pinta la tinta, la imagen de la hoja y el texto.
    let ruta = fichero("contenido.pdf", &hecho);
    let doc = pixpin_pdf::Documento::abrir(&ruta).expect("abre");
    let img = doc.renderizar(0, 1400).expect("pinta");
    let _ = std::fs::write(
        ruta.with_extension("png"),
        pixpin_codec::codificar_png(&img).unwrap(),
    );
    assert!(
        es_rojo(pixel(&img, 200, 200)),
        "la tinta: {:?}",
        pixel(&img, 200, 200)
    );
    // La imagen de 2x2 en (400..500, 92..192) puntos: 1400/612 px por punto.
    let k = 1400.0 / 612.0;
    let p = pixel(&img, (450.0 * k) as u32, (142.0 * k) as u32);
    assert!(
        p[0] < 200 || p[1] < 200 || p[2] < 200,
        "la imagen de la hoja no se pinta: {p:?}"
    );
    // Caso negativo: fuera de todo, blanco.
    assert!(!es_rojo(pixel(&img, 1300, 1700)));
}

/// **Los marcadores vuelven** (Android v0.98.2): lo que se escribe en el
/// indice se vuelve a leer igual, con su hoja y su altura.
#[test]
fn los_marcadores_del_indice_se_leen_como_se_escribieron() {
    let original = pdf_inventado(3, 0, false);
    let nada = |_: usize| None;
    let marcadores = [
        Marcador {
            titulo: "⭐ Hoja 1".into(),
            pagina: 0,
            alto: 0.0,
        },
        Marcador {
            titulo: "🔖 Hoja 3".into(),
            pagina: 2,
            alto: 0.5,
        },
    ];
    let hecho = con_anotaciones::hacer(&original, &anotaciones(&nada, &marcadores)).expect("sale");
    let leidos = con_anotaciones::marcadores_del_indice(&hecho, 200);
    assert_eq!(leidos.len(), 2, "{leidos:?}");
    assert_eq!(
        (leidos[0].titulo.as_str(), leidos[0].pagina),
        ("⭐ Hoja 1", 0)
    );
    assert_eq!(
        (leidos[1].titulo.as_str(), leidos[1].pagina),
        ("🔖 Hoja 3", 2)
    );
    assert!(
        leidos[0].alto.abs() < 1e-6 && (leidos[1].alto - 0.5).abs() < 1e-3,
        "{leidos:?}"
    );
    // Con tope, solo los primeros.
    assert_eq!(con_anotaciones::marcadores_del_indice(&hecho, 1).len(), 1);
    // Casos negativos: sin indice, nada; y lo que no es un PDF, nada.
    assert!(con_anotaciones::marcadores_del_indice(&original, 200).is_empty());
    assert!(con_anotaciones::marcadores_del_indice(b"hola", 200).is_empty());
}
