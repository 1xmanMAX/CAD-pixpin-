//! **El 3D desde la planta**: levantar muros, losas y contornos con su
//! altura (como hacen el banco BIM de FreeCAD o EXTRUDE y THICKNESS de
//! AutoCAD), sacar el modelo a OBJ, STL, glTF e IFC4, y traer mallas de
//! otros programas.
//!
//! Todo lo 3D vive en el dibujo como entidades MESH (se guarda en DWG y DXF
//! y lo abre cualquier CAD); lo que se exporta o se ve en 3D se saca de el
//! con [`Escena::de_documento`]. Que es cada pieza (muro, losa, puerta…) lo
//! dice su capa, como en los planos de siempre.

pub mod escena;
pub mod geometria;
pub mod gltf;
pub mod ifc;
pub mod importar;
pub mod modelado;
pub mod obj;
pub mod stl;

use std::path::Path;

pub use escena::Escena;
pub use geometria::{Malla, P3, Prisma};

/// **Que es una pieza del modelo** (la clase IFC con que se exporta).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tipo {
    Muro,
    Losa,
    Puerta,
    Ventana,
    Columna,
    Viga,
    /// Lo demas (un mueble, un volumen, una cara 3D…).
    Otro,
}

impl Tipo {
    /// **El tipo por la capa** (`A-MUROS` es muro, `A-LOSAS` losa…), sin
    /// mirar mayusculas ni tildes.
    pub fn de_capa(capa: &str) -> Tipo {
        let c = crate::ordenes::sin_tildes(capa).to_uppercase();
        let tiene = |vs: &[&str]| vs.iter().any(|v| c.contains(v));
        if tiene(&["MURO", "WALL", "TABIQUE", "PARED"]) {
            Tipo::Muro
        } else if tiene(&["LOSA", "SLAB", "FLOOR", "FORJADO", "PISO", "TECHO"]) {
            Tipo::Losa
        } else if tiene(&["PUERTA", "DOOR"]) {
            Tipo::Puerta
        } else if tiene(&["VENTANA", "WINDOW", "MAMPARA"]) {
            Tipo::Ventana
        } else if tiene(&["COLUMNA", "COLUMN", "PILAR"]) {
            Tipo::Columna
        } else if tiene(&["VIGA", "BEAM"]) {
            Tipo::Viga
        } else {
            Tipo::Otro
        }
    }

    /// La clase IFC4.
    pub fn clase_ifc(self) -> &'static str {
        match self {
            Tipo::Muro => "IfcWall",
            Tipo::Losa => "IfcSlab",
            Tipo::Puerta => "IfcDoor",
            Tipo::Ventana => "IfcWindow",
            Tipo::Columna => "IfcColumn",
            Tipo::Viga => "IfcBeam",
            Tipo::Otro => "IfcBuildingElementProxy",
        }
    }

    /// Como se llama en castellano.
    pub fn nombre(self) -> &'static str {
        match self {
            Tipo::Muro => "Muro",
            Tipo::Losa => "Losa",
            Tipo::Puerta => "Puerta",
            Tipo::Ventana => "Ventana",
            Tipo::Columna => "Columna",
            Tipo::Viga => "Viga",
            Tipo::Otro => "Elemento",
        }
    }
}

/// **Cuantas unidades del dibujo es un metro**, segun `$INSUNITS` (4 mm, 5
/// cm, 6 m, 1 pulgadas, 2 pies). Sin unidades se suponen milimetros, como
/// en [`crate::sesion::Ajustes::para`].
pub fn unidades_por_metro(insunits: i16) -> f64 {
    match insunits {
        1 => 1.0 / 0.0254,
        2 => 1.0 / 0.3048,
        5 => 100.0,
        6 => 1.0,
        7 => 0.001,
        _ => 1000.0,
    }
}

/// **Los formatos de [`exportar`]**, por su extension.
pub const FORMATOS: &[(&str, &str)] = &[
    ("IFC4", "ifc"),
    ("glTF binario", "glb"),
    ("OBJ con colores (MTL)", "obj"),
    ("STL binario", "stl"),
    ("DXF (mallas)", "dxf"),
    ("DWG (mallas)", "dwg"),
];

/// **Escribe el modelo en `ruta`**, en el formato de su extension. Devuelve
/// lo que se escribio, para el historial.
pub fn exportar(escena: &Escena, ruta: &Path) -> Result<String, String> {
    let ext = ruta
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if escena.elementos.is_empty() {
        return Err("No hay nada en 3D que exportar (use EXTRUIR, MURO3D o LOSA)".into());
    }
    let nombre = ruta
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "modelo".into());
    let escribir = |b: &[u8]| std::fs::write(ruta, b).map_err(|e| e.to_string());
    let n = escena.elementos.len();
    match ext.as_str() {
        "ifc" => {
            escribir(ifc::escribir(escena, &nombre).as_bytes())?;
            Ok(format!("IFC4 con {n} elementos"))
        }
        "glb" => {
            escribir(&gltf::escribir_glb(escena))?;
            Ok(format!("glTF binario con {n} nodos"))
        }
        "obj" => {
            let mtl = format!("{nombre}.mtl");
            let (o, m) = obj::escribir(escena, &mtl);
            escribir(o.as_bytes())?;
            std::fs::write(ruta.with_extension("mtl"), m).map_err(|e| e.to_string())?;
            Ok(format!("OBJ con {n} objetos y su {mtl}"))
        }
        "stl" => {
            let b = stl::escribir(escena, &nombre);
            escribir(&b)?;
            Ok(format!("STL con {} triangulos", stl::triangulos(&b)))
        }
        "dxf" | "dwg" => {
            let mut d = escena.a_dibujo();
            d.guardar_como(ruta)?;
            Ok(format!("{} con {n} mallas", ext.to_uppercase()))
        }
        _ => Err(format!(
            "No se exportar a «.{ext}»: use .ifc, .glb, .obj, .stl, .dxf o .dwg"
        )),
    }
}
