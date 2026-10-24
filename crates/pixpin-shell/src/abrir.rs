//! Abrir un fichero con su aplicacion, o enseñarlo en el Explorador.
//!
//! Las dos cosas que espera quien tiene una ficha de archivo pineada:
//! doble clic la abre, y «Abrir ubicacion» lleva a su carpeta con el
//! fichero YA seleccionado, que es distinto (y mas util) que abrir la
//! carpeta a secas.

use std::path::Path;

use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, PCWSTR, w};

/// El valor por debajo del cual `ShellExecuteW` devuelve un codigo de error
/// en vez de un handle de instancia. Es la comprobacion que documenta la
/// propia API, y no comprobarla convierte un fallo en un silencio.
const EXITO_MINIMO: isize = 32;

/// Abre el fichero o la carpeta con la aplicacion predeterminada.
pub fn abrir(ruta: &Path) -> windows::core::Result<()> {
    let objetivo = HSTRING::from(ruta.as_os_str());
    // SAFETY: cadenas vivas durante toda la llamada; ShellExecuteW no
    // retiene ninguno de los punteros.
    let r = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(objetivo.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };
    if r.0 as isize <= EXITO_MINIMO {
        return Err(windows::core::Error::from_thread());
    }
    Ok(())
}

/// Abre el Explorador con el fichero seleccionado.
pub fn abrir_ubicacion(ruta: &Path) -> windows::core::Result<()> {
    // Las comillas importan: sin ellas, una ruta con espacios llega al
    // Explorador partida en trozos y abre la carpeta equivocada.
    let parametros = HSTRING::from(format!("/select,\"{}\"", ruta.display()));
    // SAFETY: igual que arriba; explorer.exe es el ejecutable del sistema.
    let r = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            w!("explorer.exe"),
            PCWSTR(parametros.as_ptr()),
            None,
            SW_SHOWNORMAL,
        )
    };
    if r.0 as isize <= EXITO_MINIMO {
        return Err(windows::core::Error::from_thread());
    }
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn abrir_una_ruta_inexistente_da_error_en_vez_de_fingir_exito() {
        // Caso negativo: sin comprobar el valor de retorno, ShellExecuteW
        // "funciona" siempre y el usuario ve que no pasa nada sin saber por
        // que. No abre ninguna ventana porque la ruta no existe.
        assert!(abrir(Path::new(r"Z:\no\existe\nada.qqq")).is_err());
    }
}
