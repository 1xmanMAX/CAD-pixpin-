//! **Lo de Civil 3D**: LandXML (superficies, alineamientos con su perfil,
//! puntos, redes de tuberias, parcelas y lineas caracteristicas) y ficheros
//! de puntos (PNEZD, PENZD, ENZ...), en 3D. El usuario lo pidio el
//! 9-oct-2026: «ver las curvas de nivel, los replanteos, los puntos en 3D,
//! las carreteras que se hacen en Civil 3D».
//!
//! Las superficies salen con color por cota (de verde a marron y a claro) y
//! con sus curvas de nivel; los puntos sueltos de un fichero se triangulan
//! (Delaunay) para tener tambien su superficie.

use std::path::Path;

use pixpin_cad::modelo3d::{Constructor3d, Modelo3d};

/// Si el fichero es un LandXML (por su contenido, no por la extension).
pub fn es_landxml(ruta: &Path) -> bool {
    let Ok(mut f) = std::fs::File::open(ruta) else {
        return false;
    };
    let mut b = vec![0u8; 4096];
    let n = std::io::Read::read(&mut f, &mut b).unwrap_or(0);
    String::from_utf8_lossy(&b[..n]).contains("<LandXML")
}

/// Si un .txt o .csv son coordenadas de puntos: casi todas sus lineas con 3
/// o mas numeros seguidos (y quiza un numero de punto y una descripcion).
pub fn es_fichero_de_puntos(ruta: &Path) -> bool {
    let ext = ruta
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if !matches!(
        ext.as_str(),
        "txt" | "csv" | "xyz" | "pnezd" | "penzd" | "pts"
    ) {
        return false;
    }
    let Ok(mut f) = std::fs::File::open(ruta) else {
        return false;
    };
    let mut b = vec![0u8; 8192];
    let n = std::io::Read::read(&mut f, &mut b).unwrap_or(0);
    let texto = String::from_utf8_lossy(&b[..n]);
    let lineas: Vec<&str> = texto
        .lines()
        .filter(|l| !l.trim().is_empty())
        .take(40)
        .collect();
    // La ultima puede venir cortada.
    let lineas = &lineas[..lineas.len().saturating_sub(1)];
    if lineas.len() < 3 {
        return false;
    }
    let buenas = lineas
        .iter()
        .filter(|l| campos_numericos(l).len() >= 3)
        .count();
    buenas * 10 >= lineas.len() * 8
}

fn trocear(l: &str) -> Vec<&str> {
    l.split([',', ';', '\t', ' '])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

/// Los numeros del principio de una linea.
fn campos_numericos(l: &str) -> Vec<f64> {
    trocear(l)
        .iter()
        .map_while(|t| t.parse::<f64>().ok())
        .filter(|v| v.is_finite())
        .collect()
}

/// El orden de las columnas de un fichero de puntos.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Orden {
    /// Norte primero (PNEZD, NEZ).
    NorteEste,
    /// Este primero (PENZD, ENZ, XYZ).
    EsteNorte,
}

fn orden_por_nombre(nombre: &str, con_numero: bool) -> Orden {
    let n = nombre.to_ascii_uppercase();
    if n.contains("PENZ") || n.contains("ENZ") || n.contains("XYZ") {
        Orden::EsteNorte
    } else if n.contains("PNEZ") || n.contains("NEZ") {
        Orden::NorteEste
    } else if con_numero {
        // Lo que Civil 3D exporta por defecto: PNEZD.
        Orden::NorteEste
    } else {
        Orden::EsteNorte
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Punto {
    pub nombre: String,
    pub desc: String,
    pub p: [f64; 3],
}

/// Lee un fichero de puntos.
pub fn leer_puntos(texto: &str, nombre_fichero: &str) -> Vec<Punto> {
    let filas: Vec<(Vec<f64>, Vec<&str>)> = texto
        .lines()
        .map(|l| {
            let t = trocear(l);
            let nums: Vec<f64> = t
                .iter()
                .map_while(|x| x.parse::<f64>().ok())
                .filter(|v| v.is_finite())
                .collect();
            let resto = t[nums.len().min(t.len())..].to_vec();
            (nums, resto)
        })
        .filter(|(n, _)| n.len() >= 3)
        .collect();
    // Con numero de punto si casi todas tienen 4 o mas numeros y el primero es entero.
    let con_numero = filas
        .iter()
        .filter(|(n, _)| n.len() >= 4 && n[0].fract() == 0.0)
        .count()
        * 10
        >= filas.len() * 8;
    let orden = orden_por_nombre(nombre_fichero, con_numero);
    filas
        .into_iter()
        .enumerate()
        .map(|(i, (n, resto))| {
            let (num, c) = if con_numero && n.len() >= 4 {
                (format!("{}", n[0]), &n[1..4])
            } else {
                (format!("{}", i + 1), &n[0..3])
            };
            let (x, y) = match orden {
                Orden::NorteEste => (c[1], c[0]),
                Orden::EsteNorte => (c[0], c[1]),
            };
            Punto {
                nombre: num,
                desc: resto.join(" "),
                p: [x, y, c[2]],
            }
        })
        .collect()
}

// ------------------------------------------------------------------ superficie

/// Una superficie triangulada (TIN), en el mundo.
pub struct Tin {
    pub puntos: Vec<[f64; 3]>,
    pub triangulos: Vec<[u32; 3]>,
}

/// Delaunay de los puntos (en planta).
pub fn triangular(puntos: &[[f64; 3]]) -> Tin {
    if puntos.len() < 3 {
        return Tin {
            puntos: puntos.to_vec(),
            triangulos: Vec::new(),
        };
    }
    let o = puntos[0];
    let pts: Vec<delaunator::Point> = puntos
        .iter()
        .map(|p| delaunator::Point {
            x: p[0] - o[0],
            y: p[1] - o[1],
        })
        .collect();
    let t = delaunator::triangulate(&pts);
    let triangulos: Vec<[u32; 3]> = t
        .triangles
        .chunks_exact(3)
        .map(|c| [c[0] as u32, c[1] as u32, c[2] as u32])
        .collect();
    // Fuera los triangulos largos del borde (los que cierran el contorno
    // convexo saltando huecos): mas de 6 veces el lado tipico.
    let mut lados: Vec<f64> = triangulos.iter().map(|t| lado_max(puntos, t)).collect();
    lados.sort_by(f64::total_cmp);
    let tipico = lados.get(lados.len() / 2).copied().unwrap_or(0.0);
    let triangulos = triangulos
        .into_iter()
        .filter(|t| tipico <= 0.0 || lado_max(puntos, t) <= tipico * 6.0)
        .collect();
    Tin {
        puntos: puntos.to_vec(),
        triangulos,
    }
}

fn lado_max(p: &[[f64; 3]], t: &[u32; 3]) -> f64 {
    let d = |a: u32, b: u32| {
        let (a, b) = (p[a as usize], p[b as usize]);
        (a[0] - b[0]).hypot(a[1] - b[1])
    };
    d(t[0], t[1]).max(d(t[1], t[2])).max(d(t[2], t[0]))
}

/// El color de una cota (0 abajo, 1 arriba): verde, amarillo, marron, claro.
fn color_cota(k: f64) -> [f32; 4] {
    const P: [[f32; 3]; 5] = [
        [0.24, 0.52, 0.34],
        [0.50, 0.70, 0.33],
        [0.86, 0.80, 0.45],
        [0.66, 0.47, 0.30],
        [0.90, 0.88, 0.84],
    ];
    let k = (k.clamp(0.0, 1.0) * 4.0) as f32;
    let i = (k.floor() as usize).min(3);
    let f = k - i as f32;
    let (a, b) = (P[i], P[i + 1]);
    [
        a[0] + (b[0] - a[0]) * f,
        a[1] + (b[1] - a[1]) * f,
        a[2] + (b[2] - a[2]) * f,
        1.0,
    ]
}

/// Un paso de curvas «redondo» (1, 2, 5 por potencia de 10) que de unas 25
/// curvas en `rango`.
pub fn paso_de_curvas(rango: f64) -> f64 {
    if !(rango > 0.0) {
        return 1.0;
    }
    let bruto = rango / 25.0;
    let p = 10f64.powf(bruto.log10().floor());
    for m in [1.0, 2.0, 5.0, 10.0] {
        if p * m >= bruto {
            return p * m;
        }
    }
    p * 10.0
}

/// Pone la superficie en el modelo: triangulos con color por cota y sus
/// curvas de nivel (cada 5, una mas marcada).
pub fn poner_tin(c: &mut Constructor3d, tin: &Tin, nombre: &str) {
    if tin.triangulos.is_empty() {
        return;
    }
    let (mut zmin, mut zmax) = (f64::INFINITY, f64::NEG_INFINITY);
    for t in &tin.triangulos {
        for &i in t {
            let z = tin.puntos[i as usize][2];
            zmin = zmin.min(z);
            zmax = zmax.max(z);
        }
    }
    let rango = (zmax - zmin).max(1e-9);
    let e = c.elemento("Superficie", nombre);
    // Un vertice por esquina de cada triangulo: el color va por cota, y la
    // normal por cara se ve mejor en un terreno.
    let mut por_color: std::collections::BTreeMap<u8, (Vec<[f64; 3]>, Vec<u32>)> =
        Default::default();
    for t in &tin.triangulos {
        let p = [
            tin.puntos[t[0] as usize],
            tin.puntos[t[1] as usize],
            tin.puntos[t[2] as usize],
        ];
        let zm = (p[0][2] + p[1][2] + p[2][2]) / 3.0;
        let banda = (((zm - zmin) / rango) * 24.0).round().clamp(0.0, 24.0) as u8;
        let (pts, ii) = por_color.entry(banda).or_default();
        for q in p {
            ii.push(pts.len() as u32);
            pts.push(q);
        }
    }
    for (banda, (pts, ii)) in por_color {
        c.malla_sin_aristas(e, &pts, &ii, color_cota(banda as f64 / 24.0));
    }
    // Curvas de nivel.
    let paso = paso_de_curvas(rango);
    let primera = (zmin / paso).ceil() as i64;
    let ultima = (zmax / paso).floor() as i64;
    if ultima - primera > 2000 {
        return;
    }
    let curvas = c.elemento(
        "Curvas de nivel",
        &format!("{nombre} · cada {}", formato_paso(paso)),
    );
    for n in primera..=ultima {
        let z = n as f64 * paso;
        let maestra = n % 5 == 0;
        let mut tramos: Vec<[f64; 3]> = Vec::new();
        for t in &tin.triangulos {
            let p = [
                tin.puntos[t[0] as usize],
                tin.puntos[t[1] as usize],
                tin.puntos[t[2] as usize],
            ];
            let mut corte = Vec::with_capacity(2);
            for (a, b) in [(p[0], p[1]), (p[1], p[2]), (p[2], p[0])] {
                if (a[2] < z) != (b[2] < z) {
                    let f = (z - a[2]) / (b[2] - a[2]);
                    corte.push([a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f, z]);
                }
            }
            if corte.len() == 2 {
                tramos.extend(corte);
            }
        }
        let color = if maestra {
            [0.30, 0.18, 0.08, 1.0]
        } else {
            [0.40, 0.28, 0.16, 0.75]
        };
        c.segmentos(curvas, &tramos, color);
    }
}

fn formato_paso(p: f64) -> String {
    if p >= 1.0 {
        format!("{p:.0}")
    } else {
        format!("{p}")
    }
}

// ------------------------------------------------------------------ LandXML

fn hijos<'a, 'i>(
    n: roxmltree::Node<'a, 'i>,
    nombre: &str,
) -> impl Iterator<Item = roxmltree::Node<'a, 'i>> {
    let nombre = nombre.to_string();
    n.children()
        .filter(move |c| c.is_element() && c.tag_name().name() == nombre)
}

fn hijo<'a, 'i>(n: roxmltree::Node<'a, 'i>, nombre: &str) -> Option<roxmltree::Node<'a, 'i>> {
    hijos(n, nombre).next()
}

fn numeros(t: &str) -> Vec<f64> {
    t.split_whitespace()
        .filter_map(|x| x.parse().ok())
        .collect()
}

/// Un punto de LandXML: «norte este [cota]».
fn ne(n: Option<roxmltree::Node>) -> Option<[f64; 3]> {
    let v = numeros(n?.text()?);
    (v.len() >= 2).then(|| [v[1], v[0], v.get(2).copied().unwrap_or(f64::NAN)])
}

/// La geometria en planta de un `CoordGeom`, como polilinea (con la cota si
/// la trae) y la estacion de cada punto.
fn geometria(cg: roxmltree::Node) -> Vec<([f64; 3], f64)> {
    let mut out: Vec<([f64; 3], f64)> = Vec::new();
    let mut sta = 0.0;
    let poner = |p: [f64; 3], out: &mut Vec<([f64; 3], f64)>, sta: &mut f64| {
        if let Some((q, s)) = out.last() {
            let d = (p[0] - q[0]).hypot(p[1] - q[1]);
            if d < 1e-9 {
                return;
            }
            *sta = s + d;
        }
        out.push((p, *sta));
    };
    for el in cg.children().filter(|c| c.is_element()) {
        let (a, b) = (ne(hijo(el, "Start")), ne(hijo(el, "End")));
        let (Some(a), Some(b)) = (a, b) else { continue };
        match el.tag_name().name() {
            "Curve" => {
                if let Some(cen) = ne(hijo(el, "Center")) {
                    let r = (a[0] - cen[0]).hypot(a[1] - cen[1]);
                    let a0 = (a[1] - cen[1]).atan2(a[0] - cen[0]);
                    let mut a1 = (b[1] - cen[1]).atan2(b[0] - cen[0]);
                    let horario = el.attribute("rot") == Some("cw");
                    if horario {
                        while a1 > a0 {
                            a1 -= std::f64::consts::TAU;
                        }
                    } else {
                        while a1 < a0 {
                            a1 += std::f64::consts::TAU;
                        }
                    }
                    let n = (((a1 - a0).abs() / 0.05).ceil() as usize).clamp(2, 200);
                    for k in 0..=n {
                        let t = a0 + (a1 - a0) * k as f64 / n as f64;
                        let z = a[2] + (b[2] - a[2]) * k as f64 / n as f64;
                        poner(
                            [cen[0] + r * t.cos(), cen[1] + r * t.sin(), z],
                            &mut out,
                            &mut sta,
                        );
                    }
                    continue;
                }
                poner(a, &mut out, &mut sta);
                poner(b, &mut out, &mut sta);
            }
            "Spiral" | "IrregularLine" | "Chain" => {
                // La espiral, como una curva de Bezier por su PI: a la vista
                // es lo mismo.
                let pi = ne(hijo(el, "PI")).unwrap_or([
                    (a[0] + b[0]) / 2.0,
                    (a[1] + b[1]) / 2.0,
                    f64::NAN,
                ]);
                for k in 0..=16 {
                    let t = k as f64 / 16.0;
                    let u = 1.0 - t;
                    let p = [
                        u * u * a[0] + 2.0 * u * t * pi[0] + t * t * b[0],
                        u * u * a[1] + 2.0 * u * t * pi[1] + t * t * b[1],
                        a[2] + (b[2] - a[2]) * t,
                    ];
                    poner(p, &mut out, &mut sta);
                }
            }
            _ => {
                poner(a, &mut out, &mut sta);
                poner(b, &mut out, &mut sta);
            }
        }
    }
    out
}

/// Un perfil (rasante): sus PVI con las curvas verticales parabolicas.
struct Perfil {
    nombre: String,
    /// (estacion, cota, largo de la curva vertical)
    pvi: Vec<(f64, f64, f64)>,
}

impl Perfil {
    fn cota(&self, s: f64) -> Option<f64> {
        let v = &self.pvi;
        if v.is_empty() {
            return None;
        }
        if v.len() == 1 || s <= v[0].0 {
            return Some(v[0].1);
        }
        if s >= v[v.len() - 1].0 {
            return Some(v[v.len() - 1].1);
        }
        let i = v.partition_point(|p| p.0 <= s).max(1) - 1;
        let tangente = |i: usize, s: f64| {
            let (a, b) = (v[i], v[(i + 1).min(v.len() - 1)]);
            if (b.0 - a.0).abs() < 1e-12 {
                a.1
            } else {
                a.1 + (b.1 - a.1) * (s - a.0) / (b.0 - a.0)
            }
        };
        // Dentro de la curva vertical de un PVI: la parabola.
        for j in [i, i + 1] {
            if j == 0 || j >= v.len() - 1 {
                continue;
            }
            let (sp, zp, l) = v[j];
            if l > 0.0 && (s - sp).abs() < l / 2.0 {
                let g1 = (zp - v[j - 1].1) / (sp - v[j - 1].0).max(1e-12);
                let g2 = (v[j + 1].1 - zp) / (v[j + 1].0 - sp).max(1e-12);
                let x = s - (sp - l / 2.0);
                let z0 = zp - g1 * l / 2.0;
                return Some(z0 + g1 * x + (g2 - g1) / (2.0 * l) * x * x);
            }
        }
        Some(tangente(i, s))
    }
}

fn perfiles(al: roxmltree::Node) -> Vec<Perfil> {
    let mut out = Vec::new();
    for pr in hijos(al, "Profile") {
        for pa in hijos(pr, "ProfAlign") {
            let mut pvi = Vec::new();
            for e in pa.children().filter(|c| c.is_element()) {
                let v = numeros(e.text().unwrap_or(""));
                if v.len() < 2 {
                    continue;
                }
                let l = match e.tag_name().name() {
                    "ParaCurve" | "UnsymParaCurve" => e
                        .attribute("length")
                        .and_then(|x| x.parse().ok())
                        .unwrap_or(0.0),
                    "PVI" | "CircCurve" => 0.0,
                    _ => continue,
                };
                pvi.push((v[0], v[1], l));
            }
            if !pvi.is_empty() {
                out.push(Perfil {
                    nombre: pa.attribute("name").unwrap_or("").to_string(),
                    pvi,
                });
            }
        }
    }
    out
}

/// Para poner en el terreno lo que no trae cota: una rejilla de triangulos.
struct Rejilla<'a> {
    tins: &'a [Tin],
    celda: f64,
    o: [f64; 2],
    mapa: std::collections::HashMap<(i64, i64), Vec<(usize, usize)>>,
}

impl<'a> Rejilla<'a> {
    fn de(tins: &'a [Tin]) -> Rejilla<'a> {
        let (mut min, mut max) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
        let mut n = 0usize;
        for t in tins {
            for p in &t.puntos {
                min = [min[0].min(p[0]), min[1].min(p[1])];
                max = [max[0].max(p[0]), max[1].max(p[1])];
            }
            n += t.triangulos.len();
        }
        let lado = (max[0] - min[0]).max(max[1] - min[1]).max(1e-9);
        let celda = lado / (n as f64).sqrt().clamp(1.0, 1024.0);
        let mut mapa: std::collections::HashMap<(i64, i64), Vec<(usize, usize)>> =
            Default::default();
        for (k, t) in tins.iter().enumerate() {
            for (j, tr) in t.triangulos.iter().enumerate() {
                let p = tr.map(|i| t.puntos[i as usize]);
                let (x0, x1) = (
                    p[0][0].min(p[1][0]).min(p[2][0]),
                    p[0][0].max(p[1][0]).max(p[2][0]),
                );
                let (y0, y1) = (
                    p[0][1].min(p[1][1]).min(p[2][1]),
                    p[0][1].max(p[1][1]).max(p[2][1]),
                );
                let c = |v: f64, o: f64| ((v - o) / celda).floor() as i64;
                for i in c(x0, min[0])..=c(x1, min[0]) {
                    for jj in c(y0, min[1])..=c(y1, min[1]) {
                        mapa.entry((i, jj)).or_default().push((k, j));
                    }
                }
            }
        }
        Rejilla {
            tins,
            celda,
            o: min,
            mapa,
        }
    }

    fn cota(&self, x: f64, y: f64) -> Option<f64> {
        let k = (
            ((x - self.o[0]) / self.celda).floor() as i64,
            ((y - self.o[1]) / self.celda).floor() as i64,
        );
        for &(t, j) in self.mapa.get(&k)? {
            let tin = &self.tins[t];
            let p = tin.triangulos[j].map(|i| tin.puntos[i as usize]);
            let d = (p[1][1] - p[2][1]) * (p[0][0] - p[2][0])
                + (p[2][0] - p[1][0]) * (p[0][1] - p[2][1]);
            if d.abs() < 1e-18 {
                continue;
            }
            let a = ((p[1][1] - p[2][1]) * (x - p[2][0]) + (p[2][0] - p[1][0]) * (y - p[2][1])) / d;
            let b = ((p[2][1] - p[0][1]) * (x - p[2][0]) + (p[0][0] - p[2][0]) * (y - p[2][1])) / d;
            let c = 1.0 - a - b;
            if a >= -1e-9 && b >= -1e-9 && c >= -1e-9 {
                return Some(a * p[0][2] + b * p[1][2] + c * p[2][2]);
            }
        }
        None
    }
}

/// Lee un LandXML.
pub fn de_landxml(texto: &str) -> Result<Modelo3d, String> {
    let doc = roxmltree::Document::parse_with_options(
        texto,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    )
    .map_err(|e| format!("no es un LandXML valido: {e}"))?;
    let raiz = doc.root_element();
    let mut c = Constructor3d::nuevo();
    // Superficies.
    let mut tins: Vec<(String, Tin)> = Vec::new();
    for ss in hijos(raiz, "Surfaces") {
        for s in hijos(ss, "Surface") {
            let nombre = s.attribute("name").unwrap_or("Superficie").to_string();
            let Some(def) = hijo(s, "Definition") else {
                continue;
            };
            let mut ids: std::collections::HashMap<String, u32> = Default::default();
            let mut puntos = Vec::new();
            if let Some(pnts) = hijo(def, "Pnts") {
                for p in hijos(pnts, "P") {
                    let v = numeros(p.text().unwrap_or(""));
                    if v.len() >= 3 {
                        ids.insert(
                            p.attribute("id").unwrap_or("").to_string(),
                            puntos.len() as u32,
                        );
                        puntos.push([v[1], v[0], v[2]]);
                    }
                }
            }
            let mut triangulos = Vec::new();
            if let Some(fs) = hijo(def, "Faces") {
                for f in hijos(fs, "F") {
                    // i="1": cara invisible (fuera del borde).
                    if f.attribute("i") == Some("1") {
                        continue;
                    }
                    let t: Vec<u32> = f
                        .text()
                        .unwrap_or("")
                        .split_whitespace()
                        .filter_map(|x| ids.get(x).copied())
                        .collect();
                    if t.len() == 3 {
                        triangulos.push([t[0], t[1], t[2]]);
                    }
                }
            }
            let tin = if triangulos.is_empty() {
                triangular(&puntos)
            } else {
                Tin { puntos, triangulos }
            };
            tins.push((nombre, tin));
        }
    }
    for (n, t) in &tins {
        poner_tin(&mut c, t, n);
    }
    let solo: Vec<Tin> = tins.into_iter().map(|(_, t)| t).collect();
    let rejilla = Rejilla::de(&solo);
    let zbase = solo
        .iter()
        .flat_map(|t| t.puntos.iter().map(|p| p[2]))
        .fold(f64::INFINITY, f64::min);
    let zbase = if zbase.is_finite() { zbase } else { 0.0 };
    let al_terreno = |p: [f64; 3]| -> [f64; 3] {
        if p[2].is_finite() {
            p
        } else {
            [p[0], p[1], rejilla.cota(p[0], p[1]).unwrap_or(zbase)]
        }
    };
    // Alineamientos: en planta (sobre el terreno) y con cada rasante.
    let colores = [
        [0.85, 0.12, 0.10, 1.0],
        [0.10, 0.45, 0.95, 1.0],
        [0.95, 0.55, 0.05, 1.0],
        [0.55, 0.20, 0.75, 1.0],
    ];
    for als in hijos(raiz, "Alignments") {
        for al in hijos(als, "Alignment") {
            let nombre = al.attribute("name").unwrap_or("Alineamiento");
            let sta0: f64 = al
                .attribute("staStart")
                .and_then(|x| x.parse().ok())
                .unwrap_or(0.0);
            let Some(cg) = hijo(al, "CoordGeom") else {
                continue;
            };
            let geo = geometria(cg);
            let e = c.elemento("Alineamiento", nombre);
            let planta: Vec<[f64; 3]> = geo.iter().map(|(p, _)| al_terreno(*p)).collect();
            c.linea(e, &planta, [0.85, 0.12, 0.10, 1.0]);
            for (k, perfil) in perfiles(al).iter().enumerate() {
                let e = c.elemento("Rasante", &format!("{nombre} · {}", perfil.nombre));
                let pts: Vec<[f64; 3]> = geo
                    .iter()
                    .filter_map(|(p, s)| Some([p[0], p[1], perfil.cota(s + sta0)?]))
                    .collect();
                c.linea(e, &pts, colores[(k + 1) % colores.len()]);
            }
        }
    }
    // Lineas caracteristicas y parcelas.
    for (grupo, tipo, color) in [
        ("PlanFeatures", "PlanFeature", [0.95, 0.60, 0.10, 1.0]),
        ("Parcels", "Parcel", [0.55, 0.30, 0.70, 1.0]),
    ] {
        for g in hijos(raiz, grupo) {
            for f in hijos(g, tipo) {
                let Some(cg) = hijo(f, "CoordGeom") else {
                    continue;
                };
                let tipo_es = if tipo == "Parcel" {
                    "Parcela"
                } else {
                    "Linea caracteristica"
                };
                let e = c.elemento(tipo_es, f.attribute("name").unwrap_or(""));
                let pts: Vec<[f64; 3]> = geometria(cg)
                    .into_iter()
                    .map(|(p, _)| al_terreno(p))
                    .collect();
                c.linea(e, &pts, color);
            }
        }
    }
    // Puntos COGO.
    for g in hijos(raiz, "CgPoints") {
        for p in hijos(g, "CgPoint") {
            let Some(q) = ne(Some(p)) else { continue };
            let q = al_terreno(q);
            let nombre = p.attribute("name").or(p.attribute("oID")).unwrap_or("");
            let desc = p.attribute("desc").or(p.attribute("code")).unwrap_or("");
            let e = c.elemento("Punto", &etiqueta_punto(nombre, desc, q[2]));
            c.punto(e, q, [1.0, 0.85, 0.15, 1.0]);
        }
    }
    // Redes de tuberias: estructuras y tubos entre sus cotas de fondo.
    for redes in hijos(raiz, "PipeNetworks") {
        for red in hijos(redes, "PipeNetwork") {
            let mut estructuras: std::collections::HashMap<String, ([f64; 3], f64)> =
                Default::default();
            let mut fondos: std::collections::HashMap<(String, String), f64> = Default::default();
            if let Some(ss) = hijo(red, "Structs") {
                for s in hijos(ss, "Struct") {
                    let nombre = s.attribute("name").unwrap_or("").to_string();
                    let Some(cen) = ne(hijo(s, "Center")) else {
                        continue;
                    };
                    let tapa: f64 = s
                        .attribute("elevRim")
                        .and_then(|x| x.parse().ok())
                        .unwrap_or(f64::NAN);
                    let fondo: f64 = s
                        .attribute("elevSump")
                        .and_then(|x| x.parse().ok())
                        .unwrap_or(f64::NAN);
                    for inv in hijos(s, "Invert") {
                        if let (Some(t), Some(z)) = (
                            inv.attribute("refPipe"),
                            inv.attribute("elev").and_then(|x| x.parse().ok()),
                        ) {
                            fondos.insert((nombre.clone(), t.to_string()), z);
                        }
                    }
                    let tapa = if tapa.is_finite() {
                        tapa
                    } else {
                        rejilla.cota(cen[0], cen[1]).unwrap_or(zbase)
                    };
                    let fondo = if fondo.is_finite() { fondo } else { tapa - 1.5 };
                    let e = c.elemento("Estructura", &nombre);
                    c.linea(
                        e,
                        &[[cen[0], cen[1], fondo], [cen[0], cen[1], tapa]],
                        [0.35, 0.35, 0.40, 1.0],
                    );
                    c.punto(e, [cen[0], cen[1], tapa], [0.55, 0.55, 0.60, 1.0]);
                    estructuras.insert(nombre, ([cen[0], cen[1], tapa], fondo));
                }
            }
            if let Some(ps) = hijo(red, "Pipes") {
                for p in hijos(ps, "Pipe") {
                    let nombre = p.attribute("name").unwrap_or("");
                    let (Some(a), Some(b)) = (p.attribute("refStart"), p.attribute("refEnd"))
                    else {
                        continue;
                    };
                    let (Some(sa), Some(sb)) = (estructuras.get(a), estructuras.get(b)) else {
                        continue;
                    };
                    let za = fondos
                        .get(&(a.to_string(), nombre.to_string()))
                        .copied()
                        .unwrap_or(sa.1);
                    let zb = fondos
                        .get(&(b.to_string(), nombre.to_string()))
                        .copied()
                        .unwrap_or(sb.1);
                    let e = c.elemento("Tuberia", nombre);
                    c.linea(
                        e,
                        &[[sa.0[0], sa.0[1], za], [sb.0[0], sb.0[1], zb]],
                        [0.10, 0.55, 0.85, 1.0],
                    );
                }
            }
        }
    }
    let mut m = c.terminar();
    if let Some(u) = hijo(raiz, "Units").and_then(|u| u.children().find(|n| n.is_element())) {
        if u.tag_name().name() == "Imperial" {
            m.metros = if u
                .attribute("linearUnit")
                .is_some_and(|x| x.contains("USSurvey"))
            {
                1200.0 / 3937.0
            } else {
                0.3048
            } as f32;
        }
    }
    Ok(m)
}

fn etiqueta_punto(nombre: &str, desc: &str, z: f64) -> String {
    let mut t = nombre.to_string();
    if !desc.is_empty() {
        t.push_str(" · ");
        t.push_str(desc);
    }
    t.push_str(&format!(" · Z {z:.3}"));
    t
}

/// Un fichero de puntos: los puntos y la superficie que forman.
pub fn de_puntos(texto: &str, nombre_fichero: &str) -> Result<Modelo3d, String> {
    let pts = leer_puntos(texto, nombre_fichero);
    if pts.is_empty() {
        return Err("no hay puntos en el fichero".into());
    }
    let mut c = Constructor3d::nuevo();
    let coords: Vec<[f64; 3]> = pts.iter().map(|p| p.p).collect();
    let tin = triangular(&coords);
    poner_tin(&mut c, &tin, nombre_fichero);
    // Muchos puntos (una nube de un dron) taparian la superficie.
    if pts.len() <= 20_000 {
        for p in &pts {
            let e = c.elemento("Punto", &etiqueta_punto(&p.nombre, &p.desc, p.p[2]));
            c.punto(e, p.p, [1.0, 0.85, 0.15, 1.0]);
        }
    }
    Ok(c.terminar())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn lee_pnezd_y_enz() {
        let t = "1,100.0,200.0,10.5,GRND\n2,101.0,205.0,11.0,TREE\n3,110.0,201.0,12.0\n";
        let p = leer_puntos(t, "puntos.csv");
        assert_eq!(p.len(), 3);
        // PNEZD: norte primero, asi que x es el este.
        assert_eq!(p[0].p, [200.0, 100.0, 10.5]);
        assert_eq!(p[0].nombre, "1");
        assert_eq!(p[0].desc, "GRND");
        let p = leer_puntos(
            "500.5 600.25 3.0\n501 601 3.5\n502 600 3.1\n",
            "nube ENZ.txt",
        );
        assert_eq!(p[0].p, [500.5, 600.25, 3.0]);
        // Caso negativo: un texto sin numeros no da puntos.
        assert!(leer_puntos("hola\nque tal\n", "a.txt").is_empty());
    }

    #[test]
    fn un_cuadrado_de_puntos_da_dos_triangulos_y_sus_curvas() {
        let pts = [
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 10.0],
            [0.0, 10.0, 10.0],
        ];
        let tin = triangular(&pts);
        assert_eq!(tin.triangulos.len(), 2);
        let mut c = Constructor3d::nuevo();
        poner_tin(&mut c, &tin, "prueba");
        let m = c.terminar();
        assert_eq!(m.opacos.len(), 6);
        assert!(!m.lineas.is_empty(), "sin curvas de nivel");
    }

    #[test]
    fn el_paso_de_curvas_es_redondo() {
        assert_eq!(paso_de_curvas(50.0), 2.0);
        assert_eq!(paso_de_curvas(4.0), 0.2);
        assert_eq!(paso_de_curvas(1000.0), 50.0);
        // Caso negativo: sin desnivel, un paso cualquiera pero valido.
        assert_eq!(paso_de_curvas(0.0), 1.0);
    }

    #[test]
    fn la_rasante_sigue_la_parabola_en_la_curva_vertical() {
        let p = Perfil {
            nombre: String::new(),
            pvi: vec![(0.0, 100.0, 0.0), (100.0, 110.0, 40.0), (200.0, 100.0, 0.0)],
        };
        // En las tangentes, la recta.
        assert!((p.cota(50.0).unwrap() - 105.0).abs() < 1e-9);
        // En el PVI la curva queda por debajo del vertice (cresta).
        let z = p.cota(100.0).unwrap();
        assert!(z < 110.0 && z > 108.0, "{z}");
        // Al borde de la curva, igual que la tangente.
        assert!((p.cota(80.0).unwrap() - 108.0).abs() < 1e-6);
    }

    #[test]
    fn landxml_minimo() {
        let x = r#"<?xml version="1.0"?><LandXML><Units><Metric linearUnit="meter"/></Units>
        <Surfaces><Surface name="EG"><Definition surfType="TIN"><Pnts>
        <P id="1">0 0 10</P><P id="2">0 10 11</P><P id="3">10 10 12</P><P id="4">10 0 13</P></Pnts>
        <Faces><F>1 2 3</F><F>1 3 4</F><F i="1">2 3 4</F></Faces></Definition></Surface></Surfaces>
        <Alignments><Alignment name="Eje" staStart="0"><CoordGeom><Line><Start>1 1</Start><End>9 9</End></Line></CoordGeom></Alignment></Alignments>
        <CgPoints><CgPoint name="7" desc="BM">5 5 20</CgPoint></CgPoints></LandXML>"#;
        let m = de_landxml(x).unwrap();
        assert_eq!(m.opacos.len(), 6, "la cara invisible no se dibuja");
        assert!(
            m.elementos
                .iter()
                .any(|e| e.tipo == "Alineamiento" && e.nombre == "Eje")
        );
        assert!(
            m.elementos
                .iter()
                .any(|e| e.tipo == "Punto" && e.nombre.starts_with("7 · BM"))
        );
        assert_eq!(m.puntos.len(), 6);
        // Caso negativo: un XML roto da error, no un modelo vacio.
        assert!(de_landxml("<LandXML><Surfaces>").is_err());
    }
}
