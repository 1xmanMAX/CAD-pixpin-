//! **STL binario**: todos los triangulos del modelo, en las unidades del
//! dibujo y con la Z arriba (lo de las impresoras 3D y los CAD).

use super::escena::Escena;
use super::geometria::{cruz3, resta3, unitario3};

/// **El STL binario**: 80 bytes de cabecera, cuantos triangulos y cada uno
/// con su normal, sus tres vertices y dos bytes libres.
pub fn escribir(escena: &Escena, nombre: &str) -> Vec<u8> {
    let mut tri: Vec<[[f64; 3]; 3]> = Vec::new();
    for el in &escena.elementos {
        let m = el.malla_completa();
        for t in m.triangulos() {
            tri.push(t.map(|i| m.puntos[i as usize]));
        }
    }
    let mut b = Vec::with_capacity(84 + tri.len() * 50);
    let mut cabeza = format!("CAD-pixpin STL {nombre}").into_bytes();
    // Una cabecera que empiece por «solid» se confunde con un STL de texto.
    cabeza.retain(|c| c.is_ascii());
    cabeza.resize(80, b' ');
    b.extend_from_slice(&cabeza);
    b.extend_from_slice(&(tri.len() as u32).to_le_bytes());
    for t in &tri {
        let n = unitario3(cruz3(resta3(t[1], t[0]), resta3(t[2], t[0])));
        for v in std::iter::once(n).chain(t.iter().copied()) {
            for c in v {
                b.extend_from_slice(&(c as f32).to_le_bytes());
            }
        }
        b.extend_from_slice(&[0, 0]);
    }
    b
}

/// Cuantos triangulos dice un STL binario que tiene.
pub fn triangulos(b: &[u8]) -> u32 {
    b.get(80..84)
        .map_or(0, |c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
}
