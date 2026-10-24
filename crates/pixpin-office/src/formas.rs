//! **Las formas predefinidas de DrawingML** (`a:prstGeom prst="..."`).
//!
//! La tabla (`formas.def`, incluida en el binario) lleva cada forma tal como
//! la define ECMA-376 (parte 1, 20.1.9 y `presetShapeDefinitions.xml`): sus
//! ajustes (`avLst`), sus guias (`gdLst`) y sus trayectos, con `fill="none"`,
//! `stroke="false"` y los rellenos `darken` / `lighten` / `darkenLess` /
//! `lightenLess` de las caras del cubo y del bisel. Las guias se evaluan con
//! el mismo evaluador que la geometria propia (`geometria::evaluar`), asi que
//! los ajustes del archivo (`a:gd name="adj"...`) se respetan y se acotan
//! como manda la especificacion (`pin`).
//!
//! Las estrellas, los engranajes, el sol y la nube no se escriben a mano en
//! la tabla sino que se generan (la especificacion los define con docenas de
//! guias trigonometricas). La nube y su globo de pensamiento siguen la idea
//! del generador de deckcraft.
//!
//! Partes traducidas de deckcraft (https://github.com/storytold/deckcraft), MIT OR Apache-2.0, Copyright (c) 2026 ArtCraft Team and the Deckcraft contributors.

use std::collections::HashMap;
use std::f32::consts::PI;
use std::sync::OnceLock;

use crate::geometria::{arco, elipse, evaluar, variables_base};
use crate::pagina::{Figura, Sombra, Tramo};

/// Un paso de la lista de guias de una forma.
enum Paso {
    Guia(String, String),
    Usa(String),
}

struct Orden {
    op: char,
    args: Vec<String>,
}

struct Trayecto {
    rellenar: bool,
    trazar: bool,
    sombra: Sombra,
    w: Option<String>,
    h: Option<String>,
    ordenes: Vec<Orden>,
}

#[derive(Default)]
struct Def {
    ajustes: Vec<(String, f64)>,
    pasos: Vec<Paso>,
    trayectos: Vec<Trayecto>,
}

static TABLA: OnceLock<HashMap<String, Def>> = OnceLock::new();

fn tabla() -> &'static HashMap<String, Def> {
    TABLA.get_or_init(|| leer_tabla(include_str!("formas.def")))
}

fn leer_tabla(texto: &str) -> HashMap<String, Def> {
    let mut mapa: HashMap<String, Def> = HashMap::new();
    let mut actual: Option<(String, Def)> = None;
    for linea in texto.lines() {
        let l = linea.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        if let Some(n) = l.strip_prefix('@') {
            if let Some((nombre, d)) = actual.take() {
                mapa.insert(nombre, d);
            }
            actual = Some((n.trim().to_string(), Def::default()));
            continue;
        }
        let Some((_, def)) = actual.as_mut() else {
            continue;
        };
        let mut it = l.split_whitespace();
        let cabeza = it.next().unwrap_or("");
        let resto: Vec<&str> = it.collect();
        match cabeza {
            "av" if resto.len() == 2 => {
                if let Ok(v) = resto[1].parse::<f64>() {
                    def.ajustes.push((resto[0].to_string(), v));
                }
            }
            "gd" if resto.len() >= 2 => def
                .pasos
                .push(Paso::Guia(resto[0].to_string(), resto[1..].join(" "))),
            "use" if resto.len() == 1 => def.pasos.push(Paso::Usa(resto[0].to_string())),
            "p" => {
                let mut t = Trayecto {
                    rellenar: true,
                    trazar: true,
                    sombra: Sombra::Ninguna,
                    w: None,
                    h: None,
                    ordenes: vec![],
                };
                for o in resto {
                    match o {
                        "none" => t.rellenar = false,
                        "nostroke" => t.trazar = false,
                        "dark" => t.sombra = Sombra::Oscura,
                        "darkless" => t.sombra = Sombra::OscuraSuave,
                        "light" => t.sombra = Sombra::Clara,
                        "lightless" => t.sombra = Sombra::ClaraSuave,
                        "sep" => t.sombra = Sombra::Aparte,
                        _ => {
                            if let Some(v) = o.strip_prefix("w=") {
                                t.w = Some(v.to_string());
                            } else if let Some(v) = o.strip_prefix("h=") {
                                t.h = Some(v.to_string());
                            }
                        }
                    }
                }
                def.trayectos.push(t);
            }
            _ => {
                // Una linea de ordenes de un trayecto.
                if let Some(t) = def.trayectos.last_mut() {
                    for trozo in l.split(';') {
                        let mut p = trozo.split_whitespace();
                        if let Some(op) = p.next().and_then(|s| s.chars().next()) {
                            t.ordenes.push(Orden {
                                op,
                                args: p.map(str::to_string).collect(),
                            });
                        }
                    }
                }
            }
        }
    }
    if let Some((nombre, d)) = actual.take() {
        mapa.insert(nombre, d);
    }
    mapa
}

/// Evalua las guias de `def` (las de las que usa, antes) sobre `v`.
fn correr_guias(def: &Def, v: &mut HashMap<String, f64>, nivel: usize) {
    for paso in &def.pasos {
        match paso {
            Paso::Guia(n, f) => {
                let x = evaluar(f, v);
                v.insert(n.clone(), x);
            }
            Paso::Usa(otra) => {
                if nivel < 4
                    && let Some(d) = tabla().get(otra)
                {
                    correr_guias(d, v, nivel + 1);
                }
            }
        }
    }
}

fn trazar(def: &Def, w: f32, h: f32, adj: &HashMap<String, f32>) -> Vec<Figura> {
    let mut v = variables_base(w as f64, h as f64);
    for (n, por_defecto) in &def.ajustes {
        // Un ajuste que no es un numero (NaN, infinito) se ignora.
        let x = adj
            .get(n)
            .map(|a| *a as f64)
            .filter(|a| a.is_finite())
            .unwrap_or(*por_defecto);
        v.insert(n.clone(), x);
    }
    correr_guias(def, &mut v, 0);
    let num = |s: &str| -> f32 {
        s.parse::<f64>()
            .ok()
            .or_else(|| v.get(s).copied())
            .unwrap_or(0.0) as f32
    };
    let mut salida = Vec::new();
    for t in &def.trayectos {
        let sx =
            t.w.as_deref()
                .map(num)
                .filter(|pw| *pw > 0.0)
                .map(|pw| w / pw)
                .unwrap_or(1.0);
        let sy =
            t.h.as_deref()
                .map(num)
                .filter(|ph| *ph > 0.0)
                .map(|ph| h / ph)
                .unwrap_or(1.0);
        let nueva = |inicio: (f32, f32)| Figura {
            inicio,
            rellenar: t.rellenar,
            trazar: t.trazar,
            sombra: t.sombra,
            ..Default::default()
        };
        let mut actual: Option<Figura> = None;
        let mut pos = (0.0f32, 0.0f32);
        let cerrar = |f: Option<Figura>, salida: &mut Vec<Figura>| {
            if let Some(f) = f
                && !f.tramos.is_empty()
            {
                salida.push(f);
            }
        };
        for o in &t.ordenes {
            let a: Vec<f32> = o.args.iter().map(|s| num(s)).collect();
            let pt = |i: usize| {
                (
                    a.get(i).copied().unwrap_or(0.0) * sx,
                    a.get(i + 1).copied().unwrap_or(0.0) * sy,
                )
            };
            match o.op {
                'M' => {
                    cerrar(actual.take(), &mut salida);
                    pos = pt(0);
                    actual = Some(nueva(pos));
                }
                'L' => {
                    let p = pt(0);
                    actual
                        .get_or_insert_with(|| nueva(pos))
                        .tramos
                        .push(Tramo::Linea(p.0, p.1));
                    pos = p;
                }
                'C' => {
                    let (p1, p2, p3) = (pt(0), pt(2), pt(4));
                    actual
                        .get_or_insert_with(|| nueva(pos))
                        .tramos
                        .push(Tramo::Bezier(p1, p2, p3));
                    pos = p3;
                }
                'Q' => {
                    let (c, p) = (pt(0), pt(2));
                    let c1 = (
                        pos.0 + 2.0 / 3.0 * (c.0 - pos.0),
                        pos.1 + 2.0 / 3.0 * (c.1 - pos.1),
                    );
                    let c2 = (p.0 + 2.0 / 3.0 * (c.0 - p.0), p.1 + 2.0 / 3.0 * (c.1 - p.1));
                    actual
                        .get_or_insert_with(|| nueva(pos))
                        .tramos
                        .push(Tramo::Bezier(c1, c2, p));
                    pos = p;
                }
                'A' => {
                    let f = actual.get_or_insert_with(|| nueva(pos));
                    let (wr, hr) = (
                        a.first().copied().unwrap_or(0.0) * sx,
                        a.get(1).copied().unwrap_or(0.0) * sy,
                    );
                    let (st, sw) = (
                        a.get(2).copied().unwrap_or(0.0) / 60000.0,
                        a.get(3).copied().unwrap_or(0.0) / 60000.0,
                    );
                    pos = arco(f, pos, wr, hr, st, sw);
                }
                'Z' => {
                    if let Some(mut f) = actual.take() {
                        f.cerrada = true;
                        pos = f.inicio;
                        cerrar(Some(f), &mut salida);
                    }
                }
                'E' => {
                    cerrar(actual.take(), &mut salida);
                    let (c, rx, ry) = (
                        pt(0),
                        a.get(2).copied().unwrap_or(0.0) * sx,
                        a.get(3).copied().unwrap_or(0.0) * sy,
                    );
                    let mut f = elipse(c.0 - rx, c.1 - ry, 2.0 * rx, 2.0 * ry);
                    (f.rellenar, f.trazar, f.sombra) = (t.rellenar, t.trazar, t.sombra);
                    salida.push(f);
                }
                'R' => {
                    cerrar(actual.take(), &mut salida);
                    let (p, q) = (pt(0), pt(2));
                    let mut f = nueva(p);
                    f.tramos = vec![
                        Tramo::Linea(q.0, p.1),
                        Tramo::Linea(q.0, q.1),
                        Tramo::Linea(p.0, q.1),
                    ];
                    f.cerrada = true;
                    salida.push(f);
                }
                'K' => {
                    cerrar(actual.take(), &mut salida);
                    let (p, q) = (pt(0), pt(2));
                    let r = (a.get(4).copied().unwrap_or(0.0) * sx)
                        .clamp(0.0, ((q.0 - p.0).abs().min((q.1 - p.1).abs())) / 2.0);
                    salida.push(redondeado(p, q, r, &nueva));
                }
                _ => {}
            }
        }
        cerrar(actual.take(), &mut salida);
    }
    salida
}

fn redondeado(
    p: (f32, f32),
    q: (f32, f32),
    r: f32,
    nueva: &dyn Fn((f32, f32)) -> Figura,
) -> Figura {
    let mut f = nueva((p.0 + r, p.1));
    f.tramos.push(Tramo::Linea(q.0 - r, p.1));
    arco(&mut f, (q.0 - r, p.1), r, r, 270.0, 90.0);
    f.tramos.push(Tramo::Linea(q.0, q.1 - r));
    arco(&mut f, (q.0, q.1 - r), r, r, 0.0, 90.0);
    f.tramos.push(Tramo::Linea(p.0 + r, q.1));
    arco(&mut f, (p.0 + r, q.1), r, r, 90.0, 90.0);
    f.tramos.push(Tramo::Linea(p.0, p.1 + r));
    arco(&mut f, (p.0, p.1 + r), r, r, 180.0, 90.0);
    f.cerrada = true;
    f
}

// --- Las que se generan -------------------------------------------------

const ESTRELLAS: [(usize, f32); 10] = [
    (4, 12500.0),
    (5, 19098.0),
    (6, 28868.0),
    (7, 34601.0),
    (8, 37500.0),
    (10, 42533.0),
    (12, 37500.0),
    (16, 37500.0),
    (24, 37500.0),
    (32, 37500.0),
];

fn poligono(p: Vec<(f32, f32)>) -> Figura {
    Figura {
        inicio: p[0],
        tramos: p[1..].iter().map(|&(x, y)| Tramo::Linea(x, y)).collect(),
        cerrada: true,
        rellenar: true,
        trazar: true,
        ..Default::default()
    }
}

fn estrella(n: usize, por_defecto: f32, w: f32, h: f32, adj: &HashMap<String, f32>) -> Vec<Figura> {
    let a = adj
        .get("adj")
        .copied()
        .filter(|a| a.is_finite())
        .unwrap_or(por_defecto)
        .clamp(0.0, 50000.0);
    let interior = a / 50000.0;
    // Los vertices exteriores salen desde arriba; los interiores, entre dos
    // de ellos. Como en la especificacion (`hf` y `vf`), las puntas tocan los
    // cuatro lados de la caja: se escala para que asi sea.
    let mut crudo = Vec::with_capacity(2 * n);
    for k in 0..2 * n {
        let ang = -PI / 2.0 + PI * k as f32 / n as f32;
        let r = if k % 2 == 0 { 1.0 } else { interior };
        crudo.push((r * ang.cos(), r * ang.sin()));
    }
    let (mut x0, mut x1, mut y0, mut y1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for (i, (x, y)) in crudo.iter().enumerate() {
        if i % 2 == 0 {
            (x0, x1, y0, y1) = (x0.min(*x), x1.max(*x), y0.min(*y), y1.max(*y));
        }
    }
    let (sx, sy) = (w / (x1 - x0).max(1e-6), h / (y1 - y0).max(1e-6));
    vec![poligono(
        crudo
            .iter()
            .map(|(x, y)| (w / 2.0 + x * sx, (y - y0) * sy))
            .collect(),
    )]
}

/// Engranaje de `n` dientes (gear6, gear9): aproximado (la especificacion lo
/// define con guias muy largas); dientes trapezoidales sobre un cuerpo
/// redondo, sin agujero.
fn engranaje(n: usize, w: f32, h: f32, adj: &HashMap<String, f32>) -> Vec<Figura> {
    let (alto_def, ancho_def) = if n == 6 {
        (15000.0, 3526.0)
    } else {
        (10000.0, 1763.0)
    };
    let a1 = adj
        .get("adj1")
        .copied()
        .filter(|a| a.is_finite())
        .unwrap_or(alto_def)
        .clamp(0.0, 20000.0);
    let a2 = adj
        .get("adj2")
        .copied()
        .filter(|a| a.is_finite())
        .unwrap_or(ancho_def)
        .clamp(0.0, 5358.0);
    let ss = w.min(h);
    let raiz = (1.0 - 2.0 * ss * a1 / 100000.0 / ss).clamp(0.4, 1.0);
    let paso = 2.0 * PI / n as f32;
    // La mitad del diente en la punta y en la raiz (en radianes).
    let punta = paso * (0.11 + a2 / 5358.0 * 0.06);
    let base = paso * (0.24 + a2 / 5358.0 * 0.06);
    let (cx, cy) = (w / 2.0, h / 2.0);
    let en = |r: f32, ang: f32| (cx + cx * r * ang.cos(), cy + cy * r * ang.sin());
    let mut p = Vec::new();
    for k in 0..n {
        let c = -PI / 2.0 + paso * k as f32;
        p.push(en(raiz, c - base));
        p.push(en(1.0, c - punta));
        p.push(en(1.0, c + punta));
        p.push(en(raiz, c + base));
        // El valle redondo hasta el diente siguiente.
        for j in 1..4 {
            p.push(en(raiz, c + base + (paso - 2.0 * base) * j as f32 / 4.0));
        }
    }
    vec![poligono(p)]
}

fn sol(w: f32, h: f32, adj: &HashMap<String, f32>) -> Vec<Figura> {
    let a = adj
        .get("adj")
        .copied()
        .filter(|a| a.is_finite())
        .unwrap_or(25000.0)
        .clamp(12500.0, 46875.0)
        / 100000.0;
    // Radio del disco (1 = media caja): crece al bajar el ajuste.
    let disco = (1.0 - 2.0 * a).clamp(0.05, 0.8);
    let base = (disco + 0.14).min(0.92);
    let (cx, cy) = (w / 2.0, h / 2.0);
    let en = |r: f32, ang: f32| (cx + cx * r * ang.cos(), cy + cy * r * ang.sin());
    let mut v = Vec::new();
    for k in 0..8 {
        let c = k as f32 * PI / 4.0;
        let d = (0.12 / base).clamp(0.0, 1.0).asin();
        v.push(poligono(vec![en(1.0, c), en(base, c - d), en(base, c + d)]));
    }
    let mut disco = elipse(
        cx - cx * disco,
        cy - cy * disco,
        2.0 * cx * disco,
        2.0 * cy * disco,
    );
    disco.rellenar = true;
    v.push(disco);
    v
}

/// La nube: ocho bultos sobre un anillo irregular, escalada a la caja.
fn nube(w: f32, h: f32) -> Figura {
    let n = 10usize;
    let c = (0.5f32, 0.5f32);
    let pts: Vec<(f32, f32)> = (0..n)
        .map(|i| {
            let t = i as f32 / n as f32 * 2.0 * PI - PI / 2.0;
            let rebote = if i % 2 == 0 { 1.0 } else { 0.92 };
            (0.5 + 0.40 * rebote * t.cos(), 0.5 + 0.36 * rebote * t.sin())
        })
        .collect();
    let mut tramos = Vec::new();
    for i in 0..n {
        let (p, q) = (pts[i], pts[(i + 1) % n]);
        let medio = ((p.0 + q.0) / 2.0, (p.1 + q.1) / 2.0);
        let fuera = (medio.0 - c.0, medio.1 - c.1);
        let lf = (fuera.0 * fuera.0 + fuera.1 * fuera.1).sqrt().max(1e-6);
        let fuera = (fuera.0 / lf, fuera.1 / lf);
        let bulto = ((q.0 - p.0).hypot(q.1 - p.1)) * 0.75;
        let c1 = (
            p.0 + (medio.0 - p.0) * 0.2 + fuera.0 * bulto,
            p.1 + (medio.1 - p.1) * 0.2 + fuera.1 * bulto,
        );
        let c2 = (
            q.0 + (medio.0 - q.0) * 0.2 + fuera.0 * bulto,
            q.1 + (medio.1 - q.1) * 0.2 + fuera.1 * bulto,
        );
        tramos.push(Tramo::Bezier(c1, c2, q));
    }
    let mut f = Figura {
        inicio: pts[0],
        tramos,
        cerrada: true,
        rellenar: true,
        trazar: true,
        ..Default::default()
    };
    // Caja exacta de la curva (muestreada) para escalarla a la forma.
    let (mut x0, mut x1, mut y0, mut y1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    let mut desde = f.inicio;
    for t in &f.tramos {
        if let Tramo::Bezier(a, b, d) = *t {
            for k in 0..=16 {
                let u = k as f32 / 16.0;
                let m = 1.0 - u;
                let x = m * m * m * desde.0
                    + 3.0 * m * m * u * a.0
                    + 3.0 * m * u * u * b.0
                    + u * u * u * d.0;
                let y = m * m * m * desde.1
                    + 3.0 * m * m * u * a.1
                    + 3.0 * m * u * u * b.1
                    + u * u * u * d.1;
                (x0, x1, y0, y1) = (x0.min(x), x1.max(x), y0.min(y), y1.max(y));
            }
            desde = d;
        }
    }
    let (sx, sy) = (w / (x1 - x0).max(1e-6), h / (y1 - y0).max(1e-6));
    let m = |p: (f32, f32)| ((p.0 - x0) * sx, (p.1 - y0) * sy);
    f.inicio = m(f.inicio);
    for t in &mut f.tramos {
        if let Tramo::Bezier(a, b, d) = t {
            (*a, *b, *d) = (m(*a), m(*b), m(*d));
        }
    }
    f
}

fn nube_globo(w: f32, h: f32, adj: &HashMap<String, f32>) -> Vec<Figura> {
    let lee =
        |k: &str, d: f32| adj.get(k).copied().filter(|a| a.is_finite()).unwrap_or(d) / 100000.0;
    let punta = (
        w / 2.0 + w * lee("adj1", -20833.0),
        h / 2.0 + h * lee("adj2", 62500.0),
    );
    let c = (w / 2.0, h / 2.0);
    let mut v = vec![nube(w, h)];
    let ss = w.min(h);
    for (k, r) in [(0.84, 0.07), (0.93, 0.05), (1.0, 0.035)] {
        let p = (c.0 + (punta.0 - c.0) * k, c.1 + (punta.1 - c.1) * k);
        let r = ss * r;
        let mut e = elipse(p.0 - r, p.1 - r, 2.0 * r, 2.0 * r);
        e.sombra = Sombra::Aparte;
        v.push(e);
    }
    v
}

fn especial(nombre: &str, w: f32, h: f32, adj: &HashMap<String, f32>) -> Option<Vec<Figura>> {
    if let Some(n) = nombre
        .strip_prefix("star")
        .and_then(|s| s.parse::<usize>().ok())
        && let Some((_, def)) = ESTRELLAS.iter().find(|(k, _)| *k == n)
    {
        return Some(estrella(n, *def, w, h, adj));
    }
    match nombre {
        "gear6" => Some(engranaje(6, w, h, adj)),
        "gear9" => Some(engranaje(9, w, h, adj)),
        "sun" => Some(sol(w, h, adj)),
        "cloud" => Some(vec![nube(w, h)]),
        "cloudCallout" => Some(nube_globo(w, h, adj)),
        _ => None,
    }
}

/// Las figuras de la forma `nombre` de `w` x `h` puntos, o `None` si no es
/// una forma que se conozca (el que llama decide: sale un rectangulo).
pub fn buscar(nombre: &str, w: f32, h: f32, adj: &HashMap<String, f32>) -> Option<Vec<Figura>> {
    if !(w.is_finite() && h.is_finite()) {
        return None;
    }
    let (w, h) = (w.max(0.0), h.max(0.0));
    if let Some(d) = tabla().get(nombre).filter(|_| !nombre.starts_with('_')) {
        return Some(trazar(d, w, h, adj));
    }
    especial(nombre, w, h, adj)
}

/// Los nombres de todas las formas que se dibujan (tabla y generadas).
pub fn nombres() -> Vec<String> {
    let mut v: Vec<String> = tabla()
        .keys()
        .filter(|k| !k.starts_with('_'))
        .cloned()
        .collect();
    v.extend(ESTRELLAS.iter().map(|(n, _)| format!("star{n}")));
    v.extend(["gear6", "gear9", "sun", "cloud", "cloudCallout"].map(String::from));
    v.sort();
    v
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn sin_ajustes() -> HashMap<String, f32> {
        HashMap::new()
    }

    fn caja_de(figs: &[Figura]) -> (f32, f32, f32, f32) {
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        let mut p = |q: (f32, f32)| {
            (x0, y0, x1, y1) = (x0.min(q.0), y0.min(q.1), x1.max(q.0), y1.max(q.1));
        };
        for f in figs {
            p(f.inicio);
            for t in &f.tramos {
                match *t {
                    Tramo::Linea(x, y) => p((x, y)),
                    Tramo::Bezier(a, b, c) => {
                        p(a);
                        p(b);
                        p(c)
                    }
                }
            }
        }
        (x0, y0, x1, y1)
    }

    #[test]
    fn la_tabla_se_lee_entera() {
        let t = tabla();
        assert!(t.len() >= 150, "solo {} formas en la tabla", t.len());
        for (n, d) in t {
            if !n.starts_with('_') {
                assert!(!d.trayectos.is_empty(), "{n} no tiene trayectos");
                assert!(
                    d.trayectos.iter().all(|p| !p.ordenes.is_empty()),
                    "{n} tiene un trayecto vacio"
                );
            }
        }
    }

    /// Las que, por ser un rectangulo de verdad, pueden parecerse al de reserva.
    const SON_RECTANGULO: [&str; 3] = ["rect", "flowChartProcess", "actionButtonBlank"];

    fn es_el_de_reserva(figs: &[Figura], w: f32, h: f32) -> bool {
        figs == crate::geometria::predefinida("noExisteEstaForma", w, h, &sin_ajustes())
    }

    #[test]
    fn ninguna_forma_de_la_tabla_cae_al_rectangulo() {
        let nombres = crate::geometria::conocidas();
        assert!(nombres.len() >= 170, "solo {} formas", nombres.len());
        for n in &nombres {
            let f = crate::geometria::predefinida(n, 120.0, 80.0, &sin_ajustes());
            assert!(!f.is_empty(), "{n}: sin figuras");
            if !SON_RECTANGULO.contains(&n.as_str()) {
                assert!(
                    !es_el_de_reserva(&f, 120.0, 80.0),
                    "{n}: sale como el rectangulo de reserva"
                );
            }
        }
    }

    #[test]
    fn todas_caben_en_una_caja_razonable_y_son_finitas() {
        for n in crate::geometria::conocidas() {
            for (w, h) in [(120.0, 80.0), (60.0, 150.0), (40.0, 40.0), (500.0, 20.0)] {
                let f = crate::geometria::predefinida(&n, w, h, &sin_ajustes());
                let (x0, y0, x1, y1) = caja_de(&f);
                assert!(
                    [x0, y0, x1, y1].iter().all(|v| v.is_finite()),
                    "{n} {w}x{h}: no finita"
                );
                // Las llamadas con punta (callouts) se salen de la caja a proposito.
                let margen = w.max(h) * 2.0;
                assert!(
                    x0 >= -margen && y0 >= -margen && x1 <= w + margen && y1 <= h + margen,
                    "{n} {w}x{h}: ({x0},{y0})-({x1},{y1})"
                );
            }
        }
    }

    #[test]
    fn un_nombre_desconocido_cae_al_rectangulo() {
        assert!(buscar("noExisteEstaForma", 10.0, 20.0, &sin_ajustes()).is_none());
        let f = crate::geometria::predefinida("noExisteEstaForma", 10.0, 20.0, &sin_ajustes());
        assert_eq!(f.len(), 1);
        assert_eq!(caja_de(&f), (0.0, 0.0, 10.0, 20.0));
        assert_eq!(f[0].tramos.len(), 3);
    }

    #[test]
    fn los_ajustes_fuera_de_rango_se_acotan() {
        let con = |k: &str, v: f32| HashMap::from([(k.to_string(), v)]);
        for (n, k, tope) in [
            ("roundRect", "adj", 50000.0),
            ("snip1Rect", "adj", 50000.0),
            ("plus", "adj", 50000.0),
            ("octagon", "adj", 50000.0),
            ("frame", "adj1", 50000.0),
        ] {
            let enorme = crate::geometria::predefinida(n, 100.0, 60.0, &con(k, 9.0e9));
            let en_el_tope = crate::geometria::predefinida(n, 100.0, 60.0, &con(k, tope));
            assert_eq!(enorme, en_el_tope, "{n}: el ajuste enorme no se acota");
            let negativo = crate::geometria::predefinida(n, 100.0, 60.0, &con(k, -9.0e9));
            let cero = crate::geometria::predefinida(n, 100.0, 60.0, &con(k, 0.0));
            assert_eq!(negativo, cero, "{n}: el ajuste negativo no se acota");
        }
        // Y un ajuste que no es un numero se ignora: sale el de por defecto.
        let por_defecto = crate::geometria::predefinida("roundRect", 100.0, 60.0, &sin_ajustes());
        assert_eq!(
            crate::geometria::predefinida("roundRect", 100.0, 60.0, &con("adj", f32::NAN)),
            por_defecto
        );
        assert_eq!(
            crate::geometria::predefinida("star5", 100.0, 60.0, &con("adj", f32::NAN)),
            crate::geometria::predefinida("star5", 100.0, 60.0, &sin_ajustes())
        );
    }

    #[test]
    fn los_ajustes_del_archivo_se_respetan() {
        let con = |v: f32| HashMap::from([("adj".to_string(), v)]);
        let a = crate::geometria::predefinida("roundRect", 100.0, 60.0, &con(5000.0));
        let b = crate::geometria::predefinida("roundRect", 100.0, 60.0, &con(40000.0));
        assert_ne!(a, b);
        // El triangulo con la punta a la izquierda del todo.
        let t = crate::geometria::predefinida("triangle", 100.0, 60.0, &con(0.0));
        assert!(
            t[0].tramos
                .iter()
                .any(|x| matches!(x, Tramo::Linea(x, y) if *x == 0.0 && *y == 0.0))
        );
        // Ajustes de varias claves (la flecha: grosor y punta).
        let delgada = HashMap::from([("adj1".to_string(), 10000.0), ("adj2".to_string(), 20000.0)]);
        let f = crate::geometria::predefinida("rightArrow", 100.0, 60.0, &delgada);
        assert!(
            f[0].tramos
                .iter()
                .any(|x| matches!(x, Tramo::Linea(_, y) if (*y - 27.0).abs() < 1e-3)),
            "{:?}",
            f[0]
        );
    }

    #[test]
    fn el_cubo_y_el_bisel_traen_sus_caras_sombreadas() {
        let cubo = crate::geometria::predefinida("cube", 100.0, 100.0, &sin_ajustes());
        assert!(
            cubo.iter()
                .any(|f| f.sombra == Sombra::OscuraSuave && !f.trazar)
        );
        assert!(
            cubo.iter()
                .any(|f| f.sombra == Sombra::ClaraSuave && !f.trazar)
        );
        assert!(
            cubo.iter()
                .any(|f| !f.rellenar && f.trazar && f.sombra == Sombra::Ninguna),
            "falta el contorno"
        );
        let bisel = crate::geometria::predefinida("bevel", 100.0, 100.0, &sin_ajustes());
        for s in [
            Sombra::ClaraSuave,
            Sombra::Clara,
            Sombra::OscuraSuave,
            Sombra::Oscura,
        ] {
            assert!(bisel.iter().any(|f| f.sombra == s), "bevel sin {s:?}");
        }
        let doblada = crate::geometria::predefinida("foldedCorner", 100.0, 100.0, &sin_ajustes());
        assert!(doblada.iter().any(|f| f.sombra == Sombra::OscuraSuave));
    }

    #[test]
    fn el_donut_tiene_su_agujero_y_el_marco_tambien() {
        for n in ["donut", "frame"] {
            let f = crate::geometria::predefinida(n, 100.0, 100.0, &sin_ajustes());
            assert_eq!(f.len(), 2, "{n}");
            let (a, b) = (caja_de(&f[..1]), caja_de(&f[1..]));
            assert!(b.0 > a.0 && b.2 < a.2, "{n}: el hueco no esta dentro");
        }
    }

    #[test]
    fn las_estrellas_tocan_los_cuatro_lados() {
        for n in [4, 5, 6, 7, 8, 10, 12, 16, 24, 32] {
            let f = crate::geometria::predefinida(&format!("star{n}"), 100.0, 60.0, &sin_ajustes());
            let (x0, y0, x1, y1) = caja_de(&f);
            assert!(
                x0.abs() < 0.01
                    && y0.abs() < 0.01
                    && (x1 - 100.0).abs() < 0.01
                    && (y1 - 60.0).abs() < 0.01,
                "star{n}: {:?}",
                (x0, y0, x1, y1)
            );
            assert_eq!(f[0].tramos.len(), 2 * n - 1);
        }
    }

    #[test]
    fn los_trayectos_sin_relleno_o_sin_trazo_se_respetan() {
        let arco = crate::geometria::predefinida("leftBracket", 30.0, 100.0, &sin_ajustes());
        assert!(arco.iter().any(|f| !f.rellenar && f.trazar));
        assert!(arco.iter().any(|f| f.rellenar && !f.trazar));
        let lata = crate::geometria::predefinida("can", 60.0, 100.0, &sin_ajustes());
        assert!(lata.iter().any(|f| f.sombra == Sombra::Clara && !f.trazar));
    }

    #[test]
    fn una_forma_con_valores_hostiles_no_revienta() {
        for n in crate::geometria::conocidas() {
            for v in [f32::NAN, f32::INFINITY, -1.0e12, 1.0e12, 0.0] {
                let adj: HashMap<String, f32> = [
                    "adj", "adj1", "adj2", "adj3", "adj4", "adj5", "adj6", "adj7", "adj8",
                ]
                .iter()
                .map(|k| (k.to_string(), v))
                .collect();
                let f = crate::geometria::predefinida(&n, 100.0, 70.0, &adj);
                let (x0, y0, x1, y1) = caja_de(&f);
                assert!(
                    [x0, y0, x1, y1].iter().all(|c| c.is_finite()),
                    "{n} con {v}"
                );
            }
            // Una caja vacia tampoco.
            let _ = crate::geometria::predefinida(&n, 0.0, 0.0, &sin_ajustes());
        }
    }

    #[test]
    fn el_arco_de_una_elipse_achatada_usa_angulos_visuales() {
        // Un cuarto de elipse achatada, de la derecha (angulo visual 0) al
        // fondo (90): termina justo debajo del centro.
        let mut f = Figura::default();
        let (wr, hr) = (50.0f32, 10.0f32);
        let fin = arco(&mut f, (wr, 0.0), wr, hr, 0.0, 90.0);
        assert!(fin.0.abs() < 1e-3 && (fin.1 - hr).abs() < 1e-3, "{fin:?}");
        // Y a 45 grados visuales cae sobre la diagonal x = y desde el centro

        let mut g = Figura::default();
        let _ = arco(&mut g, (wr, 0.0), wr, hr, 0.0, 45.0);
        if let Some(Tramo::Bezier(_, _, p)) = g.tramos.last() {
            assert!((p.0 - p.1).abs() < 1e-2, "{p:?}");
        } else {
            panic!("sin tramo");
        }
    }

    /// Los 187 valores de `ST_ShapeType` (ECMA-376, 20.1.10.56).
    const ESPECIFICACION: &str = "line lineInv triangle rtTriangle rect diamond parallelogram trapezoid nonIsoscelesTrapezoid pentagon hexagon heptagon octagon decagon dodecagon star4 star5 star6 star7 star8 star10 star12 star16 star24 star32 roundRect round1Rect round2SameRect round2DiagRect snipRoundRect snip1Rect snip2SameRect snip2DiagRect plaque ellipse teardrop homePlate chevron pieWedge pie blockArc donut noSmoking rightArrow leftArrow upArrow downArrow stripedRightArrow notchedRightArrow bentUpArrow leftRightArrow upDownArrow leftUpArrow leftRightUpArrow quadArrow leftArrowCallout rightArrowCallout upArrowCallout downArrowCallout leftRightArrowCallout upDownArrowCallout quadArrowCallout bentArrow uturnArrow circularArrow leftCircularArrow leftRightCircularArrow curvedRightArrow curvedLeftArrow curvedUpArrow curvedDownArrow swooshArrow cube can lightningBolt heart sun moon smileyFace irregularSeal1 irregularSeal2 foldedCorner bevel frame halfFrame corner diagStripe chord arc leftBracket rightBracket leftBrace rightBrace bracketPair bracePair straightConnector1 bentConnector2 bentConnector3 bentConnector4 bentConnector5 curvedConnector2 curvedConnector3 curvedConnector4 curvedConnector5 callout1 callout2 callout3 accentCallout1 accentCallout2 accentCallout3 borderCallout1 borderCallout2 borderCallout3 accentBorderCallout1 accentBorderCallout2 accentBorderCallout3 wedgeRectCallout wedgeRoundRectCallout wedgeEllipseCallout cloudCallout cloud ribbon ribbon2 ellipseRibbon ellipseRibbon2 leftRightRibbon verticalScroll horizontalScroll wave doubleWave plus flowChartProcess flowChartAlternateProcess flowChartDecision flowChartInputOutput flowChartPredefinedProcess flowChartInternalStorage flowChartDocument flowChartMultidocument flowChartTerminator flowChartPreparation flowChartManualInput flowChartManualOperation flowChartConnector flowChartPunchedCard flowChartPunchedTape flowChartSummingJunction flowChartOr flowChartCollate flowChartSort flowChartExtract flowChartMerge flowChartOfflineStorage flowChartOnlineStorage flowChartMagneticTape flowChartMagneticDisk flowChartMagneticDrum flowChartDisplay flowChartDelay flowChartOffpageConnector actionButtonBlank actionButtonHome actionButtonHelp actionButtonInformation actionButtonForwardNext actionButtonBackPrevious actionButtonEnd actionButtonBeginning actionButtonReturn actionButtonDocument actionButtonSound actionButtonMovie gear6 gear9 funnel mathPlus mathMinus mathMultiply mathDivide mathEqual mathNotEqual cornerTabs squareTabs plaqueTabs chartX chartStar chartPlus";

    /// Las que aun salen como rectangulo, y por que: sus definiciones son
    /// de 30 a 80 guias trigonometricas (arcos que giran y se estrechan) y no
    /// se pueden escribir de memoria sin inventarlas; ninguna es de las que
    /// se usan a diario. Cuando se escriban, hay que quitarlas de aqui.
    const SIN_HACER: [&str; 12] = [
        "circularArrow",
        "leftCircularArrow",
        "leftRightCircularArrow",
        "curvedRightArrow",
        "curvedLeftArrow",
        "curvedUpArrow",
        "curvedDownArrow",
        "swooshArrow",
        "ellipseRibbon",
        "ellipseRibbon2",
        "leftRightRibbon",
        "funnel",
    ];

    #[test]
    fn toda_la_especificacion_esta_hecha_salvo_la_lista_explicita() {
        let hechas = crate::geometria::conocidas();
        let todas: Vec<&str> = ESPECIFICACION.split_whitespace().collect();
        assert_eq!(todas.len(), 187);
        for n in &todas {
            let hecha = hechas.iter().any(|h| h == n);
            let excusada = SIN_HACER.contains(n);
            assert!(
                hecha != excusada,
                "{n}: {}",
                if hecha {
                    "esta hecha y tambien en SIN_HACER"
                } else {
                    "ni esta hecha ni excusada"
                }
            );
            if excusada {
                let f = crate::geometria::predefinida(n, 100.0, 60.0, &sin_ajustes());
                assert!(es_el_de_reserva(&f, 100.0, 60.0), "{n} no es el rectangulo");
            }
        }
        for h in &hechas {
            assert!(
                todas.contains(&h.as_str()),
                "{h} no es una forma de la especificacion"
            );
        }
    }
}
