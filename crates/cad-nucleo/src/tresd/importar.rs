//! **Traer mallas de otros programas** (STL, OBJ, glTF/GLB, FBX y COLLADA)
//! con los lectores de opencadcodec: cada malla, una entidad MESH en una
//! capa por material, a la escala pedida y, si el fichero va con la Y
//! arriba (glTF siempre; OBJ casi siempre), girada a la Z del dibujo.

use std::path::Path;

use opencadcodec::{Color, EntityType, ImportConfig, Vector3};

/// Las extensiones que se importan.
pub const EXTENSIONES: &[&str] = &["stl", "obj", "gltf", "glb", "fbx", "dae"];

/// Si un fichero de esta extension suele ir con la Y arriba.
pub fn y_arriba_por_defecto(ext: &str) -> bool {
    matches!(ext.to_ascii_lowercase().as_str(), "gltf" | "glb" | "obj")
}

/// En que unidades suele venir un fichero (cuantos metros es su unidad):
/// glTF en metros por norma, OBJ casi siempre (Blender); STL, FBX y
/// COLLADA, en las del dibujo (`None`).
pub fn metros_por_defecto(ext: &str) -> Option<f64> {
    matches!(ext.to_ascii_lowercase().as_str(), "gltf" | "glb" | "obj").then_some(1.0)
}

/// **Lo importado**: las mallas (ya en el dibujo: escala, giro y punto de
/// insercion) y las capas que traen, con su color.
pub struct Importado {
    pub entidades: Vec<EntityType>,
    pub capas: Vec<(String, Color)>,
}

/// **Lee el fichero** y deja sus mallas a `escala`, giradas si `y_arriba`
/// y movidas a `punto`.
pub fn importar(
    ruta: &Path,
    escala: f64,
    y_arriba: bool,
    punto: [f64; 3],
) -> Result<Importado, String> {
    if !escala.is_finite() || escala <= 0.0 {
        return Err("La escala tiene que ser mayor que cero".into());
    }
    let cfg = ImportConfig {
        layer_prefix: "3D".into(),
        scale_factor: escala,
        ..Default::default()
    };
    let ruta = ruta.to_path_buf();
    let doc = std::panic::catch_unwind(move || opencadcodec::import_file(&ruta, &cfg))
        .map_err(|_| "el fichero esta dañado".to_string())?
        .map_err(|e| e.to_string())?;
    let mueve = |v: &mut Vector3| {
        let (x, y, z) = if y_arriba {
            (v.x, -v.z, v.y)
        } else {
            (v.x, v.y, v.z)
        };
        *v = Vector3::new(x + punto[0], y + punto[1], z + punto[2]);
    };
    let mut entidades = Vec::new();
    for e in doc.entities() {
        let mut e = e.clone();
        match &mut e {
            EntityType::Mesh(m) => m.vertices.iter_mut().for_each(mueve),
            EntityType::Face3D(f) => {
                for v in [
                    &mut f.first_corner,
                    &mut f.second_corner,
                    &mut f.third_corner,
                    &mut f.fourth_corner,
                ] {
                    mueve(v);
                }
            }
            EntityType::PolyfaceMesh(p) => {
                p.vertices.iter_mut().for_each(|v| mueve(&mut v.location))
            }
            _ => continue,
        }
        entidades.push(e);
    }
    if entidades.is_empty() {
        return Err("El fichero no trae mallas".into());
    }
    let capas = doc
        .layers
        .iter()
        .filter(|l| entidades.iter().any(|e| e.common().layer == l.name))
        .map(|l| (l.name.clone(), l.color))
        .collect();
    Ok(Importado { entidades, capas })
}
