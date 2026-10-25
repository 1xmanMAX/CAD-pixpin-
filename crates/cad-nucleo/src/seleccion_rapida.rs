//! **Seleccion rapida**: elegir lo parecido a lo elegido, todo lo de unas
//! capas o lo contrario de lo elegido. Solo cuenta lo que se puede elegir
//! (lo visible del espacio modelo, fuera de capas bloqueadas), como al elegir
//! con el raton. Cada funcion recorre el dibujo una vez.

use std::collections::HashSet;
use std::mem::Discriminant;

use opencadcodec::{EntityType, Handle};

use crate::dibujo::Dibujo;

/// Los handles que se pueden elegir, en el orden del indice.
fn elegibles(d: &Dibujo) -> Vec<Handle> {
    d.indice()
        .entradas
        .iter()
        .filter(|e| !e.bloqueada)
        .map(|e| e.handle)
        .collect()
}

fn clave(e: &EntityType) -> (Discriminant<EntityType>, String) {
    (std::mem::discriminant(e), e.common().layer.to_uppercase())
}

/// **Lo parecido** a `modelos`: del mismo tipo y en la misma capa que alguno.
pub fn similares(d: &Dibujo, modelos: &[Handle]) -> Vec<Handle> {
    let claves: HashSet<_> = modelos
        .iter()
        .filter_map(|h| d.entidad(*h))
        .map(clave)
        .collect();
    if claves.is_empty() {
        return Vec::new();
    }
    elegibles(d)
        .into_iter()
        .filter(|h| d.entidad(*h).is_some_and(|e| claves.contains(&clave(e))))
        .collect()
}

/// **Todo lo de las capas** `capas` (sin importar mayusculas).
pub fn de_capas(d: &Dibujo, capas: &[String]) -> Vec<Handle> {
    let capas: HashSet<String> = capas.iter().map(|c| c.to_uppercase()).collect();
    elegibles(d)
        .into_iter()
        .filter(|h| {
            d.entidad(*h)
                .is_some_and(|e| capas.contains(&e.common().layer.to_uppercase()))
        })
        .collect()
}

/// **Lo contrario** de `sel`: lo que se puede elegir y no esta elegido.
pub fn invertir(d: &Dibujo, sel: &[Handle]) -> Vec<Handle> {
    let fuera: HashSet<Handle> = sel.iter().copied().collect();
    elegibles(d)
        .into_iter()
        .filter(|h| !fuera.contains(h))
        .collect()
}

/// Las capas de unas entidades, sin repetir (en el orden en que salen).
pub fn capas_de(d: &Dibujo, hs: &[Handle]) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for h in hs {
        if let Some(e) = d.entidad(*h) {
            let c = &e.common().layer;
            if !v.iter().any(|x| x.eq_ignore_ascii_case(c)) {
                v.push(c.clone());
            }
        }
    }
    v
}

/// Lo de `hs` que aun esta en el dibujo y se puede elegir.
pub fn vigentes(d: &Dibujo, hs: &[Handle]) -> Vec<Handle> {
    let ind = d.indice();
    let ok: HashSet<Handle> = ind
        .entradas
        .iter()
        .filter(|e| !e.bloqueada)
        .map(|e| e.handle)
        .collect();
    hs.iter().copied().filter(|h| ok.contains(h)).collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::especialidad::{circulo, linea};

    /// Dos lineas y un circulo en A-MUROS; una linea en E-ESTRUCTURAS.
    fn dibujo() -> (Dibujo, Vec<Handle>) {
        let mut d = Dibujo::nuevo();
        let hs = d.editar("x", |t| {
            vec![
                t.anadir_en(linea([0.0, 0.0], [1.0, 0.0]), "A-MUROS")
                    .unwrap(),
                t.anadir_en(linea([0.0, 1.0], [1.0, 1.0]), "A-MUROS")
                    .unwrap(),
                t.anadir_en(circulo([5.0, 5.0], 1.0), "A-MUROS").unwrap(),
                t.anadir_en(linea([0.0, 2.0], [1.0, 2.0]), "E-ESTRUCTURAS")
                    .unwrap(),
            ]
        });
        (d, hs)
    }

    fn ordenado(mut v: Vec<Handle>) -> Vec<Handle> {
        v.sort();
        v
    }

    #[test]
    fn similares_por_tipo_y_capa() {
        let (d, h) = dibujo();
        assert_eq!(ordenado(similares(&d, &[h[0]])), ordenado(vec![h[0], h[1]]));
        assert_eq!(similares(&d, &[h[2]]), vec![h[2]]);
        assert!(similares(&d, &[]).is_empty());
        assert!(similares(&d, &[Handle::new(0xFF_FFFF)]).is_empty());
    }

    #[test]
    fn por_capas_sin_mayusculas() {
        let (d, h) = dibujo();
        assert_eq!(de_capas(&d, &["e-estructuras".into()]), vec![h[3]]);
        assert_eq!(de_capas(&d, &["A-MUROS".into()]).len(), 3);
        assert!(de_capas(&d, &["NO-HAY".into()]).is_empty());
    }

    #[test]
    fn invertir_y_lo_bloqueado() {
        let (mut d, h) = dibujo();
        assert_eq!(ordenado(invertir(&d, &[h[0]])), ordenado(h[1..].to_vec()));
        assert!(invertir(&d, &h).is_empty());
        d.con_capa("E-ESTRUCTURAS", |l| l.lock());
        assert_eq!(invertir(&d, &[]).len(), 3);
        assert_eq!(vigentes(&d, &h).len(), 3);
        d.con_capa("A-MUROS", |l| l.turn_off());
        assert!(similares(&d, &[h[0]]).is_empty());
    }

    #[test]
    fn capas_sin_repetir() {
        let (d, h) = dibujo();
        assert_eq!(capas_de(&d, &h), vec!["A-MUROS", "E-ESTRUCTURAS"]);
        assert!(capas_de(&d, &[]).is_empty());
    }
}
