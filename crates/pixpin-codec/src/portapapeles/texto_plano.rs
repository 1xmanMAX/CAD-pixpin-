//! **El texto plano de un HTML o un RTF del portapapeles.**
//!
//! Casi todo programa que copia con formato deja ademas `CF_UNICODETEXT`, y
//! ese manda (`leer`). Pero hay alguno que solo deja `HTML Format` o
//! `Rich Text Format` (editores viejos, algun navegador embebido): para que
//! pegar en el lienzo no se quede en nada, se saca aqui su texto. No se
//! pretende entender el formato: solo las letras y los saltos de parrafo.

use windows::Win32::Foundation::HGLOBAL;
use windows::Win32::System::DataExchange::{GetClipboardData, RegisterClipboardFormatW};
use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows::core::w;

/// El texto de lo que haya como HTML o RTF, con el portapapeles ya abierto.
pub(super) fn leer_con_formato() -> Option<String> {
    let de_html = super::tabla::leer_html()
        .map(|b| texto_de_html(&super::tabla::fragmento_de(&b)))
        .filter(|t| !t.trim().is_empty());
    de_html.or_else(|| {
        leer_rtf()
            .map(|b| texto_de_rtf(&b))
            .filter(|t| !t.trim().is_empty())
    })
}

/// Los bytes de `Rich Text Format`, con el portapapeles ya abierto.
fn leer_rtf() -> Option<Vec<u8>> {
    // SAFETY: cadena literal terminada en NUL; la funcion solo la lee.
    let formato = unsafe { RegisterClipboardFormatW(w!("Rich Text Format")) };
    if formato == 0 {
        return None;
    }
    // SAFETY: portapapeles abierto; se lee sin pasar de `GlobalSize` y solo
    // mientras dura el bloqueo.
    unsafe {
        let handle = GetClipboardData(formato).ok()?;
        let bloque = HGLOBAL(handle.0);
        let tamano = GlobalSize(bloque);
        let datos = GlobalLock(bloque) as *const u8;
        if datos.is_null() {
            return None;
        }
        let bytes = std::slice::from_raw_parts(datos, tamano.min(64 * 1024 * 1024));
        let fin = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        let copia = bytes[..fin].to_vec();
        let _ = GlobalUnlock(bloque);
        Some(copia)
    }
}

/// Las etiquetas que cierran un renglon.
const DE_BLOQUE: [&str; 16] = [
    "p",
    "div",
    "br",
    "li",
    "tr",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "pre",
    "blockquote",
    "ul",
    "ol",
    "table",
];

/// **El texto de un fragmento HTML**: sin etiquetas, sin `<script>` ni
/// `<style>`, con las entidades deshechas, los espacios juntados como los
/// junta el navegador y un salto por cada bloque.
pub fn texto_de_html(html: &str) -> String {
    let mut salida = String::with_capacity(html.len() / 2);
    let mut resto = html;
    let mut en_pre = false;
    while !resto.is_empty() {
        if let Some(tras) = resto.strip_prefix("<!--") {
            resto = tras.find("-->").map_or("", |i| &tras[i + 3..]);
            continue;
        }
        if resto.starts_with('<') {
            let Some(fin) = resto.find('>') else { break };
            let etiqueta = &resto[1..fin];
            resto = &resto[fin + 1..];
            let cierra = etiqueta.starts_with('/');
            let nombre: String = etiqueta
                .trim_start_matches('/')
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect::<String>()
                .to_ascii_lowercase();
            if !cierra && (nombre == "script" || nombre == "style") {
                let cierre = format!("</{nombre}");
                let bajo = resto.to_ascii_lowercase();
                resto = match bajo.find(&cierre) {
                    Some(i) => {
                        let tras = &resto[i..];
                        tras.find('>').map_or("", |j| &tras[j + 1..])
                    }
                    None => "",
                };
                continue;
            }
            if nombre == "pre" {
                en_pre = !cierra;
            }
            if nombre == "td" || nombre == "th" {
                if cierra {
                    salida.push('\t');
                }
            } else if DE_BLOQUE.contains(&nombre.as_str())
                && (cierra || nombre == "br")
                && !salida.ends_with('\n')
            {
                // `<br>` siempre hace renglon; un cierre de bloque, solo si
                // no se acaba de hacer.
                salida.push('\n');
            } else if nombre == "br" {
                salida.push('\n');
            }
            continue;
        }
        let hasta = resto.find('<').unwrap_or(resto.len());
        let trozo = deshacer_entidades(&resto[..hasta]);
        resto = &resto[hasta..];
        if en_pre {
            salida.push_str(&trozo);
        } else {
            for c in trozo.chars() {
                if c.is_whitespace() && c != '\u{a0}' {
                    if !salida.is_empty() && !salida.ends_with([' ', '\n', '\t']) {
                        salida.push(' ');
                    }
                } else if c == '\u{a0}' {
                    salida.push(' ');
                } else {
                    salida.push(c);
                }
            }
        }
    }
    salida
        .lines()
        .map(|r| r.trim_end_matches([' ', '\t']))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// `&amp;`, `&lt;`, `&#233;`, `&#x1F600;` y las mas comunes con nombre.
fn deshacer_entidades(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut salida = String::with_capacity(s.len());
    let mut resto = s;
    while let Some(i) = resto.find('&') {
        salida.push_str(&resto[..i]);
        let tras = &resto[i + 1..];
        let fin = tras.find(';').filter(|&f| f <= 10);
        let decodificado = fin.and_then(|f| {
            let e = &tras[..f];
            let c = if let Some(n) = e.strip_prefix("#x").or_else(|| e.strip_prefix("#X")) {
                u32::from_str_radix(n, 16).ok().and_then(char::from_u32)
            } else if let Some(n) = e.strip_prefix('#') {
                n.parse().ok().and_then(char::from_u32)
            } else {
                match e {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    "nbsp" => Some('\u{a0}'),
                    "ntilde" => Some('ñ'),
                    "Ntilde" => Some('Ñ'),
                    "aacute" => Some('á'),
                    "eacute" => Some('é'),
                    "iacute" => Some('í'),
                    "oacute" => Some('ó'),
                    "uacute" => Some('ú'),
                    "hellip" => Some('…'),
                    "mdash" => Some('—'),
                    "ndash" => Some('–'),
                    _ => None,
                }
            };
            c.map(|c| (c, f))
        });
        match decodificado {
            Some((c, f)) => {
                salida.push(c);
                resto = &tras[f + 1..];
            }
            None => {
                salida.push('&');
                resto = tras;
            }
        }
    }
    salida.push_str(resto);
    salida
}

/// **El texto de un RTF**: las letras de los grupos que se ven, `\par` y
/// `\line` como saltos, `\tab`, los `\'e9` (pagina de codigos 1252) y los
/// `\uN` de Unicode con su sustituto saltado (`\ucN`).
pub fn texto_de_rtf(rtf: &[u8]) -> String {
    let mut salida = String::new();
    // Por grupo: si se ve, y cuantos sustitutos lleva cada `\u`.
    let mut pila: Vec<(bool, usize)> = vec![(true, 1)];
    let mut saltar = 0usize;
    let mut alto_sustituto: Option<u16> = None;
    let mut i = 0;
    let visible = |p: &Vec<(bool, usize)>| p.last().is_some_and(|g| g.0);
    while i < rtf.len() {
        let b = rtf[i];
        match b {
            b'{' => {
                let arriba = *pila.last().unwrap_or(&(true, 1));
                pila.push(arriba);
                i += 1;
                // `{\*\destino ...}`: un destino que no se conoce, fuera.
                if rtf[i..].starts_with(b"\\*")
                    && let Some(g) = pila.last_mut()
                {
                    g.0 = false;
                }
            }
            b'}' => {
                if pila.len() > 1 {
                    pila.pop();
                }
                i += 1;
            }
            b'\\' => {
                i += 1;
                let Some(&c) = rtf.get(i) else { break };
                if !c.is_ascii_alphabetic() {
                    i += 1;
                    match c {
                        b'\'' => {
                            let hex = rtf.get(i..i + 2).and_then(|h| std::str::from_utf8(h).ok());
                            i += 2;
                            if let Some(v) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                                if saltar > 0 {
                                    saltar -= 1;
                                } else if visible(&pila) {
                                    salida.push(de_1252(v));
                                }
                            }
                        }
                        b'\\' | b'{' | b'}' if visible(&pila) && saltar == 0 => {
                            salida.push(c as char)
                        }
                        b'~' if visible(&pila) => salida.push(' '),
                        _ => {}
                    }
                    continue;
                }
                let inicio = i;
                while i < rtf.len() && rtf[i].is_ascii_alphabetic() {
                    i += 1;
                }
                let palabra = std::str::from_utf8(&rtf[inicio..i]).unwrap_or("");
                let inicio_num = i;
                if i < rtf.len() && rtf[i] == b'-' {
                    i += 1;
                }
                while i < rtf.len() && rtf[i].is_ascii_digit() {
                    i += 1;
                }
                let numero: Option<i32> = std::str::from_utf8(&rtf[inicio_num..i])
                    .ok()
                    .and_then(|n| n.parse().ok());
                if i < rtf.len() && rtf[i] == b' ' {
                    i += 1;
                }
                let ve = visible(&pila);
                match palabra {
                    "fonttbl" | "colortbl" | "stylesheet" | "info" | "pict" | "object"
                    | "header" | "footer" | "listtable" | "listoverridetable" | "themedata"
                    | "datastore" | "latentstyles" | "generator" => {
                        if let Some(g) = pila.last_mut() {
                            g.0 = false;
                        }
                    }
                    "par" | "line" | "row" if ve => salida.push('\n'),
                    "tab" | "cell" if ve => salida.push('\t'),
                    "uc" => {
                        if let Some(g) = pila.last_mut() {
                            g.1 = numero.unwrap_or(1).max(0) as usize;
                        }
                    }
                    "u" => {
                        let v = numero.unwrap_or(0);
                        let v = if v < 0 { v + 65536 } else { v } as u32;
                        if ve {
                            match v {
                                0xD800..=0xDBFF => alto_sustituto = Some(v as u16),
                                0xDC00..=0xDFFF => {
                                    if let Some(alto) = alto_sustituto.take() {
                                        salida
                                            .extend(char::decode_utf16([alto, v as u16]).flatten());
                                    }
                                }
                                _ => salida.extend(char::from_u32(v)),
                            }
                        }
                        saltar = pila.last().map_or(1, |g| g.1);
                    }
                    _ => {}
                }
            }
            b'\r' | b'\n' => i += 1,
            _ => {
                if saltar > 0 {
                    saltar -= 1;
                } else if visible(&pila) {
                    salida.push(de_1252(b));
                }
                i += 1;
            }
        }
    }
    salida.trim().to_string()
}

/// Un byte de la pagina de codigos 1252 (la del RTF de Windows en Europa y
/// America): igual que Latin-1 salvo el tramo 0x80..0x9F.
fn de_1252(b: u8) -> char {
    const TRAMO: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž',
        '\u{8f}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}',
        'ž', 'Ÿ',
    ];
    match b {
        0x80..=0x9F => TRAMO[(b - 0x80) as usize],
        _ => b as char,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn html_con_parrafos_entidades_y_basura() {
        let html = "<style>p{color:red}</style><p>Hola&nbsp;<b>mundo</b>  &amp; más</p>\
                    <!-- nota --><div>Segundo<br>renglón &#x1F600;</div><script>x()</script>";
        assert_eq!(texto_de_html(html), "Hola mundo & más\nSegundo\nrenglón 😀");
    }

    #[test]
    fn html_vacio_o_solo_etiquetas_no_da_texto() {
        assert_eq!(texto_de_html(""), "");
        assert_eq!(texto_de_html("<p></p><br><img src=x>"), "");
        // Un & suelto no se pierde.
        assert_eq!(texto_de_html("a & b"), "a & b");
    }

    #[test]
    fn rtf_de_wordpad_con_tildes_y_unicode() {
        let rtf = br"{\rtf1\ansi\ansicpg1252{\fonttbl{\f0\fnil Calibri;}}{\*\generator Riched20}\f0\fs22 Canci\'f3n \u241?and\'fa\par Segunda\tab l\'ednea \u-10179?\u-8704?\par}";
        assert_eq!(texto_de_rtf(rtf), "Canción ñandú\nSegunda\tlínea 😀");
    }

    #[test]
    fn rtf_raro_no_rompe() {
        assert_eq!(texto_de_rtf(b""), "");
        assert_eq!(texto_de_rtf(b"{\\rtf1 \\"), "");
        assert_eq!(texto_de_rtf(b"}}}{{"), "");
        assert_eq!(texto_de_rtf(b"{\\rtf1 \\'zz hola}"), "hola");
    }
}
