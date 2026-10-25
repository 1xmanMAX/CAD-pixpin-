//! **SVG**: exportar la [`Escena`] (una capa de Inkscape por capa, los
//! arcos y circulos exactos, los textos como `<text>`) e importar un SVG
//! (caminos con todas sus ordenes, lineas, polilineas, poligonos,
//! rectangulos, circulos, elipses y textos, con sus `transform` y su
//! estilo heredado). Al importar, las capas salen de las capas de Inkscape
//! o, si no hay, del color del trazo.

use std::fmt::Write as _;

use crate::formatos::escena::{Escena, Pieza};
use crate::formatos::pdf::exportar::{MAYUS_HELVETICA, largo_helvetica, num};
use crate::formatos::{Figura, Geometria, Importado, Rgb, Rotulo, bezier};
use crate::geo::{self, Caja, Forma, P};

const INKSCAPE: &str = "http://www.inkscape.org/namespaces/inkscape";
/// Cuantos elementos se importan como mucho.
const TOPE_PIEZAS: usize = 200_000;

fn hex(c: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

fn escapar(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            c if c.is_control() => {}
            c => o.push(c),
        }
    }
    o
}

/// **Escribe el SVG** de lo que cae en `zona`. Una unidad del dibujo es una
/// unidad del SVG; `mm_por_unidad` da el tamano en mm del papel (1 si el
/// dibujo esta en mm) y `unidades_por_mm_de_grosor` pasa los grosores de
/// papel a unidades (la escala de la lamina).
pub fn escribir(
    escena: &Escena,
    zona: Caja,
    mm_por_unidad: f64,
    unidades_por_mm_de_grosor: f64,
    monocromo: bool,
) -> String {
    let (w, h) = ((zona[2] - zona[0]).max(1e-9), (zona[3] - zona[1]).max(1e-9));
    let p = |q: P| [q[0] - zona[0], zona[3] - q[1]];
    let mut s = String::new();
    let _ = writeln!(s, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    let _ = writeln!(
        s,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:inkscape=\"{INKSCAPE}\" version=\"1.1\" width=\"{}mm\" height=\"{}mm\" viewBox=\"0 0 {} {}\">",
        num(w * mm_por_unidad),
        num(h * mm_por_unidad),
        num(w),
        num(h)
    );
    let _ = writeln!(s, "<!-- CAD-pixpin -->");
    for (i, capa) in escena.capas().iter().enumerate() {
        let _ = writeln!(
            s,
            "<g id=\"capa{i}\" inkscape:groupmode=\"layer\" inkscape:label=\"{}\" fill=\"none\" stroke-linecap=\"round\" stroke-linejoin=\"round\">",
            escapar(capa)
        );
        for e in escena.elementos.iter().filter(|e| e.capa == *capa) {
            if !geo::se_tocan(&crate::formatos::escena::caja_de_pieza(&e.pieza), &zona) {
                continue;
            }
            let color = hex(if monocromo { [0, 0, 0] } else { e.color });
            let ancho = num(e.grosor * unidades_por_mm_de_grosor);
            match &e.pieza {
                Pieza::Formas(fs) => {
                    let mut d = String::new();
                    let mut pluma: Option<P> = None;
                    for f in fs {
                        match *f {
                            Forma::Circulo { c, r } => {
                                let c = p(c);
                                let _ = writeln!(
                                    s,
                                    "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" stroke=\"{color}\" stroke-width=\"{ancho}\"/>",
                                    num(c[0]),
                                    num(c[1]),
                                    num(r)
                                );
                            }
                            Forma::Segmento { a, b } => {
                                camino_a(&mut d, &mut pluma, p(a));
                                let b = p(b);
                                let _ = write!(d, "L{} {}", num(b[0]), num(b[1]));
                                pluma = Some(b);
                            }
                            Forma::Arco {
                                c,
                                r,
                                inicio,
                                barrido,
                            } => {
                                // En dos mitades si pasa de media vuelta casi entera.
                                let partes = if barrido > std::f64::consts::PI * 1.9 {
                                    2
                                } else {
                                    1
                                };
                                let paso = barrido / partes as f64;
                                camino_a(&mut d, &mut pluma, p(geo::polar(c, r, inicio)));
                                for k in 0..partes {
                                    let fin = p(geo::polar(c, r, inicio + paso * (k + 1) as f64));
                                    let grande = if paso > std::f64::consts::PI { 1 } else { 0 };
                                    let _ = write!(
                                        d,
                                        "A{} {} 0 {grande} 0 {} {}",
                                        num(r),
                                        num(r),
                                        num(fin[0]),
                                        num(fin[1])
                                    );
                                    pluma = Some(fin);
                                }
                            }
                        }
                    }
                    if !d.is_empty() {
                        let _ = writeln!(
                            s,
                            "<path d=\"{d}\" stroke=\"{color}\" stroke-width=\"{ancho}\"/>"
                        );
                    }
                }
                Pieza::Relleno(anillos) => {
                    let mut d = String::new();
                    for an in anillos {
                        for (k, q) in an.iter().enumerate() {
                            let q = p(*q);
                            let _ = write!(
                                d,
                                "{}{} {}",
                                if k == 0 { "M" } else { "L" },
                                num(q[0]),
                                num(q[1])
                            );
                        }
                        d.push('Z');
                    }
                    let _ = writeln!(
                        s,
                        "<path d=\"{d}\" fill=\"{color}\" fill-rule=\"evenodd\" stroke=\"none\"/>"
                    );
                }
                Pieza::Texto(t) => {
                    let o = p(t.origen(largo_helvetica(&t.texto)));
                    let (sn, cs) = t.giro.sin_cos();
                    let _ = writeln!(
                        s,
                        "<text transform=\"matrix({} {} {} {} {} {})\" font-family=\"Arial, Helvetica, sans-serif\" font-size=\"{}\" fill=\"{color}\" stroke=\"none\" xml:space=\"preserve\">{}</text>",
                        num(cs * t.ancho),
                        num(-sn * t.ancho),
                        num(sn),
                        num(cs),
                        num(o[0]),
                        num(o[1]),
                        num(t.alto / MAYUS_HELVETICA),
                        escapar(&t.texto)
                    );
                }
            }
        }
        let _ = writeln!(s, "</g>");
    }
    s.push_str("</svg>\n");
    s
}

fn camino_a(d: &mut String, pluma: &mut Option<P>, a: P) {
    if pluma.is_none_or(|q| geo::dist(q, a) > 1e-9) {
        let _ = write!(d, "M{} {}", num(a[0]), num(a[1]));
        *pluma = Some(a);
    }
}

// ----------------------------------------------------------------- importar

/// Una matriz `[a b c d e f]` como las de SVG.
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

fn aplicar(m: &M, p: P) -> P {
    [
        m[0] * p[0] + m[2] * p[1] + m[4],
        m[1] * p[0] + m[3] * p[1] + m[5],
    ]
}

/// Los numeros de un texto (`10,5 -3e2.5`).
fn numeros(s: &str) -> Vec<f64> {
    let mut v = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if let Some((n, fin)) = leer_numero(b, i) {
            v.push(n);
            i = fin;
        } else {
            i += 1;
        }
    }
    v
}

/// Un numero desde `i` (salta blancos y comas antes).
fn leer_numero(b: &[u8], mut i: usize) -> Option<(f64, usize)> {
    while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b',') {
        i += 1;
    }
    let ini = i;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let mut digitos = false;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
        digitos = true;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
            digitos = true;
        }
    }
    if !digitos {
        return None;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        if j < b.len() && b[j].is_ascii_digit() {
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            i = j;
        }
    }
    let n: f64 = std::str::from_utf8(&b[ini..i]).ok()?.parse().ok()?;
    n.is_finite().then_some((n, i))
}

/// **Un `transform`** de SVG.
fn transformacion(s: &str) -> M {
    let mut m = ID;
    for parte in s.split(')') {
        let Some((nombre, args)) = parte.split_once('(') else {
            continue;
        };
        let a = numeros(args);
        let g = |i: usize| a.get(i).copied().unwrap_or(0.0);
        let t: M = match nombre.trim().trim_start_matches(',').trim() {
            "matrix" if a.len() >= 6 => [g(0), g(1), g(2), g(3), g(4), g(5)],
            "translate" => [1.0, 0.0, 0.0, 1.0, g(0), g(1)],
            "scale" => {
                let sx = a.first().copied().unwrap_or(1.0);
                let sy = a.get(1).copied().unwrap_or(sx);
                [sx, 0.0, 0.0, sy, 0.0, 0.0]
            }
            "rotate" => {
                let (sn, cs) = g(0).to_radians().sin_cos();
                let r = [cs, sn, -sn, cs, 0.0, 0.0];
                if a.len() >= 3 {
                    let (cx, cy) = (g(1), g(2));
                    mul(
                        &mul(&[1.0, 0.0, 0.0, 1.0, -cx, -cy], &r),
                        &[1.0, 0.0, 0.0, 1.0, cx, cy],
                    )
                } else {
                    r
                }
            }
            "skewX" => [1.0, 0.0, g(0).to_radians().tan(), 1.0, 0.0, 0.0],
            "skewY" => [1.0, g(0).to_radians().tan(), 0.0, 1.0, 0.0, 0.0],
            _ => ID,
        };
        // Lo de la derecha se aplica primero.
        m = mul(&t, &m);
    }
    m
}

/// Un largo con unidades, en mm (`210mm`, `8.5in`, `100` = px).
fn largo_mm(s: &str) -> Option<f64> {
    let s = s.trim();
    let fin = s
        .find(|c: char| c.is_ascii_alphabetic() || c == '%')
        .unwrap_or(s.len());
    let v: f64 = s[..fin].trim().parse().ok()?;
    let k = match s[fin..].trim() {
        "mm" => 1.0,
        "cm" => 10.0,
        "in" => 25.4,
        "pt" => 25.4 / 72.0,
        "pc" => 25.4 / 6.0,
        "" | "px" => 25.4 / 96.0,
        _ => return None,
    };
    (v.is_finite() && v > 0.0).then_some(v * k)
}

/// Un color de SVG (`#f00`, `#ff0000`, `rgb(255,0,0)`, `red`). `Some(None)`
/// es `none`.
fn color(s: &str, actual: Rgb) -> Option<Option<Rgb>> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("none") || s.eq_ignore_ascii_case("transparent") {
        return Some(None);
    }
    if s.eq_ignore_ascii_case("currentColor") {
        return Some(Some(actual));
    }
    if s.starts_with("url(") {
        return Some(Some([128, 128, 128]));
    }
    if let Some(h) = s.strip_prefix('#') {
        let v: Vec<u8> = h
            .bytes()
            .filter_map(|c| (c as char).to_digit(16).map(|d| d as u8))
            .collect();
        return match v.len() {
            3 => Some(Some([v[0] * 17, v[1] * 17, v[2] * 17])),
            6 => Some(Some([v[0] * 16 + v[1], v[2] * 16 + v[3], v[4] * 16 + v[5]])),
            _ => None,
        };
    }
    if let Some(r) = s.strip_prefix("rgb(") {
        let pct = r.contains('%');
        let n = numeros(r);
        if n.len() >= 3 {
            let c = |v: f64| (if pct { v * 2.55 } else { v }).clamp(0.0, 255.0).round() as u8;
            return Some(Some([c(n[0]), c(n[1]), c(n[2])]));
        }
        return None;
    }
    const NOMBRES: &[(&str, Rgb)] = &[
        ("black", [0, 0, 0]),
        ("white", [255, 255, 255]),
        ("red", [255, 0, 0]),
        ("lime", [0, 255, 0]),
        ("green", [0, 128, 0]),
        ("blue", [0, 0, 255]),
        ("yellow", [255, 255, 0]),
        ("cyan", [0, 255, 255]),
        ("aqua", [0, 255, 255]),
        ("magenta", [255, 0, 255]),
        ("fuchsia", [255, 0, 255]),
        ("gray", [128, 128, 128]),
        ("grey", [128, 128, 128]),
        ("silver", [192, 192, 192]),
        ("maroon", [128, 0, 0]),
        ("olive", [128, 128, 0]),
        ("navy", [0, 0, 128]),
        ("purple", [128, 0, 128]),
        ("teal", [0, 128, 128]),
        ("orange", [255, 165, 0]),
        ("brown", [165, 42, 42]),
    ];
    NOMBRES
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(s))
        .map(|(_, c)| Some(*c))
}

/// El estilo que se hereda.
#[derive(Debug, Clone)]
struct Estilo {
    m: M,
    trazo: Option<Rgb>,
    relleno: Option<Rgb>,
    grosor: f64,
    tam_letra: f64,
    actual: Rgb,
    capa: Option<String>,
}

/// Lo que dice el elemento de una propiedad (atributo o `style`).
fn propiedad<'a>(n: roxmltree::Node<'a, 'a>, nombre: &str) -> Option<String> {
    if let Some(st) = n.attribute("style") {
        for decl in st.split(';') {
            if let Some((k, v)) = decl.split_once(':')
                && k.trim() == nombre
            {
                return Some(v.trim().to_string());
            }
        }
    }
    n.attribute(nombre).map(str::to_string)
}

struct Lector<'a> {
    doc: &'a roxmltree::Document<'a>,
    out: Importado,
    imagenes: usize,
    cortado: bool,
}

/// **Importa un SVG** de un texto.
pub fn importar(texto: &str) -> Result<Importado, String> {
    let opciones = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    let doc = roxmltree::Document::parse_with_options(texto, opciones)
        .map_err(|e| format!("el SVG esta mal formado: {e}"))?;
    let raiz = doc.root_element();
    if raiz.tag_name().name() != "svg" {
        return Err("no es un SVG".into());
    }
    // Las unidades: el ancho en mm entre el ancho del viewBox.
    let vb = raiz
        .attribute("viewBox")
        .map(numeros)
        .filter(|v| v.len() == 4);
    let ancho = raiz.attribute("width").and_then(largo_mm);
    let alto = raiz.attribute("height").and_then(largo_mm);
    let (sx, sy, vx, vy, alto_mm, ancho_mm) = match &vb {
        Some(v) if v[2] > 0.0 && v[3] > 0.0 => {
            let sx = ancho.map_or(25.4 / 96.0, |w| w / v[2]);
            let sy = alto.map_or(sx, |h| h / v[3]);
            (sx, sy, v[0], v[1], v[3] * sy, v[2] * sx)
        }
        _ => {
            let k = 25.4 / 96.0;
            (k, k, 0.0, 0.0, alto.unwrap_or(0.0), ancho.unwrap_or(0.0))
        }
    };
    // Del SVG (y hacia abajo) a mm de papel con la y hacia arriba.
    let base: M = [sx, 0.0, 0.0, -sy, -vx * sx, alto_mm + vy * sy];
    let mut l = Lector {
        doc: &doc,
        out: Importado {
            pagina: (ancho_mm, alto_mm),
            ..Default::default()
        },
        imagenes: 0,
        cortado: false,
    };
    let est = Estilo {
        m: base,
        trazo: None,
        relleno: Some([0, 0, 0]),
        grosor: 1.0,
        tam_letra: 16.0,
        actual: [0, 0, 0],
        capa: None,
    };
    l.nodo(raiz, &est, 0);
    if l.imagenes > 0 {
        l.out
            .avisos
            .push(format!("{} imagen(es) no importada(s)", l.imagenes));
    }
    if l.cortado {
        l.out.avisos.push(format!(
            "el SVG es muy grande: se importaron {TOPE_PIEZAS} piezas"
        ));
    }
    Ok(l.out)
}

impl<'a> Lector<'a> {
    fn estilo(&self, n: roxmltree::Node<'a, 'a>, padre: &Estilo) -> Option<Estilo> {
        let mut e = padre.clone();
        if propiedad(n, "display").as_deref() == Some("none")
            || propiedad(n, "visibility").as_deref() == Some("hidden")
        {
            return None;
        }
        if let Some(t) = n.attribute("transform") {
            e.m = mul(&transformacion(t), &e.m);
        }
        if let Some(c) = propiedad(n, "color")
            .and_then(|c| color(&c, e.actual))
            .flatten()
        {
            e.actual = c;
        }
        if let Some(c) = propiedad(n, "stroke").and_then(|c| color(&c, e.actual)) {
            e.trazo = c;
        }
        if let Some(c) = propiedad(n, "fill").and_then(|c| color(&c, e.actual)) {
            e.relleno = c;
        }
        if let Some(w) = propiedad(n, "stroke-width").and_then(|w| numeros(&w).first().copied()) {
            e.grosor = w.abs();
        }
        if let Some(t) = propiedad(n, "font-size").and_then(|w| numeros(&w).first().copied()) {
            e.tam_letra = t.abs();
        }
        if n.tag_name().name() == "g"
            && n.attribute((INKSCAPE, "groupmode")) == Some("layer")
            && let Some(nombre) = n.attribute((INKSCAPE, "label")).or(n.attribute("id"))
        {
            e.capa = Some(nombre.to_string());
        }
        Some(e)
    }

    fn nodo(&mut self, n: roxmltree::Node<'a, 'a>, padre: &Estilo, prof: u32) {
        if prof > 64 || self.cortado {
            return;
        }
        let Some(est) = self.estilo(n, padre) else {
            return;
        };
        let f = |k: &str| n.attribute(k).and_then(|v| numeros(v).first().copied());
        let num0 = |k: &str| f(k).unwrap_or(0.0);
        match n.tag_name().name() {
            "svg" | "g" | "a" | "switch" => {
                for h in n.children().filter(|h| h.is_element()) {
                    self.nodo(h, &est, prof + 1);
                }
            }
            "use" => {
                let href = n
                    .attribute(("http://www.w3.org/1999/xlink", "href"))
                    .or(n.attribute("href"))
                    .and_then(|h| h.strip_prefix('#'));
                if let Some(id) = href
                    && prof < 16
                    && let Some(destino) = self
                        .doc
                        .descendants()
                        .find(|d| d.attribute("id") == Some(id))
                {
                    let mut e = est.clone();
                    e.m = mul(&[1.0, 0.0, 0.0, 1.0, num0("x"), num0("y")], &e.m);
                    if destino.tag_name().name() == "symbol" {
                        for h in destino.children().filter(|h| h.is_element()) {
                            self.nodo(h, &e, prof + 1);
                        }
                    } else {
                        self.nodo(destino, &e, prof + 1);
                    }
                }
            }
            "path" => {
                let subs = camino(n.attribute("d").unwrap_or(""), &est.m);
                self.poner(subs, &est);
            }
            "line" => {
                let a = aplicar(&est.m, [num0("x1"), num0("y1")]);
                let b = aplicar(&est.m, [num0("x2"), num0("y2")]);
                let mut e = est.clone();
                e.relleno = None;
                self.poner(vec![(vec![a, b], false)], &e);
            }
            "polyline" | "polygon" => {
                let v = numeros(n.attribute("points").unwrap_or(""));
                let pts: Vec<P> = v
                    .chunks_exact(2)
                    .map(|c| aplicar(&est.m, [c[0], c[1]]))
                    .collect();
                self.poner(vec![(pts, n.tag_name().name() == "polygon")], &est);
            }
            "rect" => {
                let (x, y, w, h) = (num0("x"), num0("y"), num0("width"), num0("height"));
                if w > 0.0 && h > 0.0 {
                    let pts = [[x, y], [x + w, y], [x + w, y + h], [x, y + h]]
                        .iter()
                        .map(|p| aplicar(&est.m, *p))
                        .collect();
                    self.poner(vec![(pts, true)], &est);
                }
            }
            "circle" | "ellipse" => {
                let (cx, cy) = (num0("cx"), num0("cy"));
                let (rx, ry) = if n.tag_name().name() == "circle" {
                    (num0("r"), num0("r"))
                } else {
                    (num0("rx"), num0("ry"))
                };
                if rx <= 0.0 || ry <= 0.0 {
                    return;
                }
                let m = est.m;
                let semejanza = (m[0] * m[0] + m[1] * m[1] - m[2] * m[2] - m[3] * m[3]).abs()
                    < 1e-9 * (m[0].abs() + m[1].abs() + 1.0)
                    && (m[0] * m[2] + m[1] * m[3]).abs() < 1e-9 * (m[0].abs() + m[1].abs() + 1.0);
                if (rx - ry).abs() < 1e-12 && semejanza {
                    let k = (m[0] * m[3] - m[1] * m[2]).abs().sqrt();
                    let c = aplicar(&m, [cx, cy]);
                    self.figura(vec![Geometria::Circulo { c, r: rx * k }], &est);
                } else {
                    let pts = (0..72)
                        .map(|i| {
                            let t = std::f64::consts::TAU * i as f64 / 72.0;
                            aplicar(&m, [cx + rx * t.cos(), cy + ry * t.sin()])
                        })
                        .collect();
                    self.poner(vec![(pts, true)], &est);
                }
            }
            "text" => self.texto(n, &est),
            "image" => self.imagenes += 1,
            _ => {}
        }
    }

    fn texto(&mut self, n: roxmltree::Node<'a, 'a>, est: &Estilo) {
        let contenido: String = n
            .descendants()
            .filter(|d| d.is_text())
            .filter_map(|d| d.text())
            .collect::<Vec<_>>()
            .join("");
        let contenido = contenido.split_whitespace().collect::<Vec<_>>().join(" ");
        if contenido.is_empty() {
            return;
        }
        // La posicion: la del texto o la del primer `tspan`.
        let pos = |d: roxmltree::Node| -> Option<P> {
            let x = d.attribute("x").and_then(|v| numeros(v).first().copied())?;
            let y = d.attribute("y").and_then(|v| numeros(v).first().copied())?;
            Some([x, y])
        };
        let q = pos(n)
            .or_else(|| n.descendants().find_map(pos))
            .unwrap_or([0.0, 0.0]);
        let m = est.m;
        let p = aplicar(&m, q);
        let k = (m[0] * m[3] - m[1] * m[2]).abs().sqrt();
        let tam = est.tam_letra * k;
        if tam <= 0.0 {
            return;
        }
        // La matriz ya da la vuelta a la y: su eje x es la direccion del texto.
        let giro = m[1].atan2(m[0]);
        let ancho = geo::norma([m[0], m[1]]) / geo::norma([m[2], m[3]]).max(1e-12);
        self.out.rotulos.push(Rotulo {
            texto: contenido,
            p,
            alto: tam * MAYUS_HELVETICA,
            giro,
            ancho,
            color: est.relleno.unwrap_or([0, 0, 0]),
            grupo: est.capa.clone(),
        });
        if self.out.figuras.len() + self.out.rotulos.len() >= TOPE_PIEZAS {
            self.cortado = true;
        }
    }

    fn figura(&mut self, partes: Vec<Geometria>, est: &Estilo) {
        if partes.is_empty() {
            return;
        }
        let m = est.m;
        let k = (m[0] * m[3] - m[1] * m[2]).abs().sqrt();
        let trazo = est.trazo.map(|c| (c, est.grosor * k));
        if trazo.is_none() && est.relleno.is_none() {
            return;
        }
        self.out.figuras.push(Figura {
            partes,
            trazo,
            relleno: est.relleno,
            grupo: est.capa.clone(),
        });
        if self.out.figuras.len() + self.out.rotulos.len() >= TOPE_PIEZAS {
            self.cortado = true;
        }
    }

    fn poner(&mut self, subs: Vec<(Vec<P>, bool)>, est: &Estilo) {
        let partes = subs
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
        self.figura(partes, est);
    }
}

/// **Los tramos de un `d`** de SVG ya llevados por `m` (cada uno, sus puntos
/// y si se cierra).
fn camino(d: &str, m: &M) -> Vec<(Vec<P>, bool)> {
    let b = d.as_bytes();
    let mut subs: Vec<(Vec<P>, bool)> = Vec::new();
    let mut actual: Vec<P> = Vec::new();
    // En coordenadas del SVG (sin `m`).
    let mut pluma = [0.0, 0.0];
    let mut inicio = [0.0, 0.0];
    let mut ultimo_control: Option<(u8, P)> = None;
    let mut i = 0;
    let mut orden = 0u8;
    let tol = 0.01;
    let lleva = |p: P| aplicar(m, p);
    let cerrar_sub = |actual: &mut Vec<P>, subs: &mut Vec<(Vec<P>, bool)>, cerrada: bool| {
        if actual.len() > 1 {
            subs.push((std::mem::take(actual), cerrada));
        } else {
            actual.clear();
        }
    };
    loop {
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b',') {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        if b[i].is_ascii_alphabetic() {
            orden = b[i];
            i += 1;
            if matches!(orden, b'Z' | b'z') {
                if !actual.is_empty() {
                    cerrar_sub(&mut actual, &mut subs, true);
                }
                pluma = inicio;
                ultimo_control = None;
                continue;
            }
        } else if orden == 0 {
            break;
        }
        let rel = orden.is_ascii_lowercase();
        let base = if rel { pluma } else { [0.0, 0.0] };
        let leer = |n: usize, i: &mut usize| -> Option<Vec<f64>> {
            let mut v = Vec::with_capacity(n);
            for _ in 0..n {
                let (x, fin) = leer_numero(b, *i)?;
                v.push(x);
                *i = fin;
            }
            Some(v)
        };
        let antes = i;
        match orden.to_ascii_uppercase() {
            b'M' => {
                let Some(v) = leer(2, &mut i) else { break };
                cerrar_sub(&mut actual, &mut subs, false);
                pluma = [base[0] + v[0], base[1] + v[1]];
                inicio = pluma;
                actual.push(lleva(pluma));
                // Lo que sigue a un M sin orden es un L.
                orden = if rel { b'l' } else { b'L' };
                ultimo_control = None;
            }
            b'L' => {
                let Some(v) = leer(2, &mut i) else { break };
                if actual.is_empty() {
                    actual.push(lleva(pluma));
                }
                pluma = [base[0] + v[0], base[1] + v[1]];
                actual.push(lleva(pluma));
                ultimo_control = None;
            }
            b'H' | b'V' => {
                let Some(v) = leer(1, &mut i) else { break };
                if actual.is_empty() {
                    actual.push(lleva(pluma));
                }
                if orden.eq_ignore_ascii_case(&b'H') {
                    pluma[0] = base[0] + v[0];
                } else {
                    pluma[1] = base[1] + v[0];
                }
                actual.push(lleva(pluma));
                ultimo_control = None;
            }
            b'C' | b'S' | b'Q' | b'T' => {
                let up = orden.to_ascii_uppercase();
                let n = match up {
                    b'C' => 6,
                    b'S' | b'Q' => 4,
                    _ => 2,
                };
                let Some(v) = leer(n, &mut i) else { break };
                if actual.is_empty() {
                    actual.push(lleva(pluma));
                }
                let pt = |k: usize| [base[0] + v[k], base[1] + v[k + 1]];
                let reflejo = |tipo: &[u8]| match ultimo_control {
                    Some((t, c)) if tipo.contains(&t) => {
                        [2.0 * pluma[0] - c[0], 2.0 * pluma[1] - c[1]]
                    }
                    _ => pluma,
                };
                let (c1, c2, fin, control) = match up {
                    b'C' => (pt(0), pt(2), pt(4), (b'C', pt(2))),
                    b'S' => (reflejo(b"CS"), pt(0), pt(2), (b'C', pt(0))),
                    b'Q' => {
                        let q = pt(0);
                        let f = pt(2);
                        (
                            [
                                pluma[0] + 2.0 / 3.0 * (q[0] - pluma[0]),
                                pluma[1] + 2.0 / 3.0 * (q[1] - pluma[1]),
                            ],
                            [
                                f[0] + 2.0 / 3.0 * (q[0] - f[0]),
                                f[1] + 2.0 / 3.0 * (q[1] - f[1]),
                            ],
                            f,
                            (b'Q', q),
                        )
                    }
                    _ => {
                        let q = reflejo(b"QT");
                        let f = pt(0);
                        (
                            [
                                pluma[0] + 2.0 / 3.0 * (q[0] - pluma[0]),
                                pluma[1] + 2.0 / 3.0 * (q[1] - pluma[1]),
                            ],
                            [
                                f[0] + 2.0 / 3.0 * (q[0] - f[0]),
                                f[1] + 2.0 / 3.0 * (q[1] - f[1]),
                            ],
                            f,
                            (b'Q', q),
                        )
                    }
                };
                actual.extend(bezier(lleva(pluma), lleva(c1), lleva(c2), lleva(fin), tol));
                pluma = fin;
                ultimo_control = Some(control);
            }
            b'A' => {
                // rx ry giro grande barrido x y (las banderas pueden ir pegadas).
                let Some(v) = leer(3, &mut i) else { break };
                let bandera = |i: &mut usize| -> Option<bool> {
                    while *i < b.len() && (b[*i].is_ascii_whitespace() || b[*i] == b',') {
                        *i += 1;
                    }
                    let c = *b.get(*i)?;
                    *i += 1;
                    match c {
                        b'0' => Some(false),
                        b'1' => Some(true),
                        _ => None,
                    }
                };
                let (Some(grande), Some(barrido)) = (bandera(&mut i), bandera(&mut i)) else {
                    break;
                };
                let Some(f) = leer(2, &mut i) else { break };
                if actual.is_empty() {
                    actual.push(lleva(pluma));
                }
                let fin = [base[0] + f[0], base[1] + f[1]];
                for q in arco_svg(pluma, fin, v[0], v[1], v[2], grande, barrido) {
                    actual.push(lleva(q));
                }
                pluma = fin;
                ultimo_control = None;
            }
            _ => {
                i += 1;
            }
        }
        if i == antes {
            i += 1;
        }
    }
    cerrar_sub(&mut actual, &mut subs, false);
    subs
}

/// Los puntos de un arco de SVG de `a` a `b` (sin `a`), segun la nota de
/// implementacion F.6.5 de la norma.
fn arco_svg(a: P, b: P, rx: f64, ry: f64, giro_g: f64, grande: bool, barrido: bool) -> Vec<P> {
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx < 1e-12 || ry < 1e-12 || geo::dist(a, b) < 1e-12 {
        return vec![b];
    }
    let (sn, cs) = giro_g.to_radians().sin_cos();
    let dx = (a[0] - b[0]) / 2.0;
    let dy = (a[1] - b[1]) / 2.0;
    let x1 = cs * dx + sn * dy;
    let y1 = -sn * dx + cs * dy;
    let l = x1 * x1 / (rx * rx) + y1 * y1 / (ry * ry);
    if l > 1.0 {
        rx *= l.sqrt();
        ry *= l.sqrt();
    }
    let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut k = (num / den).max(0.0).sqrt();
    if grande == barrido {
        k = -k;
    }
    let cx1 = k * rx * y1 / ry;
    let cy1 = -k * ry * x1 / rx;
    let cx = cs * cx1 - sn * cy1 + (a[0] + b[0]) / 2.0;
    let cy = sn * cx1 + cs * cy1 + (a[1] + b[1]) / 2.0;
    let ang = |ux: f64, uy: f64, vx: f64, vy: f64| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    let t1 = ang(1.0, 0.0, (x1 - cx1) / rx, (y1 - cy1) / ry);
    let mut dt = ang(
        (x1 - cx1) / rx,
        (y1 - cy1) / ry,
        (-x1 - cx1) / rx,
        (-y1 - cy1) / ry,
    );
    if !barrido && dt > 0.0 {
        dt -= std::f64::consts::TAU;
    } else if barrido && dt < 0.0 {
        dt += std::f64::consts::TAU;
    }
    let n = ((dt.abs() / 5f64.to_radians()).ceil() as usize).clamp(2, 144);
    let mut v: Vec<P> = (1..=n)
        .map(|k| {
            let t = t1 + dt * k as f64 / n as f64;
            let (x, y) = (rx * t.cos(), ry * t.sin());
            [cs * x - sn * y + cx, sn * x + cs * y + cy]
        })
        .collect();
    if let Some(u) = v.last_mut() {
        *u = b;
    }
    v
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn numeros_pegados() {
        assert_eq!(numeros("1.5.5-2e1,3"), vec![1.5, 0.5, -20.0, 3.0]);
        assert_eq!(largo_mm("210mm"), Some(210.0));
        assert_eq!(largo_mm("96"), Some(25.4));
        assert_eq!(largo_mm("10%"), None);
    }

    #[test]
    fn colores() {
        assert_eq!(color("#f00", [0; 3]), Some(Some([255, 0, 0])));
        assert_eq!(color("rgb(0, 128, 255)", [0; 3]), Some(Some([0, 128, 255])));
        assert_eq!(color("none", [0; 3]), Some(None));
        assert_eq!(color("Navy", [0; 3]), Some(Some([0, 0, 128])));
        assert_eq!(color("raro", [0; 3]), None);
    }

    #[test]
    fn transformaciones_en_orden() {
        let m = transformacion("translate(10,0) scale(2)");
        assert_eq!(aplicar(&m, [1.0, 1.0]), [12.0, 2.0]);
        let r = transformacion("rotate(90 1 1)");
        let p = aplicar(&r, [2.0, 1.0]);
        assert!(geo::dist(p, [1.0, 2.0]) < 1e-9, "{p:?}");
    }

    #[test]
    fn caminos_con_todas_las_ordenes() {
        let s = camino(
            "M0 0 h10 v10 H0 z m20 0 l5 5 5-5 Q 30 10 35 0 T 45 0 a5 5 0 0 1 10 0",
            &ID,
        );
        assert_eq!(s.len(), 2);
        assert!(s[0].1);
        assert_eq!(s[0].0.len(), 4);
        let ultimo = *s[1].0.last().unwrap();
        assert!(geo::dist(ultimo, [55.0, 0.0]) < 1e-9);
        // El arco de media vuelta pasa por arriba o por abajo del eje.
        assert!(s[1].0.iter().any(|p| (p[1].abs() - 5.0).abs() < 0.05));
    }

    #[test]
    fn importa_con_unidades_capas_y_y_hacia_arriba() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" width="100mm" height="50mm" viewBox="0 0 200 100">
            <g inkscape:groupmode="layer" inkscape:label="Muros">
              <line x1="0" y1="0" x2="200" y2="0" stroke="red" stroke-width="2"/>
            </g>
            <circle cx="100" cy="50" r="10" stroke="#00f" fill="none"/>
            <rect x="0" y="0" width="10" height="10" style="fill:#0f0;stroke:none"/>
            <g display="none"><line x1="0" y1="0" x2="1" y2="1" stroke="black"/></g>
            <text x="10" y="90" font-size="10">Hola  mundo</text>
        </svg>"##;
        let imp = importar(svg).unwrap();
        assert_eq!(imp.figuras.len(), 3);
        let f = &imp.figuras[0];
        assert_eq!(f.grupo.as_deref(), Some("Muros"));
        let (c, w) = f.trazo.unwrap();
        assert_eq!(c, [255, 0, 0]);
        assert!((w - 1.0).abs() < 1e-9);
        let Geometria::Polilinea { puntos, .. } = &f.partes[0] else {
            panic!()
        };
        // La y del SVG va hacia abajo: lo de arriba (y=0) queda a 50 mm.
        assert!(geo::dist(puntos[0], [0.0, 50.0]) < 1e-9);
        assert!(geo::dist(puntos[1], [100.0, 50.0]) < 1e-9);
        assert_eq!(
            imp.figuras[1].partes[0],
            Geometria::Circulo {
                c: [50.0, 25.0],
                r: 5.0
            }
        );
        assert_eq!(imp.figuras[2].relleno, Some([0, 255, 0]));
        assert!(imp.figuras[2].trazo.is_none());
        assert_eq!(imp.rotulos.len(), 1);
        assert_eq!(imp.rotulos[0].texto, "Hola mundo");
        assert!(geo::dist(imp.rotulos[0].p, [5.0, 5.0]) < 1e-9);
        assert!(imp.rotulos[0].giro.abs() < 1e-9);
        assert!(importar("<html/>").is_err());
        assert!(importar("<svg").is_err());
    }
}
