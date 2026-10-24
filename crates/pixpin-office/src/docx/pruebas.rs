//! Pruebas con documentos fabricados en la propia prueba.

use std::io::Write;
use std::sync::Arc;

use super::*;
use crate::medidor::MedidorFijo;
use crate::pagina::Elemento;

/// Un ZIP con las partes dadas (ruta, contenido).
pub fn zip(partes: &[(&str, &str)]) -> Arc<[u8]> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let op = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (ruta, texto) in partes {
            z.start_file(*ruta, op).unwrap();
            z.write_all(texto.as_bytes()).unwrap();
        }
        z.finish().unwrap();
    }
    Arc::from(buf.into_inner())
}

const W: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#;

fn documento(cuerpo: &str, extra: &[(&str, &str)]) -> DocumentoWord {
    let doc =
        format!(r#"<?xml version="1.0"?><w:document {W}><w:body>{cuerpo}</w:body></w:document>"#);
    let mut partes = vec![("word/document.xml", doc.as_str())];
    partes.extend_from_slice(extra);
    let p = Paquete::de_bytes(zip(&partes)).unwrap();
    DocumentoWord::de_paquete(p).unwrap()
}

fn textos(p: &Pagina) -> Vec<String> {
    p.elementos
        .iter()
        .filter_map(|e| match e {
            Elemento::Texto(t) => Some(t.texto.clone()),
            _ => None,
        })
        .collect()
}

const SECCION_PEQUENA: &str = r#"<w:sectPr><w:pgSz w:w="2880" w:h="2880"/><w:pgMar w:top="720" w:right="720" w:bottom="720" w:left="720" w:header="0" w:footer="0"/></w:sectPr>"#;

#[test]
fn un_documento_vacio_tiene_una_pagina_en_blanco() {
    let mut d = documento("", &[]);
    let p = d.pagina(0, &MedidorFijo::default()).unwrap();
    assert_eq!((p.ancho, p.alto), (612.0, 792.0));
    assert!(d.terminado());
    assert_eq!(d.paginas_listas(), 1);
}

#[test]
fn el_tamano_y_los_margenes_vienen_de_la_seccion() {
    let mut d = documento(
        &format!(r#"<w:p><w:r><w:t>Hola</w:t></w:r></w:p>{SECCION_PEQUENA}"#),
        &[],
    );
    let p = d.pagina(0, &MedidorFijo::default()).unwrap();
    assert_eq!((p.ancho, p.alto), (144.0, 144.0));
    let Some(Elemento::Texto(t)) = p.elementos.first() else {
        panic!("{:?}", p.elementos)
    };
    assert_eq!(t.texto, "Hola");
    assert_eq!(t.x, 36.0, "el texto empieza en el margen");
    // Letra por defecto 10 pt: sube 8.
    assert_eq!(t.y, 36.0 + 8.0);
}

#[test]
fn el_texto_que_no_cabe_pasa_a_la_pagina_siguiente() {
    // Caja de 72 x 72 puntos; renglones de 10 pt: caben 7.
    let parrafos: String = (0..20)
        .map(|i| format!("<w:p><w:r><w:t>p{i}</w:t></w:r></w:p>"))
        .collect();
    let mut d = documento(&format!("{parrafos}{SECCION_PEQUENA}"), &[]);
    let m = MedidorFijo::default();
    let p0 = d.pagina(0, &m).unwrap();
    assert_eq!(d.paginas_listas(), 1, "solo se maqueta lo que hace falta");
    assert!(!d.terminado());
    assert_eq!(textos(&p0).len(), 7);
    d.maquetar_todo(&m);
    assert_eq!(d.paginas_listas(), 3);
    assert_eq!(
        textos(&d.pagina(2, &m).unwrap()),
        vec!["p14", "p15", "p16", "p17", "p18", "p19"]
    );
}

#[test]
fn el_salto_de_pagina_empieza_otra() {
    let mut d = documento(
        &format!(
            r#"<w:p><w:r><w:t>a</w:t><w:br w:type="page"/><w:t>b</w:t></w:r></w:p>{SECCION_PEQUENA}"#
        ),
        &[],
    );
    let m = MedidorFijo::default();
    d.maquetar_todo(&m);
    assert_eq!(d.paginas_listas(), 2);
    assert_eq!(textos(&d.pagina(1, &m).unwrap()), vec!["b"]);
}

#[test]
fn los_estilos_y_la_numeracion_se_aplican() {
    let estilos = format!(
        r#"<w:styles {W}><w:style w:type="paragraph" w:styleId="T1"><w:rPr><w:b/><w:sz w:val="40"/></w:rPr></w:style></w:styles>"#
    );
    let numeracion = format!(
        r#"<w:numbering {W}><w:abstractNum w:abstractNumId="1"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/><w:pPr><w:ind w:left="720" w:hanging="360"/></w:pPr></w:lvl></w:abstractNum><w:num w:numId="3"><w:abstractNumId w:val="1"/></w:num></w:numbering>"#
    );
    let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="r1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/><Relationship Id="r2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/></Relationships>"#;
    let cuerpo = r#"<w:p><w:pPr><w:pStyle w:val="T1"/></w:pPr><w:r><w:t>Titulo</w:t></w:r></w:p>
        <w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="3"/></w:numPr></w:pPr><w:r><w:t>uno</w:t></w:r></w:p>
        <w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="3"/></w:numPr></w:pPr><w:r><w:t>dos</w:t></w:r></w:p>"#;
    let mut d = documento(
        cuerpo,
        &[
            ("word/styles.xml", estilos.as_str()),
            ("word/numbering.xml", numeracion.as_str()),
            ("word/_rels/document.xml.rels", rels),
        ],
    );
    let p = d.pagina(0, &MedidorFijo::default()).unwrap();
    let t: Vec<&crate::pagina::Texto> = p
        .elementos
        .iter()
        .filter_map(|e| {
            if let Elemento::Texto(t) = e {
                Some(t)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(t[0].texto, "Titulo");
    assert!(t[0].letra.negrita);
    assert_eq!(t[0].letra.tam, 20.0);
    assert_eq!(t[1].texto, "1.");
    assert_eq!(t[1].x, 72.0 + 18.0, "el numero en la sangria francesa");
    assert_eq!(t[2].texto, "uno");
    assert_eq!(t[2].x, 72.0 + 36.0, "el texto en el tope de la sangria");
    assert_eq!(t[3].texto, "2.");
}

#[test]
fn una_tabla_pone_cada_celda_en_su_columna() {
    let cuerpo = r#"<w:tbl><w:tblPr><w:tblBorders><w:top w:val="single" w:sz="8"/><w:insideV w:val="single" w:sz="4"/></w:tblBorders></w:tblPr>
        <w:tblGrid><w:gridCol w:w="2000"/><w:gridCol w:w="3000"/></w:tblGrid>
        <w:tr><w:tc><w:p><w:r><w:t>A</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:shd w:fill="FF0000"/></w:tcPr><w:p><w:r><w:t>B</w:t></w:r></w:p></w:tc></w:tr>
        <w:tr><w:tc><w:tcPr><w:gridSpan w:val="2"/></w:tcPr><w:p><w:r><w:t>C</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#;
    let mut d = documento(cuerpo, &[]);
    let p = d.pagina(0, &MedidorFijo::default()).unwrap();
    let pos = |s: &str| {
        p.elementos
            .iter()
            .find_map(|e| {
                if let Elemento::Texto(t) = e {
                    (t.texto == s).then_some((t.x, t.y))
                } else {
                    None
                }
            })
            .unwrap()
    };
    // Sin modo de compatibilidad 15, el borde sale 5,4 pt hacia fuera y el
    // texto queda en el margen.
    assert_eq!(pos("A").0, 72.0);
    assert_eq!(pos("B").0, 72.0 + 100.0);
    assert_eq!(pos("C").0, 72.0);
    assert!(pos("C").1 > pos("A").1);
    assert!(
        p.elementos
            .iter()
            .any(|e| matches!(e, Elemento::Rect { relleno: Some(crate::pagina::Relleno::Solido(c)), .. } if *c == Color::rgb(255, 0, 0)))
    );
    assert!(
        p.elementos
            .iter()
            .any(|e| matches!(e, Elemento::Linea { .. }))
    );
}

#[test]
fn una_fila_larga_se_parte_entre_paginas() {
    let parrafos: String = (0..15)
        .map(|i| format!("<w:p><w:r><w:t>c{i}</w:t></w:r></w:p>"))
        .collect();
    let cuerpo = format!(
        r#"<w:tbl><w:tblGrid><w:gridCol w:w="1000"/></w:tblGrid><w:tr><w:tc>{parrafos}</w:tc></w:tr></w:tbl>{SECCION_PEQUENA}"#
    );
    let mut d = documento(&cuerpo, &[]);
    let m = MedidorFijo::default();
    d.maquetar_todo(&m);
    assert_eq!(d.paginas_listas(), 3);
    assert_eq!(textos(&d.pagina(0, &m).unwrap()).len(), 7);
}

#[test]
fn el_numero_de_pagina_del_pie_cuenta() {
    let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rP" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/></Relationships>"#;
    let pie = format!(
        r#"<w:ftr {W}><w:p><w:r><w:t>Pag </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> PAGE </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>9</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p></w:ftr>"#
    );
    let cuerpo = r#"<w:p><w:r><w:t>a</w:t><w:br w:type="page"/><w:t>b</w:t></w:r></w:p>
        <w:sectPr><w:footerReference w:type="default" r:id="rP"/><w:pgSz w:w="2880" w:h="2880"/><w:pgMar w:top="720" w:right="720" w:bottom="720" w:left="720" w:header="0" w:footer="200"/></w:sectPr>"#;
    let mut d = documento(
        cuerpo,
        &[
            ("word/_rels/document.xml.rels", rels),
            ("word/footer1.xml", pie.as_str()),
        ],
    );
    let m = MedidorFijo::default();
    let p1 = d.pagina(1, &m).unwrap();
    let t = textos(&p1);
    assert!(t.contains(&"2".to_string()), "{t:?}");
    assert!(
        !t.contains(&"9".to_string()),
        "el resultado guardado no se ensena"
    );
}

#[test]
fn un_documento_roto_no_entra_en_panico() {
    let base = format!(
        r#"<w:p><w:pPr><w:numPr><w:numId w:val="7"/></w:numPr><w:spacing w:line="-5" w:lineRule="exact"/></w:pPr><w:r><w:t>a</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:tcPr><w:gridSpan w:val="99"/><w:vMerge/></w:tcPr><w:p/></w:tc></w:tr></w:tbl>{SECCION_PEQUENA}"#
    );
    let doc = format!(r#"<w:document {W}><w:body>{base}</w:body></w:document>"#);
    let m = MedidorFijo::default();
    for corte in (0..doc.len()).step_by(7) {
        let mut t = doc[..corte].to_string();
        while !t.is_char_boundary(t.len()) {
            t.pop();
        }
        let p = Paquete::de_bytes(zip(&[("word/document.xml", t.as_str())])).unwrap();
        if let Ok(mut d) = DocumentoWord::de_paquete(p) {
            d.maquetar_todo(&m);
        }
    }
}

/// Hay algun `Elemento::Efecto` en la lista (a cualquier profundidad).
fn hay_efecto(els: &[Elemento]) -> bool {
    els.iter().any(|e| match e {
        Elemento::Efecto { .. } => true,
        Elemento::Grupo { elementos, .. } => hay_efecto(elementos),
        _ => false,
    })
}

#[test]
fn la_sombra_de_una_imagen_de_word_se_lee() {
    let foto = |efectos: &str| {
        format!(
            r#"<w:p><w:r><w:drawing><wp:inline xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"><wp:extent cx="635000" cy="635000"/><a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:blipFill><a:blip r:embed="rI"/></pic:blipFill><pic:spPr><a:prstGeom prst="rect"/>{efectos}</pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"#
        )
    };
    let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rI" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/a.png"/></Relationships>"#;
    let extra = [
        ("word/_rels/document.xml.rels", rels),
        ("word/media/a.png", "no es un png"),
    ];
    let m = MedidorFijo::default();
    let sombra = r#"<a:effectLst><a:outerShdw blurRad="50800" dist="38100" dir="2700000"><a:prstClr val="black"><a:alpha val="40000"/></a:prstClr></a:outerShdw></a:effectLst>"#;
    let mut d = documento(&foto(sombra), &extra);
    assert!(hay_efecto(&d.pagina(0, &m).unwrap().elementos));
    // Caso negativo: un effectLst vacio no pone nada.
    let mut d = documento(&foto("<a:effectLst/>"), &extra);
    let p = d.pagina(0, &m).unwrap();
    assert!(!hay_efecto(&p.elementos));
    assert!(!p.elementos.is_empty(), "la imagen sigue ahi");
}

/// Todos los textos de una lista de elementos, entrando en los grupos.
fn textos_hondos(els: &[Elemento], v: &mut Vec<String>) {
    for e in els {
        match e {
            Elemento::Texto(t) => v.push(t.texto.clone()),
            Elemento::Grupo { elementos, .. } => textos_hondos(elementos, v),
            _ => {}
        }
    }
}

#[test]
fn un_grafico_en_linea_se_dibuja_y_el_nuevo_usa_su_respaldo() {
    const A: &str = r#"xmlns:wp="x" xmlns:a="y" xmlns:mc="z""#;
    let grafico = r#"<c:chartSpace xmlns:c="c"><c:chart><c:autoTitleDeleted val="1"/><c:plotArea><c:pieChart><c:varyColors val="1"/><c:ser><c:idx val="0"/><c:order val="0"/><c:cat><c:strLit><c:ptCount val="2"/><c:pt idx="0"><c:v>Si</c:v></c:pt><c:pt idx="1"><c:v>No</c:v></c:pt></c:strLit></c:cat><c:val><c:numLit><c:ptCount val="2"/><c:pt idx="0"><c:v>3</c:v></c:pt><c:pt idx="1"><c:v>1</c:v></c:pt></c:numLit></c:val><c:dLbls><c:showPercent val="1"/></c:dLbls></c:ser></c:pieChart></c:plotArea><c:legend><c:legendPos val="r"/></c:legend></c:chart></c:chartSpace>"#;
    let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rC" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart" Target="charts/chart1.xml"/></Relationships>"#;
    let cuerpo = format!(
        r#"<w:p {A}><w:r><w:drawing><wp:inline><wp:extent cx="2540000" cy="1905000"/><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart xmlns:c="c" r:id="rC"/></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>
        <w:p {A}><w:r><mc:AlternateContent><mc:Choice Requires="cx1"><w:drawing><wp:inline><wp:extent cx="2540000" cy="635000"/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/drawing/2014/chartex"/></a:graphic></wp:inline></w:drawing></mc:Choice><mc:Fallback><w:pict><v:rect xmlns:v="v" style="width:100pt;height:20pt"><v:textbox><w:txbxContent><w:p><w:r><w:t>respaldo</w:t></w:r></w:p></w:txbxContent></v:textbox></v:rect></w:pict></mc:Fallback></mc:AlternateContent></w:r></w:p>"#
    );
    let mut d = documento(
        &cuerpo,
        &[
            ("word/charts/chart1.xml", grafico),
            ("word/_rels/document.xml.rels", rels),
        ],
    );
    let p = d.pagina(0, &MedidorFijo::default()).unwrap();
    let mut t = Vec::new();
    textos_hondos(&p.elementos, &mut t);
    assert!(
        t.contains(&"75%".to_string()) && t.contains(&"Si".to_string()),
        "{t:?}"
    );
    assert!(
        t.contains(&"respaldo".to_string()),
        "el grafico nuevo deja su respaldo: {t:?}"
    );
}

#[test]
fn un_grafico_sin_su_parte_deja_un_aviso() {
    const A: &str = r#"xmlns:wp="x" xmlns:a="y""#;
    let cuerpo = format!(
        r#"<w:p {A}><w:r><w:drawing><wp:inline><wp:extent cx="2540000" cy="1905000"/><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart xmlns:c="c" r:id="rNo"/></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"#
    );
    let mut d = documento(&cuerpo, &[]);
    let p = d.pagina(0, &MedidorFijo::default()).unwrap();
    let mut t = Vec::new();
    textos_hondos(&p.elementos, &mut t);
    assert!(t.iter().any(|x| x.starts_with("Gráfico")), "{t:?}");
}

// ---------------------------------------------------------------------
// Dibujos flotantes que apartan el texto
// ---------------------------------------------------------------------

/// Los textos de la pagina con su caja: (texto, x0, x1, linea base).
fn cajas_texto(p: &Pagina) -> Vec<(String, f32, f32, f32)> {
    p.elementos
        .iter()
        .filter_map(|e| match e {
            // Sin los espacios que cuelgan al final (5 pt cada uno).
            Elemento::Texto(t) => Some((
                t.texto.clone(),
                t.x,
                t.x + t.ancho - 5.0 * (t.texto.len() - t.texto.trim_end().len()) as f32,
                t.y,
            )),
            _ => None,
        })
        .collect()
}

/// Un parrafo con un dibujo anclado de 100 x 100 pt (`ajuste` es el
/// elemento `wp:wrap...`) y mucho texto, en una hoja carta (columna de 72 a
/// 540).
fn con_flotante(ajuste: &str, alin_h: &str, dist_l: &str) -> DocumentoWord {
    let texto = "palabra ".repeat(200);
    let cuerpo = format!(
        r#"<w:p xmlns:wp="wp"><w:r><w:drawing><wp:anchor distT="0" distB="0" distL="{dist_l}" distR="{dist_l}" behindDoc="0"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="column"><wp:align>{alin_h}</wp:align></wp:positionH><wp:positionV relativeFrom="paragraph"><wp:posOffset>0</wp:posOffset></wp:positionV><wp:extent cx="1270000" cy="1270000"/>{ajuste}</wp:anchor></w:drawing></w:r><w:r><w:t xml:space="preserve">{texto}</w:t></w:r></w:p>"#
    );
    documento(&cuerpo, &[])
}

#[test]
fn un_flotante_pequeno_a_la_derecha_acorta_los_renglones_de_su_altura() {
    // 100 pt de 468: antes pasaba por debajo del texto (solo apartaba si
    // ocupaba mas del 55 % de la columna).
    let mut d = con_flotante(
        r#"<wp:wrapSquare wrapText="bothSides"/>"#,
        "right",
        "114300",
    );
    let p = d.pagina(0, &MedidorFijo::default()).unwrap();
    let cajas = cajas_texto(&p);
    // El dibujo va de x 440 a 540 y de y 72 a 172; con 9 pt de distancia.
    let al_lado: Vec<_> = cajas.iter().filter(|c| c.3 - 8.0 < 172.0).collect();
    let debajo: Vec<_> = cajas.iter().filter(|c| c.3 - 8.0 >= 172.0).collect();
    assert!(al_lado.len() >= 9, "{al_lado:?}");
    for c in &al_lado {
        assert!(
            c.2 <= 440.0 - 9.0 + 0.5,
            "el texto no se mete bajo el dibujo ni en su distancia: {c:?}"
        );
    }
    assert!(
        debajo.iter().any(|c| c.2 > 500.0),
        "debajo del dibujo, otra vez a lo ancho"
    );
}

#[test]
fn sin_ajuste_el_texto_pasa_por_debajo_del_dibujo() {
    // Caso negativo: `wrapNone` (delante del texto) no aparta nada.
    let mut d = con_flotante("<wp:wrapNone/>", "right", "114300");
    let p = d.pagina(0, &MedidorFijo::default()).unwrap();
    let cajas = cajas_texto(&p);
    assert!(
        cajas.iter().any(|c| c.3 < 172.0 && c.2 > 450.0),
        "{:?}",
        &cajas[..3]
    );
}

#[test]
fn arriba_y_abajo_deja_libre_la_altura_del_dibujo() {
    let mut d = con_flotante("<wp:wrapTopAndBottom/>", "center", "0");
    let p = d.pagina(0, &MedidorFijo::default()).unwrap();
    let cajas = cajas_texto(&p);
    assert!(
        cajas.iter().all(|c| c.3 - 8.0 >= 172.0 - 0.5),
        "{:?}",
        &cajas[..2]
    );
}

#[test]
fn a_los_dos_lados_el_renglon_se_parte_en_dos() {
    let mut d = con_flotante(r#"<wp:wrapSquare wrapText="bothSides"/>"#, "center", "0");
    let p = d.pagina(0, &MedidorFijo::default()).unwrap();
    let cajas = cajas_texto(&p);
    // El dibujo va de 256 a 356.
    let primera_y = cajas[0].3;
    let renglon: Vec<_> = cajas
        .iter()
        .filter(|c| (c.3 - primera_y).abs() < 0.1)
        .collect();
    assert!(
        renglon.iter().any(|c| c.2 <= 256.5) && renglon.iter().any(|c| c.1 >= 355.5),
        "{renglon:?}"
    );
    assert!(
        renglon.iter().all(|c| c.2 <= 256.5 || c.1 >= 355.5),
        "{renglon:?}"
    );
    // Con `largest`, solo al lado con mas sitio.
    let mut d = con_flotante(r#"<wp:wrapSquare wrapText="right"/>"#, "center", "0");
    let p = d.pagina(0, &MedidorFijo::default()).unwrap();
    let cajas = cajas_texto(&p);
    assert!(
        cajas
            .iter()
            .filter(|c| c.3 - 8.0 < 172.0)
            .all(|c| c.1 >= 355.5),
        "{:?}",
        &cajas[..2]
    );
}

#[test]
fn el_ajuste_estrecho_sigue_el_poligono() {
    // Un triangulo con la punta arriba en medio: arriba el texto se acerca
    // mas que abajo.
    let poligono = r#"<wp:wrapTight wrapText="left"><wp:wrapPolygon edited="0"><wp:start x="10800" y="0"/><wp:lineTo x="21600" y="21600"/><wp:lineTo x="0" y="21600"/><wp:lineTo x="10800" y="0"/></wp:wrapPolygon></wp:wrapTight>"#;
    let mut d = con_flotante(poligono, "right", "0");
    let p = d.pagina(0, &MedidorFijo::default()).unwrap();
    let cajas = cajas_texto(&p);
    let arriba = cajas.iter().find(|c| c.3 - 8.0 < 80.0).unwrap();
    let abajo = cajas
        .iter()
        .find(|c| c.3 - 8.0 > 155.0 && c.3 - 8.0 < 172.0)
        .unwrap();
    assert!(
        arriba.2 > abajo.2 + 20.0,
        "arriba {arriba:?} abajo {abajo:?}"
    );
    assert!(abajo.2 <= 440.5);
    // Sin poligono, el estrecho es como el cuadrado.
    let mut d = con_flotante(r#"<wp:wrapTight wrapText="left"/>"#, "right", "0");
    let p = d.pagina(0, &MedidorFijo::default()).unwrap();
    assert!(
        cajas_texto(&p)
            .iter()
            .filter(|c| c.3 - 8.0 < 172.0)
            .all(|c| c.2 <= 440.5)
    );
}

// ---------------------------------------------------------------------
// Filas partidas, cabeceras repetidas, viudas y «conservar con el siguiente»
// ---------------------------------------------------------------------

#[test]
fn una_tabla_larga_repite_su_cabecera_y_parte_las_filas() {
    let mut filas = String::from(
        r#"<w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:p><w:r><w:t>CAB</w:t></w:r></w:p></w:tc></w:tr>"#,
    );
    for i in 0..4 {
        let ps: String = (0..4)
            .map(|k| format!("<w:p><w:r><w:t>f{i}l{k}</w:t></w:r></w:p>"))
            .collect();
        filas.push_str(&format!("<w:tr><w:tc>{ps}</w:tc></w:tr>"));
    }
    let cuerpo = format!(
        r#"<w:tbl><w:tblGrid><w:gridCol w:w="1000"/></w:tblGrid>{filas}</w:tbl>{SECCION_PEQUENA}"#
    );
    let mut d = documento(&cuerpo, &[]);
    let m = MedidorFijo::default();
    d.maquetar_todo(&m);
    assert!(d.paginas_listas() >= 3);
    for i in 0..d.paginas_listas() {
        let t = textos(&d.pagina(i, &m).unwrap());
        assert_eq!(
            t.first().map(String::as_str),
            Some("CAB"),
            "pagina {i}: {t:?}"
        );
    }
    // 7 renglones por pagina: CAB + 4 de f0 + 2 de f1 (la fila se parte).
    assert_eq!(
        textos(&d.pagina(0, &m).unwrap()),
        vec!["CAB", "f0l0", "f0l1", "f0l2", "f0l3", "f1l0", "f1l1"]
    );
}

#[test]
fn una_fila_que_no_se_parte_salta_entera_y_una_mas_alta_que_la_pagina_no_cuelga() {
    let fila = |n: usize, partir: bool| {
        let ps: String = (0..n)
            .map(|k| format!("<w:p><w:r><w:t>x{k}</w:t></w:r></w:p>"))
            .collect();
        let pr = if partir {
            ""
        } else {
            "<w:trPr><w:cantSplit/></w:trPr>"
        };
        format!("<w:tr>{pr}<w:tc>{ps}</w:tc></w:tr>")
    };
    // 5 renglones, luego una fila de 4 que no se parte: salta entera.
    let cuerpo = format!(
        r#"<w:p><w:r><w:t>a</w:t></w:r></w:p><w:p><w:r><w:t>b</w:t></w:r></w:p><w:p><w:r><w:t>c</w:t></w:r></w:p><w:p><w:r><w:t>d</w:t></w:r></w:p><w:p><w:r><w:t>e</w:t></w:r></w:p><w:tbl><w:tblGrid><w:gridCol w:w="1000"/></w:tblGrid>{}</w:tbl>{SECCION_PEQUENA}"#,
        fila(4, false)
    );
    let mut d = documento(&cuerpo, &[]);
    let m = MedidorFijo::default();
    d.maquetar_todo(&m);
    assert_eq!(
        textos(&d.pagina(1, &m).unwrap()),
        vec!["x0", "x1", "x2", "x3"]
    );
    // Una fila de 30 renglones que no se parte (mas alta que la pagina): se
    // pone entera y la paginacion sigue.
    let cuerpo = format!(
        r#"<w:tbl><w:tblGrid><w:gridCol w:w="1000"/></w:tblGrid>{}</w:tbl><w:p><w:r><w:t>fin</w:t></w:r></w:p>{SECCION_PEQUENA}"#,
        fila(30, false)
    );
    let mut d = documento(&cuerpo, &[]);
    d.maquetar_todo(&m);
    assert!(d.paginas_listas() <= 3, "{}", d.paginas_listas());
    let todos: Vec<String> = (0..d.paginas_listas())
        .flat_map(|i| textos(&d.pagina(i, &m).unwrap()))
        .collect();
    assert!(todos.contains(&"fin".to_string()));
}

#[test]
fn las_viudas_y_huerfanas_no_quedan_solas() {
    // 6 renglones y luego un parrafo de 3 (palabras de 40 pt en una columna
    // de 72): solo cabe 1 renglon; con control de viudas se va entero.
    let largo = "<w:p><w:r><w:t>aaaaaaa bbbbbbb ccccccc</w:t></w:r></w:p>";
    let antes: String = (0..6)
        .map(|i| format!("<w:p><w:r><w:t>p{i}</w:t></w:r></w:p>"))
        .collect();
    let mut d = documento(&format!("{antes}{largo}{SECCION_PEQUENA}"), &[]);
    let m = MedidorFijo::default();
    d.maquetar_todo(&m);
    assert_eq!(textos(&d.pagina(0, &m).unwrap()).len(), 6);
    assert_eq!(textos(&d.pagina(1, &m).unwrap()).len(), 3);
    // Caso negativo: sin control, el primer renglon se queda abajo.
    let suelto = "<w:p><w:pPr><w:widowControl w:val=\"0\"/></w:pPr><w:r><w:t>aaaaaaa bbbbbbb ccccccc</w:t></w:r></w:p>";
    let mut d = documento(&format!("{antes}{suelto}{SECCION_PEQUENA}"), &[]);
    d.maquetar_todo(&m);
    assert_eq!(textos(&d.pagina(0, &m).unwrap()).len(), 7);
}

#[test]
fn conservar_con_el_siguiente_lleva_el_titulo_con_su_parrafo() {
    let antes: String = (0..6)
        .map(|i| format!("<w:p><w:r><w:t>p{i}</w:t></w:r></w:p>"))
        .collect();
    let cuerpo = format!(
        r#"{antes}<w:p><w:pPr><w:keepNext/></w:pPr><w:r><w:t>Titulo</w:t></w:r></w:p><w:p><w:r><w:t>texto</w:t></w:r></w:p>{SECCION_PEQUENA}"#
    );
    let mut d = documento(&cuerpo, &[]);
    let m = MedidorFijo::default();
    d.maquetar_todo(&m);
    assert_eq!(textos(&d.pagina(1, &m).unwrap()), vec!["Titulo", "texto"]);
}

#[test]
fn conservar_con_el_siguiente_en_todo_el_documento_no_cuelga() {
    // Toda la cadena no cabe en una pagina: Word no hace caso y pagina
    // normal.
    let ps: String = (0..300)
        .map(|i| {
            format!(
                "<w:p><w:pPr><w:keepNext/><w:keepLines/></w:pPr><w:r><w:t>p{i}</w:t></w:r></w:p>"
            )
        })
        .collect();
    let mut d = documento(&format!("{ps}{SECCION_PEQUENA}"), &[]);
    let m = MedidorFijo::default();
    let t = std::time::Instant::now();
    d.maquetar_todo(&m);
    assert_eq!(d.paginas_listas(), 300usize.div_ceil(7));
    assert!(t.elapsed().as_secs_f32() < 5.0);
}

// ---------------------------------------------------------------------
// Formato condicional de los estilos de tabla
// ---------------------------------------------------------------------

#[test]
fn el_estilo_de_tabla_pinta_la_primera_fila_y_las_bandas_segun_tbl_look() {
    let estilos = format!(
        r#"<w:styles {W}><w:style w:type="table" w:styleId="Rejilla"><w:tblStylePr w:type="firstRow"><w:rPr><w:b/></w:rPr><w:tcPr><w:shd w:fill="0000FF"/></w:tcPr></w:tblStylePr><w:tblStylePr w:type="band1Horz"><w:tcPr><w:shd w:fill="DDDDDD"/></w:tcPr></w:tblStylePr><w:tblStylePr w:type="lastRow"><w:rPr><w:i/></w:rPr></w:tblStylePr></w:style></w:styles>"#
    );
    let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="r1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#;
    let tabla = |look: &str| {
        let filas: String = ["A", "B", "C", "D"]
            .iter()
            .map(|t| format!("<w:tr><w:tc><w:p><w:r><w:t>{t}</w:t></w:r></w:p></w:tc></w:tr>"))
            .collect();
        format!(
            r#"<w:tbl><w:tblPr><w:tblStyle w:val="Rejilla"/>{look}</w:tblPr><w:tblGrid><w:gridCol w:w="2000"/></w:tblGrid>{filas}</w:tbl>"#
        )
    };
    let extra = [
        ("word/styles.xml", estilos.as_str()),
        ("word/_rels/document.xml.rels", rels),
    ];
    let m = MedidorFijo::default();
    let ver = |look: &str| {
        let mut d = documento(&tabla(look), &extra);
        let p = d.pagina(0, &m).unwrap();
        let letra = |s: &str| {
            p.elementos
                .iter()
                .find_map(|e| {
                    if let Elemento::Texto(t) = e {
                        (t.texto == s).then(|| t.letra.clone())
                    } else {
                        None
                    }
                })
                .unwrap()
        };
        let fondos: Vec<Color> = p
            .elementos
            .iter()
            .filter_map(|e| match e {
                Elemento::Rect {
                    relleno: Some(crate::pagina::Relleno::Solido(c)),
                    ..
                } => Some(*c),
                _ => None,
            })
            .collect();
        (letra("A").negrita, letra("D").cursiva, fondos)
    };
    let (neg, cur, fondos) = ver(r#"<w:tblLook w:firstRow="1" w:lastRow="1" w:noHBand="0"/>"#);
    assert!(neg && cur);
    assert_eq!(
        fondos
            .iter()
            .filter(|c| **c == Color::rgb(0, 0, 255))
            .count(),
        1
    );
    // Filas de datos B, C, D: bandas 1, 2, 1.
    assert_eq!(
        fondos
            .iter()
            .filter(|c| **c == Color::rgb(0xDD, 0xDD, 0xDD))
            .count(),
        2
    );
    // Caso negativo: sin primera fila ni bandas en el tblLook.
    let (neg, cur, fondos) = ver(r#"<w:tblLook w:firstRow="0" w:lastRow="0" w:noHBand="1"/>"#);
    assert!(!neg && !cur);
    assert!(fondos.is_empty(), "{fondos:?}");
}
