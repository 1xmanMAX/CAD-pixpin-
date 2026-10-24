//! **El texto de un PDF, con donde cae cada letra** (D9: buscar dentro del
//! documento).
//!
//! `Windows.Data.Pdf` solo dibuja: no dice que texto hay en la hoja ni donde.
//! Traer pdfium o lopdf al ejecutable seria meter megas por una caja de
//! buscar, asi que se lee con el mismo analizador propio que ya pega PDF
//! (`union`), que entiende los dos indices, los flujos de objetos y Flate.
//! Aqui se anade lo que falta para el texto:
//!
//! - **Los flujos de contenido**: se siguen los operadores de texto (`BT`,
//!   `Tf`, `Td`/`TD`/`Tm`/`T*`, `Tj`/`TJ`/`'`/`"`, `Tc`/`Tw`/`Tz`/`TL`/`Ts`)
//!   con la matriz de texto y la de `cm`/`q`/`Q`, y se entra en los
//!   formularios (`Do`) hasta tres niveles.
//! - **Que letra es cada codigo**: el `ToUnicode` de la fuente si lo trae
//!   (lo normal en cualquier PDF de Word o de un navegador); si no, en una
//!   fuente de un byte, `WinAnsi` con sus `Differences` mas comunes. Una
//!   fuente de dos bytes sin `ToUnicode` no se puede leer y se salta: es
//!   mejor no encontrar que encontrar basura.
//! - **Donde cae**: el ancho de cada glifo (`Widths`, o `W` en las de dos
//!   bytes) da la caja de cada letra, que sale en fracciones de la hoja
//!   (0..1 desde arriba a la izquierda, con su `/Rotate`), para que el
//!   lector la ponga encima de la hoja pintada a cualquier aumento.
//!
//! Lo que NO hace: un PDF escaneado no tiene texto (es una foto) y aqui se
//! dice asi; uno cifrado tampoco se lee (habria que descifrar cada flujo).
//! El orden es el del flujo de contenido, que es el de lectura en lo que
//! sale de Word, LibreOffice o un navegador.

use std::collections::HashMap;

use crate::union::{Archivo, Dicc, Valor, descodificar, en, entero};

/// Una hoja leida: su texto y, por cada letra de `texto`, su caja
/// `[x0, y0, x1, y1]` en fracciones de la hoja (0..1, desde arriba a la
/// izquierda). Los espacios que se anaden entre palabras no tienen caja.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PaginaDeTexto {
    pub texto: String,
    pub cajas: Vec<Option<[f32; 4]>>,
}

impl PaginaDeTexto {
    /// Las cajas de las letras `inicio..inicio+largo` (en letras), unidas
    /// por renglon: lo que se marca al encontrar una palabra. Las letras de
    /// un mismo renglon se juntan en una caja; al cambiar de renglon, otra.
    pub fn cajas_de(&self, inicio: usize, largo: usize) -> Vec<[f32; 4]> {
        let mut salida: Vec<[f32; 4]> = Vec::new();
        for c in self.cajas.iter().skip(inicio).take(largo).flatten() {
            if let Some(u) = salida.last_mut() {
                let alto = (u[3] - u[1]).max(c[3] - c[1]);
                let mismo_renglon = (c[1] - u[1]).abs() < alto * 0.5 && c[0] >= u[0] - alto;
                if mismo_renglon {
                    u[0] = u[0].min(c[0]);
                    u[1] = u[1].min(c[1]);
                    u[2] = u[2].max(c[2]);
                    u[3] = u[3].max(c[3]);
                    continue;
                }
            }
            salida.push(*c);
        }
        salida
    }
}

/// Por que no hay texto que buscar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SinTexto {
    NoEsPdf,
    Cifrado,
}

/// Cuantos niveles de formularios dentro de formularios se siguen.
const HONDO_FORMULARIOS: u32 = 3;

/// **El texto de todas las hojas.** Una hoja sin texto (una foto escaneada)
/// sale vacia, no falta: las posiciones siguen siendo las de las hojas.
pub fn de_bytes(bytes: &[u8]) -> Result<Vec<PaginaDeTexto>, SinTexto> {
    let archivo = Archivo::leer(bytes).ok_or(SinTexto::NoEsPdf)?;
    if archivo.cifrado() {
        return Err(SinTexto::Cifrado);
    }
    let mut fuentes = Fuentes::default();
    Ok(archivo
        .paginas()
        .into_iter()
        .map(|n| pagina(&archivo, n, &mut fuentes))
        .collect())
}

/// Lo mismo, del fichero.
pub fn de_fichero(ruta: &std::path::Path) -> Result<Vec<PaginaDeTexto>, SinTexto> {
    let bytes = std::fs::read(ruta).map_err(|_| SinTexto::NoEsPdf)?;
    de_bytes(&bytes)
}

/// **Un trozo de texto corrido**, para pasar el PDF a texto
/// (`pixpin_docs::pdf_a_texto`, el `PdfAHtml.trozosDe` del movil): lo que
/// escribe un `Tj` (o una cadena de un `TJ`), en **puntos** desde arriba a
/// la izquierda de lo que se ve de la hoja (con su `/Rotate`). `x`..`fin`
/// a lo ancho, `y` la linea base y `alto` el tamano de la letra. Solo sale
/// lo que va derecho y de izquierda a derecha: un rotulo girado no es el
/// texto corrido.
#[derive(Debug, Clone, PartialEq)]
pub struct TrozoDeTexto {
    pub x: f32,
    pub y: f32,
    pub fin: f32,
    pub alto: f32,
    pub texto: String,
    pub negrita: bool,
    pub cursiva: bool,
}

/// **Los trozos de texto de cada hoja** (ver [`TrozoDeTexto`]). Una hoja
/// sin texto (un escaneo) sale vacia, no falta.
pub fn trozos_de_bytes(bytes: &[u8]) -> Result<Vec<Vec<TrozoDeTexto>>, SinTexto> {
    let archivo = Archivo::leer(bytes).ok_or(SinTexto::NoEsPdf)?;
    if archivo.cifrado() {
        return Err(SinTexto::Cifrado);
    }
    let mut fuentes = Fuentes::default();
    Ok(archivo
        .paginas()
        .into_iter()
        .map(|n| leer_hoja(&archivo, n, &mut fuentes, true).1)
        .collect())
}

/// Lo mismo, del fichero.
pub fn trozos_de_fichero(ruta: &std::path::Path) -> Result<Vec<Vec<TrozoDeTexto>>, SinTexto> {
    let bytes = std::fs::read(ruta).map_err(|_| SinTexto::NoEsPdf)?;
    trozos_de_bytes(&bytes)
}

// ---------------------------------------------------------------------------
// Matrices

#[derive(Debug, Clone, Copy, PartialEq)]
struct M([f32; 6]);

const IDENTIDAD: M = M([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);

impl M {
    /// `self × otra`, en el orden de la norma (la de la izquierda se aplica
    /// primero).
    fn por(self, o: M) -> M {
        let [a, b, c, d, e, f] = self.0;
        let [p, q, r, s, t, u] = o.0;
        M([
            a * p + b * r,
            a * q + b * s,
            c * p + d * r,
            c * q + d * s,
            e * p + f * r + t,
            e * q + f * s + u,
        ])
    }

    fn punto(self, x: f32, y: f32) -> (f32, f32) {
        let [a, b, c, d, e, f] = self.0;
        (x * a + y * c + e, x * b + y * d + f)
    }

    fn mover(x: f32, y: f32) -> M {
        M([1.0, 0.0, 0.0, 1.0, x, y])
    }
}

// ---------------------------------------------------------------------------
// Fuentes

#[derive(Debug, Clone, Default)]
struct Fuente {
    /// Codigos de dos bytes (`Type0`).
    dos_bytes: bool,
    unicode: HashMap<u32, String>,
    /// Ancho de cada codigo en milesimas de la letra.
    anchos: HashMap<u32, f32>,
    ancho_por_omision: f32,
    /// Sin `ToUnicode` y de un byte: se lee como WinAnsi (con `Differences`).
    diferencias: HashMap<u32, char>,
    /// Lo que no dicen ni el `ToUnicode` ni las `Differences`, en MacRoman
    /// y no en WinAnsi (ver [`mac_roman`]).
    mac_roman: bool,
    /// Por el nombre de la letra (`Arial-BoldMT`, `Times-Italic`): lo que
    /// usa el PDF pasado a texto para los titulos en negrita.
    negrita: bool,
    cursiva: bool,
}

/// Negrita y cursiva por el nombre de la letra, que es lo unico que dicen
/// casi todas (el `FontWeight` del descriptor no lo pone casi nadie).
fn estilo_del_nombre(nombre: &[u8]) -> (bool, bool) {
    let n = String::from_utf8_lossy(nombre).to_lowercase();
    let negrita = ["bold", "black", "heavy", "semibold", "demi"]
        .iter()
        .any(|k| n.contains(k));
    let cursiva = ["italic", "oblique"].iter().any(|k| n.contains(k));
    (negrita, cursiva)
}

impl Fuente {
    fn letras(&self, codigo: u32) -> Option<String> {
        if let Some(s) = self.unicode.get(&codigo) {
            return Some(s.clone());
        }
        if self.dos_bytes {
            return None;
        }
        if let Some(c) = self.diferencias.get(&codigo) {
            return Some(c.to_string());
        }
        if self.mac_roman && codigo >= 128 {
            return mac_roman(codigo as u8).map(str::to_string);
        }
        winansi(codigo as u8).map(|c| c.to_string())
    }

    fn ancho(&self, codigo: u32) -> f32 {
        self.anchos
            .get(&codigo)
            .copied()
            .unwrap_or(self.ancho_por_omision)
    }
}

#[derive(Default)]
struct Fuentes {
    por_objeto: HashMap<u32, std::rc::Rc<Fuente>>,
}

impl Fuentes {
    fn de(&mut self, archivo: &Archivo, v: &Valor) -> std::rc::Rc<Fuente> {
        let n = match v {
            Valor::Ref(n, _) => Some(*n),
            _ => None,
        };
        if let Some(f) = n.and_then(|n| self.por_objeto.get(&n)) {
            return f.clone();
        }
        let f = std::rc::Rc::new(
            archivo
                .dicc_de(Some(v))
                .map(|d| leer_fuente(archivo, &d))
                .unwrap_or_default(),
        );
        if let Some(n) = n {
            self.por_objeto.insert(n, f.clone());
        }
        f
    }
}

fn nombre_de(v: Option<&Valor>) -> Option<&[u8]> {
    match v {
        Some(Valor::Nombre(n)) => Some(n),
        _ => None,
    }
}

pub(crate) fn numero_de(v: &Valor) -> Option<f32> {
    match v {
        Valor::Numero(t) => std::str::from_utf8(t).ok()?.parse().ok(),
        _ => None,
    }
}

fn leer_fuente(archivo: &Archivo, d: &Dicc) -> Fuente {
    let mut f = Fuente {
        ancho_por_omision: 500.0,
        ..Default::default()
    };
    if let Some(n) = nombre_de(en(d, b"BaseFont")) {
        (f.negrita, f.cursiva) = estilo_del_nombre(n);
    }
    let tipo0 = nombre_de(en(d, b"Subtype")) == Some(b"Type0");
    if tipo0 {
        f.dos_bytes = true;
        f.ancho_por_omision = 1000.0;
        if let Some(Valor::Lista(hijas)) = archivo.resolver(en(d, b"DescendantFonts"))
            && let Some(hija) = archivo.dicc_de(hijas.first())
        {
            if let Some(dw) = archivo
                .resolver(en(&hija, b"DW"))
                .as_ref()
                .and_then(numero_de)
            {
                f.ancho_por_omision = dw;
            }
            if let Some(Valor::Lista(w)) = archivo.resolver(en(&hija, b"W")) {
                anchos_cid(archivo, &w, &mut f.anchos);
            }
        }
    } else {
        let primero = entero(en(d, b"FirstChar")).unwrap_or(0).max(0) as u32;
        if let Some(Valor::Lista(w)) = archivo.resolver(en(d, b"Widths")) {
            for (i, v) in w.iter().enumerate() {
                if let Some(a) = archivo.resolver(Some(v)).as_ref().and_then(numero_de) {
                    f.anchos.insert(primero + i as u32, a);
                }
            }
        }
        if let Some(desc) = archivo.dicc_de(en(d, b"FontDescriptor"))
            && let Some(a) = archivo
                .resolver(en(&desc, b"MissingWidth"))
                .as_ref()
                .and_then(numero_de)
            && a > 0.0
        {
            f.ancho_por_omision = a;
        }
        f.mac_roman = es_mac_roman(archivo, d);
        if let Some(Valor::Dicc(enc)) = archivo.resolver(en(d, b"Encoding"))
            && let Some(Valor::Lista(dif)) = archivo.resolver(en(&enc, b"Differences"))
        {
            let mut codigo = 0u32;
            for v in &dif {
                match v {
                    Valor::Numero(_) => codigo = numero_de(v).unwrap_or(0.0).max(0.0) as u32,
                    Valor::Nombre(n) => {
                        if let Some(c) = letra_de_glifo(n) {
                            f.diferencias.insert(codigo, c);
                        }
                        codigo += 1;
                    }
                    _ => {}
                }
            }
        }
    }
    if let Some(Valor::Flujo(sd, crudo)) = archivo.resolver(en(d, b"ToUnicode"))
        && let Some(datos) = descodificar(&sd, &crudo)
    {
        let (unicode, bytes) = leer_cmap(&datos);
        f.unicode = unicode;
        if let Some(b) = bytes {
            f.dos_bytes = b >= 2;
        }
    }
    f
}

/// `W` de una fuente CID: `c [w1 w2 ...]` o `c1 c2 w`.
fn anchos_cid(archivo: &Archivo, w: &[Valor], anchos: &mut HashMap<u32, f32>) {
    let mut i = 0;
    while i < w.len() {
        let Some(c) = numero_de(&w[i]) else {
            i += 1;
            continue;
        };
        match archivo.resolver(w.get(i + 1)) {
            Some(Valor::Lista(l)) => {
                for (k, v) in l.iter().enumerate().take(65536) {
                    if let Some(a) = numero_de(v) {
                        anchos.insert(c as u32 + k as u32, a);
                    }
                }
                i += 2;
            }
            Some(v2) => {
                let (Some(c2), Some(a)) = (numero_de(&v2), w.get(i + 2).and_then(numero_de)) else {
                    break;
                };
                let (c, c2) = (c as u32, (c2 as u32).min(c as u32 + 65535));
                for k in c..=c2 {
                    anchos.insert(k, a);
                }
                i += 3;
            }
            None => break,
        }
    }
}

/// Los nombres de glifo mas comunes de un `Differences`: letras sueltas, las
/// acentuadas del castellano y `uniXXXX`.
fn letra_de_glifo(n: &[u8]) -> Option<char> {
    let s = std::str::from_utf8(n).ok()?;
    if s.chars().count() == 1 {
        return s.chars().next();
    }
    if let Some(h) = s.strip_prefix("uni").filter(|h| h.len() == 4) {
        return char::from_u32(u32::from_str_radix(h, 16).ok()?);
    }
    Some(match s {
        "space" => ' ',
        "period" => '.',
        "comma" => ',',
        "colon" => ':',
        "semicolon" => ';',
        "hyphen" | "minus" => '-',
        "quoteright" => '\u{2019}',
        "quoteleft" => '\u{2018}',
        "quotedbl" => '"',
        "parenleft" => '(',
        "parenright" => ')',
        "question" => '?',
        "questiondown" => '¿',
        "exclam" => '!',
        "exclamdown" => '¡',
        "aacute" => 'á',
        "eacute" => 'é',
        "iacute" => 'í',
        "oacute" => 'ó',
        "uacute" => 'ú',
        "ntilde" => 'ñ',
        "udieresis" => 'ü',
        "Aacute" => 'Á',
        "Eacute" => 'É',
        "Iacute" => 'Í',
        "Oacute" => 'Ó',
        "Uacute" => 'Ú',
        "Ntilde" => 'Ñ',
        "Udieresis" => 'Ü',
        "zero" => '0',
        "one" => '1',
        "two" => '2',
        "three" => '3',
        "four" => '4',
        "five" => '5',
        "six" => '6',
        "seven" => '7',
        "eight" => '8',
        "nine" => '9',
        _ => return None,
    })
}

/// `MacRomanEncoding` de 128 a 255 (`MAC_ROMAN` de `PlanoDePdf` del movil).
const MAC_ROMAN: &str = "ÄÅÇÉÑÖÜáàâäãåçéèêëíìîïñóòôöõúùûü†°¢£§•¶ß®©™´¨≠ÆØ∞±≤≥¥µ∂∑∏π∫ªºΩæø¿¡¬√ƒ≈∆«»…\u{a0}ÀÃÕŒœ–—“”‘’÷◊ÿŸ⁄€‹›ﬁﬂ‡·‚„‰ÂÊÁËÈÍÎÏÌÓÔ\u{f8ff}ÒÚÛÙıˆ˜¯˘˙˚¸˝˛ˇ";

/// **Una letra de 128 a 255 en MacRoman** (Android v0.98.0): de 128 en
/// adelante no es Latin-1, y sin esto en los articulos hechos en Mac la
/// ligadura «fi» salia «Þ» y «defined» no se encontraba. Las ligaduras van
/// en sus dos letras, que es lo que se busca.
pub(crate) fn mac_roman(b: u8) -> Option<&'static str> {
    static TABLA: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    let t = TABLA.get_or_init(|| {
        let mut v = Vec::with_capacity(128);
        let mut resto = MAC_ROMAN;
        while let Some(c) = resto.chars().next() {
            let (una, sigue) = resto.split_at(c.len_utf8());
            v.push(match c {
                'ﬁ' => "fi",
                'ﬂ' => "fl",
                _ => una,
            });
            resto = sigue;
        }
        v
    });
    b.checked_sub(128).and_then(|i| t.get(i as usize).copied())
}

/// Si la fuente de un byte `d` dice `MacRomanEncoding`, suelta o como
/// `BaseEncoding` de su diccionario de codificacion.
pub(crate) fn es_mac_roman(archivo: &Archivo, d: &Dicc) -> bool {
    let es = |v: Option<&Valor>| matches!(v, Some(Valor::Nombre(n)) if n == b"MacRomanEncoding");
    match archivo.resolver(en(d, b"Encoding")) {
        Some(Valor::Dicc(enc)) => es(archivo.resolver(en(&enc, b"BaseEncoding")).as_ref()),
        otro => es(otro.as_ref()),
    }
}

/// La tabla de `WinAnsiEncoding` (la de Windows-1252): lo que se lee en una
/// fuente de un byte sin `ToUnicode`, que es lo que escribe este mismo
/// crate con Helvetica.
fn winansi(b: u8) -> Option<char> {
    const ALTOS: [u16; 32] = [
        0x20AC, 0, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039,
        0x0152, 0, 0x017D, 0, 0, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC,
        0x2122, 0x0161, 0x203A, 0x0153, 0, 0x017E, 0x0178,
    ];
    match b {
        0x20..=0x7E => Some(b as char),
        0x80..=0x9F => char::from_u32(ALTOS[(b - 0x80) as usize] as u32).filter(|c| *c != '\0'),
        0xA0..=0xFF => char::from_u32(b as u32),
        _ => None,
    }
}

/// Un `ToUnicode`: de codigo a letras, y cuantos bytes tiene un codigo (por
/// su `codespacerange`), si lo dice.
fn leer_cmap(datos: &[u8]) -> (HashMap<u32, String>, Option<usize>) {
    let mut mapa = HashMap::new();
    let mut bytes = None;
    let toks = trocear(datos);
    let mut i = 0;
    let hex = |t: &Tok| match t {
        Tok::Cadena(b, true) => Some(b.clone()),
        _ => None,
    };
    let codigo = |b: &[u8]| b.iter().fold(0u32, |a, x| (a << 8) | *x as u32);
    while i < toks.len() {
        match &toks[i] {
            Tok::Op(o) if o == b"begincodespacerange" => {
                if let Some(b) = toks.get(i + 1).and_then(hex) {
                    bytes = Some(b.len());
                }
                i += 1;
            }
            Tok::Op(o) if o == b"beginbfchar" => {
                i += 1;
                while i + 1 < toks.len() && !matches!(&toks[i], Tok::Op(_)) {
                    if let (Some(a), Some(b)) = (hex(&toks[i]), hex(&toks[i + 1])) {
                        mapa.insert(codigo(&a), utf16be(&b));
                    }
                    i += 2;
                }
            }
            Tok::Op(o) if o == b"beginbfrange" => {
                i += 1;
                while i + 2 < toks.len() && !matches!(&toks[i], Tok::Op(_)) {
                    let (Some(a), Some(b)) = (hex(&toks[i]), hex(&toks[i + 1])) else {
                        i += 3;
                        continue;
                    };
                    let (lo, hi) = (codigo(&a), codigo(&b));
                    // Un rango absurdo es un fichero roto: no se reserva un
                    // millon de entradas por el.
                    let hi = hi.min(lo.saturating_add(65535));
                    match &toks[i + 2] {
                        Tok::Cadena(d, true) => {
                            let mut unidades: Vec<u16> = d
                                .chunks(2)
                                .map(|c| ((c[0] as u16) << 8) | *c.get(1).unwrap_or(&0) as u16)
                                .collect();
                            for k in lo..=hi {
                                mapa.insert(k, String::from_utf16_lossy(&unidades));
                                if let Some(u) = unidades.last_mut() {
                                    *u = u.wrapping_add(1);
                                }
                            }
                        }
                        Tok::Lista(l) => {
                            for (k, t) in (lo..=hi).zip(l.iter()) {
                                if let Some(d) = hex(t) {
                                    mapa.insert(k, utf16be(&d));
                                }
                            }
                        }
                        _ => {}
                    }
                    i += 3;
                }
            }
            _ => i += 1,
        }
    }
    (mapa, bytes)
}

fn utf16be(b: &[u8]) -> String {
    let unidades: Vec<u16> = b
        .chunks(2)
        .map(|c| ((c[0] as u16) << 8) | *c.get(1).unwrap_or(&0) as u16)
        .collect();
    String::from_utf16_lossy(&unidades)
}

// ---------------------------------------------------------------------------
// El flujo de contenido, en piezas

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f32),
    Nombre(Vec<u8>),
    /// Los bytes ya sin escapar, y si venia en hexadecimal.
    Cadena(Vec<u8>, bool),
    Lista(Vec<Tok>),
    Op(Vec<u8>),
}

fn es_blanco(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\r' | b'\n' | b'\x0c' | b'\0')
}

fn es_delimitador(c: u8) -> bool {
    matches!(
        c,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

/// Trocea un flujo de contenido (o un CMap). Los diccionarios en linea
/// (`<< >>` de un `BDC`) se saltan: no llevan texto que se vea. Una imagen
/// en linea (`BI ... ID ... EI`) se salta entera: sus bytes no son piezas.
fn trocear(b: &[u8]) -> Vec<Tok> {
    let mut pila: Vec<Vec<Tok>> = vec![Vec::new()];
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if es_blanco(c) {
            i += 1;
            continue;
        }
        let tok = match c {
            b'%' => {
                while i < b.len() && !matches!(b[i], b'\n' | b'\r') {
                    i += 1;
                }
                continue;
            }
            b'[' => {
                pila.push(Vec::new());
                i += 1;
                continue;
            }
            b']' => {
                i += 1;
                if pila.len() > 1 {
                    let l = pila.pop().unwrap_or_default();
                    Tok::Lista(l)
                } else {
                    continue;
                }
            }
            b'<' if b.get(i + 1) == Some(&b'<') => {
                // Un diccionario en linea: se salta hasta su cierre.
                let mut nivel = 0;
                while i < b.len() {
                    if b[i..].starts_with(b"<<") {
                        nivel += 1;
                        i += 2;
                    } else if b[i..].starts_with(b">>") {
                        nivel -= 1;
                        i += 2;
                        if nivel == 0 {
                            break;
                        }
                    } else {
                        i += 1;
                    }
                }
                continue;
            }
            b'<' => {
                i += 1;
                let mut digitos = Vec::new();
                while i < b.len() && b[i] != b'>' {
                    if b[i].is_ascii_hexdigit() {
                        digitos.push(b[i]);
                    }
                    i += 1;
                }
                i += 1;
                if digitos.len() % 2 == 1 {
                    digitos.push(b'0');
                }
                let bytes = digitos
                    .chunks(2)
                    .filter_map(|p| u8::from_str_radix(std::str::from_utf8(p).ok()?, 16).ok())
                    .collect();
                Tok::Cadena(bytes, true)
            }
            b'(' => {
                let (bytes, fin) = cadena_literal(b, i + 1);
                i = fin;
                Tok::Cadena(bytes, false)
            }
            b'/' => {
                i += 1;
                let inicio = i;
                while i < b.len() && !es_blanco(b[i]) && !es_delimitador(b[i]) {
                    i += 1;
                }
                Tok::Nombre(b[inicio..i].to_vec())
            }
            b'>' | b')' | b'{' | b'}' => {
                i += 1;
                continue;
            }
            _ => {
                let inicio = i;
                while i < b.len() && !es_blanco(b[i]) && !es_delimitador(b[i]) {
                    i += 1;
                }
                let palabra = &b[inicio..i];
                if palabra.is_empty() {
                    i += 1;
                    continue;
                }
                if let Some(n) = std::str::from_utf8(palabra)
                    .ok()
                    .and_then(|s| s.parse::<f32>().ok())
                {
                    Tok::Num(n)
                } else if palabra == b"ID" {
                    // Los datos de una imagen en linea, hasta `EI` suelto.
                    i += 1;
                    while i + 2 <= b.len() {
                        if &b[i..i + 2] == b"EI"
                            && (i == 0 || es_blanco(b[i - 1]))
                            && b.get(i + 2).is_none_or(|c| es_blanco(*c))
                        {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                    continue;
                } else {
                    Tok::Op(palabra.to_vec())
                }
            }
        };
        if let Some(cima) = pila.last_mut() {
            cima.push(tok);
        }
    }
    // Una lista sin cerrar se cierra sola.
    while pila.len() > 1 {
        let l = pila.pop().unwrap_or_default();
        if let Some(cima) = pila.last_mut() {
            cima.push(Tok::Lista(l));
        }
    }
    pila.pop().unwrap_or_default()
}

/// `( ... )` con sus escapes y parentesis anidados. Devuelve los bytes y por
/// donde seguir.
fn cadena_literal(b: &[u8], mut i: usize) -> (Vec<u8>, usize) {
    let mut s = Vec::new();
    let mut nivel = 1;
    while i < b.len() {
        let c = b[i];
        i += 1;
        match c {
            b'\\' => {
                let Some(&e) = b.get(i) else { break };
                i += 1;
                match e {
                    b'n' => s.push(b'\n'),
                    b'r' => s.push(b'\r'),
                    b't' => s.push(b'\t'),
                    b'b' => s.push(8),
                    b'f' => s.push(12),
                    b'0'..=b'7' => {
                        let mut v = (e - b'0') as u32;
                        for _ in 0..2 {
                            match b.get(i) {
                                Some(&d @ b'0'..=b'7') => {
                                    v = v * 8 + (d - b'0') as u32;
                                    i += 1;
                                }
                                _ => break,
                            }
                        }
                        s.push(v as u8);
                    }
                    b'\r' => {
                        if b.get(i) == Some(&b'\n') {
                            i += 1;
                        }
                    }
                    b'\n' => {}
                    otro => s.push(otro),
                }
            }
            b'(' => {
                nivel += 1;
                s.push(c);
            }
            b')' => {
                nivel -= 1;
                if nivel == 0 {
                    break;
                }
                s.push(c);
            }
            _ => s.push(c),
        }
    }
    (s, i)
}

// ---------------------------------------------------------------------------
// La hoja

/// Lo que una hoja hereda de su arbol: se busca subiendo por `/Parent`.
pub(crate) fn heredado(archivo: &Archivo, pagina: &Dicc, clave: &[u8]) -> Option<Valor> {
    let mut d = pagina.clone();
    for _ in 0..32 {
        if let Some(v) = en(&d, clave) {
            return archivo.resolver(Some(v));
        }
        d = archivo.dicc_de(en(&d, b"Parent"))?;
    }
    None
}

pub(crate) fn caja_de(v: Option<Valor>) -> Option<[f32; 4]> {
    let Some(Valor::Lista(l)) = v else {
        return None;
    };
    let n: Vec<f32> = l.iter().filter_map(numero_de).collect();
    if n.len() != 4 {
        return None;
    }
    let caja = [
        n[0].min(n[2]),
        n[1].min(n[3]),
        n[0].max(n[2]),
        n[1].max(n[3]),
    ];
    (caja[2] - caja[0] > 0.0 && caja[3] - caja[1] > 0.0).then_some(caja)
}

/// Lo que se va leyendo de una hoja.
struct Lectura<'a> {
    archivo: &'a Archivo<'a>,
    fuentes: &'a mut Fuentes,
    caja: [f32; 4],
    giro: i64,
    salida: PaginaDeTexto,
    /// El final de la ultima letra puesta: `(x, y de la base, tamano)` en
    /// el espacio de la hoja, para decidir si hace falta un espacio.
    ultima: Option<(f32, f32, f32)>,
    /// Los trozos de texto corrido, si se piden (`trozos_de_bytes`).
    trozos: Option<Vec<TrozoDeTexto>>,
}

#[derive(Clone)]
struct EstadoTexto {
    ctm: M,
    fuente: Option<std::rc::Rc<Fuente>>,
    tam: f32,
    tc: f32,
    tw: f32,
    th: f32,
    tl: f32,
    rise: f32,
}

fn pagina(archivo: &Archivo, n: u32, fuentes: &mut Fuentes) -> PaginaDeTexto {
    leer_hoja(archivo, n, fuentes, false).0
}

/// Una hoja leida: su texto con las cajas y, con `con_trozos`, sus trozos de
/// texto corrido (sin ellos no se apunta nada de mas).
fn leer_hoja(
    archivo: &Archivo,
    n: u32,
    fuentes: &mut Fuentes,
    con_trozos: bool,
) -> (PaginaDeTexto, Vec<TrozoDeTexto>) {
    let Some(d) = archivo.dicc_de(Some(&Valor::Ref(n, 0))) else {
        return Default::default();
    };
    let caja = caja_de(heredado(archivo, &d, b"CropBox"))
        .or_else(|| caja_de(heredado(archivo, &d, b"MediaBox")))
        .unwrap_or([0.0, 0.0, 612.0, 792.0]);
    let giro = giro_de_la_hoja(archivo, &d);
    let recursos = heredado(archivo, &d, b"Resources")
        .and_then(|v| match v {
            Valor::Dicc(d) => Some(d),
            _ => None,
        })
        .unwrap_or_default();
    let mut datos = Vec::new();
    match archivo.resolver(en(&d, b"Contents")) {
        Some(Valor::Flujo(sd, crudo)) => {
            if let Some(x) = descodificar(&sd, &crudo) {
                datos = x;
            }
        }
        Some(Valor::Lista(l)) => {
            for v in &l {
                if let Some(Valor::Flujo(sd, crudo)) = archivo.resolver(Some(v))
                    && let Some(x) = descodificar(&sd, &crudo)
                {
                    datos.extend_from_slice(&x);
                    // Los trozos se leen como uno solo; un blanco entre
                    // medias evita pegar el ultimo operador con el primero.
                    datos.push(b'\n');
                }
            }
        }
        _ => {}
    }
    let mut l = Lectura {
        archivo,
        fuentes,
        caja,
        giro,
        salida: PaginaDeTexto::default(),
        ultima: None,
        trozos: con_trozos.then(Vec::new),
    };
    l.flujo(&datos, &recursos, IDENTIDAD, 0);
    (l.salida, l.trozos.unwrap_or_default())
}

impl Lectura<'_> {
    fn flujo(&mut self, datos: &[u8], recursos: &Dicc, ctm: M, hondo: u32) {
        let fuentes_de_la_hoja = self
            .archivo
            .dicc_de(en(recursos, b"Font"))
            .unwrap_or_default();
        let formularios = self
            .archivo
            .dicc_de(en(recursos, b"XObject"))
            .unwrap_or_default();
        let mut g = EstadoTexto {
            ctm,
            fuente: None,
            tam: 0.0,
            tc: 0.0,
            tw: 0.0,
            th: 1.0,
            tl: 0.0,
            rise: 0.0,
        };
        let mut guardados: Vec<EstadoTexto> = Vec::new();
        let (mut tm, mut tlm) = (IDENTIDAD, IDENTIDAD);
        let mut args: Vec<Tok> = Vec::new();
        let num = |args: &[Tok], i: usize| match args.get(i) {
            Some(Tok::Num(n)) => *n,
            _ => 0.0,
        };
        for t in trocear(datos) {
            let Tok::Op(op) = t else {
                args.push(t);
                continue;
            };
            let n = args.len();
            match op.as_slice() {
                b"q" => guardados.push(g.clone()),
                b"Q" => {
                    if let Some(x) = guardados.pop() {
                        g = x;
                    }
                }
                b"cm" if n >= 6 => {
                    let m = M([
                        num(&args, n - 6),
                        num(&args, n - 5),
                        num(&args, n - 4),
                        num(&args, n - 3),
                        num(&args, n - 2),
                        num(&args, n - 1),
                    ]);
                    g.ctm = m.por(g.ctm);
                }
                b"BT" => {
                    tm = IDENTIDAD;
                    tlm = IDENTIDAD;
                }
                b"Tf" if n >= 2 => {
                    g.tam = num(&args, n - 1);
                    if let Some(Tok::Nombre(nombre)) = args.get(n - 2) {
                        g.fuente = en(&fuentes_de_la_hoja, nombre)
                            .map(|v| self.fuentes.de(self.archivo, v));
                    }
                }
                b"Tc" if n >= 1 => g.tc = num(&args, n - 1),
                b"Tw" if n >= 1 => g.tw = num(&args, n - 1),
                b"Tz" if n >= 1 => g.th = num(&args, n - 1) / 100.0,
                b"TL" if n >= 1 => g.tl = num(&args, n - 1),
                b"Ts" if n >= 1 => g.rise = num(&args, n - 1),
                b"Td" | b"TD" if n >= 2 => {
                    let (x, y) = (num(&args, n - 2), num(&args, n - 1));
                    if op == b"TD" {
                        g.tl = -y;
                    }
                    tlm = M::mover(x, y).por(tlm);
                    tm = tlm;
                }
                b"Tm" if n >= 6 => {
                    tlm = M([
                        num(&args, n - 6),
                        num(&args, n - 5),
                        num(&args, n - 4),
                        num(&args, n - 3),
                        num(&args, n - 2),
                        num(&args, n - 1),
                    ]);
                    tm = tlm;
                }
                b"T*" => {
                    tlm = M::mover(0.0, -g.tl).por(tlm);
                    tm = tlm;
                }
                b"Tj" | b"'" | b"\"" => {
                    if op == b"\"" && n >= 3 {
                        g.tw = num(&args, n - 3);
                        g.tc = num(&args, n - 2);
                    }
                    if op != b"Tj" {
                        tlm = M::mover(0.0, -g.tl).por(tlm);
                        tm = tlm;
                    }
                    if let Some(Tok::Cadena(b, _)) = args.last() {
                        let b = b.clone();
                        self.escribir(&b, &g, &mut tm);
                    }
                }
                b"TJ" => {
                    if let Some(Tok::Lista(l)) = args.last() {
                        let l = l.clone();
                        for pieza in &l {
                            match pieza {
                                Tok::Cadena(b, _) => self.escribir(b, &g, &mut tm),
                                Tok::Num(k) => {
                                    let tx = -k / 1000.0 * g.tam * g.th;
                                    tm = M::mover(tx, 0.0).por(tm);
                                }
                                _ => {}
                            }
                        }
                    }
                }
                b"Do" if hondo < HONDO_FORMULARIOS => {
                    if let Some(Tok::Nombre(nombre)) = args.last()
                        && let Some(Valor::Flujo(sd, crudo)) =
                            self.archivo.resolver(en(&formularios, nombre))
                        && nombre_de(en(&sd, b"Subtype")) == Some(b"Form")
                        && let Some(x) = descodificar(&sd, &crudo)
                    {
                        let matriz = match self.archivo.resolver(en(&sd, b"Matrix")) {
                            Some(Valor::Lista(l)) if l.len() == 6 => {
                                let v: Vec<f32> =
                                    l.iter().map(|v| numero_de(v).unwrap_or(0.0)).collect();
                                M([v[0], v[1], v[2], v[3], v[4], v[5]])
                            }
                            _ => IDENTIDAD,
                        };
                        let propios = self
                            .archivo
                            .dicc_de(en(&sd, b"Resources"))
                            .unwrap_or_else(|| recursos.clone());
                        self.flujo(&x, &propios, matriz.por(g.ctm), hondo + 1);
                    }
                }
                _ => {}
            }
            args.clear();
        }
    }

    /// Pone las letras de una cadena y corre la matriz de texto.
    fn escribir(&mut self, bytes: &[u8], g: &EstadoTexto, tm: &mut M) {
        let Some(fuente) = g.fuente.clone() else {
            return;
        };
        let paso = if fuente.dos_bytes { 2 } else { 1 };
        // Para el trozo de texto corrido: donde empieza, que dice y donde acaba.
        let apuntar = self.trozos.is_some();
        let mut corrido: Option<(M, String, (f32, f32))> = None;
        for trozo in bytes.chunks(paso) {
            let codigo = trozo.iter().fold(0u32, |a, x| (a << 8) | *x as u32);
            let w0 = fuente.ancho(codigo) / 1000.0;
            let espacio = if paso == 1 && codigo == 32 { g.tw } else { 0.0 };
            let trm = M([g.tam * g.th, 0.0, 0.0, g.tam, 0.0, g.rise])
                .por(*tm)
                .por(g.ctm);
            if apuntar && let Some(letras) = fuente.letras(codigo) {
                let c = corrido.get_or_insert_with(|| (trm, String::new(), (0.0, 0.0)));
                c.1.extend(letras.chars().filter(|c| *c != '\0'));
                c.2 = trm.punto(w0, 0.0);
            }
            if let Some(letras) = fuente.letras(codigo) {
                // La caja del glifo: su ancho y de la base un poco abajo a
                // bastante arriba, que es donde caen las letras.
                let esquinas = [
                    trm.punto(0.0, -0.22),
                    trm.punto(w0, -0.22),
                    trm.punto(0.0, 0.85),
                    trm.punto(w0, 0.85),
                ];
                let x0 = esquinas.iter().map(|p| p.0).fold(f32::MAX, f32::min);
                let x1 = esquinas.iter().map(|p| p.0).fold(f32::MIN, f32::max);
                let y0 = esquinas.iter().map(|p| p.1).fold(f32::MAX, f32::min);
                let y1 = esquinas.iter().map(|p| p.1).fold(f32::MIN, f32::max);
                let base = trm.punto(0.0, 0.0).1;
                let tam = (y1 - y0).abs().max(0.1);
                self.poner(&letras, [x0, y0, x1, y1], base, tam);
            }
            let tx = (w0 * g.tam + g.tc + espacio) * g.th;
            *tm = M::mover(tx, 0.0).por(*tm);
        }
        if let Some((trm, texto, fin)) = corrido
            && let Some(t) = self.trozo_corrido(trm, texto, fin, &fuente)
            && let Some(lista) = self.trozos.as_mut()
        {
            lista.push(t);
        }
    }

    /// Un punto de la hoja en puntos de lo que se ve de ella, desde arriba a
    /// la izquierda y con su giro.
    fn a_la_vista(&self, p: (f32, f32)) -> (f32, f32) {
        let f = a_fraccion(self.caja, self.giro, [p.0, p.1, p.0, p.1]);
        let (w, h) = (self.caja[2] - self.caja[0], self.caja[3] - self.caja[1]);
        let (w, h) = if matches!(self.giro, 90 | 270) {
            (h, w)
        } else {
            (w, h)
        };
        (f[0] * w, f[1] * h)
    }

    /// El trozo de texto corrido de una cadena, si va derecho y de izquierda
    /// a derecha (`PdfAHtml.trozosDe`): un rotulo girado o en espejo, no.
    fn trozo_corrido(
        &self,
        trm: M,
        texto: String,
        fin: (f32, f32),
        fuente: &Fuente,
    ) -> Option<TrozoDeTexto> {
        if texto.is_empty() {
            return None;
        }
        let origen = self.a_la_vista(trm.punto(0.0, 0.0));
        let eje_x = self.a_la_vista(trm.punto(1.0, 0.0));
        let eje_y = self.a_la_vista(trm.punto(0.0, 1.0));
        let (ax, ay) = (eje_x.0 - origen.0, eje_x.1 - origen.1);
        let ancho = ax.hypot(ay);
        let alto = (eje_y.0 - origen.0).hypot(eje_y.1 - origen.1);
        if alto < 0.5 || ax <= 0.0 || ay.abs() > 0.2 * ancho {
            return None;
        }
        Some(TrozoDeTexto {
            x: origen.0,
            y: origen.1,
            fin: self.a_la_vista(fin).0,
            alto,
            texto,
            negrita: fuente.negrita,
            cursiva: fuente.cursiva,
        })
    }

    fn poner(&mut self, letras: &str, caja: [f32; 4], base: f32, tam: f32) {
        if let Some((x_fin, y_base, t)) = self.ultima {
            let otra_linea = (base - y_base).abs() > 0.5 * t.min(tam);
            let hueco = caja[0] - x_fin > 0.2 * tam;
            let ya_hay = self.salida.texto.ends_with(char::is_whitespace);
            let empieza = letras.starts_with(char::is_whitespace);
            if (otra_linea || hueco) && !ya_hay && !empieza {
                self.salida.texto.push(' ');
                self.salida.cajas.push(None);
            }
        }
        let fraccion = self.a_fraccion(caja);
        for c in letras.chars() {
            if c == '\0' {
                continue;
            }
            self.salida.texto.push(c);
            self.salida.cajas.push(Some(fraccion));
        }
        self.ultima = Some((caja[2], base, tam));
    }

    /// De puntos de la hoja a fracciones de lo que se ve de ella, desde
    /// arriba a la izquierda y con su giro.
    fn a_fraccion(&self, c: [f32; 4]) -> [f32; 4] {
        a_fraccion(self.caja, self.giro, c)
    }
}

/// El giro de la hoja (heredado), en 0, 90, 180 o 270.
pub(crate) fn giro_de_la_hoja(archivo: &Archivo, d: &Dicc) -> i64 {
    heredado(archivo, d, b"Rotate")
        .as_ref()
        .and_then(numero_de)
        .map_or(0, |g| ((g as i64 % 360) + 360) % 360)
}

/// Una caja en puntos de la hoja (`caja` es su papel, `giro` su `/Rotate`)
/// en fracciones de lo que se ve de ella: 0..1 desde arriba a la izquierda.
pub(crate) fn a_fraccion(caja: [f32; 4], giro: i64, c: [f32; 4]) -> [f32; 4] {
    let [bx0, by0, bx1, by1] = caja;
    let (w, h) = (bx1 - bx0, by1 - by0);
    let u0 = (c[0] - bx0) / w;
    let u1 = (c[2] - bx0) / w;
    let v0 = (by1 - c[3]) / h;
    let v1 = (by1 - c[1]) / h;
    let (a0, b0, a1, b1) = match giro {
        90 => (1.0 - v1, u0, 1.0 - v0, u1),
        180 => (1.0 - u1, 1.0 - v1, 1.0 - u0, 1.0 - v0),
        270 => (v0, 1.0 - u1, v1, 1.0 - u0),
        _ => (u0, v0, u1, v1),
    };
    [a0, b0, a1, b1]
}

/// **El texto y los enlaces de una sola hoja** (desde 0), leyendo el indice
/// del PDF una vez: lo que pide el lector para elegir texto y seguir
/// enlaces en la hoja que se ve, sin leer el documento entero.
pub fn hoja_con_enlaces(
    bytes: &[u8],
    i: usize,
) -> Result<(PaginaDeTexto, Vec<crate::enlaces::Enlace>), SinTexto> {
    let archivo = Archivo::leer(bytes).ok_or(SinTexto::NoEsPdf)?;
    if archivo.cifrado() {
        return Err(SinTexto::Cifrado);
    }
    let paginas = archivo.paginas();
    let Some(&n) = paginas.get(i) else {
        return Ok(Default::default());
    };
    let mut fuentes = Fuentes::default();
    let texto = pagina(&archivo, n, &mut fuentes);
    let enlaces = crate::enlaces::de_la_hoja_en(&archivo, &paginas, i);
    Ok((texto, enlaces))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un PDF escrito a mano, sin indice: el lector lo reconstruye
    /// recorriendo los objetos, como con cualquier PDF remendado.
    fn pdf(objetos: &[String]) -> Vec<u8> {
        let mut s = String::from("%PDF-1.4\n");
        for (i, o) in objetos.iter().enumerate() {
            s.push_str(&format!("{} 0 obj\n{o}\nendobj\n", i + 1));
        }
        s.push_str("trailer\n<< /Root 1 0 R >>\n%%EOF\n");
        s.into_bytes()
    }

    fn flujo(contenido: &str) -> String {
        format!(
            "<< /Length {} >>\nstream\n{contenido}\nendstream",
            contenido.len()
        )
    }

    /// Una hoja de 200x100 puntos con Helvetica y lo que se pida escrito.
    fn una_hoja(contenido: &str, extra_pagina: &str) -> Vec<u8> {
        pdf(&[
            "<< /Type /Catalog /Pages 2 0 R >>".into(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 200 100] >>".into(),
            format!("<< /Type /Page /Parent 2 0 R /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R {extra_pagina} >>"),
            flujo(contenido),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /FirstChar 32 /Widths [250 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 500 500 500 500 500 500 500 500 500 500 0 0 0 0 0 0 0 600 600 600 600 600 600 600 600 600 600 600 600 600 600 600 600 600 600 600 600 600 600 600 600 600 600 0 0 0 0 0 0 500 500 500 500 500 500 500 500 500 500 500 500 500 500 500 500 500 500 500 500 500 500 500 500 500 500] >>".into(),
        ])
    }

    #[test]
    fn el_texto_de_una_hoja_sale_con_la_caja_de_cada_letra() {
        let b = una_hoja("BT /F1 10 Tf 20 50 Td (Hola mundo) Tj ET", "");
        let p = de_bytes(&b).unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].texto, "Hola mundo");
        assert_eq!(p[0].cajas.len(), p[0].texto.chars().count());
        // La H empieza en x=20 de 200 (0,1) y ocupa 6 puntos (0,03).
        let h = p[0].cajas[0].unwrap();
        assert!((h[0] - 0.1).abs() < 1e-3, "{h:?}");
        assert!((h[2] - 0.13).abs() < 1e-3, "{h:?}");
        // La base esta a 50 de 100 desde abajo: la letra cae hacia la mitad,
        // mas arriba que abajo.
        assert!(h[1] < 0.5 && h[3] > 0.5, "{h:?}");
        // Las letras van de izquierda a derecha.
        let o = p[0].cajas[1].unwrap();
        assert!(o[0] >= h[2] - 1e-3);
    }

    // ---- Los trozos de texto corrido (el PDF pasado a texto) ----

    fn uno(b: &[u8]) -> Vec<TrozoDeTexto> {
        let mut hojas = trozos_de_bytes(b).unwrap();
        assert_eq!(hojas.len(), 1);
        hojas.remove(0)
    }

    fn cerca(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn los_trozos_salen_en_puntos_desde_arriba_con_su_tamano() {
        let t = uno(&una_hoja("BT /F1 10 Tf 20 50 Td (Hola) Tj ET", ""));
        assert_eq!(t.len(), 1);
        let t = &t[0];
        assert_eq!(t.texto, "Hola");
        // H 600 y «ola» 500 cada una, a 10 puntos: 21 de ancho.
        assert!(cerca(t.x, 20.0) && cerca(t.fin, 41.0), "{t:?}");
        // La base a 50 desde abajo en una hoja de 100: 50 desde arriba.
        assert!(cerca(t.y, 50.0) && cerca(t.alto, 10.0), "{t:?}");
        assert!(!t.negrita && !t.cursiva);
    }

    #[test]
    fn lo_girado_no_es_texto_corrido_y_lo_derecho_si() {
        let t = uno(&una_hoja(
            "BT /F1 10 Tf 0 1 -1 0 100 10 Tm (Arriba) Tj ET\n\
             BT /F1 10 Tf -1 0 0 1 150 70 Tm (Espejo) Tj ET\n\
             BT /F1 10 Tf 20 50 Td (Derecho) Tj ET",
            "",
        ));
        let textos: Vec<&str> = t.iter().map(|t| t.texto.as_str()).collect();
        assert_eq!(textos, ["Derecho"]);
    }

    #[test]
    fn en_una_hoja_girada_el_texto_que_se_ve_derecho_cuenta() {
        // Con /Rotate 90 lo que va hacia arriba en el papel se ve de
        // izquierda a derecha: la hoja vista mide 100 de ancho y 200 de alto.
        let t = uno(&una_hoja(
            "BT /F1 10 Tf 0 1 -1 0 50 20 Tm (Hola) Tj ET",
            "/Rotate 90",
        ));
        assert_eq!(t.len(), 1, "{t:?}");
        assert!(
            cerca(t[0].x, 20.0) && cerca(t[0].y, 50.0) && cerca(t[0].fin, 41.0),
            "{t:?}"
        );
    }

    #[test]
    fn la_negrita_y_la_cursiva_salen_del_nombre_de_la_letra() {
        let b = pdf(&[
            "<< /Type /Catalog /Pages 2 0 R >>".into(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 200 100] >>".into(),
            "<< /Type /Page /Parent 2 0 R /Resources << /Font << /F1 5 0 R /F2 6 0 R >> >> /Contents 4 0 R >>".into(),
            flujo("BT /F1 12 Tf 10 80 Td (Titulo) Tj ET BT /F2 10 Tf 10 40 Td (nota) Tj ET"),
            "<< /Type /Font /Subtype /Type1 /BaseFont /ABCDEF+Arial-BoldMT >>".into(),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Times-Italic >>".into(),
        ]);
        let t = uno(&b);
        assert_eq!(t.len(), 2);
        assert!(t[0].negrita && !t[0].cursiva, "{:?}", t[0]);
        assert!(!t[1].negrita && t[1].cursiva, "{:?}", t[1]);
        assert_eq!(estilo_del_nombre(b"Helvetica"), (false, false));
    }

    #[test]
    fn sin_pdf_no_hay_trozos_y_un_escaneo_da_hojas_vacias() {
        assert_eq!(
            trozos_de_bytes(b"esto no es un pdf"),
            Err(SinTexto::NoEsPdf)
        );
        let t = uno(&una_hoja("0 0 1 rg 10 10 50 50 re f", ""));
        assert!(t.is_empty());
        // Y pedir los trozos no cambia el texto de buscar.
        let b = una_hoja("BT /F1 10 Tf 20 50 Td (Hola mundo) Tj ET", "");
        assert_eq!(de_bytes(&b).unwrap()[0].texto, "Hola mundo");
    }

    #[test]
    fn las_palabras_separadas_por_hueco_o_por_linea_llevan_espacio() {
        // TJ con un hueco grande entre palabras (lo que escribe Word) y una
        // segunda linea con T*.
        let b = una_hoja(
            "BT /F1 10 Tf 12 TL 20 80 Td [(uno) -600 (dos)] TJ T* (tres) Tj ET",
            "",
        );
        let p = de_bytes(&b).unwrap();
        assert_eq!(p[0].texto, "uno dos tres");
        // El espacio anadido no tiene caja: no se pinta nada donde no hay letra.
        assert_eq!(p[0].cajas[3], None);
        // El kerning pequeno de una palabra no la parte.
        let b = una_hoja("BT /F1 10 Tf 20 80 Td [(pa) -20 (labra)] TJ ET", "");
        assert_eq!(de_bytes(&b).unwrap()[0].texto, "palabra");
    }

    #[test]
    fn una_palabra_partida_en_dos_renglones_se_marca_con_dos_cajas() {
        let b = una_hoja(
            "BT /F1 10 Tf 12 TL 20 80 Td (hola mun) Tj T* (do) Tj ET",
            "",
        );
        let p = &de_bytes(&b).unwrap()[0];
        assert_eq!(p.texto, "hola mun do");
        // «mun do»: dos renglones, dos cajas; «hola»: una.
        assert_eq!(p.cajas_de(5, 6).len(), 2);
        let hola = p.cajas_de(0, 4);
        assert_eq!(hola.len(), 1);
        assert!((hola[0][0] - 0.1).abs() < 1e-3);
        // Fuera del texto no hay cajas.
        assert!(p.cajas_de(100, 3).is_empty());
    }

    #[test]
    fn las_cadenas_con_escapes_y_winansi_se_leen_bien() {
        let b = una_hoja(r"BT /F1 10 Tf 20 50 Td (a\(b\) a\361o \200) Tj ET", "");
        assert_eq!(de_bytes(&b).unwrap()[0].texto, "a(b) año €");
    }

    /// Una hoja con la fuente `F1` en otra codificacion (`encoding`: lo que
    /// va detras de `/Encoding`).
    fn una_hoja_con(contenido: &str, encoding: &str) -> Vec<u8> {
        pdf(&[
            "<< /Type /Catalog /Pages 2 0 R >>".into(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 200 100] >>".into(),
            "<< /Type /Page /Parent 2 0 R /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>".into(),
            flujo(contenido),
            format!("<< /Type /Font /Subtype /Type1 /BaseFont /Times-Roman /Encoding {encoding} >>"),
        ])
    }

    /// **MacRoman** (Android v0.98.0): de 128 en adelante no es Latin-1. En
    /// los articulos hechos en Mac la ligadura «fi» (0xDE) salia «Þ» y
    /// «defined» no se encontraba.
    #[test]
    fn en_macroman_la_ligadura_fi_es_fi_y_no_una_thorn() {
        let b = una_hoja_con(
            r"BT /F1 10 Tf 20 50 Td (de\336ned \216t\216 \321) Tj ET",
            "/MacRomanEncoding",
        );
        assert_eq!(de_bytes(&b).unwrap()[0].texto, "defined été —");
        // Tambien como `BaseEncoding` de un diccionario, y las `Differences`
        // mandan sobre la tabla.
        let b = una_hoja_con(
            r"BT /F1 10 Tf 20 50 Td (\336\337\216) Tj ET",
            "<< /BaseEncoding /MacRomanEncoding /Differences [142 /A] >>",
        );
        assert_eq!(de_bytes(&b).unwrap()[0].texto, "fiflA");
        // La tabla llena de 128 a 255, y nada por debajo.
        assert_eq!(
            (128..=255u8).filter(|b| mac_roman(*b).is_some()).count(),
            128
        );
        assert_eq!(
            (mac_roman(0x80), mac_roman(0xFF), mac_roman(0x41)),
            (Some("Ä"), Some("ˇ"), None)
        );
        // Caso negativo: en WinAnsi el 0xDE sigue siendo «Þ».
        let b = una_hoja_con(r"BT /F1 10 Tf 20 50 Td (\336) Tj ET", "/WinAnsiEncoding");
        assert_eq!(de_bytes(&b).unwrap()[0].texto, "Þ");
    }

    #[test]
    fn una_fuente_de_dos_bytes_se_lee_por_su_to_unicode() {
        let cmap = "/CIDInit /ProcSet findresource begin 12 dict begin begincmap\n1 begincodespacerange <0000> <FFFF> endcodespacerange\n2 beginbfchar <0003> <0041> <0004> <00F1> endbfchar\n1 beginbfrange <0010> <0012> <0061> endbfrange\nendcmap end end";
        let b = pdf(&[
            "<< /Type /Catalog /Pages 2 0 R >>".into(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Resources << /Font << /F2 5 0 R >> >> /Contents 4 0 R >>".into(),
            flujo("BT /F2 12 Tf 10 10 Td <0003000400100011 0012> Tj ET"),
            "<< /Type /Font /Subtype /Type0 /BaseFont /X /Encoding /Identity-H /DescendantFonts [6 0 R] /ToUnicode 7 0 R >>".into(),
            "<< /Type /Font /Subtype /CIDFontType2 /DW 600 /W [3 [700 800]] >>".into(),
            flujo(cmap),
        ]);
        let p = de_bytes(&b).unwrap();
        assert_eq!(p[0].texto, "Añabc");
        // La A mide 700 milesimas de 12 puntos: 8,4 de 100.
        let a = p[0].cajas[0].unwrap();
        assert!((a[2] - a[0] - 0.084).abs() < 1e-3, "{a:?}");
    }

    #[test]
    fn una_fuente_de_dos_bytes_sin_to_unicode_no_inventa_letras() {
        let b = pdf(&[
            "<< /Type /Catalog /Pages 2 0 R >>".into(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Resources << /Font << /F2 5 0 R >> >> /Contents 4 0 R >>".into(),
            flujo("BT /F2 12 Tf 10 10 Td <00030004> Tj ET"),
            "<< /Type /Font /Subtype /Type0 /BaseFont /X /Encoding /Identity-H >>".into(),
        ]);
        assert_eq!(de_bytes(&b).unwrap()[0].texto, "");
    }

    #[test]
    fn una_hoja_girada_pone_las_cajas_donde_se_ve() {
        // Girada 90: lo que estaba arriba a la izquierda sale arriba a la
        // derecha.
        let b = una_hoja("BT /F1 10 Tf 0 90 Td (H) Tj ET", "/Rotate 90");
        let h = de_bytes(&b).unwrap()[0].cajas[0].unwrap();
        assert!(h[0] > 0.5 && h[1] < 0.2, "{h:?}");
    }

    #[test]
    fn el_texto_dentro_de_un_formulario_tambien_sale_y_la_imagen_en_linea_no_estorba() {
        let b = pdf(&[
            "<< /Type /Catalog /Pages 2 0 R >>".into(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Resources << /XObject << /X1 5 0 R >> >> /Contents 4 0 R >>".into(),
            flujo("q BI /W 2 /H 1 /BPC 8 /CS /G ID \u{1}(Tj EI Q q 1 0 0 1 10 10 cm /X1 Do Q"),
            format!(
                "<< /Type /XObject /Subtype /Form /BBox [0 0 50 50] /Resources << /Font << /F1 6 0 R >> >> /Length 30 >>\nstream\n{}\nendstream",
                "BT /F1 8 Tf (dentro) Tj ET"
            ),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),
        ]);
        assert_eq!(de_bytes(&b).unwrap()[0].texto, "dentro");
    }

    #[test]
    fn lo_que_no_es_pdf_y_lo_cifrado_se_dicen() {
        assert_eq!(de_bytes(b"hola, no soy un pdf"), Err(SinTexto::NoEsPdf));
        let mut b = una_hoja("BT /F1 10 Tf (x) Tj ET", "");
        let s = String::from_utf8(b.clone()).unwrap().replace(
            "<< /Root 1 0 R >>",
            "<< /Root 1 0 R /Encrypt << /Filter /Standard >> >>",
        );
        b = s.into_bytes();
        assert_eq!(de_bytes(&b), Err(SinTexto::Cifrado));
    }

    #[test]
    fn una_sola_hoja_sale_con_su_texto_y_sus_enlaces() {
        let b = una_hoja(
            "BT /F1 10 Tf 20 50 Td (Hola) Tj ET",
            "/Annots [<< /Subtype /Link /Rect [20 45 44 60] /A << /S /URI /URI (https://example.org) >> >>]",
        );
        let (t, e) = hoja_con_enlaces(&b, 0).unwrap();
        assert_eq!(t, de_bytes(&b).unwrap()[0], "lo mismo que leyendo todas");
        assert_eq!(e.len(), 1);
        assert_eq!(
            e[0].destino,
            crate::enlaces::Destino::Web("https://example.org".into())
        );
        // Casos negativos: una hoja que no existe sale vacia; un no-PDF, error.
        assert_eq!(hoja_con_enlaces(&b, 3).unwrap(), Default::default());
        assert_eq!(hoja_con_enlaces(b"nada", 0).unwrap_err(), SinTexto::NoEsPdf);
    }

    #[test]
    fn una_hoja_sin_texto_sale_vacia_y_no_falta() {
        let b = pdf(&[
            "<< /Type /Catalog /Pages 2 0 R >>".into(),
            "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>".into(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] >>".into(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Resources << /Font << /F1 6 0 R >> >> /Contents 5 0 R >>".into(),
            flujo("BT /F1 10 Tf (dos) Tj ET"),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),
        ]);
        let p = de_bytes(&b).unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].texto, "");
        assert_eq!(p[1].texto, "dos");
    }

    /// Con un PDF de verdad: `PIXPIN_PDF_DE_PRUEBA=<ruta>`. Sin la variable
    /// no hace nada (el repositorio no lleva PDF ajenos).
    #[test]
    fn un_pdf_de_verdad_se_lee_y_se_mide() {
        let Ok(ruta) = std::env::var("PIXPIN_PDF_DE_PRUEBA") else {
            return;
        };
        let t0 = std::time::Instant::now();
        let p = de_fichero(std::path::Path::new(&ruta)).unwrap();
        let letras: usize = p.iter().map(|x| x.texto.chars().count()).sum();
        eprintln!(
            "{} hojas, {letras} letras, {} ms",
            p.len(),
            t0.elapsed().as_millis()
        );
        eprintln!(
            "hoja 1: {}",
            p[0].texto.chars().take(400).collect::<String>()
        );
    }

    fn hoja_con(texto: &str) -> pixpin_motor2d::exportar::Hoja {
        pixpin_motor2d::exportar::Hoja {
            nombre: String::new(),
            caja: (0.0, 0.0, 1400.0, 1980.0),
            ordenes: vec![pixpin_motor2d::pintado::Orden::Texto {
                texto: texto.into(),
                x: 100.0,
                y: 200.0,
                tam: 40.0,
                familia: String::new(),
                color: pixpin_motor2d::ColorRgba::opaco(0.0, 0.0, 0.0),
                ancho_max: 1200.0,
                negrita: false,
                cursiva: false,
            }],
            marcos: Vec::new(),
            granos: Vec::new(),
            grafitos: Vec::new(),
        }
    }

    #[test]
    fn lo_que_escribe_este_mismo_crate_se_vuelve_a_leer() {
        // Con Helvetica (un byte, WinAnsi) y con la letra de la pantalla
        // incrustada (dos bytes con su ToUnicode): los dos caminos del PDF
        // que sale de compartir.
        let hojas = [hoja_con("El árbol de la ciencia"), hoja_con("Segunda hoja")];
        let b = crate::escribir::de_hojas(&hojas, None, &|_| None).unwrap();
        let p = de_bytes(&b).unwrap();
        assert_eq!(p.len(), 2);
        assert!(
            p[0].texto.contains("El árbol de la ciencia"),
            "{:?}",
            p[0].texto
        );
        assert!(p[1].texto.contains("Segunda hoja"));
        // La primera letra cae a la izquierda y arriba, donde se escribio.
        let c = p[0].cajas.iter().flatten().next().unwrap();
        assert!(c[0] > 0.05 && c[0] < 0.2 && c[1] < 0.2, "{c:?}");

        if let Some(segoe) = crate::letra::del_sistema("Segoe UI") {
            let b =
                crate::escribir::de_hojas_con_letra(&hojas, None, &|_| None, Some(&segoe)).unwrap();
            let p = de_bytes(&b).unwrap();
            assert!(
                p[0].texto.contains("El árbol de la ciencia"),
                "{:?}",
                p[0].texto
            );
        }
    }
}
