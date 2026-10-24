//! Rellenos: el contorno de un sombreado (par-impar, con islas). Las rayas
//! de un patron no se hacen aqui: las pinta la tarjeta (`modelo::Trama`).

use lyon_tessellation::geom::point;
use lyon_tessellation::path::Path;
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, VertexBuffers,
};

/// Los triangulos que rellenan los anillos con la regla par-impar (un
/// anillo dentro de otro es un hueco): puntos e indices, compartidos.
pub fn rellenar(anillos: &[Vec<[f64; 2]>]) -> (Vec<[f64; 2]>, Vec<u32>) {
    // Se trabaja respecto a la primera esquina: lyon va en f32.
    let Some(o) = anillos.iter().flatten().next().copied() else {
        return (Vec::new(), Vec::new());
    };
    let mut b = Path::builder();
    for a in anillos {
        let mut primero = true;
        for p in a {
            let q = point((p[0] - o[0]) as f32, (p[1] - o[1]) as f32);
            if primero {
                b.begin(q);
                primero = false;
            } else {
                b.line_to(q);
            }
        }
        if !primero {
            b.end(true);
        }
    }
    let camino = b.build();
    let mut bufs: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
    let mut t = FillTessellator::new();
    let r = t.tessellate_path(
        &camino,
        &FillOptions::tolerance(0.01).with_fill_rule(FillRule::EvenOdd),
        &mut BuffersBuilder::new(&mut bufs, |v: FillVertex| v.position().to_array()),
    );
    if r.is_err() {
        return (Vec::new(), Vec::new());
    }
    let puntos = bufs
        .vertices
        .iter()
        .map(|v| [v[0] as f64 + o[0], v[1] as f64 + o[1]])
        .collect();
    (puntos, bufs.indices)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn cuadrado(x: f64, y: f64, l: f64) -> Vec<[f64; 2]> {
        vec![[x, y], [x + l, y], [x + l, y + l], [x, y + l]]
    }

    #[test]
    fn un_cuadrado_con_hueco_deja_el_hueco_sin_rellenar() {
        let lleno = rellenar(&[cuadrado(0.0, 0.0, 10.0)]);
        let con_hueco = rellenar(&[cuadrado(0.0, 0.0, 10.0), cuadrado(4.0, 4.0, 2.0)]);
        let area = |(p, i): &(Vec<[f64; 2]>, Vec<u32>)| -> f64 {
            i.chunks_exact(3)
                .map(|c| {
                    let (a, b, d) = (p[c[0] as usize], p[c[1] as usize], p[c[2] as usize]);
                    ((b[0] - a[0]) * (d[1] - a[1]) - (d[0] - a[0]) * (b[1] - a[1])).abs() / 2.0
                })
                .sum()
        };
        assert!((area(&lleno) - 100.0).abs() < 1e-3);
        assert!((area(&con_hueco) - 96.0).abs() < 1e-3);
        assert!(rellenar(&[]).0.is_empty());
    }
}
