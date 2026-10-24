//! **Las demas partes del paquete**: tipos, relaciones, estilos,
//! numeracion, ajustes, letras, comentarios y propiedades. Todo texto fijo
//! o casi: lo unico que cambia es la letra de la nota, las imagenes, los
//! enlaces y los comentarios.

use super::cuerpo::Medio;
use super::{ComentarioW, Letra};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const RELS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const W14: &str = "http://schemas.microsoft.com/office/word/2010/wordml";
const W15: &str = "http://schemas.microsoft.com/office/word/2012/wordml";
const MC: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
const CABECERA: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;

/// La letra del codigo: la monoespaciada de Windows desde Vista.
pub(super) const CODIGO: &str = "Consolas";

/// **Texto para XML**: escapado y sin lo que XML 1.0 no admite (letras de
/// control salvo tabulador y saltos, y los no-caracteres `U+FFFE`/`U+FFFF`).
/// Una sola letra prohibida y Word dice que el fichero esta danado: aqui se
/// quitan, que es lo que se quiere de una marca invisible colada en la nota.
pub(super) fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\t' | '\n' | '\r' => o.push(c),
            c if (c as u32) < 0x20 => {}
            '\u{FFFE}' | '\u{FFFF}' | '\u{FFF9}' | '\u{FFFA}' | '\u{FFFB}' => {}
            c => o.push(c),
        }
    }
    o
}

// ---------------------------------------------------------------------------
// Tipos y relaciones

pub(super) fn tipos(medios: &[Medio], comentarios: bool) -> String {
    let mut s = format!(
        r#"{CABECERA}<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>"#
    );
    let mut vistas: Vec<&str> = Vec::new();
    for m in medios {
        let ext = m.formato.extension();
        if !vistas.contains(&ext) {
            vistas.push(ext);
            s.push_str(&format!(
                r#"<Default Extension="{ext}" ContentType="{}"/>"#,
                m.formato.mime()
            ));
        }
    }
    let mut partes = vec![
        (
            "/word/document.xml",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
        ),
        (
            "/word/styles.xml",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml",
        ),
        (
            "/word/numbering.xml",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml",
        ),
        (
            "/word/settings.xml",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml",
        ),
        (
            "/word/fontTable.xml",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.fontTable+xml",
        ),
        (
            "/docProps/core.xml",
            "application/vnd.openxmlformats-package.core-properties+xml",
        ),
        (
            "/docProps/app.xml",
            "application/vnd.openxmlformats-officedocument.extended-properties+xml",
        ),
    ];
    if comentarios {
        partes.push((
            "/word/comments.xml",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml",
        ));
        partes.push((
            "/word/commentsExtended.xml",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.commentsExtended+xml",
        ));
    }
    for (p, t) in partes {
        s.push_str(&format!(r#"<Override PartName="{p}" ContentType="{t}"/>"#));
    }
    s.push_str("</Types>");
    s
}

pub(super) const RELS_RAIZ: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>"#,
    r#"<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>"#,
    r#"<Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/>"#,
    "</Relationships>"
);

pub(super) fn rels_documento(
    medios: &[Medio],
    enlaces: &[(String, String)],
    comentarios: bool,
) -> String {
    let mut s = format!(r#"{CABECERA}<Relationships xmlns="{RELS}">"#);
    let mut fijas = vec![
        ("rId1", "styles", "styles.xml"),
        ("rId2", "numbering", "numbering.xml"),
        ("rId3", "settings", "settings.xml"),
        ("rId4", "fontTable", "fontTable.xml"),
    ];
    if comentarios {
        fijas.push(("rId5", "comments", "comments.xml"));
    }
    for (id, tipo, destino) in fijas {
        s.push_str(&format!(
            r#"<Relationship Id="{id}" Type="{R}/{tipo}" Target="{destino}"/>"#
        ));
    }
    if comentarios {
        s.push_str(r#"<Relationship Id="rId6" Type="http://schemas.microsoft.com/office/2011/relationships/commentsExtended" Target="commentsExtended.xml"/>"#);
    }
    for m in medios {
        s.push_str(&format!(
            r#"<Relationship Id="{}" Type="{R}/image" Target="media/{}"/>"#,
            m.rid,
            esc(&m.nombre)
        ));
    }
    for (rid, url) in enlaces {
        s.push_str(&format!(
            r#"<Relationship Id="{rid}" Type="{R}/hyperlink" Target="{}" TargetMode="External"/>"#,
            esc(url.trim())
        ));
    }
    s.push_str("</Relationships>");
    s
}

// ---------------------------------------------------------------------------
// Propiedades

/// Milisegundos UTC como fecha W3C (`2026-10-01T09:30:00Z`).
pub(super) fn fecha(ms: i64) -> String {
    let s = ms.div_euclid(1000);
    let dias = s.div_euclid(86_400);
    let resto = s.rem_euclid(86_400);
    // De dias desde 1970 a fecha civil (Howard Hinnant, `civil_from_days`).
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let a = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!(
        "{a:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        resto / 3600,
        resto % 3600 / 60,
        resto % 60
    )
}

pub(super) fn core(titulo: &str, autor: &str, ms: i64) -> String {
    let autor = if autor.trim().is_empty() {
        "PixPin"
    } else {
        autor.trim()
    };
    let f = fecha(ms);
    format!(
        concat!(
            "{c}",
            r#"<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" "#,
            r#"xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" "#,
            r#"xmlns:dcmitype="http://purl.org/dc/dcmitype/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">"#,
            "<dc:title>{t}</dc:title><dc:creator>{a}</dc:creator><cp:lastModifiedBy>{a}</cp:lastModifiedBy>",
            r#"<dcterms:created xsi:type="dcterms:W3CDTF">{f}</dcterms:created>"#,
            r#"<dcterms:modified xsi:type="dcterms:W3CDTF">{f}</dcterms:modified>"#,
            "</cp:coreProperties>"
        ),
        c = CABECERA,
        t = esc(titulo),
        a = esc(autor),
        f = f
    )
}

pub(super) const APP: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">"#,
    "<Application>PixPin</Application></Properties>"
);

pub(super) const AJUSTES: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
    r#"<w:zoom w:percent="100"/><w:defaultTabStop w:val="708"/>"#,
    r#"<w:characterSpacingControl w:val="doNotCompress"/>"#,
    r#"<w:compat><w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/></w:compat>"#,
    "</w:settings>"
);

// ---------------------------------------------------------------------------
// Letras

/// Para una letra de las del lienzo, que solo PixPin trae (la registra para
/// su proceso, no en Windows): la de Windows que mas se le parece, su
/// familia y su PANOSE. Word la pide por su nombre; donde no esta instalada
/// usa la de `altName` o la mas parecida por familia y PANOSE, que son las
/// de esa sustituta. `None`: la letra ya es de Windows.
pub(super) fn sustituta(nombre: &str) -> (Option<&'static str>, &'static str, &'static str) {
    match nombre {
        "Work Sans" | "Nunito" => (Some("Segoe UI"), "swiss", "020B0502040204020203"),
        "Lilita One" => (Some("Segoe UI Black"), "swiss", "020B0A02040204020203"),
        "Fraunces" => (Some("Georgia"), "roman", "02040502050405020303"),
        "Comic Shanns" => (Some("Comic Sans MS"), "script", "030F0702030302020204"),
        "Excalifont" | "Caveat" => (Some("Segoe Print"), "script", "02000600000000000000"),
        "Courier New" => (None, "modern", "02070309020205020404"),
        "Consolas" => (None, "modern", "020B0609020204030204"),
        "Cambria Math" => (None, "roman", "02040503050406030204"),
        "Segoe UI Symbol" => (None, "swiss", "020B0502040204020203"),
        "Arial" => (None, "swiss", "020B0604020202020204"),
        _ => (None, "auto", ""),
    }
}

/// `word/fontTable.xml`: cada letra usada con su sustituta.
pub(super) fn letras(l: &Letra) -> String {
    let mut s = format!(r#"{CABECERA}<w:fonts xmlns:w="{W}">"#);
    let nombres: Vec<&str> = vec![
        l.cuerpo.as_str(),
        l.titulos.as_str(),
        CODIGO,
        "Cambria Math",
        "Segoe UI Symbol",
        "Arial",
    ];
    let mut vistas: Vec<&str> = Vec::new();
    for n in nombres {
        if vistas.contains(&n) {
            continue;
        }
        vistas.push(n);
        let (alt, familia, panose) = sustituta(n);
        s.push_str(&format!(r#"<w:font w:name="{}">"#, esc(n)));
        if let Some(a) = alt {
            s.push_str(&format!(r#"<w:altName w:val="{a}"/>"#));
        }
        if !panose.is_empty() {
            s.push_str(&format!(r#"<w:panose1 w:val="{panose}"/>"#));
        }
        let paso = if familia == "modern" {
            "fixed"
        } else {
            "variable"
        };
        s.push_str(&format!(r#"<w:charset w:val="00"/><w:family w:val="{familia}"/><w:pitch w:val="{paso}"/></w:font>"#));
    }
    s.push_str("</w:fonts>");
    s
}

/// Los cuatro nombres de una letra en `w:rFonts`.
fn caras(n: &str) -> String {
    let n = esc(n);
    format!(r#"<w:rFonts w:ascii="{n}" w:hAnsi="{n}" w:eastAsia="{n}" w:cs="{n}"/>"#)
}

// ---------------------------------------------------------------------------
// Estilos

/// `word/styles.xml`. Los titulos son los `Heading1-6` de Word (con su
/// nombre interno `heading N`): salen en el panel de navegacion y en un
/// indice. Tamanos en proporcion al cuerpo, como en el editor.
pub(super) fn estilos(l: &Letra) -> String {
    // Medios puntos: 16 px son 12 pt, 24 medios puntos.
    let cuerpo = (l.px.clamp(8, 48) * 3 / 2).max(12);
    let x = |f: f32| ((cuerpo as f32 * f).round() as u32).max(12);
    let mut s = format!(
        r#"{CABECERA}<w:styles xmlns:w="{W}"><w:docDefaults><w:rPrDefault><w:rPr>{}<w:sz w:val="{cuerpo}"/><w:szCs w:val="{cuerpo}"/><w:lang w:val="es-ES" w:eastAsia="en-US" w:bidi="ar-SA"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after="120" w:line="276" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults>"#,
        caras(&l.cuerpo)
    );
    s.push_str(r#"<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/><w:rPr><w:color w:val="1F1F1F"/></w:rPr></w:style>"#);
    s.push_str(r#"<w:style w:type="character" w:default="1" w:styleId="DefaultParagraphFont"><w:name w:val="Default Paragraph Font"/><w:uiPriority w:val="1"/><w:semiHidden/><w:unhideWhenUsed/></w:style>"#);
    let tamanos = [1.75, 1.4, 1.2, 1.05, 1.0, 0.95];
    let antes = [360, 280, 240, 200, 200, 200];
    for n in 1..=6usize {
        s.push_str(&format!(
            concat!(
                r#"<w:style w:type="paragraph" w:styleId="Heading{n}"><w:name w:val="heading {n}"/>"#,
                r#"<w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:uiPriority w:val="9"/><w:qFormat/>"#,
                r#"<w:pPr><w:keepNext/><w:keepLines/><w:spacing w:before="{antes}" w:after="120"/><w:outlineLvl w:val="{nivel}"/></w:pPr>"#,
                r#"<w:rPr>{caras}<w:b/><w:bCs/><w:color w:val="111111"/><w:sz w:val="{tam}"/><w:szCs w:val="{tam}"/></w:rPr></w:style>"#
            ),
            n = n,
            antes = antes[n - 1],
            nivel = n - 1,
            caras = caras(&l.titulos),
            tam = x(tamanos[n - 1])
        ));
    }
    s.push_str(r#"<w:style w:type="paragraph" w:styleId="ListParagraph"><w:name w:val="List Paragraph"/><w:basedOn w:val="Normal"/><w:uiPriority w:val="34"/><w:qFormat/><w:pPr><w:spacing w:after="60"/><w:ind w:left="720"/><w:contextualSpacing/></w:pPr></w:style>"#);
    s.push_str(r#"<w:style w:type="paragraph" w:styleId="Cita"><w:name w:val="Quote"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:uiPriority w:val="29"/><w:qFormat/><w:pPr><w:pBdr><w:left w:val="single" w:sz="18" w:space="10" w:color="C9C9C9"/></w:pBdr><w:spacing w:after="60"/><w:ind w:left="284"/></w:pPr><w:rPr><w:i/><w:iCs/><w:color w:val="555555"/></w:rPr></w:style>"#);
    s.push_str(&format!(
        r#"<w:style w:type="paragraph" w:styleId="CodigoBloque"><w:name w:val="Codigo"/><w:basedOn w:val="Normal"/><w:uiPriority w:val="39"/><w:qFormat/><w:pPr><w:pBdr><w:top w:val="single" w:sz="4" w:space="4" w:color="E1E1E1"/><w:left w:val="single" w:sz="4" w:space="4" w:color="E1E1E1"/><w:bottom w:val="single" w:sz="4" w:space="4" w:color="E1E1E1"/><w:right w:val="single" w:sz="4" w:space="4" w:color="E1E1E1"/></w:pBdr><w:shd w:val="clear" w:color="auto" w:fill="F5F5F5"/><w:spacing w:after="160" w:line="240" w:lineRule="auto"/><w:ind w:left="113" w:right="113"/><w:contextualSpacing/></w:pPr><w:rPr>{}<w:noProof/><w:color w:val="24292F"/><w:sz w:val="{t}"/><w:szCs w:val="{t}"/></w:rPr></w:style>"#,
        caras(CODIGO),
        t = x(0.85)
    ));
    s.push_str(&format!(
        r#"<w:style w:type="paragraph" w:styleId="Pie"><w:name w:val="caption"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:uiPriority w:val="35"/><w:qFormat/><w:pPr><w:spacing w:after="200"/></w:pPr><w:rPr><w:i/><w:iCs/><w:color w:val="666666"/><w:sz w:val="{t}"/><w:szCs w:val="{t}"/></w:rPr></w:style>"#,
        t = x(0.85)
    ));
    s.push_str(r#"<w:style w:type="character" w:styleId="Hyperlink"><w:name w:val="Hyperlink"/><w:basedOn w:val="DefaultParagraphFont"/><w:uiPriority w:val="99"/><w:unhideWhenUsed/><w:rPr><w:color w:val="0563C1"/><w:u w:val="single"/></w:rPr></w:style>"#);
    s.push_str(&format!(
        r#"<w:style w:type="character" w:styleId="CodigoEnLinea"><w:name w:val="Codigo en linea"/><w:basedOn w:val="DefaultParagraphFont"/><w:uiPriority w:val="39"/><w:qFormat/><w:rPr>{}<w:noProof/><w:color w:val="C7254E"/><w:sz w:val="{t}"/><w:szCs w:val="{t}"/><w:shd w:val="clear" w:color="auto" w:fill="F2F2F2"/></w:rPr></w:style>"#,
        caras(CODIGO),
        t = x(0.9)
    ));
    s.push_str(r#"<w:style w:type="character" w:styleId="CommentReference"><w:name w:val="annotation reference"/><w:basedOn w:val="DefaultParagraphFont"/><w:uiPriority w:val="99"/><w:semiHidden/><w:unhideWhenUsed/><w:rPr><w:sz w:val="16"/><w:szCs w:val="16"/></w:rPr></w:style>"#);
    s.push_str(r#"<w:style w:type="paragraph" w:styleId="CommentText"><w:name w:val="annotation text"/><w:basedOn w:val="Normal"/><w:uiPriority w:val="99"/><w:unhideWhenUsed/><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:rPr><w:sz w:val="20"/><w:szCs w:val="20"/></w:rPr></w:style>"#);
    s.push_str(r#"<w:style w:type="table" w:default="1" w:styleId="TableNormal"><w:name w:val="Normal Table"/><w:uiPriority w:val="99"/><w:semiHidden/><w:unhideWhenUsed/><w:tblPr><w:tblInd w:w="0" w:type="dxa"/><w:tblCellMar><w:top w:w="0" w:type="dxa"/><w:left w:w="108" w:type="dxa"/><w:bottom w:w="0" w:type="dxa"/><w:right w:w="108" w:type="dxa"/></w:tblCellMar></w:tblPr></w:style>"#);
    s.push_str(r#"<w:style w:type="table" w:styleId="TablaNota"><w:name w:val="Tabla de nota"/><w:basedOn w:val="TableNormal"/><w:uiPriority w:val="59"/><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:tblPr><w:tblBorders><w:top w:val="single" w:sz="4" w:space="0" w:color="BFBFBF"/><w:left w:val="single" w:sz="4" w:space="0" w:color="BFBFBF"/><w:bottom w:val="single" w:sz="4" w:space="0" w:color="BFBFBF"/><w:right w:val="single" w:sz="4" w:space="0" w:color="BFBFBF"/><w:insideH w:val="single" w:sz="4" w:space="0" w:color="BFBFBF"/><w:insideV w:val="single" w:sz="4" w:space="0" w:color="BFBFBF"/></w:tblBorders><w:tblCellMar><w:top w:w="0" w:type="dxa"/><w:left w:w="100" w:type="dxa"/><w:bottom w:w="0" w:type="dxa"/><w:right w:w="100" w:type="dxa"/></w:tblCellMar></w:tblPr></w:style>"#);
    s.push_str("</w:styles>");
    s
}

// ---------------------------------------------------------------------------
// Numeracion

/// `word/numbering.xml`: la vineta (`numId` 1) y una numeracion por racha
/// (`numId` 2…), cada una empezando donde empezaba en el Markdown. Nueve
/// niveles, como Word: •, ◦, ▪ y 1., a., i. dando la vuelta.
pub(super) fn numeracion(rachas: &[u32]) -> String {
    let mut s = format!(r#"{CABECERA}<w:numbering xmlns:w="{W}">"#);
    let vinetas = ["\u{2022}", "\u{25E6}", "\u{25AA}"];
    s.push_str(
        r#"<w:abstractNum w:abstractNumId="0"><w:multiLevelType w:val="hybridMultilevel"/>"#,
    );
    for i in 0..9u32 {
        s.push_str(&format!(
            r#"<w:lvl w:ilvl="{i}"><w:start w:val="1"/><w:numFmt w:val="bullet"/><w:lvlText w:val="{}"/><w:lvlJc w:val="left"/><w:pPr><w:ind w:left="{}" w:hanging="360"/></w:pPr><w:rPr><w:rFonts w:ascii="Arial" w:hAnsi="Arial" w:cs="Arial" w:hint="default"/></w:rPr></w:lvl>"#,
            vinetas[i as usize % 3],
            720 * (i + 1)
        ));
    }
    s.push_str("</w:abstractNum>");
    let formatos = ["decimal", "lowerLetter", "lowerRoman"];
    s.push_str(
        r#"<w:abstractNum w:abstractNumId="1"><w:multiLevelType w:val="hybridMultilevel"/>"#,
    );
    for i in 0..9u32 {
        s.push_str(&format!(
            r#"<w:lvl w:ilvl="{i}"><w:start w:val="1"/><w:numFmt w:val="{}"/><w:lvlText w:val="%{}."/><w:lvlJc w:val="left"/><w:pPr><w:ind w:left="{}" w:hanging="360"/></w:pPr></w:lvl>"#,
            formatos[i as usize % 3],
            i + 1,
            720 * (i + 1)
        ));
    }
    s.push_str("</w:abstractNum>");
    s.push_str(r#"<w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>"#);
    for (k, inicio) in rachas.iter().enumerate() {
        // Cada racha se reinicia (si no, Word seguiria contando desde la
        // anterior con el mismo `abstractNum`).
        s.push_str(&format!(
            r#"<w:num w:numId="{}"><w:abstractNumId w:val="1"/>"#,
            k + 2
        ));
        for i in 0..9 {
            let v = if i == 0 { *inicio } else { 1 };
            s.push_str(&format!(
                r#"<w:lvlOverride w:ilvl="{i}"><w:startOverride w:val="{v}"/></w:lvlOverride>"#
            ));
        }
        s.push_str("</w:num>");
    }
    s.push_str("</w:numbering>");
    s
}

// ---------------------------------------------------------------------------
// Comentarios

/// Las iniciales de un nombre («PC de Max» → «PdM»), hasta tres.
fn iniciales(n: &str) -> String {
    let i: String = n
        .split_whitespace()
        .filter_map(|p| p.chars().next())
        .take(3)
        .collect();
    if i.is_empty() { "P".into() } else { i }
}

/// El `paraId` del ultimo parrafo de un comentario: lo que enlaza la
/// respuesta con su hilo en `commentsExtended.xml`. Ocho cifras hex por
/// debajo de `0x80000000`, como pide Word.
fn para_id(id: usize) -> String {
    format!("{:08X}", 0x1000_0000 + id as u32)
}

/// `word/comments.xml` y `word/commentsExtended.xml`.
pub(super) fn comentarios(hilos: &[ComentarioW]) -> (String, String) {
    let mut c = format!(
        r#"{CABECERA}<w:comments xmlns:w="{W}" xmlns:w14="{W14}" xmlns:mc="{MC}" mc:Ignorable="w14">"#
    );
    let mut ex = format!(
        r#"{CABECERA}<w15:commentsEx xmlns:w15="{W15}" xmlns:mc="{MC}" mc:Ignorable="w15">"#
    );
    for h in hilos {
        let autor = if h.autor.trim().is_empty() {
            "PixPin"
        } else {
            h.autor.trim()
        };
        c.push_str(&format!(
            r#"<w:comment w:id="{}" w:author="{}" w:date="{}" w:initials="{}">"#,
            h.id,
            esc(autor),
            fecha(h.cuando.max(0)),
            esc(&iniciales(autor))
        ));
        let renglones: Vec<&str> = h
            .texto
            .split('\n')
            .map(|r| r.trim_end_matches('\r'))
            .collect();
        let ultimo = renglones.len() - 1;
        for (i, r) in renglones.iter().enumerate() {
            let pid = if i == ultimo {
                para_id(h.id)
            } else {
                format!("{:08X}", 0x2000_0000 + (h.id as u32) * 64 + i as u32 % 64)
            };
            c.push_str(&format!(
                r#"<w:p w14:paraId="{pid}" w14:textId="77777777"><w:pPr><w:pStyle w:val="CommentText"/></w:pPr>"#
            ));
            if i == 0 {
                c.push_str(r#"<w:r><w:rPr><w:rStyle w:val="CommentReference"/></w:rPr><w:annotationRef/></w:r>"#);
            }
            if !r.is_empty() {
                c.push_str(&format!(
                    r#"<w:r><w:t xml:space="preserve">{}</w:t></w:r>"#,
                    esc(r)
                ));
            }
            c.push_str("</w:p>");
        }
        c.push_str("</w:comment>");
        let padre = h
            .padre
            .map(|p| format!(r#" w15:paraIdParent="{}""#, para_id(p)))
            .unwrap_or_default();
        ex.push_str(&format!(
            r#"<w15:commentEx w15:paraId="{}"{padre} w15:done="{}"/>"#,
            para_id(h.id),
            if h.resuelto { 1 } else { 0 }
        ));
    }
    c.push_str("</w:comments>");
    ex.push_str("</w15:commentsEx>");
    (c, ex)
}
