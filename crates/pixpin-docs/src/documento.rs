//! Lo que un Word, un libro o una pagina tienen en comun una vez leidos:
//! **bloques de texto con estilo**, que es lo unico que el visor pinta.
//!
//! El movil escribe HTML porque quien lo ensena es un `WebView`. Aqui el
//! visor pinta con DirectWrite, asi que el resultado natural no es una
//! cadena de HTML sino esta lista de bloques; el HTML se saca de ella
//! (`a_html`) cuando hace falta guardarlo o abrirlo en el navegador, y sale
//! con el mismo estilo que el del movil (`DocxAHtml.kt`, constante `ESTILO`)
//! para que las dos aplicaciones ensenen lo mismo.
//!
//! Aqui no se dibuja nada y no se sabe que existe Windows: todo esto se
//! prueba sin abrir una ventana.

/// Que es un bloque. Se parece a `pixpin_pin::markdown::Tipo` a proposito:
/// el visor traduce uno en otro sin pensar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clase {
    Parrafo,
    /// Nivel 1..=6.
    Titulo(u8),
    /// Un punto de lista. Los numeros de Word viven en `numbering.xml` y no
    /// se siguen: lo que importa es que se lea como lista (igual que el
    /// movil, `DocxAHtml.kt`, comentario de `parrafo`).
    Lista,
    Cita,
    Codigo,
    /// Una raya de separacion.
    Regla,
    /// Una fila de tabla, con las celdas ya unidas por ` | `.
    Fila,
    /// Algo que el traductor tiene que decir: «[imagen]». Se pinta en gris
    /// y en cursiva.
    Nota,
    /// El principio de un capitulo de un libro: raya arriba y aire.
    Capitulo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Alineacion {
    #[default]
    Izquierda,
    Centro,
    Derecha,
}

/// Como va escrito un trozo de texto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Estilo {
    pub negrita: bool,
    pub cursiva: bool,
    pub subrayado: bool,
    pub tachado: bool,
    pub mono: bool,
    /// Parte de un enlace. La direccion no se guarda: la pagina se ensena
    /// sin salir a la red, igual que en el movil.
    pub enlace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trozo {
    pub texto: String,
    pub estilo: Estilo,
}

impl Trozo {
    pub fn llano(texto: impl Into<String>) -> Self {
        Self {
            texto: texto.into(),
            estilo: Estilo::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bloque {
    pub clase: Clase,
    pub alineacion: Alineacion,
    pub trozos: Vec<Trozo>,
    /// En una fila de tabla de un Word, sus celdas contadas (columnas que
    /// ocupa, uniones, rejilla). `trozos` sigue siendo la fila en una linea
    /// con [`SEPARADOR_DE_CELDA`]: es lo que buscan, exportan y guardan los
    /// demas. Un libro o una pagina no la traen y se parte por el separador
    /// (`tabla::celdas_de`).
    pub fila: Option<crate::tabla::FilaDeTabla>,
}

impl Bloque {
    pub fn nuevo(clase: Clase, trozos: Vec<Trozo>) -> Self {
        Self {
            clase,
            alineacion: Alineacion::default(),
            trozos,
            fila: None,
        }
    }

    pub fn nota(texto: &str) -> Self {
        Self::nuevo(Clase::Nota, vec![Trozo::llano(texto)])
    }

    /// El texto del bloque, sin estilos.
    pub fn texto(&self) -> String {
        self.trozos.iter().map(|t| t.texto.as_str()).collect()
    }

    pub fn vacio(&self) -> bool {
        self.trozos.iter().all(|t| t.texto.trim().is_empty())
    }
}

/// Una imagen del documento, guardada aparte para que los bloques sigan
/// siendo texto y se puedan comparar en una prueba.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Imagen {
    pub mime: String,
    pub datos: Vec<u8>,
}

/// Un documento leido: su nombre y sus bloques.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Documento {
    pub titulo: String,
    /// El autor, si el documento lo dice (los libros lo dicen).
    pub autor: String,
    pub bloques: Vec<Bloque>,
    /// Las imagenes, en el orden en que aparecen. Un bloque `Nota` con el
    /// texto de `MARCA_IMAGEN` se corresponde con la siguiente de la lista.
    pub imagenes: Vec<Imagen>,
}

/// Lo que se escribe en el sitio de una imagen cuando el visor solo pinta
/// texto. Sale tal cual en la pantalla, y en el HTML se cambia por la
/// imagen de verdad.
pub const MARCA_IMAGEN: &str = "[imagen]";

/// Cuantas imagenes lleva una fila de tabla en sus celdas. Tambien estan en
/// `Documento::imagenes`, en su orden: quien recorre los bloques contando
/// imagenes tiene que sumarlas o las de despues de la tabla le salen
/// corridas una.
pub fn imagenes_de_fila(b: &Bloque) -> usize {
    match &b.fila {
        Some(f) => f
            .celdas
            .iter()
            .flat_map(|c| &c.trozos)
            .filter(|t| t.texto == MARCA_IMAGEN)
            .count(),
        None => 0,
    }
}

impl Documento {
    /// Cuanto texto tiene, en letras. Sirve para saber si el documento
    /// estaba vacio sin recorrerlo dos veces desde fuera.
    pub fn letras(&self) -> usize {
        self.bloques.iter().map(|b| b.texto().chars().count()).sum()
    }
}

/// Un tramo de estilo sobre el texto de un bloque, en unidades UTF-16 (que
/// es lo que cuenta DirectWrite, y lo que espera `pixpin_render::Tramo`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TramoDoc {
    pub inicio: u32,
    pub longitud: u32,
    pub estilo: Estilo,
}

/// El bloque partido en «un texto y sus tramos», que es como se pinta.
///
/// Los trozos seguidos con el mismo estilo se juntan en un tramo, y los que
/// no llevan nada marcado no gastan tramo: pintar es una llamada por tramo.
pub fn texto_y_tramos(b: &Bloque) -> (String, Vec<TramoDoc>) {
    let mut texto = String::new();
    let mut tramos: Vec<TramoDoc> = Vec::new();
    let mut pos: u32 = 0;
    for t in &b.trozos {
        let largo: u32 = t.texto.encode_utf16().count() as u32;
        texto.push_str(&t.texto);
        if largo == 0 {
            continue;
        }
        if t.estilo != Estilo::default() {
            match tramos.last_mut() {
                Some(u) if u.estilo == t.estilo && u.inicio + u.longitud == pos => {
                    u.longitud += largo;
                }
                _ => tramos.push(TramoDoc {
                    inicio: pos,
                    longitud: largo,
                    estilo: t.estilo,
                }),
            }
        }
        pos += largo;
    }
    (texto, tramos)
}

/// El documento como una pagina HTML de un solo fichero.
///
/// Es la salida que el movil produce directamente, y aqui sirve para
/// guardarla o abrirla en el navegador, donde si se ven las imagenes y las
/// tablas de verdad. El estilo es el del movil, letra por letra.
pub fn a_html(d: &Documento) -> String {
    let mut s = String::with_capacity(4096 + d.letras() * 2);
    s.push_str("<!DOCTYPE html>\n<html lang=\"es\"><head><meta charset=\"utf-8\">");
    s.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">");
    s.push_str("<title>");
    escapar_en(&d.titulo, &mut s);
    s.push_str("</title><style>");
    s.push_str(ESTILO);
    s.push_str("</style></head><body><main>\n");
    if !d.autor.trim().is_empty() {
        s.push_str("<header class=\"libro\"><h1>");
        escapar_en(&d.titulo, &mut s);
        s.push_str("</h1><p>");
        escapar_en(&d.autor, &mut s);
        s.push_str("</p></header>\n");
    }

    let mut imagen = 0usize;
    let mut en_tabla = false;
    for b in &d.bloques {
        // Las filas seguidas son una tabla; en cuanto viene otra cosa, se
        // cierra. Asi el modelo se queda plano y el HTML sale bien.
        if b.clase == Clase::Fila && !en_tabla {
            s.push_str("<div class=\"tabla\"><table>\n");
            en_tabla = true;
        } else if b.clase != Clase::Fila && en_tabla {
            s.push_str("</table></div>\n");
            en_tabla = false;
        }
        let alineado = match b.alineacion {
            Alineacion::Izquierda => "",
            Alineacion::Centro => " style=\"text-align:center\"",
            Alineacion::Derecha => " style=\"text-align:right\"",
        };
        match b.clase {
            Clase::Regla => s.push_str("<hr>\n"),
            Clase::Capitulo => s.push_str("<hr class=\"capitulo\">\n"),
            Clase::Fila => {
                s.push_str("<tr>");
                for celda in b.texto().split(SEPARADOR_DE_CELDA) {
                    s.push_str("<td>");
                    escapar_en(celda, &mut s);
                    s.push_str("</td>");
                }
                s.push_str("</tr>\n");
                // Las de sus celdas tambien estan en la lista.
                imagen += imagenes_de_fila(b);
            }
            Clase::Nota if b.texto() == MARCA_IMAGEN => {
                match d.imagenes.get(imagen) {
                    Some(im) => {
                        s.push_str("<p><img alt=\"\" src=\"data:");
                        s.push_str(&im.mime);
                        s.push_str(";base64,");
                        s.push_str(&base64(&im.datos));
                        s.push_str("\"></p>\n");
                    }
                    // Nunca deberia pasar: el bloque y la imagen se anaden
                    // a la vez. Si pasa, mejor decirlo que callar.
                    None => s.push_str("<p class=\"nota\">[imagen]</p>\n"),
                }
                imagen += 1;
            }
            Clase::Nota => {
                s.push_str("<p class=\"nota\">");
                tramos_html(b, &mut s);
                s.push_str("</p>\n");
            }
            Clase::Titulo(n) => {
                let n = n.clamp(1, 6);
                s.push_str(&format!("<h{n}{alineado}>"));
                tramos_html(b, &mut s);
                s.push_str(&format!("</h{n}>\n"));
            }
            Clase::Lista => {
                s.push_str("<p class=\"lista\">• ");
                tramos_html(b, &mut s);
                s.push_str("</p>\n");
            }
            Clase::Cita => {
                s.push_str("<blockquote>");
                tramos_html(b, &mut s);
                s.push_str("</blockquote>\n");
            }
            Clase::Codigo => {
                s.push_str("<pre>");
                escapar_en(&b.texto(), &mut s);
                s.push_str("</pre>\n");
            }
            Clase::Parrafo => {
                if b.vacio() {
                    s.push_str("<p>&nbsp;</p>\n");
                } else {
                    s.push_str(&format!("<p{alineado}>"));
                    tramos_html(b, &mut s);
                    s.push_str("</p>\n");
                }
            }
        }
    }
    if en_tabla {
        s.push_str("</table></div>\n");
    }
    if d.letras() == 0 {
        s.push_str("<p class=\"nota\">El documento no tiene texto.</p>\n");
    }
    s.push_str("</main></body></html>\n");
    s
}

/// Lo que separa una celda de otra dentro de un bloque `Fila`. Se elige un
/// caracter con aire a los lados para que en la pantalla se lea como tabla
/// sin tener que dibujar rejillas.
pub const SEPARADOR_DE_CELDA: &str = "  │  ";

fn tramos_html(b: &Bloque, s: &mut String) {
    for t in &b.trozos {
        let e = t.estilo;
        let mut cierres: Vec<&str> = Vec::new();
        let abrir = |etiqueta: &'static str, s: &mut String, cierres: &mut Vec<&str>| {
            s.push('<');
            s.push_str(etiqueta);
            s.push('>');
            cierres.insert(0, etiqueta);
        };
        if e.negrita {
            abrir("b", s, &mut cierres);
        }
        if e.cursiva {
            abrir("i", s, &mut cierres);
        }
        if e.subrayado {
            abrir("u", s, &mut cierres);
        }
        if e.tachado {
            abrir("s", s, &mut cierres);
        }
        if e.mono {
            abrir("code", s, &mut cierres);
        }
        if e.enlace {
            s.push_str("<span class=\"enlace\">");
        }
        escapar_en(&t.texto, s);
        if e.enlace {
            s.push_str("</span>");
        }
        for c in cierres {
            s.push_str("</");
            s.push_str(c);
            s.push('>');
        }
    }
}

pub fn escapar(s: &str) -> String {
    let mut salida = String::with_capacity(s.len() + 16);
    escapar_en(s, &mut salida);
    salida
}

fn escapar_en(s: &str, salida: &mut String) {
    for c in s.chars() {
        match c {
            '&' => salida.push_str("&amp;"),
            '<' => salida.push_str("&lt;"),
            '>' => salida.push_str("&gt;"),
            '"' => salida.push_str("&quot;"),
            '\'' => salida.push_str("&#39;"),
            _ => salida.push(c),
        }
    }
}

/// Base64 corriente, el de las direcciones `data:`. Son doce lineas y no
/// merece una dependencia.
pub fn base64(datos: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(datos.len().div_ceil(3) * 4);
    for t in datos.chunks(3) {
        let a = t[0] as u32;
        let b = *t.get(1).unwrap_or(&0) as u32;
        let c = *t.get(2).unwrap_or(&0) as u32;
        let n = (a << 16) | (b << 8) | c;
        s.push(ABC[(n >> 18) as usize & 63] as char);
        s.push(ABC[(n >> 12) as usize & 63] as char);
        s.push(if t.len() > 1 {
            ABC[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        s.push(if t.len() > 2 {
            ABC[n as usize & 63] as char
        } else {
            '='
        });
    }
    s
}

/// El mismo de `DocxAHtml.kt`: columna estrecha, letra del sistema y los
/// colores del modo noche si toca, mas las reglas de impresion del libro
/// (`EpubAHtml.kt`) para que «guardar como PDF» del navegador de algo
/// decente.
pub(crate) const ESTILO: &str = concat!(
    "html{-webkit-text-size-adjust:100%}",
    "body{margin:0;background:#fff;color:#1b1b1f;font:16px/1.55 system-ui,-apple-system,Segoe UI,sans-serif}",
    "main{max-width:46em;margin:0 auto;padding:18px 16px 48px;overflow-wrap:break-word}",
    "header.libro{text-align:center;margin:2em 0 1em}",
    "header.libro p{margin:0;color:#6b6b73;font-style:italic}",
    "h1,h2,h3,h4,h5,h6{line-height:1.25;margin:1.2em 0 .5em}",
    "h1{font-size:1.7em}h2{font-size:1.4em}h3{font-size:1.2em}h4,h5,h6{font-size:1.05em}",
    "p{margin:.55em 0}p.lista{margin:.25em 0 .25em 1.2em;text-indent:-1em}",
    "blockquote{margin:1em 1.4em;color:#55555d}pre{white-space:pre-wrap}",
    "img{max-width:100%;height:auto}",
    ".tabla{overflow-x:auto;margin:.8em 0}",
    "table{border-collapse:collapse}",
    "td{border:1px solid #b9b9c2;padding:4px 8px;vertical-align:top}td p{margin:.2em 0}",
    ".enlace{color:#2a5bd7;text-decoration:underline}",
    ".nota{color:#77777f;font-style:italic}",
    "hr{border:0;border-top:1px solid #d5d2ca;margin:1.5em 0}",
    "hr.capitulo{margin:2.5em 0 2em}",
    "@media (prefers-color-scheme: dark){body{background:#121316;color:#e4e2e6}",
    "blockquote{color:#a8a8b0}header.libro p{color:#9a9aa2}",
    "td{border-color:#4a4a52}.enlace{color:#9db7ff}.nota{color:#9a9aa2}hr{border-color:#3a3a42}}",
    "@media print{@page{size:A4;margin:20mm}",
    "body{background:#fff;color:#000;font-size:11pt}main{max-width:none;padding:0}",
    "hr.capitulo{break-before:page;page-break-before:always;border:0;margin:0}",
    "h1,h2,h3,h4,h5,h6{break-after:avoid;page-break-after:avoid}",
    "img,table{break-inside:avoid;page-break-inside:avoid}}",
);

/// El atributo con el que se marca cada bloque en [`cuerpo_para_anotar`].
pub const MARCA_DE_BLOQUE: &str = "data-b";

/// **El cuerpo del documento para la pagina web anotada**: los mismos
/// bloques que pinta el lector, uno por elemento y **cada uno marcado con
/// `data-b`**, en el mismo orden que el lector los coloca: primero la
/// cabecera (el titulo si lo hay, el autor si lo hay) y luego un elemento
/// por bloque, tambien los vacios. Asi la altura de cada bloque medida en el
/// PC y la medida por el navegador casan una a una, y lo anotado se corre
/// con su parrafo (ver `pixpin_motor2d::exportar_html::documento`).
///
/// Es HTML de verdad —texto que se selecciona, se busca y se copia—, pero
/// escrito **como lo pinta el lector** y no como [`a_html`]: una fila de
/// tabla es una linea con sus celdas separadas, como en la pantalla, y no
/// una tabla con otras medidas; asi lo rayado encima de una celda sigue
/// encima de ella. `imagen` da la direccion `data:` de cada imagen (quien
/// llama la encoge si pesa); `None` la deja anunciada.
///
/// Lo que el lector pinta en gris (una nota, el autor) va con la clase
/// `aparte` y no `nota`: el guion de la pagina web toma el primer `.nota`
/// de la hoja por el documento entero, y mediria un parrafo suelto.
pub fn cuerpo_para_anotar(d: &Documento, imagen: &dyn Fn(&Imagen) -> Option<String>) -> String {
    let mut s = String::with_capacity(4096 + d.letras() * 2);
    if !d.titulo.trim().is_empty() {
        s.push_str("<h1 data-b>");
        escapar_en(d.titulo.trim(), &mut s);
        s.push_str("</h1>\n");
    }
    if !d.autor.trim().is_empty() {
        s.push_str("<p class=\"aparte autor\" data-b>");
        escapar_en(d.autor.trim(), &mut s);
        s.push_str("</p>\n");
    }
    let mut n_imagen = 0usize;
    for b in &d.bloques {
        match b.clase {
            Clase::Regla => s.push_str("<hr data-b>\n"),
            Clase::Capitulo => s.push_str("<hr class=\"capitulo\" data-b>\n"),
            Clase::Nota if b.texto() == MARCA_IMAGEN => {
                let src = d.imagenes.get(n_imagen).and_then(imagen);
                n_imagen += 1;
                match src {
                    Some(src) => {
                        s.push_str("<p class=\"imagen\" data-b><img alt=\"\" src=\"");
                        s.push_str(&src);
                        s.push_str("\"></p>\n");
                    }
                    None => s.push_str("<p class=\"aparte\" data-b>[imagen]</p>\n"),
                }
            }
            // Una lista vacia sigue siendo su punto, como en el lector.
            Clase::Lista => {
                s.push_str("<p class=\"lista\" data-b>• ");
                tramos_html(b, &mut s);
                s.push_str("</p>\n");
            }
            // Lo vacio no pinta nada pero ocupa su hueco: el lector lo salta
            // dejando aire, y aqui ese aire lo pone su clase.
            _ if b.vacio() => s.push_str("<p class=\"vacio\" data-b></p>\n"),
            Clase::Nota => {
                s.push_str("<p class=\"aparte\" data-b>");
                tramos_html(b, &mut s);
                s.push_str("</p>\n");
            }
            Clase::Titulo(n) => {
                let n = n.clamp(1, 6);
                s.push_str(&format!("<h{n} data-b>"));
                tramos_html(b, &mut s);
                s.push_str(&format!("</h{n}>\n"));
            }
            Clase::Cita => {
                s.push_str("<blockquote data-b>");
                tramos_html(b, &mut s);
                s.push_str("</blockquote>\n");
            }
            Clase::Codigo => {
                s.push_str("<pre data-b>");
                escapar_en(&b.texto(), &mut s);
                s.push_str("</pre>\n");
            }
            Clase::Fila => {
                // Las imagenes de sus celdas tambien estan en la lista.
                n_imagen += imagenes_de_fila(b);
                s.push_str("<p class=\"fila\" data-b>");
                tramos_html(b, &mut s);
                s.push_str("</p>\n");
            }
            Clase::Parrafo => {
                s.push_str("<p data-b>");
                tramos_html(b, &mut s);
                s.push_str("</p>\n");
            }
        }
    }
    s
}

/// Cuantos elementos marca [`cuerpo_para_anotar`]: la cabecera y un bloque
/// cada uno. Quien mide tiene que dar exactamente estas alturas.
pub fn bloques_para_anotar(d: &Documento) -> usize {
    usize::from(!d.titulo.trim().is_empty())
        + usize::from(!d.autor.trim().is_empty())
        + d.bloques.len()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn con(texto: &str, estilo: Estilo) -> Trozo {
        Trozo {
            texto: texto.into(),
            estilo,
        }
    }

    const NEGRITA: Estilo = Estilo {
        negrita: true,
        cursiva: false,
        subrayado: false,
        tachado: false,
        mono: false,
        enlace: false,
    };

    #[test]
    fn los_tramos_se_cuentan_en_utf16_y_no_en_bytes() {
        // «ñ» son dos bytes en UTF-8 y una unidad en UTF-16; el emoji son
        // cuatro bytes y DOS unidades. DirectWrite cuenta en UTF-16: si
        // aqui se contaran bytes, la negrita caeria en otro sitio.
        let b = Bloque::nuevo(
            Clase::Parrafo,
            vec![Trozo::llano("añ🙂"), con("gordo", NEGRITA)],
        );
        let (texto, tramos) = texto_y_tramos(&b);
        assert_eq!(texto, "añ🙂gordo");
        assert_eq!(tramos.len(), 1);
        assert_eq!(tramos[0].inicio, 4, "a + ñ + dos unidades del emoji");
        assert_eq!(tramos[0].longitud, 5);
    }

    #[test]
    fn dos_trozos_seguidos_con_el_mismo_estilo_son_un_solo_tramo() {
        let b = Bloque::nuevo(
            Clase::Parrafo,
            vec![con("uno", NEGRITA), con("dos", NEGRITA)],
        );
        let (_, tramos) = texto_y_tramos(&b);
        assert_eq!(tramos.len(), 1);
        assert_eq!(tramos[0].longitud, 6);
    }

    #[test]
    fn el_texto_llano_no_gasta_tramos() {
        let b = Bloque::nuevo(Clase::Parrafo, vec![Trozo::llano("todo normal")]);
        let (_, tramos) = texto_y_tramos(&b);
        assert!(tramos.is_empty(), "salieron tramos: {tramos:?}");
    }

    #[test]
    fn un_trozo_vacio_no_parte_el_tramo_de_al_lado() {
        let b = Bloque::nuevo(
            Clase::Parrafo,
            vec![con("uno", NEGRITA), Trozo::llano(""), con("dos", NEGRITA)],
        );
        let (_, tramos) = texto_y_tramos(&b);
        assert_eq!(tramos.len(), 1, "el vacio no tenia que cortar nada");
    }

    #[test]
    fn el_html_escapa_lo_que_podria_ser_una_etiqueta() {
        let d = Documento {
            titulo: "<script>".into(),
            bloques: vec![Bloque::nuevo(
                Clase::Parrafo,
                vec![Trozo::llano("a < b & \"c\"")],
            )],
            ..Default::default()
        };
        let h = a_html(&d);
        assert!(h.contains("&lt;script&gt;"), "{h}");
        assert!(h.contains("a &lt; b &amp; &quot;c&quot;"), "{h}");
        assert!(
            !h.contains("<script>"),
            "no puede quedar ni una etiqueta de guion: {h}"
        );
    }

    #[test]
    fn las_filas_seguidas_salen_como_una_sola_tabla() {
        let fila = |t: &str| Bloque::nuevo(Clase::Fila, vec![Trozo::llano(t)]);
        let d = Documento {
            titulo: "t".into(),
            bloques: vec![
                fila(&format!("a{SEPARADOR_DE_CELDA}b")),
                fila(&format!("c{SEPARADOR_DE_CELDA}d")),
                Bloque::nuevo(Clase::Parrafo, vec![Trozo::llano("fuera")]),
            ],
            ..Default::default()
        };
        let h = a_html(&d);
        assert_eq!(h.matches("<table>").count(), 1, "{h}");
        assert_eq!(h.matches("</table>").count(), 1, "{h}");
        assert!(h.contains("<td>a</td><td>b</td>"), "{h}");
    }

    #[test]
    fn un_documento_sin_texto_lo_dice() {
        let h = a_html(&Documento {
            titulo: "vacio".into(),
            ..Default::default()
        });
        assert!(h.contains("El documento no tiene texto."), "{h}");
    }

    #[test]
    fn base64_da_lo_mismo_que_el_de_siempre() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64(&[0xFF, 0xFE, 0xFD]), "//79");
    }

    #[test]
    fn la_marca_de_imagen_se_cambia_por_la_imagen_de_verdad() {
        let d = Documento {
            titulo: "t".into(),
            bloques: vec![Bloque::nota(MARCA_IMAGEN)],
            imagenes: vec![Imagen {
                mime: "image/png".into(),
                datos: b"foo".to_vec(),
            }],
            ..Default::default()
        };
        let h = a_html(&d);
        assert!(h.contains("src=\"data:image/png;base64,Zm9v\""), "{h}");
    }
}

#[cfg(test)]
mod pruebas_para_anotar {
    use super::*;

    fn doc() -> Documento {
        Documento {
            titulo: "Libro".into(),
            autor: String::new(),
            bloques: vec![
                Bloque::nuevo(Clase::Titulo(2), vec![Trozo::llano("Uno")]),
                Bloque::nuevo(Clase::Parrafo, vec![Trozo::llano("  ")]),
                Bloque::nota(MARCA_IMAGEN),
                Bloque::nuevo(Clase::Fila, vec![Trozo::llano("a  │  b")]),
                Bloque::nuevo(Clase::Regla, vec![]),
                Bloque::nuevo(Clase::Parrafo, vec![Trozo::llano("<fin>")]),
            ],
            imagenes: vec![Imagen {
                mime: "image/png".into(),
                datos: vec![1, 2, 3],
            }],
        }
    }

    #[test]
    fn cada_bloque_sale_marcado_y_en_el_orden_del_lector() {
        let d = doc();
        let html = cuerpo_para_anotar(&d, &|im| {
            Some(format!("data:{};base64,{}", im.mime, base64(&im.datos)))
        });
        assert_eq!(html.matches("data-b").count(), bloques_para_anotar(&d));
        assert_eq!(bloques_para_anotar(&d), 7, "titulo y seis bloques");
        let orden: Vec<usize> = [
            "<h1 data-b>Libro",
            "<h2 data-b>Uno",
            "class=\"vacio\"",
            "<img",
            "class=\"fila\"",
            "<hr data-b>",
            "&lt;fin&gt;",
        ]
        .iter()
        .map(|a| html.find(a).unwrap_or_else(|| panic!("falta {a}")))
        .collect();
        assert!(orden.windows(2).all(|w| w[0] < w[1]), "{orden:?}");
        assert!(html.contains("src=\"data:image/png;base64,AQID\""));
        // Caso negativo: una fila no se vuelve tabla, que mediria distinto.
        assert!(!html.contains("<table"));
        // Ni nada con la clase que el guion toma por el documento entero.
        assert!(!html.contains("class=\"nota"));
    }

    #[test]
    fn una_imagen_sin_direccion_se_queda_anunciada_y_sigue_contando() {
        let d = doc();
        let html = cuerpo_para_anotar(&d, &|_| None);
        assert!(html.contains("<p class=\"aparte\" data-b>[imagen]</p>"));
        assert_eq!(html.matches("data-b").count(), 7);
    }
}

#[cfg(test)]
mod pruebas_imagenes_en_tablas {
    use super::*;
    use crate::tabla::{Celda, FilaDeTabla};

    /// Una fila con una imagen en su celda y, despues, otra imagen suelta.
    fn documento() -> Documento {
        let mut fila = Bloque::nuevo(Clase::Fila, vec![Trozo::llano(MARCA_IMAGEN)]);
        fila.fila = Some(FilaDeTabla {
            tabla: 0,
            rejilla: Vec::new(),
            pagina: 0,
            celdas: vec![Celda::llana(vec![Trozo::llano(MARCA_IMAGEN)])],
        });
        let im = |d: &[u8]| Imagen {
            mime: "image/png".into(),
            datos: d.to_vec(),
        };
        Documento {
            bloques: vec![fila, Bloque::nota(MARCA_IMAGEN)],
            imagenes: vec![im(b"celda"), im(b"suelta")],
            ..Documento::default()
        }
    }

    #[test]
    fn la_imagen_de_despues_de_una_tabla_es_la_suya() {
        let d = documento();
        assert_eq!(imagenes_de_fila(&d.bloques[0]), 1);
        // Caso negativo: lo que no es una fila de Word no cuenta.
        assert_eq!(imagenes_de_fila(&d.bloques[1]), 0);
        let html = a_html(&d);
        assert!(html.contains(&base64(b"suelta")), "{html}");
        assert!(!html.contains(&base64(b"celda")));
        let src = |i: &Imagen| Some(String::from_utf8_lossy(&i.datos).into_owned());
        let anotar = cuerpo_para_anotar(&d, &src);
        assert!(anotar.contains("src=\"suelta\""), "{anotar}");
    }
}
