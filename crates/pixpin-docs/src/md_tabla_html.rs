//! **Las tablas en HTML de una nota** (H12): la forma en que el movil guarda
//! una tabla que las barras de Markdown no saben decir, y lo que dejan en el
//! portapapeles Excel, Google Sheets, Word, LibreOffice o una pagina web.
//!
//! # Lo que se guarda en la nota
//!
//! Exactamente lo que escribe `Tablas.aHtml` del movil, renglon a renglon,
//! para que su `Markdown.kt` (que empieza a leer en un renglon que empieza
//! por `<table` y acaba en el que lleva `</table>`) la entienda:
//!
//! ```text
//! <table>
//!   <tr>
//!     <th colspan="2" align="center">Presupuesto</th>
//!   </tr>
//!   <tr>
//!     <td style="background:#ffc9c9;color:#e03131">Hormigon</td>
//!     <td align="right">3,20</td>
//!   </tr>
//! </table>
//! ```
//!
//! Solo van las celdas que mandan (una combinada es una sola con su
//! `colspan`/`rowspan`), `<th>` en las de cabecera, `align` y `valign` como
//! los escribe el movil, y el texto escapado (`&amp;`, `&lt;`, `&gt;`) con
//! su Markdown dentro (`**negrita**`). Los colores van en `style`: el lector
//! del movil (`Tablas.deHtml`) se salta lo que no conoce en vez de fallar,
//! asi que alli la tabla se ve entera con sus combinadas, sin los colores,
//! y aqui vuelve con ellos.
//!
//! # Lo que se pega
//!
//! Cada programa lo cuenta a su manera, y aqui se escucha a todos:
//!
//! - **Google Sheets**: todo en el `style` de la celda (`background-color`,
//!   `color`, `font-weight:bold`, `text-align`).
//! - **Excel**: clases (`class=xl65`) cuyo estilo va en el `<style>` del
//!   HTML entero, no del fragmento; y `align=right` en los numeros.
//! - **LibreOffice**: `bgcolor`, `align` y `<font color>`, `<b>` dentro.
//! - **Word** y las paginas: `style` en la celda y en el `<p>`/`<span>` de
//!   dentro.
//!
//! El negro de la letra y el blanco del fondo son «sin color» (es lo que
//! ponen todos por defecto, y en el tema oscuro serian una mancha). La
//! alineacion se queda en una por columna, la de la mayoria del cuerpo: si
//! no, cada tabla de Excel (numeros a la derecha, titulos a la izquierda)
//! dejaria de poder guardarse con barras.

use std::collections::HashMap;

use crate::md_tabla::{Alineacion, Formato, MAX_COLUMNAS, MAX_FILAS, Rgb, Tabla, Vertical};

// ---------------------------------------------------------------------------
// Escribir

/// `#rrggbb`, como lo escribe el movil en sus colores.
pub fn hex(c: Rgb) -> String {
    format!("#{:06x}", c & 0xff_ffff)
}

fn escapar(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// La tabla en el HTML de `Tablas.aHtml` del movil (sin salto al final).
pub fn a_html(t: &Tabla) -> String {
    let n = t.columnas();
    let mut s = String::from("<table>\n");
    for (f, fila) in t.filas.iter().enumerate() {
        s.push_str("  <tr>\n");
        for c in 0..n {
            let x = t.formato(f, c);
            if x.tapada {
                continue;
            }
            let etiqueta = if t.es_cabecera(f, c) { "th" } else { "td" };
            s.push_str("    <");
            s.push_str(etiqueta);
            if x.columnas > 1 {
                s.push_str(&format!(" colspan=\"{}\"", x.columnas));
            }
            if x.filas > 1 {
                s.push_str(&format!(" rowspan=\"{}\"", x.filas));
            }
            match t.alineacion_de(f, c) {
                Alineacion::Centro => s.push_str(" align=\"center\""),
                Alineacion::Derecha => s.push_str(" align=\"right\""),
                Alineacion::Izquierda => {}
            }
            match x.vertical {
                Vertical::Medio => s.push_str(" valign=\"middle\""),
                Vertical::Abajo => s.push_str(" valign=\"bottom\""),
                Vertical::Arriba => {}
            }
            let estilo: Vec<String> = [
                x.fondo.map(|c| format!("background:{}", hex(c))),
                x.letra.map(|c| format!("color:{}", hex(c))),
            ]
            .into_iter()
            .flatten()
            .collect();
            if !estilo.is_empty() {
                s.push_str(&format!(" style=\"{}\"", estilo.join(";")));
            }
            s.push('>');
            s.push_str(&escapar(fila.get(c).map(String::as_str).unwrap_or("")));
            s.push_str(&format!("</{etiqueta}>\n"));
        }
        s.push_str("  </tr>\n");
    }
    s.push_str("</table>");
    s
}

// ---------------------------------------------------------------------------
// Trocitos de HTML

fn buscar(pajar_bajo: &str, aguja: &str, desde: usize) -> Option<usize> {
    pajar_bajo.get(desde..)?.find(aguja).map(|i| i + desde)
}

/// Las entidades: las cinco de siempre, `&nbsp;` y las numericas.
fn entidades(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut sal = String::with_capacity(s.len());
    let mut resto = s;
    while let Some(i) = resto.find('&') {
        sal.push_str(&resto[..i]);
        let tras = &resto[i..];
        let fin = tras[..tras.len().min(12)].find(';');
        let c = fin.and_then(|fin| {
            let e = &tras[1..fin];
            if let Some(h) = e.strip_prefix("#x").or_else(|| e.strip_prefix("#X")) {
                u32::from_str_radix(h, 16).ok().and_then(char::from_u32)
            } else if let Some(d) = e.strip_prefix('#') {
                d.parse().ok().and_then(char::from_u32)
            } else {
                match e.to_ascii_lowercase().as_str() {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    "nbsp" => Some(' '),
                    _ => None,
                }
            }
        });
        match (c, fin) {
            (Some(c), Some(fin)) => {
                sal.push(c);
                resto = &tras[fin + 1..];
            }
            _ => {
                sal.push('&');
                resto = &tras[1..];
            }
        }
    }
    sal.push_str(resto);
    sal
}

/// El valor de un atributo (con comillas dobles, simples o sin ellas, que
/// Excel escribe `class=xl65`). `align` no se confunde con `valign`.
fn atributo(atributos: &str, nombre: &str) -> Option<String> {
    let bajo = atributos.to_ascii_lowercase();
    let mut desde = 0;
    while let Some(i) = buscar(&bajo, nombre, desde) {
        desde = i + 1;
        let antes_bien = i == 0 || bajo[..i].ends_with(|c: char| c.is_whitespace() || c == '<');
        let tras = &atributos[i + nombre.len()..];
        if !antes_bien || !tras.trim_start().starts_with('=') {
            continue;
        }
        let valor = tras.trim_start()[1..].trim_start();
        let crudo = match valor.chars().next() {
            Some(q @ ('"' | '\'')) => valor[1..].split(q).next().unwrap_or(""),
            _ => valor
                .split(|c: char| c.is_whitespace() || c == '>')
                .next()
                .unwrap_or(""),
        };
        return Some(entidades(crudo));
    }
    None
}

/// Las declaraciones de un `style` (o de una regla de CSS): `clave: valor`
/// en minusculas la clave, en el orden en que vienen.
fn declaraciones(css: &str) -> Vec<(String, String)> {
    css.split(';')
        .filter_map(|d| {
            let (k, v) = d.split_once(':')?;
            Some((k.trim().to_ascii_lowercase(), v.trim().to_string()))
        })
        .collect()
}

fn valor<'a>(decl: &'a [(String, String)], clave: &str) -> Option<&'a str> {
    decl.iter()
        .rev()
        .find(|(k, _)| k == clave)
        .map(|(_, v)| v.as_str())
}

/// Un color de CSS o de atributo: `#rgb`, `#rrggbb`, `rgb(…)` y los
/// nombres de siempre. `None` con lo que no es un color de verdad
/// (`transparent`, `auto`, `windowtext`…).
pub fn color(v: &str) -> Option<Rgb> {
    let v = v
        .trim()
        .trim_end_matches("!important")
        .trim()
        .to_ascii_lowercase();
    // `background: #FFC000 none` de Excel: el primer trozo que sea color.
    if v.contains(' ') && !v.starts_with("rgb") {
        return v.split_whitespace().find_map(color);
    }
    if let Some(h) = v.strip_prefix('#') {
        return match h.len() {
            6 => u32::from_str_radix(h, 16).ok(),
            3 => {
                let n = u32::from_str_radix(h, 16).ok()?;
                let (r, g, b) = ((n >> 8) & 0xf, (n >> 4) & 0xf, n & 0xf);
                Some(((r * 17) << 16) | ((g * 17) << 8) | (b * 17))
            }
            _ => None,
        };
    }
    if let Some(dentro) = v.strip_prefix("rgba(").or_else(|| v.strip_prefix("rgb(")) {
        let partes: Vec<&str> = dentro
            .trim_end_matches(')')
            .split(',')
            .map(str::trim)
            .collect();
        if partes.len() == 4 && partes[3].parse::<f32>().ok()? == 0.0 {
            return None;
        }
        let n = |i: usize| {
            partes
                .get(i)?
                .parse::<f32>()
                .ok()
                .map(|x| x.clamp(0.0, 255.0) as u32)
        };
        return Some((n(0)? << 16) | (n(1)? << 8) | n(2)?);
    }
    Some(match v.as_str() {
        "black" => 0x000000,
        "white" => 0xffffff,
        "red" => 0xff0000,
        "green" => 0x008000,
        "lime" => 0x00ff00,
        "blue" => 0x0000ff,
        "yellow" => 0xffff00,
        "orange" => 0xffa500,
        "gray" | "grey" => 0x808080,
        "silver" => 0xc0c0c0,
        "maroon" => 0x800000,
        "navy" => 0x000080,
        "purple" => 0x800080,
        "teal" => 0x008080,
        "olive" => 0x808000,
        "aqua" | "cyan" => 0x00ffff,
        "fuchsia" | "magenta" => 0xff00ff,
        _ => return None,
    })
}

/// Quita los comentarios y los bloques `<style>`, `<script>` y `<head>`:
/// Excel mete alli su hoja de estilo entera, con `td` que no son celdas.
fn sin_ruido(html: &str) -> String {
    let mut s = String::with_capacity(html.len());
    let mut resto = html;
    while let Some(i) = resto.find("<!--") {
        s.push_str(&resto[..i]);
        resto = match resto[i + 4..].find("-->") {
            Some(j) => &resto[i + 4 + j + 3..],
            None => "",
        };
    }
    s.push_str(resto);
    for bloque in ["style", "script", "head"] {
        loop {
            let bajo = s.to_ascii_lowercase();
            let Some(a) = buscar(&bajo, &format!("<{bloque}"), 0).filter(|&a| {
                bajo[a + 1 + bloque.len()..].starts_with(|c: char| c == '>' || c.is_whitespace())
            }) else {
                break;
            };
            let cierre = format!("</{bloque}>");
            let b = buscar(&bajo, &cierre, a).map_or(s.len(), |b| b + cierre.len());
            s.replace_range(a..b, "");
        }
    }
    s
}

/// Las reglas de clase de los `<style>` (`.xl65 {…}`, `td.xl65 {…}`): lo
/// que Excel pone fuera de la celda.
fn clases(html: &str) -> HashMap<String, Vec<(String, String)>> {
    let mut sal = HashMap::new();
    let bajo = html.to_ascii_lowercase();
    let mut desde = 0;
    while let Some(a) = buscar(&bajo, "<style", desde) {
        let Some(abre) = buscar(&bajo, ">", a) else {
            break;
        };
        let b = buscar(&bajo, "</style>", abre).unwrap_or(html.len());
        let css = html[abre + 1..b].replace("<!--", " ").replace("-->", " ");
        for regla in css.split('}') {
            let Some((selectores, cuerpo)) = regla.split_once('{') else {
                continue;
            };
            let decl = declaraciones(cuerpo);
            for sel in selectores.split(',') {
                let sel = sel.trim();
                if let Some((_, clase)) = sel.rsplit_once('.')
                    && !clase.is_empty()
                    && clase
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                {
                    sal.insert(clase.to_ascii_lowercase(), decl.clone());
                }
            }
        }
        desde = b;
    }
    sal
}

/// Donde empieza la etiqueta `nombre` (`<td`), y no otra que empiece igual
/// (`<thead`).
fn etiqueta(bajo: &str, nombre: &str, mut desde: usize) -> Option<usize> {
    loop {
        let i = buscar(bajo, nombre, desde)?;
        if bajo[i + nombre.len()..].starts_with(|c: char| c == '>' || c == '/' || c.is_whitespace())
        {
            return Some(i);
        }
        desde = i + 1;
    }
}

/// Una celda tal como viene: si es `<th>`, sus atributos y lo de dentro.
struct CeldaHtml {
    cabecera: bool,
    atributos: String,
    interior: String,
}

/// Las filas de la tabla (o, si el fragmento no trae `<table>`, de las
/// `<tr>` sueltas que deja Excel a veces).
fn filas_html(html: &str) -> Vec<Vec<CeldaHtml>> {
    let bajo = html.to_ascii_lowercase();
    let mut filas = Vec::new();
    let mut desde = 0;
    while let Some(tr) = buscar(&bajo, "<tr", desde) {
        if !bajo[tr + 3..].starts_with(|c: char| c == '>' || c.is_whitespace()) {
            desde = tr + 3;
            continue;
        }
        let Some(abre_fin) = buscar(&bajo, ">", tr) else {
            break;
        };
        let siguiente = buscar(&bajo, "<tr", abre_fin).unwrap_or(html.len());
        let cierra = buscar(&bajo, "</tr>", abre_fin)
            .unwrap_or(html.len())
            .min(siguiente);
        let dentro = &html[abre_fin + 1..cierra];
        let dentro_bajo = &bajo[abre_fin + 1..cierra];
        let mut fila = Vec::new();
        let mut k = 0;
        loop {
            let td = etiqueta(dentro_bajo, "<td", k);
            let th = etiqueta(dentro_bajo, "<th", k);
            let Some(c0) = [td, th].into_iter().flatten().min() else {
                break;
            };
            let Some(c1) = buscar(dentro_bajo, ">", c0) else {
                break;
            };
            let fin = [
                buscar(dentro_bajo, "</td>", c1),
                buscar(dentro_bajo, "</th>", c1),
            ]
            .into_iter()
            .flatten()
            .min()
            .unwrap_or(dentro.len());
            fila.push(CeldaHtml {
                cabecera: th == Some(c0),
                atributos: dentro[c0 + 3..c1].to_string(),
                interior: dentro[c1 + 1..fin].to_string(),
            });
            k = fin + 1;
            if k >= dentro.len() {
                break;
            }
        }
        filas.push(fila);
        desde = cierra.max(tr + 3);
        if filas.len() >= MAX_FILAS {
            break;
        }
    }
    filas
}

/// Sin etiquetas.
fn sin_etiquetas(s: &str) -> String {
    let mut sal = String::with_capacity(s.len());
    let mut dentro = false;
    for c in s.chars() {
        match c {
            '<' => dentro = true,
            '>' if dentro => dentro = false,
            _ if !dentro => sal.push(c),
            _ => {}
        }
    }
    sal
}

/// Coloca las celdas en la rejilla: cada una en el primer hueco libre de
/// su fila (los de arriba los pueden ocupar las de `rowspan`), y las que
/// tapa, marcadas.
fn en_rejilla(filas: Vec<Vec<(String, Formato)>>) -> Option<Tabla> {
    let alto = filas.len();
    let mut ocupadas = std::collections::HashSet::<(usize, usize)>::new();
    let mut puestas: Vec<Vec<Option<(String, Formato)>>> = vec![Vec::new(); alto];
    for (f, fila) in filas.into_iter().enumerate() {
        let mut c = 0;
        for (texto, mut x) in fila {
            while ocupadas.contains(&(f, c)) {
                c += 1;
            }
            if c >= MAX_COLUMNAS {
                break;
            }
            x.filas = x.filas.clamp(1, alto - f);
            x.columnas = x.columnas.clamp(1, MAX_COLUMNAS - c);
            for df in 0..x.filas {
                for dc in 0..x.columnas {
                    ocupadas.insert((f + df, c + dc));
                }
            }
            let fila = &mut puestas[f];
            if fila.len() <= c {
                fila.resize(c + 1, None);
            }
            fila[c] = Some((texto, x));
            c += x.columnas;
        }
    }
    let ancho = ocupadas.iter().map(|(_, c)| c + 1).max().unwrap_or(0);
    if alto == 0 || ancho == 0 {
        return None;
    }
    let mut t = Tabla {
        filas: vec![vec![String::new(); ancho]; alto],
        alineaciones: vec![Alineacion::Izquierda; ancho],
        formato: vec![vec![Formato::default(); ancho]; alto],
    };
    for (f, c) in &ocupadas {
        t.formato[*f][*c].tapada = true;
    }
    for (f, fila) in puestas.into_iter().enumerate() {
        for (c, celda) in fila.into_iter().enumerate() {
            if let Some((texto, x)) = celda {
                t.filas[f][c] = texto;
                t.formato[f][c] = Formato { tapada: false, ..x };
            }
        }
    }
    // Una combinada que se salia de la tabla ya esta recortada; las que
    // ocupan mas alla de una fila corta dejan celdas sueltas, y esas son
    // celdas normales vacias.
    for f in 0..alto {
        for c in 0..ancho {
            if t.formato[f][c].tapada && t.ancla(f, c) == (f, c) {
                t.formato[f][c] = Formato::default();
            }
        }
    }
    Some(t)
}

fn alineacion_de(v: &str) -> Option<Alineacion> {
    match v.trim().to_ascii_lowercase().as_str() {
        "center" | "middle" => Some(Alineacion::Centro),
        "right" | "end" => Some(Alineacion::Derecha),
        "left" | "start" | "justify" => Some(Alineacion::Izquierda),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Leer lo que hay en una nota

/// **Una tabla HTML de una nota** (la del movil o la que se escribio
/// aqui), con todo lo que [`a_html`] escribe. `None` si no es una tabla de
/// principio a fin o si lleva titulo (`<caption>`), que el editor no sabe
/// ensenar: se queda como texto.
pub fn leer_de_nota(html: &str) -> Option<Tabla> {
    let limpio = html.trim();
    let bajo = limpio.to_ascii_lowercase();
    if !bajo.starts_with("<table") || !bajo.ends_with("</table>") || bajo.contains("<caption") {
        return None;
    }
    let filas = filas_html(limpio);
    if filas.iter().all(Vec::is_empty) {
        return None;
    }
    let mut leidas = Vec::new();
    for fila in &filas {
        let mut sal = Vec::new();
        for celda in fila {
            let a = &celda.atributos;
            let estilo = declaraciones(&atributo(a, "style").unwrap_or_default());
            let tramo = |nombre: &str, tope: usize| {
                atributo(a, nombre)
                    .and_then(|v| v.trim().parse::<usize>().ok())
                    .unwrap_or(1)
                    .clamp(1, tope)
            };
            let x = Formato {
                columnas: tramo("colspan", 20),
                filas: tramo("rowspan", 40),
                fondo: valor(&estilo, "background")
                    .or(valor(&estilo, "background-color"))
                    .and_then(color),
                letra: valor(&estilo, "color").and_then(color),
                alineacion: atributo(a, "align").as_deref().and_then(alineacion_de),
                cabecera: Some(celda.cabecera),
                vertical: match atributo(a, "valign")
                    .unwrap_or_default()
                    .to_ascii_lowercase()
                    .as_str()
                {
                    "middle" => Vertical::Medio,
                    "bottom" => Vertical::Abajo,
                    _ => Vertical::Arriba,
                },
                ..Formato::default()
            };
            sal.push((
                entidades(&sin_etiquetas(&celda.interior))
                    .trim()
                    .to_string(),
                x,
            ));
        }
        leidas.push(sal);
    }
    let mut t = en_rejilla(leidas)?;
    for (f, fila) in t.formato.iter_mut().enumerate() {
        for x in fila.iter_mut().filter(|x| !x.tapada) {
            x.alineacion = Some(x.alineacion.unwrap_or_default());
            x.cabecera = x.cabecera.filter(|es| *es != (f == 0));
        }
    }
    t.canonica();
    Some(t)
}

// ---------------------------------------------------------------------------
// Pegar

/// El texto de una celda pegada: `<br>` y los blancos seguidos, un
/// espacio (una celda no tiene saltos en Markdown); sin etiquetas.
fn texto_pegado(html: &str) -> String {
    let bajo = html.to_ascii_lowercase();
    let mut s = String::with_capacity(html.len());
    let mut i = 0;
    while i < html.len() {
        if html[i..].starts_with('<') {
            let fin = html[i..].find('>').map_or(html.len(), |k| i + k + 1);
            if bajo[i..].starts_with("<br")
                || bajo[i..].starts_with("<p")
                || bajo[i..].starts_with("</p")
            {
                s.push(' ');
            }
            i = fin;
            continue;
        }
        let fin = html[i..].find('<').map_or(html.len(), |k| i + k);
        s.push_str(&html[i..fin]);
        i = fin;
    }
    entidades(&s)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn es_negrita(decl: &[(String, String)]) -> Option<bool> {
    let v = valor(decl, "font-weight")?.to_ascii_lowercase();
    Some(v.starts_with("bold") || v.parse::<u32>().is_ok_and(|n| n >= 600))
}

fn es_cursiva(decl: &[(String, String)]) -> Option<bool> {
    Some(
        valor(decl, "font-style")?
            .to_ascii_lowercase()
            .starts_with("italic"),
    )
}

/// Todos los `style` de las etiquetas de dentro de una celda, en orden.
fn estilos_de_dentro(interior: &str) -> Vec<(String, Vec<(String, String)>)> {
    let bajo = interior.to_ascii_lowercase();
    let mut sal = Vec::new();
    let mut i = 0;
    while let Some(a) = buscar(&bajo, "<", i) {
        let Some(b) = buscar(&bajo, ">", a) else {
            break;
        };
        let etiqueta = &interior[a + 1..b];
        if !etiqueta.starts_with('/') {
            let nombre: String = etiqueta
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect::<String>()
                .to_ascii_lowercase();
            let mut decl = declaraciones(&atributo(etiqueta, "style").unwrap_or_default());
            if let Some(c) = atributo(etiqueta, "color") {
                decl.insert(0, ("color".into(), c));
            }
            if let Some(al) = atributo(etiqueta, "align") {
                decl.insert(0, ("text-align".into(), al));
            }
            sal.push((nombre, decl));
        }
        i = b + 1;
    }
    sal
}

/// **La tabla que se pega**, con sus combinadas, su negrita (escrita como
/// `**…**`), sus colores y su alineacion. `fragmento` es lo de dentro del
/// `HTML Format`; `entero`, el HTML entero (de donde salen las clases de
/// Excel). `None` si no hay tabla, si es una sola celda (eso se pega como
/// texto) o si alrededor de la tabla hay texto (se copio media pagina: se
/// pega como texto, que la tabla sola perderia lo demas).
pub fn leer_pegado(fragmento: &str, entero: Option<&str>) -> Option<Tabla> {
    let css = clases(entero.unwrap_or(fragmento));
    let limpio = sin_ruido(fragmento);
    let bajo = limpio.to_ascii_lowercase();
    let (tabla, fuera) = match (buscar(&bajo, "<table", 0), buscar(&bajo, "</table>", 0)) {
        (Some(a), Some(b)) if a < b => (
            &limpio[a..b],
            format!("{}{}", &limpio[..a], &limpio[b + 8..]),
        ),
        _ if bajo.contains("<tr") => (limpio.as_str(), String::new()),
        _ => return None,
    };
    if !entidades(&sin_etiquetas(&fuera)).trim().is_empty() {
        return None;
    }
    let filas = filas_html(tabla);
    let mut leidas = Vec::new();
    let mut alin_celdas = Vec::new();
    for fila in &filas {
        let mut sal = Vec::new();
        let mut alin = Vec::new();
        for celda in fila {
            let a = &celda.atributos;
            // De menos a mas fuerte: la clase, los atributos y el estilo de
            // la celda, y lo de dentro (el `<span>` de Word, el `<font>`).
            let mut decl: Vec<(String, String)> = Vec::new();
            for clase in atributo(a, "class").unwrap_or_default().split_whitespace() {
                if let Some(d) = css.get(&clase.to_ascii_lowercase()) {
                    decl.extend(d.iter().cloned());
                }
            }
            if let Some(b) = atributo(a, "bgcolor") {
                decl.push(("background-color".into(), b));
            }
            if let Some(al) = atributo(a, "align") {
                decl.push(("text-align".into(), al));
            }
            decl.extend(declaraciones(&atributo(a, "style").unwrap_or_default()));
            let dentro = estilos_de_dentro(&celda.interior);
            let mut negrita = es_negrita(&decl).unwrap_or(false);
            let mut cursiva = es_cursiva(&decl).unwrap_or(false);
            let mut letra = valor(&decl, "color").and_then(color);
            let mut alineacion = valor(&decl, "text-align").and_then(alineacion_de);
            for (nombre, d) in &dentro {
                negrita |=
                    matches!(nombre.as_str(), "b" | "strong") || es_negrita(d).unwrap_or(false);
                cursiva |= matches!(nombre.as_str(), "i" | "em") || es_cursiva(d).unwrap_or(false);
                if let Some(c) = valor(d, "color").and_then(color) {
                    letra = Some(c);
                }
                if let Some(al) = valor(d, "text-align").and_then(alineacion_de) {
                    alineacion = Some(al);
                }
            }
            let fondo = valor(&decl, "background-color")
                .and_then(color)
                .or_else(|| valor(&decl, "background").and_then(color))
                .filter(|c| *c != 0xffffff);
            let letra = letra.filter(|c| *c != 0x000000);
            let mut texto = texto_pegado(&celda.interior);
            if !texto.is_empty() {
                texto = match (negrita, cursiva) {
                    (true, true) => format!("***{texto}***"),
                    (true, false) => format!("**{texto}**"),
                    (false, true) => format!("*{texto}*"),
                    _ => texto,
                };
            }
            let tramo = |nombre: &str| {
                atributo(a, nombre)
                    .and_then(|v| v.trim().parse::<usize>().ok())
                    .unwrap_or(1)
                    .clamp(1, 50)
            };
            let (columnas, filas) = (tramo("colspan"), tramo("rowspan"));
            sal.push((
                texto,
                Formato {
                    columnas,
                    filas,
                    // Una combinada (el titulo a lo ancho) guarda la suya.
                    alineacion: (columnas > 1 || filas > 1).then(|| alineacion.unwrap_or_default()),
                    fondo,
                    letra,
                    ..Formato::default()
                },
            ));
            alin.push(alineacion.unwrap_or_default());
        }
        leidas.push(sal);
        alin_celdas.push(alin);
    }
    // Las filas de rellenar del final (Sheets deja alguna) fuera.
    while leidas.last().is_some_and(|f| {
        f.iter()
            .all(|(t, x)| t.is_empty() && x.fondo.is_none() && x.filas == 1)
    }) {
        leidas.pop();
        alin_celdas.pop();
    }
    let mut t = en_rejilla(leidas)?;
    if t.filas.len() * t.columnas() <= 1 {
        return None;
    }
    // Una alineacion por columna: la de la mayoria del cuerpo (o de la
    // cabecera si no hay cuerpo). La rejilla puede haber movido celdas de
    // sitio: se busca cada una por su orden en su fila.
    let n = t.columnas();
    let mut votos = vec![[0usize; 3]; n];
    for (f, alin) in alin_celdas.iter().enumerate() {
        let mut k = 0;
        for (c, voto) in votos.iter_mut().enumerate() {
            if t.formato[f][c].tapada {
                continue;
            }
            if (f > 0 || t.filas.len() == 1)
                && let Some(a) = alin.get(k)
            {
                voto[*a as usize] += 1;
            }
            k += 1;
        }
    }
    for (c, v) in votos.iter().enumerate() {
        let mejor = (0..3)
            .max_by_key(|i| (v[*i], usize::from(*i == 0)))
            .unwrap_or(0);
        t.alineaciones[c] = [
            Alineacion::Izquierda,
            Alineacion::Centro,
            Alineacion::Derecha,
        ][mejor];
    }
    for fila in &mut t.formato {
        for (c, x) in fila.iter_mut().enumerate() {
            if x.alineacion == Some(t.alineaciones[c]) {
                x.alineacion = None;
            }
        }
    }
    t.normalizar();
    Some(t)
}

/// **Texto con tabuladores** (el `CF_UNICODETEXT` de una hoja de calculo,
/// con las comillas de Excel: una celda con un salto o un tabulador va
/// entre comillas y sus comillas, dobladas). Es tabla solo si todas las
/// filas tienen las mismas columnas, al menos dos, y hay al menos dos
/// filas: un texto con algun tabulador suelto (codigo sangrado) no lo es.
pub fn leer_tsv(texto: &str) -> Option<Tabla> {
    let mut filas: Vec<Vec<String>> = vec![Vec::new()];
    let mut actual = String::new();
    let mut comillas = false;
    let mut letras = texto.chars().peekable();
    let mut al_principio = true;
    while let Some(c) = letras.next() {
        match c {
            '"' if al_principio && !comillas => comillas = true,
            '"' if comillas => {
                if letras.peek() == Some(&'"') {
                    actual.push('"');
                    letras.next();
                } else {
                    comillas = false;
                }
            }
            '\t' if !comillas => {
                filas.last_mut()?.push(std::mem::take(&mut actual));
                al_principio = true;
                continue;
            }
            '\r' if !comillas => {}
            '\n' if !comillas => {
                filas.last_mut()?.push(std::mem::take(&mut actual));
                filas.push(Vec::new());
                al_principio = true;
                continue;
            }
            '\n' | '\r' | '\t' => actual.push(' '),
            _ => actual.push(c),
        }
        al_principio = false;
    }
    filas.last_mut()?.push(actual);
    while filas
        .last()
        .is_some_and(|f| f.iter().all(|x| x.trim().is_empty()))
    {
        filas.pop();
    }
    let n = filas.first()?.len();
    if n < 2 || filas.len() < 2 || filas.iter().any(|f| f.len() != n) {
        return None;
    }
    filas.truncate(MAX_FILAS);
    for f in &mut filas {
        f.truncate(MAX_COLUMNAS);
        for x in f.iter_mut() {
            *x = x.trim().to_string();
        }
    }
    let n = n.min(MAX_COLUMNAS);
    Some(Tabla {
        filas,
        alineaciones: vec![Alineacion::Izquierda; n],
        formato: Vec::new(),
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::md_tabla::{a_texto, leer_gfm};

    /// **El lector del movil**, `Tablas.deHtml` de `motormd/Tablas.kt`,
    /// regla a regla (sus `indexOf`, su `atributo` con su expresion
    /// `nombre\s*=\s*"([^"]*)"` sin mirar lo de delante, su `sinEtiquetas`
    /// y su `desescapar` de tres entidades): lo que lee de cada celda.
    #[derive(Debug, PartialEq)]
    struct DelMovil {
        texto: String,
        cabecera: bool,
        alineacion: &'static str,
        colspan: usize,
        rowspan: usize,
    }

    fn atributo_del_movil(etiqueta: &str, nombre: &str) -> Option<String> {
        // `Regex("$nombre\\s*=\\s*\"([^\"]*)\"", IGNORE_CASE).find(...)`
        let b = etiqueta.to_lowercase();
        let mut desde = 0;
        while let Some(i) = b[desde..].find(nombre).map(|i| i + desde) {
            let resto = etiqueta[i + nombre.len()..].trim_start();
            if let Some(r) = resto.strip_prefix('=')
                && let Some(r) = r.trim_start().strip_prefix('"')
                && let Some(fin) = r.find('"')
            {
                return Some(r[..fin].to_string());
            }
            desde = i + 1;
        }
        None
    }

    fn lee_como_el_movil(texto: &str) -> Vec<Vec<DelMovil>> {
        let bajo = texto.to_lowercase();
        let desescapar = |s: &str| {
            s.replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&amp;", "&")
        };
        let mut filas = Vec::new();
        let mut pos = 0;
        while let Some(abre) = bajo[pos..].find("<tr").map(|i| i + pos) {
            let cierra = bajo[abre..].find("</tr>").map(|i| i + abre);
            let hasta = cierra.unwrap_or(texto.len());
            let fila = &texto[abre..hasta];
            let fila_b = &bajo[abre..hasta];
            let mut celdas = Vec::new();
            let mut p = 0;
            loop {
                let th = fila_b[p..].find("<th").map(|i| i + p);
                let td = fila_b[p..].find("<td").map(|i| i + p);
                let Some(a) = [th, td].into_iter().flatten().min() else {
                    break;
                };
                let es_th = Some(a) == th;
                let Some(fin) = fila[a..].find('>').map(|i| i + a) else {
                    break;
                };
                let attrs = &fila[a..fin];
                let cierre = if es_th { "</th>" } else { "</td>" };
                let c = fila_b[fin..].find(cierre).map(|i| i + fin);
                let h = c.unwrap_or(fila.len());
                celdas.push(DelMovil {
                    texto: desescapar(sin_etiquetas(&fila[fin + 1..h]).trim()),
                    cabecera: es_th,
                    alineacion: match atributo_del_movil(attrs, "align")
                        .map(|x| x.to_lowercase())
                        .as_deref()
                    {
                        Some("center") => "centro",
                        Some("right") => "derecha",
                        _ => "izquierda",
                    },
                    colspan: atributo_del_movil(attrs, "colspan")
                        .and_then(|x| x.parse().ok())
                        .unwrap_or(1),
                    rowspan: atributo_del_movil(attrs, "rowspan")
                        .and_then(|x| x.parse().ok())
                        .unwrap_or(1),
                });
                p = c.map_or(fila.len(), |c| c + cierre.len());
            }
            if !celdas.is_empty() {
                filas.push(celdas);
            }
            pos = cierra.map_or(texto.len(), |c| c + 5);
        }
        filas
    }

    fn movil(
        texto: &str,
        cabecera: bool,
        alineacion: &'static str,
        colspan: usize,
        rowspan: usize,
    ) -> DelMovil {
        DelMovil {
            texto: texto.into(),
            cabecera,
            alineacion,
            colspan,
            rowspan,
        }
    }

    /// La tabla de la muestra: presupuesto con titulo combinado a lo
    /// ancho, una partida combinada a lo alto, colores y un `<` escapado.
    fn presupuesto() -> Tabla {
        let mut t = leer_gfm(
            "| Presupuesto | | |\n|:---|:---|---:|\n| Obra gruesa | Hormigon | 3,20 |\n|  | Acero <12 mm> & alambre | 1,10 |\n| **Total** |  | 4,30 |",
        )
        .unwrap();
        t.combinar((0, 0), (0, 2));
        t.formato_mut(0, 0).alineacion = Some(Alineacion::Centro);
        t.combinar((1, 0), (2, 0));
        t.poner_fondo((1, 0), (1, 0), Some(0xffc9c9));
        t.poner_letra((1, 0), (1, 0), Some(0xe03131));
        t.poner_fondo((3, 0), (3, 2), Some(0xffec99));
        t
    }

    #[test]
    fn lo_que_escribe_el_pc_lo_lee_el_movil_con_sus_combinadas() {
        let html = a_html(&presupuesto());
        // `Markdown.kt` empieza en el renglon que empieza por `<table` y
        // acaba en el que lleva `</table>`.
        assert!(html.lines().next().unwrap().trim().starts_with("<table"));
        assert!(html.lines().last().unwrap().contains("</table>"));
        let leido = lee_como_el_movil(&html);
        assert_eq!(
            leido,
            vec![
                vec![movil("Presupuesto", true, "centro", 3, 1)],
                vec![
                    movil("Obra gruesa", false, "izquierda", 1, 2),
                    movil("Hormigon", false, "izquierda", 1, 1),
                    movil("3,20", false, "derecha", 1, 1),
                ],
                vec![
                    movil("Acero <12 mm> & alambre", false, "izquierda", 1, 1),
                    movil("1,10", false, "derecha", 1, 1),
                ],
                vec![
                    movil("**Total**", false, "izquierda", 1, 1),
                    movil("", false, "izquierda", 1, 1),
                    movil("4,30", false, "derecha", 1, 1),
                ],
            ]
        );
        assert!(html.contains(
            "<td rowspan=\"2\" style=\"background:#ffc9c9;color:#e03131\">Obra gruesa</td>"
        ));
    }

    #[test]
    fn la_tabla_del_pc_vuelve_igual_de_su_html() {
        let t = presupuesto();
        let html = a_html(&t);
        let vuelta = leer_de_nota(&html).unwrap();
        // La misma tabla dicha de la forma unica (la alineacion de la
        // combinada de arriba pasa a ser la de sus tres columnas).
        let mut unica = t.clone();
        unica.canonica();
        assert_eq!(vuelta, unica);
        assert_eq!(unica.alineaciones, vec![Alineacion::Centro; 3]);
        assert_eq!(a_html(&vuelta), html);
        assert_eq!(a_texto(&t), html, "con combinadas se escribe en HTML");
    }

    #[test]
    fn el_html_que_escribe_el_movil_se_abre_y_se_escribe_igual() {
        // Tal cual `Tablas.aHtml`: cabecera en la primera columna, una
        // celda abajo y una combinada.
        let del_movil = "<table>\n  <tr>\n    <th>Fase</th>\n    <th align=\"center\">Dias</th>\n  </tr>\n  <tr>\n    <th>Excavar</th>\n    <td align=\"center\" valign=\"bottom\">3</td>\n  </tr>\n  <tr>\n    <td colspan=\"2\">Sin lluvia &amp; con permiso</td>\n  </tr>\n</table>";
        let t = leer_de_nota(del_movil).unwrap();
        assert_eq!(t.filas[2][0], "Sin lluvia & con permiso");
        assert!(t.es_cabecera(1, 0));
        assert_eq!(t.formato(1, 1).vertical, Vertical::Abajo);
        assert_eq!(t.alineacion(1), Alineacion::Centro);
        assert!(t.formato(2, 1).tapada);
        assert_eq!(a_html(&t), del_movil);
    }

    #[test]
    fn una_tabla_html_sin_nada_especial_se_guarda_con_barras() {
        let t = leer_de_nota("<table>\n  <tr>\n    <th>a</th>\n    <th align=\"right\">b</th>\n  </tr>\n  <tr>\n    <td>1</td>\n    <td align=\"right\">2</td>\n  </tr>\n</table>").unwrap();
        assert!(!t.es_avanzada());
        assert_eq!(a_texto(&t), "| a | b |\n|:---|---:|\n| 1 | 2 |");
    }

    #[test]
    fn lo_que_no_es_una_tabla_entera_no_se_lee_de_la_nota() {
        assert!(
            leer_de_nota("<table><caption>Plan</caption><tr><td>a</td></tr></table>").is_none()
        );
        assert!(leer_de_nota("<table><tr><td>a</td></tr></table> y texto").is_none());
        assert!(leer_de_nota("<table></table>").is_none());
        assert!(leer_de_nota("<p>hola</p>").is_none());
    }

    #[test]
    fn los_colores_se_entienden_como_los_escribe_cada_uno() {
        assert_eq!(color("#FFC000"), Some(0xffc000));
        assert_eq!(color("#fc0"), Some(0xffcc00));
        assert_eq!(color("rgb(255, 0, 0)"), Some(0xff0000));
        assert_eq!(color("#4472C4 none"), Some(0x4472c4));
        assert_eq!(color("yellow"), Some(0xffff00));
        assert_eq!(color("red !important"), Some(0xff0000));
        // Lo que no es un color de verdad.
        for v in [
            "transparent",
            "windowtext",
            "auto",
            "#12",
            "rgba(0,0,0,0)",
            "",
            "inherit",
        ] {
            assert_eq!(color(v), None, "{v}");
        }
        assert_eq!(hex(0xa5d8ff), "#a5d8ff");
    }

    // Lo que dejan en el portapapeles, recortado de copias de verdad.

    const DE_SHEETS: &str = r#"<meta charset="utf-8"><google-sheets-html-origin><style type="text/css"><!--td {border: 1px solid #cccccc;}br {mso-data-placement:same-cell;}--></style><table xmlns="http://www.w3.org/1999/xhtml" cellspacing="0" cellpadding="0" dir="ltr" border="1" style="table-layout:fixed;font-size:10pt;font-family:Arial;width:0px;border-collapse:collapse;border:none" data-sheets-root="1"><colgroup><col width="100"/><col width="100"/><col width="100"/></colgroup><tbody><tr style="height:21px;"><td style="overflow:hidden;padding:2px 3px 2px 3px;vertical-align:bottom;background-color:#4a86e8;font-weight:bold;color:#ffffff;text-align:center;" rowspan="1" colspan="3" data-sheets-value="{&quot;1&quot;:2,&quot;2&quot;:&quot;MENSUALIDAD RUBY&quot;}">MENSUALIDAD RUBY</td></tr><tr style="height:21px;"><td style="overflow:hidden;padding:2px 3px 2px 3px;vertical-align:bottom;" rowspan="2" colspan="1">Enero</td><td style="overflow:hidden;padding:2px 3px 2px 3px;vertical-align:bottom;">Luz</td><td style="overflow:hidden;padding:2px 3px 2px 3px;vertical-align:bottom;text-align:right;color:#000000;" data-sheets-value="{&quot;1&quot;:3,&quot;3&quot;:120}">120</td></tr><tr style="height:21px;"><td style="overflow:hidden;padding:2px 3px 2px 3px;vertical-align:bottom;">Agua</td><td style="overflow:hidden;padding:2px 3px 2px 3px;vertical-align:bottom;text-align:right;background-color:#ffffff;" data-sheets-value="{&quot;1&quot;:3,&quot;3&quot;:45}">45</td></tr><tr style="height:21px;"><td></td><td></td><td></td></tr></tbody></table>"#;

    #[test]
    fn pegar_de_sheets_trae_sus_combinadas_negrita_y_colores() {
        let t = leer_pegado(DE_SHEETS, None).unwrap();
        assert_eq!(t.filas.len(), 3, "la fila vacia del final fuera");
        assert_eq!(t.columnas(), 3);
        assert_eq!(t.filas[0][0], "**MENSUALIDAD RUBY**");
        let titulo = t.formato(0, 0);
        assert_eq!((titulo.columnas, titulo.filas), (3, 1));
        assert_eq!(titulo.fondo, Some(0x4a86e8));
        assert_eq!(titulo.letra, Some(0xffffff));
        assert!(t.formato(0, 1).tapada && t.formato(0, 2).tapada);
        assert_eq!(t.formato(1, 0).filas, 2);
        assert!(t.formato(2, 0).tapada);
        assert_eq!(t.filas[2][1], "Agua");
        assert_eq!(t.filas[2][2], "45");
        // El negro de la letra y el blanco del fondo son «sin color».
        assert_eq!(t.formato(1, 2).letra, None);
        assert_eq!(t.formato(2, 2).fondo, None);
        // `vertical-align:bottom` lo ponen en todas: no cuenta.
        assert_eq!(t.formato(1, 1).vertical, Vertical::Arriba);
        assert_eq!(t.alineacion(2), Alineacion::Derecha);
        assert!(t.es_avanzada());
    }

    #[test]
    fn pegar_de_excel_lee_el_estilo_de_sus_clases() {
        let entero = "<html xmlns:x=\"urn:schemas-microsoft-com:office:excel\"><head><style><!--table\n\t{mso-displayed-decimal-separator:\"\\,\";}\ntd\n\t{color:black;font-weight:400;}\n.xl65\n\t{mso-style-parent:style0;\n\tcolor:white;\n\tfont-weight:700;\n\tbackground:#C00000;\n\tmso-pattern:black none;}\n.xl66\n\t{mso-style-parent:style0;\n\tbackground:#FFC000;}\n--></style></head><body><table><!--StartFragment--><col width=80><tr height=20><td class=xl65 height=20 width=80>Partida</td><td class=xl65 width=80>Importe</td></tr><tr height=20><td height=20>Arena</td><td align=right x:num>12,5</td></tr><tr height=20><td class=xl66 height=20>Grava</td><td align=right x:num>8</td></tr><!--EndFragment--></table></body></html>";
        let fragmento = "<col width=80><tr height=20><td class=xl65 height=20 width=80>Partida</td><td class=xl65 width=80>Importe</td></tr><tr height=20><td height=20>Arena</td><td align=right x:num>12,5</td></tr><tr height=20><td class=xl66 height=20>Grava</td><td align=right x:num>8</td></tr>";
        let t = leer_pegado(fragmento, Some(entero)).unwrap();
        assert_eq!(t.filas[0], vec!["**Partida**", "**Importe**"]);
        assert_eq!(t.formato(0, 0).fondo, Some(0xc00000));
        assert_eq!(t.formato(0, 0).letra, Some(0xffffff));
        assert_eq!(t.formato(2, 0).fondo, Some(0xffc000));
        assert_eq!(t.formato(1, 0).fondo, None);
        assert_eq!(
            t.alineaciones,
            vec![Alineacion::Izquierda, Alineacion::Derecha]
        );
        assert!(
            t.formato.iter().flatten().all(|x| x.alineacion.is_none()),
            "una alineacion por columna"
        );
    }

    #[test]
    fn pegar_una_tabla_sin_colores_ni_combinadas_se_guarda_con_barras() {
        let html =
            "<table><tr><td>a</td><td>b</td></tr><tr><td>1</td><td align=right>2</td></tr></table>";
        let t = leer_pegado(html, None).unwrap();
        assert!(!t.es_avanzada());
        assert_eq!(a_texto(&t), "| a | b |\n|:---|---:|\n| 1 | 2 |");
    }

    #[test]
    fn pegar_de_libreoffice_y_de_word() {
        let lo = "<table><tr><td bgcolor=\"#FFFF00\" align=\"center\"><b><font color=\"#FF0000\">Ojo</font></b></td><td>x</td></tr><tr><td>1</td><td>2</td></tr></table>";
        let t = leer_pegado(lo, None).unwrap();
        assert_eq!(t.filas[0][0], "**Ojo**");
        assert_eq!(t.formato(0, 0).fondo, Some(0xffff00));
        assert_eq!(t.formato(0, 0).letra, Some(0xff0000));
        let word = "<table class=MsoTableGrid><tr><td width=200 style='background:yellow;padding:0cm'><p class=MsoNormal align=center><span style='color:#C00000'>Riesgo</span><o:p></o:p></p></td><td><p class=MsoNormal>Alto</p></td></tr><tr><td><p class=MsoNormal>a&nbsp;b</p></td><td><p class=MsoNormal><i>c</i></p></td></tr></table><p class=MsoNormal><o:p>&nbsp;</o:p></p>";
        let t = leer_pegado(word, None).unwrap();
        assert_eq!(t.filas[0][0], "Riesgo");
        assert_eq!(t.formato(0, 0).fondo, Some(0xffff00));
        assert_eq!(t.formato(0, 0).letra, Some(0xc00000));
        assert_eq!(t.filas[1][0], "a b");
        assert_eq!(t.filas[1][1], "*c*");
    }

    #[test]
    fn no_se_pega_como_tabla_lo_que_no_lo_es() {
        // Una sola celda: se pega como texto.
        assert!(leer_pegado("<table><tr><td>12</td></tr></table>", None).is_none());
        // Media pagina con una tabla dentro: tambien, que se perderia lo demas.
        assert!(
            leer_pegado(
                "<p>Antes</p><table><tr><td>a</td><td>b</td></tr></table>",
                None
            )
            .is_none()
        );
        assert!(leer_pegado("<b>sin tabla</b>", None).is_none());
        assert!(leer_pegado("", None).is_none());
    }

    #[test]
    fn el_texto_con_tabuladores_es_tabla_si_es_una_rejilla() {
        let t = leer_tsv("a\tb\r\n1\t\"dos\tlineas\"\"\"\r\n").unwrap();
        assert_eq!(t.filas, vec![vec!["a", "b"], vec!["1", "dos lineas\""]]);
        assert!(!t.es_avanzada());
    }

    #[test]
    fn el_texto_con_algun_tabulador_suelto_no_es_tabla() {
        assert!(leer_tsv("fn a() {\n\tx\n}").is_none());
        assert!(leer_tsv("a\tb\tc\n1\t2").is_none(), "columnas desiguales");
        assert!(leer_tsv("solo\ttres\tcosas").is_none(), "una fila");
        assert!(leer_tsv("uno\ndos").is_none(), "una columna");
        assert!(leer_tsv("").is_none());
    }
}
