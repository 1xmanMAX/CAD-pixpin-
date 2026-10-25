//! Las pruebas de los formatos de punta a punta: un PDF escrito a mano que
//! se importa, exportar e importar de nuevo (PDF y SVG), las ordenes con su
//! dialogo y los casos malos (PDF roto, pagina vacia, fichero que no esta).

use std::io::Write as _;
use std::path::PathBuf;

use opencadcodec::EntityType;

use crate::especialidad as esp;
use crate::formatos::escena::{self, Que};
use crate::formatos::pdf::{self, Documento, OpcionesPdf, Papel};
use crate::formatos::{self, Geometria, svg};
use crate::geo;
use crate::herramienta::PedidoApp;
use crate::{Dibujo, Sesion};

fn carpeta(nombre: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("cad-nucleo-fmt-{nombre}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn comprimir(b: &[u8]) -> Vec<u8> {
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    z.write_all(b).unwrap();
    z.finish().unwrap()
}

/// **Un PDF a mano**: los objetos dados (el 1 es el catalogo) con su tabla
/// xref y su trailer.
fn pdf_a_mano(objetos: &[Vec<u8>]) -> Vec<u8> {
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut pos = Vec::new();
    for (i, o) in objetos.iter().enumerate() {
        pos.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(o);
        out.extend_from_slice(b"\nendobj\n");
    }
    let x = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objetos.len() + 1).as_bytes(),
    );
    for p in pos {
        out.extend_from_slice(format!("{p:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{x}\n%%EOF\n",
            objetos.len() + 1
        )
        .as_bytes(),
    );
    out
}

fn flujo(dic: &str, datos: &[u8]) -> Vec<u8> {
    let mut v = format!("<< {dic} /Length {} >>\nstream\n", datos.len()).into_bytes();
    v.extend_from_slice(datos);
    v.extend_from_slice(b"\nendstream");
    v
}

/// Un PDF A4 con lineas, una curva, un circulo, un relleno, un grupo de
/// contenido opcional (apagado), un formulario comprimido, una imagen y
/// textos (uno girado y con TJ, otro con una ñ).
fn pdf_de_prueba() -> Vec<u8> {
    // Un circulo de radio 50 pt en (300, 600) con cuatro Bezier.
    let k = 0.552_284_75 * 50.0;
    let circulo = format!(
        "350 600 m 350 {a} {b} 650 300 650 c {c} 650 250 {a} 250 600 c 250 {d} {c} 550 300 550 c {b} 550 350 {d} 350 600 c S\n",
        a = 600.0 + k,
        b = 300.0 + k,
        c = 300.0 - k,
        d = 600.0 - k
    );
    let contenido = format!(
        "q 1 0 0 RG 2 w 72 72 m 144 72 l S Q\n\
         0 0 1 RG 100 100 m 100 155 145 200 200 200 c S\n\
         0 g 10 10 50 50 re f\n\
         1 g 0 0 595 842 re f\n\
         /OC /oc1 BDC 0 G 300 300 m 400 300 l S EMC\n\
         q 1 0 0 1 100 0 cm /Fm1 Do Q\n\
         q 10 0 0 10 0 0 cm /Im1 Do Q\n\
         {circulo}\
         BT /F1 12 Tf 1 0 0 1 72 500 Tm (Hola) Tj ET\n\
         BT /F1 12 Tf 0 1 -1 0 500 400 Tm [(Mun) -300 (do)] TJ ET\n\
         BT /F1 10 Tf 72 450 Td (A\\361o) Tj ET\n"
    );
    let forma = comprimir(b"0.5 0 0.5 RG 0 700 m 50 700 l S");
    pdf_a_mano(&[
        b"<< /Type /Catalog /Pages 2 0 R /OCProperties << /OCGs [5 0 R] /D << /OFF [5 0 R] >> >> >>"
            .to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 595 842] >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /Contents 4 0 R /Resources << /Font << /F1 6 0 R >> /Properties << /oc1 5 0 R >> /XObject << /Fm1 7 0 R /Im1 8 0 R >> >> >>".to_vec(),
        flujo("", contenido.as_bytes()),
        b"<< /Type /OCG /Name (Cotas) >>".to_vec(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
            .to_vec(),
        flujo(
            "/Type /XObject /Subtype /Form /BBox [0 0 595 842] /Filter /FlateDecode",
            &forma,
        ),
        flujo(
            "/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8",
            b"\x80",
        ),
    ])
}

const MM: f64 = 25.4 / 72.0;

#[test]
fn importa_un_pdf_hecho_a_mano() {
    let doc = Documento::leer(&pdf_de_prueba()).unwrap();
    assert_eq!(pdf::importar::paginas(&doc), 1);
    let imp = pdf::importar::importar(&doc, 1, Default::default()).unwrap();
    assert!((imp.pagina.0 - 595.0 * MM).abs() < 1e-9);
    // La linea roja de 2 pt.
    let roja = imp
        .figuras
        .iter()
        .find(|f| f.trazo.is_some_and(|t| t.0 == [255, 0, 0]))
        .unwrap();
    let (_, w) = roja.trazo.unwrap();
    assert!((w - 2.0 * MM).abs() < 1e-9);
    let Geometria::Polilinea { puntos, .. } = &roja.partes[0] else {
        panic!()
    };
    assert!(geo::dist(puntos[0], [72.0 * MM, 72.0 * MM]) < 1e-9);
    assert!(geo::dist(puntos[1], [144.0 * MM, 72.0 * MM]) < 1e-9);
    // La curva en muchos puntos y acabando en su sitio.
    let azul = imp
        .figuras
        .iter()
        .find(|f| f.trazo.is_some_and(|t| t.0 == [0, 0, 255]))
        .unwrap();
    let Geometria::Polilinea { puntos, .. } = &azul.partes[0] else {
        panic!()
    };
    assert!(puntos.len() > 4);
    assert!(geo::dist(*puntos.last().unwrap(), [200.0 * MM, 200.0 * MM]) < 1e-9);
    // El relleno negro (y el blanco del fondo no).
    assert_eq!(
        imp.figuras.iter().filter(|f| f.relleno.is_some()).count(),
        1
    );
    assert!(imp.avisos.iter().any(|a| a.contains("blancos")));
    // El grupo de contenido opcional, y apagado.
    let cotas = imp
        .figuras
        .iter()
        .find(|f| f.grupo.as_deref() == Some("Cotas"))
        .unwrap();
    assert!(cotas.trazo.is_some());
    assert_eq!(imp.apagados, ["Cotas"]);
    // El formulario, movido 100 pt por su `cm`.
    let forma = imp
        .figuras
        .iter()
        .find(|f| f.trazo.is_some_and(|t| t.0 == [128, 0, 128]))
        .unwrap();
    let Geometria::Polilinea { puntos, .. } = &forma.partes[0] else {
        panic!()
    };
    assert!(geo::dist(puntos[0], [100.0 * MM, 700.0 * MM]) < 1e-9);
    assert!(imp.avisos.iter().any(|a| a.contains("imagen")));
    // Los textos.
    let textos: Vec<&str> = imp.rotulos.iter().map(|r| r.texto.as_str()).collect();
    assert_eq!(textos, ["Hola", "Mun do", "Año"]);
    let hola = &imp.rotulos[0];
    assert!(geo::dist(hola.p, [72.0 * MM, 500.0 * MM]) < 1e-9);
    assert!((hola.alto - 12.0 * MM * 0.7).abs() < 1e-9);
    let girado = &imp.rotulos[1];
    assert!((girado.giro - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    // A entidades: el circulo de cuatro Bezier vuelve a ser un circulo.
    let r = formatos::a_entidades(&imp, "PDF_", [0.0, 0.0], 1.0);
    let circulos: Vec<_> = r
        .entidades
        .iter()
        .filter_map(|(e, _)| match e {
            EntityType::Circle(c) => Some(c.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(circulos.len(), 1);
    assert!((circulos[0].radius - 50.0 * MM).abs() < 0.01);
    assert!(
        r.entidades
            .iter()
            .any(|(e, _)| matches!(e, EntityType::Hatch(_)))
    );
    assert!(r.capas.iter().any(|c| c.nombre == "PDF_Cotas" && c.apagada));
    assert!(r.capas.iter().any(|c| c.nombre == "PDF_FF0000"));
}

#[test]
fn la_orden_importarpdf_con_dialogo_escala_y_punto() {
    let dir = carpeta("orden-pdf");
    let ruta = dir.join("plano.pdf");
    std::fs::write(&ruta, pdf_de_prueba()).unwrap();
    let mut s = Sesion::nueva(Dibujo::nuevo());
    s.escribir("pdfimport");
    let Some(PedidoApp::Archivo {
        guardar, filtros, ..
    }) = s.memoria.pedidos.pop()
    else {
        panic!("pide el dialogo")
    };
    assert!(!guardar && filtros[0].1 == "*.pdf");
    assert!(s.pide_texto());
    s.escribir(&format!("\"{}\"", ruta.display()));
    // Una sola pagina: pide la escala. A 1:100 en un dibujo en mm.
    s.escribir("100");
    s.escribir("1000,2000");
    assert_eq!(s.activa(), None, "{:?}", s.registro);
    let n = s.dibujo.doc.model_space_entities().count();
    assert!(n >= 9, "{n}: {:?}", s.registro);
    assert_eq!(s.seleccion.len(), n);
    let linea = s
        .dibujo
        .doc
        .model_space_entities()
        .find_map(|e| match e {
            EntityType::Line(l) if l.common.layer == "PDF_FF0000" => Some(l.clone()),
            _ => None,
        })
        .unwrap();
    assert!((linea.start.x - (1000.0 + 72.0 * MM * 100.0)).abs() < 1e-6);
    assert!((linea.start.y - (2000.0 + 72.0 * MM * 100.0)).abs() < 1e-6);
    assert!(
        s.dibujo
            .doc
            .layers
            .get("PDF_Cotas")
            .is_some_and(|l| !l.is_visible())
    );
    // Un solo paso de deshacer.
    s.escribir("u");
    assert_eq!(s.dibujo.doc.model_space_entities().count(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn calibrar_con_escalaref() {
    let mut s = Sesion::nueva(Dibujo::nuevo());
    let h = s.dibujo.editar("X", |t| {
        t.anadir_en(esp::linea([10.0, 0.0], [20.0, 0.0]), "0")
            .unwrap()
    });
    s.seleccion = vec![h];
    s.escribir("escalaref");
    s.escribir("10,0");
    s.escribir("10,0");
    s.escribir("20,0");
    // Esos 10 son 250 de verdad.
    s.escribir("250");
    assert_eq!(s.activa(), None);
    let Some(EntityType::Line(l)) = s.dibujo.entidad(h) else {
        panic!()
    };
    assert!((l.start.x - 10.0).abs() < 1e-9 && (l.end.x - 260.0).abs() < 1e-9);
}

#[test]
fn pdf_roto_vacio_y_que_no_esta() {
    let dir = carpeta("malos");
    // Lo que no esta.
    let mut s = Sesion::nueva(Dibujo::nuevo());
    s.escribir("importarpdf");
    s.escribir(&dir.join("no-esta.pdf").to_string_lossy());
    assert_eq!(s.activa(), None);
    assert!(s.registro.iter().any(|r| r.contains("No se pudo importar")));
    // Basura con cabecera de PDF.
    let roto = dir.join("roto.pdf");
    std::fs::write(&roto, b"%PDF-1.7\n\x00\x01 basura 3 0 obj << /Type /Nada").unwrap();
    assert!(pdf::importar::abrir(&roto).is_err());
    // Un flujo comprimido danado: la pagina no se lee pero no se cuelga.
    let danado = pdf_a_mano(&[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R >>".to_vec(),
        flujo("/Filter /FlateDecode", b"esto no es zlib"),
    ]);
    let doc = Documento::leer(&danado).unwrap();
    assert!(pdf::importar::importar(&doc, 1, Default::default()).is_err());
    assert!(pdf::importar::importar(&doc, 2, Default::default()).is_err());
    // Una pagina vacia.
    let vacio = dir.join("vacio.pdf");
    std::fs::write(
        &vacio,
        pdf_a_mano(&[
            b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] >>".to_vec(),
        ]),
    )
    .unwrap();
    s.escribir("importarpdf");
    s.escribir(&vacio.to_string_lossy());
    s.escribir("");
    s.escribir("");
    assert_eq!(s.activa(), None);
    assert!(
        s.registro
            .iter()
            .any(|r| r.contains("No hay nada que importar"))
    );
    assert_eq!(s.dibujo.doc.model_space_entities().count(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn un_pdf_enorme_se_corta_y_avisa() {
    let mut c = String::new();
    for i in 0..5000 {
        c.push_str(&format!("{i} 0 m {i} 10 l S\n"));
    }
    let d = pdf_a_mano(&[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R >>".to_vec(),
        flujo("", c.as_bytes()),
    ]);
    let doc = Documento::leer(&d).unwrap();
    let topes = pdf::importar::Topes {
        operadores: 1_000_000,
        piezas: 100,
    };
    let imp = pdf::importar::importar(&doc, 1, topes).unwrap();
    assert_eq!(imp.figuras.len(), 100);
    assert!(imp.avisos.iter().any(|a| a.contains("muy grande")));
    let topes = pdf::importar::Topes {
        operadores: 30,
        piezas: 100,
    };
    let imp = pdf::importar::importar(&doc, 1, topes).unwrap();
    assert!(imp.figuras.len() <= 10);
}

/// Un dibujo con lineas, un circulo, un arco, un texto y un relleno.
fn dibujo_de_prueba() -> Dibujo {
    let mut d = Dibujo::nuevo();
    d.editar("X", |t| {
        t.anadir_en(esp::linea([0.0, 0.0], [100.0, 0.0]), esp::CAPA_MUROS);
        t.anadir_en(esp::linea([100.0, 0.0], [100.0, 50.0]), esp::CAPA_MUROS);
        t.anadir_en(esp::linea([0.0, 50.0], [100.0, 50.0]), esp::CAPA_MUROS);
        t.anadir_en(esp::circulo([50.0, 25.0], 10.0), esp::CAPA_EJES);
        t.anadir_en(
            esp::texto("PLANTA", [10.0, 60.0], 5.0, 0.0, false),
            esp::CAPA_TEXTOS,
        );
        t.anadir_en(
            esp::relleno([0.0, 0.0], [5.0, 0.0], [0.0, 5.0]),
            esp::CAPA_COTAS,
        );
    });
    d
}

#[test]
fn exportar_a_pdf_e_importar_de_nuevo() {
    let d = dibujo_de_prueba();
    let esc = escena::escena(&d.doc, Que::Todo, None);
    let caja = esc.caja().unwrap();
    let hoja = pdf::exportar::hoja_para(caja, Papel::Ajustar, Some(1.0));
    let bytes = pdf::exportar::escribir(&esc, &[hoja], &OpcionesPdf::default(), "prueba");
    let doc = Documento::leer(&bytes).unwrap();
    let imp = pdf::importar::importar(&doc, 1, Default::default()).unwrap();
    // De vuelta en el sitio: la esquina de la zona cae en el margen.
    let origen = [
        caja[0] - pdf::exportar::MARGEN,
        caja[1] - pdf::exportar::MARGEN,
    ];
    let r = formatos::a_entidades(&imp, "PDF_", origen, 1.0);
    let lineas: Vec<_> = r
        .entidades
        .iter()
        .filter(|(e, c)| matches!(e, EntityType::Line(_)) && c == "PDF_A-MUROS")
        .collect();
    assert_eq!(lineas.len(), 3, "{:?}", r.capas);
    let EntityType::Line(l) = &lineas[0].0 else {
        panic!()
    };
    assert!(
        geo::dist([l.start.x, l.start.y], [0.0, 0.0]) < 1e-3,
        "{l:?}"
    );
    assert!(geo::dist([l.end.x, l.end.y], [100.0, 0.0]) < 1e-3);
    let circulo = r
        .entidades
        .iter()
        .find_map(|(e, _)| match e {
            EntityType::Circle(c) => Some(c.clone()),
            _ => None,
        })
        .expect("el circulo vuelve");
    assert!((circulo.radius - 10.0).abs() < 0.01);
    assert!(geo::dist([circulo.center.x, circulo.center.y], [50.0, 25.0]) < 0.01);
    let texto = imp.rotulos.iter().find(|t| t.texto == "PLANTA").unwrap();
    assert_eq!(texto.grupo.as_deref(), Some(esp::CAPA_TEXTOS));
    assert!((texto.alto - 5.0).abs() < 0.2, "{}", texto.alto);
    assert_eq!(
        imp.figuras.iter().filter(|f| f.relleno.is_some()).count(),
        1
    );
    // En rayas: no hay texto, pero si muchas lineas mas.
    let op = OpcionesPdf {
        trazos: true,
        monocromo: true,
        sin_capas: true,
    };
    let bytes = pdf::exportar::escribir(&esc, &[hoja], &op, "prueba");
    let imp2 =
        pdf::importar::importar(&Documento::leer(&bytes).unwrap(), 1, Default::default()).unwrap();
    assert!(imp2.rotulos.is_empty());
    assert!(imp2.figuras.len() > imp.figuras.len());
    assert!(imp2.figuras.iter().all(|f| f.grupo.is_none()));
    assert!(
        imp2.figuras
            .iter()
            .all(|f| f.trazo.is_none_or(|t| t.0 == [0, 0, 0]))
    );
}

#[test]
fn la_orden_exportarpdf_por_laminas_y_por_ventana() {
    let dir = carpeta("orden-exp");
    let mut d = dibujo_de_prueba();
    // Dos laminas A3 a 1/1 (el dibujo en mm), una al lado de la otra.
    for x in [0.0, 1000.0] {
        let v = esp::lamina(
            [x - 100.0, -100.0],
            420.0,
            297.0,
            1.0,
            1.0,
            &Default::default(),
        );
        d.editar("LAMINA", |t| {
            for e in v {
                t.anadir_en(e, esp::CAPA_LAMINA);
            }
        });
    }
    let mut s = Sesion::nueva(d);
    s.ajustes.escala_lamina = 1.0;
    s.escribir("exportpdf");
    s.escribir("l");
    let Some(PedidoApp::Archivo { guardar: true, .. }) = s.memoria.pedidos.pop() else {
        panic!("pide donde guardar")
    };
    let ruta = dir.join("laminas");
    s.escribir(&ruta.to_string_lossy());
    assert_eq!(s.activa(), None, "{:?}", s.registro);
    let pdf_ruta = dir.join("laminas.pdf");
    let doc = pdf::importar::abrir(&pdf_ruta).unwrap();
    let paginas = doc.paginas();
    assert_eq!(paginas.len(), 2);
    let c = paginas[0].caja;
    assert!(((c[2] - c[0]) * MM - 420.0).abs() < 0.01);
    drop(paginas);
    // Por ventana, en A4 a 1:2 y monocromo.
    s.escribir("exportpdf");
    s.escribir("p");
    s.escribir("a4");
    s.escribir("e");
    s.escribir("2");
    s.escribir("m");
    s.escribir("-10,-10");
    s.escribir("110,70");
    s.memoria.pedidos.clear();
    let ruta = dir.join("ventana.pdf");
    s.escribir(&ruta.to_string_lossy());
    assert_eq!(s.activa(), None, "{:?}", s.registro);
    let doc = pdf::importar::abrir(&ruta).unwrap();
    let c = doc.paginas()[0].caja;
    assert!(((c[2] - c[0]) * MM - 297.0).abs() < 0.01);
    let imp = pdf::importar::importar(&doc, 1, Default::default()).unwrap();
    assert!(
        imp.figuras
            .iter()
            .all(|f| f.trazo.is_none_or(|t| t.0 == [0, 0, 0]))
    );
    // A 1:2 la linea de 100 mide 50 mm en el papel.
    let larga = imp.figuras.iter().find_map(|f| match &f.partes[0] {
        Geometria::Polilinea { puntos, .. }
            if puntos.len() == 2 && (geo::dist(puntos[0], puntos[1]) - 50.0).abs() < 1e-3 =>
        {
            Some(())
        }
        _ => None,
    });
    assert!(larga.is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn exportar_a_svg_e_importar_de_nuevo() {
    let d = dibujo_de_prueba();
    let esc = escena::escena(&d.doc, Que::Todo, None);
    let caja = esc.caja().unwrap();
    let texto = svg::escribir(&esc, caja, 1.0, 1.0, false);
    assert!(texto.contains("inkscape:label=\"A-MUROS\""));
    let imp = svg::importar(&texto).unwrap();
    let r = formatos::a_entidades(&imp, "SVG_", [caja[0], caja[1]], 1.0);
    let lineas: Vec<_> = r
        .entidades
        .iter()
        .filter_map(|(e, c)| match e {
            EntityType::Line(l) if c == "SVG_A-MUROS" => Some(l.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(lineas.len(), 3);
    assert!(geo::dist([lineas[0].start.x, lineas[0].start.y], [0.0, 0.0]) < 1e-3);
    assert!(geo::dist([lineas[0].end.x, lineas[0].end.y], [100.0, 0.0]) < 1e-3);
    let c = r
        .entidades
        .iter()
        .find_map(|(e, _)| match e {
            EntityType::Circle(c) => Some(c.clone()),
            _ => None,
        })
        .unwrap();
    assert!((c.radius - 10.0).abs() < 1e-6);
    assert!(geo::dist([c.center.x, c.center.y], [50.0, 25.0]) < 1e-6);
    let t = r
        .entidades
        .iter()
        .find_map(|(e, _)| match e {
            EntityType::Text(t) => Some(t.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(t.value, "PLANTA");
    assert!((t.height - 5.0).abs() < 1e-3, "{}", t.height);
    assert!(geo::dist([t.insertion_point.x, t.insertion_point.y], [10.0, 60.0]) < 1e-3);
    assert!(
        r.entidades
            .iter()
            .any(|(e, _)| matches!(e, EntityType::Hatch(_)))
    );
}

#[test]
fn las_ordenes_de_svg() {
    let dir = carpeta("orden-svg");
    let mut s = Sesion::nueva(dibujo_de_prueba());
    s.escribir("exportsvg");
    s.escribir("");
    assert!(matches!(
        s.memoria.pedidos.pop(),
        Some(PedidoApp::Archivo { guardar: true, .. })
    ));
    let ruta = dir.join("plano.svg");
    s.escribir(&ruta.to_string_lossy());
    assert_eq!(s.activa(), None, "{:?}", s.registro);
    let mut s2 = Sesion::nueva(Dibujo::nuevo());
    s2.escribir("importsvg");
    s2.escribir(&ruta.to_string_lossy());
    s2.escribir("");
    s2.escribir("0,0");
    assert_eq!(s2.activa(), None, "{:?}", s2.registro);
    assert_eq!(
        s2.dibujo.doc.model_space_entities().count(),
        6,
        "{:?}",
        s2.registro
    );
    assert!(s2.dibujo.doc.layers.contains("SVG_A-EJES"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn imagen_guardarversion_y_wbloque_con_sus_ordenes() {
    let dir = carpeta("ordenes");
    let foto = dir.join("foto.bmp");
    let mut bmp = b"BM".to_vec();
    bmp.resize(54, 0);
    bmp[18..22].copy_from_slice(&40u32.to_le_bytes());
    bmp[22..26].copy_from_slice(&20u32.to_le_bytes());
    std::fs::write(&foto, &bmp).unwrap();
    let mut s = Sesion::nueva(Dibujo::nuevo());
    s.escribir("imageattach");
    s.escribir(&foto.to_string_lossy());
    s.escribir("0,0");
    s.escribir("200");
    s.escribir("90");
    assert_eq!(s.activa(), None, "{:?}", s.registro);
    let Some(EntityType::RasterImage(im)) = s.dibujo.doc.model_space_entities().next().cloned()
    else {
        panic!()
    };
    assert!((im.width() - 200.0).abs() < 1e-9 && (im.height() - 100.0).abs() < 1e-9);
    assert!(im.u_vector.x.abs() < 1e-9 && im.u_vector.y > 0.0);
    // Se elige por su marco.
    assert_eq!(s.dibujo.indice().len(), 1);
    // GUARDARVERSION 2004 y guardar.
    s.escribir("guardarversion");
    s.escribir("2004");
    assert_eq!(s.dibujo.doc.version, opencadcodec::DxfVersion::AC1018);
    assert!(s.pide_texto());
    let ruta = dir.join("v2004");
    s.escribir(&ruta.to_string_lossy());
    assert_eq!(s.activa(), None, "{:?}", s.registro);
    let d = Dibujo::abrir(&dir.join("v2004.dwg")).unwrap();
    assert_eq!(d.doc.version, opencadcodec::DxfVersion::AC1018);
    assert_eq!(d.doc.model_space_entities().count(), 1);
    // Solo la version (Esc en el dialogo).
    s.escribir("guardarversion");
    s.escribir("2010");
    s.escape();
    assert_eq!(s.dibujo.doc.version, opencadcodec::DxfVersion::AC1024);
    // W con lo elegido.
    let hs = s.dibujo.editar("X", |t| {
        vec![
            t.anadir_en(esp::linea([0.0, 0.0], [1.0, 1.0]), "A-MUROS")
                .unwrap(),
            t.anadir_en(esp::linea([5.0, 0.0], [6.0, 1.0]), "A-EJES")
                .unwrap(),
        ]
    });
    s.seleccion = vec![hs[1]];
    s.escribir("w");
    s.escribir("5,0");
    assert!(s.pide_texto());
    let parte = dir.join("parte.dxf");
    s.escribir(&parte.to_string_lossy());
    assert_eq!(s.activa(), None, "{:?}", s.registro);
    let d = Dibujo::abrir(&parte).unwrap();
    assert_eq!(d.doc.model_space_entities().count(), 1);
    assert_eq!(d.doc.version, opencadcodec::DxfVersion::AC1024);
    assert_eq!(
        d.doc.model_space_entities().next().unwrap().common().layer,
        "A-EJES"
    );
    // Una imagen que no es imagen.
    let falsa = dir.join("falsa.png");
    std::fs::write(&falsa, b"no soy png").unwrap();
    s.escribir("imagen");
    s.escribir(&falsa.to_string_lossy());
    assert_eq!(s.activa(), None);
    assert!(s.registro.iter().any(|r| r.contains("No se pudo adjuntar")));
    let _ = std::fs::remove_dir_all(&dir);
}
