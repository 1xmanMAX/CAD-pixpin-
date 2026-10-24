//! **Un libro (`.epub`) leido dentro.** Puerto de `motor/EpubAHtml.kt` del
//! movil (v0.63, 452 lineas).
//!
//! Un EPUB ya *es* una web metida en un ZIP —capitulos en XHTML, un indice
//! (`.opf`) que dice en que orden van—, asi que aqui no se traduce nada: se
//! cosen los capitulos **en el orden del lomo** (`spine`) y cada uno pasa
//! por [`crate::html`], que es el mismo lector que abre un `.html` suelto.
//!
//! **El CSS del libro se tira entero.** Esta pensado para un lector de
//! tinta y aqui no pinta nada: quien decide como se lee es el visor, con la
//! letra que el usuario haya elegido.
//!
//! Dos topes que no estaban de mas en el movil y aqui tampoco: nada suelto
//! mayor de 25 MB sale del ZIP, y las imagenes de todo el libro juntas no
//! pasan de 48 MB. Un libro ilustrado trae cientos y el documento entero
//! vive en memoria mientras se lee.

use crate::documento::{Bloque, Clase, Documento, Imagen, Trozo};
use crate::{ErrorDocs, Paquete};
use std::collections::BTreeMap;
use std::path::Path;

/// Nada suelto mayor que esto sale del ZIP: ni una imagen ni un capitulo.
const TOPE_DE_RECURSO: u64 = 25 * 1024 * 1024;

/// Y todas las imagenes juntas no pasan de aqui.
const TOPE_DE_IMAGENES: usize = 48 * 1024 * 1024;

const NO_ES_EPUB: &str = "El archivo no es un libro EPUB";

const IMAGENES: [&str; 6] = ["png", "jpg", "jpeg", "gif", "webp", "svg"];

pub fn es_epub(nombre: &str) -> bool {
    crate::extension(nombre) == "epub"
}

pub fn leer(ruta: &Path, titulo: &str) -> Result<Documento, ErrorDocs> {
    let mut paquete = Paquete::abrir(ruta).map_err(|e| match e {
        ErrorDocs::NoEsZip => ErrorDocs::NoSeLee(NO_ES_EPUB.into()),
        otro => otro,
    })?;
    de_paquete(&mut paquete, titulo)
}

pub fn de_paquete(paquete: &mut Paquete, titulo_de_fuera: &str) -> Result<Documento, ErrorDocs> {
    let contenedor = paquete
        .xml("META-INF/container.xml")
        .ok_or_else(|| no_es(": le falta META-INF/container.xml"))?;
    let ruta_opf = contenedor
        .descendientes("rootfile")
        .iter()
        .map(|r| descodificar(r.atributo("full-path")))
        .map(|r| r.trim_start_matches('/').to_string())
        .find(|r| !r.trim().is_empty())
        .ok_or_else(|| no_es(": no dice donde esta su indice"))?;
    let opf = paquete
        .xml(&ruta_opf)
        .ok_or_else(|| no_es(&format!(": no se puede leer su indice ({ruta_opf})")))?;
    let dir_opf = ruta_opf.rsplit_once('/').map(|(d, _)| d).unwrap_or("");

    // El manifiesto: id → ruta entera dentro del ZIP, y su tipo.
    let mut rutas: BTreeMap<String, String> = BTreeMap::new();
    let mut tipos: BTreeMap<String, String> = BTreeMap::new();
    if let Some(manifiesto) = opf.hijo("manifest") {
        for item in manifiesto.elementos() {
            if item.nombre != "item" {
                continue;
            }
            let Some(ruta) = resolver(dir_opf, item.atributo("href")) else {
                continue;
            };
            rutas.insert(item.atributo("id").to_string(), ruta);
            tipos.insert(
                item.atributo("id").to_string(),
                item.atributo("media-type").to_lowercase(),
            );
        }
    }

    // El lomo: el orden de lectura. Lo que no este en el ZIP o no sea
    // pagina, fuera.
    let dentro = paquete.nombres();
    let mut hojas: Vec<Hoja> = Vec::new();
    if let Some(lomo) = opf.hijo("spine") {
        for referencia in lomo.elementos() {
            if referencia.nombre != "itemref" {
                continue;
            }
            let id = referencia.atributo("idref");
            let Some(ruta) = rutas.get(id) else { continue };
            if !dentro.iter().any(|n| n == ruta) {
                continue;
            }
            let ext = crate::extension(ruta);
            let tipo = tipos.get(id).cloned().unwrap_or_default();
            if IMAGENES.contains(&ext.as_str()) || tipo.starts_with("image/") {
                hojas.push(Hoja {
                    ruta: ruta.clone(),
                    es_imagen: true,
                });
            } else if tipo.contains("html")
                || matches!(ext.as_str(), "xhtml" | "html" | "htm" | "xml")
            {
                hojas.push(Hoja {
                    ruta: ruta.clone(),
                    es_imagen: false,
                });
            }
        }
    }
    if hojas.is_empty() {
        return Err(ErrorDocs::NoSeLee(
            "El libro no tiene capitulos que leer".into(),
        ));
    }
    if protegido(paquete, &hojas) {
        return Err(ErrorDocs::NoSeLee(
            "Este libro esta protegido (DRM) y no se puede leer aqui".into(),
        ));
    }

    let metadatos = opf.hijo("metadata");
    let titulo = metadatos
        .map(|m| m.descendientes("title"))
        .unwrap_or_default()
        .iter()
        .map(|e| e.texto().trim().to_string())
        .find(|t| !t.is_empty())
        .unwrap_or_else(|| titulo_de_fuera.to_string());
    let autor = metadatos
        .map(|m| m.descendientes("creator"))
        .unwrap_or_default()
        .iter()
        .map(|e| e.texto().trim().to_string())
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join(", ");

    let mut doc = Documento {
        titulo,
        autor,
        ..Default::default()
    };
    let mut gastado = 0usize;
    for (i, hoja) in hojas.iter().enumerate() {
        // Un capitulo en blanco (los hay: hojas de cortesia) no gasta un
        // separador.
        let antes = doc.bloques.len();
        if i > 0 {
            doc.bloques
                .push(Bloque::nuevo(Clase::Capitulo, vec![Trozo::llano(" ")]));
        }
        if hoja.es_imagen {
            if let Some(im) = imagen(paquete, &hoja.ruta, &mut gastado) {
                doc.imagenes.push(im);
                doc.bloques
                    .push(Bloque::nota(crate::documento::MARCA_IMAGEN));
            }
        } else {
            capitulo(paquete, hoja, &mut doc, &mut gastado);
        }
        if doc.bloques.len() == antes + usize::from(i > 0) {
            doc.bloques.truncate(antes);
        }
    }
    if doc.letras() == 0 && doc.imagenes.is_empty() {
        return Err(ErrorDocs::NoSeLee(
            "El libro no tiene capitulos que leer".into(),
        ));
    }
    Ok(doc)
}

/// Un documento del lomo: su ruta dentro del ZIP y si es una imagen suelta
/// (una portada) y no XHTML.
struct Hoja {
    ruta: String,
    es_imagen: bool,
}

fn capitulo(paquete: &mut Paquete, hoja: &Hoja, doc: &mut Documento, gastado: &mut usize) {
    let Some(texto) = paquete.texto(&hoja.ruta, TOPE_DE_RECURSO) else {
        return;
    };
    let dir = hoja.ruta.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    // Las imagenes del capitulo se sacan del propio libro. Se recogen aqui
    // porque el analizador de HTML no sabe nada de ZIP ni tiene por que.
    let mut pedidas: Vec<String> = Vec::new();
    let mut imagenes = Vec::new();
    let bloques = crate::html::analizar(
        &texto,
        &mut |src| {
            let ruta = ruta_de_imagen(dir, src)?;
            pedidas.push(ruta);
            // Se devuelve una imagen vacia como senal de «esta si»: los
            // bytes se ponen despues, cuando se puede tocar el ZIP.
            Some(Imagen {
                mime: String::new(),
                datos: Vec::new(),
            })
        },
        &mut imagenes,
    );
    // Cada marca de imagen del capitulo, en orden, con su ruta.
    let mut buenas: Vec<Imagen> = Vec::new();
    let mut cuales: Vec<bool> = Vec::new();
    for ruta in &pedidas {
        match imagen(paquete, ruta, gastado) {
            Some(im) => {
                buenas.push(im);
                cuales.push(true);
            }
            None => cuales.push(false),
        }
    }
    // Los bloques de marca cuya imagen no salio del ZIP se tiran, para que
    // las marcas y las imagenes sigan yendo una a una.
    let mut k = 0usize;
    for b in bloques {
        if b.clase == Clase::Nota && b.texto() == crate::documento::MARCA_IMAGEN {
            let vale = cuales.get(k).copied().unwrap_or(false);
            k += 1;
            if !vale {
                continue;
            }
        }
        doc.bloques.push(b);
    }
    doc.imagenes.append(&mut buenas);
}

/// Los bytes de una imagen del libro, contando lo que ya se lleva gastado.
fn imagen(paquete: &mut Paquete, ruta: &str, gastado: &mut usize) -> Option<Imagen> {
    let mime = crate::mime_de_imagen(&crate::extension(ruta))?;
    if *gastado >= TOPE_DE_IMAGENES {
        return None;
    }
    let datos = paquete.bytes(ruta, TOPE_DE_RECURSO)?;
    *gastado += datos.len();
    Some(Imagen {
        mime: mime.to_string(),
        datos,
    })
}

/// Adonde apunta el `src` de una imagen, o nada si no es una imagen de
/// dentro del libro (una de la red no se baja: la pagina no sale a
/// internet).
fn ruta_de_imagen(dir: &str, src: &str) -> Option<String> {
    let u = src.trim();
    if u.is_empty() || u.starts_with("data:") || u.starts_with("//") {
        return None;
    }
    // Un esquema delante («http:», «file:») es de fuera del libro.
    if u.split_once(':')
        .is_some_and(|(e, _)| !e.is_empty() && e.chars().all(|c| c.is_ascii_alphanumeric()))
    {
        return None;
    }
    let sin_ancla = u.split(['#', '?']).next().unwrap_or(u);
    let ruta = resolver(dir, sin_ancla)?;
    IMAGENES
        .contains(&crate::extension(&ruta).as_str())
        .then_some(ruta)
}

/// Con DRM, `META-INF/encryption.xml` lista lo cifrado. Que liste
/// **tipografias** es lo normal y no estorba (es ofuscacion, y las fuentes
/// no se usan); que liste los capitulos es que el libro no se lee.
fn protegido(paquete: &mut Paquete, hojas: &[Hoja]) -> bool {
    let Some(cifrado) = paquete.xml("META-INF/encryption.xml") else {
        return false;
    };
    let cifradas: Vec<String> = cifrado
        .descendientes("CipherReference")
        .iter()
        .filter_map(|e| resolver("", e.atributo("URI")))
        .collect();
    hojas.iter().any(|h| cifradas.contains(&h.ruta))
}

fn no_es(cola: &str) -> ErrorDocs {
    ErrorDocs::NoSeLee(format!("{NO_ES_EPUB}{cola}"))
}

/// `relativa` vista desde `dir`, las dos dentro del ZIP, con los `.` y `..`
/// resueltos. Nada si se sale por arriba: esa ruta no es de este libro.
pub(crate) fn resolver(dir: &str, relativa: &str) -> Option<String> {
    let limpia = descodificar(relativa.trim()).replace('\\', "/");
    if limpia.is_empty() {
        return None;
    }
    let mut pila: Vec<&str> = Vec::new();
    if !limpia.starts_with('/') {
        pila.extend(dir.split('/').filter(|t| !t.is_empty()));
    }
    for trozo in limpia.split('/') {
        match trozo {
            "" | "." => {}
            ".." => {
                // Salirse por arriba es justo lo que intenta un ZIP con
                // trampa: no hay ruta que devolver.
                pila.pop()?;
            }
            otro => pila.push(otro),
        }
    }
    if pila.is_empty() {
        return None;
    }
    Some(pila.join("/"))
}

/// «Cap%C3%ADtulo%201.xhtml» → «Capítulo 1.xhtml». No es un descodificador
/// de formularios: aqui un `+` es un `+`.
pub(crate) fn descodificar(s: &str) -> String {
    if !s.contains('%') {
        return s.to_string();
    }
    let bytes = s.as_bytes();
    let mut crudo: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            crudo.push(v);
            i += 3;
            continue;
        }
        crudo.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&crudo).to_string()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::pruebas::zip_de;

    /// Un EPUB de juguete: el contenedor, un `.opf` con su lomo y los
    /// capitulos que se le pidan.
    fn epub_de(capitulos: &[(&str, &str)], extra: &[(&str, &[u8])]) -> Vec<u8> {
        let mut manifiesto = String::new();
        let mut lomo = String::new();
        for (i, (ruta, _)) in capitulos.iter().enumerate() {
            let nombre = ruta.strip_prefix("OEBPS/").unwrap_or(ruta);
            manifiesto.push_str(&format!(
                r#"<item id="c{i}" href="{nombre}" media-type="application/xhtml+xml"/>"#
            ));
            lomo.push_str(&format!(r#"<itemref idref="c{i}"/>"#));
        }
        let opf = format!(
            r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0">
<metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
<dc:title>El libro</dc:title><dc:creator>Quien sea</dc:creator><dc:language>es</dc:language>
</metadata><manifest>{manifiesto}</manifest><spine>{lomo}</spine></package>"#
        );
        let contenedor = r#"<?xml version="1.0"?><container version="1.0"
xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
<rootfiles><rootfile full-path="OEBPS/libro.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#;

        let mut entradas: Vec<(&str, Vec<u8>)> = vec![
            ("META-INF/container.xml", contenedor.as_bytes().to_vec()),
            ("OEBPS/libro.opf", opf.into_bytes()),
        ];
        for (ruta, cuerpo) in capitulos {
            entradas.push((ruta, cuerpo.as_bytes().to_vec()));
        }
        for (ruta, datos) in extra {
            entradas.push((ruta, datos.to_vec()));
        }
        let prestadas: Vec<(&str, &[u8])> =
            entradas.iter().map(|(n, d)| (*n, d.as_slice())).collect();
        zip_de(&prestadas)
    }

    fn leer_epub(bytes: &[u8]) -> Result<Documento, ErrorDocs> {
        let mut p = Paquete::de_bytes(bytes).expect("tiene que ser un ZIP");
        de_paquete(&mut p, "nombre del fichero")
    }

    #[test]
    fn los_capitulos_salen_en_el_orden_del_lomo() {
        let bytes = epub_de(
            &[
                (
                    "OEBPS/uno.xhtml",
                    "<html><body><p>primero</p></body></html>",
                ),
                (
                    "OEBPS/dos.xhtml",
                    "<html><body><p>segundo</p></body></html>",
                ),
            ],
            &[],
        );
        let d = leer_epub(&bytes).unwrap();
        let textos: Vec<String> = d
            .bloques
            .iter()
            .filter(|b| b.clase != Clase::Capitulo)
            .map(|b| b.texto())
            .collect();
        assert_eq!(textos, vec!["primero".to_string(), "segundo".to_string()]);
        assert!(
            d.bloques.iter().any(|b| b.clase == Clase::Capitulo),
            "entre capitulo y capitulo tiene que haber separacion"
        );
    }

    #[test]
    fn el_titulo_y_el_autor_salen_de_los_metadatos_del_libro() {
        let bytes = epub_de(&[("OEBPS/uno.xhtml", "<body><p>a</p></body>")], &[]);
        let d = leer_epub(&bytes).unwrap();
        assert_eq!(d.titulo, "El libro");
        assert_eq!(d.autor, "Quien sea");
    }

    #[test]
    fn el_doctype_y_las_entidades_de_un_capitulo_no_tumban_la_lectura() {
        // Es lo que obligo al movil a tratar los capitulos como texto: un
        // analizador de XML estricto los rechaza enteros.
        let bytes = epub_de(
            &[(
                "OEBPS/uno.xhtml",
                r#"<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.1//EN" "http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd">
<html><body><p>uno&nbsp;dos</p></body></html>"#,
            )],
            &[],
        );
        let d = leer_epub(&bytes).unwrap();
        assert_eq!(d.bloques[0].texto(), "uno\u{A0}dos");
    }

    #[test]
    fn las_imagenes_del_capitulo_salen_del_propio_libro() {
        let bytes = epub_de(
            &[(
                "OEBPS/uno.xhtml",
                r#"<body><p>foto</p><img src="img/a.png"/></body>"#,
            )],
            &[("OEBPS/img/a.png", b"datos")],
        );
        let d = leer_epub(&bytes).unwrap();
        assert_eq!(d.imagenes.len(), 1);
        assert_eq!(d.imagenes[0].datos, b"datos");
        assert_eq!(d.imagenes[0].mime, "image/png");
    }

    #[test]
    fn una_imagen_de_fuera_del_libro_no_se_baja() {
        let bytes = epub_de(
            &[(
                "OEBPS/uno.xhtml",
                r#"<body><p>t</p><img src="https://ejemplo/a.png"/><img src="../../fuera.png"/></body>"#,
            )],
            &[],
        );
        let d = leer_epub(&bytes).unwrap();
        assert!(d.imagenes.is_empty(), "{:?}", d.imagenes);
    }

    #[test]
    fn un_libro_con_drm_lo_dice_en_vez_de_ensenar_ruido() {
        let cifrado = br#"<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
<EncryptedData><CipherData><CipherReference URI="OEBPS/uno.xhtml"/></CipherData></EncryptedData></encryption>"#;
        let bytes = epub_de(
            &[("OEBPS/uno.xhtml", "<body><p>cifrado</p></body>")],
            &[("META-INF/encryption.xml", cifrado)],
        );
        let e = leer_epub(&bytes).unwrap_err();
        assert!(
            matches!(&e, ErrorDocs::NoSeLee(m) if m.contains("DRM")),
            "salio {e:?}"
        );
    }

    #[test]
    fn unas_tipografias_ofuscadas_no_son_drm() {
        let cifrado = br#"<encryption><EncryptedData><CipherData>
<CipherReference URI="OEBPS/fuentes/a.otf"/></CipherData></EncryptedData></encryption>"#;
        let bytes = epub_de(
            &[("OEBPS/uno.xhtml", "<body><p>se lee</p></body>")],
            &[("META-INF/encryption.xml", cifrado)],
        );
        assert_eq!(leer_epub(&bytes).unwrap().bloques[0].texto(), "se lee");
    }

    #[test]
    fn sin_contenedor_no_hay_libro() {
        let bytes = zip_de(&[("hola.txt", b"nada")]);
        let e = leer_epub(&bytes).unwrap_err();
        assert!(
            matches!(&e, ErrorDocs::NoSeLee(m) if m.contains("container.xml")),
            "salio {e:?}"
        );
    }

    #[test]
    fn un_lomo_vacio_lo_dice() {
        let bytes = epub_de(&[], &[]);
        let e = leer_epub(&bytes).unwrap_err();
        assert!(
            matches!(&e, ErrorDocs::NoSeLee(m) if m.contains("capitulos")),
            "salio {e:?}"
        );
    }

    #[test]
    fn una_hoja_del_lomo_que_no_esta_en_el_zip_se_salta() {
        // El `.opf` puede prometer mas de lo que el ZIP trae; el resto del
        // libro tiene que leerse igual.
        let mut bytes = epub_de(
            &[
                ("OEBPS/uno.xhtml", "<body><p>si esta</p></body>"),
                ("OEBPS/falta.xhtml", "<body><p>no</p></body>"),
            ],
            &[],
        );
        // Se rehace el ZIP sin el segundo capitulo, dejando el `.opf` como
        // estaba.
        let p = Paquete::de_bytes(&bytes).unwrap();
        assert!(p.nombres().iter().any(|n| n == "OEBPS/falta.xhtml"));
        drop(p);
        bytes = sin_entrada(&bytes, "OEBPS/falta.xhtml");
        let d = leer_epub(&bytes).unwrap();
        assert_eq!(d.bloques.len(), 1, "{:?}", d.bloques);
        assert_eq!(d.bloques[0].texto(), "si esta");
    }

    /// Rehace el ZIP sin una de sus entradas.
    fn sin_entrada(bytes: &[u8], fuera: &str) -> Vec<u8> {
        let mut p = Paquete::de_bytes(bytes).unwrap();
        let nombres: Vec<String> = p.nombres().into_iter().filter(|n| n != fuera).collect();
        let datos: Vec<(String, Vec<u8>)> = nombres
            .iter()
            .map(|n| (n.clone(), p.bytes(n, 10_000_000).unwrap_or_default()))
            .collect();
        let prestadas: Vec<(&str, &[u8])> = datos
            .iter()
            .map(|(n, d)| (n.as_str(), d.as_slice()))
            .collect();
        zip_de(&prestadas)
    }

    #[test]
    fn resolver_no_deja_salir_de_la_carpeta() {
        assert_eq!(
            resolver("OEBPS", "img/a.png").as_deref(),
            Some("OEBPS/img/a.png")
        );
        assert_eq!(
            resolver("OEBPS/texto", "../img/a.png").as_deref(),
            Some("OEBPS/img/a.png")
        );
        assert_eq!(
            resolver("OEBPS", "/otra/a.png").as_deref(),
            Some("otra/a.png")
        );
        assert_eq!(resolver("", "../fuera.png"), None, "no se sale por arriba");
        assert_eq!(resolver("OEBPS", "../../../fuera.png"), None);
        assert_eq!(resolver("OEBPS", "  "), None);
    }

    #[test]
    fn los_porcentajes_del_nombre_se_descodifican() {
        assert_eq!(descodificar("Cap%C3%ADtulo%201.xhtml"), "Capítulo 1.xhtml");
        assert_eq!(descodificar("a+b.xhtml"), "a+b.xhtml", "un mas es un mas");
        assert_eq!(descodificar("sin-nada"), "sin-nada");
        assert_eq!(
            descodificar("%zz"),
            "%zz",
            "lo que no es hexadecimal se queda"
        );
    }

    #[test]
    fn es_epub_solo_por_la_extension() {
        assert!(es_epub("libro.epub"));
        assert!(es_epub("LIBRO.EPUB"));
        assert!(!es_epub("libro.mobi"));
        assert!(!es_epub("epub"));
    }
}
