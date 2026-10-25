//! **Letras de trazos** para exportar los textos como rayas (PDF y SVG sin
//! fuentes): el lector de formas SHX copiado de `pixpin-cad/src/shx.rs`
//! (que no compila fuera de Windows) y la `romans.shx` de PixPin (hecha con
//! las fuentes de Hershey, ver `fuentes/LEEME-HERSHEY.txt`).

use std::sync::LazyLock;

use crate::geo::{self, P};
use std::collections::HashMap;

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

/// La `romans.shx` de PixPin, leida una vez.
static ROMANS: LazyLock<Option<Shx>> =
    LazyLock::new(|| Shx::leer(include_bytes!("../../fuentes/romans.shx")));

/// **Lo que mide de largo** un texto de una linea en la romans, en alturas
/// de mayuscula (sin el factor de ancho).
pub fn largo(texto: &str) -> f64 {
    let Some(f) = ROMANS.as_ref() else {
        return texto.chars().count() as f64 * 0.8;
    };
    texto
        .chars()
        .map(|c| f.letra(c).map_or(0.8, |l| l.avance as f64))
        .sum()
}

/// **Las rayas de un texto** de una linea con la romans: el inicio de la
/// linea base en `origen`, mayusculas de `alto`, girado `giro` radianes y
/// estirado `ancho` a lo largo. Cada raya, una polilinea.
pub fn rayas(texto: &str, origen: P, alto: f64, giro: f64, ancho: f64) -> Vec<Vec<P>> {
    let Some(f) = ROMANS.as_ref() else {
        return Vec::new();
    };
    let mut x = 0.0;
    let mut v = Vec::new();
    for c in texto.chars() {
        let Some(l) = f.letra(c).or_else(|| f.letra('?')) else {
            x += 0.8;
            continue;
        };
        for t in &l.trazos {
            v.push(
                t.iter()
                    .map(|q| {
                        let local = [(x + q[0] as f64) * alto * ancho, q[1] as f64 * alto];
                        geo::sumar(origen, geo::rotar(local, [0.0, 0.0], giro))
                    })
                    .collect(),
            );
        }
        x += l.avance as f64;
    }
    v
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_romans_se_lee_y_da_rayas() {
        assert!(ROMANS.is_some());
        let r = rayas("AB", [10.0, 0.0], 2.0, 0.0, 1.0);
        assert!(r.len() >= 3);
        // Todo a la derecha del origen y con la mayuscula de 2 de alto.
        let alto = r.iter().flatten().map(|p| p[1]).fold(0.0_f64, f64::max);
        assert!((alto - 2.0).abs() < 0.2, "{alto}");
        assert!(r.iter().flatten().all(|p| p[0] >= 9.9));
        assert!(largo("AB") > 1.0);
    }
}
