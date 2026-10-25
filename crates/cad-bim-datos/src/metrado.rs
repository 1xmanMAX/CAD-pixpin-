//! **El metrado con los datos del modelo**: por nivel, categoria, familia y
//! tipo, con sus materiales y las cantidades que trae el propio modelo (los
//! Qto del IFC, las medidas de rvt-rs), en unidades SI. Completa el metrado
//! por mallas de `cad_nucleo::metrados` (que mide lo que se ve en 3D).

use std::collections::BTreeMap;

use crate::modelo::*;

/// **Una partida**: un tipo en un nivel.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FilaMetrado {
    pub nivel: String,
    pub categoria: String,
    pub familia: String,
    pub tipo: String,
    pub materiales: String,
    pub cantidad: usize,
    /// m.
    pub longitud: f64,
    /// m².
    pub area: f64,
    /// m³.
    pub volumen: f64,
    /// Cuantos elementos no traian volumen.
    pub sin_volumen: usize,
}

/// Los nombres de las cantidades, por orden de preferencia.
const VOLUMEN: &[&str] = &["NetVolume", "GrossVolume", "Volume", "Volumen"];
const AREA: &[&str] = &[
    "NetSideArea",
    "NetArea",
    "NetFloorArea",
    "GrossSideArea",
    "GrossArea",
    "GrossFloorArea",
    "Area",
    "Area en planta",
];
const LONGITUD: &[&str] = &["Length", "NetLength", "GrossLength", "Longitud"];

/// La primera de estas cantidades que tenga el elemento (en SI).
pub fn cantidad(e: &ElementoBim2, nombres: &[&str]) -> Option<f64> {
    nombres.iter().find_map(|n| {
        e.conjuntos
            .iter()
            .filter(|c| matches!(c.tipo, TipoConjunto::Cantidades | TipoConjunto::Propiedades))
            .find_map(|c| c.buscar(n))
            .and_then(Propiedad::numero_si)
    })
}

/// **El metrado del modelo** por nivel, categoria y tipo (los niveles por
/// su cota; lo sin nivel al final). Los espacios y huecos no cuentan.
pub fn por_tipo(m: &ModeloBim) -> Vec<FilaMetrado> {
    let mut filas: BTreeMap<(usize, String, String, String), FilaMetrado> = BTreeMap::new();
    for e in &m.elementos {
        if matches!(e.clase.as_str(), "IfcSpace" | "IfcOpeningElement") {
            continue;
        }
        let k = e.nivel.unwrap_or(usize::MAX);
        let f = filas
            .entry((k, e.categoria.clone(), e.familia.clone(), e.tipo.clone()))
            .or_insert_with(|| FilaMetrado {
                nivel: m.nivel_de(e).unwrap_or("(sin nivel)").to_string(),
                categoria: e.categoria.clone(),
                familia: e.familia.clone(),
                tipo: e.tipo.clone(),
                ..Default::default()
            });
        f.cantidad += 1;
        for x in &e.materiales {
            if !f.materiales.split("; ").any(|y| y == x) {
                if !f.materiales.is_empty() {
                    f.materiales.push_str("; ");
                }
                f.materiales.push_str(x);
            }
        }
        match cantidad(e, VOLUMEN) {
            Some(v) => f.volumen += v,
            None => f.sin_volumen += 1,
        }
        f.area += cantidad(e, AREA).unwrap_or(0.0);
        f.longitud += cantidad(e, LONGITUD).unwrap_or(0.0);
    }
    filas.into_values().collect()
}

/// **El metrado en CSV** (con BOM para Excel).
pub fn csv(filas: &[FilaMetrado]) -> String {
    let mut t = crate::exportar::Tabla {
        columnas: [
            "Nivel",
            "Categoria",
            "Familia",
            "Tipo",
            "Materiales",
            "Cantidad",
            "Longitud (m)",
            "Area (m2)",
            "Volumen (m3)",
            "Sin volumen",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect(),
        ..Default::default()
    };
    use crate::exportar::Celda::{Numero, Texto};
    for f in filas {
        t.filas.push(vec![
            Texto(f.nivel.clone()),
            Texto(f.categoria.clone()),
            Texto(f.familia.clone()),
            Texto(f.tipo.clone()),
            Texto(f.materiales.clone()),
            Numero(f.cantidad as f64),
            Numero(f.longitud),
            Numero(f.area),
            Numero(f.volumen),
            Numero(f.sin_volumen as f64),
        ]);
    }
    crate::exportar::csv(&t)
}
