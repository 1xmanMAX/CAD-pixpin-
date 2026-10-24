//! **Un lector de XML rapido** que da el mismo arbol que
//! `pixpin_docs::xml` ([`Nodo`]) y con las mismas reglas de seguridad: el
//! `DOCTYPE` se salta entero sin mirarlo y no existen las entidades
//! declaradas por el documento (solo las cinco de siempre y las numericas).
//!
//! La diferencia es la velocidad: aquel convierte el texto entero a `char`
//! y copia cada nombre letra a letra; este recorre los bytes y corta
//! trozos del `&str`. Un `document.xml` de 12 MB pasa de ~700 ms a ~100.
//! Y ademas sabe leer **un elemento suelto** desde una posicion
//! ([`elemento_en`]), que es lo que deja leer el cuerpo de un Word bloque a
//! bloque sin tener el documento entero en arbol.
//!
//! Los textos que son solo espacio entre etiquetas se tiran, salvo dentro
//! de los elementos de texto (`t`, `instrText`, `delText`), donde cuentan.

use pixpin_docs::xml::{Hijo, Nodo, descodificar};

/// Hasta donde se anidan los elementos (un documento real no pasa de unas
/// decenas; una bomba de anidamiento haria desbordar la pila al soltar el
/// arbol o al recorrerlo).
pub const MAX_PROFUNDIDAD: usize = 1024;
/// Cuantos elementos como mucho en un solo arbol (un `document.xml` de 12 MB
/// tiene del orden de medio millon).
pub const MAX_ELEMENTOS: usize = 8_000_000;

/// Por que se rechaza un XML.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorXml {
    Vacio,
    DemasiadaProfundidad,
    DemasiadosElementos,
}

impl std::fmt::Display for ErrorXml {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorXml::Vacio => write!(f, "no hay ningun elemento XML"),
            ErrorXml::DemasiadaProfundidad => write!(
                f,
                "el XML se anida demasiado (mas de {MAX_PROFUNDIDAD} niveles): archivo danado o malicioso"
            ),
            ErrorXml::DemasiadosElementos => write!(
                f,
                "el XML tiene demasiados elementos (mas de {MAX_ELEMENTOS}): archivo danado o malicioso"
            ),
        }
    }
}

impl std::error::Error for ErrorXml {}

/// Lee un documento entero (`None` si no hay nada o se pasa de los limites).
pub fn leer(texto: &str) -> Option<Nodo> {
    leer_con_limites(texto).ok()
}

/// Como [`leer`], diciendo por que no.
pub fn leer_con_limites(texto: &str) -> Result<Nodo, ErrorXml> {
    leer_i(texto).ok_or(ErrorXml::Vacio).and_then(|r| r)
}

fn leer_i(texto: &str) -> Option<Result<Nodo, ErrorXml>> {
    let mut i = 0;
    let b = texto.as_bytes();
    if texto.starts_with('\u{FEFF}') {
        i = 3;
    }
    // Saltar el prologo hasta el primer elemento.
    loop {
        i = saltar_no_elementos(b, i)?;
        if b.get(i) == Some(&b'<') && b.get(i + 1).is_some_and(|c| *c != b'/') {
            break;
        }
        i += 1;
    }
    let mut fallo = None;
    match elemento_en_lim(texto, i, &mut fallo) {
        Some((n, _)) => Some(Ok(n)),
        None => fallo.map(Err),
    }
}

/// Salta texto, comentarios, instrucciones y el DOCTYPE desde `i`.
/// Devuelve donde empieza la siguiente etiqueta (`<` de un elemento o de un
/// cierre), o `None` al final.
fn saltar_no_elementos(b: &[u8], mut i: usize) -> Option<usize> {
    while i < b.len() {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        match b.get(i + 1) {
            Some(b'?') => i = buscar(b, i + 2, b"?>").map(|k| k + 2).unwrap_or(b.len()),
            Some(b'!') => {
                if b[i..].starts_with(b"<!--") {
                    i = buscar(b, i + 4, b"-->").map(|k| k + 3).unwrap_or(b.len());
                } else if b[i..].starts_with(b"<![CDATA[") {
                    return Some(i);
                } else {
                    i = saltar_doctype(b, i);
                }
            }
            _ => return Some(i),
        }
    }
    None
}

fn buscar(b: &[u8], desde: usize, que: &[u8]) -> Option<usize> {
    if desde >= b.len() {
        return None;
    }
    memchr::memmem::find(&b[desde..], que).map(|k| k + desde)
}

fn saltar_doctype(b: &[u8], i: usize) -> usize {
    let mut j = i + 2;
    let mut dentro = false;
    while j < b.len() {
        match b[j] {
            b'[' => dentro = true,
            b']' => dentro = false,
            b'>' if !dentro => return j + 1,
            _ => {}
        }
        j += 1;
    }
    b.len()
}

fn es_espacio(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\r' | b'\n')
}

fn partir_nombre(n: &str) -> (String, String) {
    match n.rsplit_once(':') {
        Some((p, l)) => (l.to_string(), p.to_string()),
        None => (n.to_string(), String::new()),
    }
}

fn local(n: &str) -> &str {
    n.rsplit(':').next().unwrap_or(n)
}

/// Donde importan los espacios sueltos.
fn guarda_espacios(nombre: &str) -> bool {
    matches!(nombre, "t" | "instrText" | "delText" | "delInstrText")
}

/// Lee la etiqueta de apertura en `i` (que apunta a `<`). Devuelve el nodo
/// sin hijos, si se cierra sola y donde sigue.
fn apertura(texto: &str, i: usize) -> Option<(Nodo, bool, usize)> {
    let b = texto.as_bytes();
    let mut j = i + 1;
    let inicio = j;
    while j < b.len() && !es_espacio(b[j]) && b[j] != b'>' && b[j] != b'/' {
        j += 1;
    }
    if j == inicio {
        return None;
    }
    let (nombre, prefijo) = partir_nombre(texto.get(inicio..j)?);
    let mut nodo = Nodo {
        nombre,
        prefijo,
        atributos: Vec::new(),
        hijos: Vec::new(),
    };
    let mut suelta = false;
    loop {
        while j < b.len() && es_espacio(b[j]) {
            j += 1;
        }
        if j >= b.len() {
            return Some((nodo, suelta, j));
        }
        match b[j] {
            b'>' => return Some((nodo, suelta, j + 1)),
            b'/' => {
                suelta = true;
                j += 1;
                continue;
            }
            _ => {}
        }
        let ini = j;
        while j < b.len() && !es_espacio(b[j]) && b[j] != b'=' && b[j] != b'>' && b[j] != b'/' {
            j += 1;
        }
        let clave = texto.get(ini..j).unwrap_or("").to_string();
        if clave.is_empty() {
            j += 1;
            continue;
        }
        while j < b.len() && es_espacio(b[j]) {
            j += 1;
        }
        let mut valor = String::new();
        if b.get(j) == Some(&b'=') {
            j += 1;
            while j < b.len() && es_espacio(b[j]) {
                j += 1;
            }
            match b.get(j) {
                Some(&c @ (b'"' | b'\'')) => {
                    j += 1;
                    let ini = j;
                    while j < b.len() && b[j] != c {
                        j += 1;
                    }
                    valor = texto_de(texto.get(ini..j).unwrap_or(""));
                    j = (j + 1).min(b.len());
                }
                _ => {
                    let ini = j;
                    while j < b.len() && !es_espacio(b[j]) && b[j] != b'>' && b[j] != b'/' {
                        j += 1;
                    }
                    valor = texto_de(texto.get(ini..j).unwrap_or(""));
                }
            }
        }
        nodo.atributos.push((clave, valor));
    }
}

fn texto_de(s: &str) -> String {
    if s.contains('&') {
        descodificar(s)
    } else {
        s.to_string()
    }
}

/// Lee el elemento completo que empieza en `i` (un `<` de apertura) y
/// devuelve el nodo y la posicion justo detras de su cierre. Tolerante: lo
/// que queda abierto al final se cierra solo, y un cierre que no
/// corresponde cierra hasta donde puede.
pub fn elemento_en(texto: &str, i: usize) -> Option<(Nodo, usize)> {
    elemento_en_lim(texto, i, &mut None)
}

/// Como [`elemento_en`], pero si se pasa de los limites (profundidad o
/// numero de elementos) deja el motivo en `fallo` y devuelve `None`.
pub fn elemento_en_lim(
    texto: &str,
    i: usize,
    fallo: &mut Option<ErrorXml>,
) -> Option<(Nodo, usize)> {
    elemento_en_max(texto, i, fallo, MAX_PROFUNDIDAD, MAX_ELEMENTOS)
}

fn elemento_en_max(
    texto: &str,
    i: usize,
    fallo: &mut Option<ErrorXml>,
    max_prof: usize,
    max_elem: usize,
) -> Option<(Nodo, usize)> {
    let b = texto.as_bytes();
    let (raiz, suelta, mut j) = apertura(texto, i)?;
    if suelta {
        return Some((raiz, j));
    }
    let mut pila: Vec<Nodo> = vec![raiz];
    let mut elementos = 1usize;
    while j < b.len() {
        if b[j] != b'<' {
            let ini = j;
            j = memchr::memchr(b'<', &b[j..])
                .map(|k| k + j)
                .unwrap_or(b.len());
            let crudo = texto.get(ini..j).unwrap_or("");
            if let Some(padre) = pila.last_mut()
                && (guarda_espacios(&padre.nombre) || !crudo.bytes().all(es_espacio))
            {
                padre.hijos.push(Hijo::Texto(texto_de(crudo)));
            }
            continue;
        }
        match b.get(j + 1) {
            Some(b'!') => {
                if b[j..].starts_with(b"<!--") {
                    j = buscar(b, j + 4, b"-->").map(|k| k + 3).unwrap_or(b.len());
                } else if b[j..].starts_with(b"<![CDATA[") {
                    let fin = buscar(b, j + 9, b"]]>").unwrap_or(b.len());
                    if let Some(padre) = pila.last_mut() {
                        padre
                            .hijos
                            .push(Hijo::Texto(texto.get(j + 9..fin).unwrap_or("").to_string()));
                    }
                    j = (fin + 3).min(b.len());
                } else {
                    j = saltar_doctype(b, j);
                }
            }
            Some(b'?') => j = buscar(b, j + 2, b"?>").map(|k| k + 2).unwrap_or(b.len()),
            Some(b'/') => {
                let fin = memchr::memchr(b'>', b.get(j + 2..).unwrap_or(&[]))
                    .map(|k| k + j + 2)
                    .unwrap_or(b.len());
                let nombre = local(texto.get(j + 2..fin).unwrap_or("").trim());
                j = (fin + 1).min(b.len());
                if let Some(pos) = pila.iter().rposition(|n| n.nombre == nombre) {
                    while pila.len() > pos + 1 {
                        let n = pila.pop().expect("hay mas de pos");
                        pila.last_mut()
                            .expect("queda el padre")
                            .hijos
                            .push(Hijo::Elemento(n));
                    }
                    let n = pila.pop().expect("el que se cierra");
                    match pila.last_mut() {
                        Some(p) => p.hijos.push(Hijo::Elemento(n)),
                        None => return Some((n, j)),
                    }
                }
            }
            _ => match apertura(texto, j) {
                Some((nodo, suelta, sig)) => {
                    j = sig;
                    elementos += 1;
                    if elementos > max_elem {
                        *fallo = Some(ErrorXml::DemasiadosElementos);
                        return None;
                    }
                    if !suelta && pila.len() >= max_prof {
                        *fallo = Some(ErrorXml::DemasiadaProfundidad);
                        return None;
                    }
                    if suelta {
                        if let Some(p) = pila.last_mut() {
                            p.hijos.push(Hijo::Elemento(nodo));
                        }
                    } else {
                        pila.push(nodo);
                    }
                }
                None => {
                    if let Some(p) = pila.last_mut() {
                        p.hijos.push(Hijo::Texto("<".into()));
                    }
                    j += 1;
                }
            },
        }
    }
    // Lo que quedo abierto se cierra solo.
    while pila.len() > 1 {
        let n = pila.pop().expect("hay mas de uno");
        pila.last_mut()
            .expect("queda el padre")
            .hijos
            .push(Hijo::Elemento(n));
    }
    pila.pop().map(|n| (n, b.len()))
}

/// Lee los hijos de un elemento uno a uno: el cursor apunta dentro del
/// elemento padre (justo detras de su apertura).
pub struct Hijos<'a> {
    texto: &'a str,
    pub pos: usize,
    pub acabado: bool,
    /// Si se dejo de leer por pasarse de los limites, por que.
    pub fallo: Option<ErrorXml>,
}

impl<'a> Hijos<'a> {
    pub fn nuevo(texto: &'a str, pos: usize) -> Hijos<'a> {
        Hijos {
            texto,
            pos,
            acabado: false,
            fallo: None,
        }
    }

    /// El siguiente hijo elemento y su posicion de inicio, o `None` al
    /// cerrar el padre o acabar el texto.
    pub fn siguiente(&mut self) -> Option<(Nodo, usize)> {
        let b = self.texto.as_bytes();
        loop {
            if self.acabado {
                return None;
            }
            let Some(i) = saltar_no_elementos(b, self.pos) else {
                self.acabado = true;
                return None;
            };
            if b.get(i + 1) == Some(&b'/') {
                self.acabado = true;
                self.pos = i;
                return None;
            }
            if b[i..].starts_with(b"<![CDATA[") {
                self.pos = buscar(b, i + 9, b"]]>").map(|k| k + 3).unwrap_or(b.len());
                continue;
            }
            let mut fallo = None;
            match elemento_en_lim(self.texto, i, &mut fallo) {
                Some((n, fin)) => {
                    self.pos = fin;
                    return Some((n, i));
                }
                None if fallo.is_some() => {
                    // Se paso de los limites: no se sigue leyendo.
                    self.fallo = fallo;
                    self.acabado = true;
                    return None;
                }
                None => self.pos = i + 1,
            }
        }
    }
}

/// Donde empieza el contenido del primer elemento con nombre local
/// `nombre` (detras de su `>`), buscando desde `desde`.
pub fn dentro_de(texto: &str, nombre: &str, desde: usize) -> Option<usize> {
    let b = texto.as_bytes();
    let mut i = desde;
    while let Some(k) = buscar(b, i, nombre.as_bytes()) {
        let antes = if k > 0 { b[k - 1] } else { 0 };
        let despues = b.get(k + nombre.len()).copied().unwrap_or(0);
        if (antes == b'<' || antes == b':') && (es_espacio(despues) || despues == b'>') {
            // Comprobar que es una apertura: hacia atras hasta el `<`.
            let ini = texto[..k].rfind('<')?;
            if b.get(ini + 1) != Some(&b'/') {
                let (_, suelta, sig) = apertura(texto, ini)?;
                if !suelta {
                    return Some(sig);
                }
            }
        }
        i = k + nombre.len();
    }
    None
}

/// Las posiciones (del `<`) de todas las aperturas con ese nombre local.
pub fn aperturas(texto: &str, nombre: &str) -> Vec<usize> {
    let b = texto.as_bytes();
    let mut v = Vec::new();
    for k in memchr::memmem::find_iter(b, nombre.as_bytes()) {
        let antes = if k > 0 { b[k - 1] } else { 0 };
        let despues = b.get(k + nombre.len()).copied().unwrap_or(0);
        if !(antes == b'<' || antes == b':')
            || !(es_espacio(despues) || despues == b'>' || despues == b'/')
        {
            continue;
        }
        let Some(ini) = texto[..k].rfind('<') else {
            continue;
        };
        // Entre el `<` y el nombre solo puede haber un prefijo.
        if texto[ini + 1..k]
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b':' || c == b'_' || c == b'-')
            && b.get(ini + 1) != Some(&b'/')
        {
            v.push(ini);
        }
    }
    v
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn da_el_mismo_arbol_que_el_lector_comun() {
        let x = r#"<?xml version="1.0"?><w:document xmlns:w="x"><w:body><w:p><w:r><w:t xml:space="preserve"> hola &amp; adios </w:t></w:r></w:p>
            <!-- comentario --><w:p/></w:body></w:document>"#;
        let a = leer(x).unwrap();
        assert_eq!(a.nombre, "document");
        let body = a.hijo("body").unwrap();
        assert_eq!(
            body.elementos().count(),
            2,
            "los espacios sueltos no son hijos"
        );
        assert_eq!(body.hijo("p").unwrap().texto(), " hola & adios ");
    }

    #[test]
    fn el_doctype_y_sus_entidades_se_saltan() {
        let n =
            leer(r#"<!DOCTYPE a [<!ENTITY x SYSTEM "file:///c:/secreto">]><a>&x;</a>"#).unwrap();
        assert_eq!(n.texto(), "&x;");
    }

    #[test]
    fn los_hijos_se_leen_uno_a_uno() {
        let x = r#"<d><body> <p>1</p><tbl><tr/></tbl> <sectPr/></body></d>"#;
        let ini = dentro_de(x, "body", 0).unwrap();
        let mut h = Hijos::nuevo(x, ini);
        let nombres: Vec<String> =
            std::iter::from_fn(|| h.siguiente().map(|(n, _)| n.nombre)).collect();
        assert_eq!(nombres, vec!["p", "tbl", "sectPr"]);
        assert_eq!(aperturas(x, "sectPr").len(), 1);
        assert!(aperturas("<a><sectPrChange/></a>", "sectPr").is_empty());
    }

    #[test]
    fn un_xml_roto_no_entra_en_panico() {
        let x = "<a><b c=\"1\"><d>texto</b><e/><f";
        for k in 0..=x.len() {
            let _ = leer(&x[..k]);
        }
        assert!(leer("").is_none());
        assert!(leer("solo texto").is_none());
    }
}

#[cfg(test)]
mod pruebas_limites {
    use super::*;

    #[test]
    fn cien_mil_niveles_de_anidamiento_se_rechazan_sin_panico() {
        let x = "<a>".repeat(100_000);
        assert_eq!(
            leer_con_limites(&x).unwrap_err(),
            ErrorXml::DemasiadaProfundidad
        );
        assert!(leer(&x).is_none());
        assert!(
            leer_con_limites(&x)
                .unwrap_err()
                .to_string()
                .contains("se anida demasiado")
        );
        // Leyendo hijo a hijo, se detiene y dice por que.
        let dentro = format!("<d>{x}</d>");
        let mut h = Hijos::nuevo(&dentro, 3);
        assert!(h.siguiente().is_none());
        assert_eq!(h.fallo, Some(ErrorXml::DemasiadaProfundidad));
    }

    #[test]
    fn un_anidamiento_normal_pasa() {
        let x = "<a>".repeat(200);
        assert!(leer_con_limites(&x).is_ok());
        assert!(leer_con_limites("<a><b/><c>t</c></a>").is_ok());
    }

    #[test]
    fn demasiados_elementos_se_rechazan() {
        let y = format!("<r>{}</r>", "<a/>".repeat(60));
        let mut fallo = None;
        assert!(elemento_en_max(&y, 0, &mut fallo, 100, 50).is_none());
        assert_eq!(fallo, Some(ErrorXml::DemasiadosElementos));
        let mut fallo = None;
        assert!(elemento_en_max(&y, 0, &mut fallo, 100, 100).is_some());
        assert_eq!(fallo, None);
    }
}
