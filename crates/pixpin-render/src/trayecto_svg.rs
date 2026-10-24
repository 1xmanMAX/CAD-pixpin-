//! El atributo `d` de un `<path>` de SVG, traducido a tres ordenes.
//!
//! Existe para pintar los iconos de Excalidraw (MIT) tal cual los dibuja el
//! navegador. Direct2D sabe de lineas y de Bezier cubicas; el resto de SVG
//! se reduce a eso aqui: coordenadas relativas a absolutas, `H`/`V` a
//! lineas, `S`/`T` con su control reflejado, cuadraticas elevadas a cubicas
//! y arcos elipticos partidos en cubicas de 90 grados como mucho.
//!
//! Puro: se prueba sin GPU, y el pintor solo recorre la lista.

/// Un tramo ya en coordenadas absolutas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tramo {
    Mover((f32, f32)),
    Linea((f32, f32)),
    Cubica {
        c1: (f32, f32),
        c2: (f32, f32),
        fin: (f32, f32),
    },
    /// Cierra la figura en curso volviendo a su primer punto.
    Cerrar,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErrorTrayecto {
    #[error("orden desconocida '{0}' en el trayecto")]
    OrdenDesconocida(char),
    #[error("faltan numeros tras la orden '{0}'")]
    FaltanNumeros(char),
    #[error("el trayecto no empieza con M")]
    SinInicio,
}

/// Lector de numeros de un trayecto: separa por espacios, comas, signos y
/// segundos puntos decimales (`.5.5` son dos numeros), como hace SVG.
struct Lector<'a> {
    b: &'a [u8],
    i: usize,
}

impl Lector<'_> {
    fn saltar_separadores(&mut self) {
        while matches!(
            self.b.get(self.i),
            Some(b' ' | b',' | b'\t' | b'\n' | b'\r')
        ) {
            self.i += 1;
        }
    }

    /// La siguiente letra de orden, si lo siguiente es una letra.
    fn orden(&mut self) -> Option<char> {
        self.saltar_separadores();
        let c = *self.b.get(self.i)?;
        if c.is_ascii_alphabetic() {
            self.i += 1;
            Some(c as char)
        } else {
            None
        }
    }

    fn hay_numero(&mut self) -> bool {
        self.saltar_separadores();
        matches!(self.b.get(self.i), Some(c) if c.is_ascii_digit() || matches!(c, b'-' | b'+' | b'.'))
    }

    fn numero(&mut self) -> Option<f32> {
        self.saltar_separadores();
        let inicio = self.i;
        if matches!(self.b.get(self.i), Some(b'-' | b'+')) {
            self.i += 1;
        }
        let mut punto = false;
        let mut digitos = false;
        while let Some(&c) = self.b.get(self.i) {
            if c.is_ascii_digit() {
                digitos = true;
                self.i += 1;
            } else if c == b'.' && !punto {
                punto = true;
                self.i += 1;
            } else if (c == b'e' || c == b'E') && digitos {
                // Exponente: solo si detras hay de verdad un numero.
                let mut j = self.i + 1;
                if matches!(self.b.get(j), Some(b'-' | b'+')) {
                    j += 1;
                }
                if !matches!(self.b.get(j), Some(d) if d.is_ascii_digit()) {
                    break;
                }
                self.i = j;
                while matches!(self.b.get(self.i), Some(d) if d.is_ascii_digit()) {
                    self.i += 1;
                }
                break;
            } else {
                break;
            }
        }
        if !digitos {
            self.i = inicio;
            return None;
        }
        std::str::from_utf8(&self.b[inicio..self.i])
            .ok()?
            .parse()
            .ok()
    }

    /// Las banderas de un arco son un solo digito y pueden ir pegadas:
    /// `a1 1 0 011 1` es valido.
    fn bandera(&mut self) -> Option<bool> {
        self.saltar_separadores();
        let c = *self.b.get(self.i)?;
        let valor = match c {
            b'0' => false,
            b'1' => true,
            _ => return None,
        };
        self.i += 1;
        Some(valor)
    }
}

/// Traduce el atributo `d` a tramos absolutos.
pub fn analizar(d: &str) -> Result<Vec<Tramo>, ErrorTrayecto> {
    let mut l = Lector {
        b: d.as_bytes(),
        i: 0,
    };
    let mut salida = Vec::new();
    let mut actual = (0.0f32, 0.0f32);
    let mut inicio_figura = (0.0f32, 0.0f32);
    // El ultimo control, para reflejarlo en S (cubico) y T (cuadratico).
    let mut ultimo_c2: Option<(f32, f32)> = None;
    let mut ultimo_q: Option<(f32, f32)> = None;
    let mut orden: Option<char> = None;

    loop {
        let letra = match l.orden() {
            Some(c) => c,
            None => {
                if !l.hay_numero() {
                    break;
                }
                // Numeros sin letra repiten la orden anterior; tras M, son L.
                match orden {
                    Some('M') => 'L',
                    Some('m') => 'l',
                    Some(c) if !matches!(c, 'Z' | 'z') => c,
                    _ => return Err(ErrorTrayecto::SinInicio),
                }
            }
        };
        if salida.is_empty() && !matches!(letra, 'M' | 'm') {
            return Err(ErrorTrayecto::SinInicio);
        }
        let base = if letra.is_ascii_lowercase() {
            actual
        } else {
            (0.0, 0.0)
        };
        let falta = ErrorTrayecto::FaltanNumeros(letra);
        let num = |l: &mut Lector| l.numero().ok_or_else(|| falta.clone());
        let par = |l: &mut Lector| -> Result<(f32, f32), ErrorTrayecto> {
            let x = num(l)?;
            let y = num(l)?;
            Ok((x + base.0, y + base.1))
        };

        let mut c2_nuevo = None;
        let mut q_nuevo = None;
        match letra.to_ascii_uppercase() {
            'M' => {
                actual = par(&mut l)?;
                inicio_figura = actual;
                salida.push(Tramo::Mover(actual));
            }
            'L' => {
                actual = par(&mut l)?;
                salida.push(Tramo::Linea(actual));
            }
            'H' => {
                actual = (num(&mut l)? + base.0, actual.1);
                salida.push(Tramo::Linea(actual));
            }
            'V' => {
                actual = (actual.0, num(&mut l)? + base.1);
                salida.push(Tramo::Linea(actual));
            }
            'C' => {
                let c1 = par(&mut l)?;
                let c2 = par(&mut l)?;
                let fin = par(&mut l)?;
                salida.push(Tramo::Cubica { c1, c2, fin });
                actual = fin;
                c2_nuevo = Some(c2);
            }
            'S' => {
                let c1 = reflejar(ultimo_c2, actual);
                let c2 = par(&mut l)?;
                let fin = par(&mut l)?;
                salida.push(Tramo::Cubica { c1, c2, fin });
                actual = fin;
                c2_nuevo = Some(c2);
            }
            'Q' => {
                let q = par(&mut l)?;
                let fin = par(&mut l)?;
                salida.push(cuadratica(actual, q, fin));
                actual = fin;
                q_nuevo = Some(q);
            }
            'T' => {
                let q = reflejar(ultimo_q, actual);
                let fin = par(&mut l)?;
                salida.push(cuadratica(actual, q, fin));
                actual = fin;
                q_nuevo = Some(q);
            }
            'A' => {
                let rx = num(&mut l)?;
                let ry = num(&mut l)?;
                let giro = num(&mut l)?;
                let grande = l.bandera().ok_or_else(|| falta.clone())?;
                let horario = l.bandera().ok_or_else(|| falta.clone())?;
                let fin = par(&mut l)?;
                arco(&mut salida, actual, (rx, ry), giro, (grande, horario), fin);
                actual = fin;
            }
            'Z' => {
                salida.push(Tramo::Cerrar);
                actual = inicio_figura;
            }
            _ => return Err(ErrorTrayecto::OrdenDesconocida(letra)),
        }
        ultimo_c2 = c2_nuevo;
        ultimo_q = q_nuevo;
        orden = Some(letra);
    }
    Ok(salida)
}

fn reflejar(control: Option<(f32, f32)>, actual: (f32, f32)) -> (f32, f32) {
    match control {
        Some(c) => (2.0 * actual.0 - c.0, 2.0 * actual.1 - c.1),
        None => actual,
    }
}

/// Una cuadratica es una cubica con los controles a dos tercios.
fn cuadratica(p0: (f32, f32), q: (f32, f32), fin: (f32, f32)) -> Tramo {
    Tramo::Cubica {
        c1: (
            p0.0 + 2.0 / 3.0 * (q.0 - p0.0),
            p0.1 + 2.0 / 3.0 * (q.1 - p0.1),
        ),
        c2: (
            fin.0 + 2.0 / 3.0 * (q.0 - fin.0),
            fin.1 + 2.0 / 3.0 * (q.1 - fin.1),
        ),
        fin,
    }
}

/// Arco eliptico de SVG (notacion de extremos) a cubicas. Es la conversion
/// a centro de la especificacion (SVG 1.1, F.6.5 y F.6.6) y despues una
/// cubica por cada cuarto de vuelta o menos.
fn arco(
    salida: &mut Vec<Tramo>,
    p0: (f32, f32),
    radios: (f32, f32),
    giro_grados: f32,
    (grande, horario): (bool, bool),
    fin: (f32, f32),
) {
    let (x1, y1) = (p0.0 as f64, p0.1 as f64);
    let (x2, y2) = (fin.0 as f64, fin.1 as f64);
    if (x1 - x2).abs() < 1e-9 && (y1 - y2).abs() < 1e-9 {
        return;
    }
    let (mut rx, mut ry) = ((radios.0 as f64).abs(), (radios.1 as f64).abs());
    if rx < 1e-9 || ry < 1e-9 {
        salida.push(Tramo::Linea(fin));
        return;
    }
    let fi = (giro_grados as f64).to_radians();
    let (cos, sin) = (fi.cos(), fi.sin());
    let dx = (x1 - x2) / 2.0;
    let dy = (y1 - y2) / 2.0;
    let x1p = cos * dx + sin * dy;
    let y1p = -sin * dx + cos * dy;
    // Radios demasiado cortos para unir los extremos: se agrandan (F.6.6).
    let lambda = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry);
    if lambda > 1.0 {
        let s = lambda.sqrt();
        rx *= s;
        ry *= s;
    }
    let num = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p;
    let den = rx * rx * y1p * y1p + ry * ry * x1p * x1p;
    let mut coef = (num / den).max(0.0).sqrt();
    if grande == horario {
        coef = -coef;
    }
    let cxp = coef * rx * y1p / ry;
    let cyp = -coef * ry * x1p / rx;
    let cx = cos * cxp - sin * cyp + (x1 + x2) / 2.0;
    let cy = sin * cxp + cos * cyp + (y1 + y2) / 2.0;

    let angulo = |ux: f64, uy: f64, vx: f64, vy: f64| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    let ux = (x1p - cxp) / rx;
    let uy = (y1p - cyp) / ry;
    let vx = (-x1p - cxp) / rx;
    let vy = (-y1p - cyp) / ry;
    let theta1 = angulo(1.0, 0.0, ux, uy);
    let mut delta = angulo(ux, uy, vx, vy);
    let tau = std::f64::consts::TAU;
    if !horario && delta > 0.0 {
        delta -= tau;
    } else if horario && delta < 0.0 {
        delta += tau;
    }

    let partes = (delta.abs() / (tau / 4.0) - 1e-9).ceil().max(1.0) as usize;
    let paso = delta / partes as f64;
    // Longitud de los controles para aproximar un arco de `paso` radianes.
    let k = 4.0 / 3.0 * (paso / 4.0).tan();
    let punto = |t: f64| -> (f64, f64) {
        let (x, y) = (rx * t.cos(), ry * t.sin());
        (cos * x - sin * y + cx, sin * x + cos * y + cy)
    };
    let derivada = |t: f64| -> (f64, f64) {
        let (x, y) = (-rx * t.sin(), ry * t.cos());
        (cos * x - sin * y, sin * x + cos * y)
    };
    for i in 0..partes {
        let t1 = theta1 + paso * i as f64;
        let t2 = t1 + paso;
        let (a, da) = (punto(t1), derivada(t1));
        let (b, db) = (punto(t2), derivada(t2));
        salida.push(Tramo::Cubica {
            c1: ((a.0 + k * da.0) as f32, (a.1 + k * da.1) as f32),
            c2: ((b.0 - k * db.0) as f32, (b.1 - k * db.1) as f32),
            // El ultimo cae EXACTO en el extremo pedido: el redondeo de la
            // trigonometria dejaria una rendija al cerrar la figura.
            fin: if i + 1 == partes {
                fin
            } else {
                (b.0 as f32, b.1 as f32)
            },
        });
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn cerca(a: (f32, f32), b: (f32, f32)) -> bool {
        (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
    }

    fn fin_de(t: &Tramo) -> (f32, f32) {
        match *t {
            Tramo::Mover(p) | Tramo::Linea(p) => p,
            Tramo::Cubica { fin, .. } => fin,
            Tramo::Cerrar => panic!("cerrar no tiene fin"),
        }
    }

    #[test]
    fn las_relativas_se_suman_al_punto_actual() {
        let t = analizar("M6 6l4 1h2v-3H1V2z").unwrap();
        assert_eq!(
            t,
            vec![
                Tramo::Mover((6.0, 6.0)),
                Tramo::Linea((10.0, 7.0)),
                Tramo::Linea((12.0, 7.0)),
                Tramo::Linea((12.0, 4.0)),
                Tramo::Linea((1.0, 4.0)),
                Tramo::Linea((1.0, 2.0)),
                Tramo::Cerrar,
            ]
        );
    }

    #[test]
    fn los_numeros_pegados_se_separan_como_en_el_navegador() {
        // `.5.25` son dos numeros y `-.2` un negativo sin cero delante.
        let t = analizar("M0.5.25L-.2-1e1").unwrap();
        assert_eq!(t[0], Tramo::Mover((0.5, 0.25)));
        assert_eq!(t[1], Tramo::Linea((-0.2, -10.0)));
    }

    #[test]
    fn numeros_tras_m_son_lineas_y_tras_z_el_punto_vuelve_al_inicio() {
        let t = analizar("m1 1 2 0 0 2zl1 0").unwrap();
        assert_eq!(t[1], Tramo::Linea((3.0, 1.0)));
        assert_eq!(t[2], Tramo::Linea((3.0, 3.0)));
        assert_eq!(t[3], Tramo::Cerrar);
        // Tras Z el punto actual es el del ultimo M: (1,1) + (1,0).
        assert_eq!(t[4], Tramo::Linea((2.0, 1.0)));
    }

    #[test]
    fn s_refleja_el_control_de_la_cubica_anterior() {
        let t = analizar("M0 0C0 1 1 1 1 0S2 -1 2 0").unwrap();
        match t[2] {
            Tramo::Cubica { c1, .. } => assert!(cerca(c1, (1.0, -1.0)), "{c1:?}"),
            _ => panic!("esperaba cubica"),
        }
    }

    #[test]
    fn un_circulo_de_dos_arcos_pasa_por_sus_extremos_y_cierra_exacto() {
        // Asi dibuja Excalidraw el icono de la elipse.
        let t = analizar("M3 12A9 9 0 1 0 21 12A9 9 0 1 0 3 12Z").unwrap();
        let cubicas: Vec<_> = t
            .iter()
            .filter(|x| matches!(x, Tramo::Cubica { .. }))
            .collect();
        assert_eq!(cubicas.len(), 4, "dos medias vueltas, dos cuartos cada una");
        assert!(cerca(fin_de(cubicas[1]), (21.0, 12.0)));
        assert_eq!(fin_de(cubicas[3]), (3.0, 12.0), "cierra sin rendija");
        for c in cubicas {
            let f = fin_de(c);
            let r = ((f.0 - 12.0).powi(2) + (f.1 - 12.0).powi(2)).sqrt();
            assert!((r - 9.0).abs() < 1e-3, "radio {r}");
        }
    }

    #[test]
    fn un_cuarto_de_arco_se_desvia_del_circulo_menos_de_una_milesima_del_radio() {
        let t = analizar("M10 0A10 10 0 0 1 0 10").unwrap();
        let Tramo::Cubica { c1, c2, fin } = t[1] else {
            panic!("esperaba cubica");
        };
        // Punto medio de la cubica (t = 0,5).
        let p0 = (10.0f32, 0.0f32);
        let m = (
            0.125 * p0.0 + 0.375 * c1.0 + 0.375 * c2.0 + 0.125 * fin.0,
            0.125 * p0.1 + 0.375 * c1.1 + 0.375 * c2.1 + 0.125 * fin.1,
        );
        let r = (m.0 * m.0 + m.1 * m.1).sqrt();
        assert!((r - 10.0).abs() < 0.01, "radio en el medio {r}");
    }

    #[test]
    fn las_banderas_del_arco_pueden_ir_pegadas() {
        let a = analizar("M0 0a1 1 0 011 1").unwrap();
        let b = analizar("M0 0a1 1 0 0 1 1 1").unwrap();
        assert_eq!(a, b);
        assert!(cerca(fin_de(a.last().unwrap()), (1.0, 1.0)));
    }

    #[test]
    fn un_arco_de_radio_cero_es_una_linea() {
        let t = analizar("M0 0A0 0 0 0 1 5 5").unwrap();
        assert_eq!(t[1], Tramo::Linea((5.0, 5.0)));
    }

    #[test]
    fn lo_que_no_es_svg_se_rechaza_sin_panico() {
        assert_eq!(analizar("L1 1"), Err(ErrorTrayecto::SinInicio));
        assert_eq!(
            analizar("M1 1X2"),
            Err(ErrorTrayecto::OrdenDesconocida('X'))
        );
        assert_eq!(analizar("M1"), Err(ErrorTrayecto::FaltanNumeros('M')));
        assert_eq!(analizar(""), Ok(vec![]));
    }

    #[test]
    fn todos_los_iconos_de_excalidraw_se_leen_sin_error() {
        // El inventario extraido de icons.tsx: si alguno no se lee, ese
        // icono saldria en blanco en la interfaz.
        let ruta = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/excalidraw/iconos.json"
        );
        let Ok(texto) = std::fs::read_to_string(ruta) else {
            return;
        };
        let mut leidos = 0;
        for trozo in texto.split("\"d\": \"").skip(1) {
            let d = &trozo[..trozo.find('"').unwrap()];
            if let Err(e) = analizar(d) {
                panic!("{e} en {d}");
            }
            leidos += 1;
        }
        assert!(leidos > 500, "solo {leidos} trazados");
    }
}
