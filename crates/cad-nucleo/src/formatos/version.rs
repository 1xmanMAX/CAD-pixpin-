//! **La version de DWG/DXF en que se guarda** (como la lista de GUARDARCOMO
//! de AutoCAD): 2000, 2004, 2007, 2010, 2013 o 2018. Es la del documento
//! (`CadDocument::version`), que es la que usan los dos escritores de
//! opencadcodec: [`crate::Dibujo::guardar_como`] la respeta sin mas.

use std::path::Path;

use opencadcodec::DxfVersion;

use crate::dibujo::Dibujo;

/// Las versiones que se ofrecen, de la mas vieja a la mas nueva.
pub const VERSIONES: &[(&str, DxfVersion)] = &[
    ("2000", DxfVersion::AC1015),
    ("2004", DxfVersion::AC1018),
    ("2007", DxfVersion::AC1021),
    ("2010", DxfVersion::AC1024),
    ("2013", DxfVersion::AC1027),
    ("2018", DxfVersion::AC1032),
];

/// **La version escrita** (`2010`, `R2010`, `AC1024`, `10`).
pub fn de_texto(t: &str) -> Option<DxfVersion> {
    let t = t.trim().to_ascii_uppercase();
    let t = t.trim_start_matches("AUTOCAD").trim();
    let t = t.strip_prefix('R').unwrap_or(t);
    VERSIONES.iter().find_map(|(n, v)| {
        let corto = &n[2..];
        (t == *n || t == corto || t == v.as_str()).then_some(*v)
    })
}

/// El nombre de una version («2018»), o su codigo si no es de la lista.
pub fn nombre(v: DxfVersion) -> &'static str {
    VERSIONES
        .iter()
        .find(|(_, x)| *x == v)
        .map_or(v.as_str(), |(n, _)| n)
}

/// **Pone la version** en que se guardara el dibujo. Devuelve un aviso si se
/// perdera algo que venia guardado tal cual del DWG de otra version.
pub fn poner(d: &mut Dibujo, v: DxfVersion) -> Option<String> {
    let aviso = d.doc.has_version_locked_data(v).then(|| {
        format!(
            "Al guardar en {} se pierden los objetos que el DWG traia sin leer (de su version {})",
            nombre(v),
            d.doc.dwg_source_version.map_or("?", nombre)
        )
    });
    if d.doc.version != v {
        d.doc.version = v;
        d.tocar();
    }
    aviso
}

/// **Guarda en `ruta`** (DWG o DXF por la extension) en la version `v`.
pub fn guardar_en(d: &mut Dibujo, ruta: &Path, v: DxfVersion) -> Result<Option<String>, String> {
    let aviso = poner(d, v);
    d.guardar_como(ruta)?;
    Ok(aviso)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::especialidad as esp;
    use opencadcodec::EntityType;

    #[test]
    fn versiones_escritas() {
        assert_eq!(de_texto("2010"), Some(DxfVersion::AC1024));
        assert_eq!(de_texto("r2000"), Some(DxfVersion::AC1015));
        assert_eq!(de_texto("AC1027"), Some(DxfVersion::AC1027));
        assert_eq!(de_texto("18"), Some(DxfVersion::AC1032));
        assert_eq!(de_texto("AutoCAD 2004"), Some(DxfVersion::AC1018));
        assert_eq!(de_texto("R12"), None);
        assert_eq!(nombre(DxfVersion::AC1021), "2007");
    }

    #[test]
    fn cada_version_se_guarda_y_se_vuelve_a_abrir() {
        let dir = std::env::temp_dir().join(format!("cad-nucleo-versiones-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (n, v) in VERSIONES {
            for ext in ["dwg", "dxf"] {
                let mut d = Dibujo::nuevo();
                d.editar("X", |t| {
                    t.anadir_en(esp::linea([0.0, 0.0], [3.0, 4.0]), "A-MUROS");
                    t.anadir_en(esp::circulo([5.0, 5.0], 2.0), "A-EJES");
                    t.anadir_en(esp::texto("Hola", [0.0, 0.0], 2.5, 0.0, false), "A-TEXTOS");
                });
                let ruta = dir.join(format!("v{n}.{ext}"));
                guardar_en(&mut d, &ruta, *v).unwrap_or_else(|e| panic!("{n} {ext}: {e}"));
                assert!(!d.sucio());
                let d2 = Dibujo::abrir(&ruta).unwrap_or_else(|e| panic!("{n} {ext}: {e}"));
                assert_eq!(d2.doc.version, *v, "{n} {ext}");
                let tipos: Vec<_> = d2
                    .doc
                    .model_space_entities()
                    .map(|e| match e {
                        EntityType::Line(_) => "L",
                        EntityType::Circle(_) => "C",
                        EntityType::Text(_) => "T",
                        _ => "?",
                    })
                    .collect();
                assert_eq!(tipos.len(), 3, "{n} {ext}: {tipos:?}");
                assert!(
                    ["L", "C", "T"].iter().all(|t| tipos.contains(t)),
                    "{n} {ext}"
                );
                assert!(d2.doc.layers.contains("A-MUROS"), "{n} {ext}");
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
