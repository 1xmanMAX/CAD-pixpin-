//! **Los modelos BIM**: se abren en la ventana 3D de PixPin (copiada tal
//! cual en `pixpin-cad`: orbita, caja de seccion, niveles, medidas de cada
//! pieza) y de su lectura sale el metrado por nivel y partida.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Sender, channel};

use cad_nucleo::metrados::{self, ElementoBim};
use pixpin_cad::modelo3d::Modelo3d;

/// Lo que se le cuenta a la ventana principal desde los hilos del modelo.
#[derive(Debug)]
pub enum Aviso {
    Texto(String),
}

pub fn se_abre(ruta: &Path) -> bool {
    let ext = ruta
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    pixpin_bim::se_abre(&ext) || pixpin_bim::es_de_civil(ruta)
}

/// **Abre el modelo en su ventana 3D** (en su propio hilo: la ventana tiene
/// su bucle) y vuelve enseguida.
pub fn abrir(ruta: PathBuf, avisos: Sender<Aviso>) {
    let titulo = ruta
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let _ = std::thread::Builder::new()
        .name("bim-ventana".into())
        .spawn(move || {
            // La ventana 3D pinta con Direct2D y WIC: COM en este hilo.
            // SAFETY: inicializar COM en un hilo propio que no lo tenia.
            unsafe {
                let _ = windows::Win32::System::Com::CoInitializeEx(
                    None,
                    windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
                );
            }
            let (tx, rx) = channel();
            let r2 = ruta.clone();
            let _ = std::thread::Builder::new()
                .name("bim-leer".into())
                .spawn(move || {
                    let _ = tx.send(leer(&r2));
                });
            if let Err(e) = pixpin_cad::ventana3d::ver(&titulo, rx, Default::default()) {
                let _ = avisos.send(Aviso::Texto(format!("No se pudo abrir {titulo}: {e}")));
            }
        });
}

/// Lee un modelo sin que un fichero roto tumbe la app.
pub fn leer(ruta: &Path) -> Result<Modelo3d, String> {
    let r = ruta.to_path_buf();
    std::panic::catch_unwind(move || pixpin_bim::convertir_fichero(&r))
        .unwrap_or_else(|_| Err("el modelo esta dañado".into()))
}

/// Los elementos del modelo, con su nivel por nombre, para el metrado.
pub fn elementos(m: &Modelo3d) -> Vec<ElementoBim> {
    // Las medidas van en unidades del modelo: a metros.
    let k = if m.metros > 0.0 { m.metros as f64 } else { 1.0 };
    m.elementos
        .iter()
        .map(|e| ElementoBim {
            tipo: e.tipo.clone(),
            nombre: e.nombre.clone(),
            nivel: e
                .nivel
                .and_then(|i| m.niveles.get(i as usize))
                .map(|n| n.nombre.clone()),
            volumen: e.medidas.volumen as f64 * k * k * k,
            planta: e.medidas.planta as f64 * k * k,
            alzado: e.medidas.alzado as f64 * k * k,
            superficie: e.medidas.superficie as f64 * k * k,
            cerrado: e.medidas.cerrado,
        })
        .collect()
}

/// **El metrado de un modelo en CSV** y cuantas partidas tiene.
pub fn metrado_csv(ruta: &Path) -> Result<(String, usize), String> {
    let m = leer(ruta)?;
    let filas = metrados::del_modelo(&elementos(&m));
    Ok((metrados::csv_modelo(&filas), filas.len()))
}
