//! **WBLOQUE** (WBLOCK de objetos): lo elegido a un plano nuevo, DWG o DXF,
//! con sus capas, estilos y bloques. Se parte de una copia del dibujo (asi
//! nada de lo que usan se pierde: tipos de linea, estilos de texto y de
//! cota, definiciones de bloque) y se quita del modelo todo lo demas, y del
//! papel todo salvo las ventanas.

use std::path::Path;

use opencadcodec::{CadDocument, EntityType, Handle, Vector3};

use crate::dibujo::Dibujo;
use crate::geo::P;

/// **El documento con solo `sel`** del espacio modelo; `base` es su punto
/// de insercion (`$INSBASE`) para cuando se inserte como bloque.
pub fn documento_con(doc: &CadDocument, sel: &[Handle], base: Option<P>) -> CadDocument {
    let mut nuevo = doc.clone();
    let modelo = doc.block_records.get("*Model_Space").map(|b| b.handle);
    let papeles: Vec<Handle> = doc
        .block_records
        .iter()
        .filter(|b| b.name.to_ascii_uppercase().starts_with("*PAPER_SPACE"))
        .map(|b| b.handle)
        .collect();
    let elegidos: std::collections::HashSet<Handle> = sel.iter().copied().collect();
    let quitar: Vec<Handle> = doc
        .entities()
        .filter(|e| {
            let c = e.common();
            (Some(c.owner_handle) == modelo && !elegidos.contains(&c.handle))
                || (papeles.contains(&c.owner_handle) && !matches!(e, EntityType::Viewport(_)))
        })
        .map(|e| e.common().handle)
        .collect();
    for h in quitar {
        nuevo.remove_entity_arc(h);
    }
    if let Some(b) = base {
        nuevo.header.model_space_insertion_base = Vector3::new(b[0], b[1], 0.0);
    }
    nuevo
}

/// **Escribe lo elegido** en `ruta` (DWG o DXF por la extension, en la
/// version del dibujo). Devuelve cuantas entidades salieron.
pub fn escribir(d: &Dibujo, sel: &[Handle], base: Option<P>, ruta: &Path) -> Result<usize, String> {
    let validos: Vec<Handle> = sel
        .iter()
        .copied()
        .filter(|h| d.entidad(*h).is_some())
        .collect();
    if validos.is_empty() {
        return Err("no hay nada elegido que escribir".into());
    }
    if d.ruta.as_deref().is_some_and(|r| r == ruta) {
        return Err("ese es el plano abierto: elija otro nombre".into());
    }
    let doc = documento_con(&d.doc, &validos, base);
    let n = doc.model_space_entities().count();
    let mut nuevo = Dibujo::de_documento(doc, None);
    nuevo.guardar_como(ruta)?;
    Ok(n)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::especialidad as esp;

    #[test]
    fn solo_lo_elegido_con_su_bloque_y_su_capa() {
        let dir = std::env::temp_dir().join(format!("cad-nucleo-wbloque-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut d = Dibujo::nuevo();
        crate::bloque::definir(
            &mut d.doc,
            "SILLA",
            vec![esp::circulo([0.0, 0.0], 5.0)],
            [0.0, 0.0],
        )
        .unwrap();
        let hs = d.editar("X", |t| {
            vec![
                t.anadir_en(esp::linea([0.0, 0.0], [10.0, 0.0]), "A-MUROS")
                    .unwrap(),
                t.anadir_en(
                    crate::bloque::insercion("SILLA", [20.0, 0.0], 1.0, 0.0),
                    "A-PUERTAS",
                )
                .unwrap(),
                t.anadir_en(esp::linea([0.0, 5.0], [10.0, 5.0]), "A-EJES")
                    .unwrap(),
            ]
        });
        for ext in ["dxf", "dwg"] {
            let ruta = dir.join(format!("parte.{ext}"));
            let n = escribir(&d, &hs[..2], Some([1.0, 2.0]), &ruta).unwrap();
            assert_eq!(n, 2);
            let d2 = Dibujo::abrir(&ruta).unwrap();
            let v: Vec<_> = d2.doc.model_space_entities().collect();
            assert_eq!(v.len(), 2, "{ext}");
            assert!(
                v.iter()
                    .any(|e| matches!(e, EntityType::Insert(i) if i.block_name == "SILLA"))
            );
            assert_eq!(d2.doc.entities_in_block("SILLA").count(), 1, "{ext}");
            assert!(d2.doc.layers.contains("A-PUERTAS"));
            assert_eq!(d2.doc.header.model_space_insertion_base.y, 2.0, "{ext}");
        }
        // El dibujo de partida no cambia.
        assert_eq!(d.doc.model_space_entities().count(), 3);
        assert!(escribir(&d, &[], None, &dir.join("nada.dxf")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
