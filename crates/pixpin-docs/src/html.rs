//! **Una pagina de HTML leida como texto con formato.**
//!
//! Sirve para dos cosas: abrir un `.html` de la conversacion, y coser los
//! capitulos de un `.epub`, que ya *son* HTML (por eso [`crate::epub`] pasa
//! por aqui, igual que `EpubAHtml.kt` en el movil).
//!
//! No es un navegador y no quiere serlo: no hay CSS, ni guion, ni
//! maquetacion. Se recorre la pagina etiqueta a etiqueta y se sacan bloques
//! de texto con su estilo, que es lo que el visor pinta. Lo que trae peligro
//! —`script`, `style`, `iframe`, `object`, `embed`— **no se lee siquiera**:
//! su contenido se salta entero, igual que en el movil.
//!
//! El analizador es tolerante por obligacion: el HTML de un libro real trae
//! etiquetas sin cerrar, `<br>` sueltos y `<p>` anidados. Cuando algo no
//! cuadra se cierra el bloque y se sigue; nunca se tira la pagina.

use crate::documento::{
    Alineacion, Bloque, Clase, Documento, Estilo, Imagen, MARCA_IMAGEN, SEPARADOR_DE_CELDA, Trozo,
};

/// Cuanto texto seguido cabe en un bloque antes de partirlo. Un capitulo
/// sin un solo `<p>` (los hay) daria un bloque de un megabyte, y medirlo
/// para pintarlo cuesta mas que leerlo.
const TOPE_DE_BLOQUE: usize = 20_000;

/// Una pagina entera: `<title>` para el titulo, `<body>` para los bloques.
pub fn pagina(texto: &str, titulo_de_fuera: &str) -> Documento {
    let mut doc = Documento {
        titulo: titulo_de_fuera.to_string(),
        ..Default::default()
    };
    let titulo = entre_etiquetas(texto, "title");
    if let Some(t) = titulo {
        let t = descodificar(&t).trim().to_string();
        if !t.is_empty() {
            doc.titulo = t;
        }
    }
    doc.bloques = analizar(texto, &mut |_| None, &mut doc.imagenes);
    doc
}

/// Texto llano (o Markdown, que se lee tal cual): cada linea, una linea.
///
/// No se interpreta nada. Un `.md` se lee como esta escrito, que para unos
/// apuntes es lo que se quiere; formatearlo seria otro traductor y el movil
/// tampoco lo hace.
pub fn llano(texto: &str, titulo: &str) -> Documento {
    let bloques = texto
        .lines()
        .map(|l| {
            let l = l.trim_end_matches('\r');
            if l.trim().is_empty() {
                Bloque::nuevo(Clase::Parrafo, vec![])
            } else {
                Bloque::nuevo(Clase::Parrafo, vec![Trozo::llano(l)])
            }
        })
        .collect();
    Documento {
        titulo: titulo.to_string(),
        bloques,
        ..Default::default()
    }
}

/// Lo de dentro de la primera etiqueta que se llame asi, sin descodificar.
fn entre_etiquetas(texto: &str, etiqueta: &str) -> Option<String> {
    let bajo = texto.to_lowercase();
    let abre = bajo.find(&format!("<{etiqueta}"))?;
    let fin_abre = bajo[abre..].find('>')? + abre + 1;
    let cierra = bajo[fin_abre..].find(&format!("</{etiqueta}"))? + fin_abre;
    Some(texto[fin_abre..cierra].to_string())
}

/// Recorre el HTML y devuelve sus bloques.
///
/// `imagen_de` traduce el `src` de una imagen en la imagen de verdad; en un
/// `.html` suelto devuelve siempre nada (las imagenes viven fuera del
/// fichero y aqui no se sale al disco ni a la red), y en un `.epub` la saca
/// del propio libro.
pub fn analizar(
    texto: &str,
    imagen_de: &mut dyn FnMut(&str) -> Option<Imagen>,
    imagenes: &mut Vec<Imagen>,
) -> Vec<Bloque> {
    let mut e = Estado {
        bloques: Vec::new(),
        clase: Clase::Parrafo,
        alineacion: Alineacion::Izquierda,
        trozos: Vec::new(),
        estilos: Vec::new(),
        saltando: Vec::new(),
        en_cabecera: false,
        en_fila: false,
        celdas: 0,
    };
    let cs: Vec<char> = texto.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        if cs[i] != '<' {
            let ini = i;
            while i < cs.len() && cs[i] != '<' {
                i += 1;
            }
            let crudo: String = cs[ini..i].iter().collect();
            e.texto(&descodificar(&crudo));
            continue;
        }
        if empieza(&cs, i, "<!--") {
            i = buscar(&cs, i + 4, "-->").map(|k| k + 3).unwrap_or(cs.len());
            continue;
        }
        if cs.get(i + 1) == Some(&'!') || cs.get(i + 1) == Some(&'?') {
            i = buscar(&cs, i + 2, ">").map(|k| k + 1).unwrap_or(cs.len());
            continue;
        }
        let Some((etiqueta, atributos, cierre, suelta, siguiente)) = partir(&cs, i) else {
            e.texto("<");
            i += 1;
            continue;
        };
        i = siguiente;
        if cierre {
            e.cerrar(&etiqueta);
        } else {
            e.abrir(&etiqueta, &atributos, suelta, imagen_de, imagenes);
        }
    }
    e.cerrar_bloque();
    e.bloques
}

struct Estado {
    bloques: Vec<Bloque>,
    clase: Clase,
    alineacion: Alineacion,
    trozos: Vec<Trozo>,
    /// Estilos abiertos, con el nombre de la etiqueta que los abrio para
    /// poder cerrar el que toca.
    estilos: Vec<(String, Estilo)>,
    /// Etiquetas cuyo contenido no se lee (`script`, `style`…).
    saltando: Vec<String>,
    en_cabecera: bool,
    en_fila: bool,
    celdas: usize,
}

/// Lo que ni se lee: su contenido no es texto para nadie.
const MUDAS: [&str; 8] = [
    "script", "style", "iframe", "object", "embed", "noscript", "applet", "template",
];

/// Lo que corta el bloque: cada una empieza uno nuevo.
const DE_BLOQUE: [&str; 22] = [
    "p",
    "div",
    "section",
    "article",
    "header",
    "footer",
    "main",
    "aside",
    "nav",
    "ul",
    "ol",
    "dl",
    "dt",
    "dd",
    "li",
    "blockquote",
    "pre",
    "figure",
    "figcaption",
    "table",
    "tr",
    "form",
];

impl Estado {
    fn texto(&mut self, t: &str) {
        // En `pre` los espacios y los saltos son el contenido; fuera, una
        // retahila de espacios y saltos de linea es un espacio y ya.
        if self.clase == Clase::Codigo {
            self.crudo(t);
        } else {
            self.crudo(&apretar(t));
        }
    }

    /// Texto tal cual, sin apretar: lo que mete un `<br>`, que si es un
    /// salto de verdad.
    fn crudo(&mut self, limpio: &str) {
        if !self.saltando.is_empty() || self.en_cabecera || limpio.is_empty() {
            return;
        }
        let estilo = self.estilos.last().map(|(_, e)| *e).unwrap_or_default();
        // Dos espacios seguidos, uno de cada trozo, no son dos espacios.
        if limpio == " "
            && self
                .trozos
                .last()
                .is_none_or(|u| u.texto.ends_with([' ', '\n']))
        {
            return;
        }
        match self.trozos.last_mut() {
            Some(u) if u.estilo == estilo => u.texto.push_str(limpio),
            _ => self.trozos.push(Trozo {
                texto: limpio.to_string(),
                estilo,
            }),
        }
        if self.largo() > TOPE_DE_BLOQUE {
            self.cerrar_bloque();
        }
    }

    fn largo(&self) -> usize {
        self.trozos.iter().map(|t| t.texto.len()).sum()
    }

    fn cerrar_bloque(&mut self) {
        let trozos = std::mem::take(&mut self.trozos);
        let clase = self.clase;
        let alineacion = self.alineacion;
        self.clase = Clase::Parrafo;
        self.alineacion = Alineacion::Izquierda;
        if trozos.iter().all(|t| t.texto.trim().is_empty()) {
            return;
        }
        self.bloques.push(Bloque {
            clase,
            alineacion,
            trozos,
            fila: None,
        });
    }

    fn abrir(
        &mut self,
        etiqueta: &str,
        atributos: &str,
        suelta: bool,
        imagen_de: &mut dyn FnMut(&str) -> Option<Imagen>,
        imagenes: &mut Vec<Imagen>,
    ) {
        if MUDAS.contains(&etiqueta) {
            if !suelta {
                self.saltando.push(etiqueta.to_string());
            }
            return;
        }
        if !self.saltando.is_empty() {
            return;
        }
        match etiqueta {
            "head" => self.en_cabecera = true,
            "body" => self.en_cabecera = false,
            _ => {}
        }
        if self.en_cabecera {
            return;
        }
        if DE_BLOQUE.contains(&etiqueta) {
            self.cerrar_bloque();
            self.en_fila = false;
            self.celdas = 0;
        }
        match etiqueta {
            "br" => self.crudo("\n"),
            "hr" => {
                self.cerrar_bloque();
                self.bloques
                    .push(Bloque::nuevo(Clase::Regla, vec![Trozo::llano(" ")]));
            }
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                self.cerrar_bloque();
                let n = etiqueta.as_bytes()[1] - b'0';
                self.clase = Clase::Titulo(n);
            }
            // La marca de cada hoja de un PDF leido como texto
            // (`pdf_a_texto`) y los avisos de la pagina: en gris, como nota.
            "p" if atributo(atributos, "class").is_some_and(|c| {
                c.split_whitespace()
                    .any(|k| k == crate::pdf_a_texto::CLASE_DE_HOJA || k == "nota")
            }) =>
            {
                self.clase = Clase::Nota
            }
            "li" => self.clase = Clase::Lista,
            "blockquote" => self.clase = Clase::Cita,
            "pre" => self.clase = Clase::Codigo,
            "tr" => {
                self.clase = Clase::Fila;
                self.en_fila = true;
                self.celdas = 0;
            }
            "td" | "th" => {
                if self.en_fila {
                    if self.celdas > 0 {
                        self.trozos.push(Trozo::llano(SEPARADOR_DE_CELDA));
                    }
                    self.celdas += 1;
                }
            }
            "img" | "image" => {
                let src = atributo(atributos, "src")
                    .or_else(|| atributo(atributos, "href"))
                    .unwrap_or_default();
                if let Some(im) = imagen_de(&src) {
                    self.cerrar_bloque();
                    imagenes.push(im);
                    self.bloques.push(Bloque::nota(MARCA_IMAGEN));
                }
            }
            "b" | "strong" => self.empujar(etiqueta, |e| e.negrita = true),
            "i" | "em" | "cite" | "var" => self.empujar(etiqueta, |e| e.cursiva = true),
            "u" | "ins" => self.empujar(etiqueta, |e| e.subrayado = true),
            "s" | "strike" | "del" => self.empujar(etiqueta, |e| e.tachado = true),
            "code" | "kbd" | "samp" | "tt" => self.empujar(etiqueta, |e| e.mono = true),
            "a" => self.empujar(etiqueta, |e| e.enlace = true),
            _ => {}
        }
        if self.clase == Clase::Parrafo
            && let Some(a) = alineacion_de(atributos)
        {
            self.alineacion = a;
        }
        // `<b/>` es XHTML valido: abre y cierra a la vez.
        if suelta {
            self.cerrar(etiqueta);
        }
    }

    fn empujar(&mut self, etiqueta: &str, cambiar: impl Fn(&mut Estilo)) {
        let mut e = self.estilos.last().map(|(_, e)| *e).unwrap_or_default();
        cambiar(&mut e);
        self.estilos.push((etiqueta.to_string(), e));
    }

    fn cerrar(&mut self, etiqueta: &str) {
        if let Some(pos) = self.saltando.iter().rposition(|m| m == etiqueta) {
            self.saltando.truncate(pos);
            return;
        }
        if !self.saltando.is_empty() {
            return;
        }
        if etiqueta == "head" {
            self.en_cabecera = false;
            return;
        }
        if self.en_cabecera {
            return;
        }
        // El estilo se cierra hasta el que coincide; uno que sobra se tira,
        // que es lo que hace un navegador con `</b>` sin `<b>`.
        if let Some(pos) = self.estilos.iter().rposition(|(m, _)| m == etiqueta) {
            self.estilos.truncate(pos);
            return;
        }
        if DE_BLOQUE.contains(&etiqueta)
            || matches!(etiqueta, "h1" | "h2" | "h3" | "h4" | "h5" | "h6")
        {
            self.cerrar_bloque();
            if etiqueta == "tr" {
                self.en_fila = false;
                self.celdas = 0;
            }
        }
    }
}

/// Espacios, tabuladores y saltos seguidos, reducidos a un espacio: es lo
/// que hace cualquier navegador con el texto de una pagina.
///
/// El espacio duro (`&nbsp;`) **no** entra aqui aunque para Rust sea un
/// blanco: en una pagina esta puesto justo para que no se apriete.
fn apretar(t: &str) -> String {
    let mut salida = String::with_capacity(t.len());
    let mut blanco = false;
    for c in t.chars() {
        if matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{0B}' | '\u{0C}') {
            blanco = true;
            continue;
        }
        if blanco && !salida.is_empty() {
            salida.push(' ');
        }
        // Un espacio duro al principio del trozo si que cuenta.
        if blanco && salida.is_empty() {
            salida.push(' ');
        }
        blanco = false;
        salida.push(c);
    }
    if blanco {
        salida.push(' ');
    }
    salida
}

fn alineacion_de(atributos: &str) -> Option<Alineacion> {
    let a = atributo(atributos, "align")?.to_lowercase();
    Some(match a.as_str() {
        "center" => Alineacion::Centro,
        "right" => Alineacion::Derecha,
        "left" => Alineacion::Izquierda,
        _ => return None,
    })
}

/// Parte `<etiqueta atributos>` desde `i`: nombre en minusculas, el resto
/// tal cual, si era un cierre, si se cerraba sola, y por donde seguir.
fn partir(cs: &[char], i: usize) -> Option<(String, String, bool, bool, usize)> {
    let mut j = i + 1;
    let cierre = cs.get(j) == Some(&'/');
    if cierre {
        j += 1;
    }
    let ini = j;
    while j < cs.len() && (cs[j].is_ascii_alphanumeric() || cs[j] == ':' || cs[j] == '-') {
        j += 1;
    }
    if j == ini {
        return None;
    }
    let nombre: String = cs[ini..j].iter().collect::<String>().to_lowercase();
    let nombre = nombre.rsplit(':').next().unwrap_or(&nombre).to_string();
    // El resto, hasta el `>`, saltando las comillas: un `src` con una
    // imagen en base64 lleva dentro de todo, `>` incluido.
    let ini_atributos = j;
    let mut comilla: Option<char> = None;
    while j < cs.len() {
        match cs[j] {
            c @ ('"' | '\'') if comilla.is_none() => comilla = Some(c),
            c if comilla == Some(c) => comilla = None,
            '>' if comilla.is_none() => break,
            _ => {}
        }
        j += 1;
    }
    let atributos: String = cs[ini_atributos..j.min(cs.len())].iter().collect();
    let suelta = atributos.trim_end().ends_with('/');
    Some((nombre, atributos, cierre, suelta, (j + 1).min(cs.len())))
}

/// Un atributo de una etiqueta, ya descodificado.
fn atributo(atributos: &str, nombre: &str) -> Option<String> {
    let cs: Vec<char> = atributos.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        while i < cs.len() && (cs[i].is_whitespace() || cs[i] == '/') {
            i += 1;
        }
        let ini = i;
        while i < cs.len() && !cs[i].is_whitespace() && cs[i] != '=' && cs[i] != '/' {
            i += 1;
        }
        let clave: String = cs[ini..i].iter().collect::<String>().to_lowercase();
        let clave = clave.rsplit(':').next().unwrap_or(&clave).to_string();
        while i < cs.len() && cs[i].is_whitespace() {
            i += 1;
        }
        let mut valor = String::new();
        if cs.get(i) == Some(&'=') {
            i += 1;
            while i < cs.len() && cs[i].is_whitespace() {
                i += 1;
            }
            match cs.get(i) {
                Some(&c @ ('"' | '\'')) => {
                    i += 1;
                    let ini = i;
                    while i < cs.len() && cs[i] != c {
                        i += 1;
                    }
                    valor = cs[ini..i].iter().collect();
                    i = (i + 1).min(cs.len());
                }
                _ => {
                    let ini = i;
                    while i < cs.len() && !cs[i].is_whitespace() {
                        i += 1;
                    }
                    valor = cs[ini..i].iter().collect();
                }
            }
        }
        if clave == nombre {
            return Some(descodificar(&valor));
        }
        if clave.is_empty() && i < cs.len() {
            i += 1;
        }
    }
    None
}

fn empieza(cs: &[char], i: usize, que: &str) -> bool {
    que.chars()
        .enumerate()
        .all(|(k, c)| cs.get(i + k) == Some(&c))
}

fn buscar(cs: &[char], desde: usize, que: &str) -> Option<usize> {
    let n = que.chars().count();
    (desde..cs.len().saturating_sub(n.saturating_sub(1))).find(|&k| empieza(cs, k, que))
}

/// Las entidades con nombre que de verdad aparecen en libros y paginas, mas
/// las numericas. Una que no este se deja escrita tal cual, que es lo menos
/// mentiroso.
const ENTIDADES: [(&str, char); 42] = [
    ("amp", '&'),
    ("lt", '<'),
    ("gt", '>'),
    ("quot", '"'),
    ("apos", '\''),
    ("nbsp", '\u{A0}'),
    ("ndash", '–'),
    ("mdash", '—'),
    ("hellip", '…'),
    ("lsquo", '‘'),
    ("rsquo", '’'),
    ("ldquo", '“'),
    ("rdquo", '”'),
    ("laquo", '«'),
    ("raquo", '»'),
    ("bull", '•'),
    ("middot", '·'),
    ("copy", '©'),
    ("reg", '®'),
    ("trade", '™'),
    ("deg", '°'),
    ("euro", '€'),
    ("pound", '£'),
    ("sect", '§'),
    ("para", '¶'),
    ("dagger", '†'),
    ("times", '×'),
    ("divide", '÷'),
    ("frac12", '½'),
    ("iquest", '¿'),
    ("iexcl", '¡'),
    ("ntilde", 'ñ'),
    ("Ntilde", 'Ñ'),
    ("aacute", 'á'),
    ("eacute", 'é'),
    ("iacute", 'í'),
    ("oacute", 'ó'),
    ("uacute", 'ú'),
    ("uuml", 'ü'),
    ("ccedil", 'ç'),
    ("shy", '\u{AD}'),
    ("ensp", '\u{2002}'),
];

/// Las entidades de HTML a las letras que representan.
pub fn descodificar(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let cs: Vec<char> = s.chars().collect();
    let mut salida = String::with_capacity(s.len());
    let mut i = 0;
    while i < cs.len() {
        if cs[i] != '&' {
            salida.push(cs[i]);
            i += 1;
            continue;
        }
        let tope = (i + 12).min(cs.len());
        let Some(fin) = (i + 1..tope).find(|&k| cs[k] == ';') else {
            salida.push('&');
            i += 1;
            continue;
        };
        let cuerpo: String = cs[i + 1..fin].iter().collect();
        match traducir(&cuerpo) {
            Some(c) => salida.push(c),
            None => {
                salida.push('&');
                salida.push_str(&cuerpo);
                salida.push(';');
            }
        }
        i = fin + 1;
    }
    salida
}

fn traducir(cuerpo: &str) -> Option<char> {
    if let Some(numero) = cuerpo.strip_prefix('#') {
        let valor = match numero.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => numero.parse::<u32>().ok()?,
        };
        return char::from_u32(valor);
    }
    ENTIDADES
        .iter()
        .find(|(n, _)| *n == cuerpo)
        .map(|(_, c)| *c)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::documento::texto_y_tramos;

    fn bloques(html: &str) -> Vec<Bloque> {
        let mut imagenes = Vec::new();
        analizar(html, &mut |_| None, &mut imagenes)
    }

    #[test]
    fn los_parrafos_se_separan_y_el_texto_se_aprieta() {
        let b = bloques("<p>uno\n   dos</p><p>  tres  </p>");
        assert_eq!(b.len(), 2);
        assert_eq!(b[0].texto(), "uno dos");
        assert_eq!(b[1].texto().trim(), "tres");
    }

    #[test]
    fn la_marca_de_hoja_de_un_pdf_es_una_nota_y_un_parrafo_cualquiera_no() {
        let b = bloques(
            "<p class=\"hoja-pdf\" id=\"hoja-1\" aria-hidden=\"true\">Hoja 1</p>\
             <h1>Título</h1><p class=\"otra\">Texto</p><p class=\"nota\">Sin texto</p>",
        );
        let clases: Vec<Clase> = b.iter().map(|x| x.clase).collect();
        assert_eq!(
            clases,
            [Clase::Nota, Clase::Titulo(1), Clase::Parrafo, Clase::Nota]
        );
    }

    #[test]
    fn el_guion_no_se_lee_ni_por_dentro() {
        // Es la razon de ser de esto: un documento que llega de otro no
        // ejecuta nada, y su codigo tampoco se ensena como si fuera texto.
        let b = bloques("<p>antes</p><script>alert('hola')</script><p>despues</p>");
        let todo: String = b.iter().map(|x| x.texto()).collect();
        assert_eq!(todo, "antesdespues", "salio: {todo}");
        assert!(!todo.contains("alert"));
    }

    #[test]
    fn el_estilo_dentro_de_la_pagina_tampoco_se_lee() {
        let b = bloques("<style>body{color:red}</style><p>texto</p>");
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].texto(), "texto");
    }

    #[test]
    fn la_cabecera_no_es_contenido() {
        let b = bloques("<html><head><title>Titulo</title></head><body><p>hola</p></body></html>");
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].texto(), "hola");
    }

    #[test]
    fn el_titulo_sale_de_la_etiqueta_title_y_si_no_del_nombre() {
        let d = pagina(
            "<html><head><title>Mis apuntes</title></head><body>a</body>",
            "fichero",
        );
        assert_eq!(d.titulo, "Mis apuntes");
        let sin = pagina("<html><body>a</body></html>", "fichero");
        assert_eq!(sin.titulo, "fichero");
        let blanco = pagina("<title>   </title><body>a</body>", "fichero");
        assert_eq!(blanco.titulo, "fichero", "un titulo en blanco no vale");
    }

    #[test]
    fn la_negrita_anidada_cae_donde_toca() {
        let b = bloques("<p>uno <b>dos <i>tres</i></b> cuatro</p>");
        let (texto, tramos) = texto_y_tramos(&b[0]);
        assert_eq!(texto, "uno dos tres cuatro");
        assert_eq!(tramos.len(), 2);
        assert!(tramos[0].estilo.negrita && !tramos[0].estilo.cursiva);
        assert!(tramos[1].estilo.negrita && tramos[1].estilo.cursiva);
    }

    #[test]
    fn un_cierre_que_sobra_no_arrastra_el_estilo_de_al_lado() {
        let b = bloques("<p></b>uno <b>dos</b> tres</p>");
        let (texto, tramos) = texto_y_tramos(&b[0]);
        assert_eq!(texto, "uno dos tres");
        assert_eq!(tramos.len(), 1, "salio {tramos:?}");
        assert_eq!((tramos[0].inicio, tramos[0].longitud), (4, 3));
    }

    #[test]
    fn los_titulos_y_las_listas_se_reconocen() {
        let b = bloques("<h2>Capitulo</h2><ul><li>uno</li><li>dos</li></ul><hr>");
        assert_eq!(b[0].clase, Clase::Titulo(2));
        assert_eq!(b[1].clase, Clase::Lista);
        assert_eq!(b[2].clase, Clase::Lista);
        assert_eq!(b[3].clase, Clase::Regla);
    }

    #[test]
    fn una_fila_de_tabla_junta_sus_celdas() {
        let b = bloques("<table><tr><td>a</td><td>b</td></tr></table>");
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].clase, Clase::Fila);
        assert_eq!(b[0].texto(), format!("a{SEPARADOR_DE_CELDA}b"));
    }

    #[test]
    fn las_entidades_se_traducen_y_las_desconocidas_se_quedan() {
        let b = bloques("<p>5 &lt; 6 &nbsp;&mdash; &#233; &inventada;</p>");
        assert_eq!(b[0].texto(), "5 < 6 \u{A0}— é &inventada;");
    }

    #[test]
    fn el_pre_conserva_los_saltos_y_lo_demas_no() {
        let b = bloques("<pre>uno\n  dos</pre><p>tres\n  cuatro</p>");
        assert_eq!(b[0].clase, Clase::Codigo);
        assert_eq!(b[0].texto(), "uno\n  dos");
        assert_eq!(b[1].texto(), "tres cuatro");
    }

    #[test]
    fn una_etiqueta_sin_cerrar_no_se_come_la_pagina() {
        let b = bloques("<p>uno<p>dos<p>tres");
        assert_eq!(b.len(), 3);
        assert_eq!(b[2].texto(), "tres");
    }

    #[test]
    fn un_menor_que_suelto_es_texto_y_no_una_etiqueta() {
        let b = bloques("<p>si a < b entonces</p>");
        assert!(b[0].texto().contains('<'), "salio: {}", b[0].texto());
    }

    #[test]
    fn una_pagina_sin_nada_no_da_bloques() {
        assert!(bloques("").is_empty());
        assert!(bloques("<html><body>   </body></html>").is_empty());
        assert!(bloques("<script>solo guion</script>").is_empty());
    }

    #[test]
    fn el_br_es_un_salto_dentro_del_mismo_parrafo() {
        let b = bloques("<p>uno<br>dos</p>");
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].texto(), "uno\ndos");
    }

    #[test]
    fn la_imagen_se_pide_al_de_fuera_y_deja_su_marca() {
        let mut imagenes = Vec::new();
        let b = analizar(
            r#"<p>antes</p><img src="foto.png"><p>despues</p>"#,
            &mut |src| {
                assert_eq!(src, "foto.png");
                Some(Imagen {
                    mime: "image/png".into(),
                    datos: vec![1, 2, 3],
                })
            },
            &mut imagenes,
        );
        assert_eq!(imagenes.len(), 1);
        assert!(b.iter().any(|x| x.texto() == MARCA_IMAGEN));
    }

    #[test]
    fn una_imagen_que_no_esta_no_deja_marca() {
        let mut imagenes = Vec::new();
        let b = analizar(r#"<img src="no-existe.png">"#, &mut |_| None, &mut imagenes);
        assert!(imagenes.is_empty());
        assert!(b.is_empty(), "{b:?}");
    }

    #[test]
    fn un_atributo_con_un_mayor_que_dentro_no_corta_la_etiqueta() {
        let mut imagenes = Vec::new();
        let b = analizar(
            r#"<img alt="a > b" src="x.png">texto"#,
            &mut |src| {
                assert_eq!(src, "x.png");
                Some(Imagen {
                    mime: "image/png".into(),
                    datos: vec![],
                })
            },
            &mut imagenes,
        );
        assert_eq!(imagenes.len(), 1);
        assert!(
            b.iter().any(|x| x.texto() == "texto"),
            "el texto de detras tenia que salir: {b:?}"
        );
    }

    #[test]
    fn el_texto_llano_conserva_las_lineas() {
        let d = llano("uno\n\ndos\n", "apuntes");
        assert_eq!(d.titulo, "apuntes");
        assert_eq!(d.bloques.len(), 3);
        assert_eq!(d.bloques[0].texto(), "uno");
        assert!(d.bloques[1].vacio());
        assert_eq!(d.bloques[2].texto(), "dos");
    }
}
