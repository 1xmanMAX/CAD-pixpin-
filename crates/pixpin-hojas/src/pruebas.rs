//! Pruebas de punta a punta con libros pequenos montados en memoria (sin un
//! solo fichero binario en el repositorio).

use std::io::Write;

use crate::*;

#[test]
fn tablas_y_formato_condicional() {
    let hoja = br#"<worksheet><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Nombre</t></is></c><c r="B1" t="inlineStr"><is><t>Monto</t></is></c></row><row r="2"><c r="A2" t="inlineStr"><is><t>a</t></is></c><c r="B2"><v>5</v></c></row><row r="3"><c r="A3" t="inlineStr"><is><t>b</t></is></c><c r="B3"><v>50</v></c></row></sheetData><conditionalFormatting sqref="B2:B3"><cfRule type="cellIs" dxfId="0" priority="1" operator="greaterThan"><formula>10</formula></cfRule></conditionalFormatting><tableParts count="1"><tablePart r:id="rId1"/></tableParts></worksheet>"#;
    let rels = br#"<Relationships><Relationship Id="rId1" Type="http://x/table" Target="../tables/table1.xml"/></Relationships>"#;
    let tabla = br#"<table ref="A1:B3" headerRowCount="1"><tableStyleInfo name="TableStyleMedium2" showRowStripes="1"/></table>"#;
    let estilos = br#"<styleSheet><dxfs><dxf><font><b/><color rgb="FFFF0000"/></font></dxf></dxfs><cellXfs><xf/></cellXfs></styleSheet>"#;
    let z = zip_de(&[
        ("_rels/.rels", RELS),
        ("xl/workbook.xml", br#"<workbook><sheets><sheet name="H" r:id="rId1"/></sheets></workbook>"#),
        (
            "xl/_rels/workbook.xml.rels",
            br#"<Relationships><Relationship Id="rId1" Type="http://x/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://x/styles" Target="styles.xml"/></Relationships>"#,
        ),
        ("xl/styles.xml", estilos),
        ("xl/worksheets/sheet1.xml", hoja),
        ("xl/worksheets/_rels/sheet1.xml.rels", rels),
        ("xl/tables/table1.xml", tabla),
    ]);
    let l = Libro::desde_bytes(z, Regional::en_us()).unwrap();
    let mut h = l.abrir_hoja(0);
    h.cargar_todo();
    assert_eq!(h.cola.tablas.len(), 1);
    let t = &h.cola.tablas[0];
    let e = l.estilo_de_tabla(t).unwrap();
    assert_eq!(crate::tablas::formato_en(t, &e, 0, 0).negrita, Some(true));
    let alto = l.efecto_condicional(&h, 2, h.celda(2, 1).unwrap()).unwrap();
    assert_eq!(alto.dxf.color_fuente, Some(Rgb { r: 255, g: 0, b: 0 }));
    assert!(
        l.efecto_condicional(&h, 1, h.celda(1, 1).unwrap())
            .is_none()
    );
}

pub(crate) fn zip_de(partes: &[(&str, &[u8])]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let op = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (n, d) in partes {
        w.start_file(*n, op).unwrap();
        w.write_all(d).unwrap();
    }
    w.finish().unwrap().into_inner()
}

const RELS: &[u8] = br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#;

const LIBRO: &[u8] = br#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><workbookPr date1904="0"/><bookViews><workbookView activeTab="1"/></bookViews><sheets><sheet name="Datos" sheetId="1" r:id="rId1"/><sheet name="Oculta" sheetId="2" state="hidden" r:id="rId2"/><sheet name="Grande" sheetId="3" r:id="rId3"/></sheets></workbook>"#;

const LIBRO_RELS: &[u8] = br#"<Relationships><Relationship Id="rId1" Type="http://x/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://x/worksheet" Target="/xl/worksheets/sheet2.xml"/><Relationship Id="rId3" Type="http://x/worksheet" Target="worksheets/sheet3.xml"/><Relationship Id="rId4" Type="http://x/styles" Target="styles.xml"/><Relationship Id="rId5" Type="http://x/sharedStrings" Target="sharedStrings.xml"/></Relationships>"#;

const ESTILOS: &[u8] = br#"<styleSheet><numFmts><numFmt numFmtId="164" formatCode="yyyy-mm-dd"/></numFmts><fonts><font><sz val="11"/><name val="Calibri"/></font><font><b/><sz val="20"/><name val="Calibri"/></font></fonts><fills><fill><patternFill/></fill><fill><patternFill patternType="gray125"/></fill><fill><patternFill patternType="solid"><fgColor rgb="FFFFFF00"/></patternFill></fill></fills><borders><border/><border><top style="thin"/></border></borders><cellXfs><xf/><xf numFmtId="4"/><xf numFmtId="164"/><xf fontId="1" fillId="2" borderId="1"/></cellXfs></styleSheet>"#;

const SST: &[u8] = br#"<sst><si><t>Nombre</t></si><si><t>Total</t></si><si><r><rPr><b/></rPr><t>Ri</t></r><r><t>co</t></r></si></sst>"#;

const HOJA1: &[u8] = br#"<worksheet xmlns:r="r"><dimension ref="A1:D4"/><sheetViews><sheetView showGridLines="0" zoomScale="125"><pane xSplit="1" ySplit="2" topLeftCell="B3" state="frozen"/></sheetView></sheetViews><sheetFormatPr defaultRowHeight="15"/><cols><col min="1" max="1" width="20.7109375" customWidth="1"/><col min="3" max="4" hidden="1" width="5"/></cols>
<sheetData><row r="1" ht="30" customHeight="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="s"><v>1</v></c></row>
<row r="2"><c r="A2" t="inlineStr"><is><t>en l&#237;nea</t></is></c><c r="B2" s="1"><v>1234.5</v></c><c r="C2" t="b"><v>1</v></c><c r="D2" t="e"><v>#DIV/0!</v></c></row>
<row r="3" hidden="1"><c r="A3" t="str"><f>A1</f><v>calc</v></c><c r="B3" s="2"><v>45000</v></c></row>
<row r="4"><c r="A4" s="3"><v>7</v></c><c r="B4" t="s"><v>2</v></c><c t="n"><v>8</v></c></row></sheetData>
<mergeCells count="1"><mergeCell ref="A1:A2"/></mergeCells><hyperlinks><hyperlink ref="B1" r:id="rId9"/><hyperlink ref="B2" location="Datos!A1"/></hyperlinks></worksheet>"#;

const HOJA1_RELS: &[u8] =
    br#"<Relationships><Relationship Id="rId9" Type="http://x/hyperlink" Target="https://example.com/" TargetMode="External"/></Relationships>"#;

fn hoja_grande(filas: u32) -> Vec<u8> {
    let mut s = String::from("<worksheet><sheetData>");
    for r in 1..=filas {
        s.push_str(&format!(
            "<row r=\"{r}\"><c r=\"A{r}\"><v>{r}</v></c><c r=\"B{r}\" t=\"s\"><v>0</v></c></row>"
        ));
    }
    s.push_str("</sheetData><mergeCells><mergeCell ref=\"C1:D1\"/></mergeCells></worksheet>");
    s.into_bytes()
}

fn libro_de_prueba(grande: u32) -> Vec<u8> {
    let g = hoja_grande(grande);
    zip_de(&[
        ("_rels/.rels", RELS),
        ("xl/workbook.xml", LIBRO),
        ("xl/_rels/workbook.xml.rels", LIBRO_RELS),
        ("xl/styles.xml", ESTILOS),
        ("xl/sharedStrings.xml", SST),
        ("xl/worksheets/sheet1.xml", HOJA1),
        ("xl/worksheets/_rels/sheet1.xml.rels", HOJA1_RELS),
        (
            "xl/worksheets/sheet2.xml",
            b"<worksheet><sheetData/></worksheet>",
        ),
        ("xl/worksheets/sheet3.xml", &g),
    ])
}

fn texto(l: &Libro, h: &Hoja, f: u32, c: u32) -> String {
    h.celda(f, c)
        .and_then(|x| l.contenido(h, x))
        .map(|x| x.mostrado.texto)
        .unwrap_or_default()
}

#[test]
fn abre_un_libro_completo() {
    let l = Libro::desde_bytes(libro_de_prueba(10), Regional::en_us()).unwrap();
    assert_eq!(l.hojas.len(), 3);
    assert_eq!(l.hojas[1].visibilidad, Visibilidad::Oculta);
    assert_eq!(l.hojas[1].ruta, "xl/worksheets/sheet2.xml");
    // La activa es la 1, pero esta oculta: se abre la primera visible.
    assert_eq!(l.hoja_activa, 0);
    let mut h = l.abrir_hoja(0);
    assert!(!h.completa);
    let v = h.vista().clone();
    assert!(!v.cuadricula);
    assert_eq!(v.zoom, 125);
    assert_eq!(
        (
            v.filas_fijas,
            v.columnas_fijas,
            v.primera_fila,
            v.primera_columna
        ),
        (2, 1, 2, 1)
    );
    h.cargar_todo();
    assert!(h.completa);
    assert_eq!(texto(&l, &h, 0, 0), "Nombre");
    assert_eq!(texto(&l, &h, 1, 0), "en línea");
    assert_eq!(texto(&l, &h, 1, 1), "1,234.50");
    assert_eq!(texto(&l, &h, 1, 2), "TRUE");
    assert_eq!(texto(&l, &h, 1, 3), "#DIV/0!");
    assert_eq!(texto(&l, &h, 2, 0), "calc");
    assert_eq!(texto(&l, &h, 2, 1), "2023-03-15");
    // Celda sin `r`: la siguiente columna.
    assert_eq!(texto(&l, &h, 3, 2), "8");
    let c = l.contenido(&h, h.celda(3, 1).unwrap()).unwrap();
    assert_eq!(c.tramos.as_ref().map(|t| t.len()), Some(2));
    assert_eq!(
        l.contenido(&h, h.celda(1, 1).unwrap()).unwrap().horizontal,
        Horizontal::Derecha
    );
    assert_eq!(
        h.cola.combinadas,
        vec![Rango {
            fila0: 0,
            col0: 0,
            fila1: 1,
            col1: 0
        }]
    );
    assert_eq!(h.combinada_en(1, 0).map(|r| r.fila0), Some(0));
    let hv = h.hipervinculo_en(0, 1).unwrap();
    assert!(hv.externo && hv.destino == "https://example.com/");
    assert_eq!(h.hipervinculo_en(1, 1).unwrap().destino, "Datos!A1");
    // Fila 4 sin ht con letra de 20 puntos: crece.
    assert!(h.fila(3).unwrap().alto > 20.0);
    // La rejilla: A mide 20,71 caracteres -> 145 px; C y D ocultas.
    let r = Rejilla::nueva(&h, &l.estilos.fuente_normal);
    assert_eq!(r.ancho(0), 145.0);
    assert_eq!(r.ancho(1), 64.0);
    assert_eq!(r.ancho(2), 0.0);
    assert_eq!(r.alto(0), 40.0);
    assert_eq!(r.alto(1), 20.0);
    assert_eq!(r.alto(2), 0.0);
    assert_eq!(r.y(10), 40.0 + 20.0 + 0.0 + r.alto(3) + 6.0 * 20.0);
    // Con paneles: 4 trozos, y el punto (10, 10) es A1.
    let vista = Vista {
        ancho: 800.0,
        alto: 600.0,
        escala: 1.0,
        x: 0.0,
        y: 0.0,
    };
    let ps = r.paneles(&vista);
    assert_eq!(ps.len(), 4);
    assert_eq!(r.celda_en(&vista, 10.0, 10.0), Some((0, 0)));
    assert_eq!(r.celda_en_hoja(&h, &vista, 10.0, 45.0), Some((0, 0)));
    assert_eq!(r.celda_en(&vista, 160.0, 70.0), Some((3, 1)));
}

#[test]
fn la_hoja_grande_llega_por_tandas() {
    let l = Libro::desde_bytes(libro_de_prueba(20_000), Regional::en_us()).unwrap();
    let mut h = l.abrir_hoja(2);
    assert!(h.filas.is_empty());
    h.cargar_hasta_fila(99);
    assert!(
        h.filas.len() >= 100 && h.filas.len() < 20_000,
        "{}",
        h.filas.len()
    );
    assert_eq!(texto(&l, &h, 99, 0), "100");
    assert!(h.cola.combinadas.is_empty());
    // El resto, en otro hilo.
    let mut lector = h.tomar_lector().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        loop {
            let t = lector.leer(u32::MAX, 5000);
            let fin = t.fin;
            tx.send(t).unwrap();
            if fin {
                break;
            }
        }
    });
    for t in rx {
        h.recibir(t);
    }
    assert!(h.completa);
    assert_eq!(h.filas.len(), 20_000);
    assert_eq!(texto(&l, &h, 19_999, 0), "20000");
    assert_eq!(texto(&l, &h, 19_999, 1), "Nombre");
    assert_eq!(h.cola.combinadas.len(), 1);
}

#[test]
fn lo_roto_no_revienta() {
    assert_eq!(
        Libro::desde_bytes(Vec::new(), Regional::en_us()).err(),
        Some(ErrorHojas::NoEsZip)
    );
    assert!(Libro::desde_bytes(b"PK\x03\x04basura".to_vec(), Regional::en_us()).is_err());
    let bueno = libro_de_prueba(300);
    // ZIP cortado por cualquier sitio: error o libro, nunca panico.
    for corte in (0..bueno.len()).step_by(97) {
        if let Ok(l) = Libro::desde_bytes(bueno[..corte].to_vec(), Regional::en_us()) {
            for i in 0..l.hojas.len() {
                let mut h = l.abrir_hoja(i);
                h.cargar_todo();
            }
        }
    }
    // XML de la hoja cortado y con basura dentro.
    let g = hoja_grande(50);
    for corte in [0, 1, 10, 25, 100, g.len() / 2, g.len() - 3] {
        let mut sucia = g[..corte].to_vec();
        sucia.extend_from_slice(b"<c r=\"ZZZZZZ99999999999\" s=\"-1\" t=\"s\"><v>4294967295</v></c><row r=\"0\"><c><v>x</v></c></row>&&<<");
        let z = zip_de(&[
            ("_rels/.rels", RELS),
            ("xl/workbook.xml", LIBRO),
            ("xl/_rels/workbook.xml.rels", LIBRO_RELS),
            ("xl/worksheets/sheet3.xml", &sucia),
            ("xl/sharedStrings.xml", b"<sst><si><t>a</t></si><si"),
            ("xl/styles.xml", b"<styleSheet><cellXfs><xf numFmtId=\"99999\" fontId=\"77\"/></cellXfs><fonts><font><sz val=\"-4\"/></font>"),
        ]);
        let l = Libro::desde_bytes(z, Regional::es_pe()).unwrap();
        let mut h = l.abrir_hoja(2);
        h.cargar_todo();
        assert!(h.completa);
        for f in &h.filas {
            for c in h.celdas_de(f) {
                let _ = l.contenido(&h, c);
            }
        }
        let r = Rejilla::nueva(&h, &l.estilos.fuente_normal);
        let _ = r.paneles(&Vista {
            ancho: 100.0,
            alto: 100.0,
            escala: 2.0,
            x: 1e12,
            y: 1e12,
        });
        let _ = r.celda_en(
            &Vista {
                ancho: 100.0,
                alto: 100.0,
                escala: 0.0,
                x: -5.0,
                y: f64::NAN,
            },
            5.0,
            5.0,
        );
    }
    // Una hoja que no existe.
    let l = Libro::desde_bytes(libro_de_prueba(1), Regional::en_us()).unwrap();
    let h = l.abrir_hoja(99);
    assert!(h.completa && h.filas.is_empty());
}

#[test]
fn dibujos_anclados() {
    let hoja = br#"<worksheet><sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData><drawing r:id="rId1"/></worksheet>"#;
    let hoja_rels = br#"<Relationships><Relationship Id="rId1" Type="http://x/drawing" Target="../drawings/drawing1.xml"/></Relationships>"#;
    let dibujo = br#"<xdr:wsDr xmlns:xdr="x" xmlns:a="a" xmlns:r="r">
<xdr:twoCellAnchor><xdr:from><xdr:col>1</xdr:col><xdr:colOff>95250</xdr:colOff><xdr:row>2</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from><xdr:to><xdr:col>3</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>5</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to>
<xdr:pic><xdr:blipFill><a:blip r:embed="rId1"/></xdr:blipFill></xdr:pic></xdr:twoCellAnchor>
<xdr:oneCellAnchor><xdr:from><xdr:col>0</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>6</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from><xdr:ext cx="952500" cy="476250"/>
<xdr:graphicFrame><a:graphic><a:graphicData><c:chart xmlns:c="c" r:id="rId2"/></a:graphicData></a:graphic></xdr:graphicFrame></xdr:oneCellAnchor></xdr:wsDr>"#;
    let dibujo_rels = br#"<Relationships><Relationship Id="rId1" Type="http://x/image" Target="../media/image1.png"/><Relationship Id="rId2" Type="http://x/chart" Target="../charts/chart1.xml"/></Relationships>"#;
    let grafico = br#"<c:chartSpace xmlns:c="c" xmlns:a="a"><c:chart><c:title><c:tx><c:rich><a:p><a:r><a:t>Ventas</a:t></a:r></a:p></c:rich></c:tx></c:title><c:plotArea><c:barChart><c:barDir val="col"/><c:ser><c:tx><c:strRef><c:strCache><c:pt idx="0"><c:v>2024</c:v></c:pt></c:strCache></c:strRef></c:tx><c:cat><c:strRef><c:strCache><c:pt idx="0"><c:v>Ene</c:v></c:pt><c:pt idx="1"><c:v>Feb</c:v></c:pt></c:strCache></c:strRef></c:cat><c:val><c:numRef><c:numCache><c:ptCount val="2"/><c:pt idx="0"><c:v>3</c:v></c:pt><c:pt idx="1"><c:v>5</c:v></c:pt></c:numCache></c:numRef></c:val></c:ser></c:barChart></c:plotArea></c:chart></c:chartSpace>"#;
    let z = zip_de(&[
        ("_rels/.rels", RELS),
        ("xl/workbook.xml", br#"<workbook><sheets><sheet name="H" r:id="rId1"/></sheets></workbook>"#),
        (
            "xl/_rels/workbook.xml.rels",
            br#"<Relationships><Relationship Id="rId1" Type="http://x/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#,
        ),
        ("xl/worksheets/sheet1.xml", hoja),
        ("xl/worksheets/_rels/sheet1.xml.rels", hoja_rels),
        ("xl/drawings/drawing1.xml", dibujo),
        ("xl/drawings/_rels/drawing1.xml.rels", dibujo_rels),
        ("xl/charts/chart1.xml", grafico),
        ("xl/media/image1.png", b"no es un png"),
    ]);
    let l = Libro::desde_bytes(z, Regional::en_us()).unwrap();
    let mut h = l.abrir_hoja(0);
    h.cargar_todo();
    assert_eq!(h.cola.dibujos.len(), 2);
    let r = Rejilla::nueva(&h, &l.estilos.fuente_normal);
    match &h.cola.dibujos[0].contenido {
        ContenidoDibujo::Imagen { ruta, .. } => {
            assert_eq!(l.parte(ruta).as_deref(), Some(&b"no es un png"[..]))
        }
        o => panic!("{o:?}"),
    }
    assert_eq!(
        r.caja_dibujo(&h.cola.dibujos[0]),
        [64.0 + 10.0, 40.0, 118.0, 60.0]
    );
    match &h.cola.dibujos[1].contenido {
        ContenidoDibujo::Grafico {
            titulo,
            series,
            categorias,
            tipo,
        } => {
            assert_eq!(titulo, "Ventas");
            assert_eq!(tipo, "barChart");
            assert_eq!(categorias, &["Ene", "Feb"]);
            assert_eq!(series[0].valores, vec![3.0, 5.0]);
            assert_eq!(series[0].nombre, "2024");
        }
        o => panic!("{o:?}"),
    }
    assert_eq!(r.caja_dibujo(&h.cola.dibujos[1]), [0.0, 120.0, 100.0, 50.0]);
}

#[test]
fn validacion_de_lista_de_punta_a_punta() {
    use crate::validacion::{TipoValidacion, valores_lista};
    let h1 = br#"<worksheet><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>x</t></is></c></row></sheetData>
    <dataValidations count="3"><dataValidation type="list" sqref="A1:A3"><formula1>"si,no"</formula1></dataValidation>
    <dataValidation type="list" sqref="B1"><formula1>Listas!$A$1:$A$3</formula1></dataValidation>
    <dataValidation type="whole" operator="between" sqref="C1"><formula1>1</formula1><formula2>10</formula2></dataValidation>
    <dataValidation type="list" sqref="D1"><formula1>INDIRECT("Z1")</formula1></dataValidation></dataValidations></worksheet>"#;
    let h2 = br#"<worksheet><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>rojo</t></is></c></row><row r="2"><c r="A2"/></row>
    <row r="3"><c r="A3" t="inlineStr"><is><t>verde</t></is></c></row></sheetData></worksheet>"#;
    let z = zip_de(&[
        ("_rels/.rels", RELS),
        ("xl/workbook.xml", br#"<workbook><sheets><sheet name="Datos" r:id="rId1"/><sheet name="Listas" r:id="rId2"/></sheets></workbook>"#),
        (
            "xl/_rels/workbook.xml.rels",
            br#"<Relationships><Relationship Id="rId1" Type="http://x/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://x/worksheet" Target="worksheets/sheet2.xml"/></Relationships>"#,
        ),
        ("xl/worksheets/sheet1.xml", h1),
        ("xl/worksheets/sheet2.xml", h2),
    ]);
    let l = Libro::desde_bytes(z, Regional::en_us()).unwrap();
    let mut h = l.abrir_hoja(0);
    h.cargar_todo();
    assert_eq!(h.cola.validaciones.len(), 4);
    let literal = h.lista_en(2, 0).expect("A3 tiene lista");
    assert_eq!(valores_lista(&l, &h, literal), ["si", "no"]);
    let por_rango = h.lista_en(0, 1).expect("B1 tiene lista");
    assert_eq!(valores_lista(&l, &h, por_rango), ["rojo", "verde"]);
    // Un entero con limites no es una lista; una formula rara da una lista vacia.
    assert!(h.lista_en(0, 2).is_none());
    assert_eq!(h.cola.validaciones[2].tipo, TipoValidacion::Entero);
    assert!(valores_lista(&l, &h, h.lista_en(0, 3).unwrap()).is_empty());
    // Fuera de toda regla.
    assert!(h.lista_en(9, 9).is_none());
}

const XLS_PEQUENO: &[u8] = include_bytes!("../tests/muestras/pequeno.xls");

#[test]
fn un_xls_de_verdad_se_abre_como_un_libro_normal() {
    let l = Libro::desde_bytes(XLS_PEQUENO.to_vec(), Regional::en_us()).unwrap();
    assert_eq!(
        l.hojas
            .iter()
            .map(|h| h.nombre.as_str())
            .collect::<Vec<_>>(),
        vec!["Ventas", "Notas"]
    );
    let mut h = l.abrir_hoja(0);
    h.cargar_todo();
    assert!(h.completa);
    assert_eq!(texto(&l, &h, 0, 0), "Producto");
    assert_eq!(texto(&l, &h, 1, 0), "Café ñandú");
    assert_eq!(texto(&l, &h, 1, 1), "1,234.50");
    assert_eq!(texto(&l, &h, 2, 1), "9.99");
    assert_eq!(texto(&l, &h, 3, 0), "15/03/2026");
    assert_eq!(
        h.cola.combinadas,
        vec![Rango {
            fila0: 5,
            col0: 0,
            fila1: 5,
            col1: 2
        }]
    );
    // Negrita y relleno de los estilos del archivo.
    let c = h.celda(1, 1).unwrap();
    let xf = l.xf(c);
    assert!(l.estilos.fuente(xf).negrita);
    assert!(matches!(
        l.estilos.relleno(xf),
        crate::estilos::Relleno::Liso(_)
    ));
    // La segunda hoja se lee al pedirla, y pedirla otra vez da lo mismo.
    let h2 = l.abrir_hoja(1);
    assert_eq!(texto(&l, &h2, 0, 0), "otra hoja");
    assert_eq!(texto(&l, &l.abrir_hoja(1), 0, 0), "otra hoja");
    // Una hoja que no existe se ve vacia, sin panico.
    assert!(l.abrir_hoja(9).filas.is_empty());
}

#[test]
fn un_xls_roto_o_cortado_da_error_y_nunca_panico() {
    for corte in (0..XLS_PEQUENO.len()).step_by(211) {
        if let Ok(l) = Libro::desde_bytes(XLS_PEQUENO[..corte].to_vec(), Regional::en_us()) {
            for i in 0..l.hojas.len() {
                let mut h = l.abrir_hoja(i);
                h.cargar_todo();
            }
        }
    }
    // Un contenedor sin Workbook (o a medias) no es un libro.
    assert!(Libro::desde_bytes(XLS_PEQUENO[..600].to_vec(), Regional::en_us()).is_err());
    let mut malo = XLS_PEQUENO.to_vec();
    for b in malo.iter_mut().skip(512).step_by(7) {
        *b = 0xFF;
    }
    if let Ok(l) = Libro::desde_bytes(malo, Regional::en_us()) {
        let _ = l.abrir_hoja(0);
    }
}

#[test]
fn comentarios_autofiltro_minigraficos_y_condicionales_x14_de_punta_a_punta() {
    let hoja = br#"<worksheet xmlns:r="r"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Region</t></is></c><c r="B1" t="inlineStr"><is><t>V</t></is></c></row><row r="2" hidden="1"><c r="A2" t="inlineStr"><is><t>Sur</t></is></c><c r="B2"><v>1</v></c></row><row r="3"><c r="A3" t="inlineStr"><is><t>Norte</t></is></c><c r="B3"><v>5</v></c></row></sheetData>
<autoFilter ref="A1:B3"><filterColumn colId="0"><filters><filter val="Norte"/></filters></filterColumn></autoFilter>
<conditionalFormatting sqref="B2:B3"><cfRule type="iconSet" priority="1"><iconSet iconSet="3Arrows"><cfvo type="percent" val="0"/><cfvo type="percent" val="33"/><cfvo type="percent" val="67"/></iconSet></cfRule></conditionalFormatting>
<legacyDrawing r:id="rId1"/>
<extLst><ext xmlns:x14="x" xmlns:xm="m"><x14:sparklineGroups><x14:sparklineGroup type="column"><x14:sparklines><x14:sparkline><xm:f>B2:B3</xm:f><xm:sqref>C3</xm:sqref></x14:sparkline></x14:sparklines></x14:sparklineGroup></x14:sparklineGroups></ext></extLst></worksheet>"#;
    let rels = br#"<Relationships><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="../comments1.xml"/><Relationship Id="rId3" Type="http://schemas.microsoft.com/office/2017/10/relationships/threadedComment" Target="../threadedComments/threadedComment1.xml"/></Relationships>"#;
    let notas = br#"<comments><authors><author>Luis</author></authors><commentList><comment ref="B3" authorId="0"><text><t>Revisar</t></text></comment><comment ref="A1" authorId="0"><text><t>[Threaded comment] copia</t></text></comment></commentList></comments>"#;
    let hilos = br#"<ThreadedComments><threadedComment ref="A1" personId="{P}" id="{1}"><text>Hilo real</text></threadedComment></ThreadedComments>"#;
    let personas = br#"<personList><person displayName="Ana" id="{P}"/></personList>"#;
    let z = zip_de(&[
        ("_rels/.rels", RELS),
        ("xl/workbook.xml", br#"<workbook><sheets><sheet name="H" r:id="rId1"/></sheets></workbook>"#),
        ("xl/_rels/workbook.xml.rels", br#"<Relationships><Relationship Id="rId1" Type="http://x/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#),
        ("xl/worksheets/sheet1.xml", hoja),
        ("xl/worksheets/_rels/sheet1.xml.rels", rels),
        ("xl/comments1.xml", notas),
        ("xl/threadedComments/threadedComment1.xml", hilos),
        ("xl/persons/person.xml", personas),
    ]);
    let l = Libro::desde_bytes(z, Regional::en_us()).unwrap();
    let mut h = l.abrir_hoja(0);
    h.cargar_todo();
    // Comentarios: la nota de B3 y el hilo de A1 (que manda sobre su nota de compatibilidad).
    assert_eq!(h.cola.comentarios.len(), 2);
    assert_eq!(
        h.comentario_en(2, 1).map(|c| c.texto.as_str()),
        Some("Revisar")
    );
    let hilo = h.comentario_en(0, 0).unwrap();
    assert!(hilo.encadenado && hilo.autor == "Ana" && hilo.texto == "Hilo real");
    assert!(h.comentario_en(1, 1).is_none());
    // Autofiltro: boton en A1 y B1, filtro en A, y la fila 2 oculta ya viene asi.
    let f = h.cola.autofiltro.as_ref().unwrap();
    assert_eq!(f.columnas_con_boton().collect::<Vec<_>>(), vec![0, 1]);
    assert!(f.tiene_filtro(0) && !f.tiene_filtro(1));
    assert!(h.filas.iter().find(|x| x.indice == 1).unwrap().oculta);
    // Minigrafico en C3 con datos de B2:B3.
    assert_eq!(h.cola.minigraficos.len(), 1);
    let m = &h.cola.minigraficos[0];
    assert_eq!((m.fila, m.col), (2, 2));
    assert_eq!(l.valores_mini(&h, m), vec![Some(1.0), Some(5.0)]);
    let g = &h.cola.grupos_mini[0];
    let eje = l
        .eje_mini(
            &h,
            m,
            &crate::minigraficos::puntos_de(g, &l.valores_mini(&h, m)),
        )
        .unwrap();
    assert_eq!(eje, (0.0, 5.0), "columnas desde cero");
    // Formato condicional de iconos: el 1 es el peor, el 5 el mejor.
    let ef = |fila: u32| {
        l.efecto_condicional(&h, fila, h.celda(fila, 1).unwrap())
            .and_then(|e| e.icono)
    };
    assert_eq!(ef(1), Some((crate::condicional::Conjunto::Flechas3, 0)));
    assert_eq!(ef(2), Some((crate::condicional::Conjunto::Flechas3, 2)));
}

#[test]
fn partes_de_comentarios_rotas_o_ausentes_no_rompen_la_hoja() {
    let hoja = br#"<worksheet xmlns:r="r"><sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData></worksheet>"#;
    let rels = br#"<Relationships><Relationship Id="rId2" Type="http://x/comments" Target="../noexiste.xml"/><Relationship Id="rId3" Type="http://x/threadedComment" Target="../roto.xml"/></Relationships>"#;
    let z = zip_de(&[
        ("_rels/.rels", RELS),
        ("xl/workbook.xml", br#"<workbook><sheets><sheet name="H" r:id="rId1"/></sheets></workbook>"#),
        ("xl/_rels/workbook.xml.rels", br#"<Relationships><Relationship Id="rId1" Type="http://x/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#),
        ("xl/worksheets/sheet1.xml", hoja),
        ("xl/worksheets/_rels/sheet1.xml.rels", rels),
        ("xl/roto.xml", b"<ThreadedComments><threadedComment ref=\"A1\"><text>sin cerrar"),
    ]);
    let l = Libro::desde_bytes(z, Regional::en_us()).unwrap();
    let mut h = l.abrir_hoja(0);
    h.cargar_todo();
    assert!(h.cola.autofiltro.is_none() && h.cola.minigraficos.is_empty());
    assert!(h.cola.comentarios.len() <= 1);
}
