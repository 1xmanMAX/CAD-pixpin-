//! **Ver modelos BIM (IFC y Revit)** (9-oct-2026): el usuario pidio «un visor
//! sencillo como un pin, que se pueda ver facilmente cualquier modelo». Se
//! lee con `pixpin-bim` y se ve en `pixpin_cad::ventana3d`; aqui, como se
//! abre desde PixPin. Igual que los planos (`plano_cad`): el modelo se lee en
//! otro proceso (`pixpinmax.exe --bim-convertir <modelo> <cache>`), asi un
//! fichero roto solo tumba a ese proceso, y lo leido queda en
//! `<raiz>/cache/bim/`.

use std::path::{Path, PathBuf};

use pixpin_store::{Idioma, Ubicacion};

pub const ORDEN: &str = "--bim-convertir";
const EN_CACHE: usize = 16;

pub fn se_abre(nombre: &str) -> bool {
    pixpin_bim::se_abre(&pixpin_docs::extension(nombre))
}

/// Si PixPin se lanzo para leer un modelo, lo lee y dice con que codigo
/// salir. `None`: es un arranque normal.
pub fn convertir_si_toca() -> Option<i32> {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    if args.get(1).is_none_or(|a| a != ORDEN) {
        return None;
    }
    let (Some(entrada), Some(salida)) = (args.get(2), args.get(3)) else {
        return Some(2);
    };
    let (entrada, salida) = (PathBuf::from(entrada), PathBuf::from(salida));
    Some(match pixpin_bim::convertir_fichero(&entrada) {
        Ok(modelo) => {
            let temporal = salida.with_extension("tmp");
            let escrito = std::fs::write(&temporal, modelo.a_bytes())
                .and_then(|()| std::fs::rename(&temporal, &salida));
            if escrito.is_ok() { 0 } else { 3 }
        }
        Err(e) => {
            let _ = std::fs::write(salida.with_extension("error"), e);
            1
        }
    })
}

fn podar(carpeta: &Path) {
    let Ok(d) = std::fs::read_dir(carpeta) else {
        return;
    };
    let mut v: Vec<(std::time::SystemTime, PathBuf)> = d
        .flatten()
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    v.sort_by_key(|x| std::cmp::Reverse(x.0));
    for (_, p) in v.into_iter().skip(EN_CACHE) {
        let _ = std::fs::remove_file(p);
    }
}

fn cargar(raiz: &Path, ruta: &Path) -> Result<pixpin_cad::modelo3d::Modelo3d, String> {
    use pixpin_cad::modelo3d::Modelo3d;
    let carpeta = raiz.join("cache").join("bim");
    std::fs::create_dir_all(&carpeta).map_err(|e| e.to_string())?;
    let clave = crate::plano_cad::clave(ruta).ok_or("no se puede leer el fichero")?;
    let destino = carpeta.join(format!("{clave}.px3d"));
    if let Ok(b) = std::fs::read(&destino)
        && let Some(m) = Modelo3d::de_bytes(&b)
    {
        let _ = std::fs::File::options().append(true).open(&destino);
        return Ok(m);
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut orden = std::process::Command::new(exe);
    orden.arg(ORDEN).arg(ruta).arg(&destino);
    {
        use std::os::windows::process::CommandExt;
        orden.creation_flags(0x0800_0000);
    }
    let t = std::time::Instant::now();
    let estado = orden.status().map_err(|e| e.to_string())?;
    tracing::info!(ms = t.elapsed().as_millis() as u64, codigo = ?estado.code(), ruta = %ruta.display(), "modelo BIM leido aparte");
    if !estado.success() {
        let motivo = std::fs::read_to_string(destino.with_extension("error")).unwrap_or_default();
        let _ = std::fs::remove_file(destino.with_extension("error"));
        return Err(if motivo.is_empty() {
            "el modelo no se pudo leer (puede estar dañado)".into()
        } else {
            motivo
        });
    }
    podar(&carpeta);
    let b = std::fs::read(&destino).map_err(|e| e.to_string())?;
    Modelo3d::de_bytes(&b).ok_or_else(|| "lo leido no se entiende".to_string())
}

fn textos(idioma: Idioma) -> pixpin_cad::ventana3d::TextosUi3d {
    match idioma {
        Idioma::Ingles => pixpin_cad::ventana3d::TextosUi3d {
            abriendo: "Opening the model…".into(),
            vacio: "The model has nothing to show".into(),
            error: "Could not open the model".into(),
            todo: "Show whole model	F".into(),
            tema: "Light or dark background	B".into(),
            corte: "Section box	C".into(),
            aislar: "Isolate the selected element	I".into(),
            panel: "Levels and categories	L".into(),
            niveles: "LEVELS".into(),
            categorias: "CATEGORIES".into(),
            sin_nivel: "No level".into(),
            mostrar_todo: "Show all".into(),
            aristas: "Edges	E".into(),
            planta: "Plan view	P".into(),
            encima: "Always on top	T".into(),
            cerrar: "Close	Esc".into(),
            altura_corte: "Section".into(),
            pista: "Drag: orbit · Right drag: pan · Wheel: zoom · Click: what is it · Click again: isolate".into(),
        },
        _ => pixpin_cad::ventana3d::TextosUi3d::default(),
    }
}

/// Los textos de los marcadores y los marcos del modelo.
fn textos_marcos(idioma: Idioma) -> pixpin_cad::ventana3d::TextosMarcos3d {
    match idioma {
        Idioma::Ingles => pixpin_cad::ventana3d::TextosMarcos3d {
            marcador: "Bookmark here (with the section and what is hidden)\tCtrl+M".into(),
            marcos: "Frames to print\tI".into(),
            imprimir: "Print the frames…\tCtrl+P".into(),
            borrar_marcos: "Clear frames".into(),
            pista_marcos: "Drag to frame what is printed · Ctrl+P: print · Del: remove the last one · The numbers: go to its view · Esc: exit".into(),
            pista_anotar: "What you annotate is only seen in this bookmark's view".into(),
            anotar: "Annotate this view\tA".into(),
            botones: pixpin_cad::ventana::NombresDeBotones::ingles(),
        },
        _ => pixpin_cad::ventana3d::TextosMarcos3d::default(),
    }
}

/// **El riel del modelo** (19-oct-2026): los marcadores con todo el estado
/// de la vista, lo anotado sobre cada uno con el motor del lienzo (una capa
/// por marcador) y los marcos numerados que se imprimen en grupo: se
/// fotografia cada uno en su vista y van juntos al dialogo
/// (`marco_imprimir::imprimir_fotos`). De un adjunto del chat, todo en los
/// ficheros `anot-<uid>…` de su mensaje, que viajan
/// (`plano_cad::SitioDeMarcadores`).
fn barra_del_modelo(
    idioma: Idioma,
    raiz: &Path,
    ruta: &Path,
    nombre: &str,
) -> pixpin_cad::ventana3d::Barra3d {
    use pixpin_cad::ventana3d::Foto;
    let sitio = crate::plano_cad::SitioDeMarcadores::abrir(raiz, ruta);
    let (s1, s2) = (sitio.clone(), sitio.clone());
    let titulo = nombre.to_string();
    pixpin_cad::ventana3d::Barra3d {
        tipo: crate::plano_cad::tipo_de_archivo(ruta),
        marcadores: Some(sitio.vistas()),
        anotador: Some(Box::new(move |id: &str| {
            Box::new(crate::plano_cad::anotar::motor_de_marcador(&s1, id))
                as Box<dyn pixpin_cad::anotado::Anotador>
        })),
        // Sin borrar los ficheros: la capa con todo borrado (ver `vaciar_capa`).
        quitar_tinta: Some(Box::new(move |id: &str| {
            let r = crate::plano_cad::anotar::rutas_de_marcador(&s2, id);
            if let Err(e) = crate::plano_cad::anotar::vaciar_capa(&r) {
                tracing::warn!(?e, tinta = %r.tinta.display(), "no se pudo vaciar lo anotado del marcador");
            }
        })),
        foto: Some(Box::new(|r: [i32; 4]| {
            let area = pixpin_geom::Rect {
                x: r[0],
                y: r[1],
                ancho: r[2].max(0) as u32,
                alto: r[3].max(0) as u32,
            };
            crate::marco_imprimir::foto_de_pantalla(area).map(|i| (i.ancho, i.alto, i.pixeles))
        })),
        imprimir: Some(Box::new(move |v: isize, fotos: Vec<Foto>| {
            let fotos = fotos
                .into_iter()
                .map(|(ancho, alto, pixeles)| pixpin_codec::imagen::ImagenRgba {
                    ancho,
                    alto,
                    pixeles,
                })
                .collect();
            if let Err(e) = crate::marco_imprimir::imprimir_fotos(v, fotos, &titulo) {
                tracing::warn!(?e, "no se pudieron imprimir los marcos del modelo");
            }
        })),
        textos: textos_marcos(idioma),
        globo: Some(crate::plano_cad::globo_de_la_barra()),
    }
}

/// Abre el modelo en su ventana, en su propio hilo.
pub fn lanzar(idioma: Idioma, ubicacion: Ubicacion, ruta: &Path) {
    let ruta = ruta.to_path_buf();
    let raiz = ubicacion.raiz().to_path_buf();
    let lanzado = std::thread::Builder::new()
        .name("modelo-bim".into())
        .spawn(move || {
            let (tx, rx) = std::sync::mpsc::channel();
            let (r2, raiz2) = (ruta.clone(), raiz.clone());
            let _ = std::thread::Builder::new()
                .name("modelo-bim-leer".into())
                .spawn(move || {
                    let _ = tx.send(cargar(&raiz2, &r2));
                });
            let titulo = ruta
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let extras = pixpin_cad::ventana3d::Extras {
                exportar: Some(crate::compartir::exportable::al_exportar_3d(
                    idioma,
                    titulo.clone(),
                    Some(ruta.clone()),
                    None,
                )),
                ..Default::default()
            };
            // En la barra, el tipo y el nombre, como en los demas pines.
            let nombre = crate::plano_cad::nombre_para_la_barra(&ruta);
            let barra = barra_del_modelo(idioma, &raiz, &ruta, &nombre);
            if let Err(e) =
                pixpin_cad::ventana3d::ver_con_barra(&nombre, barra, rx, textos(idioma), extras)
            {
                tracing::warn!(%e, "no se pudo abrir el visor de modelos");
                let _ = pixpin_shell::abrir(&ruta);
            }
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el visor de modelos");
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn se_abren_los_ifc_y_los_revit() {
        assert!(se_abre("Edificio.IFC") && se_abre("obra.rvt"));
        // Caso negativo: lo demas no (un DWG va al visor de planos).
        assert!(!se_abre("plano.dwg") && !se_abre("ifc"));
    }
}
