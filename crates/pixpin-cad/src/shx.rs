//! **Las fuentes SHX de AutoCAD** (romans, simplex, txt, isocp…): las letras
//! de casi todos los planos. Son trazos, no contornos: cada letra es un
//! pequeno programa de «pluma» (bajar, subir, moverse en 16 direcciones,
//! arcos por octantes, arcos con *bulge*, subformas). Aqui se lee el fichero
//! compilado (`shapes 1.0/1.1` y `unifont 1.0`) y se ejecuta ese programa
//! para sacar las rayas de cada letra.
//!
//! Hecho a partir de la descripcion publica del formato de formas de
//! AutoCAD («Shape Descriptions», guia de personalizacion); no lleva
//! ninguna fuente dentro: usa las que haya en el equipo (las de AutoCAD,
//! si esta instalado, y las de `C:\Windows\Fonts`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Una fuente SHX leida: los bytes de cada forma por su numero.
pub struct Shx {
    formas: HashMap<u16, Vec<u8>>,
    /// Lo que mide una mayuscula sobre la base (unidades de la fuente).
    pub arriba: f32,
    pub unifont: bool,
}

/// Una letra: sus trazos (cada uno, una polilinea) en unidades donde una
/// mayuscula mide 1, y lo que avanza.
#[derive(Debug, Clone, PartialEq)]
pub struct LetraShx {
    pub trazos: Vec<Vec<[f32; 2]>>,
    pub avance: f32,
}

fn u16le(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(i)?, *b.get(i + 1)?]))
}

/// Salta el nombre de una forma (texto acabado en cero).
fn sin_nombre(d: &[u8]) -> &[u8] {
    match d.iter().position(|&c| c == 0) {
        Some(i) => &d[i + 1..],
        None => &[],
    }
}

impl Shx {
    pub fn leer(bytes: &[u8]) -> Option<Shx> {
        let fin_cab = bytes.iter().position(|&c| c == 0x1a)? + 1;
        let cab = std::str::from_utf8(&bytes[..fin_cab - 1])
            .ok()?
            .trim()
            .to_ascii_lowercase();
        let mut formas = HashMap::new();
        if cab.contains("unifont") {
            // u32 numero de formas, u16 largo de la cabecera de la fuente,
            // la cabecera; despues cada forma: u16 numero, u16 largo, datos.
            let n = u32::from_le_bytes(bytes.get(fin_cab..fin_cab + 4)?.try_into().ok()?) as usize;
            let largo_info = u16le(bytes, fin_cab + 4)? as usize;
            let info = bytes.get(fin_cab + 6..fin_cab + 6 + largo_info)?;
            let datos = sin_nombre(info);
            let arriba = *datos.first()? as f32;
            let mut i = fin_cab + 6 + largo_info;
            for _ in 0..n.min(70_000) {
                let (Some(num), Some(largo)) = (u16le(bytes, i), u16le(bytes, i + 2)) else {
                    break;
                };
                let Some(d) = bytes.get(i + 4..i + 4 + largo as usize) else {
                    break;
                };
                formas.insert(num, sin_nombre(d).to_vec());
                i += 4 + largo as usize;
            }
            return Some(Shx {
                formas,
                arriba: arriba.max(1.0),
                unifont: true,
            });
        }
        if cab.contains("shapes") {
            // u16 primero, u16 ultimo, u16 cuantas; el indice (numero, largo)
            // y los datos seguidos en el mismo orden.
            let n = u16le(bytes, fin_cab + 4)? as usize;
            let mut indice = Vec::with_capacity(n);
            let mut i = fin_cab + 6;
            for _ in 0..n {
                indice.push((u16le(bytes, i)?, u16le(bytes, i + 2)? as usize));
                i += 4;
            }
            let mut arriba = 0.0;
            for (num, largo) in indice {
                let d = bytes.get(i..i + largo)?;
                i += largo;
                let d = sin_nombre(d);
                if num == 0 {
                    arriba = *d.first()? as f32;
                } else {
                    formas.insert(num, d.to_vec());
                }
            }
            return Some(Shx {
                formas,
                arriba: if arriba > 0.0 { arriba } else { 9.0 },
                unifont: false,
            });
        }
        None
    }

    pub fn tiene(&self, ch: char) -> bool {
        u16::try_from(ch as u32).is_ok_and(|c| self.formas.contains_key(&c))
    }

    /// Las rayas de una letra; `None` si la fuente no la tiene.
    pub fn letra(&self, ch: char) -> Option<LetraShx> {
        let num = u16::try_from(ch as u32).ok()?;
        let datos = self.formas.get(&num)?;
        let mut p = Pluma::default();
        p.correr(self, datos, 0);
        p.cortar();
        let k = 1.0 / self.arriba;
        Some(LetraShx {
            trazos: p
                .trazos
                .into_iter()
                .filter(|t| t.len() >= 2)
                .map(|t| t.into_iter().map(|[x, y]| [x * k, y * k]).collect())
                .collect(),
            avance: p.x * k,
        })
    }
}

/// Las 16 direcciones de un vector de forma (0 = este, en contra del reloj).
const DIRECCIONES: [(f32, f32); 16] = [
    (1.0, 0.0),
    (1.0, 0.5),
    (1.0, 1.0),
    (0.5, 1.0),
    (0.0, 1.0),
    (-0.5, 1.0),
    (-1.0, 1.0),
    (-1.0, 0.5),
    (-1.0, 0.0),
    (-1.0, -0.5),
    (-1.0, -1.0),
    (-0.5, -1.0),
    (0.0, -1.0),
    (0.5, -1.0),
    (1.0, -1.0),
    (1.0, -0.5),
];

struct Pluma {
    x: f32,
    y: f32,
    abajo: bool,
    escala: f32,
    pila: Vec<(f32, f32)>,
    trazos: Vec<Vec<[f32; 2]>>,
    actual: Vec<[f32; 2]>,
    /// Lo de la orden 14 (solo para texto vertical): se ejecuta sin pintar
    /// y sin moverse.
    seco: bool,
}

impl Default for Pluma {
    fn default() -> Self {
        Pluma {
            x: 0.0,
            y: 0.0,
            abajo: true,
            escala: 1.0,
            pila: Vec::new(),
            trazos: Vec::new(),
            actual: Vec::new(),
            seco: false,
        }
    }
}

fn sb(b: u8) -> f32 {
    b as i8 as f32
}

impl Pluma {
    fn cortar(&mut self) {
        if self.actual.len() >= 2 {
            self.trazos.push(std::mem::take(&mut self.actual));
        } else {
            self.actual.clear();
        }
    }

    fn ir(&mut self, dx: f32, dy: f32) {
        if self.seco {
            return;
        }
        if self.abajo {
            if self.actual.is_empty() {
                self.actual.push([self.x, self.y]);
            }
            self.actual.push([self.x + dx, self.y + dy]);
        }
        self.x += dx;
        self.y += dy;
    }

    /// Un arco desde donde esta la pluma: centro, radio, angulos (grados).
    fn arco(&mut self, cx: f32, cy: f32, r: f32, a0: f32, a1: f32) {
        let pasos = (((a1 - a0).abs() / 11.25).ceil() as usize).clamp(2, 64);
        for i in 1..=pasos {
            let a = (a0 + (a1 - a0) * i as f32 / pasos as f32).to_radians();
            let (nx, ny) = (cx + r * a.cos(), cy + r * a.sin());
            self.ir(nx - self.x, ny - self.y);
        }
    }

    fn bulge(&mut self, dx: f32, dy: f32, b: f32) {
        let b = b / 127.0;
        if b.abs() < 1e-6 {
            self.ir(dx, dy);
            return;
        }
        let cuerda = (dx * dx + dy * dy).sqrt();
        if cuerda < 1e-6 {
            return;
        }
        let theta = 4.0 * b.atan();
        let (mx, my) = (self.x + dx / 2.0, self.y + dy / 2.0);
        let h = cuerda * (1.0 - b * b) / (4.0 * b);
        let (cx, cy) = (mx - dy / cuerda * h, my + dx / cuerda * h);
        let r = ((self.x - cx).powi(2) + (self.y - cy).powi(2)).sqrt();
        let a0 = (self.y - cy).atan2(self.x - cx).to_degrees();
        let (fx, fy) = (self.x + dx, self.y + dy);
        self.arco(cx, cy, r, a0, a0 + theta.to_degrees());
        // Que acabe exacto.
        let (ex, ey) = (fx - self.x, fy - self.y);
        if ex.abs() + ey.abs() > 1e-4 {
            self.ir(ex, ey);
        }
    }

    /// Ejecuta los bytes de una forma. Devuelve cuantos uso.
    fn correr(&mut self, f: &Shx, d: &[u8], profundidad: u32) -> usize {
        let mut i = 0;
        while i < d.len() {
            let c = d[i];
            i += 1;
            match c {
                0 => break,
                1 => {
                    if !self.seco {
                        self.abajo = true;
                    }
                }
                2 => {
                    if !self.seco {
                        self.cortar();
                        self.abajo = false;
                    }
                }
                3 => {
                    let k = *d.get(i).unwrap_or(&1) as f32;
                    i += 1;
                    if !self.seco && k > 0.0 {
                        self.escala /= k;
                    }
                }
                4 => {
                    let k = *d.get(i).unwrap_or(&1) as f32;
                    i += 1;
                    if !self.seco {
                        self.escala *= k;
                    }
                }
                5 => {
                    if !self.seco && self.pila.len() < 8 {
                        self.pila.push((self.x, self.y));
                    }
                }
                6 => {
                    if !self.seco
                        && let Some((x, y)) = self.pila.pop()
                    {
                        self.cortar();
                        self.x = x;
                        self.y = y;
                    }
                }
                7 => {
                    let num = if f.unifont {
                        let n = u16::from_be_bytes([
                            *d.get(i).unwrap_or(&0),
                            *d.get(i + 1).unwrap_or(&0),
                        ]);
                        i += 2;
                        n
                    } else {
                        let n = *d.get(i).unwrap_or(&0) as u16;
                        i += 1;
                        n
                    };
                    if !self.seco
                        && profundidad < 8
                        && let Some(sub) = f.formas.get(&num)
                    {
                        let sub = sub.clone();
                        self.correr(f, &sub, profundidad + 1);
                    }
                }
                8 => {
                    let (dx, dy) = (sb(*d.get(i).unwrap_or(&0)), sb(*d.get(i + 1).unwrap_or(&0)));
                    i += 2;
                    let e = self.escala;
                    self.ir(dx * e, dy * e);
                }
                9 => loop {
                    let (dx, dy) = (sb(*d.get(i).unwrap_or(&0)), sb(*d.get(i + 1).unwrap_or(&0)));
                    i += 2;
                    if (dx == 0.0 && dy == 0.0) || i > d.len() {
                        break;
                    }
                    let e = self.escala;
                    self.ir(dx * e, dy * e);
                },
                10 => {
                    let r = *d.get(i).unwrap_or(&0) as f32 * self.escala;
                    let s = *d.get(i + 1).unwrap_or(&0);
                    i += 2;
                    let sentido = if s & 0x80 != 0 { -1.0 } else { 1.0 };
                    let inicio = ((s >> 4) & 7) as f32 * 45.0;
                    let n = match s & 7 {
                        0 => 8,
                        k => k,
                    } as f32;
                    let (cx, cy) = (
                        self.x - r * inicio.to_radians().cos(),
                        self.y - r * inicio.to_radians().sin(),
                    );
                    self.arco(cx, cy, r, inicio, inicio + sentido * n * 45.0);
                }
                11 => {
                    let (o0, o1) = (
                        *d.get(i).unwrap_or(&0) as f32,
                        *d.get(i + 1).unwrap_or(&0) as f32,
                    );
                    let r = (*d.get(i + 2).unwrap_or(&0) as f32 * 256.0
                        + *d.get(i + 3).unwrap_or(&0) as f32)
                        * self.escala;
                    let s = *d.get(i + 4).unwrap_or(&0);
                    i += 5;
                    let sentido = if s & 0x80 != 0 { -1.0 } else { 1.0 };
                    let oct = ((s >> 4) & 7) as f32;
                    let n = match s & 7 {
                        0 => 8,
                        k => k,
                    } as f32;
                    let a0 = oct * 45.0 + sentido * o0 * 45.0 / 256.0;
                    let a1 = (oct + sentido * (n - 1.0)) * 45.0 + sentido * o1 * 45.0 / 256.0;
                    let (cx, cy) = (
                        self.x - r * a0.to_radians().cos(),
                        self.y - r * a0.to_radians().sin(),
                    );
                    self.arco(cx, cy, r, a0, a1);
                }
                12 => {
                    let (dx, dy, b) = (
                        sb(*d.get(i).unwrap_or(&0)),
                        sb(*d.get(i + 1).unwrap_or(&0)),
                        sb(*d.get(i + 2).unwrap_or(&0)),
                    );
                    i += 3;
                    let e = self.escala;
                    if !self.seco {
                        self.bulge(dx * e, dy * e, b);
                    }
                }
                13 => loop {
                    let (dx, dy) = (sb(*d.get(i).unwrap_or(&0)), sb(*d.get(i + 1).unwrap_or(&0)));
                    i += 2;
                    if (dx == 0.0 && dy == 0.0) || i > d.len() {
                        break;
                    }
                    let b = sb(*d.get(i).unwrap_or(&0));
                    i += 1;
                    let e = self.escala;
                    if !self.seco {
                        self.bulge(dx * e, dy * e, b);
                    }
                },
                14 => {
                    // Solo para texto vertical: la orden siguiente no cuenta.
                    let antes = self.seco;
                    self.seco = true;
                    let usado = self.una(f, &d[i..], profundidad);
                    i += usado;
                    self.seco = antes;
                }
                v => {
                    let largo = (v >> 4) as f32 * self.escala;
                    let (ux, uy) = DIRECCIONES[(v & 15) as usize];
                    self.ir(ux * largo, uy * largo);
                }
            }
        }
        i
    }

    /// Ejecuta UNA orden (para la 14) y dice cuantos bytes ocupaba.
    fn una(&mut self, f: &Shx, d: &[u8], profundidad: u32) -> usize {
        let Some(&c) = d.first() else { return 0 };
        let largo = match c {
            0 => return 0,
            1 | 2 | 5 | 6 | 14 => 1,
            3 | 4 => 2,
            7 => {
                if f.unifont {
                    3
                } else {
                    2
                }
            }
            8 => 3,
            10 => 3,
            11 => 6,
            12 => 4,
            9 => {
                let mut k = 1;
                while k + 1 < d.len() && !(d[k] == 0 && d[k + 1] == 0) {
                    k += 2;
                }
                k + 2
            }
            13 => {
                let mut k = 1;
                while k + 1 < d.len() && !(d[k] == 0 && d[k + 1] == 0) {
                    k += 3;
                }
                k + 2
            }
            _ => 1,
        }
        .min(d.len());
        let _ = (f, profundidad);
        largo
    }
}

/// Donde buscar las fuentes SHX: primero la carpeta de letras de PixPin (las
/// que puso el usuario mandan, ver [`crate::letras`]), luego las de AutoCAD
/// (todas las versiones) y las de Windows.
pub fn carpetas() -> Vec<PathBuf> {
    let mut v = crate::letras::carpetas_del_usuario();
    v.extend(carpetas_del_sistema());
    v
}

/// Las de AutoCAD y las de Windows, sin las del usuario.
pub fn carpetas_del_sistema() -> Vec<PathBuf> {
    let mut v = Vec::new();
    for base in [
        "C:\\Program Files\\Autodesk",
        "C:\\Program Files (x86)\\Autodesk",
    ] {
        if let Ok(d) = std::fs::read_dir(base) {
            let mut versiones: Vec<PathBuf> = d
                .flatten()
                .map(|e| e.path().join("Fonts"))
                .filter(|p| p.is_dir())
                .collect();
            versiones.sort();
            versiones.reverse();
            v.extend(versiones);
        }
    }
    v.push(PathBuf::from("C:\\Windows\\Fonts"));
    v
}

/// El fichero de una fuente SHX por su nombre (`romans.shx`, `ROMANS`).
pub fn buscar(nombre: &str, carpetas: &[PathBuf]) -> Option<PathBuf> {
    let base = nombre.rsplit(['\\', '/']).next()?.trim();
    if base.is_empty() {
        return None;
    }
    let con_ext = if base.to_ascii_lowercase().ends_with(".shx") {
        base.to_string()
    } else {
        format!("{base}.shx")
    };
    for c in carpetas {
        let r = c.join(&con_ext);
        if r.is_file() {
            return Some(r);
        }
        // Windows no distingue mayusculas, pero por si acaso.
        if let Ok(d) = std::fs::read_dir(c) {
            for e in d.flatten() {
                if e.file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(&con_ext)
                {
                    return Some(e.path());
                }
            }
        }
    }
    None
}

pub fn abrir(ruta: &Path) -> Option<Shx> {
    Shx::leer(&std::fs::read(ruta).ok()?)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Una fuente «shapes 1.0» hecha a mano: la «L» baja 8 y va a la derecha 4.
    fn fuente_l() -> Vec<u8> {
        let mut b = b"AutoCAD-86 shapes 1.0\r\n\x1a".to_vec();
        let info: Vec<u8> = [b"prueba".to_vec(), vec![0, 8, 2, 0, 0]].concat();
        // Pluma arriba, a (0,8): 8 al norte; pluma abajo, 8 al sur, 4 al este.
        let l: Vec<u8> = [b"L".to_vec(), vec![0, 2, 0x84, 1, 0x8C, 0x40, 2, 0x20, 0]].concat();
        b.extend_from_slice(&0u16.to_le_bytes());
        b.extend_from_slice(&76u16.to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes());
        b.extend_from_slice(&(info.len() as u16).to_le_bytes());
        b.extend_from_slice(&76u16.to_le_bytes());
        b.extend_from_slice(&(l.len() as u16).to_le_bytes());
        b.extend(info);
        b.extend(l);
        b
    }

    #[test]
    fn una_l_hecha_a_mano_sale_con_sus_dos_rayas() {
        let f = Shx::leer(&fuente_l()).unwrap();
        assert_eq!(f.arriba, 8.0);
        let l = f.letra('L').unwrap();
        assert_eq!(l.trazos, vec![vec![[0.0, 1.0], [0.0, 0.0], [0.5, 0.0]]]);
        assert!((l.avance - 0.75).abs() < 1e-6);
        // Casos negativos: una letra que no tiene, y basura por fichero.
        assert!(f.letra('Z').is_none());
        assert!(Shx::leer(b"no es una fuente").is_none());
    }

    #[test]
    fn las_fuentes_de_autocad_se_leen_si_estan() {
        let Some(r) = buscar("romans", &carpetas()) else {
            return; // sin AutoCAD en este equipo
        };
        let f = abrir(&r).expect("romans.shx se lee");
        let a = f.letra('A').expect("la A");
        assert!(
            !a.trazos.is_empty() && a.avance > 0.4 && a.avance < 1.5,
            "{:?}",
            a.avance
        );
        // La A llega hasta arriba (mayuscula = 1).
        let alto = a
            .trazos
            .iter()
            .flatten()
            .map(|p| p[1])
            .fold(f32::MIN, f32::max);
        assert!((alto - 1.0).abs() < 0.15, "{alto}");
        assert!(f.letra('ñ').is_some() || f.letra('n').is_some());
    }
}

/// **Las SHX de PixPin**, para cuando falta la de AutoCAD (un equipo sin
/// AutoCAD): las fuentes de Hershey pasadas a SHX, con tildes, ñ, °, ±, ² y Ø,
/// hechas en PixPin Android (v0.111.0, `fuentes/hershey-a-shx.py`); ver
/// `fuentes-shx/LEEME-HERSHEY.txt`. Van dentro del programa: 82 KB.
const DE_PIXPIN: [(&str, &[u8]); 8] = [
    ("romans.shx", include_bytes!("../fuentes-shx/romans.shx")),
    ("romand.shx", include_bytes!("../fuentes-shx/romand.shx")),
    ("romant.shx", include_bytes!("../fuentes-shx/romant.shx")),
    ("italic.shx", include_bytes!("../fuentes-shx/italic.shx")),
    ("scripts.shx", include_bytes!("../fuentes-shx/scripts.shx")),
    ("scriptc.shx", include_bytes!("../fuentes-shx/scriptc.shx")),
    ("gothice.shx", include_bytes!("../fuentes-shx/gothice.shx")),
    ("greeks.shx", include_bytes!("../fuentes-shx/greeks.shx")),
];

/// Cual de las de PixPin va en lugar de una SHX que no esta: la de su nombre
/// o la que mas se le parece (txt, simplex, isocp… → romans), como Android.
/// Solo para lo que es SHX: un estilo «Arial» no pasa por aqui.
pub fn reserva_para(clave: &str) -> Option<&'static str> {
    let c = clave.trim().to_ascii_lowercase();
    let base = c.rsplit(['/', '\\']).next().unwrap_or(&c);
    let base = base.strip_suffix(".shx").unwrap_or(base);
    let conocida = [
        "txt", "simplex", "romans", "romand", "romanc", "romant", "complex", "italic", "italicc",
        "italict", "scripts", "scriptc", "gothice", "gothicg", "gothici", "greeks", "greekc",
        "monotxt", "isocp", "isocp2", "isocp3", "isoct", "isoct2", "isoct3", "isocteur",
        "isocpeur",
    ];
    if !c.ends_with(".shx") && !conocida.contains(&base) {
        return None;
    }
    Some(match base {
        "romand" | "romanc" | "complex" => "romand.shx",
        "romant" => "romant.shx",
        "italic" | "italicc" | "italict" => "italic.shx",
        "scripts" => "scripts.shx",
        "scriptc" => "scriptc.shx",
        "gothice" | "gothicg" | "gothici" => "gothice.shx",
        "greeks" | "greekc" => "greeks.shx",
        _ => "romans.shx",
    })
}

/// La SHX de PixPin que va en lugar de `clave`, ya leida.
pub fn de_reserva(clave: &str) -> Option<Shx> {
    let nombre = reserva_para(clave)?;
    DE_PIXPIN
        .iter()
        .find(|(n, _)| *n == nombre)
        .and_then(|(_, b)| Shx::leer(b))
}

#[cfg(test)]
mod pruebas_reserva {
    use super::*;

    #[test]
    fn las_shx_de_pixpin_se_leen_y_tienen_las_letras_del_castellano() {
        for (n, b) in DE_PIXPIN {
            let f = Shx::leer(b).unwrap_or_else(|| panic!("{n} no se lee"));
            assert!(f.tiene('A') && f.tiene('a'), "{n}");
        }
        let r = de_reserva("romans.shx").unwrap();
        for c in ['ñ', 'á', '°', '±', 'Ø'] {
            assert!(r.tiene(c), "romans sin {c}");
        }
    }

    #[test]
    fn cada_shx_que_falta_va_a_la_que_mas_se_parece() {
        assert_eq!(reserva_para("txt"), Some("romans.shx"));
        assert_eq!(reserva_para("C:\\fuentes\\complex.shx"), Some("romand.shx"));
        assert_eq!(reserva_para("isocp"), Some("romans.shx"));
        assert_eq!(reserva_para("rara.shx"), Some("romans.shx"));
        // Caso negativo: una TrueType no.
        assert_eq!(reserva_para("arial"), None);
        assert_eq!(reserva_para("arial.ttf"), None);
    }
}
