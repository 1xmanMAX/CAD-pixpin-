//! **`word/document.xml`**: los bloques de la nota escritos como parrafos,
//! tablas y dibujos de Word.
//!
//! El orden de los hijos de `w:pPr`, `w:rPr`, `w:tcPr`, `w:tblPr` y
//! `w:sectPr` es el del esquema (ECMA-376, parte 1, §17): Word se para en
//! uno fuera de su sitio y dice que el documento esta danado. Por eso cada
//! uno se escribe en una funcion y en ese orden, sin atajos.

use std::collections::{BTreeMap, BTreeSet};

use super::partes::esc;
use super::{Bloque1, Celda1, Colocados, Letra, Letra1, Marcas, Modelo, Parrafo, Tipo, imagen};
use crate::md_tabla::{Alineacion, Tabla, Vertical};
use crate::md_tabla_html::hex;

/// A4, que es el papel de casa (el del usuario y el de casi todo el mundo
/// salvo Norteamerica), con margenes de 2,54 cm: en veintavos de punto.
pub(super) const PAPEL_ANCHO: u32 = 11_906;
pub(super) const PAPEL_ALTO: u32 = 16_838;
pub(super) const MARGEN: u32 = 1_440;
/// Lo que queda para el texto, de pie y tumbado.
pub(super) const TEXTO_DE_PIE: u32 = PAPEL_ANCHO - 2 * MARGEN;
pub(super) const TEXTO_TUMBADO: u32 = PAPEL_ALTO - 2 * MARGEN;
/// La columna del editor, a 96 ppp (`md_imagen::ANCHO_COLUMNA`): una foto
/// de ese ancho llena la columna, y en Word llena el ancho de texto.
const COLUMNA_EDITOR: u32 = crate::md_imagen::ANCHO_COLUMNA;
/// Un veintavo de punto en EMU (la unidad de los dibujos).
const EMU_POR_TWIP: u64 = 635;
/// El aire de cada lado de una celda (`tblCellMar` del estilo de tabla).
const AIRE_CELDA: u32 = 100;

/// Una imagen metida en `word/media`.
pub(super) struct Medio {
    pub nombre: String,
    pub datos: Vec<u8>,
    pub formato: imagen::Formato,
    pub rid: String,
}

pub(super) struct Hecho {
    pub documento: String,
    pub medios: Vec<Medio>,
    /// `(rid, url)` de los hipervinculos.
    pub enlaces: Vec<(String, String)>,
    /// El numero con que empieza cada racha numerada.
    pub numeradas: Vec<u32>,
}

/// Lo que una celda le pone a sus letras ademas de sus marcas.
#[derive(Default, Clone, Copy)]
struct DeCelda {
    negrita: bool,
    color: Option<u32>,
    /// En medios puntos; nada, el del estilo.
    tam: Option<u32>,
}

/// Si una direccion se puede abrir desde Word: la web y el correo. Las de
/// PixPin (`pixpin:hoja=`, `pixpin:mensaje=`) y las rutas no: fuera de
/// PixPin no llevan a ningun sitio, y queda su nombre como texto.
pub(super) fn es_enlace_web(url: &str) -> bool {
    let u = url.trim().to_ascii_lowercase();
    (u.starts_with("https://") || u.starts_with("http://") || u.starts_with("mailto:"))
        && !u.contains(char::is_whitespace)
}

struct Escritor<'a> {
    s: String,
    letra: &'a Letra,
    com: &'a Colocados,
    enlaces: &'a [String],
    imagen: &'a dyn Fn(&str) -> Option<Vec<u8>>,
    medios: Vec<Medio>,
    por_ruta: BTreeMap<String, Option<(usize, u32, u32)>>,
    rids: BTreeMap<usize, String>,
    /// Los comentarios ya abiertos y cerrados en el texto.
    abiertos: BTreeSet<usize>,
    cerrados: BTreeSet<usize>,
    /// La seccion en curso va tumbada.
    tumbada: bool,
    /// Ya se ha escrito algun bloque.
    algo: bool,
    /// Lo ultimo fue una tabla (dos seguidas, Word las junta en una).
    tras_tabla: bool,
    /// Donde empiezan los titulos seguidos que acaban de escribirse.
    titulos_desde: Option<usize>,
    dibujos: u32,
}

/// **Escribe el cuerpo.**
pub(super) fn escribir(
    m: &Modelo,
    com: &Colocados,
    letra: &Letra,
    imagen: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> Hecho {
    let mut e = Escritor {
        s: String::with_capacity(16 * 1024),
        letra,
        com,
        enlaces: &m.enlaces,
        imagen,
        medios: Vec::new(),
        por_ruta: BTreeMap::new(),
        rids: BTreeMap::new(),
        abiertos: BTreeSet::new(),
        cerrados: BTreeSet::new(),
        tumbada: false,
        algo: false,
        tras_tabla: false,
        titulos_desde: None,
        dibujos: 0,
    };
    for b in &m.bloques {
        match b {
            Bloque1::Parrafo(p) => {
                let desde = e.titulos_desde;
                e.antes(false);
                if matches!(p.tipo, Tipo::Titulo(_)) {
                    e.titulos_desde = Some(desde.unwrap_or(e.s.len()));
                }
                e.parrafo(p);
            }
            Bloque1::Vacio => {
                e.antes(false);
                e.s.push_str("<w:p/>");
            }
            Bloque1::Regla => {
                e.antes(false);
                e.s.push_str(
                    r#"<w:p><w:pPr><w:pBdr><w:bottom w:val="single" w:sz="6" w:space="1" w:color="BFBFBF"/></w:pBdr><w:spacing w:before="120" w:after="240"/></w:pPr></w:p>"#,
                );
            }
            Bloque1::Foto { foto, orden, .. } => {
                e.antes(false);
                e.foto(foto, *orden);
            }
            Bloque1::Tabla { tabla, celdas } => e.tabla(tabla, celdas),
        }
    }
    // Una tabla no puede ser lo ultimo del cuerpo (Word pone un parrafo
    // detras igualmente), y un documento vacio lleva su parrafo.
    if e.tras_tabla || !e.algo {
        e.s.push_str("<w:p/>");
    }
    // Lo que no se coloco (un comentario de una nota sin letras): en un
    // parrafo suyo al final, para que no se pierda.
    let sueltos: Vec<usize> = e
        .com
        .hilos
        .iter()
        .map(|h| h.id)
        .filter(|id| !e.cerrados.contains(id))
        .collect();
    if !sueltos.is_empty() {
        e.s.push_str("<w:p>");
        for id in &sueltos {
            if !e.abiertos.contains(id) {
                e.s.push_str(&format!(r#"<w:commentRangeStart w:id="{id}"/>"#));
            }
        }
        for id in &sueltos {
            e.cerrar_comentario(*id);
        }
        e.s.push_str("</w:p>");
    }
    let seccion = seccion(e.tumbada);
    let documento = format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
            "\n",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" "#,
            r#"xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" "#,
            r#"xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" "#,
            r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" "#,
            r#"xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">"#,
            "<w:body>{}{}</w:body></w:document>"
        ),
        e.s, seccion
    );
    let enlaces = e
        .rids
        .iter()
        .map(|(i, rid)| (rid.clone(), m.enlaces[*i].clone()))
        .collect();
    Hecho {
        documento,
        medios: e.medios,
        enlaces,
        numeradas: m.rachas.clone(),
    }
}

/// Las medidas de una seccion: A4 de pie o tumbado.
fn seccion(tumbada: bool) -> String {
    let (an, al, orient) = if tumbada {
        (PAPEL_ALTO, PAPEL_ANCHO, r#" w:orient="landscape""#)
    } else {
        (PAPEL_ANCHO, PAPEL_ALTO, "")
    };
    format!(
        r#"<w:sectPr><w:pgSz w:w="{an}" w:h="{al}"{orient}/><w:pgMar w:top="{MARGEN}" w:right="{MARGEN}" w:bottom="{MARGEN}" w:left="{MARGEN}" w:header="708" w:footer="708" w:gutter="0"/><w:cols w:space="708"/><w:docGrid w:linePitch="360"/></w:sectPr>"#
    )
}

impl Escritor<'_> {
    fn ancho_de_texto(&self) -> u32 {
        if self.tumbada {
            TEXTO_TUMBADO
        } else {
            TEXTO_DE_PIE
        }
    }

    /// Antes de un bloque: cierra la seccion si cambia de pie a tumbada o
    /// al reves (el parrafo con su `sectPr` es como Word marca el salto de
    /// seccion), y separa dos tablas seguidas. Al tumbar, los titulos que
    /// van justo antes de la tabla se van con ella a su pagina: un titulo
    /// solo al pie de una hoja de pie no dice nada.
    fn antes(&mut self, tumbada: bool) {
        if tumbada != self.tumbada {
            let salto = format!("<w:p><w:pPr>{}</w:pPr></w:p>", seccion(self.tumbada));
            match self.titulos_desde.filter(|_| tumbada && !self.tras_tabla) {
                Some(0) => {}
                Some(i) => self.s.insert_str(i, &salto),
                None if self.algo => self.s.push_str(&salto),
                None => {}
            }
            self.tumbada = tumbada;
            self.tras_tabla = false;
        }
        self.titulos_desde = None;
        if self.tras_tabla {
            // Dos tablas pegadas Word las funde en una: un parrafo vacio
            // y pequeno entre las dos.
            self.s
                .push_str(r#"<w:p><w:pPr><w:spacing w:before="0" w:after="0"/></w:pPr></w:p>"#);
        }
        self.tras_tabla = false;
        self.algo = true;
    }

    fn parrafo(&mut self, p: &Parrafo) {
        self.s.push_str("<w:p>");
        let mut pr = String::new();
        let mut prefijo: Option<&str> = None;
        match p.tipo {
            Tipo::Normal => {}
            Tipo::Titulo(n) => pr.push_str(&format!(r#"<w:pStyle w:val="Heading{n}"/>"#)),
            Tipo::Cita => pr.push_str(r#"<w:pStyle w:val="Cita"/>"#),
            Tipo::Codigo => pr.push_str(r#"<w:pStyle w:val="CodigoBloque"/>"#),
            Tipo::Vineta { nivel } => pr.push_str(&format!(
                r#"<w:pStyle w:val="ListParagraph"/><w:numPr><w:ilvl w:val="{nivel}"/><w:numId w:val="1"/></w:numPr>"#
            )),
            Tipo::Numerada { nivel, lista } => pr.push_str(&format!(
                r#"<w:pStyle w:val="ListParagraph"/><w:numPr><w:ilvl w:val="{nivel}"/><w:numId w:val="{}"/></w:numPr>"#,
                lista + 2
            )),
            Tipo::Casilla { nivel, hecha } => {
                // La casilla va en el sitio de la vineta (la sangria
                // francesa), con el texto alineado como en una lista.
                let izquierda = 720 * (nivel as u32 + 1);
                pr.push_str(&format!(
                    r#"<w:pStyle w:val="ListParagraph"/><w:ind w:left="{izquierda}" w:hanging="360"/>"#
                ));
                prefijo = Some(if hecha { "\u{2612}" } else { "\u{2610}" });
            }
            Tipo::Adjunto => prefijo = Some("\u{1F4CE}"),
        }
        if !pr.is_empty() {
            self.s.push_str("<w:pPr>");
            self.s.push_str(&pr);
            self.s.push_str("</w:pPr>");
        }
        if let Some(x) = prefijo {
            // En la letra de los simbolos de Windows, que tiene ☐ ☒ y el
            // clip; seguida de un tabulador (casilla) o un espacio.
            let tras = if matches!(p.tipo, Tipo::Casilla { .. }) {
                "<w:tab/>"
            } else {
                r#"<w:t xml:space="preserve"> </w:t>"#
            };
            self.s.push_str(&format!(
                r#"<w:r><w:rPr><w:rFonts w:ascii="Segoe UI Symbol" w:hAnsi="Segoe UI Symbol" w:cs="Segoe UI Symbol"/></w:rPr><w:t>{x}</w:t>{tras}</w:r>"#
            ));
        }
        self.letras(&p.letras, DeCelda::default());
        self.s.push_str("</w:p>");
    }

    fn cerrar_comentario(&mut self, id: usize) {
        self.cerrados.insert(id);
        self.s.push_str(&format!(
            r#"<w:commentRangeEnd w:id="{id}"/><w:r><w:rPr><w:rStyle w:val="CommentReference"/></w:rPr><w:commentReference w:id="{id}"/></w:r>"#
        ));
    }

    /// Las letras de un parrafo en tramos (`w:r`), con los hipervinculos y
    /// las marcas de los comentarios en su sitio.
    fn letras(&mut self, letras: &[Letra1], celda: DeCelda) {
        let mut tramo = String::new();
        let mut marcas = Marcas::default();
        let mut enlace: Option<usize> = None;
        for l in letras {
            let quiere = l.marcas.enlace.filter(|i| es_enlace_web(&self.enlaces[*i]));
            if quiere != enlace {
                self.tramo(&mut tramo, marcas, celda);
                if enlace.is_some() {
                    self.s.push_str("</w:hyperlink>");
                }
                if let Some(i) = quiere {
                    let siguiente = format!("rIdE{}", self.rids.len() + 1);
                    let rid = self.rids.entry(i).or_insert(siguiente).clone();
                    self.s
                        .push_str(&format!(r#"<w:hyperlink r:id="{rid}" w:history="1">"#));
                }
                enlace = quiere;
            }
            let com = self.com;
            if let Some(ids) = com.empiezan.get(&l.orden) {
                self.tramo(&mut tramo, marcas, celda);
                for id in ids {
                    if self.abiertos.insert(*id) {
                        self.s
                            .push_str(&format!(r#"<w:commentRangeStart w:id="{id}"/>"#));
                    }
                }
            }
            if l.marcas != marcas {
                self.tramo(&mut tramo, marcas, celda);
                marcas = l.marcas;
            }
            tramo.push(l.c);
            if let Some(ids) = com.acaban.get(&l.orden) {
                self.tramo(&mut tramo, marcas, celda);
                for id in ids.clone() {
                    if self.abiertos.contains(&id) && !self.cerrados.contains(&id) {
                        self.cerrar_comentario(id);
                    }
                }
            }
        }
        self.tramo(&mut tramo, marcas, celda);
        if enlace.is_some() {
            self.s.push_str("</w:hyperlink>");
        }
    }

    /// Escribe lo acumulado como un `w:r` y lo vacia.
    fn tramo(&mut self, texto: &mut String, m: Marcas, celda: DeCelda) {
        if texto.is_empty() {
            return;
        }
        let web = m.enlace.is_some_and(|i| es_enlace_web(&self.enlaces[i]));
        let mut pr = String::new();
        // El orden de `w:rPr`: rStyle, rFonts, b, i, strike, color, sz, u, shd.
        if m.codigo {
            pr.push_str(r#"<w:rStyle w:val="CodigoEnLinea"/>"#);
        } else if web {
            pr.push_str(r#"<w:rStyle w:val="Hyperlink"/>"#);
        }
        if m.formula {
            pr.push_str(
                r#"<w:rFonts w:ascii="Cambria Math" w:hAnsi="Cambria Math" w:cs="Cambria Math"/>"#,
            );
        }
        if m.negrita || celda.negrita {
            pr.push_str("<w:b/><w:bCs/>");
        }
        if m.cursiva || m.formula {
            pr.push_str("<w:i/><w:iCs/>");
        }
        if m.tachado || m.hecha {
            pr.push_str("<w:strike/>");
        }
        if m.hecha {
            pr.push_str(r#"<w:color w:val="8A8A8A"/>"#);
        } else if let Some(c) = celda.color.filter(|_| !web) {
            pr.push_str(&format!(
                r#"<w:color w:val="{}"/>"#,
                hex(c)[1..].to_ascii_uppercase()
            ));
        }
        if let Some(t) = celda.tam {
            pr.push_str(&format!(r#"<w:sz w:val="{t}"/><w:szCs w:val="{t}"/>"#));
        }
        self.s.push_str("<w:r>");
        if !pr.is_empty() {
            self.s.push_str("<w:rPr>");
            self.s.push_str(&pr);
            self.s.push_str("</w:rPr>");
        }
        texto_de_tramo(&mut self.s, texto);
        self.s.push_str("</w:r>");
        texto.clear();
    }

    // -----------------------------------------------------------------------
    // Fotos

    /// La foto de esa ruta en `word/media`: su numero y su tamano. Una
    /// misma foto dos veces va una vez.
    fn medio(&mut self, ruta: &str) -> Option<(usize, u32, u32)> {
        if let Some(x) = self.por_ruta.get(ruta) {
            return *x;
        }
        let hecho = (self.imagen)(ruta).and_then(|datos| {
            let (formato, an, al) = imagen::leer(&datos)?;
            let n = self.medios.len() + 1;
            self.medios.push(Medio {
                nombre: format!("imagen{n}.{}", formato.extension()),
                datos,
                formato,
                rid: format!("rIdM{n}"),
            });
            Some((n - 1, an, al))
        });
        self.por_ruta.insert(ruta.to_string(), hecho);
        hecho
    }

    fn foto(&mut self, foto: &crate::md_imagen::Foto, orden: usize) {
        let Some((i, an, al)) = self.medio(&foto.ruta) else {
            // Sin la foto (no esta en este equipo, o es de un formato que
            // Word no pinta): un aviso en su sitio, no un documento roto.
            let nombre = if foto.alt.trim().is_empty() {
                foto.ruta
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or("")
                    .to_string()
            } else {
                foto.alt.clone()
            };
            self.s.push_str(&format!(
                r#"<w:p><w:pPr><w:pStyle w:val="Pie"/></w:pPr><w:r><w:t xml:space="preserve">[{}]</w:t></w:r></w:p>"#,
                esc(&format!("\u{1F5BC} {nombre}"))
            ));
            return;
        };
        // El ancho del editor (el guardado, o el suyo hasta la columna)
        // llevado al ancho de texto de la pagina: la foto ocupa en Word la
        // misma parte de la linea que en la nota.
        let px = foto.ancho.unwrap_or(an).clamp(1, COLUMNA_EDITOR);
        let mut ancho =
            px as u64 * self.ancho_de_texto() as u64 / COLUMNA_EDITOR as u64 * EMU_POR_TWIP;
        let mut alto = ancho * al as u64 / an as u64;
        // Y que quepa de alto en la pagina (una captura de un movil, larga).
        let tope = (PAPEL_ALTO.max(PAPEL_ANCHO) as u64 - 2 * MARGEN as u64 - 720) * EMU_POR_TWIP;
        let tope = if self.tumbada {
            tope.min((PAPEL_ANCHO as u64 - 2 * MARGEN as u64 - 720) * EMU_POR_TWIP)
        } else {
            tope
        };
        if alto > tope {
            ancho = ancho * tope / alto;
            alto = tope;
        }
        let (ancho, alto) = (ancho.max(1), alto.max(1));
        self.dibujos += 1;
        let id = self.dibujos;
        let rid = self.medios[i].rid.clone();
        let nombre = esc(&self.medios[i].nombre);
        let descr = esc(&foto.alt);
        self.s.push_str(
            "<w:p><w:pPr><w:keepNext/><w:spacing w:before=\"120\" w:after=\"60\"/></w:pPr>",
        );
        if let Some(ids) = self.com.empiezan.get(&orden) {
            for id in ids.clone() {
                if self.abiertos.insert(id) {
                    self.s
                        .push_str(&format!(r#"<w:commentRangeStart w:id="{id}"/>"#));
                }
            }
        }
        self.s.push_str(&format!(
            concat!(
                r#"<w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0">"#,
                r#"<wp:extent cx="{an}" cy="{al}"/><wp:effectExtent l="0" t="0" r="0" b="0"/>"#,
                r#"<wp:docPr id="{id}" name="Imagen {id}" descr="{descr}"/>"#,
                r#"<wp:cNvGraphicFramePr><a:graphicFrameLocks noChangeAspect="1"/></wp:cNvGraphicFramePr>"#,
                r#"<a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">"#,
                r#"<pic:pic><pic:nvPicPr><pic:cNvPr id="{id}" name="{nombre}"/><pic:cNvPicPr/></pic:nvPicPr>"#,
                r#"<pic:blipFill><a:blip r:embed="{rid}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>"#,
                r#"<pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{an}" cy="{al}"/></a:xfrm>"#,
                r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic>"#,
                r#"</a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"#
            ),
            an = ancho,
            al = alto,
            id = id,
            descr = descr,
            nombre = nombre,
            rid = rid
        ));
        if let Some(ids) = self.com.acaban.get(&orden) {
            for id in ids.clone() {
                if self.abiertos.contains(&id) && !self.cerrados.contains(&id) {
                    self.cerrar_comentario(id);
                }
            }
        }
        self.s.push_str("</w:p>");
        // El pie, como en la nota (el texto de la foto, sin su ancho).
        if !foto.alt.trim().is_empty() {
            self.s.push_str(&format!(
                r#"<w:p><w:pPr><w:pStyle w:val="Pie"/></w:pPr><w:r><w:t xml:space="preserve">{}</w:t></w:r></w:p>"#,
                esc(foto.alt.trim())
            ));
        }
    }

    // -----------------------------------------------------------------------
    // Tablas

    fn tabla(&mut self, t: &Tabla, celdas: &[Vec<Celda1>]) {
        let n = t.columnas();
        let reparto = repartir(t, celdas, self.letra.px);
        self.antes(reparto.tumbada);
        let anchos = reparto.anchos;
        let total: u32 = anchos.iter().sum();
        self.s.push_str(&format!(
            r#"<w:tbl><w:tblPr><w:tblStyle w:val="TablaNota"/><w:tblW w:w="{total}" w:type="dxa"/><w:tblLayout w:type="fixed"/><w:tblLook w:val="04A0" w:firstRow="1" w:lastRow="0" w:firstColumn="0" w:lastColumn="0" w:noHBand="1" w:noVBand="1"/></w:tblPr><w:tblGrid>"#
        ));
        for a in &anchos {
            self.s.push_str(&format!(r#"<w:gridCol w:w="{a}"/>"#));
        }
        self.s.push_str("</w:tblGrid>");
        // Las filas de cabecera seguidas desde arriba se repiten en cada
        // pagina (`tblHeader`), como hace Word con «Repetir como fila de
        // encabezado».
        let mut de_cabecera = 0;
        while de_cabecera < t.filas.len()
            && (0..n).all(|c| t.formato(de_cabecera, c).tapada || t.es_cabecera(de_cabecera, c))
        {
            de_cabecera += 1;
        }
        // Una tabla que es toda cabecera no repite nada.
        if de_cabecera == t.filas.len() {
            de_cabecera = de_cabecera.min(1);
        }
        for f in 0..t.filas.len() {
            self.s.push_str("<w:tr>");
            if f < de_cabecera {
                self.s.push_str("<w:trPr><w:tblHeader/></w:trPr>");
            }
            let mut c = 0;
            while c < n {
                let x = t.formato(f, c);
                if !x.tapada {
                    let span = x.columnas.clamp(1, n - c);
                    let rehace = x.filas > 1;
                    self.celda(
                        t,
                        f,
                        c,
                        span,
                        if rehace { Some(true) } else { None },
                        &anchos,
                        celdas,
                        reparto.tam,
                    );
                    c += span;
                    continue;
                }
                let (af, ac) = t.ancla(f, c);
                if af < f && ac == c {
                    // La continuacion de una combinada de arriba.
                    let span = t.formato(af, ac).columnas.clamp(1, n - c);
                    self.celda(t, af, ac, span, Some(false), &anchos, celdas, reparto.tam);
                    c += span;
                } else {
                    c += 1;
                }
            }
            self.s.push_str("</w:tr>");
        }
        self.s.push_str("</w:tbl>");
        self.tras_tabla = true;
    }

    /// Una celda. `combinada`: `Some(true)` empieza una combinada hacia
    /// abajo, `Some(false)` la sigue (va vacia), `None` ninguna.
    #[allow(clippy::too_many_arguments)]
    fn celda(
        &mut self,
        t: &Tabla,
        f: usize,
        c: usize,
        span: usize,
        combinada: Option<bool>,
        anchos: &[u32],
        celdas: &[Vec<Celda1>],
        tam: Option<u32>,
    ) {
        let x = t.formato(f, c);
        let cabecera = t.es_cabecera(f, c);
        let ancho: u32 = anchos[c..c + span].iter().sum();
        let mut pr = format!(r#"<w:tcW w:w="{ancho}" w:type="dxa"/>"#);
        if span > 1 {
            pr.push_str(&format!(r#"<w:gridSpan w:val="{span}"/>"#));
        }
        match combinada {
            Some(true) => pr.push_str(r#"<w:vMerge w:val="restart"/>"#),
            Some(false) => pr.push_str("<w:vMerge/>"),
            None => {}
        }
        // El fondo de la celda; la cabecera sin color lleva un gris suave,
        // como las tablas del editor.
        let fondo = x
            .fondo
            .map(|c| hex(c)[1..].to_ascii_uppercase())
            .or(cabecera.then(|| "F2F2F2".to_string()));
        if let Some(fill) = fondo {
            pr.push_str(&format!(
                r#"<w:shd w:val="clear" w:color="auto" w:fill="{fill}"/>"#
            ));
        }
        match x.vertical {
            Vertical::Medio => pr.push_str(r#"<w:vAlign w:val="center"/>"#),
            Vertical::Abajo => pr.push_str(r#"<w:vAlign w:val="bottom"/>"#),
            Vertical::Arriba => {}
        }
        self.s.push_str("<w:tc><w:tcPr>");
        self.s.push_str(&pr);
        self.s.push_str("</w:tcPr>");
        let jc = match t.alineacion_de(f, c) {
            Alineacion::Centro => r#"<w:jc w:val="center"/>"#,
            Alineacion::Derecha => r#"<w:jc w:val="right"/>"#,
            Alineacion::Izquierda => "",
        };
        let ppr = format!(
            r#"<w:pPr><w:spacing w:before="40" w:after="40" w:line="240" w:lineRule="auto"/>{jc}</w:pPr>"#
        );
        let parrafos: &[Vec<Letra1>] = match combinada {
            Some(false) => &[],
            _ => celdas
                .get(f)
                .and_then(|fila| fila.get(c))
                .map(Vec::as_slice)
                .unwrap_or(&[]),
        };
        let de_celda = DeCelda {
            negrita: cabecera,
            color: x.letra,
            tam,
        };
        if parrafos.is_empty() {
            self.s.push_str(&format!("<w:p>{ppr}</w:p>"));
        }
        for p in parrafos {
            self.s.push_str("<w:p>");
            self.s.push_str(&ppr);
            self.letras(p, de_celda);
            self.s.push_str("</w:p>");
        }
        self.s.push_str("</w:tc>");
    }
}

/// El texto de un `w:r`: tabuladores y saltos como sus elementos, y el
/// resto escapado y con sus blancos guardados.
fn texto_de_tramo(s: &mut String, texto: &str) {
    let mut trozo = String::new();
    let soltar = |s: &mut String, trozo: &mut String| {
        if !trozo.is_empty() {
            s.push_str(r#"<w:t xml:space="preserve">"#);
            s.push_str(&esc(trozo));
            s.push_str("</w:t>");
            trozo.clear();
        }
    };
    for c in texto.chars() {
        match c {
            '\t' => {
                soltar(s, &mut trozo);
                s.push_str("<w:tab/>");
            }
            '\n' | '\u{000B}' => {
                soltar(s, &mut trozo);
                s.push_str("<w:br/>");
            }
            '\r' => {}
            _ => trozo.push(c),
        }
    }
    soltar(s, &mut trozo);
}

/// Como se reparte una tabla en la pagina.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Reparto {
    /// El ancho de cada columna, en veintavos de punto.
    pub anchos: Vec<u32>,
    /// Va en una seccion tumbada.
    pub tumbada: bool,
    /// La letra, en medios puntos, si hubo que achicarla.
    pub tam: Option<u32>,
}

/// **Los anchos de las columnas**, como hace el navegador con una tabla
/// automatica: cada columna pide lo que mide su palabra mas larga (lo
/// minimo para no partir palabras) y su renglon mas largo (lo que querria).
/// Si todo cabe, el ancho de texto se reparte en proporcion a lo que
/// quieren; si no, cada una tiene su minimo y el resto se reparte.
///
/// **Una tabla que no cabe** (la suma de minimos pasa del ancho), por
/// orden de preferencia:
/// 1. **De pie con la letra algo menor** (90 %, 80 % o 75 % del cuerpo, no
///    menos de 9 pt): se sigue leyendo de corrido, sin saltos de pagina ni
///    un titulo suelto al pie de una hoja (probado con una nota real: con
///    la pagina tumbada cada tabla de seis columnas dejaba media hoja en
///    blanco detras de su titulo).
/// 2. **En una pagina tumbada solo para ella** (con su titulo), con la
///    letra del cuerpo o, si hace falta, algo menor.
/// 3. Si ni asi, tumbada a 8 pt (por debajo ya no se lee impreso) y las
///    palabras mas largas se parten.
pub(super) fn repartir(t: &Tabla, celdas: &[Vec<Celda1>], px: u32) -> Reparto {
    let n = t.columnas();
    let pt = (px.clamp(8, 40) * 3) as f32 / 4.0;
    let aire = (2 * AIRE_CELDA + 40) as f32;
    // Letras de la palabra mas larga y del renglon mas largo de cada
    // columna (la cabecera, en negrita, ocupa algo mas).
    let mut palabra = vec![3.0f32; n];
    let mut renglon = vec![3.0f32; n];
    for (f, fila) in celdas.iter().enumerate() {
        for (c, celda) in fila.iter().enumerate().take(n) {
            let x = t.formato(f, c);
            if x.tapada || x.columnas > 1 {
                continue;
            }
            let gordo = if t.es_cabecera(f, c) { 1.12 } else { 1.0 };
            for p in celda {
                let texto: String = p.iter().map(|l| l.c).collect();
                let w = texto
                    .split_whitespace()
                    .map(|w| w.chars().count())
                    .max()
                    .unwrap_or(0);
                palabra[c] = palabra[c].max(w as f32 * gordo);
                renglon[c] = renglon[c].max(texto.chars().count() as f32 * gordo);
            }
        }
    }
    // Lo que ocupa de media una letra a `p` puntos: algo mas de medio
    // cuadratin (en veintavos de punto).
    let medir = |p: f32, v: &[f32]| -> Vec<f32> { v.iter().map(|l| aire + l * p * 11.0).collect() };
    let cabe = |p: f32, ancho: u32| medir(p, &palabra).iter().sum::<f32>() <= ancho as f32;
    let menores = |minimo: f32| {
        [1.0f32, 0.9, 0.8, 0.75]
            .into_iter()
            .map(move |k| pt * k)
            .filter(move |p| *p >= minimo)
    };
    let eleccion = menores(9.0f32.min(pt))
        .find(|p| cabe(*p, TEXTO_DE_PIE))
        .map(|p| (false, p))
        .or_else(|| {
            menores(8.0f32.min(pt))
                .chain([8.0f32.min(pt)])
                .find(|p| cabe(*p, TEXTO_TUMBADO))
                .map(|p| (true, p))
        });
    let (tumbada, p) = eleccion.unwrap_or((true, 8.0f32.min(pt)));
    let ancho = if tumbada { TEXTO_TUMBADO } else { TEXTO_DE_PIE } as f32;
    let tam = ((p - pt).abs() > 0.01).then(|| (p * 2.0).round() as u32);
    let minimo = medir(p, &palabra);
    let quiere: Vec<f32> = medir(p, &renglon)
        .iter()
        .zip(&minimo)
        .map(|(q, m)| q.max(*m))
        .collect();
    let suma_min: f32 = minimo.iter().sum();
    let suma_quiere: f32 = quiere.iter().sum();
    let anchos: Vec<f32> = if suma_quiere <= ancho {
        quiere.iter().map(|q| q * ancho / suma_quiere).collect()
    } else if suma_min <= ancho {
        let sobra = ancho - suma_min;
        let falta = (suma_quiere - suma_min).max(1.0);
        minimo
            .iter()
            .zip(&quiere)
            .map(|(m, q)| m + (q - m) * sobra / falta)
            .collect()
    } else {
        minimo.iter().map(|m| m * ancho / suma_min).collect()
    };
    let mut enteros: Vec<u32> = anchos.iter().map(|a| a.floor().max(200.0) as u32).collect();
    // Que sumen justo el ancho: lo que sobre del redondeo, a la ultima.
    let suma: u32 = enteros.iter().sum();
    if let Some(u) = enteros.last_mut()
        && suma < ancho as u32
    {
        *u += ancho as u32 - suma;
    }
    Reparto {
        anchos: enteros,
        tumbada,
        tam,
    }
}
