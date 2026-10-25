//! **Metrados** (computo de cantidades): lo que mide lo dibujado, por capa
//! (cuantos objetos, cuanta longitud, cuanta area cerrada) y, de un modelo
//! BIM, por nivel y tipo de elemento. Salen como tabla y como CSV, para
//! llevarlos a la hoja del presupuesto.

use std::collections::BTreeMap;

use crate::dibujo::Dibujo;
use crate::formas;

/// Una fila del metrado de un plano.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FilaPlano {
    pub capa: String,
    pub cantidad: usize,
    /// Suma de largos (unidades del dibujo).
    pub longitud: f64,
    /// Suma de las areas de lo cerrado (unidades del dibujo al cuadrado).
    pub area: f64,
    /// Cuantos de los objetos eran cerrados.
    pub cerrados: usize,
}

/// **El metrado del plano por capa**: de todo, o solo de `elegidos`.
pub fn del_plano(d: &Dibujo, elegidos: Option<&[opencadcodec::Handle]>) -> Vec<FilaPlano> {
    let mut filas: BTreeMap<String, FilaPlano> = BTreeMap::new();
    for e in d.doc.model_space_entities() {
        let c = e.common();
        if elegidos.is_some_and(|v| !v.contains(&c.handle)) {
            continue;
        }
        let f = filas.entry(c.layer.clone()).or_insert_with(|| FilaPlano {
            capa: c.layer.clone(),
            ..Default::default()
        });
        let (largo, area) = formas::largo_y_area(e);
        f.cantidad += 1;
        f.longitud += largo;
        if let Some(a) = area {
            f.area += a;
            f.cerrados += 1;
        }
    }
    filas.into_values().collect()
}

/// Un elemento de un modelo BIM ya medido (lo da el visor 3D: tipo IFC,
/// nivel y medidas en metros).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ElementoBim {
    pub tipo: String,
    pub nombre: String,
    pub nivel: Option<String>,
    pub volumen: f64,
    /// Area en planta (la huella).
    pub planta: f64,
    /// Area de la cara mayor en alzado (un muro: su superficie a pintar de un lado).
    pub alzado: f64,
    /// Toda la superficie.
    pub superficie: f64,
    /// Si la malla es cerrada (si no, el volumen no es de fiar).
    pub cerrado: bool,
}

/// Una fila del metrado de un modelo: un tipo en un nivel.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FilaBim {
    pub nivel: String,
    pub tipo: String,
    pub cantidad: usize,
    pub volumen: f64,
    pub planta: f64,
    pub alzado: f64,
    pub superficie: f64,
    /// Cuantos no eran cerrados (su volumen no cuenta).
    pub sin_volumen: usize,
}

/// El nombre corto de un tipo IFC para la tabla (`IfcWallStandardCase` -> `Muro`).
pub fn tipo_legible(tipo: &str) -> &str {
    let t = tipo.to_ascii_lowercase();
    let t = t.trim_start_matches("ifc");
    match t {
        "wall" | "wallstandardcase" | "wallelementedcase" => "Muros",
        "slab" | "slabstandardcase" | "slabelementedcase" => "Losas",
        "column" | "columnstandardcase" => "Columnas",
        "beam" | "beamstandardcase" => "Vigas",
        "footing" => "Zapatas",
        "pile" => "Pilotes",
        "door" | "doorstandardcase" => "Puertas",
        "window" | "windowstandardcase" => "Ventanas",
        "stair" | "stairflight" => "Escaleras",
        "railing" => "Barandas",
        "roof" => "Techos",
        "covering" => "Revestimientos",
        "plate" => "Placas",
        "member" => "Elementos",
        "furnishingelement" | "furniture" => "Mobiliario",
        "flowterminal" | "sanitaryterminal" => "Aparatos sanitarios",
        "pipesegment" | "pipefitting" => "Tuberias",
        "ductsegment" | "ductfitting" => "Ductos",
        "cablecarriersegment" | "cablesegment" => "Bandejas y cables",
        "lightfixture" => "Luminarias",
        _ => tipo,
    }
}

/// **El metrado del modelo**, por nivel y tipo (los niveles en su orden de
/// aparicion; los sin nivel al final).
pub fn del_modelo(elementos: &[ElementoBim]) -> Vec<FilaBim> {
    let mut orden: Vec<String> = Vec::new();
    let mut filas: BTreeMap<(usize, String), FilaBim> = BTreeMap::new();
    for e in elementos {
        let nivel = e.nivel.clone().unwrap_or_else(|| "(sin nivel)".into());
        let k = match orden.iter().position(|n| *n == nivel) {
            Some(k) => k,
            None => {
                orden.push(nivel.clone());
                orden.len() - 1
            }
        };
        let k = if e.nivel.is_none() { usize::MAX } else { k };
        let tipo = tipo_legible(&e.tipo).to_string();
        let f = filas.entry((k, tipo.clone())).or_insert_with(|| FilaBim {
            nivel: nivel.clone(),
            tipo,
            ..Default::default()
        });
        f.cantidad += 1;
        if e.cerrado {
            f.volumen += e.volumen;
        } else {
            f.sin_volumen += 1;
        }
        f.planta += e.planta;
        f.alzado += e.alzado;
        f.superficie += e.superficie;
    }
    filas.into_values().collect()
}

/// Un campo de CSV (con comillas si hace falta).
fn campo(s: &str) -> String {
    if s.contains([',', '"', '\n', ';']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// El metrado del plano en CSV. `por_metro`: cuantas unidades del dibujo es
/// un metro (1000 en un plano en milimetros), para dar m y m2.
pub fn csv_plano(filas: &[FilaPlano], por_metro: f64) -> String {
    let k = if por_metro > 0.0 { por_metro } else { 1.0 };
    let mut s = String::from("Capa,Cantidad,Longitud (m),Cerrados,Area (m2)\n");
    for f in filas {
        s += &format!(
            "{},{},{:.3},{},{:.3}\n",
            campo(&f.capa),
            f.cantidad,
            f.longitud / k,
            f.cerrados,
            f.area / (k * k)
        );
    }
    s
}

pub fn csv_modelo(filas: &[FilaBim]) -> String {
    let mut s = String::from(
        "Nivel,Partida,Cantidad,Volumen (m3),Area en planta (m2),Area en alzado (m2),Superficie (m2),Sin volumen\n",
    );
    for f in filas {
        s += &format!(
            "{},{},{},{:.3},{:.3},{:.3},{:.3},{}\n",
            campo(&f.nivel),
            campo(&f.tipo),
            f.cantidad,
            f.volumen,
            f.planta,
            f.alzado,
            f.superficie,
            f.sin_volumen
        );
    }
    s
}

/// Unidades del dibujo por metro, segun `$INSUNITS`.
pub fn unidades_por_metro(insunits: i16) -> f64 {
    match insunits {
        4 => 1000.0,
        5 => 100.0,
        6 => 1.0,
        1 => 39.37,
        2 => 3.2808,
        _ => 1000.0,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::especialidad::{circulo, linea, polilinea};

    #[test]
    fn metrado_por_capa() {
        let mut d = Dibujo::nuevo();
        d.editar("x", |t| {
            t.anadir_en(linea([0.0, 0.0], [3000.0, 0.0]), "A-MUROS");
            t.anadir_en(linea([0.0, 0.0], [0.0, 2000.0]), "A-MUROS");
            t.anadir_en(
                polilinea(
                    &[[0.0, 0.0], [1000.0, 0.0], [1000.0, 1000.0], [0.0, 1000.0]],
                    true,
                ),
                "A-PISOS",
            );
            t.anadir_en(circulo([0.0, 0.0], 1.0), "0");
        });
        let f = del_plano(&d, None);
        let muros = f.iter().find(|f| f.capa == "A-MUROS").unwrap();
        assert_eq!(muros.cantidad, 2);
        assert!((muros.longitud - 5000.0).abs() < 1e-9);
        let pisos = f.iter().find(|f| f.capa == "A-PISOS").unwrap();
        assert!((pisos.area - 1_000_000.0).abs() < 1e-6);
        let csv = csv_plano(&f, 1000.0);
        assert!(csv.contains("A-MUROS,2,5.000,0,0.000"), "{csv}");
        assert!(csv.contains("A-PISOS,1,4.000,1,1.000"), "{csv}");
    }

    #[test]
    fn metrado_del_modelo_por_nivel_y_partida() {
        let el = |tipo: &str, nivel: Option<&str>, v: f64, cerrado: bool| ElementoBim {
            tipo: tipo.into(),
            nivel: nivel.map(Into::into),
            volumen: v,
            cerrado,
            ..Default::default()
        };
        let f = del_modelo(&[
            el("IfcWallStandardCase", Some("Piso 1"), 2.0, true),
            el("IfcWall", Some("Piso 1"), 1.5, true),
            el("IfcColumn", Some("Piso 2"), 0.3, true),
            el("IfcSlab", Some("Piso 1"), 9.0, false),
            el("IfcBuildingElementProxy", None, 1.0, true),
        ]);
        assert_eq!(f[0].nivel, "Piso 1");
        let muros = f.iter().find(|r| r.tipo == "Muros").unwrap();
        assert_eq!(muros.cantidad, 2);
        assert!((muros.volumen - 3.5).abs() < 1e-9);
        let losas = f.iter().find(|r| r.tipo == "Losas").unwrap();
        assert_eq!((losas.volumen, losas.sin_volumen), (0.0, 1));
        assert_eq!(f.last().unwrap().nivel, "(sin nivel)");
        assert!(csv_modelo(&f).starts_with("Nivel,Partida"));
    }

    #[test]
    fn csv_con_comas_en_el_nombre() {
        assert_eq!(campo("a,b"), "\"a,b\"");
        assert_eq!(campo("di \"x\""), "\"di \"\"x\"\"\"");
        assert_eq!(campo("muro"), "muro");
    }
}
