//! **glTF 2.0 binario (.glb)**: un nodo y una malla por pieza, un material
//! por capa y color, en metros y con la Y arriba (lo que pide la norma). Los
//! puntos van relativos a un nodo raiz con la traslacion, para que un
//! modelo lejos del cero no pierda los milimetros en `f32`.

use std::fmt::Write as _;

use super::escena::Escena;
use super::geometria::{cruz3, resta3, unitario3};
use super::obj::y_arriba;

/// Un texto JSON entre comillas.
pub fn texto_json(s: &str) -> String {
    let mut r = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => r.push_str("\\\""),
            '\\' => r.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                let _ = write!(r, "\\u{:04x}", c as u32);
            }
            c => r.push(c),
        }
    }
    r.push('"');
    r
}

fn num(v: f64) -> String {
    if v.is_finite() {
        format!("{v}")
    } else {
        "0".into()
    }
}

/// **El GLB**: cabecera, el trozo JSON y el binario.
pub fn escribir_glb(escena: &Escena) -> Vec<u8> {
    let k = 1.0 / escena.unidades_por_metro();
    // El origen: la esquina baja de la caja, ya en metros y Y arriba.
    let o = escena
        .caja()
        .map(|c| y_arriba([c[0], c[1], c[2]], k))
        .unwrap_or([0.0; 3]);
    let mut bin: Vec<u8> = Vec::new();
    let mut vistas = Vec::new();
    let mut accesores = Vec::new();
    let mut mallas = Vec::new();
    let mut nodos = Vec::new();
    let mut materiales: Vec<(String, [f32; 4])> = Vec::new();
    for el in &escena.elementos {
        let m = el.malla_completa();
        // Caras planas: tres vertices por triangulo, con su normal.
        let mut pos: Vec<[f32; 3]> = Vec::new();
        let mut nor: Vec<[f32; 3]> = Vec::new();
        for t in m.triangulos() {
            let p = t.map(|i| {
                let q = y_arriba(m.puntos[i as usize], k);
                [q[0] - o[0], q[1] - o[1], q[2] - o[2]]
            });
            let n = unitario3(cruz3(resta3(p[1], p[0]), resta3(p[2], p[0])));
            for q in p {
                pos.push(q.map(|c| c as f32));
                nor.push(n.map(|c| c as f32));
            }
        }
        if pos.is_empty() {
            continue;
        }
        let mat = match materiales
            .iter()
            .position(|(c, col)| *c == el.capa && *col == el.color)
        {
            Some(i) => i,
            None => {
                materiales.push((el.capa.clone(), el.color));
                materiales.len() - 1
            }
        };
        let (mut min, mut max) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
        for p in &pos {
            for c in 0..3 {
                min[c] = min[c].min(p[c]);
                max[c] = max[c].max(p[c]);
            }
        }
        let mut vista = |datos: &[[f32; 3]]| {
            let inicio = bin.len();
            for v in datos {
                for c in v {
                    bin.extend_from_slice(&c.to_le_bytes());
                }
            }
            vistas.push(format!(
                "{{\"buffer\":0,\"byteOffset\":{inicio},\"byteLength\":{},\"target\":34962}}",
                bin.len() - inicio
            ));
            vistas.len() - 1
        };
        let vp = vista(&pos);
        let vn = vista(&nor);
        let n = pos.len();
        let f = |v: [f32; 3]| format!("[{},{},{}]", v[0], v[1], v[2]);
        accesores.push(format!(
            "{{\"bufferView\":{vp},\"componentType\":5126,\"count\":{n},\"type\":\"VEC3\",\"min\":{},\"max\":{}}}",
            f(min),
            f(max)
        ));
        accesores.push(format!(
            "{{\"bufferView\":{vn},\"componentType\":5126,\"count\":{n},\"type\":\"VEC3\"}}"
        ));
        let (ap, an) = (accesores.len() - 2, accesores.len() - 1);
        mallas.push(format!(
            "{{\"name\":{},\"primitives\":[{{\"attributes\":{{\"POSITION\":{ap},\"NORMAL\":{an}}},\"material\":{mat}}}]}}",
            texto_json(&el.nombre)
        ));
        nodos.push(format!(
            "{{\"name\":{},\"mesh\":{},\"extras\":{{\"capa\":{},\"ifc\":{}}}}}",
            texto_json(&el.nombre),
            mallas.len() - 1,
            texto_json(&el.capa),
            texto_json(el.tipo.clase_ifc())
        ));
    }
    let hijos: Vec<String> = (1..=nodos.len()).map(|i| i.to_string()).collect();
    let mut todos = vec![format!(
        "{{\"name\":\"Modelo\",\"translation\":[{},{},{}],\"children\":[{}]}}",
        num(o[0]),
        num(o[1]),
        num(o[2]),
        hijos.join(",")
    )];
    todos.extend(nodos);
    let mats: Vec<String> = materiales
        .iter()
        .map(|(c, col)| {
            let mezcla = if col[3] < 0.999 {
                ",\"alphaMode\":\"BLEND\""
            } else {
                ""
            };
            format!(
                "{{\"name\":{},\"pbrMetallicRoughness\":{{\"baseColorFactor\":[{},{},{},{}],\"metallicFactor\":0,\"roughnessFactor\":0.9}},\"doubleSided\":true{mezcla}}}",
                texto_json(c),
                col[0],
                col[1],
                col[2],
                col[3]
            )
        })
        .collect();
    let mut json = format!(
        "{{\"asset\":{{\"version\":\"2.0\",\"generator\":\"CAD-pixpin\"}},\"scene\":0,\"scenes\":[{{\"nodes\":[0]}}],\"nodes\":[{}],\"meshes\":[{}],\"materials\":[{}],\"accessors\":[{}],\"bufferViews\":[{}],\"buffers\":[{{\"byteLength\":{}}}]}}",
        todos.join(","),
        mallas.join(","),
        mats.join(","),
        accesores.join(","),
        vistas.join(","),
        bin.len()
    )
    .into_bytes();
    while json.len() % 4 != 0 {
        json.push(b' ');
    }
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let total = 12 + 8 + json.len() + 8 + bin.len();
    let mut b = Vec::with_capacity(total);
    b.extend_from_slice(b"glTF");
    b.extend_from_slice(&2u32.to_le_bytes());
    b.extend_from_slice(&(total as u32).to_le_bytes());
    b.extend_from_slice(&(json.len() as u32).to_le_bytes());
    b.extend_from_slice(b"JSON");
    b.extend_from_slice(&json);
    b.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    b.extend_from_slice(b"BIN\0");
    b.extend_from_slice(&bin);
    b
}
