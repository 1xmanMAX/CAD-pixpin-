//! Un lector de XML de tiron (pull) sobre bytes, sin entidades externas.
//!
//! Lo justo para el XML de Office: etiquetas, atributos, texto y CDATA. Las
//! instrucciones de proceso, los comentarios y el DOCTYPE se saltan sin
//! mirar dentro, y de las entidades solo se entienden las cinco del XML y
//! las numericas: un `<!ENTITY>` de un fichero hostil no se expande nunca.
//!
//! Funciona sobre un trozo que puede estar cortado (`final_ = false`): si la
//! siguiente pieza no esta entera devuelve [`Paso::Incompleto`] sin avanzar,
//! y quien llama lee mas y vuelve a pedir. Asi la hoja se lee mientras se
//! descomprime, sin tener nunca el XML entero en memoria.

use std::borrow::Cow;

/// Una pieza del XML. Los nombres van sin prefijo (`x:c` es `c`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evento<'a> {
    /// Etiqueta de apertura; `vacia` si es `<a/>` (no habra cierre).
    Abre {
        nombre: &'a [u8],
        attrs: &'a [u8],
        vacia: bool,
    },
    Cierra {
        nombre: &'a [u8],
    },
    /// Texto tal cual viene, con las entidades sin resolver.
    Texto(&'a [u8]),
    /// Texto de un CDATA: literal, sin entidades.
    Cdata(&'a [u8]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paso<'a> {
    Evento(Evento<'a>),
    /// Falta texto para completar la pieza: leer mas y repetir.
    Incompleto,
    Fin,
}

/// El nombre sin prefijo de espacio de nombres.
#[inline]
pub fn local(nombre: &[u8]) -> &[u8] {
    match nombre.iter().rposition(|&b| b == b':') {
        Some(i) => &nombre[i + 1..],
        None => nombre,
    }
}

const UNOS: u64 = 0x0101_0101_0101_0101;
const ALTOS: u64 = 0x8080_8080_8080_8080;

/// Los bytes de `v` que son cero, marcados con su bit alto (el truco de
/// «hay un byte cero» de ocho en ocho; el primero marcado es exacto).
#[inline(always)]
fn ceros(v: u64) -> u64 {
    v.wrapping_sub(UNOS) & !v & ALTOS
}

#[inline(always)]
fn ocho(d: &[u8], i: usize) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&d[i..i + 8]);
    u64::from_le_bytes(b)
}

/// La primera `b` de `d`, mirando de ocho en ocho bytes. El XML de una
/// hoja grande son decenas de megas: esto es lo que mas se repite.
#[inline]
pub fn pos_byte(d: &[u8], b: u8) -> Option<usize> {
    let patron = UNOS.wrapping_mul(b as u64);
    let mut i = 0;
    while i + 8 <= d.len() {
        let t = ceros(ocho(d, i) ^ patron);
        if t != 0 {
            return Some(i + (t.trailing_zeros() / 8) as usize);
        }
        i += 8;
    }
    d[i..].iter().position(|&x| x == b).map(|k| i + k)
}

/// La primera de tres bytes cualquiera.
#[inline]
fn pos_de3(d: &[u8], a: u8, b: u8, c: u8) -> Option<usize> {
    let (pa, pb, pc) = (
        UNOS.wrapping_mul(a as u64),
        UNOS.wrapping_mul(b as u64),
        UNOS.wrapping_mul(c as u64),
    );
    let mut i = 0;
    while i + 8 <= d.len() {
        let v = ocho(d, i);
        let t = ceros(v ^ pa) | ceros(v ^ pb) | ceros(v ^ pc);
        if t != 0 {
            // Alguno de los tres esta en estos ocho: cual y donde, a mano.
            if let Some(p) = d[i..i + 8].iter().position(|&x| x == a || x == b || x == c) {
                return Some(i + p);
            }
        }
        i += 8;
    }
    d[i..]
        .iter()
        .position(|&x| x == a || x == b || x == c)
        .map(|k| i + k)
}

#[inline]
pub fn buscar(d: &[u8], desde: usize, aguja: &[u8]) -> Option<usize> {
    if aguja.is_empty() || desde >= d.len() {
        return None;
    }
    let primero = aguja[0];
    let mut i = desde;
    while i + aguja.len() <= d.len() {
        let j = i + pos_byte(&d[i..], primero)?;
        if j + aguja.len() > d.len() {
            return None;
        }
        if &d[j..j + aguja.len()] == aguja {
            return Some(j);
        }
        i = j + 1;
    }
    None
}

/// La siguiente pieza a partir de `*pos`. Solo avanza `*pos` si devuelve un
/// evento. Con `final_` el trozo es todo lo que hay: lo cortado se da por
/// terminado (`Fin`) en vez de pedir mas.
pub fn siguiente<'a>(d: &'a [u8], pos: &mut usize, final_: bool) -> Paso<'a> {
    let falta = if final_ { Paso::Fin } else { Paso::Incompleto };
    loop {
        let p = *pos;
        if p >= d.len() {
            return falta;
        }
        if d[p] != b'<' {
            return match pos_byte(&d[p..], b'<') {
                Some(n) => {
                    *pos = p + n;
                    Paso::Evento(Evento::Texto(&d[p..p + n]))
                }
                None if final_ => {
                    *pos = d.len();
                    Paso::Evento(Evento::Texto(&d[p..]))
                }
                None => Paso::Incompleto,
            };
        }
        let resto = &d[p..];
        if resto.len() < 2 {
            return falta;
        }
        match resto[1] {
            b'?' => match buscar(d, p + 2, b"?>") {
                Some(f) => {
                    *pos = f + 2;
                    continue;
                }
                None => return falta,
            },
            b'!' => {
                if resto.len() < 4 {
                    return falta;
                }
                if resto.starts_with(b"<!--") {
                    match buscar(d, p + 4, b"-->") {
                        Some(f) => {
                            *pos = f + 3;
                            continue;
                        }
                        None => return falta,
                    }
                }
                if resto.len() < 9 && b"<![CDATA[".starts_with(resto) {
                    return falta;
                }
                if resto.starts_with(b"<![CDATA[") {
                    return match buscar(d, p + 9, b"]]>") {
                        Some(f) => {
                            *pos = f + 3;
                            Paso::Evento(Evento::Cdata(&d[p + 9..f]))
                        }
                        None => falta,
                    };
                }
                // DOCTYPE u otra declaracion: se salta entera, con su
                // subconjunto interno `[...]` si lo trae, sin interpretarlo.
                let mut i = p + 2;
                let mut corchetes = 0i32;
                let mut comilla = 0u8;
                loop {
                    if i >= d.len() {
                        return falta;
                    }
                    let b = d[i];
                    if comilla != 0 {
                        if b == comilla {
                            comilla = 0;
                        }
                    } else if b == b'"' || b == b'\'' {
                        comilla = b;
                    } else if b == b'[' {
                        corchetes += 1;
                    } else if b == b']' {
                        corchetes -= 1;
                    } else if b == b'>' && corchetes <= 0 {
                        break;
                    }
                    i += 1;
                }
                *pos = i + 1;
                continue;
            }
            b'/' => {
                return match pos_byte(&d[p..], b'>') {
                    Some(n) => {
                        let nombre = recortar(&d[p + 2..p + n]);
                        *pos = p + n + 1;
                        Paso::Evento(Evento::Cierra {
                            nombre: local(nombre),
                        })
                    }
                    None => falta,
                };
            }
            _ => {
                // Etiqueta de apertura: hasta el `>` que no este entre comillas.
                let mut i = p + 1;
                let fin = loop {
                    let Some(k) = pos_de3(&d[i..], b'>', b'"', b'\'') else {
                        return falta;
                    };
                    let j = i + k;
                    let b = d[j];
                    if b == b'>' {
                        break j;
                    }
                    // Una comilla: hasta la que la cierra.
                    match pos_byte(&d[j + 1..], b) {
                        Some(c) => i = j + 1 + c + 1,
                        None => return falta,
                    }
                };
                let mut dentro = &d[p + 1..fin];
                let vacia = dentro.last() == Some(&b'/');
                if vacia {
                    dentro = &dentro[..dentro.len() - 1];
                }
                let corte = dentro
                    .iter()
                    .position(|b| b.is_ascii_whitespace())
                    .unwrap_or(dentro.len());
                *pos = fin + 1;
                return Paso::Evento(Evento::Abre {
                    nombre: local(&dentro[..corte]),
                    attrs: &dentro[corte..],
                    vacia,
                });
            }
        }
    }
}

fn recortar(mut s: &[u8]) -> &[u8] {
    while let Some((&b, r)) = s.split_first() {
        if b.is_ascii_whitespace() {
            s = r;
        } else {
            break;
        }
    }
    while let Some((&b, r)) = s.split_last() {
        if b.is_ascii_whitespace() {
            s = r;
        } else {
            break;
        }
    }
    s
}

/// Recorre los atributos `nombre="valor"` de una etiqueta. Nombres sin
/// prefijo y valores sin desescapar. Un atributo mal formado corta el
/// recorrido: lo que venga detras se ignora.
pub struct Atributos<'a> {
    d: &'a [u8],
    i: usize,
}

pub fn atributos(d: &[u8]) -> Atributos<'_> {
    Atributos { d, i: 0 }
}

impl<'a> Iterator for Atributos<'a> {
    type Item = (&'a [u8], &'a [u8]);
    fn next(&mut self) -> Option<Self::Item> {
        let d = self.d;
        let mut i = self.i;
        while i < d.len() && d[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= d.len() {
            return None;
        }
        let ini = i;
        while i < d.len() && d[i] != b'=' && !d[i].is_ascii_whitespace() {
            i += 1;
        }
        let nombre = &d[ini..i];
        while i < d.len() && d[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= d.len() || d[i] != b'=' {
            self.i = d.len();
            return None;
        }
        i += 1;
        while i < d.len() && d[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= d.len() || (d[i] != b'"' && d[i] != b'\'') {
            self.i = d.len();
            return None;
        }
        let q = d[i];
        i += 1;
        let vi = i;
        while i < d.len() && d[i] != q {
            i += 1;
        }
        let valor = &d[vi..i.min(d.len())];
        self.i = (i + 1).min(d.len());
        Some((local(nombre), valor))
    }
}

/// El valor de un atributo, sin desescapar.
pub fn atributo<'a>(attrs: &'a [u8], nombre: &[u8]) -> Option<&'a [u8]> {
    atributos(attrs).find(|(n, _)| *n == nombre).map(|(_, v)| v)
}

/// Resuelve las cinco entidades del XML y las numericas. Cualquier otra se
/// deja tal cual (nunca se expande nada definido en el documento).
pub fn desescapar(raw: &[u8]) -> Cow<'_, str> {
    if !raw.contains(&b'&') {
        return String::from_utf8_lossy(raw);
    }
    let s = String::from_utf8_lossy(raw);
    let mut out = String::with_capacity(s.len());
    let mut resto: &str = &s;
    while let Some(i) = resto.find('&') {
        out.push_str(&resto[..i]);
        let tras = &resto[i..];
        let fin = tras[..tras.len().min(12)].find(';');
        let Some(f) = fin else {
            out.push('&');
            resto = &tras[1..];
            continue;
        };
        let ent = &tras[1..f];
        let c = match ent {
            "lt" => Some('<'),
            "gt" => Some('>'),
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ if ent.starts_with("#x") || ent.starts_with("#X") => {
                u32::from_str_radix(&ent[2..], 16)
                    .ok()
                    .and_then(char::from_u32)
            }
            _ if ent.starts_with('#') => ent[1..].parse::<u32>().ok().and_then(char::from_u32),
            _ => None,
        };
        match c {
            Some(c) => {
                out.push(c);
                resto = &tras[f + 1..];
            }
            None => {
                out.push('&');
                resto = &tras[1..];
            }
        }
    }
    out.push_str(resto);
    Cow::Owned(out)
}

/// Quita los escapes `_xHHHH_` de OOXML (asi guarda Excel los caracteres de
/// control, p. ej. `_x000D_` para el retorno de carro).
pub fn sin_escapes_ooxml(s: &str) -> Cow<'_, str> {
    if !s.contains("_x") {
        return Cow::Borrowed(s);
    }
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let mut desde = 0;
    while i + 7 <= b.len() {
        if b[i] == b'_'
            && b[i + 1] == b'x'
            && b[i + 6] == b'_'
            && b[i + 2..i + 6].iter().all(|c| c.is_ascii_hexdigit())
        {
            // SAFETY de indices: los 7 bytes son ASCII, asi que i e i+7 caen
            // en limites de caracter.
            let codigo = u32::from_str_radix(&s[i + 2..i + 6], 16).unwrap_or(0xFFFD);
            out.push_str(&s[desde..i]);
            if let Some(c) = char::from_u32(codigo) {
                out.push(c);
            }
            i += 7;
            desde = i;
        } else {
            i += 1;
        }
    }
    out.push_str(&s[desde..]);
    Cow::Owned(out)
}

/// Un arbol pequeno para las partes chicas (libro, estilos, dibujos, tema,
/// relaciones). La hoja NO se lee asi: se lee de tiron.
#[derive(Debug, Clone, Default)]
pub struct Nodo {
    pub nombre: String,
    pub attrs: Vec<(String, String)>,
    pub hijos: Vec<Nodo>,
    pub texto: String,
}

/// Profundidad maxima del arbol: lo que pase de aqui se ignora (un fichero
/// hostil con millones de niveles no tumba la pila ni la memoria).
const PROFUNDIDAD_MAX: usize = 128;

/// Cuantos elementos como mucho en un arbol: las partes que se leen asi son
/// chicas (libro, estilos, dibujos); lo que pase de aqui se ignora.
const NODOS_MAX: usize = 4_000_000;

impl Nodo {
    pub fn attr(&self, nombre: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(n, _)| n == nombre)
            .map(|(_, v)| v.as_str())
    }

    pub fn hijo(&self, nombre: &str) -> Option<&Nodo> {
        self.hijos.iter().find(|h| h.nombre == nombre)
    }

    pub fn hijos_de<'a>(&'a self, nombre: &'a str) -> impl Iterator<Item = &'a Nodo> + 'a {
        self.hijos.iter().filter(move |h| h.nombre == nombre)
    }

    /// El primer descendiente (en profundidad) con ese nombre.
    pub fn buscar(&self, nombre: &str) -> Option<&Nodo> {
        for h in &self.hijos {
            if h.nombre == nombre {
                return Some(h);
            }
            if let Some(n) = h.buscar(nombre) {
                return Some(n);
            }
        }
        None
    }

    /// Todos los descendientes con ese nombre, en orden de documento.
    pub fn todos<'a>(&'a self, nombre: &str, salida: &mut Vec<&'a Nodo>) {
        for h in &self.hijos {
            if h.nombre == nombre {
                salida.push(h);
            }
            h.todos(nombre, salida);
        }
    }

    /// El texto de este nodo y de todos sus descendientes, seguido.
    pub fn texto_todo(&self) -> String {
        let mut s = self.texto.clone();
        for h in &self.hijos {
            s.push_str(&h.texto_todo());
        }
        s
    }
}

/// Lee un documento entero como arbol. Devuelve un nodo raiz sin nombre cuyos
/// hijos son los elementos de primer nivel. Nunca falla: lo mal formado se
/// cierra como se pueda.
pub fn arbol(d: &[u8]) -> Nodo {
    arbol_lim(d, NODOS_MAX)
}

fn arbol_lim(d: &[u8], nodos_max: usize) -> Nodo {
    let mut pila: Vec<Nodo> = vec![Nodo::default()];
    let mut ignorar = 0usize;
    let mut nodos = 0usize;
    let mut pos = 0usize;
    loop {
        match siguiente(d, &mut pos, true) {
            Paso::Evento(Evento::Abre {
                nombre,
                attrs,
                vacia,
            }) => {
                if ignorar > 0 || pila.len() > PROFUNDIDAD_MAX || nodos >= nodos_max {
                    if !vacia {
                        ignorar += 1;
                    }
                    continue;
                }
                nodos += 1;
                let nodo = Nodo {
                    nombre: String::from_utf8_lossy(nombre).into_owned(),
                    attrs: atributos(attrs)
                        .map(|(n, v)| {
                            (
                                String::from_utf8_lossy(n).into_owned(),
                                desescapar(v).into_owned(),
                            )
                        })
                        .collect(),
                    hijos: Vec::new(),
                    texto: String::new(),
                };
                if vacia {
                    if let Some(p) = pila.last_mut() {
                        p.hijos.push(nodo);
                    }
                } else {
                    pila.push(nodo);
                }
            }
            Paso::Evento(Evento::Cierra { .. }) => {
                if ignorar > 0 {
                    ignorar -= 1;
                    continue;
                }
                if pila.len() > 1 {
                    let n = pila.pop().unwrap_or_default();
                    if let Some(p) = pila.last_mut() {
                        p.hijos.push(n);
                    }
                }
            }
            Paso::Evento(Evento::Texto(t)) => {
                if ignorar == 0 && pila.len() > 1 {
                    if let Some(p) = pila.last_mut() {
                        p.texto.push_str(&desescapar(t));
                    }
                }
            }
            Paso::Evento(Evento::Cdata(t)) => {
                if ignorar == 0 && pila.len() > 1 {
                    if let Some(p) = pila.last_mut() {
                        p.texto.push_str(&String::from_utf8_lossy(t));
                    }
                }
            }
            Paso::Incompleto | Paso::Fin => break,
        }
    }
    while pila.len() > 1 {
        let n = pila.pop().unwrap_or_default();
        if let Some(p) = pila.last_mut() {
            p.hijos.push(n);
        }
    }
    pila.pop().unwrap_or_default()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn eventos(d: &[u8]) -> Vec<String> {
        let mut pos = 0;
        let mut v = Vec::new();
        while let Paso::Evento(e) = siguiente(d, &mut pos, true) {
            v.push(match e {
                Evento::Abre { nombre, vacia, .. } => format!(
                    "<{}{}",
                    String::from_utf8_lossy(nombre),
                    if vacia { "/" } else { "" }
                ),
                Evento::Cierra { nombre } => format!("</{}", String::from_utf8_lossy(nombre)),
                Evento::Texto(t) => format!("t:{}", String::from_utf8_lossy(t)),
                Evento::Cdata(t) => format!("c:{}", String::from_utf8_lossy(t)),
            });
        }
        v
    }

    #[test]
    fn lee_etiquetas_con_prefijo_comentarios_y_cdata() {
        let d = br#"<?xml version="1.0"?><!DOCTYPE x [<!ENTITY a "b">]><x:a k="1>2"><!-- n --><b/>hola<![CDATA[<z>]]></x:a>"#;
        assert_eq!(eventos(d), ["<a", "<b/", "t:hola", "c:<z>", "</a"]);
    }

    #[test]
    fn un_trozo_cortado_pide_mas_sin_avanzar() {
        let d = br#"<a k="1"><b"#;
        let mut pos = 0;
        assert!(matches!(
            siguiente(d, &mut pos, false),
            Paso::Evento(Evento::Abre { .. })
        ));
        let antes = pos;
        assert_eq!(siguiente(d, &mut pos, false), Paso::Incompleto);
        assert_eq!(pos, antes);
        assert_eq!(siguiente(d, &mut pos, true), Paso::Fin);
    }

    #[test]
    fn atributos_y_entidades() {
        let a: Vec<_> = atributos(br#" r="A1" s='3' x:t="s""#).collect();
        assert_eq!(a, [(&b"r"[..], &b"A1"[..]), (b"s", b"3"), (b"t", b"s")]);
        assert_eq!(desescapar(b"a&lt;b&amp;&#65;&#x42;&raro;"), "a<b&AB&raro;");
        assert_eq!(sin_escapes_ooxml("a_x000D_b_x41"), "a\rb_x41");
    }

    #[test]
    fn el_arbol_no_revienta_con_basura_ni_con_mucha_profundidad() {
        let mut d = Vec::new();
        for _ in 0..100_000 {
            d.extend_from_slice(b"<a>");
        }
        let n = arbol(&d);
        assert_eq!(n.hijos.len(), 1);
        let _ = arbol(b"<<<>>></a></b>&&&<a x=>");
        let _ = arbol(&[0xff, 0xfe, b'<', 0x80]);
    }
}

#[cfg(test)]
mod pruebas_limites {
    use super::*;

    #[test]
    fn un_arbol_con_mas_nodos_que_el_tope_no_crece_sin_limite() {
        let mut d = Vec::new();
        d.extend_from_slice(b"<r>");
        for _ in 0..1010 {
            d.extend_from_slice(b"<a/>");
        }
        d.extend_from_slice(b"</r>");
        let n = arbol_lim(&d, 1000);
        assert_eq!(n.hijos[0].hijos.len(), 999, "la raiz cuenta como uno");
    }
}
