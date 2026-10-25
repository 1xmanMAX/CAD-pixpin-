//! **PDFIMPORT**: el contenido de una pagina de PDF pasado a lo comun
//! ([`Importado`]): los caminos (`m l c v y h re`) trazados o rellenos con su
//! color y grosor, los textos que se pueden leer (fuentes simples con su
//! codificacion, o cualquiera con `/ToUnicode`) y los grupos de contenido
//! opcional (las capas del PDF). Entra en los formularios (`Do`) y salta
//! las imagenes. Con topes: un PDF enorme se corta y se avisa, no se cuelga.

use std::collections::HashMap;
use std::rc::Rc;

use super::objetos::{Dic, Documento, Lexico, Obj, Tok, buscar, es_blanco};
use crate::formatos::{Figura, Geometria, Importado, Rgb, Rotulo, bezier};
use crate::geo::{self, P};

/// Milimetros por punto de PDF.
pub const MM_POR_PT: f64 = 25.4 / 72.0;

/// **Hasta donde se importa.**
#[derive(Debug, Clone, Copy)]
pub struct Topes {
    /// Operadores del contenido (contando los de los formularios).
    pub operadores: usize,
    /// Caminos y textos que salen.
    pub piezas: usize,
}

impl Default for Topes {
    fn default() -> Self {
        Topes {
            operadores: 5_000_000,
            piezas: 200_000,
        }
    }
}

/// **Abre un PDF** del disco.
pub fn abrir(ruta: &std::path::Path) -> Result<Documento, String> {
    let meta = std::fs::metadata(ruta).map_err(|e| format!("no se puede abrir: {e}"))?;
    if meta.len() > 1 << 30 {
        return Err("el PDF pasa de 1 GB".into());
    }
    let d = std::fs::read(ruta).map_err(|e| format!("no se puede leer: {e}"))?;
    Documento::leer(&d)
}

/// **Importa la pagina** `pagina` (desde 1) del PDF.
pub fn importar(doc: &Documento, pagina: usize, topes: Topes) -> Result<Importado, String> {
    let paginas = doc.paginas();
    if paginas.is_empty() {
        return Err("el PDF no tiene paginas".into());
    }
    let Some(p) = pagina.checked_sub(1).and_then(|i| paginas.get(i)) else {
        return Err(format!(
            "no hay pagina {pagina}: el PDF tiene {}",
            paginas.len()
        ));
    };
    let [x0, y0, x1, y1] = p.caja;
    let (w, h) = (x1 - x0, y1 - y0);
    // De la pagina (puntos) a mm, ya girada como se ve.
    let base: M = match p.giro {
        90 => [0.0, -1.0, 1.0, 0.0, -y0, x1],
        180 => [-1.0, 0.0, 0.0, -1.0, x1, y1],
        270 => [0.0, 1.0, -1.0, 0.0, y1, -x0],
        _ => [1.0, 0.0, 0.0, 1.0, -x0, -y0],
    };
    let base = mul(&base, &[MM_POR_PT, 0.0, 0.0, MM_POR_PT, 0.0, 0.0]);
    let mut it = Interprete::new(doc, topes);
    it.est.ctm = base;
    it.out.pagina = if p.giro == 90 || p.giro == 270 {
        (h * MM_POR_PT, w * MM_POR_PT)
    } else {
        (w * MM_POR_PT, h * MM_POR_PT)
    };
    it.out.apagados = grupos_apagados(doc);
    let contenido = doc.contenido(p)?;
    let recursos = p.recursos.and_then(|r| doc.dic(r));
    it.ejecutar(&contenido, recursos, 0);
    it.terminar();
    Ok(it.out)
}

/// Cuantas paginas tiene.
pub fn paginas(doc: &Documento) -> usize {
    doc.paginas().len()
}

/// Los nombres de los grupos que el PDF trae apagados (`/OCProperties /D /OFF`).
fn grupos_apagados(doc: &Documento) -> Vec<String> {
    let Some(cat) = doc.catalogo() else {
        return Vec::new();
    };
    let Some(props) = doc.dic(cat.get("OCProperties").unwrap_or(&Obj::Nulo)) else {
        return Vec::new();
    };
    let Some(d) = doc.dic(props.get("D").unwrap_or(&Obj::Nulo)) else {
        return Vec::new();
    };
    match doc.en(d, "OFF") {
        Obj::Arr(v) => v.iter().filter_map(|o| nombre_de_grupo(doc, o)).collect(),
        _ => Vec::new(),
    }
}

/// El nombre de un grupo de contenido opcional (o del primero de un OCMD).
fn nombre_de_grupo(doc: &Documento, o: &Obj) -> Option<String> {
    let d = doc.dic(o)?;
    if let Obj::Cadena(n) = doc.en(d, "Name") {
        let s = texto_pdf(n);
        return (!s.trim().is_empty()).then_some(s);
    }
    match doc.en(d, "OCGs") {
        Obj::Arr(v) => v.iter().find_map(|x| nombre_de_grupo(doc, x)),
        Obj::Dic(_) => nombre_de_grupo(doc, d.get("OCGs")?),
        _ => None,
    }
}

/// **Un texto de PDF** (UTF-16 con BOM o PDFDocEncoding).
pub fn texto_pdf(b: &[u8]) -> String {
    if b.len() >= 2 && b[0] == 0xFE && b[1] == 0xFF {
        return utf16(&b[2..]);
    }
    b.iter().map(|c| *c as char).collect()
}

fn utf16(b: &[u8]) -> String {
    let u: Vec<u16> = b
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    char::decode_utf16(u)
        .map(|r| r.unwrap_or('\u{FFFD}'))
        .collect()
}

// ---------------------------------------------------------------- matrices

/// Una matriz de PDF `[a b c d e f]`.
type M = [f64; 6];
const ID: M = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// `a` y luego `b`.
fn mul(a: &M, b: &M) -> M {
    [
        a[0] * b[0] + a[1] * b[2],
        a[0] * b[1] + a[1] * b[3],
        a[2] * b[0] + a[3] * b[2],
        a[2] * b[1] + a[3] * b[3],
        a[4] * b[0] + a[5] * b[2] + b[4],
        a[4] * b[1] + a[5] * b[3] + b[5],
    ]
}

fn aplicar(m: &M, x: f64, y: f64) -> P {
    [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]]
}

fn escala(m: &M) -> f64 {
    (m[0] * m[3] - m[1] * m[2]).abs().sqrt()
}

// ------------------------------------------------------------------ fuentes

/// **Una fuente** lo justo para leer sus textos y avanzar.
#[derive(Debug, Default)]
struct Fuente {
    /// Los codigos de `/ToUnicode`.
    unicode: HashMap<u32, String>,
    /// Los rangos de codigos (desde, hasta, bytes).
    rangos: Vec<(u32, u32, usize)>,
    /// Bytes por codigo si no lo dicen los rangos.
    bytes: usize,
    /// La codificacion de una fuente simple.
    simple: Option<Box<[Option<char>; 256]>>,
    /// Los codigos son Unicode (CMaps `UCS2`/`UTF16`).
    cid_unicode: bool,
    anchos: HashMap<u32, f64>,
    defecto: f64,
    /// De unidades del glifo a unidades de texto (0,001 o la `/FontMatrix`).
    escala: f64,
    /// La altura de las mayusculas, en ems.
    mayus: f64,
}

impl Fuente {
    /// Parte los bytes en codigos.
    fn codigos(&self, b: &[u8]) -> Vec<u32> {
        let mut v = Vec::with_capacity(b.len());
        let mut i = 0;
        while i < b.len() {
            let mut n = self.bytes.max(1);
            for &(lo, hi, nb) in &self.rangos {
                if i + nb <= b.len() {
                    let c = b[i..i + nb].iter().fold(0u32, |a, x| a << 8 | *x as u32);
                    if c >= lo && c <= hi {
                        n = nb;
                        break;
                    }
                }
            }
            let fin = (i + n).min(b.len());
            v.push(b[i..fin].iter().fold(0u32, |a, x| a << 8 | *x as u32));
            i = fin;
        }
        v
    }

    /// El texto de un codigo (`None` si no se sabe).
    fn texto(&self, c: u32) -> Option<String> {
        if let Some(s) = self.unicode.get(&c) {
            return Some(s.clone());
        }
        if self.cid_unicode {
            return char::from_u32(c).map(String::from);
        }
        let s = self.simple.as_ref()?;
        s.get(c as usize).copied().flatten().map(String::from)
    }

    fn ancho(&self, c: u32) -> f64 {
        self.anchos.get(&c).copied().unwrap_or(self.defecto) * self.escala
    }
}

/// Lo que cambia de WinAnsi a Latin-1 (0x80 a 0x9F).
pub const WIN_80: &str = "€\u{81}‚ƒ„…†‡ˆ‰Š‹Œ\u{8D}Ž\u{8F}\u{90}‘’“”•–—˜™š›œ\u{9D}žŸ";
/// MacRoman de 0x80 a 0xFF.
const MAC_80: &str = "ÄÅÇÉÑÖÜáàâäãåçéèêëíìîïñóòôöõúùûü†°¢£§•¶ß®©™´¨≠ÆØ∞±≤≥¥µ∂∑∏π∫ªºΩæø¿¡¬√ƒ≈∆«»…\u{A0}ÀÃÕŒœ–—“”‘’÷◊ÿŸ⁄€‹›ﬁﬂ‡·‚„‰ÂÊÁËÈÍÎÏÌÓÔ\u{F8FF}ÒÚÛÙıˆ˜¯˘˙˚¸˝˛ˇ";

fn tabla(nombre: &str) -> Box<[Option<char>; 256]> {
    let mut t: Box<[Option<char>; 256]> = Box::new([None; 256]);
    for (i, c) in t.iter_mut().enumerate() {
        *c = char::from_u32(i as u32).filter(|c| !c.is_control());
    }
    match nombre {
        "WinAnsiEncoding" => {
            for (i, c) in WIN_80.chars().enumerate() {
                t[0x80 + i] = Some(c);
            }
        }
        "MacRomanEncoding" => {
            for (i, c) in MAC_80.chars().enumerate() {
                t[0x80 + i] = Some(c);
            }
        }
        _ => {
            // StandardEncoding: lo de ASCII salvo las comillas.
            t[0x27] = Some('’');
            t[0x60] = Some('‘');
        }
    }
    t
}

/// **El caracter de un nombre de glifo** (`aacute`, `uni00F1`, `zero`…).
pub fn glifo(n: &str) -> Option<char> {
    if n.chars().count() == 1 {
        return n.chars().next();
    }
    if let Some(h) = n.strip_prefix("uni").filter(|h| h.len() >= 4) {
        return u32::from_str_radix(&h[..4], 16)
            .ok()
            .and_then(char::from_u32);
    }
    if let Some(h) = n.strip_prefix('u').filter(|h| (4..=6).contains(&h.len())) {
        if let Some(c) = u32::from_str_radix(h, 16).ok().and_then(char::from_u32) {
            return Some(c);
        }
    }
    const DIGITOS: [&str; 10] = [
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
    ];
    if let Some(i) = DIGITOS.iter().position(|d| *d == n) {
        return char::from_digit(i as u32, 10);
    }
    const NOMBRES: &[(&str, char)] = &[
        ("space", ' '),
        ("exclam", '!'),
        ("quotedbl", '"'),
        ("numbersign", '#'),
        ("dollar", '$'),
        ("percent", '%'),
        ("ampersand", '&'),
        ("quotesingle", '\''),
        ("quoteright", '’'),
        ("quoteleft", '‘'),
        ("parenleft", '('),
        ("parenright", ')'),
        ("asterisk", '*'),
        ("plus", '+'),
        ("comma", ','),
        ("hyphen", '-'),
        ("minus", '−'),
        ("period", '.'),
        ("slash", '/'),
        ("colon", ':'),
        ("semicolon", ';'),
        ("less", '<'),
        ("equal", '='),
        ("greater", '>'),
        ("question", '?'),
        ("at", '@'),
        ("bracketleft", '['),
        ("backslash", '\\'),
        ("bracketright", ']'),
        ("asciicircum", '^'),
        ("underscore", '_'),
        ("grave", '`'),
        ("braceleft", '{'),
        ("bar", '|'),
        ("braceright", '}'),
        ("asciitilde", '~'),
        ("degree", '°'),
        ("plusminus", '±'),
        ("multiply", '×'),
        ("divide", '÷'),
        ("Oslash", 'Ø'),
        ("oslash", 'ø'),
        ("endash", '–'),
        ("emdash", '—'),
        ("bullet", '•'),
        ("quotedblleft", '“'),
        ("quotedblright", '”'),
        ("ellipsis", '…'),
        ("twosuperior", '²'),
        ("threesuperior", '³'),
        ("onesuperior", '¹'),
        ("mu", 'µ'),
        ("periodcentered", '·'),
        ("ordfeminine", 'ª'),
        ("ordmasculine", 'º'),
        ("exclamdown", '¡'),
        ("questiondown", '¿'),
        ("section", '§'),
        ("paragraph", '¶'),
        ("copyright", '©'),
        ("registered", '®'),
        ("trademark", '™'),
        ("Euro", '€'),
        ("cent", '¢'),
        ("sterling", '£'),
        ("yen", '¥'),
        ("onehalf", '½'),
        ("onequarter", '¼'),
        ("threequarters", '¾'),
        ("germandbls", 'ß'),
        ("fi", 'ﬁ'),
        ("fl", 'ﬂ'),
        ("nbspace", '\u{A0}'),
        ("guillemotleft", '«'),
        ("guillemotright", '»'),
    ];
    if let Some((_, c)) = NOMBRES.iter().find(|(k, _)| *k == n) {
        return Some(*c);
    }
    // Letra con acento: `aacute`, `Ntilde`, `udieresis`…
    let mut cs = n.chars();
    let letra = cs.next()?;
    let resto: String = cs.collect();
    const ACENTOS: &[(&str, &str, &str)] = &[
        ("acute", "aeiouyAEIOUY", "áéíóúýÁÉÍÓÚÝ"),
        ("grave", "aeiouAEIOU", "àèìòùÀÈÌÒÙ"),
        ("circumflex", "aeiouAEIOU", "âêîôûÂÊÎÔÛ"),
        ("dieresis", "aeiouyAEIOUY", "äëïöüÿÄËÏÖÜŸ"),
        ("tilde", "anoANO", "ãñõÃÑÕ"),
        ("cedilla", "cC", "çÇ"),
        ("ring", "aA", "åÅ"),
    ];
    for (nombre, bases, con) in ACENTOS {
        if resto == *nombre
            && let Some(i) = bases.chars().position(|b| b == letra)
        {
            return con.chars().nth(i);
        }
    }
    None
}

/// Lee un CMap `/ToUnicode` (rangos de codigos, `bfchar` y `bfrange`).
fn leer_cmap(datos: &[u8], f: &mut Fuente) {
    let mut lx = Lexico::new(datos);
    let cadena = |o: Option<Tok>| match o {
        Some(Tok::Cadena(c)) => Some(c),
        _ => None,
    };
    let num = |c: &[u8]| c.iter().fold(0u32, |a, x| a << 8 | *x as u32);
    while let Some(t) = lx.siguiente() {
        match t {
            Tok::Palabra(b"begincodespacerange") => {
                while let Some(lo) = cadena(lx.siguiente()) {
                    let Some(hi) = cadena(lx.siguiente()) else {
                        break;
                    };
                    f.rangos.push((num(&lo), num(&hi), lo.len().max(1)));
                }
            }
            Tok::Palabra(b"beginbfchar") => {
                while let Some(src) = cadena(lx.siguiente()) {
                    match lx.siguiente() {
                        Some(Tok::Cadena(dst)) => {
                            f.unicode.insert(num(&src), utf16(&dst));
                        }
                        Some(Tok::Nombre(n)) => {
                            if let Some(c) = glifo(&n) {
                                f.unicode.insert(num(&src), c.to_string());
                            }
                        }
                        _ => break,
                    }
                }
            }
            Tok::Palabra(b"beginbfrange") => {
                while let Some(lo) = cadena(lx.siguiente()) {
                    let Some(hi) = cadena(lx.siguiente()) else {
                        break;
                    };
                    let (lo, hi) = (num(&lo), num(&hi));
                    if hi < lo || hi - lo > 65_535 {
                        let _ = super::objetos::objeto(&mut lx, 0);
                        continue;
                    }
                    match super::objetos::objeto(&mut lx, 0) {
                        Some(Obj::Cadena(dst)) => {
                            let mut u: Vec<u16> = dst
                                .chunks_exact(2)
                                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                                .collect();
                            if u.is_empty() {
                                continue;
                            }
                            for c in lo..=hi {
                                f.unicode.insert(
                                    c,
                                    char::decode_utf16(u.iter().copied())
                                        .map(|r| r.unwrap_or('\u{FFFD}'))
                                        .collect(),
                                );
                                let k = u.len() - 1;
                                u[k] = u[k].wrapping_add(1);
                            }
                        }
                        Some(Obj::Arr(v)) => {
                            for (c, o) in (lo..=hi).zip(v) {
                                if let Obj::Cadena(dst) = o {
                                    f.unicode.insert(c, utf16(&dst));
                                }
                            }
                        }
                        _ => break,
                    }
                }
            }
            _ => {}
        }
    }
}

/// **Carga una fuente** de su diccionario.
fn cargar_fuente(doc: &Documento, d: &Dic) -> Fuente {
    let subtipo = doc.en(d, "Subtype").nombre().unwrap_or("").to_string();
    let base = doc.en(d, "BaseFont").nombre().unwrap_or("").to_string();
    let mut f = Fuente {
        bytes: 1,
        defecto: if base.contains("Courier") {
            600.0
        } else {
            500.0
        },
        escala: 0.001,
        mayus: 0.7,
        ..Default::default()
    };
    let mut descriptor = doc.dic(d.get("FontDescriptor").unwrap_or(&Obj::Nulo));
    if subtipo == "Type0" {
        f.bytes = 2;
        if let Some(cmap) = doc.en(d, "Encoding").nombre()
            && (cmap.contains("UCS2") || cmap.contains("UTF16"))
        {
            f.cid_unicode = true;
        }
        if let Obj::Arr(v) = doc.en(d, "DescendantFonts")
            && let Some(cid) = v.first().and_then(|o| doc.dic(o))
        {
            f.defecto = doc.num(cid, "DW").unwrap_or(1000.0);
            descriptor = doc.dic(cid.get("FontDescriptor").unwrap_or(&Obj::Nulo));
            if let Obj::Arr(w) = doc.en(cid, "W") {
                let mut i = 0;
                while i < w.len() {
                    let Some(c0) = doc.resolver(&w[i]).num() else {
                        break;
                    };
                    match w.get(i + 1).map(|o| doc.resolver(o)) {
                        Some(Obj::Arr(lista)) => {
                            for (k, x) in lista.iter().enumerate() {
                                if let Some(a) = doc.resolver(x).num() {
                                    f.anchos.insert(c0 as u32 + k as u32, a);
                                }
                            }
                            i += 2;
                        }
                        Some(Obj::Num(c1)) => {
                            let a = w.get(i + 2).and_then(|o| doc.resolver(o).num());
                            if let Some(a) = a
                                && *c1 >= c0
                                && *c1 - c0 < 65_536.0
                            {
                                for c in c0 as u32..=*c1 as u32 {
                                    f.anchos.insert(c, a);
                                }
                            }
                            i += 3;
                        }
                        _ => break,
                    }
                }
            }
        }
    } else {
        // Una fuente simple: su codificacion y sus anchos.
        let por_defecto = if subtipo == "TrueType" {
            "WinAnsiEncoding"
        } else {
            "StandardEncoding"
        };
        let mut t = match doc.en(d, "Encoding") {
            Obj::Nombre(n) => tabla(n),
            Obj::Dic(e) => {
                let base = doc.en(e, "BaseEncoding").nombre().unwrap_or(por_defecto);
                let mut t = tabla(base);
                if let Obj::Arr(dif) = doc.en(e, "Differences") {
                    let mut c = 0usize;
                    for o in dif {
                        match doc.resolver(o) {
                            Obj::Num(n) => c = *n as usize,
                            Obj::Nombre(g) => {
                                if c < 256 {
                                    t[c] = glifo(g);
                                }
                                c += 1;
                            }
                            _ => {}
                        }
                    }
                }
                t
            }
            _ => tabla(por_defecto),
        };
        if base.contains("Symbol") || base.contains("Dingbats") {
            // Sin tabla propia: no se puede saber que letra es.
            t = Box::new([None; 256]);
        }
        f.simple = Some(t);
        let primero = doc.num(d, "FirstChar").unwrap_or(0.0) as u32;
        if let Obj::Arr(w) = doc.en(d, "Widths") {
            for (k, x) in w.iter().enumerate() {
                if let Some(a) = doc.resolver(x).num() {
                    f.anchos.insert(primero + k as u32, a);
                }
            }
        }
        if subtipo == "Type3" {
            let m = doc.numeros(d.get("FontMatrix").unwrap_or(&Obj::Nulo));
            if m.len() == 6 && m[0].abs() > 1e-9 {
                f.escala = m[0];
                f.mayus = 0.7 * (m[3] / 0.001).abs().clamp(0.01, 100.0);
            }
        }
    }
    if let Some(desc) = descriptor
        && let Some(c) = doc.num(desc, "CapHeight")
        && (300.0..=1000.0).contains(&c)
        && subtipo != "Type3"
    {
        f.mayus = c / 1000.0;
    }
    if let Obj::Flujo(fl) = doc.en(d, "ToUnicode")
        && let Ok(datos) = doc.decodificar(fl)
    {
        leer_cmap(&datos, &mut f);
        f.rangos.sort_by_key(|r| r.2);
    }
    f
}

// --------------------------------------------------------------- interprete

#[derive(Debug, Clone, Copy, PartialEq)]
enum Espacio {
    Gris,
    Rgb,
    Cmyk,
    /// Separation o DeviceN: la tinta (1 es el color lleno).
    Tinta,
    /// Indexed, Pattern…: negro.
    Otro,
}

#[derive(Clone)]
struct Estado {
    ctm: M,
    trazo: Rgb,
    relleno: Rgb,
    esp_trazo: Espacio,
    esp_relleno: Espacio,
    grosor: f64,
    fuente: Option<Rc<Fuente>>,
    tam: f64,
    tc: f64,
    tw: f64,
    th: f64,
    tl: f64,
    ts: f64,
    modo: i32,
}

impl Default for Estado {
    fn default() -> Self {
        Estado {
            ctm: ID,
            trazo: [0, 0, 0],
            relleno: [0, 0, 0],
            esp_trazo: Espacio::Gris,
            esp_relleno: Espacio::Gris,
            grosor: 1.0,
            fuente: None,
            tam: 0.0,
            tc: 0.0,
            tw: 0.0,
            th: 1.0,
            tl: 0.0,
            ts: 0.0,
            modo: 0,
        }
    }
}

fn a_byte(v: f64) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn color_de(esp: Espacio, v: &[f64]) -> Rgb {
    match (esp, v.len()) {
        (Espacio::Tinta, 1) => {
            let g = a_byte(1.0 - v[0]);
            [g, g, g]
        }
        (Espacio::Otro, _) => [0, 0, 0],
        (_, 1) => {
            let g = a_byte(v[0]);
            [g, g, g]
        }
        (_, 3) => [a_byte(v[0]), a_byte(v[1]), a_byte(v[2])],
        (_, 4) => {
            let k = v[3];
            [
                a_byte((1.0 - v[0]) * (1.0 - k)),
                a_byte((1.0 - v[1]) * (1.0 - k)),
                a_byte((1.0 - v[2]) * (1.0 - k)),
            ]
        }
        _ => [0, 0, 0],
    }
}

/// El ultimo texto puesto, para juntar con el siguiente si sigue la linea.
struct UltimoTexto {
    indice: usize,
    fin: P,
    tam: f64,
    giro: f64,
}

struct Interprete<'a> {
    doc: &'a Documento,
    out: Importado,
    est: Estado,
    pila: Vec<Estado>,
    caminos: Vec<(Vec<P>, bool)>,
    actual: Vec<P>,
    tm: M,
    tlm: M,
    oc: Vec<Option<String>>,
    topes: Topes,
    operadores: usize,
    cortado: bool,
    imagenes: usize,
    textos_mal: usize,
    textos_ocultos: usize,
    blancos: usize,
    degradados: usize,
    fuentes: HashMap<usize, Rc<Fuente>>,
    en_curso: Vec<usize>,
    ultimo: Option<UltimoTexto>,
}

impl<'a> Interprete<'a> {
    fn new(doc: &'a Documento, topes: Topes) -> Interprete<'a> {
        Interprete {
            doc,
            out: Importado::default(),
            est: Estado::default(),
            pila: Vec::new(),
            caminos: Vec::new(),
            actual: Vec::new(),
            tm: ID,
            tlm: ID,
            oc: Vec::new(),
            topes,
            operadores: 0,
            cortado: false,
            imagenes: 0,
            textos_mal: 0,
            textos_ocultos: 0,
            blancos: 0,
            degradados: 0,
            fuentes: HashMap::new(),
            en_curso: Vec::new(),
            ultimo: None,
        }
    }

    fn grupo(&self) -> Option<String> {
        self.oc.iter().rev().find_map(|g| g.clone())
    }

    fn piezas(&self) -> usize {
        self.out.figuras.len() + self.out.rotulos.len()
    }

    fn terminar(&mut self) {
        let avisos = [
            (self.imagenes, "imagen(es) no importada(s)"),
            (
                self.textos_mal,
                "texto(s) con fuentes que no se pueden leer",
            ),
            (self.textos_ocultos, "texto(s) invisibles (de OCR) omitidos"),
            (self.blancos, "relleno(s) blancos omitidos"),
            (self.degradados, "degradado(s) no importado(s)"),
        ];
        for (n, que) in avisos {
            if n > 0 {
                self.out.avisos.push(format!("{n} {que}"));
            }
        }
        if self.cortado {
            self.out.avisos.push(format!(
                "la pagina es muy grande: se importo hasta {} piezas o {} operadores",
                self.topes.piezas, self.topes.operadores
            ));
        }
    }

    /// Un recurso por categoria y nombre (`/Font /F1`).
    fn recurso(&self, recursos: Option<&'a Dic>, categoria: &str, nombre: &str) -> &'a Obj {
        let doc = self.doc;
        recursos
            .and_then(|r| doc.dic(r.get(categoria).unwrap_or(&Obj::Nulo)))
            .map_or(&Obj::Nulo, |c| doc.en(c, nombre))
    }

    fn fuente(&mut self, recursos: Option<&'a Dic>, nombre: &str) -> Option<Rc<Fuente>> {
        let o = self.recurso(recursos, "Font", nombre);
        let d = self.doc.dic(o)?;
        let llave = d as *const Dic as usize;
        if let Some(f) = self.fuentes.get(&llave) {
            return Some(f.clone());
        }
        let f = Rc::new(cargar_fuente(self.doc, d));
        self.fuentes.insert(llave, f.clone());
        Some(f)
    }

    fn espacio(&self, recursos: Option<&'a Dic>, nombre: &str) -> Espacio {
        match nombre {
            "DeviceGray" | "G" | "CalGray" => return Espacio::Gris,
            "DeviceRGB" | "RGB" | "CalRGB" => return Espacio::Rgb,
            "DeviceCMYK" | "CMYK" => return Espacio::Cmyk,
            "Pattern" => return Espacio::Otro,
            _ => {}
        }
        let o = self.recurso(recursos, "ColorSpace", nombre);
        match o {
            Obj::Arr(v) => match v.first().and_then(|x| self.doc.resolver(x).nombre()) {
                Some("ICCBased") => {
                    let n = v
                        .get(1)
                        .and_then(|x| self.doc.dic(x))
                        .and_then(|d| self.doc.num(d, "N"))
                        .unwrap_or(3.0);
                    match n as i32 {
                        1 => Espacio::Gris,
                        4 => Espacio::Cmyk,
                        _ => Espacio::Rgb,
                    }
                }
                Some("Separation") | Some("DeviceN") => Espacio::Tinta,
                Some("CalRGB") | Some("Lab") => Espacio::Rgb,
                Some("CalGray") => Espacio::Gris,
                _ => Espacio::Otro,
            },
            Obj::Nombre(n) if n != nombre => self.espacio(None, &n.clone()),
            _ => Espacio::Rgb,
        }
    }

    // ------------------------------------------------------------- caminos

    fn cerrar_sub(&mut self) {
        if !self.actual.is_empty() {
            let v = std::mem::take(&mut self.actual);
            self.caminos.push((v, false));
        }
    }

    fn punto_actual(&self) -> Option<P> {
        self.actual.last().copied()
    }

    fn pintar(&mut self, trazar: bool, rellenar: bool, cerrar: bool) {
        if cerrar && self.actual.len() > 1 {
            self.cerrar_sub();
            if let Some(u) = self.caminos.last_mut() {
                u.1 = true;
            }
        }
        self.cerrar_sub();
        let caminos = std::mem::take(&mut self.caminos);
        if self.cortado || (!trazar && !rellenar) {
            return;
        }
        let partes: Vec<Geometria> = caminos
            .into_iter()
            .filter_map(|(mut puntos, mut cerrada)| {
                crate::formatos::sin_repetidos(&mut puntos, 1e-9);
                if puntos.len() > 2 && geo::dist(puntos[0], puntos[puntos.len() - 1]) < 1e-9 {
                    puntos.pop();
                    cerrada = true;
                }
                (puntos.len() >= 2).then_some(Geometria::Polilinea { puntos, cerrada })
            })
            .collect();
        if partes.is_empty() {
            return;
        }
        let mut relleno = rellenar.then_some(self.est.relleno);
        if relleno == Some([255, 255, 255]) {
            self.blancos += 1;
            relleno = None;
        }
        let trazo = trazar.then(|| (self.est.trazo, self.est.grosor * escala(&self.est.ctm)));
        if trazo.is_none() && relleno.is_none() {
            return;
        }
        self.out.figuras.push(Figura {
            partes,
            trazo,
            relleno,
            grupo: self.grupo(),
        });
        if self.piezas() >= self.topes.piezas {
            self.cortado = true;
        }
    }

    // -------------------------------------------------------------- textos

    fn mostrar(&mut self, b: &[u8]) {
        let Some(f) = self.est.fuente.clone() else {
            return;
        };
        let est = &self.est;
        let trm = mul(
            &mul(
                &[est.tam * est.th, 0.0, 0.0, est.tam, 0.0, est.ts],
                &self.tm,
            ),
            &est.ctm,
        );
        let codigos = f.codigos(b);
        let mut texto = String::new();
        let mut bien = 0;
        let mut avance = 0.0;
        for c in &codigos {
            match f.texto(*c) {
                Some(s) => {
                    bien += 1;
                    texto.push_str(&s);
                }
                None => texto.push('?'),
            }
            let espacio = if f.bytes == 1 && *c == 32 {
                est.tw
            } else {
                0.0
            };
            avance += (f.ancho(*c) * est.tam + est.tc + espacio) * est.th;
        }
        self.tm = mul(&[1.0, 0.0, 0.0, 1.0, avance, 0.0], &self.tm);
        let fin = aplicar(
            &mul(
                &[est.th, 0.0, 0.0, 1.0, 0.0, est.ts],
                &mul(&self.tm, &est.ctm),
            ),
            0.0,
            0.0,
        );
        if codigos.is_empty() || texto.trim().is_empty() {
            return;
        }
        if bien * 2 < codigos.len() {
            self.textos_mal += 1;
            return;
        }
        if matches!(est.modo, 3 | 7) {
            self.textos_ocultos += 1;
            return;
        }
        if self.cortado {
            return;
        }
        let origen = [trm[4], trm[5]];
        let tam = geo::norma([trm[2], trm[3]]);
        let ancho_x = geo::norma([trm[0], trm[1]]);
        let giro = trm[1].atan2(trm[0]);
        if tam < 1e-9 {
            return;
        }
        let color = if matches!(est.modo, 1 | 5) {
            est.trazo
        } else {
            est.relleno
        };
        // ¿Sigue la linea del texto anterior? Entonces se junta.
        let grupo = self.grupo();
        if let Some(u) = &self.ultimo
            && (u.tam - tam).abs() < tam * 0.01
            && (u.giro - giro).abs() < 1e-3
            && let Some(r) = self.out.rotulos.get_mut(u.indice)
            && r.color == color
            && r.grupo == grupo
        {
            let d = geo::restar(origen, u.fin);
            let dir = [giro.cos(), giro.sin()];
            let a_lo_largo = geo::punto(d, dir);
            let de_lado = geo::cruz(dir, d).abs();
            if de_lado < tam * 0.2 && a_lo_largo > -tam * 0.3 && a_lo_largo < tam * 1.5 {
                if a_lo_largo > tam * 0.2 && !r.texto.ends_with(' ') && !texto.starts_with(' ') {
                    r.texto.push(' ');
                }
                r.texto.push_str(&texto);
                self.ultimo = Some(UltimoTexto {
                    indice: u.indice,
                    fin,
                    tam,
                    giro,
                });
                return;
            }
        }
        self.out.rotulos.push(Rotulo {
            texto,
            p: origen,
            alto: tam * f.mayus,
            giro,
            ancho: ancho_x / tam,
            color,
            grupo: self.grupo(),
        });
        self.ultimo = Some(UltimoTexto {
            indice: self.out.rotulos.len() - 1,
            fin,
            tam,
            giro,
        });
        if self.piezas() >= self.topes.piezas {
            self.cortado = true;
        }
    }

    fn salto_de_linea(&mut self, tx: f64, ty: f64) {
        self.tlm = mul(&[1.0, 0.0, 0.0, 1.0, tx, ty], &self.tlm);
        self.tm = self.tlm;
    }

    // ---------------------------------------------------------- formularios

    fn dibujar_xobjeto(&mut self, recursos: Option<&'a Dic>, nombre: &str, prof: u32) {
        let o = self.recurso(recursos, "XObject", nombre);
        let Obj::Flujo(f) = o else { return };
        match self.doc.en(&f.dic, "Subtype").nombre() {
            Some("Image") => self.imagenes += 1,
            Some("Form") => {
                let llave = &**f as *const _ as usize;
                if prof > 12 || self.en_curso.contains(&llave) {
                    return;
                }
                let Ok(datos) = self.doc.decodificar(f) else {
                    return;
                };
                let m = self.doc.numeros(f.dic.get("Matrix").unwrap_or(&Obj::Nulo));
                let m: M = if m.len() == 6 {
                    [m[0], m[1], m[2], m[3], m[4], m[5]]
                } else {
                    ID
                };
                let propios = self.doc.dic(f.dic.get("Resources").unwrap_or(&Obj::Nulo));
                let grupo = f.dic.get("OC").and_then(|g| nombre_de_grupo(self.doc, g));
                let guardado = self.est.clone();
                let (tm, tlm) = (self.tm, self.tlm);
                self.est.ctm = mul(&m, &self.est.ctm);
                self.oc.push(grupo);
                let nivel_oc = self.oc.len();
                self.en_curso.push(llave);
                self.ejecutar(&datos, propios.or(recursos), prof + 1);
                self.en_curso.pop();
                self.oc.truncate(nivel_oc - 1);
                self.est = guardado;
                self.tm = tm;
                self.tlm = tlm;
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------- bucle

    /// **Ejecuta un flujo de contenido** con sus recursos.
    fn ejecutar(&mut self, datos: &[u8], recursos: Option<&'a Dic>, prof: u32) {
        let mut lx = Lexico::new(datos);
        let mut ops: Vec<Obj> = Vec::new();
        while let Some(t) = lx.siguiente() {
            if self.cortado {
                return;
            }
            let op = match t {
                Tok::Palabra(p) => p,
                otro => {
                    if let Some(o) = super::objetos::objeto_desde(&mut lx, otro, 0) {
                        if ops.len() < 64 {
                            ops.push(o);
                        }
                    }
                    continue;
                }
            };
            self.operadores += 1;
            if self.operadores > self.topes.operadores {
                self.cortado = true;
                return;
            }
            let n = |i: usize| ops.get(i).and_then(Obj::num).unwrap_or(0.0);
            let todos = |ops: &[Obj]| -> Vec<f64> { ops.iter().filter_map(Obj::num).collect() };
            match op {
                b"q" => {
                    if self.pila.len() < 256 {
                        self.pila.push(self.est.clone());
                    }
                }
                b"Q" => {
                    if let Some(e) = self.pila.pop() {
                        self.est = e;
                    }
                }
                b"cm" if ops.len() >= 6 => {
                    let m = [n(0), n(1), n(2), n(3), n(4), n(5)];
                    self.est.ctm = mul(&m, &self.est.ctm);
                }
                b"w" => self.est.grosor = n(0).abs(),
                b"gs" => {
                    if let Some(nombre) = ops.first().and_then(Obj::nombre)
                        && let Some(g) = self.doc.dic(self.recurso(recursos, "ExtGState", nombre))
                        && let Some(lw) = self.doc.num(g, "LW")
                    {
                        self.est.grosor = lw.abs();
                    }
                }
                b"RG" => {
                    self.est.esp_trazo = Espacio::Rgb;
                    self.est.trazo = color_de(Espacio::Rgb, &todos(&ops));
                }
                b"rg" => {
                    self.est.esp_relleno = Espacio::Rgb;
                    self.est.relleno = color_de(Espacio::Rgb, &todos(&ops));
                }
                b"G" => {
                    self.est.esp_trazo = Espacio::Gris;
                    self.est.trazo = color_de(Espacio::Gris, &todos(&ops));
                }
                b"g" => {
                    self.est.esp_relleno = Espacio::Gris;
                    self.est.relleno = color_de(Espacio::Gris, &todos(&ops));
                }
                b"K" => {
                    self.est.esp_trazo = Espacio::Cmyk;
                    self.est.trazo = color_de(Espacio::Cmyk, &todos(&ops));
                }
                b"k" => {
                    self.est.esp_relleno = Espacio::Cmyk;
                    self.est.relleno = color_de(Espacio::Cmyk, &todos(&ops));
                }
                b"CS" | b"cs" => {
                    let e = ops
                        .first()
                        .and_then(Obj::nombre)
                        .map_or(Espacio::Gris, |nm| self.espacio(recursos, nm));
                    if op == b"CS" {
                        self.est.esp_trazo = e;
                        self.est.trazo = [0, 0, 0];
                    } else {
                        self.est.esp_relleno = e;
                        self.est.relleno = [0, 0, 0];
                    }
                }
                b"SC" | b"SCN" => self.est.trazo = color_de(self.est.esp_trazo, &todos(&ops)),
                b"sc" | b"scn" => {
                    self.est.relleno = color_de(self.est.esp_relleno, &todos(&ops));
                }
                b"m" if ops.len() >= 2 => {
                    self.cerrar_sub();
                    self.actual.push(aplicar(&self.est.ctm, n(0), n(1)));
                }
                b"l" if ops.len() >= 2 => {
                    let p = aplicar(&self.est.ctm, n(0), n(1));
                    if self.actual.is_empty() {
                        self.actual.push(p);
                    }
                    self.actual.push(p);
                }
                b"c" | b"v" | b"y" => {
                    let Some(a) = self.punto_actual() else {
                        ops.clear();
                        continue;
                    };
                    let m = self.est.ctm;
                    let (b, c, d) = match op {
                        b"c" if ops.len() >= 6 => (
                            aplicar(&m, n(0), n(1)),
                            aplicar(&m, n(2), n(3)),
                            aplicar(&m, n(4), n(5)),
                        ),
                        b"v" if ops.len() >= 4 => {
                            (a, aplicar(&m, n(0), n(1)), aplicar(&m, n(2), n(3)))
                        }
                        b"y" if ops.len() >= 4 => {
                            let d = aplicar(&m, n(2), n(3));
                            (aplicar(&m, n(0), n(1)), d, d)
                        }
                        _ => {
                            ops.clear();
                            continue;
                        }
                    };
                    self.actual.extend(bezier(a, b, c, d, 0.01));
                }
                b"h" => {
                    // Se cierra y la pluma vuelve al principio del tramo.
                    if self.actual.len() > 1 {
                        let inicio = self.actual[0];
                        self.cerrar_sub();
                        if let Some(u) = self.caminos.last_mut() {
                            u.1 = true;
                        }
                        self.actual.push(inicio);
                    }
                }
                b"re" if ops.len() >= 4 => {
                    self.cerrar_sub();
                    let (x, y, w, h) = (n(0), n(1), n(2), n(3));
                    let m = self.est.ctm;
                    self.caminos.push((
                        vec![
                            aplicar(&m, x, y),
                            aplicar(&m, x + w, y),
                            aplicar(&m, x + w, y + h),
                            aplicar(&m, x, y + h),
                        ],
                        true,
                    ));
                }
                b"S" => self.pintar(true, false, false),
                b"s" => self.pintar(true, false, true),
                b"f" | b"F" | b"f*" => self.pintar(false, true, true),
                b"B" | b"B*" => self.pintar(true, true, false),
                b"b" | b"b*" => self.pintar(true, true, true),
                b"n" => {
                    self.actual.clear();
                    self.caminos.clear();
                }
                b"BT" => {
                    self.tm = ID;
                    self.tlm = ID;
                }
                b"Tf" => {
                    if let Some(nm) = ops.first().and_then(Obj::nombre) {
                        let nm = nm.to_string();
                        self.est.fuente = self.fuente(recursos, &nm);
                    }
                    self.est.tam = n(1);
                }
                b"Tc" => self.est.tc = n(0),
                b"Tw" => self.est.tw = n(0),
                b"Tz" => self.est.th = n(0) / 100.0,
                b"TL" => self.est.tl = n(0),
                b"Ts" => self.est.ts = n(0),
                b"Tr" => self.est.modo = n(0) as i32,
                b"Td" => self.salto_de_linea(n(0), n(1)),
                b"TD" => {
                    self.est.tl = -n(1);
                    self.salto_de_linea(n(0), n(1));
                }
                b"Tm" if ops.len() >= 6 => {
                    self.tlm = [n(0), n(1), n(2), n(3), n(4), n(5)];
                    self.tm = self.tlm;
                }
                b"T*" => self.salto_de_linea(0.0, -self.est.tl),
                b"Tj" => {
                    if let Some(Obj::Cadena(s)) = ops.first() {
                        let s = s.clone();
                        self.mostrar(&s);
                    }
                }
                b"'" => {
                    self.salto_de_linea(0.0, -self.est.tl);
                    if let Some(Obj::Cadena(s)) = ops.first() {
                        let s = s.clone();
                        self.mostrar(&s);
                    }
                }
                b"\"" => {
                    self.est.tw = n(0);
                    self.est.tc = n(1);
                    self.salto_de_linea(0.0, -self.est.tl);
                    if let Some(Obj::Cadena(s)) = ops.get(2) {
                        let s = s.clone();
                        self.mostrar(&s);
                    }
                }
                b"TJ" => {
                    if let Some(Obj::Arr(v)) = ops.first() {
                        let v = v.clone();
                        for o in v {
                            match o {
                                Obj::Cadena(s) => self.mostrar(&s),
                                Obj::Num(k) => {
                                    let tx = -k / 1000.0 * self.est.tam * self.est.th;
                                    self.tm = mul(&[1.0, 0.0, 0.0, 1.0, tx, 0.0], &self.tm);
                                }
                                _ => {}
                            }
                        }
                    }
                }
                b"Do" => {
                    if let Some(nm) = ops.first().and_then(Obj::nombre) {
                        let nm = nm.to_string();
                        self.dibujar_xobjeto(recursos, &nm, prof);
                    }
                }
                b"BDC" => {
                    let es_oc = ops.first().and_then(Obj::nombre) == Some("OC");
                    let g = if es_oc {
                        match ops.get(1) {
                            Some(Obj::Nombre(p)) => {
                                let o = self.recurso(recursos, "Properties", p);
                                nombre_de_grupo(self.doc, o)
                            }
                            Some(o @ Obj::Dic(_)) => nombre_de_grupo(self.doc, o),
                            _ => None,
                        }
                    } else {
                        None
                    };
                    if self.oc.len() < 256 {
                        self.oc.push(g);
                    }
                }
                b"BMC" => {
                    if self.oc.len() < 256 {
                        self.oc.push(None);
                    }
                }
                b"EMC" => {
                    self.oc.pop();
                }
                b"BI" => {
                    // Una imagen en linea: se salta hasta su `EI`.
                    self.imagenes += 1;
                    if let Some(k) = buscar(datos, b"ID", lx.i) {
                        let mut j = k + 2;
                        loop {
                            match buscar(datos, b"EI", j) {
                                Some(e)
                                    if e > 0
                                        && es_blanco(datos[e - 1])
                                        && datos.get(e + 2).is_none_or(|c| es_blanco(*c)) =>
                                {
                                    lx.i = e + 2;
                                    break;
                                }
                                Some(e) => j = e + 2,
                                None => {
                                    lx.i = datos.len();
                                    break;
                                }
                            }
                        }
                    }
                }
                b"sh" => self.degradados += 1,
                _ => {}
            }
            ops.clear();
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn tablas_completas() {
        assert_eq!(WIN_80.chars().count(), 32);
        assert_eq!(MAC_80.chars().count(), 128);
        assert_eq!(glifo("aacute"), Some('á'));
        assert_eq!(glifo("Ntilde"), Some('Ñ'));
        assert_eq!(glifo("uni00F1"), Some('ñ'));
        assert_eq!(glifo("seven"), Some('7'));
        assert_eq!(glifo("nada_raro"), None);
    }

    #[test]
    fn cmap_con_rangos() {
        let cmap = b"1 begincodespacerange <0000> <FFFF> endcodespacerange
            2 beginbfchar <0003> <0020> <0024> <0041> endbfchar
            1 beginbfrange <0044> <0046> <0061> endbfrange";
        let mut f = Fuente {
            bytes: 2,
            ..Default::default()
        };
        leer_cmap(cmap, &mut f);
        let c = f.codigos(&[0, 0x24, 0, 3, 0, 0x45]);
        let s: String = c.iter().filter_map(|c| f.texto(*c)).collect();
        assert_eq!(s, "A b");
    }

    #[test]
    fn matrices() {
        let m = mul(
            &[2.0, 0.0, 0.0, 2.0, 0.0, 0.0],
            &[1.0, 0.0, 0.0, 1.0, 10.0, 0.0],
        );
        assert_eq!(aplicar(&m, 1.0, 1.0), [12.0, 2.0]);
        assert!((escala(&m) - 2.0).abs() < 1e-12);
    }
}
