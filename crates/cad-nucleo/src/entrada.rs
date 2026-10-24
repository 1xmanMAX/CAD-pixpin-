//! **Lo que se escribe en la linea de ordenes**: coordenadas como en AutoCAD
//! (`100,50` absolutas, `@20,0` relativas, `@300<45` polares) o una
//! distancia sola, que va en la direccion del cursor.

use crate::geo::{self, P};

#[derive(Debug, Clone, PartialEq)]
pub enum Entrada {
    Punto(P),
    /// Un numero solo: una distancia, un radio, un angulo… segun la orden.
    Numero(f64),
    /// Una opcion (`C`, `S`, `A1`…) o un texto.
    Texto(String),
    /// Enter sin nada.
    Vacio,
}

fn numero(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    s.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Interpreta lo escrito; `ultimo` es el punto anterior (para `@`).
pub fn interpretar(texto: &str, ultimo: Option<P>) -> Entrada {
    let t = texto.trim();
    if t.is_empty() {
        return Entrada::Vacio;
    }
    if let Some(v) = numero(t) {
        return Entrada::Numero(v);
    }
    let (relativo, resto) = match t.strip_prefix('@') {
        Some(r) => (true, r),
        None => (false, t),
    };
    let base = if relativo {
        ultimo.unwrap_or([0.0, 0.0])
    } else {
        [0.0, 0.0]
    };
    if let Some((d, a)) = resto.split_once('<')
        && let (Some(d), Some(a)) = (numero(d), numero(a))
    {
        return Entrada::Punto(geo::polar(base, d, a.to_radians()));
    }
    let partes: Vec<&str> = resto.split(',').collect();
    if (2..=3).contains(&partes.len())
        && let (Some(x), Some(y)) = (numero(partes[0]), numero(partes[1]))
    {
        return Entrada::Punto([base[0] + x, base[1] + y]);
    }
    if relativo && resto.is_empty() {
        return Entrada::Punto(base);
    }
    Entrada::Texto(t.to_string())
}

/// Una lista de numeros separados por comas o espacios (`5000,4000 4500`),
/// con `3*5000` para repetir.
pub fn lista(texto: &str) -> Option<Vec<f64>> {
    let mut v = Vec::new();
    for parte in texto
        .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .filter(|p| !p.is_empty())
    {
        if let Some((n, d)) = parte.split_once('*') {
            let n: usize = n.trim().parse().ok()?;
            let d = numero(d)?;
            v.extend(std::iter::repeat_n(d, n.min(500)));
        } else {
            v.push(numero(parte)?);
        }
    }
    (!v.is_empty()).then_some(v)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn coordenadas_como_en_autocad() {
        assert_eq!(interpretar("100,50", None), Entrada::Punto([100.0, 50.0]));
        assert_eq!(
            interpretar("@20,-5", Some([1.0, 1.0])),
            Entrada::Punto([21.0, -4.0])
        );
        let Entrada::Punto(p) = interpretar("@10<90", Some([0.0, 0.0])) else {
            panic!()
        };
        assert!(p[0].abs() < 1e-9 && (p[1] - 10.0).abs() < 1e-9);
        assert_eq!(interpretar(" 250 ", None), Entrada::Numero(250.0));
        assert_eq!(interpretar("c", None), Entrada::Texto("c".into()));
        assert_eq!(interpretar("", None), Entrada::Vacio);
        assert_eq!(interpretar("1,2,3", None), Entrada::Punto([1.0, 2.0]));
        // Casos negativos: lo que no es numero no se cuela.
        assert_eq!(interpretar("a,b", None), Entrada::Texto("a,b".into()));
        assert_eq!(interpretar("inf", None), Entrada::Texto("inf".into()));
    }

    #[test]
    fn listas_de_separaciones() {
        assert_eq!(lista("5000,4000 4500"), Some(vec![5000.0, 4000.0, 4500.0]));
        assert_eq!(lista("3*6000"), Some(vec![6000.0; 3]));
        assert_eq!(lista("x"), None);
        assert_eq!(lista(""), None);
    }
}
