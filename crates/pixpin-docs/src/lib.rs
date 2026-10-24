//! **Documentos leidos dentro de PixPin**: Word (`.docx`), libros (`.epub`)
//! y paginas (`.html`), convertidos a algo que el visor pueda pintar.
//!
//! Es el puerto de lo que el movil anadio entre la v0.61 y la v0.66
//! (`motor/DocxAHtml.kt`, `motor/EpubAHtml.kt`, `motor/Lectura.kt`): un
//! `.docx` o un `.epub` de la conversacion se abria «con otra aplicacion»,
//! y en medio ordenador no hay ninguna que lo abra sin pedir cuenta.
//!
//! **Este crate es puro.** No sabe que existe Windows, no abre ventanas y
//! no pinta: lee ficheros y devuelve [`documento::Documento`], que es una
//! lista de bloques con estilo. Quien pinta es `apps/pixpin/src/visor.rs`.
//! Por eso todo lo de aqui se prueba con documentos pequenos construidos en
//! la propia prueba, sin un solo fichero binario en el repositorio.
//!
//! **Sin dependencias nuevas**: un `.docx` y un `.epub` son ZIP con XML
//! dentro, asi que basta el lector de ZIP que `pixpin-proyecto` ya usa para
//! el `.pixpin` y un lector de XML propio ([`xml`]) que no abre la puerta a
//! las entidades externas. Es la misma norma que el movil se puso.

#![forbid(unsafe_code)]

pub mod auto_desplazar;
pub mod buscar;
pub mod documento;
pub mod docx;
pub mod epub;
/// Una nota Markdown exportada a Word (H12, 1-oct).
pub mod exportar_docx;
pub mod html;
pub mod indice;
pub mod lectura;
pub mod md_bloques;
pub mod md_comandos;
pub mod md_comentarios;
pub mod md_edicion;
pub mod md_imagen;
pub mod md_tabla;
pub mod md_tabla_html;
pub mod md_vivo;
pub mod pdf;
/// Un PDF pasado a texto para leerlo como un Word (Android v0.92-v0.93).
pub mod pdf_a_texto;
pub mod tabla;
pub mod vista;
pub mod voz_alta;
pub mod xml;

pub use documento::{Bloque, Clase, Documento, Estilo, Trozo};

use std::io::{Read, Seek};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum ErrorDocs {
    /// Lo que se le ensena al usuario cuando el archivo no es lo que
    /// parece. El texto ya viene escrito para leerse.
    #[error("{0}")]
    NoSeLee(String),
    /// Ni siquiera es un ZIP. Es interno: quien llama lo traduce a un
    /// [`ErrorDocs::NoSeLee`] con el mensaje que toque segun el formato.
    #[error("el archivo no es un ZIP")]
    NoEsZip,
    #[error("no se pudo leer el archivo: {0}")]
    Disco(#[from] std::io::Error),
}

/// Que sabe abrir este crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Formato {
    Docx,
    Epub,
    Html,
    /// Texto llano y Markdown: no hace falta traducir nada, pero el visor
    /// tambien los ensena (y el Markdown, con su formato).
    Texto,
    Markdown,
}

/// Que es este fichero por su nombre, o nada si no es de los que se abren.
pub fn formato_de(nombre: &str) -> Option<Formato> {
    if docx::es_docx(nombre) {
        return Some(Formato::Docx);
    }
    match extension(nombre).as_str() {
        "epub" => Some(Formato::Epub),
        "html" | "htm" | "xhtml" => Some(Formato::Html),
        "md" | "markdown" => Some(Formato::Markdown),
        "txt" | "log" | "csv" => Some(Formato::Texto),
        _ => None,
    }
}

/// Lee lo que sea que haya en esa ruta, si es de los que se abren.
///
/// Es la puerta de entrada del crate: el visor llama aqui y ya.
pub fn abrir(ruta: &Path) -> Result<Documento, ErrorDocs> {
    let nombre = nombre(ruta);
    let titulo = sin_extension(&nombre);
    match formato_de(&nombre) {
        Some(Formato::Docx) => docx::leer(ruta, &titulo),
        Some(Formato::Epub) => epub::leer(ruta, &titulo),
        Some(Formato::Html) => {
            let crudo = std::fs::read(ruta)?;
            Ok(html::pagina(&texto_de(&crudo), &titulo))
        }
        Some(Formato::Markdown) | Some(Formato::Texto) => {
            let crudo = std::fs::read(ruta)?;
            Ok(html::llano(&texto_de(&crudo), &titulo))
        }
        None => Err(ErrorDocs::NoSeLee(format!(
            "PixPin no sabe abrir «{nombre}»"
        ))),
    }
}

/// El nombre del fichero, sin la carpeta.
pub fn nombre(ruta: &Path) -> String {
    ruta.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// La extension en minusculas, sin el punto; vacia si no la trae.
pub fn extension(nombre: &str) -> String {
    match nombre.rsplit_once('.') {
        Some((antes, ext)) if !antes.is_empty() && !ext.contains(['/', '\\']) => ext.to_lowercase(),
        _ => String::new(),
    }
}

/// El nombre sin su extension: lo que el movil ensena en la pastilla.
pub fn sin_extension(nombre: &str) -> String {
    match nombre.rsplit_once('.') {
        Some((antes, _)) if !antes.is_empty() => antes.to_string(),
        _ => nombre.to_string(),
    }
}

/// El tipo MIME de una imagen por su extension, o nada si no hay navegador
/// que la pinte (EMF y WMF son dibujos de Windows).
pub fn mime_de_imagen(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "png" => "image/png",
        "jpg" | "jpeg" | "jpe" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        _ => return None,
    })
}

/// Un `.doc` de los de antes empieza por la firma de OLE, no por la del
/// ZIP. Sirve para decirle al usuario que es un Word antiguo y no «esto no
/// es un Word».
pub(crate) fn empieza_por_ole(ruta: &Path) -> bool {
    let mut cabecera = [0u8; 4];
    std::fs::File::open(ruta)
        .and_then(|mut f| f.read_exact(&mut cabecera))
        .is_ok()
        && cabecera[0] == 0xD0
        && cabecera[1] == 0xCF
}

/// Unos bytes como texto: UTF-8 si nadie dice otra cosa; lo que diga la
/// marca de orden o la declaracion `<?xml encoding=…?>` si lo dicen.
///
/// Es el `texto()` de `EpubAHtml.kt`. Aqui solo se entienden UTF-8 y los
/// dos UTF-16, que es con lo que se hace hoy todo Office y todo EPUB; un
/// juego de los viejos (`windows-1252`) se lee como Latin-1, que acierta en
/// las letras acentuadas y es infinitamente mejor que no abrir el fichero.
pub fn texto_de(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return utf16(&bytes[2..], true);
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return utf16(&bytes[2..], false);
    }
    let sin_marca = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    match std::str::from_utf8(sin_marca) {
        Ok(s) => s.to_string(),
        // No es UTF-8 valido: Latin-1, que no falla nunca y conserva las
        // letras acentuadas de un fichero de Europa occidental.
        Err(_) => sin_marca.iter().map(|&b| b as char).collect(),
    }
}

fn utf16(bytes: &[u8], grande: bool) -> String {
    let unidades: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|p| {
            if grande {
                u16::from_be_bytes([p[0], p[1]])
            } else {
                u16::from_le_bytes([p[0], p[1]])
            }
        })
        .collect();
    String::from_utf16_lossy(&unidades)
}

/// Un ZIP abierto: es lo que son por dentro un `.docx` y un `.epub`.
pub struct Paquete<'a> {
    zip: zip::ZipArchive<Box<dyn LeerYBuscar + 'a>>,
}

trait LeerYBuscar: Read + Seek {}
impl<T: Read + Seek> LeerYBuscar for T {}

impl<'a> Paquete<'a> {
    pub fn abrir(ruta: &Path) -> Result<Self, ErrorDocs> {
        let f = std::fs::File::open(ruta)?;
        Self::de_lector(Box::new(f))
    }

    pub fn de_bytes(bytes: &'a [u8]) -> Result<Self, ErrorDocs> {
        Self::de_lector(Box::new(std::io::Cursor::new(bytes)))
    }

    fn de_lector(lector: Box<dyn LeerYBuscar + 'a>) -> Result<Self, ErrorDocs> {
        let zip = zip::ZipArchive::new(lector).map_err(|_| ErrorDocs::NoEsZip)?;
        Ok(Self { zip })
    }

    /// Los nombres de todo lo que hay dentro.
    pub fn nombres(&self) -> Vec<String> {
        self.zip.file_names().map(|s| s.to_string()).collect()
    }

    /// Los bytes de una entrada, o nada si no esta o si pasa del tope.
    ///
    /// El tamano que el ZIP declara **puede mentir** (una bomba de
    /// descompresion declara cuatro kilos y son cuatro gigas), asi que
    /// ademas se cuenta lo que sale de verdad y se corta.
    pub fn bytes(&mut self, ruta: &str, tope: u64) -> Option<Vec<u8>> {
        let mut e = self.zip.by_name(ruta).ok()?;
        if e.is_dir() || e.size() > tope {
            return None;
        }
        let mut salida = Vec::with_capacity(e.size().min(tope) as usize);
        // Un byte mas que el tope: si se llega a leerlo, es que se pasaba.
        let mut limitado = e.by_ref().take(tope + 1);
        limitado.read_to_end(&mut salida).ok()?;
        if salida.len() as u64 > tope {
            return None;
        }
        Some(salida)
    }

    /// Una entrada como texto.
    pub fn texto(&mut self, ruta: &str, tope: u64) -> Option<String> {
        self.bytes(ruta, tope).map(|b| texto_de(&b))
    }

    /// Una entrada leida como XML.
    pub fn xml(&mut self, ruta: &str) -> Option<xml::Nodo> {
        // Ningun `document.xml` ni ningun `.opf` honrado pasa de esto, y un
        // XML de cien megas no es un documento: es un ataque.
        const TOPE: u64 = 64 * 1024 * 1024;
        xml::leer(&self.texto(ruta, TOPE)?)
    }
}

/// Los `.docx` y `.epub` de juguete que usan las pruebas de todo el crate.
///
/// Se fabrican aqui, en memoria, para que el repositorio no tenga que
/// guardar ni un binario y para que cada prueba diga exactamente que hay
/// dentro del documento que esta comprobando.
#[cfg(test)]
pub(crate) mod pruebas {
    use std::io::Write;

    /// Un ZIP con esas entradas, sin comprimir (que es igual de valido y
    /// hace la prueba legible si alguna vez hay que mirarla con un editor).
    pub fn zip_de(entradas: &[(&str, &[u8])]) -> Vec<u8> {
        let mut salida = Vec::new();
        {
            let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut salida));
            let opciones: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            for (nombre, datos) in entradas {
                z.start_file(*nombre, opciones).unwrap();
                z.write_all(datos).unwrap();
            }
            z.finish().unwrap();
        }
        salida
    }

    /// Un `.docx` con ese `<w:body>` y esas imagenes. Las relaciones se
    /// escriben solas: `rId5` apunta a la primera imagen, `rId6` a la
    /// segunda, y asi.
    pub fn docx_de(cuerpo: &str, medios: &[(&str, &[u8])]) -> Vec<u8> {
        let documento = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
            xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
<w:body>{cuerpo}</w:body></w:document>"#
        );
        let mut rels = String::from(
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
        );
        for (i, (ruta, _)) in medios.iter().enumerate() {
            let destino = ruta.strip_prefix("word/").unwrap_or(ruta);
            rels.push_str(&format!(
                r#"<Relationship Id="rId{}" Type="image" Target="{destino}"/>"#,
                5 + i
            ));
        }
        rels.push_str("</Relationships>");

        let mut entradas: Vec<(&str, Vec<u8>)> = vec![
            ("word/document.xml", documento.into_bytes()),
            ("word/_rels/document.xml.rels", rels.into_bytes()),
        ];
        for (ruta, datos) in medios {
            entradas.push((ruta, datos.to_vec()));
        }
        let prestadas: Vec<(&str, &[u8])> =
            entradas.iter().map(|(n, d)| (*n, d.as_slice())).collect();
        zip_de(&prestadas)
    }
}

#[cfg(test)]
mod comprobaciones {
    use super::*;

    #[test]
    fn la_extension_sale_en_minusculas_y_sin_punto() {
        assert_eq!(extension("Apuntes.DOCX"), "docx");
        assert_eq!(extension("sin_punto"), "");
        assert_eq!(extension(".oculto"), "", "un punto delante no es extension");
        assert_eq!(extension("c:/carpeta.vieja/fichero"), "");
    }

    #[test]
    fn el_nombre_sin_extension_deja_los_puntos_de_en_medio() {
        assert_eq!(sin_extension("apuntes.de.clase.docx"), "apuntes.de.clase");
        assert_eq!(sin_extension("sin_punto"), "sin_punto");
        assert_eq!(sin_extension(".oculto"), ".oculto");
    }

    #[test]
    fn se_reconoce_lo_que_se_abre_y_solo_eso() {
        assert_eq!(formato_de("a.docx"), Some(Formato::Docx));
        assert_eq!(formato_de("a.EPUB"), Some(Formato::Epub));
        assert_eq!(formato_de("a.htm"), Some(Formato::Html));
        assert_eq!(formato_de("a.md"), Some(Formato::Markdown));
        assert_eq!(formato_de("a.txt"), Some(Formato::Texto));
        assert_eq!(formato_de("a.pdf"), None, "el PDF lo abre pixpin-pdf");
        assert_eq!(formato_de("a.doc"), None);
        assert_eq!(formato_de("a.png"), None);
    }

    #[test]
    fn el_texto_se_lee_con_la_marca_de_orden_que_traiga() {
        assert_eq!(texto_de(b"hola"), "hola");
        assert_eq!(texto_de(b"\xEF\xBB\xBFhola"), "hola");
        assert_eq!(texto_de(b"\xFF\xFEh\0o\0l\0a\0"), "hola");
        assert_eq!(texto_de(b"\xFE\xFF\0h\0o\0l\0a"), "hola");
        // Latin-1 no es UTF-8 valido y aun asi tiene que salir la enye.
        assert_eq!(texto_de(b"a\xF1o"), "año");
    }

    #[test]
    fn una_entrada_que_miente_sobre_su_tamano_no_se_lee_entera() {
        let bytes = pruebas::zip_de(&[("grande.bin", &vec![7u8; 5000])]);
        let mut p = Paquete::de_bytes(&bytes).unwrap();
        assert!(
            p.bytes("grande.bin", 100).is_none(),
            "el tope tiene que cortar"
        );
        assert_eq!(p.bytes("grande.bin", 10_000).map(|v| v.len()), Some(5000));
        assert!(p.bytes("no-existe.bin", 10_000).is_none());
    }

    #[test]
    fn lo_que_no_es_un_zip_no_se_abre() {
        let salio = Paquete::de_bytes(b"esto no es un ZIP ni de lejos").err();
        assert!(matches!(salio, Some(ErrorDocs::NoEsZip)), "salio {salio:?}");
    }
}
