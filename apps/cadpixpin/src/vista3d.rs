//! **VER3D: el dibujo de ahora en la ventana 3D** de PixPin (la de los
//! modelos BIM). El modelo sale de [`cad_nucleo::tresd::Escena`]: las
//! mallas, los muros y las losas con su tipo, color y nivel, y las rayas 2D
//! a su cota. Se hace en otro hilo con una copia del documento, para que
//! la mesa siga respondiendo.

use std::collections::HashMap;
use std::sync::mpsc::{Sender, channel};

use cad_nucleo::opencadcodec::CadDocument;
use cad_nucleo::tresd::Escena;
use pixpin_cad::modelo3d::{Constructor3d, Modelo3d, Nivel};

use crate::bim::Aviso;

/// **El modelo 3D de una escena** para la ventana 3D.
pub fn modelo(e: &Escena) -> Result<Modelo3d, String> {
    if e.elementos.is_empty() && e.lineas.is_empty() {
        return Err("El dibujo no tiene nada que ver en 3D".into());
    }
    let upm = e.unidades_por_metro();
    let mut c = Constructor3d::nuevo();
    let mut de_nivel = Vec::new();
    for el in &e.elementos {
        let id = c.elemento(el.tipo.clase_ifc(), &format!("{} ({})", el.nombre, el.capa));
        let m = el.malla_completa();
        let idx: Vec<u32> = m.triangulos().into_iter().flatten().collect();
        c.malla(id, &m.puntos, None, &idx, el.color);
        if let Some(n) = el.nivel {
            de_nivel.push((id, n as u32));
        }
    }
    // Las rayas, un elemento por capa.
    let mut capas: HashMap<&str, u32> = HashMap::new();
    for l in &e.lineas {
        let id = *capas
            .entry(&l.capa)
            .or_insert_with(|| c.elemento("Linea", &l.capa));
        c.linea(id, &l.puntos, l.color);
    }
    if !e.niveles.is_empty() {
        let niveles = e
            .niveles
            .iter()
            .map(|n| Nivel {
                nombre: n.nombre.clone(),
                cota: n.cota / upm,
                ..Default::default()
            })
            .collect();
        c.niveles(niveles, &de_nivel);
    }
    let mut m = c.terminar();
    m.metros = (1.0 / upm) as f32;
    Ok(m)
}

/// **Abre la ventana 3D con el documento** (en su propio hilo, con COM) y
/// vuelve enseguida; el modelo se hace en otro hilo mientras la ventana
/// dice que esta abriendo.
pub fn abrir(doc: CadDocument, titulo: String, avisos: Sender<Aviso>) {
    let _ = std::thread::Builder::new()
        .name("ver3d-ventana".into())
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
            let _ = std::thread::Builder::new()
                .name("ver3d-modelo".into())
                .stack_size(64 * 1024 * 1024)
                .spawn(move || {
                    let r =
                        std::panic::catch_unwind(move || modelo(&Escena::de_documento(&doc, true)))
                            .unwrap_or_else(|_| Err("no se pudo pasar el dibujo a 3D".into()));
                    let _ = tx.send(r);
                });
            if let Err(e) = pixpin_cad::ventana3d::ver(&titulo, rx, Default::default()) {
                let _ = avisos.send(Aviso::Texto(format!("No se pudo ver en 3D: {e}")));
            }
        });
}
