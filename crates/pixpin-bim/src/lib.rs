//! pixpin-bim — leer modelos BIM para verlos en PixPin como un pin.
//!
//! - IFC con `ifc-lite` (MPL-2.0): de cada elemento, sus triangulos con su
//!   color.
//! - Revit (.rvt) con `rvt-rs` (Apache-2.0): se pasa a IFC4 y se lee igual.
//!   Revit es un formato cerrado; `rvt-rs` saca muros, losas, columnas y
//!   vigas de los proyectos de Revit 2023 a 2025, y las puertas, ventanas y
//!   muebles como los dibujo Revit (o como cajas). Lo demas no sale.
//!
//! - Civil 3D ([`civil`]): LandXML y ficheros de puntos.
//!
//! El resultado es un [`pixpin_cad::modelo3d::Modelo3d`].

// `!(x > 0.0)` y parecidos son a proposito: un NaN no cumple la
// comparacion y se rechaza con ella (`partial_cmp` lo esconderia).
#![allow(clippy::neg_cmp_op_on_partial_ord)]

pub mod civil;
pub mod niveles;

use std::path::Path;

use ifc_lite_processing::{
    OpeningFilterMode, TessellationQuality, process_geometry_filtered_with_quality_and_ids,
};
use pixpin_cad::modelo3d::{Constructor3d, Modelo3d, Nivel};

/// Las extensiones que se abren aqui siempre (BIM).
pub fn se_abre(extension: &str) -> bool {
    matches!(extension.to_ascii_lowercase().as_str(), "ifc" | "rvt")
}

/// Lo que se abre aqui mirando el contenido: un LandXML o un fichero de
/// puntos (un .xml, .csv o .txt cualquiera no).
pub fn es_de_civil(ruta: &Path) -> bool {
    let ext = ruta
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    (ext == "xml" && civil::es_landxml(ruta)) || civil::es_fichero_de_puntos(ruta)
}

/// Pila grande: las operaciones booleanas de IFC recurren hondo (lo mismo
/// que hacen los enlaces de `ifc-lite`).
const PILA: usize = 256 * 1024 * 1024;

/// Lee un IFC o un Revit. En otro hilo con pila grande.
pub fn convertir_fichero(ruta: &Path) -> Result<Modelo3d, String> {
    let ruta = ruta.to_path_buf();
    std::thread::Builder::new()
        .stack_size(PILA)
        .name("bim-leer".into())
        .spawn(move || {
            let ext = ruta
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            // Un plano DWG o DXF visto en 3D (el boton «3D» del visor de planos).
            if matches!(ext.as_str(), "dwg" | "dxf") {
                return pixpin_cad::convertir3d::convertir3d_fichero(&ruta);
            }
            if ext == "xml" {
                let t = std::fs::read(&ruta).map_err(|e| e.to_string())?;
                return civil::de_landxml(&String::from_utf8_lossy(&t));
            }
            if !matches!(ext.as_str(), "ifc" | "rvt") {
                let t = std::fs::read(&ruta).map_err(|e| e.to_string())?;
                let nombre = ruta
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                return civil::de_puntos(&String::from_utf8_lossy(&t), &nombre);
            }
            let ifc = if ext == "rvt" {
                revit_a_ifc(&ruta)?
            } else {
                std::fs::read(&ruta).map_err(|e| e.to_string())?
            };
            Ok(de_ifc(&ifc))
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "el modelo no se pudo leer".to_string())?
}

/// Lo que no se dibuja: volumenes de aire, huecos y anotaciones.
fn se_esconde(tipo: &str) -> bool {
    matches!(
        tipo.to_ascii_lowercase().as_str(),
        "ifcspace"
            | "ifcopeningelement"
            | "ifcopeningstandardcase"
            | "ifcvirtualelement"
            | "ifcannotation"
            | "ifczone"
            | "ifcspatialzone"
    )
}

/// De los bytes de un IFC al modelo.
pub fn de_ifc(ifc: &[u8]) -> Modelo3d {
    let r = process_geometry_filtered_with_quality_and_ids(
        ifc,
        OpeningFilterMode::Default,
        TessellationQuality::default(),
        None,
    );
    let mut c = Constructor3d::nuevo();
    let mut elementos: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    // Los elementos sin `GlobalId` de `ifc-lite`: se busca en el texto.
    let mut sin_global: std::collections::HashSet<u32> = std::collections::HashSet::new();
    for m in &r.meshes {
        // Solo lo colocado en el modelo: no las formas de los «tipos».
        if matches!(m.geometry_class, 1 | 2) || m.indices.is_empty() || se_esconde(&m.ifc_type) {
            continue;
        }
        let e = *elementos.entry(m.express_id).or_insert_with(|| {
            let e = c.elemento(&m.ifc_type, m.name.as_deref().unwrap_or(""));
            // Su clave estable (la de los marcadores): el GlobalId.
            match m
                .global_id
                .as_deref()
                .map(str::trim)
                .filter(|g| !g.is_empty())
            {
                Some(g) => c.poner_clave(e, g),
                None => {
                    sin_global.insert(m.express_id);
                }
            }
            e
        });
        let o = m.origin;
        let puntos: Vec<[f64; 3]> = m
            .positions
            .chunks_exact(3)
            .map(|p| [p[0] as f64 + o[0], p[1] as f64 + o[1], p[2] as f64 + o[2]])
            .collect();
        let normales: Vec<[f32; 3]> = m
            .normals
            .chunks_exact(3)
            .map(|n| [n[0], n[1], n[2]])
            .collect();
        let normales = (normales.len() == puntos.len()).then_some(normales.as_slice());
        c.malla(e, &puntos, normales, &m.indices, m.color);
    }
    // Los niveles (plantas) y en cual esta cada elemento.
    let esp = niveles::de_ifc_con(ifc, &sin_global);
    for (id, g) in &esp.globales {
        if let Some(e) = elementos.get(id) {
            c.poner_clave(*e, g);
        }
    }
    if !esp.niveles.is_empty() {
        let indice: std::collections::HashMap<u32, u32> = esp
            .niveles
            .iter()
            .enumerate()
            .map(|(k, n)| (n.id, k as u32))
            .collect();
        let de: Vec<(u32, u32)> = elementos
            .iter()
            .filter_map(|(id, e)| Some((*e, *indice.get(esp.de.get(id)?)?)))
            .collect();
        c.niveles(
            esp.niveles
                .into_iter()
                .map(|n| Nivel {
                    nombre: n.nombre,
                    cota: n.cota,
                    clave: n.global,
                })
                .collect(),
            &de,
        );
    }
    c.terminar()
}

/// Un Revit, a IFC4 con su geometria (y los muebles como Revit los guardo).
fn revit_a_ifc(ruta: &Path) -> Result<Vec<u8>, String> {
    use rvt::ifc::{ExportQualityMode, RvtDocExporter};
    let mut rf =
        rvt::RevitFile::open(ruta).map_err(|e| format!("no es un Revit que se pueda leer: {e}"))?;
    let r = RvtDocExporter
        .export_with_diagnostics_mode_and_limits(
            &mut rf,
            ExportQualityMode::Scaffold,
            rvt::walker::WalkerLimits::default(),
        )
        .map_err(|e| e.to_string())?;
    let mut modelo = r.model;
    if let Err(e) = rvt::ifc::saved_meshes::attach(&mut rf, &mut modelo) {
        tracing::info!(%e, "revit: sin las mallas guardadas");
    }
    Ok(rvt::ifc::write_step(&modelo).into_bytes())
}
