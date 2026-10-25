//! **CAD-pixpin** — la rama de PixPin Max para planos y modelos: ver,
//! editar y anotar DWG, DXF, IFC y Revit en un equipo modesto.
//!
//! - `cadpixpin` abre la pantalla de inicio.
//! - `cadpixpin plano.dwg` (o arrastrar el fichero) lo abre en la mesa de trabajo.
#![cfg_attr(not(test), windows_subsystem = "windows")]

// Fuera de Windows solo lo usan sus pruebas.
#[cfg_attr(not(windows), allow(dead_code))]
mod autoguardado;
#[cfg(windows)]
mod bim;
// Lo portable del plano en cubos y de lo que se pinta encima (se prueba en
// cualquier sistema).
#[cfg_attr(not(windows), allow(dead_code))]
mod cubos;
#[cfg(windows)]
mod dialogos;
// La lista de planos abiertos es portable (se prueba en cualquier sistema).
#[cfg_attr(not(windows), allow(dead_code))]
mod documentos;
#[cfg_attr(not(windows), allow(dead_code))]
mod encima;
#[cfg(windows)]
mod explorador_bim;
#[cfg(windows)]
mod imagenes;
#[cfg(windows)]
mod inicio;
// La linea de ordenes (historial y Tab), portable.
#[cfg_attr(not(windows), allow(dead_code))]
mod linea;
#[cfg(windows)]
mod mesa;
#[cfg(windows)]
mod plano;
// Fuera de Windows solo lo usan sus pruebas.
#[cfg_attr(not(windows), allow(dead_code))]
mod recientes;
#[cfg(windows)]
mod tema;
#[cfg(windows)]
mod ui;
#[cfg(windows)]
mod ventana;
#[cfg(windows)]
mod vista3d;

#[cfg(windows)]
fn main() {
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .try_init();
    // SAFETY: al arrancar, antes de crear ventanas: DPI por monitor y COM
    // (los dialogos de Abrir y Guardar) en este hilo.
    unsafe {
        let _ = windows::Win32::UI::HiDpi::SetProcessDpiAwarenessContext(
            windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        );
        let _ = windows::Win32::System::Com::CoInitializeEx(
            None,
            windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
        );
    }
    let abrir = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    if let Err(e) = ventana::correr(abrir) {
        pixpin_shell::dialogo::mostrar_error_fatal("CAD-pixpin", &e);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!(
        "CAD-pixpin es una app de Windows: el nucleo de edicion (cad-nucleo) si funciona aqui."
    );
}
