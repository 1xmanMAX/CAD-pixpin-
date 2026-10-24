//! **El texto rico del lienzo** (15-oct-2026): un texto con tramos de otro
//! color, en negrita, cursiva, subrayados, tachados o de otro tamano, y
//! **formulas en linea** (`$m\sphericalangle AOB = 90^\circ$`) compuestas
//! con el compositor comun (`ecuacion::de_formula`, el de las ecuaciones del
//! asistente y la grafica), alineadas con las letras de su renglon.
//!
//! Lo pidio el usuario al ver una lamina de geometria reconstruida: «Nos
//! piden m∢COD = X» con la X en rojo, «AB» con su raya encima, «mAC» con su
//! arco, «45° = (2a+2b)/2» con la fraccion de verdad... y el texto del
//! lienzo solo sabia un color y una letra. Las formulas eran elementos
//! sueltos que no casaban con el renglon.
//!
//! ## Un solo motor
//!
//! No es un elemento nuevo: es el **mismo** `Figura::Texto`, con su fuente
//! rica en `Extras::rico`. El texto de siempre (`Figura::Texto::texto`) se
//! queda con la version **plana** ([`plano`]: `m∢COD = X`, `A̅B̅`, `AC⌒`,
//! `(2a+2b)/2`), que es lo que lee quien no entiende el formato (el movil
//! de hoy, Excalidraw, la busqueda, la voz). Pintar un texto rico es
//! producir mas `Orden::Texto` y `Orden::Polilinea` de las de siempre: por
//! eso sale igual en pantalla, en el PNG, en el SVG y en el PDF sin tocar a
//! ninguno de los que pintan.
//!
//! ## El formato (ver `docs/texto-rico-del-lienzo.md`)
//!
//! - `$...$` formula en linea (LaTeX de bolsillo de las notas); `$$...$$`
//!   formula alta (sus fracciones no encogen). `\$` es un dolar.
//! - `**negrita**`, `*cursiva*` o `_cursiva_`, `~~tachado~~`, `<u>subrayado</u>`.
//! - `\color{rojo}{...}` o `\textcolor{red}{...}` (nombre o `#rrggbb`),
//!   `\textbf{}`, `\textit{}`, `\underline{}`, `\sout{}`, `\tam{1.5}{}` (en
//!   veces el tamano del texto), `\small{}`, `\large{}`, `\Large{}`, `\huge{}`.
//!   Dentro de una formula vale el mismo `\color{red}{X}`.
//! - Al principio de un renglon: `\centro`, `\derecha`, `\izquierda` o
//!   `\justificado` (o `\center`, `\flushright`, `\flushleft`, `\justify`).
//!
//! Lo que no se entiende **se ve tal cual**: un `$` sin cerrar es un dolar,
//! un mandato desconocido sale con su barra. Nunca falla ni se cuelga.

use crate::ecuacion::{self as caja, Caja, Pieza};
use crate::elemento::{ColorRgba, EstiloTrazo};
use crate::pintado::Orden;
use crate::texto::{self, AlineacionTexto, EstiloDeTexto};
use crate::vector::Punto2;

/// Lo mas largo que se compone una fuente rica: lo de detras se pinta
/// plano. Un texto pegado de un libro entero no puede congelar el lienzo.
pub const MAX_FUENTE: usize = 20_000;

/// Como va escrito un tramo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EstiloTramo {
    /// `None`: el color del elemento.
    pub color: Option<ColorRgba>,
    pub negrita: bool,
    pub cursiva: bool,
    pub subrayado: bool,
    pub tachado: bool,
    /// El tamano en veces el del elemento: asi crece con el.
    pub escala: f32,
}

impl Default for EstiloTramo {
    fn default() -> Self {
        EstiloTramo {
            color: None,
            negrita: false,
            cursiva: false,
            subrayado: false,
            tachado: false,
            escala: 1.0,
        }
    }
}

/// Un trozo de renglon con su estilo.
#[derive(Debug, Clone, PartialEq)]
pub enum Tramo {
    Texto {
        texto: String,
        estilo: EstiloTramo,
    },
    /// Una formula (sin sus dolares). `alta`: entre `$$`.
    Formula {
        fuente: String,
        estilo: EstiloTramo,
        alta: bool,
    },
}

/// A que lado va un renglon. `Justificado` no existe en el fichero de
/// Excalidraw ni en el movil (`textAlign` solo tiene tres palabras): por eso
/// va en la fuente rica, renglon a renglon, y no en el elemento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alineado {
    Izquierda,
    Centro,
    Derecha,
    Justificado,
}

/// Un renglon de la fuente (lo que hay entre dos saltos).
#[derive(Debug, Clone, PartialEq)]
pub struct Renglon {
    /// `None`: el del elemento.
    pub alineado: Option<Alineado>,
    pub tramos: Vec<Tramo>,
}

// ---------------------------------------------------------------------
// Leer
// ---------------------------------------------------------------------

/// **Lee una fuente rica** a sus renglones y tramos. Nunca falla.
pub fn leer(fuente: &str) -> Vec<Renglon> {
    let fuente = recortar(fuente);
    fuente.split('\n').map(leer_renglon).collect()
}

fn recortar(fuente: &str) -> &str {
    match fuente.char_indices().nth(MAX_FUENTE) {
        Some((b, _)) => &fuente[..b],
        None => fuente,
    }
}

const DIRECTIVAS: &[(&str, Alineado)] = &[
    ("\\centro", Alineado::Centro),
    ("\\center", Alineado::Centro),
    ("\\centering", Alineado::Centro),
    ("\\derecha", Alineado::Derecha),
    ("\\flushright", Alineado::Derecha),
    ("\\izquierda", Alineado::Izquierda),
    ("\\flushleft", Alineado::Izquierda),
    ("\\justificado", Alineado::Justificado),
    ("\\justify", Alineado::Justificado),
];

fn leer_renglon(linea: &str) -> Renglon {
    let linea = linea.strip_suffix('\r').unwrap_or(linea);
    let mut alineado = None;
    let mut resto = linea;
    let sin_blancos = linea.trim_start();
    // La mas larga primero (`\centering` antes que `\center`).
    let mut dirs: Vec<&(&str, Alineado)> = DIRECTIVAS.iter().collect();
    dirs.sort_by_key(|(d, _)| std::cmp::Reverse(d.len()));
    for (d, a) in dirs {
        if let Some(r) = sin_blancos.strip_prefix(d)
            && !r.starts_with(|c: char| c.is_alphabetic())
        {
            alineado = Some(*a);
            resto = r.strip_prefix(' ').unwrap_or(r);
            break;
        }
    }
    let c: Vec<char> = resto.chars().collect();
    let mut tramos = Vec::new();
    leer_tramos(&c, EstiloTramo::default(), &mut tramos);
    Renglon {
        alineado,
        tramos: fusionar(tramos),
    }
}

/// Junta los textos seguidos con el mismo estilo.
fn fusionar(v: Vec<Tramo>) -> Vec<Tramo> {
    let mut out: Vec<Tramo> = Vec::with_capacity(v.len());
    for t in v {
        if let (
            Some(Tramo::Texto {
                texto: a,
                estilo: ea,
            }),
            Tramo::Texto {
                texto: b,
                estilo: eb,
            },
        ) = (out.last_mut(), &t)
            && ea == eb
        {
            a.push_str(b);
            continue;
        }
        out.push(t);
    }
    out
}

fn leer_tramos(c: &[char], est: EstiloTramo, out: &mut Vec<Tramo>) {
    let mut buf = String::new();
    let soltar = |buf: &mut String, out: &mut Vec<Tramo>| {
        if !buf.is_empty() {
            out.push(Tramo::Texto {
                texto: std::mem::take(buf),
                estilo: est,
            });
        }
    };
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch == '\\' {
            match c.get(i + 1) {
                // `\$`, `\*`, `\\`, `\{`: el caracter tal cual.
                Some(&d) if !d.is_alphabetic() => {
                    buf.push(d);
                    i += 2;
                    continue;
                }
                Some(_) => {
                    if let Some((fin, sub, desde, hasta)) = mandato(c, i, est) {
                        soltar(&mut buf, out);
                        leer_tramos(&c[desde..hasta], sub, out);
                        i = fin;
                        continue;
                    }
                }
                None => {}
            }
            buf.push('\\');
            i += 1;
            continue;
        }
        if ch == '$' {
            if let Some((fin, fuente, alta)) = formula(c, i) {
                soltar(&mut buf, out);
                out.push(Tramo::Formula {
                    fuente,
                    estilo: est,
                    alta,
                });
                i = fin;
                continue;
            }
            buf.push('$');
            i += 1;
            continue;
        }
        if let Some((abre, cierra, cambio)) = marca_en(c, i)
            && let Some(j) = cierre(c, i + abre, cierra)
        {
            soltar(&mut buf, out);
            let mut sub = est;
            cambio(&mut sub);
            leer_tramos(&c[i + abre..j], sub, out);
            i = j + cierra.chars().count();
            continue;
        }
        buf.push(ch);
        i += 1;
    }
    soltar(&mut buf, out);
}

type Cambio = fn(&mut EstiloTramo);

/// Si en `i` empieza una marca de las de Markdown que abre de verdad: su
/// largo, lo que la cierra y lo que cambia.
fn marca_en(c: &[char], i: usize) -> Option<(usize, &'static str, Cambio)> {
    let empieza = |s: &str| {
        s.chars()
            .enumerate()
            .all(|(k, ch)| c.get(i + k) == Some(&ch))
    };
    let detras_no_blanco = |k: usize| c.get(k).is_some_and(|x| !x.is_whitespace());
    if empieza("<u>") {
        return Some((3, "</u>", |e| e.subrayado = true));
    }
    if empieza("**") && detras_no_blanco(i + 2) {
        return Some((2, "**", |e| e.negrita = true));
    }
    if empieza("~~") && detras_no_blanco(i + 2) {
        return Some((2, "~~", |e| e.tachado = true));
    }
    // `*` y `_` solos: como en CommonMark, no abren pegados a una letra
    // por delante (`2*3*4` y `nombre_de_fichero` siguen siendo eso).
    let ch = c[i];
    if (ch == '*' || ch == '_')
        && c.get(i + 1) != Some(&ch)
        && detras_no_blanco(i + 1)
        && (i == 0 || !c[i - 1].is_alphanumeric())
    {
        return Some((1, if ch == '*' { "*" } else { "_" }, |e| e.cursiva = true));
    }
    None
}

/// Donde se cierra una marca abierta antes de `desde`: lo que la cierra
/// pegado a algo escrito (no a un blanco) y, para `*` y `_`, sin una letra
/// detras. Se saltan los escapes y las formulas: un `*` de una formula no
/// cierra nada.
fn cierre(c: &[char], desde: usize, marca: &str) -> Option<usize> {
    let m: Vec<char> = marca.chars().collect();
    let mut k = desde;
    while k < c.len() {
        if c[k] == '\\' {
            k += 2;
            continue;
        }
        if c[k] == '$'
            && let Some((fin, _, _)) = formula(c, k)
        {
            k = fin;
            continue;
        }
        if c[k..].starts_with(&m) && k > desde {
            let antes_ok = marca.starts_with('<') || !c[k - 1].is_whitespace();
            let simple = m.len() == 1;
            let despues_ok = !simple
                || c.get(k + 1)
                    .is_none_or(|x| !x.is_alphanumeric() && *x != m[0]);
            if antes_ok && despues_ok {
                return Some(k);
            }
        }
        k += 1;
    }
    None
}

/// Una formula que empieza en `i` (un `$`): donde acaba, su fuente y si es
/// alta. Con las reglas de Pandoc para no confundir dinero con formulas: el
/// `$` que abre no va seguido de un blanco, el que cierra no va detras de
/// un blanco ni delante de una cifra. Si no cierra, no es formula.
fn formula(c: &[char], i: usize) -> Option<(usize, String, bool)> {
    if c.get(i + 1) == Some(&'$') {
        // `$$...$$`
        let desde = i + 2;
        let mut k = desde;
        while k + 1 < c.len() {
            if c[k] == '\\' {
                k += 2;
                continue;
            }
            if c[k] == '$' && c[k + 1] == '$' {
                let f: String = c[desde..k].iter().collect();
                return (!f.trim().is_empty()).then_some((k + 2, f.trim().to_string(), true));
            }
            k += 1;
        }
        return None;
    }
    let primero = *c.get(i + 1)?;
    if primero.is_whitespace() {
        return None;
    }
    let mut k = i + 1;
    while k < c.len() {
        if c[k] == '\\' {
            k += 2;
            continue;
        }
        if c[k] == '$' {
            if c[k - 1].is_whitespace() || c.get(k + 1).is_some_and(|d| d.is_ascii_digit()) {
                return None;
            }
            let f: String = c[i + 1..k].iter().collect();
            return Some((k + 1, f, false));
        }
        k += 1;
    }
    None
}

/// Un mandato de estilo fuera de una formula (`\color{rojo}{X}`): donde
/// acaba, el estilo de dentro y el trozo de dentro. `None` si no es uno de
/// estos o le faltan sus llaves (entonces se ve tal cual).
fn mandato(c: &[char], i: usize, est: EstiloTramo) -> Option<(usize, EstiloTramo, usize, usize)> {
    let mut k = i + 1;
    let mut nombre = String::new();
    while let Some(&ch) = c.get(k).filter(|ch| ch.is_alphabetic()) {
        nombre.push(ch);
        k += 1;
    }
    let mut sub = est;
    let con_argumento = matches!(nombre.as_str(), "color" | "textcolor" | "tam");
    let argumento = if con_argumento {
        let (a, b, fin) = llaves(c, k)?;
        k = fin;
        Some(c[a..b].iter().collect::<String>())
    } else {
        None
    };
    match nombre.as_str() {
        "color" | "textcolor" => {
            // Un color que no se conoce deja el de fuera: el texto se ve.
            if let Some(col) = color_de_nombre(argumento.as_deref().unwrap_or("")) {
                sub.color = Some(col);
            }
        }
        "tam" => {
            let f: f32 = argumento
                .as_deref()
                .unwrap_or("")
                .trim()
                .replace(',', ".")
                .parse()
                .ok()?;
            sub.escala = est.escala * f.clamp(0.3, 5.0);
        }
        "textbf" | "negrita" => sub.negrita = true,
        "textit" | "emph" | "cursiva" => sub.cursiva = true,
        "underline" | "uline" | "subrayado" => sub.subrayado = true,
        "sout" | "st" | "tachado" => sub.tachado = true,
        "tiny" => sub.escala = est.escala * 0.6,
        "small" | "pequeno" => sub.escala = est.escala * 0.8,
        "large" | "grande" => sub.escala = est.escala * 1.2,
        "Large" => sub.escala = est.escala * 1.44,
        "huge" | "Huge" => sub.escala = est.escala * 1.73,
        _ => return None,
    }
    let (a, b, fin) = llaves(c, k)?;
    Some((fin, sub, a, b))
}

/// Unas llaves equilibradas en `k` (se admiten blancos delante): donde
/// empieza y acaba lo de dentro, y donde sigue despues.
fn llaves(c: &[char], mut k: usize) -> Option<(usize, usize, usize)> {
    while c.get(k).is_some_and(|x| *x == ' ') {
        k += 1;
    }
    if c.get(k) != Some(&'{') {
        return None;
    }
    let desde = k + 1;
    let mut prof = 0i32;
    while k < c.len() {
        match c[k] {
            '\\' => k += 1,
            '{' => prof += 1,
            '}' => {
                prof -= 1;
                if prof == 0 {
                    return Some((desde, k, k + 1));
                }
            }
            _ => {}
        }
        k += 1;
    }
    None
}

/// **Quita las barras de mas que ponen los modelos** al escribir JSON
/// (`\\frac` en vez de `\frac`, medido con Haiku el 14-oct): una barra
/// doble pegada a una letra es un mandato. Para lo que llega del asistente;
/// lo que teclea el usuario no pasa por aqui.
pub fn sin_barras_dobles(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < c.len() {
        if c[i] == '\\'
            && c.get(i + 1) == Some(&'\\')
            && c.get(i + 2).is_some_and(|x| x.is_ascii_alphabetic())
        {
            i += 1;
            continue;
        }
        out.push(c[i]);
        i += 1;
    }
    out
}

/// **El texto plano de reserva**: lo que se guarda en `text` para quien no
/// sabe componer (el movil de hoy, Excalidraw, la busqueda).
pub fn plano(fuente: &str) -> String {
    leer(fuente)
        .iter()
        .map(|r| {
            r.tramos
                .iter()
                .map(|t| match t {
                    Tramo::Texto { texto, .. } => texto.clone(),
                    Tramo::Formula { fuente, .. } => {
                        crate::formulas::legible(&crate::formulas::leer(fuente))
                            .trim()
                            .to_string()
                    }
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// **Si una fuente lleva algo que componer**: una formula, un tramo con
/// estilo, una alineacion o un escape. Si no, es un texto de los de siempre
/// y se guarda sin fuente rica (un texto con un `2*3` no estrena nada).
pub fn tiene_marcas(fuente: &str) -> bool {
    if fuente.contains(crate::ecuacion::ANGULOS) {
        return true;
    }
    if !fuente.contains(['$', '*', '_', '~', '<', '\\']) {
        return false;
    }
    let renglones = leer(fuente);
    renglones.iter().any(|r| {
        r.alineado.is_some()
            || r.tramos.iter().any(|t| match t {
                Tramo::Formula { .. } => true,
                Tramo::Texto { estilo, .. } => *estilo != EstiloTramo::default(),
            })
    }) || plano(fuente) != fuente
}

/// **Un color por su nombre** (espanol o ingles, en masculino, femenino o
/// plural) o en `#rrggbb`/`#rgb`. La misma paleta que el asistente
/// (`pixpin_ia::herramientas::color`).
pub fn color_de_nombre(nombre: &str) -> Option<ColorRgba> {
    let t = nombre.trim().to_lowercase();
    let hex = |h: &str| -> Option<ColorRgba> {
        let v = |i: usize, n: usize| u8::from_str_radix(h.get(i..i + n)?, 16).ok();
        let (r, g, b) = match h.len() {
            3 => (v(0, 1)? * 17, v(1, 1)? * 17, v(2, 1)? * 17),
            6 | 8 => (v(0, 2)?, v(2, 2)?, v(4, 2)?),
            _ => return None,
        };
        Some(ColorRgba::opaco(
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
        ))
    };
    if let Some(h) = t.strip_prefix('#') {
        return hex(h);
    }
    let mut candidatos = vec![t.clone()];
    for sufijo in ["es", "s"] {
        if let Some(r) = t.strip_suffix(sufijo) {
            candidatos.push(r.to_string());
        }
    }
    for c in candidatos.clone() {
        if let Some(r) = c.strip_suffix('a') {
            candidatos.push(format!("{r}o"));
        }
    }
    candidatos.iter().find_map(|c| {
        let h = match c.as_str() {
            "negro" | "black" => "1e1e1e",
            "blanco" | "white" => "ffffff",
            "rojo" | "red" => "e03131",
            "verde" | "green" => "2f9e44",
            "azul" | "blue" => "1971c2",
            "amarillo" | "yellow" => "f08c00",
            "naranja" | "orange" => "e8590c",
            "morado" | "violeta" | "purple" | "violet" => "6741d9",
            "rosa" | "pink" | "magenta" => "c2255c",
            "gris" | "gray" | "grey" => "868e96",
            "marron" | "marrón" | "cafe" | "café" | "brown" => "8b5a2b",
            "celeste" | "cyan" | "cian" | "teal" => "0c8599",
            _ => return None,
        };
        hex(h)
    })
}

// ---------------------------------------------------------------------
// Componer
// ---------------------------------------------------------------------

/// Con que letra y a que lado se compone.
#[derive(Debug, Clone)]
pub struct Opciones<'a> {
    pub tam: f32,
    pub familia: &'a str,
    /// Negrita, cursiva y tachado del elemento entero (`Extras`).
    pub estilo: EstiloDeTexto,
    /// La del elemento (`textAlign`); `None` es izquierda.
    pub alineacion: Option<AlineacionTexto>,
    /// Donde se parten los renglones: `texto::SIN_PARTIR` o mas, no se
    /// parten (un texto suelto).
    pub ancho_max: f32,
}

/// Lo compuesto: lo que mide y sus piezas, con la esquina en `(0, 0)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Maqueta {
    pub ancho: f32,
    pub alto: f32,
    pub piezas: Vec<Pieza>,
    /// Donde empieza cada renglon de la fuente en vertical (para pruebas y
    /// para poner el cursor).
    pub renglones: Vec<f32>,
}

/// El alto de un renglon de una letra: el interlineado de su familia, o lo
/// que diga quien mide para las del sistema.
fn alto_de_renglon(t: f32, familia: &str) -> f32 {
    match texto::interlineado_de(familia) {
        Some(k) => t * k,
        None => texto::medida(" ", t, familia, EstiloDeTexto::default()).1,
    }
}

struct Atomo {
    caja: Caja,
    espacio: bool,
}

/// Una caja de letras de un tramo, con su raya de subrayado o tachado.
fn caja_de_texto(s: &str, t: f32, familia: &str, e: &EstiloTramo) -> Caja {
    let estilo = EstiloDeTexto {
        negrita: e.negrita,
        cursiva: e.cursiva,
        tachado: false,
    };
    let ancho = texto::ancho_real(s, t, familia, estilo);
    let alto = alto_de_renglon(t, familia);
    let mut piezas = vec![Pieza::Texto {
        x: 0.0,
        y: 0.0,
        ancho,
        alto,
        texto: s.to_string(),
        tam: t,
        cursiva: e.cursiva,
        negrita: e.negrita,
        color: e.color,
    }];
    // La base a un tamano de letra del borde de arriba, como el tachado de
    // siempre (`texto::raya_del_tachado`): la misma cuenta.
    let grosor = (t * texto::GROSOR_DEL_TACHADO).max(1.0);
    for (si, y) in [
        (e.subrayado, t * 1.1),
        (e.tachado, t - t * texto::ALTURA_DEL_TACHADO),
    ] {
        if si && ancho > 0.0 {
            piezas.push(Pieza::Raya {
                puntos: vec![Punto2::nuevo(0.0, y), Punto2::nuevo(ancho, y)],
                grosor,
                color: e.color,
            });
        }
    }
    Caja {
        ancho,
        alto,
        eje: alto * 0.55,
        piezas,
    }
}

fn atomos(r: &Renglon, o: &Opciones<'_>) -> Vec<Atomo> {
    let base = EstiloTramo {
        negrita: o.estilo.negrita,
        cursiva: o.estilo.cursiva,
        tachado: o.estilo.tachado,
        ..EstiloTramo::default()
    };
    let mut v = Vec::new();
    for tr in &r.tramos {
        match tr {
            Tramo::Texto { texto: s, estilo } => {
                let e = EstiloTramo {
                    negrita: estilo.negrita || base.negrita,
                    cursiva: estilo.cursiva || base.cursiva,
                    tachado: estilo.tachado || base.tachado,
                    ..*estilo
                };
                let t = o.tam * e.escala;
                // Palabras y blancos por separado: por los blancos se parte
                // el renglon y se reparte lo que sobra al justificar.
                let mut trozo = String::new();
                let mut en_blanco = None;
                for ch in s.chars() {
                    // Un signo de angulo suelto en el texto («Por ∢
                    // inscrito»), dibujado como en las formulas.
                    if caja::ANGULOS.contains(&ch) {
                        if !trozo.is_empty() {
                            v.push(Atomo {
                                caja: caja_de_texto(&trozo, t, o.familia, &e),
                                espacio: en_blanco == Some(true),
                            });
                            trozo.clear();
                        }
                        en_blanco = None;
                        let familia = o.familia;
                        let medir = |s: &str, t: f32| {
                            (
                                texto::ancho_real(s, t, familia, EstiloDeTexto::default()),
                                alto_de_renglon(t, familia),
                            )
                        };
                        let mut c = caja::angulo(ch, t, &medir);
                        if let Some(col) = e.color {
                            c = c.tenida(col);
                        }
                        v.push(Atomo {
                            caja: c,
                            espacio: false,
                        });
                        continue;
                    }
                    let blanco = ch == ' ' || ch == '\t';
                    if en_blanco.is_some_and(|b| b != blanco) {
                        v.push(Atomo {
                            caja: caja_de_texto(&trozo, t, o.familia, &e),
                            espacio: en_blanco == Some(true),
                        });
                        trozo.clear();
                    }
                    en_blanco = Some(blanco);
                    trozo.push(if ch == '\t' { ' ' } else { ch });
                }
                if !trozo.is_empty() {
                    v.push(Atomo {
                        caja: caja_de_texto(&trozo, t, o.familia, &e),
                        espacio: en_blanco == Some(true),
                    });
                }
            }
            Tramo::Formula {
                fuente,
                estilo,
                alta,
            } => {
                let t = o.tam * estilo.escala;
                let familia = o.familia;
                let medir = |s: &str, t: f32| {
                    (
                        texto::ancho_real(s, t, familia, EstiloDeTexto::default()),
                        alto_de_renglon(t, familia),
                    )
                };
                let arbol = crate::formulas::leer(fuente);
                let mut c = caja::de_formula(&arbol, t, &medir, !alta);
                // Un poco de aire a cada lado, como deja TeX entre una
                // formula y el texto (un sexto de espacio).
                c = caja::fila(vec![caja::hueco(t * 0.04), c, caja::hueco(t * 0.04)]);
                if let Some(col) = estilo.color {
                    c = c.tenida(col);
                }
                if estilo.negrita || base.negrita {
                    c = c.en_negrita();
                }
                v.push(Atomo {
                    caja: c,
                    espacio: false,
                });
            }
        }
    }
    v
}

/// **Compone una fuente rica**: renglones partidos (si hay ancho), cada uno
/// con sus cajas alineadas por el eje, a su lado y, si toca, justificado.
pub fn componer(fuente: &str, o: &Opciones<'_>) -> Maqueta {
    let renglones = leer(fuente);
    let partir = o.ancho_max < texto::SIN_PARTIR * 0.5;
    let alto_base = alto_de_renglon(o.tam, o.familia);
    // (atomos del renglon visual, alineado, es el ultimo de su parrafo, renglon de la fuente)
    let mut visuales: Vec<(Vec<Atomo>, Alineado, bool, usize)> = Vec::new();
    let por_defecto = match o.alineacion {
        Some(AlineacionTexto::Centro) => Alineado::Centro,
        Some(AlineacionTexto::Derecha) => Alineado::Derecha,
        _ => Alineado::Izquierda,
    };
    let n = renglones.len();
    for (k, r) in renglones.iter().enumerate() {
        let al = r.alineado.unwrap_or(por_defecto);
        // Sin partir, el parrafo acaba donde hay un renglon vacio (o al final).
        let fin_de_parrafo = k + 1 == n || renglones[k + 1].tramos.is_empty();
        let ats = atomos(r, o);
        if !partir {
            visuales.push((ats, al, fin_de_parrafo, k));
            continue;
        }
        let mut linea: Vec<Atomo> = Vec::new();
        let mut ancho = 0.0f32;
        for a in ats {
            if !a.espacio && !linea.is_empty() && ancho + a.caja.ancho > o.ancho_max {
                while linea.last().is_some_and(|x| x.espacio) {
                    linea.pop();
                }
                visuales.push((std::mem::take(&mut linea), al, false, k));
                ancho = 0.0;
            }
            if a.espacio
                && linea.is_empty()
                && !visuales.is_empty()
                && visuales.last().is_some_and(|v| v.3 == k)
            {
                continue; // el blanco donde se partio no empieza el renglon
            }
            ancho += a.caja.ancho;
            linea.push(a);
        }
        visuales.push((linea, al, true, k));
    }
    let ancho_natural = |ats: &[Atomo]| -> f32 {
        let fin = ats.iter().rposition(|a| !a.espacio).map_or(0, |p| p + 1);
        ats[..fin].iter().map(|a| a.caja.ancho).sum()
    };
    let bloque = visuales
        .iter()
        .map(|v| ancho_natural(&v.0))
        .fold(0.0f32, f32::max);
    let bloque = if partir {
        bloque.max(o.ancho_max)
    } else {
        bloque
    };
    let mut piezas = Vec::new();
    let mut y = 0.0f32;
    let mut inicio_de_renglon = vec![0.0f32; n];
    let mut visto = vec![false; n];
    for (ats, al, ultimo, k) in &visuales {
        if !visto[*k] {
            visto[*k] = true;
            inicio_de_renglon[*k] = y;
        }
        let arriba = ats
            .iter()
            .map(|a| a.caja.eje)
            .fold(alto_base * 0.55, f32::max);
        let abajo = ats
            .iter()
            .map(|a| a.caja.alto - a.caja.eje)
            .fold(alto_base * 0.45, f32::max);
        let natural = ancho_natural(ats);
        let fin = ats.iter().rposition(|a| !a.espacio).map_or(0, |p| p + 1);
        let huecos = ats[..fin].iter().filter(|a| a.espacio).count();
        let (mut x, reparto) = match al {
            Alineado::Izquierda => (0.0, 0.0),
            Alineado::Centro => ((bloque - natural) / 2.0, 0.0),
            Alineado::Derecha => (bloque - natural, 0.0),
            Alineado::Justificado if !*ultimo && huecos > 0 => {
                (0.0, (bloque - natural).max(0.0) / huecos as f32)
            }
            Alineado::Justificado => (0.0, 0.0),
        };
        x = x.max(0.0);
        let desde = piezas.len();
        for a in &ats[..fin] {
            piezas.extend(a.caja.piezas_colocadas(x, y + arriba - a.caja.eje));
            x += a.caja.ancho;
            if a.espacio {
                x += reparto;
            }
        }
        // Sin reparto, las palabras seguidas del mismo estilo van en UNA
        // orden: menos ordenes que pintar, y quien pinta con otra letra (el
        // PDF, con la del sistema) espacia las palabras con la suya en vez
        // de montarlas.
        if reparto == 0.0 {
            let renglon = piezas.split_off(desde);
            piezas.extend(juntar_palabras(renglon));
        }
        y += arriba + abajo;
    }
    Maqueta {
        ancho: bloque,
        alto: y.max(alto_base),
        piezas,
        renglones: inicio_de_renglon,
    }
}

/// Junta los textos seguidos de un renglon con la misma cara, tamano, color
/// y altura, pegados uno detras de otro.
fn juntar_palabras(v: Vec<Pieza>) -> Vec<Pieza> {
    let mut out: Vec<Pieza> = Vec::with_capacity(v.len());
    // Donde esta el ultimo texto (las rayas de un subrayado van detras de
    // su texto y no cortan la racha).
    let mut ultimo: Option<usize> = None;
    for p in v {
        if let Pieza::Texto {
            x,
            y,
            ancho,
            texto: t,
            tam,
            cursiva,
            negrita,
            color,
            ..
        } = &p
            && let Some(i) = ultimo
            && let Pieza::Texto {
                x: ax,
                y: ay,
                ancho: aa,
                texto: at,
                tam: atam,
                cursiva: ac,
                negrita: an,
                color: acol,
                ..
            } = &mut out[i]
            && (*ay - y).abs() < 0.01
            && (*atam - tam).abs() < 0.01
            && ac == cursiva
            && an == negrita
            && acol == color
            && (*ax + *aa - x).abs() < 0.05
        {
            at.push_str(t);
            *aa += ancho;
            continue;
        }
        if matches!(p, Pieza::Texto { .. }) {
            ultimo = Some(out.len());
        }
        out.push(p);
    }
    out
}

/// **Lo que mide un texto rico** (la caja de su elemento, como
/// `texto::medida` para el plano). Un renglon vacio mide uno de letra.
pub fn medida(fuente: &str, tam: f32, familia: &str, estilo: EstiloDeTexto) -> (f32, f32) {
    let m = componer(
        fuente,
        &Opciones {
            tam,
            familia,
            estilo,
            alineacion: None,
            ancho_max: texto::SIN_PARTIR,
        },
    );
    (
        m.ancho.max(texto::ancho_real(" ", tam, familia, estilo)),
        m.alto,
    )
}

/// **Le pone a un texto del lienzo lo escrito en formato rico**: si lleva
/// marcas, la fuente va a `Extras::rico`, el texto a su version plana y la
/// caja (suelta) a lo compuesto; si no, el texto tal cual y sin fuente. Es
/// lo que hacen el editor al salir de la caja y el asistente al escribir.
/// Devuelve si quedo rico.
pub fn poner_fuente(e: &mut crate::elemento::Elemento, fuente: &str) -> bool {
    let rico = tiene_marcas(fuente);
    let estilo = EstiloDeTexto {
        negrita: e.extras.negrita,
        cursiva: e.extras.cursiva,
        tachado: e.extras.tachado,
    };
    let crate::elemento::Figura::Texto {
        texto,
        tam,
        familia,
    } = &mut e.figura
    else {
        return false;
    };
    let (ancho, alto) = if rico {
        *texto = plano(fuente);
        medida(fuente, *tam, familia, estilo)
    } else {
        *texto = fuente.to_string();
        texto::medida(fuente, *tam, familia, estilo)
    };
    e.extras.rico = rico.then(|| fuente.to_string());
    if e.extras.contenedor.is_none() {
        e.ancho = ancho;
    }
    e.alto = alto;
    rico
}

/// **La caja de un texto del lienzo con fuente rica**, o `None` si no es
/// un texto rico (entonces vale `texto::medida` de siempre). Para quien
/// remide una caja al cambiar la letra o el tamano.
pub fn medida_del_elemento(e: &crate::elemento::Elemento) -> Option<(f32, f32)> {
    let rico = e.extras.rico.as_deref()?;
    let crate::elemento::Figura::Texto { tam, familia, .. } = &e.figura else {
        return None;
    };
    let estilo = EstiloDeTexto {
        negrita: e.extras.negrita,
        cursiva: e.extras.cursiva,
        tachado: e.extras.tachado,
    };
    Some(medida(rico, *tam, familia, estilo))
}

/// **Las ordenes de pintado** de una maqueta con su esquina en `(x, y)`.
/// `color` es el del elemento, ya con su opacidad: los tramos de color
/// llevan la misma opacidad.
pub fn ordenes(m: &Maqueta, x: f32, y: f32, familia: &str, color: ColorRgba) -> Vec<Orden> {
    let tinta = |c: Option<ColorRgba>| match c {
        Some(c) => ColorRgba {
            a: c.a * color.a,
            ..c
        },
        None => color,
    };
    m.piezas
        .iter()
        .filter_map(|p| match p {
            Pieza::Texto {
                x: px,
                y: py,
                texto: s,
                tam,
                cursiva,
                negrita,
                color: c,
                ..
            } => (!s.trim().is_empty()).then(|| Orden::Texto {
                texto: s.clone(),
                x: x + px,
                y: y + py,
                tam: *tam,
                familia: familia.to_string(),
                color: tinta(*c),
                ancho_max: texto::SIN_PARTIR,
                negrita: *negrita,
                cursiva: *cursiva,
            }),
            Pieza::Raya {
                puntos,
                grosor,
                color: c,
            } => Some(Orden::Polilinea {
                puntos: puntos
                    .iter()
                    .map(|q| Punto2::nuevo(q.x + x, q.y + y))
                    .collect(),
                color: tinta(*c),
                grosor: *grosor,
                estilo: EstiloTrazo::Solido,
            }),
        })
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un medidor fijo: cada letra mide 0,5 em; el alto es el renglon.
    fn medidor(s: &str, tam: f32, _f: &str, _e: EstiloDeTexto) -> Option<(f32, f32)> {
        Some((s.chars().count() as f32 * tam * 0.5, tam * 1.25))
    }

    fn con<R>(f: impl FnOnce() -> R) -> R {
        texto::con_medidor(medidor, f)
    }

    fn opciones(tam: f32) -> Opciones<'static> {
        Opciones {
            tam,
            familia: "Nunito",
            estilo: EstiloDeTexto::default(),
            alineacion: None,
            ancho_max: texto::SIN_PARTIR,
        }
    }

    fn textos(m: &Maqueta) -> Vec<(String, f32, f32, f32, Option<ColorRgba>)> {
        m.piezas
            .iter()
            .filter_map(|p| match p {
                Pieza::Texto {
                    texto,
                    x,
                    y,
                    tam,
                    color,
                    ..
                } => Some((texto.clone(), *x, *y, *tam, *color)),
                _ => None,
            })
            .collect()
    }

    fn rayas(m: &Maqueta) -> Vec<(Vec<Punto2>, Option<ColorRgba>)> {
        m.piezas
            .iter()
            .filter_map(|p| match p {
                Pieza::Raya { puntos, color, .. } => Some((puntos.clone(), *color)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn el_texto_y_la_formula_comparten_la_linea_base() {
        con(|| {
            let m = componer("Nos piden $m\\sphericalangle COD = X$", &opciones(20.0));
            let t = textos(&m);
            // Todas las letras del renglon, a la misma altura: mismo tamano,
            // mismo eje.
            let ys: Vec<f32> = t.iter().map(|x| x.2).collect();
            assert!(ys.iter().all(|y| (y - ys[0]).abs() < 0.01), "{t:?}");
            assert!(
                !t.iter().any(|x| x.0.contains('∢')),
                "el angulo va dibujado: {t:?}"
            );
            assert_eq!(
                rayas(&m).len(),
                2,
                "el signo del angulo: sus lados y su arco"
            );
            assert!(
                !t.iter().any(|x| x.0.contains('\\')),
                "sin mandatos a la vista: {t:?}"
            );
            // Un solo renglon de alto.
            assert!((m.alto - 25.0).abs() < 0.01, "{}", m.alto);
        });
    }

    #[test]
    fn el_superindice_sube_y_encoge() {
        con(|| {
            let m = componer("area $x^2$", &opciones(20.0));
            let t = textos(&m);
            let x = t.iter().find(|p| p.0 == "x").unwrap();
            let dos = t.iter().find(|p| p.0 == "2").unwrap();
            assert!(dos.3 < x.3, "encoge: {dos:?} {x:?}");
            assert!(dos.2 < x.2, "sube: {dos:?} {x:?}");
            assert!(dos.1 > x.1, "detras: {dos:?} {x:?}");
        });
    }

    #[test]
    fn la_fraccion_en_linea_tiene_raya_y_queda_centrada_en_el_renglon() {
        con(|| {
            let m = componer("$45° = \\frac{2a+2b}{2}$ fin", &opciones(20.0));
            let t = textos(&m);
            let r = rayas(&m);
            assert_eq!(r.len(), 1, "la barra de la fraccion: {r:?}");
            let barra_y = r[0].0[0].y;
            let num = t
                .iter()
                .find(|p| p.0.starts_with('2') && p.2 < barra_y)
                .expect("numerador encima");
            let den = t
                .iter()
                .find(|p| p.0.starts_with('2') && p.2 > barra_y)
                .expect("denominador debajo");
            assert!(num.3 < 20.0 && den.3 < 20.0, "en linea encoge: {num:?}");
            // El texto de despues va en la linea del «=».
            let fin = t.iter().find(|p| p.0.trim() == "fin").unwrap();
            let igual = t.iter().find(|p| p.0.contains('=')).unwrap();
            assert!((fin.2 - igual.2).abs() < 0.01);
            // El renglon crece para que quepa la fraccion.
            assert!(m.alto > 25.0);
        });
    }

    #[test]
    fn la_raya_y_el_arco_van_encima_y_del_ancho_de_las_letras() {
        con(|| {
            let m = componer(
                "$\\overline{AB}$ y $m\\overset{\\frown}{AC}$",
                &opciones(20.0),
            );
            let t = textos(&m);
            let r = rayas(&m);
            assert_eq!(r.len(), 2, "{r:?}");
            let a = t
                .iter()
                .find(|p| p.0 == "AB")
                .expect("A y B juntas, del mismo estilo");
            let b = (a.0.clone(), a.1 + 10.0, a.2, a.3, a.4);
            let raya = &r[0].0;
            assert_eq!(raya.len(), 2, "la raya es recta");
            assert!(
                raya[0].x >= a.1 - 0.01 && raya[1].x <= b.1 + 10.0 + 0.01,
                "{raya:?} {a:?} {b:?}"
            );
            assert!(raya[0].y < a.2 + 20.0 * 0.6, "encima de la letra");
            let arco = &r[1].0;
            assert!(arco.len() > 5, "el arco es curvo");
            assert!(
                arco.iter().all(|p| p.x.is_finite() && p.y.is_finite()),
                "sin NaN: {arco:?}"
            );
            let medio = arco[arco.len() / 2].y;
            assert!(medio < arco[0].y, "sube por el medio: {arco:?}");
        });
    }

    #[test]
    fn los_colores_por_tramo_y_dentro_de_la_formula() {
        con(|| {
            let rojo = color_de_nombre("rojo").unwrap();
            let azul = color_de_nombre("blue").unwrap();
            let m = componer(
                "Nos piden \\color{rojo}{X} y $a + \\color{blue}{b}$ **ya**",
                &opciones(20.0),
            );
            let t = textos(&m);
            assert_eq!(t.iter().find(|p| p.0 == "X").unwrap().4, Some(rojo));
            assert_eq!(t.iter().find(|p| p.0 == "b").unwrap().4, Some(azul));
            assert_eq!(t.iter().find(|p| p.0 == "a").unwrap().4, None);
            assert!(
                m.piezas.iter().any(
                    |p| matches!(p, Pieza::Texto { texto, negrita: true, .. } if texto == "ya")
                )
            );
            // Un color de la formula de fuera no pisa al de dentro.
            let m = componer("$\\color{red}{a + \\color{green}{b}}$", &opciones(20.0));
            let t = textos(&m);
            assert_eq!(
                t.iter().find(|p| p.0 == "a").unwrap().4,
                color_de_nombre("red")
            );
            assert_eq!(
                t.iter().find(|p| p.0 == "b").unwrap().4,
                color_de_nombre("green")
            );
        });
    }

    #[test]
    fn el_plano_de_reserva_se_lee_sin_formato() {
        assert_eq!(
            plano("Nos piden $m\\sphericalangle COD = \\color{red}{X}$"),
            "Nos piden m∢COD = X"
        );
        assert_eq!(plano("**Si** $x^2$ y *ya*"), "Si x² y ya");
        assert_eq!(plano("$45^\\circ = \\frac{2a+2b}{2}$"), "45° = (2a + 2b)/2");
        assert_eq!(plano("\\centro $\\overline{AB}$"), "A̅B̅");
        assert_eq!(plano("$m\\overset{\\frown}{AC}=2a$"), "mAC⌒ = 2a");
        assert_eq!(plano("cuesta \\$5"), "cuesta $5");
    }

    #[test]
    fn solo_es_rico_lo_que_lleva_marcas() {
        assert!(!tiene_marcas("Hola mundo"));
        assert!(!tiene_marcas("2*3*4 y nombre_de_fichero"));
        assert!(!tiene_marcas("cuesta $5 y $10"), "dinero no es formula");
        assert!(
            !tiene_marcas("C:\\Users\\max"),
            "una ruta se queda como esta"
        );
        assert!(tiene_marcas("$x^2$"));
        assert!(tiene_marcas("**ojo**"));
        assert!(tiene_marcas("\\centro hola"));
        assert!(tiene_marcas("\\color{red}{X}"));
    }

    #[test]
    fn el_latex_roto_se_ve_como_fuente_y_no_se_cuelga() {
        con(|| {
            for roto in [
                "$\\frac{1}{$",
                "$x^$",
                "texto $sin cerrar",
                "\\color{rojo}{sin cerrar",
                "$\\overline{AB$ y $\\sqrt[$",
                "**sin cerrar",
                "$}}}{{{$",
                "$\\begin{bmatrix} 1 & 2$",
                "\\tam{x}{a}",
                "$\\noexiste{x}$",
            ] {
                let m = componer(roto, &opciones(20.0));
                assert!(m.ancho.is_finite() && m.alto.is_finite(), "{roto}");
                let _ = plano(roto);
            }
            // Lo que no se entiende se ve con su barra.
            let t = textos(&componer("$\\noexiste{x}$", &opciones(20.0)));
            assert!(t.iter().any(|p| p.0 == "\\noexiste"), "{t:?}");
            let t = textos(&componer("texto $sin cerrar", &opciones(20.0)));
            assert!(t.iter().any(|p| p.0.contains('$')), "{t:?}");
            // Muy largo: se compone lo que cabe y no tarda.
            let largo = "$x^2$ ".repeat(10_000);
            let ahora = std::time::Instant::now();
            let _ = componer(&largo, &opciones(20.0));
            assert!(ahora.elapsed().as_secs_f32() < 5.0);
        });
    }

    #[test]
    fn centrado_y_justificado_por_renglon() {
        con(|| {
            let m = componer(
                "una linea bastante larga\n\\centro corta\n\\derecha fin",
                &opciones(20.0),
            );
            let t = textos(&m);
            let corta = t.iter().find(|p| p.0 == "corta").unwrap();
            let fin = t.iter().find(|p| p.0 == "fin").unwrap();
            assert!(
                (corta.1 - (m.ancho - 50.0) / 2.0).abs() < 0.1,
                "{corta:?} {}",
                m.ancho
            );
            assert!((fin.1 + 30.0 - m.ancho).abs() < 0.1, "{fin:?}");
            let m = componer(
                "\\justificado a b c\nuna linea bastante larga\n",
                &opciones(20.0),
            );
            let t = textos(&m);
            let c = t.iter().find(|p| p.0 == "c").unwrap();
            assert!(
                (c.1 + 10.0 - m.ancho).abs() < 0.1,
                "la ultima letra llega al borde: {c:?} {}",
                m.ancho
            );
        });
    }

    #[test]
    fn dentro_de_una_figura_parte_los_renglones_por_palabras() {
        con(|| {
            let mut o = opciones(20.0);
            o.ancho_max = 100.0;
            let m = componer("uno dos tres cuatro $x^2$", &o);
            let t = textos(&m);
            let ys: std::collections::BTreeSet<i32> = t.iter().map(|p| p.2 as i32).collect();
            assert!(ys.len() >= 2, "{t:?}");
            assert!(
                t.iter()
                    .all(|p| p.1 + p.0.chars().count() as f32 * p.3 * 0.5 <= 100.5),
                "{t:?}"
            );
        });
    }

    #[test]
    fn el_recuadro_rodea_la_formula() {
        con(|| {
            let m = componer("$\\boxed{\\theta = 90^\\circ}$", &opciones(20.0));
            let r = rayas(&m);
            assert_eq!(r.len(), 1);
            assert_eq!(r[0].0.len(), 5, "cuatro lados cerrados");
            let t = textos(&m);
            assert!(t.iter().any(|p| p.0.contains('θ')), "{t:?}");
            assert!(t.iter().any(|p| p.0.contains('°')), "{t:?}");
        });
    }

    #[test]
    fn las_ordenes_llevan_su_color_y_la_opacidad_del_elemento() {
        con(|| {
            let m = componer("a \\color{red}{b}", &opciones(20.0));
            let base = ColorRgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.5,
            };
            let o = ordenes(&m, 10.0, 20.0, "Nunito", base);
            let colores: Vec<ColorRgba> = o
                .iter()
                .filter_map(|o| match o {
                    Orden::Texto { color, .. } => Some(*color),
                    _ => None,
                })
                .collect();
            assert_eq!(colores.len(), 2);
            assert_eq!(colores[0], base);
            assert!((colores[1].a - 0.5).abs() < 1e-6 && colores[1].r > 0.8);
        });
    }
}

#[cfg(test)]
mod pruebas_formula_en_renglon {
    #[test]
    fn una_formula_con_punto_y_coma_no_se_corta() {
        let r = super::leer("$F_{(x; y)}=x^{2}$ es un polinomio");
        let dbg = format!("{:?}", r);
        println!("{dbg}");
        assert!(dbg.contains("F_{(x; y)}=x^{2}"), "{dbg}");
    }
}

#[cfg(test)]
mod pruebas_lector_de_formulas {
    #[test]
    fn el_subindice_con_punto_y_coma_no_suelta_la_llave() {
        let a = format!("{:?}", crate::formulas::leer("F_{(x; y)}=x^{2}"));
        assert!(!a.contains("texto: \"}\""), "{a}");
        // Todo el «(x; y)» va abajo, como subindice de la F.
        let abajo = a.split("abajo: ").nth(1).unwrap_or_default();
        assert!(
            abajo.find("\"y\"").unwrap_or(usize::MAX) < abajo.find("\" = \"").unwrap_or(0),
            "{a}"
        );
    }
}

#[cfg(test)]
mod pruebas_llave_debajo {
    #[test]
    fn underbrace_con_su_rotulo_y_underset() {
        let a = format!(
            "{:?}",
            crate::formulas::leer(r#"\underbrace{SS\ldots S}_{\text{"n" veces}}=\alpha"#)
        );
        assert!(a.contains("LlaveDebajo") && a.contains("Debajo {"), "{a}");
        assert!(!a.contains("underbrace"), "no sale como codigo: {a}");
        let b = format!(
            "{:?}",
            crate::formulas::leer(r#"\underset{variables}{(x; y)}"#)
        );
        assert!(b.contains("Debajo {"), "{b}");
    }
}
