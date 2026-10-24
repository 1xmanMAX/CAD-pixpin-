//! Abrir un DWG o un DXF con `opencadcodec`.
use std::path::Path;

pub use opencadcodec::CadDocument;

/// Si los primeros bytes son de un DWG (`AC10xx`).
pub fn es_dwg(cabeza: &[u8]) -> bool {
    cabeza.len() >= 6 && cabeza.starts_with(b"AC10")
}

/// Lee el plano: primero estricto y, si se salto algo, otra vez en modo
/// tolerante (lo que hace OpenCADStudio).
pub fn leer(ruta: &Path) -> Result<CadDocument, String> {
    let mut cabeza = [0u8; 6];
    {
        use std::io::Read;
        let mut f = std::fs::File::open(ruta).map_err(|e| e.to_string())?;
        let _ = f.read(&mut cabeza);
    }
    if es_dwg(&cabeza) {
        use opencadcodec::{DwgReadOptions, DwgReader};
        let estricto = DwgReader::from_file(ruta)
            .map_err(|e| e.to_string())?
            .read_with_stats();
        match estricto {
            Ok(r) if r.stats.skipped_source_records == 0 && r.stats.stream_completed => {
                Ok(r.document)
            }
            _ => DwgReader::from_file_with_options(ruta, DwgReadOptions::failsafe())
                .map_err(|e| e.to_string())?
                .read()
                .map_err(|e| e.to_string()),
        }
    } else {
        use opencadcodec::{DxfReader, DxfReaderConfiguration};
        let estricto = DxfReader::from_file(ruta)
            .map_err(|e| e.to_string())?
            .read_with_stats();
        match estricto {
            Ok(r) if r.stats.skipped_source_records == 0 && r.stats.stream_completed => {
                Ok(r.document)
            }
            _ => DxfReader::from_file(ruta)
                .map_err(|e| e.to_string())?
                .with_configuration(DxfReaderConfiguration {
                    failsafe: true,
                    default_encoding: None,
                })
                .read()
                .map_err(|e| e.to_string()),
        }
    }
}

/// **El plano como DXF** (para exportar un DWG a algo que abre cualquier
/// programa de CAD). Bloquea y puede caerse con un fichero roto: proceso
/// aparte, como al abrir.
pub fn a_dxf(entrada: &Path, salida: &Path) -> Result<(), String> {
    let doc = leer(entrada)?;
    opencadcodec::DxfWriter::new(&doc)
        .write_to_file(salida)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    #[ignore = "lee los planos de muestra de E:/pixpin-cad/muestras"]
    fn lee_las_muestras() {
        for e in std::fs::read_dir("E:/pixpin-cad/muestras")
            .unwrap()
            .flatten()
        {
            let t = std::time::Instant::now();
            match leer(&e.path()) {
                Ok(doc) => {
                    let mut tipos = std::collections::BTreeMap::<String, usize>::new();
                    let mut n = 0;
                    for ent in doc.model_space_entities() {
                        *tipos
                            .entry(ent.as_entity().entity_type().to_string())
                            .or_default() += 1;
                        n += 1;
                    }
                    println!(
                        "{:?}: {:?} {} entidades, {} bloques, {:?}\n  {:?}",
                        e.file_name(),
                        doc.version,
                        n,
                        doc.block_records.iter().count(),
                        t.elapsed(),
                        tipos
                    );
                }
                Err(err) => println!("{:?}: ERROR {err}", e.file_name()),
            }
        }
    }
}

#[cfg(test)]
mod volcar {
    /// Vuelca los textos de un plano que contengan algo (para investigar).
    #[test]
    #[ignore = "vuelca textos de una muestra"]
    fn volcar_textos() {
        use opencadcodec::entities::EntityType;
        let ruta = std::env::var("PIXPIN_CAD_PLANO").unwrap();
        let buscar = std::env::var("PIXPIN_CAD_BUSCAR").unwrap_or_default();
        let doc = super::leer(std::path::Path::new(&ruta)).unwrap();
        let mut n = 0;
        let coincide = |v: &str| match buscar.strip_prefix('=') {
            Some(e) => crate::texto::texto_plano(v).trim() == e,
            None => v.contains(&buscar),
        };
        let mut ver = |e: &EntityType, donde: &str| match e {
            EntityType::MText(t) if coincide(&t.value) && n < 12 => {
                n += 1;
                println!(
                    "MTEXT[{donde}] en={:?} {:?} h={} w={} ls={} est={:?} ap={:?} rot={} dir={:?}",
                    t.insertion_point,
                    t.value,
                    t.height,
                    t.rectangle_width,
                    t.line_spacing_factor,
                    t.style,
                    t.attachment_point,
                    t.rotation,
                    t.dwg_x_direction
                );
            }
            EntityType::Text(t) if coincide(&t.value) && n < 12 => {
                n += 1;
                println!(
                    "TEXT[{donde}] {:?} h={} wf={} est={:?} h_al={:?} v_al={:?}",
                    t.value,
                    t.height,
                    t.width_factor,
                    t.style,
                    t.horizontal_alignment,
                    t.vertical_alignment
                );
            }
            EntityType::Insert(ins) => {
                for at in &ins.attributes {
                    if coincide(&at.value) && n < 12 {
                        n += 1;
                        println!(
                            "ATTRIB[{donde}] bloque={:?} esc=({},{}) rot={} en={:?} {:?} h={} rot={} ali=({:?},{:?}) mt={}",
                            ins.block_name,
                            ins.x_scale(),
                            ins.y_scale(),
                            ins.rotation,
                            at.insertion_point,
                            at.value,
                            at.height,
                            at.rotation,
                            at.horizontal_alignment,
                            at.vertical_alignment,
                            at.embedded_mtext.is_some()
                        );
                    }
                }
            }
            _ => {}
        };
        for e in doc.model_space_entities() {
            ver(e, "modelo");
        }
        for br in doc.block_records.iter() {
            for e in doc.entities_in_block(&br.name) {
                ver(e, &br.name);
            }
        }
        for s in doc.text_styles.iter() {
            println!(
                "ESTILO {:?} fuente={:?} big={:?} ttf={:?} alto={} ancho={}",
                s.name, s.font_file, s.big_font_file, s.true_type_font, s.height, s.width_factor
            );
        }
    }
}

#[cfg(test)]
mod civil {
    /// Que objetos desconocidos (Civil 3D y otros) trae un plano y cuantos
    /// con su dibujo de reserva (proxy).
    #[test]
    #[ignore = "vuelca los objetos desconocidos de PIXPIN_CAD_PLANO"]
    fn volcar_desconocidos() {
        use opencadcodec::entities::EntityType;
        let ruta = std::env::var("PIXPIN_CAD_PLANO").unwrap();
        let doc = super::leer(std::path::Path::new(&ruta)).unwrap();
        let mut cuenta = std::collections::BTreeMap::<String, (usize, usize, usize)>::new();
        let mut ver = |e: &EntityType| {
            let c = e.common();
            let nombre = match e {
                EntityType::Unknown(u) => format!("?{}", u.dxf_name),
                otro => otro.as_entity().entity_type().to_string(),
            };
            let g = c.graphic_data.as_ref().map_or(0, |g| g.len());
            let x = cuenta.entry(nombre).or_default();
            x.0 += 1;
            if g > 0 {
                x.1 += 1;
                x.2 += g;
            }
        };
        for e in doc.model_space_entities() {
            ver(e);
        }
        for (k, v) in &cuenta {
            println!("{k}: {} (con dibujo {} · {} bytes)", v.0, v.1, v.2);
        }
    }
}

#[cfg(test)]
mod civil_registros {
    /// Los registros del dibujo de reserva del primer objeto de cada clase.
    #[test]
    #[ignore = "vuelca los registros proxy de PIXPIN_CAD_PLANO"]
    fn volcar_registros_proxy() {
        use opencadcodec::entities::EntityType;
        let ruta = std::env::var("PIXPIN_CAD_PLANO").unwrap();
        let doc = super::leer(std::path::Path::new(&ruta)).unwrap();
        let mut vistos = std::collections::BTreeSet::new();
        for e in doc.model_space_entities() {
            let EntityType::Unknown(u) = e else { continue };
            if !vistos.insert(u.dxf_name.clone()) {
                continue;
            }
            let Some(g) = &u.common.graphic_data else {
                continue;
            };
            let rd = |o: usize| u32::from_le_bytes(g[o..o + 4].try_into().unwrap());
            let (total, n) = (rd(0), rd(4));
            let mut o = 8;
            let mut v = Vec::new();
            for _ in 0..n {
                if o + 8 > g.len() {
                    break;
                }
                let (tam, tipo) = (rd(o) as usize, rd(o + 4));
                v.push(format!("{tipo}:{tam}"));
                o += tam.max(8);
            }
            println!(
                "{} total={total} n={n} [{}] {:?}",
                u.dxf_name,
                v.join(" "),
                u.common.proxy_graphics().map(|p| p
                    .records
                    .iter()
                    .filter_map(|r| match r {
                        opencadcodec::entities::ProxyGraphicRecord::UnicodeText(t) =>
                            Some(t.text.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>())
            );
        }
    }
}
