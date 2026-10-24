// Partes traducidas de wordcraft (https://github.com/storytold/wordcraft), MIT OR Apache-2.0, Copyright (c) 2026 ArtCraft Team and the Wordcraft contributors.
//! **El texto alrededor de los dibujos flotantes** (`wp:wrapSquare`,
//! `wrapTight`, `wrapThrough`, `wrapTopAndBottom`; ECMA-376 1.ª parte,
//! §20.4.2.16-20.4.2.18).
//!
//! Cada dibujo anclado que aparta el texto deja en la pagina un
//! [`Obstaculo`]. Para cada renglon, [`tramos`] dice en que trozos de su
//! ancho puede ir el texto (a un lado, al otro o a los dos), o por debajo
//! de que altura tiene que bajar.

use std::sync::Arc;

use crate::docx::dibujo::{Ajuste, Flotante, Lado};

/// El trozo mas estrecho en que Word pone texto al lado de un dibujo.
pub const TRAMO_MINIMO: f32 = 20.0;

/// Un dibujo que aparta el texto en la pagina en curso (coordenadas de la
/// pagina).
#[derive(Debug, Clone, PartialEq)]
pub struct Obstaculo {
    /// La zona que ocupa con sus distancias al texto.
    pub arriba: f32,
    pub abajo: f32,
    pub x0: f32,
    pub x1: f32,
    /// `wrapTopAndBottom`: nada de texto a los lados.
    pub arriba_abajo: bool,
    pub lado: Lado,
    /// `wrapTight`/`wrapThrough` con poligono: el contorno que sigue el
    /// texto, en la pagina.
    pub contorno: Option<Arc<[(f32, f32)]>>,
    pub traspasa: bool,
    /// Distancias al texto: izquierda, derecha, arriba, abajo.
    pub dist: [f32; 4],
}

impl Obstaculo {
    /// El obstaculo de un dibujo colocado en `caja` (x0, y0, x1, y1).
    /// `None` si no aparta el texto (`wrapNone`, delante o detras).
    pub fn de_flotante(caja: (f32, f32, f32, f32), f: &Flotante) -> Option<Obstaculo> {
        if f.ajuste == Ajuste::Ninguno {
            return None;
        }
        let (x0, y0, x1, y1) = caja;
        if !(x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite())
            || x1 <= x0
            || y1 <= y0
        {
            return None;
        }
        let [arriba, abajo, izq, der] = f.margen;
        let dist = [izq, der, arriba, abajo];
        let contorno: Option<Arc<[(f32, f32)]>> = f
            .contorno
            .as_ref()
            .filter(|_| f.ajuste == Ajuste::Cuadrado)
            .map(|c| {
                c.iter()
                    .map(|(u, v)| (x0 + u * (x1 - x0), y0 + v * (y1 - y0)))
                    .collect::<Vec<_>>()
                    .into()
            });
        // Con contorno, la zona es la de sus puntos (puede salirse de la caja).
        let (mut a0, mut b0, mut a1, mut b1) = (x0, y0, x1, y1);
        if let Some(c) = &contorno {
            (a0, b0, a1, b1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for (x, y) in c.iter() {
                (a0, b0, a1, b1) = (a0.min(*x), b0.min(*y), a1.max(*x), b1.max(*y));
            }
        }
        Some(Obstaculo {
            arriba: b0 - arriba,
            abajo: b1 + abajo,
            x0: a0 - izq,
            x1: a1 + der,
            arriba_abajo: f.ajuste == Ajuste::ArribaAbajo,
            lado: f.lado,
            contorno,
            traspasa: f.traspasa,
            dist,
        })
    }

    /// Los trozos (x0, x1) que el contorno y sus distancias ocupan en la
    /// banda `arriba..abajo`, de izquierda a derecha. Estrecho: uno solo, del
    /// punto mas a la izquierda al mas a la derecha; transparente: los
    /// trozos del propio contorno, para que el texto entre en sus huecos.
    fn ocupado(&self, arriba: f32, abajo: f32) -> Vec<(f32, f32)> {
        let Some(pts) = &self.contorno else {
            return vec![(self.x0, self.x1)];
        };
        let [d_izq, d_der, d_arriba, d_abajo] = self.dist;
        // La distancia de arriba del dibujo alcanza a las bandas de encima, y
        // al reves.
        let (y0, y1) = (arriba - d_abajo, abajo + d_arriba);
        if !(y0.is_finite() && y1.is_finite()) || y1 < y0 {
            return Vec::new();
        }
        let n = pts.len();
        let mut trozos: Vec<(f32, f32)> = Vec::new();
        // Lo que cubre el contorno en la banda lo limitan sus lados (recortados
        // a la banda) y las rayas de arriba y abajo de la banda por dentro.
        for (i, &(ax, ay)) in pts.iter().enumerate() {
            let (bx, by) = pts[(i + 1) % n];
            let (lo, hi) = (ay.min(by), ay.max(by));
            if hi < y0 || lo > y1 {
                continue;
            }
            if (by - ay).abs() < 1e-6 {
                trozos.push((ax.min(bx), ax.max(bx)));
                continue;
            }
            let x_en = |y: f32| ax + (bx - ax) * (y - ay) / (by - ay);
            let (xa, xb) = (x_en(lo.max(y0)), x_en(hi.min(y1)));
            trozos.push((xa.min(xb), xa.max(xb)));
        }
        for y in [y0, y1] {
            let mut xs: Vec<f32> = Vec::new();
            for (i, &(ax, ay)) in pts.iter().enumerate() {
                let (bx, by) = pts[(i + 1) % n];
                if (ay <= y) != (by <= y) {
                    xs.push(ax + (bx - ax) * (y - ay) / (by - ay));
                }
            }
            xs.sort_by(f32::total_cmp);
            trozos.extend(xs.chunks_exact(2).map(|p| (p[0], p[1])));
        }
        if trozos.is_empty() {
            return trozos;
        }
        // La distancia a los lados ensancha cada trozo; los que se tocan se
        // juntan.
        let mut trozos: Vec<(f32, f32)> = trozos
            .into_iter()
            .map(|(a, b)| (a - d_izq, b + d_der))
            .collect();
        trozos.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut juntos: Vec<(f32, f32)> = Vec::with_capacity(trozos.len());
        for (a, b) in trozos {
            match juntos.last_mut() {
                Some(u) if a <= u.1 + 0.01 => u.1 = u.1.max(b),
                _ => juntos.push((a, b)),
            }
        }
        if !self.traspasa && juntos.len() > 1 {
            let (a, b) = (juntos[0].0, juntos[juntos.len() - 1].1);
            return vec![(a, b)];
        }
        juntos
    }
}

/// Los trozos de un renglon en `arriba` (de unos `alto` puntos) entre `lo` y
/// `hi` donde puede ir el texto, de izquierda a derecha. `Err(y)`: aqui no
/// cabe, hay que probar mas abajo, en `y` (debajo de lo que estorba).
pub fn tramos(
    obst: &[Obstaculo],
    arriba: f32,
    alto: f32,
    lo: f32,
    hi: f32,
) -> Result<Vec<(f32, f32)>, f32> {
    let aqui: Vec<&Obstaculo> = obst
        .iter()
        .filter(|o| o.abajo > arriba && o.arriba < arriba + alto && o.x1 > lo && o.x0 < hi)
        .collect();
    if aqui.is_empty() {
        return Ok(vec![(lo, hi)]);
    }
    if let Some(debajo) = aqui
        .iter()
        .filter(|o| o.arriba_abajo)
        .map(|o| o.abajo)
        .reduce(f32::max)
    {
        return Err(debajo);
    }
    // Lo que cada dibujo quita del renglon: (izquierda, derecha, abajo).
    let mut quitado: Vec<(f32, f32, f32)> = Vec::new();
    for o in &aqui {
        let desde = quitado.len();
        quitado.extend(
            o.ocupado(arriba, arriba + alto)
                .into_iter()
                .map(|(a, b)| (a, b, o.abajo)),
        );
        if quitado.len() == desde || o.lado == Lado::Ambos {
            continue;
        }
        // Texto a un solo lado: el otro lado del renglon tambien se quita.
        let izq = quitado[desde..]
            .iter()
            .map(|t| t.0)
            .fold(f32::INFINITY, f32::min);
        let der = quitado[desde..]
            .iter()
            .map(|t| t.1)
            .fold(f32::NEG_INFINITY, f32::max);
        let texto_a_la_izquierda = match o.lado {
            Lado::Izquierda => true,
            Lado::Derecha => false,
            // El lado con mas sitio en este renglon (la izquierda si empatan).
            _ => izq - lo >= hi - der,
        };
        quitado.truncate(desde);
        quitado.push(if texto_a_la_izquierda {
            (izq, hi.max(der), o.abajo)
        } else {
            (lo.min(izq), der, o.abajo)
        });
    }
    quitado.retain(|(a, b, _)| *b > lo && *a < hi);
    if quitado.is_empty() {
        return Ok(vec![(lo, hi)]);
    }
    let mut libres = vec![(lo, hi)];
    for &(a, b, _) in &quitado {
        libres = libres
            .into_iter()
            .flat_map(|(x, y)| {
                if b <= x || a >= y {
                    vec![(x, y)]
                } else {
                    vec![(x, a.min(y)), (b.max(x), y)]
                }
            })
            .collect();
    }
    libres.retain(|(a, b)| b - a >= TRAMO_MINIMO);
    if libres.is_empty() {
        // Nada lo bastante ancho: el renglon baja hasta donde acaba el
        // primer dibujo que estorba.
        return Err(quitado
            .iter()
            .map(|t| t.2)
            .reduce(f32::min)
            .unwrap_or(arriba));
    }
    Ok(libres)
}

/// Donde va un renglon que empezaria en `y`: la altura (mas abajo si hay
/// que saltar un dibujo) y sus trozos.
pub fn sitio(obst: &[Obstaculo], y: f32, alto: f32, lo: f32, hi: f32) -> (f32, Vec<(f32, f32)>) {
    let mut y = y;
    for _ in 0..50 {
        match tramos(obst, y, alto, lo, hi) {
            Ok(t) => return (y, t),
            Err(b) if b > y => y = b,
            Err(_) => break,
        }
    }
    (y, vec![(lo, hi)])
}

/// Donde va algo que no se puede partir en trozos (una fila de tabla) de
/// `x0` a `x1`: debajo de todo dibujo que le estorbe.
pub fn debajo(obst: &[Obstaculo], y: f32, alto: f32, x0: f32, x1: f32) -> f32 {
    let mut y = y;
    for _ in 0..50 {
        let b = obst
            .iter()
            .filter(|o| o.abajo > y && o.arriba < y + alto && o.x1 > x0 && o.x0 < x1)
            .map(|o| o.abajo)
            .reduce(f32::max);
        match b {
            Some(b) if b > y => y = b,
            _ => break,
        }
    }
    y
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn caja(x0: f32, y0: f32, x1: f32, y1: f32, lado: Lado) -> Obstaculo {
        Obstaculo {
            arriba: y0,
            abajo: y1,
            x0,
            x1,
            arriba_abajo: false,
            lado,
            contorno: None,
            traspasa: false,
            dist: [0.0; 4],
        }
    }

    #[test]
    fn un_dibujo_a_la_derecha_acorta_el_renglon() {
        let o = [caja(300.0, 100.0, 400.0, 200.0, Lado::Ambos)];
        assert_eq!(tramos(&o, 120.0, 12.0, 0.0, 400.0), Ok(vec![(0.0, 300.0)]));
        // Por encima y por debajo, el renglon entero.
        assert_eq!(tramos(&o, 80.0, 12.0, 0.0, 400.0), Ok(vec![(0.0, 400.0)]));
        assert_eq!(tramos(&o, 200.0, 12.0, 0.0, 400.0), Ok(vec![(0.0, 400.0)]));
    }

    #[test]
    fn a_los_dos_lados_o_al_mayor() {
        let mut o = [caja(100.0, 0.0, 200.0, 100.0, Lado::Ambos)];
        assert_eq!(
            tramos(&o, 10.0, 12.0, 0.0, 400.0),
            Ok(vec![(0.0, 100.0), (200.0, 400.0)])
        );
        o[0].lado = Lado::Mayor;
        assert_eq!(tramos(&o, 10.0, 12.0, 0.0, 400.0), Ok(vec![(200.0, 400.0)]));
        o[0].lado = Lado::Izquierda;
        assert_eq!(tramos(&o, 10.0, 12.0, 0.0, 400.0), Ok(vec![(0.0, 100.0)]));
        o[0].lado = Lado::Derecha;
        assert_eq!(tramos(&o, 10.0, 12.0, 0.0, 400.0), Ok(vec![(200.0, 400.0)]));
    }

    #[test]
    fn arriba_y_abajo_o_sin_sitio_baja_el_renglon() {
        let mut o = [caja(150.0, 0.0, 250.0, 100.0, Lado::Ambos)];
        o[0].arriba_abajo = true;
        assert_eq!(tramos(&o, 10.0, 12.0, 0.0, 400.0), Err(100.0));
        assert_eq!(
            sitio(&o, 10.0, 12.0, 0.0, 400.0),
            (100.0, vec![(0.0, 400.0)])
        );
        // Un dibujo que deja menos de 20 pt a cada lado: el renglon baja.
        let o = [caja(10.0, 0.0, 390.0, 50.0, Lado::Ambos)];
        assert_eq!(sitio(&o, 0.0, 12.0, 0.0, 400.0).0, 50.0);
    }

    #[test]
    fn el_contorno_estrecho_sigue_el_poligono() {
        // Un rombo de 0..100 x 0..100: en la banda de arriba ocupa poco.
        let rombo: Arc<[(f32, f32)]> =
            Arc::from(vec![(50.0, 0.0), (100.0, 50.0), (50.0, 100.0), (0.0, 50.0)]);
        let mut o = caja(0.0, 0.0, 100.0, 100.0, Lado::Derecha);
        o.contorno = Some(rombo);
        let arriba = tramos(std::slice::from_ref(&o), 0.0, 10.0, 0.0, 400.0).unwrap();
        let medio = tramos(std::slice::from_ref(&o), 45.0, 10.0, 0.0, 400.0).unwrap();
        assert!(
            arriba[0].0 < 70.0,
            "arriba el rombo es estrecho: {arriba:?}"
        );
        assert!(
            (medio[0].0 - 100.0).abs() < 0.5,
            "en medio llega al ancho entero: {medio:?}"
        );
    }

    #[test]
    fn sin_dibujos_el_renglon_entero() {
        assert_eq!(tramos(&[], 0.0, 12.0, 10.0, 300.0), Ok(vec![(10.0, 300.0)]));
        assert_eq!(debajo(&[], 5.0, 12.0, 0.0, 100.0), 5.0);
    }
}
