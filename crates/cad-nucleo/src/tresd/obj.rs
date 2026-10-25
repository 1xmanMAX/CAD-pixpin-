//! **Wavefront OBJ con su MTL**: un objeto por pieza, un material por capa
//! (y color), en metros y con la Y arriba, como lo escribe y lo espera
//! Blender.

use std::fmt::Write as _;

use super::escena::Escena;

/// Un nombre sin espacios (OBJ y MTL cortan en ellos).
pub fn nombre_limpio(s: &str) -> String {
    let v: String = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if v.is_empty() { "_".into() } else { v }
}

/// Del dibujo (Z arriba, en sus unidades) a metros con la Y arriba.
pub fn y_arriba(p: [f64; 3], k: f64) -> [f64; 3] {
    [p[0] * k, p[2] * k, -p[1] * k]
}

/// **El OBJ y su MTL** (`mtl`: como se llama el MTL, junto al OBJ).
pub fn escribir(escena: &Escena, mtl: &str) -> (String, String) {
    let k = 1.0 / escena.unidades_por_metro();
    let mut o = String::from("# CAD-pixpin\n");
    let _ = writeln!(o, "mtllib {mtl}");
    let mut materiales: Vec<(String, [f32; 4])> = Vec::new();
    let mut base = 1usize;
    for el in &escena.elementos {
        let m = el.malla_completa();
        let capa = nombre_limpio(&el.capa);
        let mat = match materiales
            .iter()
            .position(|(c, col)| *c == capa && *col == el.color)
        {
            Some(i) => i,
            None => {
                materiales.push((capa, el.color));
                materiales.len() - 1
            }
        };
        let _ = writeln!(o, "o {}", nombre_limpio(&el.nombre));
        for p in &m.puntos {
            let q = y_arriba(*p, k);
            let _ = writeln!(o, "v {} {} {}", q[0], q[1], q[2]);
        }
        let _ = writeln!(o, "usemtl {}", nombre_material(&materiales, mat));
        for c in &m.caras {
            if c.len() < 3 {
                continue;
            }
            o.push('f');
            for i in c {
                let _ = write!(o, " {}", *i as usize + base);
            }
            o.push('\n');
        }
        base += m.puntos.len();
    }
    let mut t = String::from("# CAD-pixpin\n");
    for (i, (_, c)) in materiales.iter().enumerate() {
        let _ = writeln!(t, "newmtl {}", nombre_material(&materiales, i));
        let _ = writeln!(t, "Kd {} {} {}", c[0], c[1], c[2]);
        let _ = writeln!(t, "Ka 0 0 0\nKs 0.05 0.05 0.05\nNs 10\nillum 2");
        let _ = writeln!(t, "d {}\n", c[3]);
    }
    (o, t)
}

/// El nombre del material `i`: el de su capa y, si la capa tiene varios
/// colores, un numero.
fn nombre_material(v: &[(String, [f32; 4])], i: usize) -> String {
    let capa = &v[i].0;
    let antes = v[..i].iter().filter(|(c, _)| c == capa).count();
    if antes == 0 {
        capa.clone()
    } else {
        format!("{capa}_{}", antes + 1)
    }
}
