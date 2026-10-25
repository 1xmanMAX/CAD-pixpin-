//! **La planta de un Revit para el dibujo**: lo que cubre cada elemento en
//! planta (el perfil de su cuerpo, girado y en su sitio, con sus huecos;
//! de los demas cuerpos, su envolvente), en metros, con la capa de su
//! categoria y su nivel. Lo mismo que dibuja `render_plan_svg` de `rvt-rs`,
//! pero en coordenadas del modelo, para hacer polilineas.

use std::path::Path;

use rvt::ifc::IfcModel;
use rvt::ifc::body_geometry::{Placement, element_body, plan_outline};
use rvt::ifc::entities::IfcEntity;

use crate::modelo::{categoria_de_clase, clase_canonica};
use crate::revit::PIE;

/// **Un contorno** cerrado de la planta.
#[derive(Debug, Clone, PartialEq)]
pub struct Trazo {
    /// Indice en [`Planta::niveles`].
    pub nivel: Option<usize>,
    /// `BIM-Muros`, `BIM-Puertas`…
    pub capa: String,
    /// En metros, en el plano XY del modelo.
    pub puntos: Vec<[f64; 2]>,
    /// Si es un hueco de otro contorno.
    pub hueco: bool,
}

/// **La planta de un modelo**: sus niveles (nombre y cota en m) y sus trazos.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Planta {
    pub niveles: Vec<(String, f64)>,
    pub trazos: Vec<Trazo>,
}

impl Planta {
    /// Los trazos de un nivel (`None`: todos).
    pub fn del_nivel(&self, nivel: Option<usize>) -> impl Iterator<Item = &Trazo> {
        self.trazos
            .iter()
            .filter(move |t| nivel.is_none() || t.nivel == nivel)
    }

    /// El nivel por su nombre o numero (1, 2…) como lo escribe alguien.
    pub fn buscar_nivel(&self, texto: &str) -> Option<usize> {
        let t = texto.trim();
        if let Ok(n) = t.parse::<usize>() {
            if (1..=self.niveles.len()).contains(&n) {
                return Some(n - 1);
            }
        }
        let t = t.to_lowercase();
        self.niveles
            .iter()
            .position(|(n, _)| n.to_lowercase() == t)
            .or_else(|| {
                self.niveles
                    .iter()
                    .position(|(n, _)| n.to_lowercase().contains(&t))
            })
    }
}

/// La capa de una clase IFC.
pub fn capa_de(clase: &str) -> String {
    format!("BIM-{}", categoria_de_clase(&clase_canonica(clase)))
}

/// **La planta de un modelo de rvt-rs.**
pub fn de_modelo(im: &IfcModel) -> Planta {
    let mut p = Planta {
        niveles: im
            .building_storeys
            .iter()
            .map(|s| (s.name.clone(), s.elevation_feet * PIE))
            .collect(),
        trazos: Vec::new(),
    };
    for ent in &im.entities {
        let IfcEntity::BuildingElement {
            ifc_type,
            location_feet,
            rotation_radians,
            storey_index,
            ..
        } = ent
        else {
            continue;
        };
        // Un hueco es un vacio en su anfitrion, no algo que se dibuje.
        if ifc_type == "IFCOPENINGELEMENT" {
            continue;
        }
        let Some(l) = location_feet else { continue };
        let Some(cuerpo) = element_body(ent, &im.representation_maps) else {
            continue;
        };
        let pl = Placement::new(Some(*l), *rotation_radians);
        let Some((fuera, huecos)) = plan_outline(cuerpo, &pl) else {
            continue;
        };
        let capa = capa_de(ifc_type);
        let a_m = |r: &[(f64, f64)]| -> Vec<[f64; 2]> {
            r.iter().map(|&(x, y)| [x * PIE, y * PIE]).collect()
        };
        if fuera.len() >= 2 {
            p.trazos.push(Trazo {
                nivel: *storey_index,
                capa: capa.clone(),
                puntos: a_m(&fuera),
                hueco: false,
            });
        }
        for h in huecos.iter().filter(|h| h.len() >= 2) {
            p.trazos.push(Trazo {
                nivel: *storey_index,
                capa: capa.clone(),
                puntos: a_m(h),
                hueco: true,
            });
        }
    }
    p
}

/// **La planta de un fichero de Revit.**
pub fn de_revit(ruta: &Path) -> Result<Planta, String> {
    let mut rf =
        rvt::RevitFile::open(ruta).map_err(|e| format!("no es un Revit que se pueda leer: {e}"))?;
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        rvt::ifc::RvtDocExporter.export_with_diagnostics(&mut rf)
    }))
    .map_err(|_| "rvt-rs se detuvo leyendo el Revit".to_string())?
    .map_err(|e| e.to_string())?;
    Ok(de_modelo(&r.model))
}
