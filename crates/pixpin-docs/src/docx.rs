//! **Un Word (`.docx`) leido de verdad.** Puerto de `motor/DocxAHtml.kt` del
//! movil (v0.61, 323 lineas), bloque a bloque.
//!
//! Aqui no se pretende ser Word: se saca **lo que dice** el documento
//! —parrafos, titulos, negritas, listas, tablas e imagenes— y se deja en la
//! lista de bloques de [`crate::documento`]. Sin maquetacion de pagina: ni
//! margenes, ni columnas, ni encabezados y pies. Lo que se quiere es leerlo.
//!
//! Un `.docx` es un ZIP con XML dentro, asi que no hace falta nada mas que
//! el lector de ZIP que el repositorio ya usa para el `.pixpin` y el XML
//! propio de [`crate::xml`], que no abre la puerta a entidades externas.

use crate::documento::{Alineacion, Bloque, Clase, Documento, Estilo, Imagen, Trozo};
use crate::documento::{MARCA_IMAGEN, SEPARADOR_DE_CELDA};
use crate::tabla::{Celda, FilaDeTabla};
use crate::xml::Nodo;
use crate::{ErrorDocs, Paquete};
use std::collections::BTreeMap;
use std::path::Path;

/// Una imagen mayor que esto no se guarda: en base64 engorda un tercio y
/// una pagina de decenas de megas no la abre bien ni el navegador. Es el
/// mismo tope que el del movil.
const TOPE_DE_IMAGEN: u64 = 12 * 1024 * 1024;

/// Si por el nombre es un Word de los que se leen: `.docx` y sus primos con
/// macros o de plantilla.
pub fn es_docx(nombre: &str) -> bool {
    matches!(
        crate::extension(nombre).as_str(),
        "docx" | "docm" | "dotx" | "dotm"
    )
}

/// Lee el documento entero.
pub fn leer(ruta: &Path, titulo: &str) -> Result<Documento, ErrorDocs> {
    // Un `.doc` de los de antes no es un ZIP y no se arregla intentandolo:
    // se dice claro, como en el movil.
    if matches!(
        crate::extension(&crate::nombre(ruta)).as_str(),
        "doc" | "dot" | "rtf"
    ) {
        return Err(ErrorDocs::NoSeLee(
            "Solo se leen los documentos .docx: guarda este como .docx o PDF".into(),
        ));
    }
    let mut paquete = Paquete::abrir(ruta).map_err(|e| match e {
        ErrorDocs::NoEsZip => {
            if crate::empieza_por_ole(ruta) {
                ErrorDocs::NoSeLee("Es un Word antiguo (.doc): solo se leen los .docx".into())
            } else {
                ErrorDocs::NoSeLee("El archivo no es un documento de Word (.docx)".into())
            }
        }
        otro => otro,
    })?;
    de_paquete(&mut paquete, titulo)
}

/// El mismo trabajo sobre un ZIP ya abierto: es lo que usan las pruebas,
/// que se fabrican el `.docx` en memoria.
pub fn de_paquete(paquete: &mut Paquete, titulo: &str) -> Result<Documento, ErrorDocs> {
    let Some(raiz) = paquete.xml("word/document.xml") else {
        return Err(ErrorDocs::NoSeLee(
            "El archivo no es un documento de Word (.docx)".into(),
        ));
    };
    let relaciones = relaciones(paquete);
    let cuerpo = raiz.hijo("body").cloned().unwrap_or(raiz);
    let pagina = ancho_de_texto(&cuerpo);
    let mut lector = Lector {
        paquete,
        relaciones,
        tablas: 0,
        pagina,
        doc: Documento {
            titulo: titulo.to_string(),
            ..Default::default()
        },
    };
    lector.bloques(&cuerpo);
    Ok(lector.doc)
}

/// id de relacion → ruta dentro del ZIP. Solo las de dentro: un enlace
/// externo no es una imagen de este documento.
fn relaciones(paquete: &mut Paquete) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    let Some(rels) = paquete.xml("word/_rels/document.xml.rels") else {
        return m;
    };
    for r in rels.elementos() {
        if r.nombre != "Relationship" || r.atributo("TargetMode") == "External" {
            continue;
        }
        let destino = r.atributo("Target");
        if destino.is_empty() {
            continue;
        }
        // «media/image1.png» cuelga de `word/`; «/word/media/…» ya viene
        // entera.
        let ruta = match destino.strip_prefix('/') {
            Some(entera) => entera.to_string(),
            None => format!("word/{destino}"),
        };
        m.insert(r.atributo("Id").to_string(), ruta);
    }
    m
}

struct Lector<'a, 'z> {
    paquete: &'a mut Paquete<'z>,
    relaciones: BTreeMap<String, String>,
    doc: Documento,
    /// Cuantas tablas van leidas: numera la siguiente.
    tablas: u32,
    /// El ancho de texto de la pagina (`w:sectPr`), en veintavos de punto;
    /// 0 si el documento no lo dice.
    pagina: u32,
}

impl Lector<'_, '_> {
    /// Lo que hay en un cuerpo, una celda o un envoltorio: parrafos y tablas.
    fn bloques(&mut self, padre: &Nodo) {
        for h in padre.elementos() {
            match h.nombre.as_str() {
                "p" => self.parrafo(h),
                "tbl" => self.tabla(h),
                // Controles de contenido y cambios con seguimiento: lo que
                // interesa esta dentro.
                "sdt" | "sdtContent" | "ins" | "smartTag" | "customXml" => self.bloques(h),
                _ => {}
            }
        }
    }

    fn parrafo(&mut self, p: &Nodo) {
        let propiedades = p.hijo("pPr");
        let estilo = propiedades
            .and_then(|pr| pr.hijo("pStyle"))
            .map(|e| e.valor())
            .unwrap_or_default();
        let nivel = nivel_de_titulo(estilo);
        // Con numeracion es una lista. Se queda en un punto delante: los
        // numeros de Word viven en `numbering.xml` y lo que importa aqui es
        // que se lea como lista.
        let en_lista = nivel == 0 && propiedades.is_some_and(|pr| pr.hijo("numPr").is_some());
        let mut trozos = Vec::new();
        self.en_linea(p, Estilo::default(), &mut trozos);
        let alineacion = match propiedades.and_then(|pr| pr.hijo("jc")).map(|e| e.valor()) {
            Some("center") => Alineacion::Centro,
            Some("right" | "end") => Alineacion::Derecha,
            _ => Alineacion::Izquierda,
        };
        if trozos.is_empty() {
            // Un parrafo vacio en Word es una linea en blanco puesta a
            // proposito: un titulo vacio, en cambio, no es nada.
            if nivel == 0 {
                self.doc.bloques.push(Bloque::nuevo(Clase::Parrafo, vec![]));
            }
            return;
        }
        // Las imagenes van en su propio bloque, para que el visor pueda
        // decir «[imagen]» en una linea suya y el HTML la saque entera.
        let hay_imagen = trozos.iter().any(|t: &Trozo| t.texto == MARCA_IMAGEN);
        if hay_imagen {
            for t in trozos {
                if t.texto == MARCA_IMAGEN {
                    self.doc.bloques.push(Bloque::nota(MARCA_IMAGEN));
                } else if !t.texto.trim().is_empty() {
                    self.doc
                        .bloques
                        .push(Bloque::nuevo(Clase::Parrafo, vec![t]));
                }
            }
            return;
        }
        let clase = match (nivel, en_lista) {
            (0, true) => Clase::Lista,
            (0, false) => Clase::Parrafo,
            (n, _) => Clase::Titulo(n),
        };
        self.doc.bloques.push(Bloque {
            clase,
            alineacion,
            trozos,
            fila: None,
        });
    }

    /// Lo que va dentro de un parrafo: tramos, enlaces y lo que los envuelve.
    fn en_linea(&mut self, padre: &Nodo, heredado: Estilo, salida: &mut Vec<Trozo>) {
        for h in padre.elementos() {
            match h.nombre.as_str() {
                "r" => self.tramo(h, heredado, salida),
                // Del enlace queda el texto marcado: la direccion esta en
                // las relaciones externas y la pagina se ensena sin salir a
                // la red, igual que en el movil.
                "hyperlink" => {
                    let mut e = heredado;
                    e.enlace = true;
                    self.en_linea(h, e, salida);
                }
                "ins" | "smartTag" | "sdt" | "sdtContent" | "fldSimple" | "customXml" => {
                    self.en_linea(h, heredado, salida)
                }
                _ => {}
            }
        }
    }

    fn tramo(&mut self, r: &Nodo, heredado: Estilo, salida: &mut Vec<Trozo>) {
        let mut estilo = heredado;
        if let Some(pr) = r.hijo("rPr") {
            estilo.negrita |= encendido(pr.hijo("b"));
            estilo.cursiva |= encendido(pr.hijo("i"));
            if let Some(u) = pr.hijo("u") {
                estilo.subrayado |= u.valor() != "none" && encendido(Some(u));
            }
            estilo.tachado |= encendido(pr.hijo("strike")) || encendido(pr.hijo("dstrike"));
        }
        let mut texto = String::new();
        let mut imagenes: Vec<String> = Vec::new();
        for h in r.elementos() {
            match h.nombre.as_str() {
                "t" => texto.push_str(&h.texto()),
                "br" | "cr" => texto.push('\n'),
                "tab" => texto.push('\t'),
                "noBreakHyphen" => texto.push('\u{2011}'),
                "drawing" | "pict" | "object" => buscar_imagenes(h, &mut imagenes),
                // Word envuelve en esto lo moderno con su alternativa
                // antigua: basta con una de las dos.
                "AlternateContent" => {
                    if let Some(e) = h.hijo("Choice").or_else(|| h.hijo("Fallback")) {
                        buscar_imagenes(e, &mut imagenes);
                    }
                }
                _ => {}
            }
        }
        if !texto.is_empty() {
            salida.push(Trozo { texto, estilo });
        }
        for id in imagenes {
            if self.guardar_imagen(&id) {
                salida.push(Trozo::llano(MARCA_IMAGEN));
            }
        }
    }

    /// Guarda la imagen de esa relacion; dice si de verdad se guardo.
    fn guardar_imagen(&mut self, id: &str) -> bool {
        let Some(ruta) = self.relaciones.get(id).cloned() else {
            return false;
        };
        // EMF y WMF son dibujos de Windows y no hay quien los pinte en una
        // pagina; en el visor tampoco se verian.
        let Some(mime) = crate::mime_de_imagen(&crate::extension(&ruta)) else {
            return false;
        };
        let Some(datos) = self.paquete.bytes(&ruta, TOPE_DE_IMAGEN) else {
            return false;
        };
        self.doc.imagenes.push(Imagen {
            mime: mime.to_string(),
            datos,
        });
        true
    }

    fn tabla(&mut self, t: &Nodo) {
        // La rejilla de Word: el reparto de columnas que el usuario ve en
        // Word. El lector la aplica en proporcion a la pagina.
        let rejilla: Vec<u32> = t
            .hijo("tblGrid")
            .map(|g| {
                g.elementos()
                    .filter(|c| c.nombre == "gridCol")
                    .map(|c| c.atributo("w").parse::<u32>().unwrap_or(0))
                    .collect()
            })
            .unwrap_or_default();
        let numero = self.tablas;
        self.tablas += 1;
        for fila in t.elementos() {
            if fila.nombre != "tr" {
                continue;
            }
            let mut trozos: Vec<Trozo> = Vec::new();
            let mut celdas: Vec<Celda> = Vec::new();
            for hijo in fila.elementos() {
                // Una fila tambien puede traer sus celdas dentro de un
                // control de contenido.
                let de_la_fila: Vec<&Nodo> = match hijo.nombre.as_str() {
                    "tc" => vec![hijo],
                    "sdt" => hijo
                        .hijo("sdtContent")
                        .map(|c| c.elementos().filter(|e| e.nombre == "tc").collect())
                        .unwrap_or_default(),
                    _ => continue,
                };
                for c in de_la_fila {
                    if !trozos.is_empty() {
                        trozos.push(Trozo::llano(SEPARADOR_DE_CELDA));
                    }
                    let pr = c.hijo("tcPr");
                    let columnas = pr
                        .and_then(|pr| pr.hijo("gridSpan"))
                        .and_then(|g| g.valor().parse::<u16>().ok())
                        .unwrap_or(1)
                        .clamp(1, 64);
                    // La continuacion de una celda unida hacia abajo no
                    // trae nada suyo (el movil la deja vacia).
                    let sigue = pr
                        .and_then(|pr| pr.hijo("vMerge"))
                        .is_some_and(|v| v.valor() != "restart");
                    let relleno = pr.and_then(|pr| pr.hijo("shd")).is_some_and(|s| {
                        let fondo = s.atributo("fill").to_ascii_lowercase();
                        !matches!(fondo.as_str(), "" | "auto" | "ffffff")
                    });
                    let mut propia: Vec<Trozo> = Vec::new();
                    if !sigue {
                        self.parrafos_de_celda(c, &mut propia, &mut true);
                        // En la fila de una linea (buscar, exportar) los
                        // parrafos de la celda con algo van separados por un
                        // espacio; los vacios no dejan hueco.
                        let (mut hay, mut hueco) = (false, false);
                        for t in &propia {
                            if t.texto == "\n" {
                                hueco |= hay;
                                continue;
                            }
                            if hueco {
                                trozos.push(Trozo::llano(" "));
                                hueco = false;
                            }
                            trozos.push(t.clone());
                            hay = true;
                        }
                    }
                    celdas.push(Celda {
                        trozos: propia,
                        columnas,
                        sigue,
                        relleno,
                    });
                }
            }
            if !trozos.is_empty() || !celdas.is_empty() {
                let mut b = Bloque::nuevo(Clase::Fila, trozos);
                b.fila = Some(FilaDeTabla {
                    tabla: numero,
                    rejilla: rejilla.clone(),
                    pagina: self.pagina,
                    celdas,
                });
                self.doc.bloques.push(b);
            }
        }
    }

    /// El texto de una celda para pintarla en su sitio: cada parrafo en su
    /// linea (`td p` del movil), y una tabla metida dentro, aplanada.
    fn parrafos_de_celda(&mut self, c: &Nodo, salida: &mut Vec<Trozo>, primero: &mut bool) {
        for h in c.elementos() {
            match h.nombre.as_str() {
                "p" => {
                    // Cada parrafo tras el primero empieza con un salto,
                    // tambien el vacio: el movil pone cada uno en su `<p>`
                    // (el vacio, `<p>&nbsp;</p>`, con su renglon) y la fila
                    // mide lo mismo aqui que alli (K16).
                    if *primero {
                        *primero = false;
                    } else {
                        salida.push(Trozo::llano("\n"));
                    }
                    self.en_linea(h, Estilo::default(), salida);
                }
                "tbl" | "tr" | "tc" | "sdt" | "sdtContent" => {
                    self.parrafos_de_celda(h, salida, primero)
                }
                _ => {}
            }
        }
    }
}

/// **El ancho de texto de la pagina**: el papel menos los dos margenes, de
/// la seccion del final del cuerpo (la de todo el documento si solo hay una).
/// Es contra lo que Word dibuja la rejilla de una tabla; 0 si no lo dice.
fn ancho_de_texto(cuerpo: &Nodo) -> u32 {
    let Some(s) = cuerpo.elementos().filter(|e| e.nombre == "sectPr").last() else {
        return 0;
    };
    let numero = |e: Option<&Nodo>, a: &str| {
        e.and_then(|e| e.atributo(a).parse::<i64>().ok())
            .unwrap_or(0)
    };
    let papel = numero(s.hijo("pgSz"), "w");
    let margenes = s.hijo("pgMar");
    let texto = papel - numero(margenes, "left") - numero(margenes, "right");
    if papel > 0 && texto > 0 {
        texto as u32
    } else {
        0
    }
}

/// «Heading1», «Ttulo1» (asi escribe Word «Titulo 1» en espanol), «Title»…
/// → 1 a 6; 0 si no es titulo.
fn nivel_de_titulo(estilo: &str) -> u8 {
    let e = estilo.to_lowercase();
    if matches!(e.as_str(), "title" | "ttulo" | "titulo" | "puesto") {
        return 1;
    }
    if matches!(e.as_str(), "subtitle" | "subttulo" | "subtitulo") {
        return 2;
    }
    for raiz in ["heading", "ttulo", "titulo", "encabezado"] {
        if let Some(resto) = e.strip_prefix(raiz)
            && let Ok(n) = resto.trim().parse::<u8>()
        {
            return n.clamp(1, 6);
        }
    }
    0
}

/// `w:b` a secas es «si»; con `w:val="0"` o `"false"` es «no» (asi se apaga
/// lo que el estilo enciende).
fn encendido(e: Option<&Nodo>) -> bool {
    match e {
        None => false,
        Some(e) => !matches!(e.valor().to_lowercase().as_str(), "0" | "false" | "off"),
    }
}

/// Los ids de todas las imagenes que cuelguen de aqui: `a:blip r:embed` en
/// lo moderno, `v:imagedata r:id` en lo antiguo.
fn buscar_imagenes(e: &Nodo, salida: &mut Vec<String>) {
    let id = match e.nombre.as_str() {
        "blip" => e.atributo_con_prefijo("r", "embed"),
        "imagedata" => e.atributo_con_prefijo("r", "id"),
        _ => "",
    };
    if !id.is_empty() {
        salida.push(id.to_string());
    }
    for h in e.elementos() {
        buscar_imagenes(h, salida);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::documento::texto_y_tramos;
    use crate::pruebas::{docx_de, zip_de};

    fn leer_cuerpo(cuerpo: &str) -> Documento {
        let bytes = docx_de(cuerpo, &[]);
        let mut p = Paquete::de_bytes(&bytes).expect("el .docx de la prueba tiene que ser un ZIP");
        de_paquete(&mut p, "prueba").expect("tiene que leerse")
    }

    #[test]
    fn un_parrafo_con_negrita_y_cursiva_conserva_donde_van() {
        let d = leer_cuerpo(
            r#"<w:p><w:r><w:t>Hola </w:t></w:r>
               <w:r><w:rPr><w:b/></w:rPr><w:t>mundo</w:t></w:r>
               <w:r><w:t> y </w:t></w:r>
               <w:r><w:rPr><w:i/></w:rPr><w:t>adios</w:t></w:r></w:p>"#,
        );
        assert_eq!(d.bloques.len(), 1);
        let (texto, tramos) = texto_y_tramos(&d.bloques[0]);
        assert_eq!(texto, "Hola mundo y adios");
        assert_eq!(tramos.len(), 2);
        assert_eq!((tramos[0].inicio, tramos[0].longitud), (5, 5));
        assert!(tramos[0].estilo.negrita && !tramos[0].estilo.cursiva);
        assert_eq!((tramos[1].inicio, tramos[1].longitud), (13, 5));
        assert!(tramos[1].estilo.cursiva && !tramos[1].estilo.negrita);
    }

    #[test]
    fn la_negrita_apagada_con_val_cero_no_se_enciende() {
        // Es como Word apaga lo que el estilo del parrafo ya encendia. Si
        // se leyera «hay etiqueta, luego negrita», todo saldria gordo.
        let d = leer_cuerpo(
            r#"<w:p><w:r><w:rPr><w:b w:val="0"/><w:i w:val="false"/></w:rPr><w:t>fino</w:t></w:r></w:p>"#,
        );
        let (_, tramos) = texto_y_tramos(&d.bloques[0]);
        assert!(tramos.is_empty(), "no tenia que haber estilo: {tramos:?}");
    }

    #[test]
    fn el_subrayado_none_no_subraya() {
        let d = leer_cuerpo(
            r#"<w:p><w:r><w:rPr><w:u w:val="none"/></w:rPr><w:t>liso</w:t></w:r></w:p>"#,
        );
        let (_, tramos) = texto_y_tramos(&d.bloques[0]);
        assert!(tramos.is_empty(), "«none» es que no hay raya: {tramos:?}");
    }

    #[test]
    fn los_titulos_en_ingles_y_en_espanol_dan_el_mismo_nivel() {
        assert_eq!(nivel_de_titulo("Heading2"), 2);
        assert_eq!(
            nivel_de_titulo("Ttulo2"),
            2,
            "asi lo escribe Word en espanol"
        );
        assert_eq!(nivel_de_titulo("Titulo3"), 3);
        assert_eq!(nivel_de_titulo("Title"), 1);
        assert_eq!(nivel_de_titulo("Subtitle"), 2);
        assert_eq!(nivel_de_titulo("Heading9"), 6, "no hay <h9>: se corta en 6");
        assert_eq!(nivel_de_titulo("Normal"), 0);
        assert_eq!(nivel_de_titulo(""), 0);
        assert_eq!(
            nivel_de_titulo("HeadingChar"),
            0,
            "el estilo del tramo no es un titulo"
        );
    }

    #[test]
    fn un_parrafo_numerado_sale_como_lista() {
        let d = leer_cuerpo(
            r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/></w:numPr></w:pPr><w:r><w:t>uno</w:t></w:r></w:p>"#,
        );
        assert_eq!(d.bloques[0].clase, Clase::Lista);
    }

    #[test]
    fn una_tabla_da_una_fila_por_tr_con_sus_celdas_separadas() {
        let d = leer_cuerpo(
            r#"<w:tbl>
                 <w:tr><w:tc><w:p><w:r><w:t>a</w:t></w:r></w:p></w:tc>
                       <w:tc><w:p><w:r><w:t>b</w:t></w:r></w:p></w:tc></w:tr>
                 <w:tr><w:tc><w:p><w:r><w:t>c</w:t></w:r></w:p></w:tc>
                       <w:tc><w:p><w:r><w:t>d</w:t></w:r></w:p></w:tc></w:tr>
               </w:tbl>"#,
        );
        assert_eq!(d.bloques.len(), 2);
        assert!(d.bloques.iter().all(|b| b.clase == Clase::Fila));
        assert_eq!(d.bloques[0].texto(), format!("a{SEPARADOR_DE_CELDA}b"));
        assert_eq!(d.bloques[1].texto(), format!("c{SEPARADOR_DE_CELDA}d"));
    }

    /// Una tabla como las del Word del usuario: rejilla, cabecera con fondo,
    /// una celda que ocupa dos columnas y otra unida hacia abajo.
    const TABLA_COMPLETA: &str = r#"<w:tbl><w:tblGrid><w:gridCol w:w="500"/><w:gridCol w:w="6000"/><w:gridCol w:w="2000"/></w:tblGrid>
        <w:tr><w:tc><w:tcPr><w:shd w:fill="D9D9D6"/></w:tcPr><w:p><w:r><w:t>N</w:t></w:r></w:p></w:tc>
              <w:tc><w:tcPr><w:gridSpan w:val="2"/><w:shd w:fill="auto"/></w:tcPr><w:p><w:r><w:t>Referencia</w:t></w:r></w:p><w:p><w:r><w:t>y enlace</w:t></w:r></w:p><w:p/></w:tc></w:tr>
        <w:tr><w:tc><w:tcPr><w:vMerge w:val="restart"/></w:tcPr><w:p><w:r><w:t>1</w:t></w:r></w:p></w:tc>
              <w:tc><w:p><w:r><w:t>Love</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Cerrado</w:t></w:r></w:p></w:tc></w:tr>
        <w:tr><w:tc><w:tcPr><w:vMerge/></w:tcPr><w:p><w:r><w:t>oculto</w:t></w:r></w:p></w:tc>
              <w:tc><w:p><w:r><w:t>Li</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Abierto</w:t></w:r></w:p></w:tc></w:tr>
      </w:tbl>
      <w:sectPr><w:pgSz w:w="11906"/><w:pgMar w:left="720" w:right="720"/></w:sectPr>"#;

    #[test]
    fn una_tabla_de_word_guarda_sus_celdas_con_rejilla_uniones_y_fondo() {
        let d = leer_cuerpo(TABLA_COMPLETA);
        assert_eq!(d.bloques.len(), 3);
        let filas: Vec<&FilaDeTabla> = d
            .bloques
            .iter()
            .map(|b| b.fila.as_ref().expect("fila de Word"))
            .collect();
        assert!(
            filas
                .iter()
                .all(|f| f.tabla == 0 && f.rejilla == vec![500, 6000, 2000])
        );
        assert_eq!(filas[0].pagina, 11906 - 1440, "papel menos margenes");
        let cab = &filas[0].celdas;
        assert_eq!(cab.len(), 2);
        assert!(cab[0].relleno, "D9D9D6 es fondo");
        assert!(!cab[1].relleno, "auto no es fondo");
        assert_eq!(cab[1].columnas, 2);
        // Cada parrafo en su linea, tambien el vacio del final: el movil lo
        // pinta como `<p>&nbsp;</p>` y la fila mide un renglon mas (K16).
        assert_eq!(
            cab[1].texto(),
            "Referencia
y enlace
"
        );
        assert!(!filas[1].celdas[0].sigue);
        assert!(filas[2].celdas[0].sigue, "la continuacion de la union");
        assert_eq!(filas[2].celdas[0].texto(), "", "no trae nada suyo");
        // La fila en una linea (buscar, exportar) sigue como antes.
        assert_eq!(
            d.bloques[0].texto(),
            format!("N{SEPARADOR_DE_CELDA}Referencia y enlace")
        );
        assert_eq!(
            d.bloques[2].texto(),
            format!("Li{SEPARADOR_DE_CELDA}Abierto"),
            "sin nada delante, sin raya delante"
        );
    }

    #[test]
    fn dos_tablas_seguidas_llevan_numeros_distintos_y_sin_seccion_la_pagina_es_cero() {
        let t = r#"<w:tbl><w:tr><w:tc><w:p><w:r><w:t>a</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#;
        let d = leer_cuerpo(&format!("{t}{t}"));
        let n: Vec<u32> = d
            .bloques
            .iter()
            .map(|b| b.fila.as_ref().unwrap().tabla)
            .collect();
        assert_eq!(n, vec![0, 1]);
        assert_eq!(d.bloques[0].fila.as_ref().unwrap().pagina, 0);
        assert!(d.bloques[0].fila.as_ref().unwrap().rejilla.is_empty());
    }

    #[test]
    fn una_imagen_dentro_de_una_celda_se_guarda_una_sola_vez() {
        let cuerpo = r#"<w:tbl><w:tr><w:tc><w:p><w:r><w:drawing><a:blip r:embed="rId5"/></w:drawing></w:r></w:p></w:tc></w:tr></w:tbl>"#;
        let bytes = docx_de(cuerpo, &[("word/media/foto.png", b"foo")]);
        let mut p = Paquete::de_bytes(&bytes).unwrap();
        let d = de_paquete(&mut p, "t").unwrap();
        assert_eq!(d.imagenes.len(), 1);
    }

    #[test]
    fn el_contenido_dentro_de_un_control_no_se_pierde() {
        // Un `sdt` es lo que Word pone alrededor de un campo o una lista
        // desplegable. Si no se bajara dentro, ese texto no aparecia.
        let d = leer_cuerpo(
            r#"<w:sdt><w:sdtContent><w:p><w:r><w:t>dentro</w:t></w:r></w:p></w:sdtContent></w:sdt>"#,
        );
        assert_eq!(d.bloques[0].texto(), "dentro");
    }

    #[test]
    fn una_imagen_del_documento_se_guarda_con_su_tipo() {
        let cuerpo = r#"<w:p><w:r><w:drawing><a:blip r:embed="rId5"/></w:drawing></w:r></w:p>"#;
        let bytes = docx_de(cuerpo, &[("word/media/foto.png", b"foo")]);
        let mut p = Paquete::de_bytes(&bytes).unwrap();
        let d = de_paquete(&mut p, "t").unwrap();
        assert_eq!(d.imagenes.len(), 1);
        assert_eq!(d.imagenes[0].mime, "image/png");
        assert_eq!(d.imagenes[0].datos, b"foo");
        assert_eq!(d.bloques[0].texto(), MARCA_IMAGEN);
    }

    #[test]
    fn una_imagen_emf_no_se_guarda_ni_deja_hueco() {
        // Nadie sabe pintar un EMF, ni el visor ni un navegador: es mejor
        // no decir nada que dejar un recuadro roto.
        let cuerpo = r#"<w:p><w:r><w:drawing><a:blip r:embed="rId5"/></w:drawing></w:r></w:p>"#;
        let bytes = docx_de(cuerpo, &[("word/media/dibujo.emf", b"MZ")]);
        let mut p = Paquete::de_bytes(&bytes).unwrap();
        let d = de_paquete(&mut p, "t").unwrap();
        assert!(d.imagenes.is_empty());
        // Queda el parrafo, que en Word era una linea suya, pero sin nada
        // escrito: ni un recuadro roto ni un «[imagen]» que engane.
        assert_eq!(d.letras(), 0, "{:?}", d.bloques);
        assert!(!crate::documento::a_html(&d).contains("<img"));
    }

    #[test]
    fn un_enlace_deja_el_texto_marcado_pero_no_la_direccion() {
        let d = leer_cuerpo(
            r#"<w:p><w:hyperlink r:id="rId9"><w:r><w:t>pincha</w:t></w:r></w:hyperlink></w:p>"#,
        );
        let (texto, tramos) = texto_y_tramos(&d.bloques[0]);
        assert_eq!(texto, "pincha");
        assert!(tramos[0].estilo.enlace);
        assert!(
            !crate::documento::a_html(&d).contains("href"),
            "la pagina no sale a la red"
        );
    }

    #[test]
    fn un_zip_que_no_es_un_docx_se_rechaza_diciendo_por_que() {
        let bytes = zip_de(&[("hola.txt", b"nada")]);
        let mut p = Paquete::de_bytes(&bytes).unwrap();
        let e = de_paquete(&mut p, "t").unwrap_err();
        assert!(
            matches!(&e, ErrorDocs::NoSeLee(m) if m.contains("Word")),
            "salio {e:?}"
        );
    }

    #[test]
    fn un_parrafo_vacio_es_una_linea_en_blanco_y_un_titulo_vacio_no_es_nada() {
        let d = leer_cuerpo(
            r#"<w:p/><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr></w:p><w:p><w:r><w:t>fin</w:t></w:r></w:p>"#,
        );
        assert_eq!(d.bloques.len(), 2, "{:?}", d.bloques);
        assert!(d.bloques[0].vacio());
        assert_eq!(d.bloques[1].texto(), "fin");
    }

    #[test]
    fn la_alineacion_del_parrafo_se_conserva() {
        let d = leer_cuerpo(
            r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:t>medio</w:t></w:r></w:p>
               <w:p><w:pPr><w:jc w:val="end"/></w:pPr><w:r><w:t>fin</w:t></w:r></w:p>"#,
        );
        assert_eq!(d.bloques[0].alineacion, Alineacion::Centro);
        assert_eq!(d.bloques[1].alineacion, Alineacion::Derecha);
    }

    #[test]
    fn es_docx_solo_dice_que_si_a_los_que_se_leen() {
        assert!(es_docx("apuntes.docx"));
        assert!(es_docx("APUNTES.DOCX"));
        assert!(es_docx("plantilla.dotx"));
        assert!(!es_docx("apuntes.doc"));
        assert!(!es_docx("apuntes.pdf"));
        assert!(!es_docx("docx"), "sin punto no hay extension");
        assert!(!es_docx(""));
    }
}
