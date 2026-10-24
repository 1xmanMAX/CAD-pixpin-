//! **El dibujo de reserva de los objetos que no se conocen** (proxy
//! graphics): lo que AutoCAD guarda de un objeto de otra aplicacion (las
//! superficies, alineamientos, puntos, corredores o tuberias de Civil 3D, por
//! ejemplo) para que se vea sin ella. Es una lista de primitivas: rayas,
//! poligonos, circulos, arcos, mallas y caras (con agujeros), textos, cambios
//! de color y transformaciones.
//!
//! Formato: cabecera (tamano total, cuantos registros) y cada registro
//! (tamano, tipo, datos), todo en little-endian, los puntos en `f64`.
//!
//! Si el dibujo se guardo con `PROXYGRAPHICS` en 0, solo queda una caja con
//! el nombre de la clase: eso es una [`Reserva::SoloCaja`] y no se dibuja.

/// Lo que se saca de un dibujo de reserva, ya en coordenadas del mundo.
#[derive(Debug, Clone, PartialEq)]
pub enum Primitiva {
    /// Una polilinea (cerrada si el ultimo punto repite el primero).
    Raya {
        puntos: Vec<[f64; 3]>,
        color: Option<u32>,
    },
    /// Triangulos, de tres en tres puntos.
    Caras {
        puntos: Vec<[f64; 3]>,
        color: Option<u32>,
    },
    Texto {
        pos: [f64; 3],
        dir: [f64; 3],
        alto: f64,
        ancho: f64,
        texto: String,
        color: Option<u32>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Reserva {
    Dibujo(Vec<Primitiva>),
    /// Solo la caja y el nombre de la clase (guardado sin dibujo).
    SoloCaja(String),
}

struct Lector<'a> {
    b: &'a [u8],
    i: usize,
}

impl Lector<'_> {
    fn u(&mut self) -> Option<u32> {
        let v = u32::from_le_bytes(self.b.get(self.i..self.i + 4)?.try_into().ok()?);
        self.i += 4;
        Some(v)
    }
    fn d(&mut self) -> Option<f64> {
        let v = f64::from_le_bytes(self.b.get(self.i..self.i + 8)?.try_into().ok()?);
        self.i += 8;
        v.is_finite().then_some(v)
    }
    fn p(&mut self) -> Option<[f64; 3]> {
        Some([self.d()?, self.d()?, self.d()?])
    }
    fn puntos(&mut self, n: usize) -> Option<Vec<[f64; 3]>> {
        if n > self.b.len() / 24 + 1 {
            return None;
        }
        (0..n).map(|_| self.p()).collect()
    }
}

/// Una matriz 4x4 por filas (la de `PUSH_MODEL_XFORM`).
type Matriz = [[f64; 4]; 4];

const IDENTIDAD: Matriz = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

fn aplicar(m: &Matriz, p: [f64; 3]) -> [f64; 3] {
    let w = m[3][0] * p[0] + m[3][1] * p[1] + m[3][2] * p[2] + m[3][3];
    let w = if w.abs() < 1e-300 { 1.0 } else { w };
    [
        (m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2] + m[0][3]) / w,
        (m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2] + m[1][3]) / w,
        (m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2] + m[2][3]) / w,
    ]
}

fn por(a: &Matriz, b: &Matriz) -> Matriz {
    let mut r = [[0.0; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            r[i][j] = (0..4).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    r
}

fn cruz(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit(a: [f64; 3]) -> [f64; 3] {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    if l < 1e-300 {
        [0.0, 0.0, 1.0]
    } else {
        [a[0] / l, a[1] / l, a[2] / l]
    }
}

/// Un arco en 3D: centro, radio, normal, vector al inicio y barrido.
fn arco(c: [f64; 3], r: f64, n: [f64; 3], inicio: [f64; 3], barrido: f64) -> Vec<[f64; 3]> {
    let n = unit(n);
    let u = unit(inicio);
    let v = cruz(n, u);
    let pasos = ((barrido.abs() / 0.08).ceil() as usize).clamp(8, 256);
    (0..=pasos)
        .map(|k| {
            let t = barrido * k as f64 / pasos as f64;
            let (s, co) = t.sin_cos();
            [
                c[0] + r * (u[0] * co + v[0] * s),
                c[1] + r * (u[1] * co + v[1] * s),
                c[2] + r * (u[2] * co + v[2] * s),
            ]
        })
        .collect()
}

fn circunferencia_3p(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> Option<([f64; 3], f64, [f64; 3])> {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = cruz(ab, ac);
    let nn = n[0] * n[0] + n[1] * n[1] + n[2] * n[2];
    if nn < 1e-24 {
        return None;
    }
    let ab2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
    let ac2 = ac[0] * ac[0] + ac[1] * ac[1] + ac[2] * ac[2];
    let t1 = cruz(n, ab);
    let t2 = cruz(ac, n);
    let k = 1.0 / (2.0 * nn);
    let o = [
        a[0] + (t2[0] * ab2 + t1[0] * ac2) * k,
        a[1] + (t2[1] * ab2 + t1[1] * ac2) * k,
        a[2] + (t2[2] * ab2 + t1[2] * ac2) * k,
    ];
    let r = ((a[0] - o[0]).powi(2) + (a[1] - o[1]).powi(2) + (a[2] - o[2]).powi(2)).sqrt();
    Some((o, r, unit(n)))
}

/// Un color ACI (1..255) o RGB de AutoCAD, como RGBA de [`crate::convertir::rgba`].
fn color_aci(i: u32) -> Option<u32> {
    if i == 0 || i >= 256 {
        return None; // por bloque / por capa: el de la entidad
    }
    if i == 7 {
        return Some(crate::convertir::COLOR_7);
    }
    let (r, g, b) = opencadcodec::types::Color::Index(i as u8).rgb()?;
    Some(crate::convertir::rgba(r, g, b))
}

/// Lee el dibujo de reserva. `None` si los bytes no se entienden.
pub fn leer(datos: &[u8]) -> Option<Reserva> {
    let mut r = Lector { b: datos, i: 0 };
    let total = r.u()? as usize;
    let n = r.u()? as usize;
    if total < 8 || total > datos.len() {
        return None;
    }
    let mut out = Vec::new();
    let mut pila: Vec<Matriz> = vec![IDENTIDAD];
    let mut color: Option<u32> = None;
    let mut tipos = Vec::new();
    let mut textos = Vec::new();
    let mut pos = 8usize;
    for _ in 0..n.min(10_000_000) {
        let mut h = Lector { b: datos, i: pos };
        let tam = h.u()? as usize;
        let tipo = h.u()?;
        if tam < 8 || pos + tam > total {
            return None;
        }
        let fin = pos + tam;
        let mut l = Lector {
            b: &datos[..fin],
            i: pos + 8,
        };
        let m = *pila.last().unwrap_or(&IDENTIDAD);
        let t = |p: [f64; 3]| aplicar(&m, p);
        tipos.push(tipo);
        match tipo {
            2 => {
                // Circulo: centro, radio, normal.
                if let (Some(c), Some(rad), Some(nor)) = (l.p(), l.d(), l.p()) {
                    let u = if nor[2].abs() < 0.9 {
                        cruz(nor, [0.0, 0.0, 1.0])
                    } else {
                        cruz(nor, [1.0, 0.0, 0.0])
                    };
                    let pts = arco(c, rad, nor, u, std::f64::consts::TAU)
                        .into_iter()
                        .map(t)
                        .collect();
                    out.push(Primitiva::Raya { puntos: pts, color });
                }
            }
            3 => {
                if let (Some(a), Some(b), Some(c)) = (l.p(), l.p(), l.p())
                    && let Some((o, rad, nor)) = circunferencia_3p(a, b, c)
                {
                    let u = [a[0] - o[0], a[1] - o[1], a[2] - o[2]];
                    let pts = arco(o, rad, nor, u, std::f64::consts::TAU)
                        .into_iter()
                        .map(t)
                        .collect();
                    out.push(Primitiva::Raya { puntos: pts, color });
                }
            }
            4 => {
                // Arco: centro, radio, normal, vector de inicio, barrido.
                if let (Some(c), Some(rad), Some(nor), Some(u), Some(b)) =
                    (l.p(), l.d(), l.p(), l.p(), l.d())
                {
                    let pts = arco(c, rad, nor, u, b).into_iter().map(t).collect();
                    out.push(Primitiva::Raya { puntos: pts, color });
                }
            }
            5 => {
                if let (Some(a), Some(b), Some(c)) = (l.p(), l.p(), l.p())
                    && let Some((o, rad, nor)) = circunferencia_3p(a, b, c)
                {
                    let ang = |p: [f64; 3], u: [f64; 3], v: [f64; 3]| {
                        let d = [p[0] - o[0], p[1] - o[1], p[2] - o[2]];
                        (d[0] * v[0] + d[1] * v[1] + d[2] * v[2])
                            .atan2(d[0] * u[0] + d[1] * u[1] + d[2] * u[2])
                    };
                    let u = unit([a[0] - o[0], a[1] - o[1], a[2] - o[2]]);
                    let v = cruz(nor, u);
                    let mut fin = ang(c, u, v);
                    if fin <= 0.0 {
                        fin += std::f64::consts::TAU;
                    }
                    let pts = arco(o, rad, nor, u, fin).into_iter().map(t).collect();
                    out.push(Primitiva::Raya { puntos: pts, color });
                }
            }
            6 | 7 | 32 => {
                // Polilinea, poligono (cerrado) y polilinea con normal.
                if let Some(k) = l.u()
                    && let Some(mut pts) = l.puntos(k as usize)
                {
                    if tipo == 7 && pts.len() > 2 {
                        pts.push(pts[0]);
                    }
                    out.push(Primitiva::Raya {
                        puntos: pts.into_iter().map(t).collect(),
                        color,
                    });
                }
            }
            8 => {
                // Malla: filas x columnas de puntos.
                if let (Some(f), Some(c)) = (l.u(), l.u())
                    && let Some(pts) = l.puntos((f as usize).saturating_mul(c as usize))
                {
                    let (f, c) = (f as usize, c as usize);
                    let pts: Vec<[f64; 3]> = pts.into_iter().map(t).collect();
                    let mut tri = Vec::new();
                    for i in 0..f.saturating_sub(1) {
                        for j in 0..c.saturating_sub(1) {
                            let (a, b, cc, d) = (
                                pts[i * c + j],
                                pts[i * c + j + 1],
                                pts[(i + 1) * c + j + 1],
                                pts[(i + 1) * c + j],
                            );
                            tri.extend([a, b, cc, a, cc, d]);
                        }
                    }
                    out.push(Primitiva::Caras { puntos: tri, color });
                }
            }
            9 => {
                // Cascara: vertices y lista de caras (n, indices; n < 0 es
                // un agujero, que se salta).
                if let Some(nv) = l.u()
                    && let Some(pts) = l.puntos(nv as usize)
                {
                    let pts: Vec<[f64; 3]> = pts.into_iter().map(t).collect();
                    if let Some(nl) = l.u() {
                        let mut lista = Vec::with_capacity(nl as usize);
                        for _ in 0..nl.min(50_000_000) {
                            match l.u() {
                                Some(v) => lista.push(v as i32),
                                None => break,
                            }
                        }
                        let mut tri = Vec::new();
                        let mut i = 0;
                        while i < lista.len() {
                            let k = lista[i];
                            let cuantos = k.unsigned_abs() as usize;
                            let cara = &lista
                                [(i + 1).min(lista.len())..(i + 1 + cuantos).min(lista.len())];
                            if k > 2 {
                                let v: Vec<usize> = cara
                                    .iter()
                                    .map(|&x| x as usize)
                                    .filter(|&x| x < pts.len())
                                    .collect();
                                if v.len() == cuantos {
                                    for j in 1..v.len() - 1 {
                                        tri.extend([pts[v[0]], pts[v[j]], pts[v[j + 1]]]);
                                    }
                                }
                            }
                            i += 1 + cuantos;
                        }
                        out.push(Primitiva::Caras { puntos: tri, color });
                    }
                }
            }
            14 => color = l.u().and_then(color_aci),
            22 => {
                // Color verdadero: metodo en el byte alto (0xC2 = RGB).
                if let Some(v) = l.u() {
                    let metodo = v >> 24;
                    color = match metodo {
                        0xC2 => Some(crate::convertir::rgba(
                            (v >> 16) as u8,
                            (v >> 8) as u8,
                            v as u8,
                        )),
                        0xC3 => color_aci(v & 0xff),
                        _ => None,
                    };
                }
            }
            29 | 30 => {
                let mut mm = [[0.0; 4]; 4];
                let mut ok = true;
                for fila in mm.iter_mut() {
                    for x in fila.iter_mut() {
                        match l.d() {
                            Some(v) => *x = v,
                            None => ok = false,
                        }
                    }
                }
                let base = *pila.last().unwrap_or(&IDENTIDAD);
                pila.push(if ok { por(&base, &mm) } else { base });
            }
            31 => {
                if pila.len() > 1 {
                    pila.pop();
                }
            }
            10 | 11 => {
                // Texto ANSI: posicion, normal, direccion, alto, ancho, oblicuo y la cadena.
                if let (Some(p), Some(_n), Some(d), Some(h), Some(w), Some(_o)) =
                    (l.p(), l.p(), l.p(), l.d(), l.d(), l.d())
                {
                    let resto = &datos[l.i.min(fin)..fin];
                    let txt: String = resto
                        .iter()
                        .take_while(|&&b| b != 0)
                        .map(|&b| b as char)
                        .collect();
                    if !txt.trim().is_empty() {
                        out.push(Primitiva::Texto {
                            pos: t(p),
                            dir: d,
                            alto: h,
                            ancho: w,
                            texto: txt,
                            color,
                        });
                    }
                }
            }
            36 | 38 => {
                if let (Some(p), Some(_n), Some(d), Some(h), Some(w), Some(_o)) =
                    (l.p(), l.p(), l.p(), l.d(), l.d(), l.d())
                {
                    // Despues de los numeros, la cadena UTF-16 hasta su cero.
                    let mut i = l.i;
                    let mut u16s = Vec::new();
                    while i + 1 < fin {
                        let c = u16::from_le_bytes([datos[i], datos[i + 1]]);
                        if c == 0 {
                            break;
                        }
                        u16s.push(c);
                        i += 2;
                    }
                    let txt = String::from_utf16_lossy(&u16s);
                    textos.push(txt.clone());
                    if !txt.trim().is_empty() {
                        out.push(Primitiva::Texto {
                            pos: t(p),
                            dir: d,
                            alto: h,
                            ancho: w,
                            texto: txt,
                            color,
                        });
                    }
                }
            }
            _ => {}
        }
        pos = fin;
    }
    // Solo la caja: un texto «Clase (Aplicacion)» y unas rayas.
    let solo_caja =
        textos.len() == 1 && tipos.iter().all(|t| matches!(t, 36 | 38 | 6 | 21 | 1)) && {
            let t = textos[0].trim();
            t.ends_with(')')
                && t.contains(" (")
                && !t.split(" (").next().unwrap_or(" ").contains(' ')
        };
    Some(if solo_caja {
        Reserva::SoloCaja(textos[0].clone())
    } else {
        Reserva::Dibujo(out)
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn registro(tipo: u32, datos: &[u8]) -> Vec<u8> {
        let mut v = ((datos.len() + 8) as u32).to_le_bytes().to_vec();
        v.extend(tipo.to_le_bytes());
        v.extend_from_slice(datos);
        v
    }

    fn metafile(registros: &[Vec<u8>]) -> Vec<u8> {
        let cuerpo: Vec<u8> = registros.concat();
        let mut v = ((cuerpo.len() + 8) as u32).to_le_bytes().to_vec();
        v.extend((registros.len() as u32).to_le_bytes());
        v.extend(cuerpo);
        v
    }

    fn d(v: &[f64]) -> Vec<u8> {
        v.iter().flat_map(|x| x.to_le_bytes()).collect()
    }

    fn texto(s: &str) -> Vec<u8> {
        let mut v = d(&[0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 2.5, 1.0, 0.0]);
        for c in s.encode_utf16() {
            v.extend(c.to_le_bytes());
        }
        v.extend([0, 0]);
        while v.len() % 4 != 0 {
            v.push(0);
        }
        v
    }

    #[test]
    fn una_polilinea_con_color_y_una_cascara() {
        let mut pl = 2u32.to_le_bytes().to_vec();
        pl.extend(d(&[0.0, 0.0, 5.0, 10.0, 0.0, 6.0]));
        let mut sh = 4u32.to_le_bytes().to_vec();
        sh.extend(d(&[
            0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 0.0,
        ]));
        sh.extend(5u32.to_le_bytes());
        for x in [4i32, 0, 1, 2, 3] {
            sh.extend(x.to_le_bytes());
        }
        let m = metafile(&[
            registro(14, &1u32.to_le_bytes()),
            registro(6, &pl),
            registro(9, &sh),
        ]);
        let Some(Reserva::Dibujo(p)) = leer(&m) else {
            panic!("no se leyo")
        };
        assert_eq!(p.len(), 2);
        match &p[0] {
            Primitiva::Raya { puntos, color } => {
                assert_eq!(puntos[1], [10.0, 0.0, 6.0]);
                assert_eq!(*color, Some(crate::convertir::rgba(255, 0, 0)));
            }
            otro => panic!("{otro:?}"),
        }
        match &p[1] {
            Primitiva::Caras { puntos, .. } => assert_eq!(puntos.len(), 6),
            otro => panic!("{otro:?}"),
        }
    }

    #[test]
    fn la_caja_de_civil_sin_dibujo_se_reconoce() {
        let mut pl = 2u32.to_le_bytes().to_vec();
        pl.extend(d(&[0.0, 0.0, 0.0, 10.0, 0.0, 0.0]));
        let m = metafile(&[
            registro(36, &texto("AeccDbSurfaceTin (AeccLand130)")),
            registro(6, &pl),
            registro(21, &[]),
        ]);
        assert_eq!(
            leer(&m),
            Some(Reserva::SoloCaja("AeccDbSurfaceTin (AeccLand130)".into()))
        );
        // Caso negativo: un texto normal con rayas es dibujo de verdad.
        let m = metafile(&[registro(36, &texto("PK 0+120")), registro(6, &pl)]);
        assert!(matches!(leer(&m), Some(Reserva::Dibujo(_))));
        // Y unos bytes rotos no se leen.
        assert_eq!(leer(&[1, 2, 3]), None);
    }

    #[test]
    fn la_transformacion_se_aplica_y_se_quita() {
        let mut mx = d(&[
            1.0, 0.0, 0.0, 100.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ]);
        mx.truncate(128);
        let mut pl = 2u32.to_le_bytes().to_vec();
        pl.extend(d(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0]));
        let m = metafile(&[
            registro(29, &mx),
            registro(6, &pl),
            registro(31, &[]),
            registro(6, &pl),
        ]);
        let Some(Reserva::Dibujo(p)) = leer(&m) else {
            panic!()
        };
        let x0 = |p: &Primitiva| match p {
            Primitiva::Raya { puntos, .. } => puntos[0][0],
            _ => f64::NAN,
        };
        assert_eq!(x0(&p[0]), 100.0);
        assert_eq!(x0(&p[1]), 0.0);
    }
}
