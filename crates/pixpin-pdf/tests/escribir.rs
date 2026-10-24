//! El PDF que escribe `escribir` se abre con lo que trae Windows
//! (`Windows.Data.Pdf`, el mismo lector del visor de documentos) y se ve
//! donde tiene que verse. Sin esto, «el PDF abre» seria una promesa: un
//! xref con un desplazamiento mal contado lo abre un lector tolerante y lo
//! rechaza otro.

use pixpin_motor2d::ColorRgba;
use pixpin_motor2d::exportar::{self, Alcance, Hoja};
use pixpin_motor2d::pintado::Orden;
use pixpin_motor2d::vector::Punto2;
use pixpin_pdf::escribir::{Pixeles, de_hojas};

fn cuadrado(x: f32, y: f32, lado: f32) -> Vec<Punto2> {
    vec![
        Punto2::nuevo(x, y),
        Punto2::nuevo(x + lado, y),
        Punto2::nuevo(x + lado, y + lado),
        Punto2::nuevo(x, y + lado),
    ]
}

fn hoja(ordenes: Vec<Orden>) -> Hoja {
    Hoja {
        nombre: String::new(),
        caja: (0.0, 0.0, 100.0, 100.0),
        ordenes,
        marcos: Vec::new(),
        granos: Vec::new(),
        grafitos: Vec::new(),
    }
}

fn fichero(nombre: &str, bytes: &[u8]) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("pixpin-pdf-escribir-{}", std::process::id()));
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

#[test]
fn el_pdf_escrito_abre_con_windows_y_pinta_cada_hoja_en_su_pagina() {
    let negro = ColorRgba::opaco(0.0, 0.0, 0.0);
    let hojas = vec![
        hoja(vec![
            Orden::Relleno {
                puntos: cuadrado(0.0, 0.0, 100.0),
                color: negro,
            },
            Orden::Texto {
                texto: "Hola (año) €".into(),
                x: 5.0,
                y: 5.0,
                tam: 12.0,
                familia: String::new(),
                color: ColorRgba::opaco(1.0, 1.0, 1.0),
                ancho_max: 90.0,
                negrita: false,
                cursiva: false,
            },
        ]),
        hoja(vec![Orden::Imagen {
            id_objeto: 7,
            x: 0.0,
            y: 0.0,
            ancho: 100.0,
            alto: 100.0,
            opacidad: 1.0,
            recorte: None,
            angulo: 0.0,
        }]),
    ];
    let bytes = de_hojas(&hojas, Some(ColorRgba::opaco(1.0, 1.0, 1.0)), &|id| {
        (id == 7).then(|| Pixeles {
            ancho: 2,
            alto: 2,
            rgba: [255u8, 0, 0, 255].repeat(4),
        })
    })
    .expect("hay hojas");
    assert!(bytes.starts_with(b"%PDF-1.4"));
    assert!(bytes.ends_with(b"%%EOF\n"));
    let ruta = fichero("dos.pdf", &bytes);
    let doc = pixpin_pdf::Documento::abrir(&ruta).expect("Windows lo abre");
    assert_eq!(doc.paginas(), 2);
    // Una A4 derecha: la hoja es cuadrada, asi que no se tumba.
    let p1 = doc.renderizar(0, 300).expect("pagina 1");
    assert!(p1.alto > p1.ancho);
    let centro = pixel(&p1, p1.ancho / 2, p1.alto / 2);
    assert!(
        centro[0] < 40 && centro[1] < 40 && centro[2] < 40,
        "el cuadrado negro: {centro:?}"
    );
    let esquina = pixel(&p1, 2, 2);
    assert!(
        esquina[0] > 200,
        "fuera de la hoja queda el papel: {esquina:?}"
    );
    let p2 = doc.renderizar(1, 300).expect("pagina 2");
    let c2 = pixel(&p2, p2.ancho / 2, p2.alto / 2);
    assert!(
        c2[0] > 200 && c2[1] < 60 && c2[2] < 60,
        "la imagen roja: {c2:?}"
    );
}

#[test]
fn una_hoja_apaisada_sale_en_una_pagina_tumbada_y_la_transparencia_se_respeta() {
    let mut h = hoja(vec![Orden::Relleno {
        puntos: cuadrado(0.0, 0.0, 100.0),
        color: ColorRgba {
            a: 0.5,
            ..ColorRgba::opaco(0.0, 0.0, 0.0)
        },
    }]);
    h.caja = (0.0, 0.0, 300.0, 100.0);
    let bytes = de_hojas(&[h], None, &|_| None).expect("una hoja");
    let texto = String::from_utf8_lossy(&bytes);
    assert!(texto.contains("/ca 0.5"), "el estado de transparencia");
    let ruta = fichero("apaisada.pdf", &bytes);
    let doc = pixpin_pdf::Documento::abrir(&ruta).expect("Windows lo abre");
    let p = doc.renderizar(0, 400).expect("pagina");
    assert!(p.ancho > p.alto, "tumbada");
    // El cuadrado ocupa el primer tercio de la hoja: gris, no negro.
    let x = p.ancho / 6;
    let gris = pixel(&p, x, p.alto / 2);
    assert!(
        gris[0] > 90 && gris[0] < 170,
        "medio transparente: {gris:?}"
    );
}

#[test]
fn un_lienzo_de_verdad_con_marcos_da_una_pagina_por_marco() {
    use pixpin_motor2d::{Elemento, Escena, Figura};
    let mut escena = Escena::nueva();
    for (y, nombre) in [(0.0, "Planta"), (400.0, "Alzado")] {
        escena.anadir(Elemento {
            figura: Figura::Marco {
                nombre: nombre.into(),
            },
            x: 0.0,
            y,
            ancho: 300.0,
            alto: 200.0,
            ..Elemento::default()
        });
        escena.anadir(Elemento {
            figura: Figura::Rectangulo,
            x: 20.0,
            y: y + 20.0,
            ancho: 100.0,
            alto: 80.0,
            ..Elemento::default()
        });
    }
    let hojas = exportar::hojas(&escena, Alcance::Marcos, &[], None);
    assert_eq!(hojas.len(), 2);
    let bytes = de_hojas(&hojas, None, &|_| None).expect("dos hojas");
    let doc = pixpin_pdf::Documento::abrir(&fichero("marcos.pdf", &bytes)).expect("abre");
    assert_eq!(doc.paginas(), 2);
}

/// La luz media de una franja de la pagina, para comparar dos PDF.
fn luz_media(img: &pixpin_codec::imagen::ImagenRgba, y0: u32, y1: u32) -> f64 {
    let mut suma = 0u64;
    let mut n = 0u64;
    for y in y0..y1 {
        for x in img.ancho / 4..img.ancho * 3 / 4 {
            let p = pixel(img, x, y);
            suma += p[0] as u64 + p[1] as u64 + p[2] as u64;
            n += 3;
        }
    }
    suma as f64 / n.max(1) as f64
}

#[test]
fn el_grano_de_una_tinta_porosa_se_ve_en_el_pdf_y_sin_el_sale_lisa() {
    use pixpin_motor2d::{Elemento, Escena, Figura};
    let mut escena = Escena::nueva();
    escena.anadir(Elemento {
        figura: Figura::Lapiz {
            puntos: (0..40)
                .map(|i| Punto2::nuevo(i as f32 * 10.0, 50.0))
                .collect(),
            presiones: Vec::new(),
            opciones: Some(Default::default()),
        },
        grosor: 16.0,
        trazo: ColorRgba::opaco(0.2, 0.3, 0.9),
        material: pixpin_motor2d::tinta::MaterialTinta::Rayado,
        ..Elemento::default()
    });
    let con = exportar::hojas(&escena, Alcance::Todo, &[], None);
    assert_eq!(con[0].granos.len(), 1, "el rayado apunta su grano");
    let mut sin = con.clone();
    sin[0].granos.clear();
    let blanco = Some(ColorRgba::opaco(1.0, 1.0, 1.0));
    let bytes_con = de_hojas(&con, blanco, &|_| None).expect("con grano");
    let bytes_sin = de_hojas(&sin, blanco, &|_| None).expect("sin grano");
    let texto = String::from_utf8_lossy(&bytes_con);
    assert!(texto.contains("/PatternType 1"), "la tela va como patron");
    assert!(!String::from_utf8_lossy(&bytes_sin).contains("/Pattern"));
    let p_con = pixpin_pdf::Documento::abrir(&fichero("grano.pdf", &bytes_con))
        .and_then(|d| d.renderizar(0, 1200))
        .expect("Windows lo abre");
    let p_sin = pixpin_pdf::Documento::abrir(&fichero("liso.pdf", &bytes_sin))
        .and_then(|d| d.renderizar(0, 1200))
        .expect("Windows lo abre");
    // La hoja es apaisada y el trazo va por el medio: en la franja central
    // el grano oscurece (la tela es la tinta oscurecida).
    let (y0, y1) = (p_con.alto / 2 - 4, p_con.alto / 2 + 4);
    let (luz_con, luz_sin) = (luz_media(&p_con, y0, y1), luz_media(&p_sin, y0, y1));
    assert!(
        luz_con + 5.0 < luz_sin,
        "con grano mas oscuro: {luz_con} contra {luz_sin}"
    );
    // Y no es un oscurecido parejo: dentro del trazo hay rayas, o sea mas
    // de un tono en la misma fila.
    let fila: Vec<u8> = (p_con.ancho / 4..p_con.ancho * 3 / 4)
        .map(|x| pixel(&p_con, x, p_con.alto / 2)[2])
        .collect();
    let (min, max) = (fila.iter().min().unwrap(), fila.iter().max().unwrap());
    assert!(max - min > 20, "rayas y no un color liso: {min}..{max}");
}

#[test]
fn una_imagen_recortada_sale_en_el_pdf_con_solo_su_trozo() {
    // Cuatro cuadrantes de 2x2: rojo, verde / azul, amarillo.
    let mut rgba = Vec::new();
    for y in 0..4 {
        for x in 0..4 {
            rgba.extend_from_slice(match (x < 2, y < 2) {
                (true, true) => &[255u8, 0, 0, 255],
                (false, true) => &[0, 255, 0, 255],
                (true, false) => &[0, 0, 255, 255],
                (false, false) => &[255, 255, 0, 255],
            });
        }
    }
    let imagen = |recorte| Orden::Imagen {
        id_objeto: 3,
        x: 0.0,
        y: 0.0,
        ancho: 100.0,
        alto: 100.0,
        opacidad: 1.0,
        recorte,
        angulo: 0.0,
    };
    let verde = Some(pixpin_motor2d::RecorteImagen {
        x: 20.0,
        y: 0.0,
        ancho: 20.0,
        alto: 20.0,
        ancho_natural: 40.0,
        alto_natural: 40.0,
    });
    let pixeles = |_: u64| {
        Some(Pixeles {
            ancho: 4,
            alto: 4,
            rgba: rgba.clone(),
        })
    };
    let bytes = de_hojas(&[hoja(vec![imagen(verde)])], None, &pixeles).expect("una hoja");
    let p = pixpin_pdf::Documento::abrir(&fichero("recorte.pdf", &bytes))
        .and_then(|d| d.renderizar(0, 400))
        .expect("Windows lo abre");
    // La hoja entera es el cuadrante verde, estirado: en el centro y cerca
    // de las cuatro esquinas de la hoja, verde.
    let (w, h) = (p.ancho, p.alto);
    for (x, y) in [
        (w / 2, h / 2),
        (w / 5, h / 2 - w / 5),
        (w * 4 / 5, h / 2 + w / 5),
    ] {
        let c = pixel(&p, x, y);
        assert!(
            c[1] > 200 && c[0] < 60 && c[2] < 60,
            "verde en ({x}, {y}): {c:?}"
        );
    }
    // Caso negativo: sin recorte sale la foto entera, con su rojo arriba a la
    // izquierda.
    let entera = de_hojas(&[hoja(vec![imagen(None)])], None, &pixeles).expect("una hoja");
    let q = pixpin_pdf::Documento::abrir(&fichero("entera.pdf", &entera))
        .and_then(|d| d.renderizar(0, 400))
        .expect("Windows lo abre");
    let c = pixel(&q, q.ancho / 5, q.alto / 2 - q.ancho / 5);
    assert!(c[0] > 200 && c[1] < 60, "rojo arriba a la izquierda: {c:?}");
}

#[test]
fn con_la_letra_de_la_pantalla_el_texto_va_incrustado_y_se_ve() {
    let Some(segoe) = pixpin_pdf::letra::del_sistema("Segoe UI") else {
        return;
    };
    let negro = ColorRgba::opaco(0.0, 0.0, 0.0);
    let texto = |ancho_max| Orden::Texto {
        texto: "Planta baja año".into(),
        x: 5.0,
        y: 30.0,
        tam: 14.0,
        familia: "Segoe UI".into(),
        color: negro,
        ancho_max,
        negrita: false,
        cursiva: false,
    };
    let bytes = pixpin_pdf::escribir::de_hojas_con_letra(
        &[hoja(vec![texto(90.0)])],
        None,
        &|_| None,
        Some(&segoe),
    )
    .expect("una hoja");
    let s = String::from_utf8_lossy(&bytes);
    assert!(
        s.contains("/FontFile2") && s.contains("+SegoeUI"),
        "la letra va dentro"
    );
    assert!(s.contains("/ToUnicode"), "y se puede copiar el texto");
    assert!(
        bytes.len() < 120_000,
        "solo las letras usadas: {} bytes",
        bytes.len()
    );
    let p = pixpin_pdf::Documento::abrir(&fichero("letra.pdf", &bytes))
        .and_then(|d| d.renderizar(0, 600))
        .expect("Windows lo abre");
    // Hay tinta en la franja del texto: la letra incrustada se pinta.
    let oscuros = (0..p.alto)
        .flat_map(|y| (0..p.ancho).map(move |x| (x, y)))
        .filter(|(x, y)| pixel(&p, *x, *y)[0] < 100)
        .count();
    assert!(oscuros > 200, "el texto se ve: {oscuros} pixeles oscuros");
    // Parte con los anchos de Segoe UI, los mismos que la tabla del motor:
    // «Planta baja» cabe en 90 a 14 puntos y «año» baja a la segunda linea.
    let dos = exportar::partir_texto_con("Planta baja año", 14.0, 90.0, &|c| segoe.ancho(c));
    assert_eq!(dos, exportar::partir_texto("Planta baja año", 14.0, 90.0));
    assert_eq!(dos.len(), 2, "{dos:?}");
    // Caso negativo: sin letra, Helvetica y nada incrustado.
    let sin = de_hojas(&[hoja(vec![texto(90.0)])], None, &|_| None).expect("una hoja");
    let s2 = String::from_utf8_lossy(&sin);
    assert!(!s2.contains("/FontFile2") && s2.contains("/Helvetica"));
}

/// La hoja de la prueba de simbolos: el renglon de un estudiante de ciencias
/// y, debajo, un caracter que no tiene ninguna letra del sistema.
fn hoja_de_simbolos(linea: &str) -> Hoja {
    let negro = ColorRgba::opaco(0.0, 0.0, 0.0);
    let texto = |t: &str, y: f32| Orden::Texto {
        texto: t.into(),
        x: 4.0,
        y,
        tam: 5.5,
        familia: String::new(),
        color: negro,
        ancho_max: 0.0,
        negrita: false,
        cursiva: false,
    };
    hoja(vec![texto(linea, 10.0), texto("falta: \u{10FFFD}.", 40.0)])
}

const SIMBOLOS: &str = "θ μ Ω Σ π − √ ≤ ∞ ° x² H₂O → 😀";

/// **Los simbolos salen y se pueden copiar**: lo que la letra del texto no
/// tiene va en su letra de reserva (Segoe UI Symbol, Cambria Math, Segoe UI
/// Emoji...), incrustada con solo sus glifos y con su `ToUnicode`, y lo que
/// no tiene nadie sale como el cuadrito del hueco, nunca como un `?`. Con
/// Helvetica (sin letra propia) y con Segoe UI dentro.
#[test]
fn los_simbolos_de_ciencias_salen_con_su_letra_y_se_copian_tal_cual() {
    let Some(segoe) = pixpin_pdf::letra::del_sistema("Segoe UI") else {
        return;
    };
    for (nombre, propia) in [("helvetica", None), ("segoe", Some(&segoe))] {
        let bytes = pixpin_pdf::escribir::de_hojas_con_letra(
            &[hoja_de_simbolos(SIMBOLOS)],
            Some(ColorRgba::opaco(1.0, 1.0, 1.0)),
            &|_| None,
            propia,
        )
        .expect("una hoja");
        let s = String::from_utf8_lossy(&bytes);
        assert!(
            s.contains("/CIDFontType2") && s.contains("/ToUnicode"),
            "{nombre}"
        );
        assert!(
            s.contains("+SegoeUIEmoji"),
            "{nombre}: el emoji va en su letra"
        );
        assert!(
            bytes.len() < 150_000,
            "{nombre}: solo los glifos usados: {} bytes",
            bytes.len()
        );
        // El texto que se copia del PDF es el que se escribio.
        let paginas = pixpin_pdf::texto::de_bytes(&bytes).expect("se lee el texto");
        let leido: String = paginas[0]
            .texto
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        let quiero: String = SIMBOLOS.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(leido.contains(&quiero), "{nombre}: {leido:?}");
        // Caso negativo: lo que no tiene nadie no se calla con un `?`: se
        // copia como el caracter de sustitucion.
        assert!(!leido.contains('?'), "{nombre}: {leido:?}");
        assert!(leido.contains("falta:\u{fffd}."), "{nombre}: {leido:?}");
        // Y Windows lo abre y lo pinta.
        let ruta = fichero(&format!("simbolos-{nombre}.pdf"), &bytes);
        let p = pixpin_pdf::Documento::abrir(&ruta)
            .and_then(|d| d.renderizar(0, 1400))
            .expect("Windows lo abre");
        let oscuros = (0..p.alto)
            .flat_map(|y| (0..p.ancho).map(move |x| (x, y)))
            .filter(|(x, y)| pixel(&p, *x, *y)[0] < 100)
            .count();
        assert!(oscuros > 500, "{nombre}: el renglon se ve: {oscuros}");
        if let Ok(dir) = std::env::var("PIXPIN_PDF_IMAGENES") {
            let png = pixpin_codec::imagen::codificar_png(&p).expect("png");
            std::fs::write(
                std::path::Path::new(&dir).join(format!("simbolos-{nombre}.png")),
                png,
            )
            .expect("escribir png");
        }
    }
}

/// **Cada simbolo es un glifo de verdad**, no el hueco: pintado solo, cada
/// uno deja una tinta distinta de la del cuadrito de «falta».
#[test]
fn cada_simbolo_se_pinta_con_su_glifo_y_no_con_el_hueco() {
    if pixpin_pdf::letra::del_sistema("Segoe UI").is_none() {
        return;
    }
    let pinta = |c: &str| {
        let negro = ColorRgba::opaco(0.0, 0.0, 0.0);
        let bytes = de_hojas(
            &[hoja(vec![Orden::Texto {
                texto: c.into(),
                x: 10.0,
                y: 10.0,
                tam: 60.0,
                familia: String::new(),
                color: negro,
                ancho_max: 0.0,
                negrita: false,
                cursiva: false,
            }])],
            Some(ColorRgba::opaco(1.0, 1.0, 1.0)),
            &|_| None,
        )
        .expect("una hoja");
        let ruta = fichero(
            &format!("uno-{:x}.pdf", c.chars().next().unwrap() as u32),
            &bytes,
        );
        let p = pixpin_pdf::Documento::abrir(&ruta)
            .and_then(|d| d.renderizar(0, 300))
            .expect("Windows lo abre");
        (0..p.alto)
            .flat_map(|y| (0..p.ancho).map(move |x| (x, y)))
            .map(|(x, y)| (pixel(&p, x, y)[0] < 128) as u8)
            .collect::<Vec<u8>>()
    };
    let hueco = pinta("\u{10FFFD}");
    assert!(hueco.contains(&1), "el hueco se ve: un cuadrito");
    for c in [
        "θ", "μ", "Ω", "Σ", "π", "−", "√", "≤", "∞", "°", "²", "₂", "→", "😀",
    ] {
        let tinta = pinta(c);
        assert!(tinta.contains(&1), "{c} no se ve");
        assert_ne!(tinta, hueco, "{c} sale como el hueco");
    }
}
