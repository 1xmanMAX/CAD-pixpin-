//! **Escribir en la consola desde una aplicacion sin consola.**
//!
//! `pixpinmax.exe` es del subsistema «windows» (`#![windows_subsystem]`): si
//! fuera de consola, cada doble clic, cada «Abrir con» y cada enlace
//! `pixpin://` abriria una ventana negra. Pero `pixpinmax ocr foto.png` y
//! `pixpinmax ayuda` tienen que escribir algo, y un script tiene que poder
//! leerlo. Hay dos casos:
//!
//! - **La salida esta redirigida** (una tuberia de PowerShell, `> fichero`,
//!   `Start-Process -RedirectStandardOutput`): Windows ya nos dio esa manija
//!   y basta escribir en ella, en UTF-8.
//! - **Se escribio a mano en una consola**: no hay manija; se engancha uno a
//!   la consola del padre (`AttachConsole`) y se escribe en `CONOUT$` con
//!   `WriteConsoleW`, en UTF-16, para que las tildes salgan bien sea cual sea
//!   la pagina de codigos. El simbolo del sistema no espera a una app de
//!   ventanas, asi que el texto sale despues de su `>`: por eso empieza con
//!   un salto de linea.
//!
//! Si no hay ni lo uno ni lo otro (un doble clic), no se escribe nada.

use std::sync::OnceLock;

use windows::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_TYPE_CHAR,
    FILE_TYPE_UNKNOWN, GetFileType, OPEN_EXISTING, WriteFile,
};
use windows::Win32::System::Console::{
    ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_ERROR_HANDLE, STD_HANDLE,
    STD_OUTPUT_HANDLE, WriteConsoleW,
};
use windows::core::w;

/// Escribe en la salida estandar (o en la consola del padre).
pub fn escribir(texto: &str) {
    escribir_en(STD_OUTPUT_HANDLE, texto);
}

/// Escribe en la salida de errores (o en la consola del padre).
pub fn escribir_error(texto: &str) {
    escribir_en(STD_ERROR_HANDLE, texto);
}

/// Si la salida estandar va a una tuberia o a un fichero.
pub fn salida_redirigida() -> bool {
    manija_redirigida(STD_OUTPUT_HANDLE).is_some()
}

fn manija_redirigida(cual: STD_HANDLE) -> Option<HANDLE> {
    // SAFETY: consultas sin punteros sobre manijas de este proceso.
    unsafe {
        let h = GetStdHandle(cual).ok()?;
        if h.is_invalid() || h.0.is_null() {
            return None;
        }
        let tipo = GetFileType(h);
        (tipo != FILE_TYPE_CHAR && tipo != FILE_TYPE_UNKNOWN).then_some(h)
    }
}

fn escribir_en(cual: STD_HANDLE, texto: &str) {
    if let Some(h) = manija_redirigida(cual) {
        let mut escritos = 0u32;
        // SAFETY: el texto vive durante la llamada; la manija es la nuestra.
        let _ = unsafe { WriteFile(h, Some(texto.as_bytes()), Some(&mut escritos), None) };
        return;
    }
    let Some(h) = consola_del_padre() else { return };
    let unidades: Vec<u16> = texto.encode_utf16().collect();
    let mut escritos = 0u32;
    // SAFETY: el texto vive durante la llamada; `CONOUT$` lo abrimos nosotros.
    let _ = unsafe { WriteConsoleW(HANDLE(h as *mut _), &unidades, Some(&mut escritos), None) };
}

/// `CONOUT$` de la consola del padre, una vez por proceso (guardada como
/// numero: una `HANDLE` no es `Sync`).
fn consola_del_padre() -> Option<isize> {
    static CONSOLA: OnceLock<Option<isize>> = OnceLock::new();
    *CONSOLA.get_or_init(|| {
        // SAFETY: engancharse a la consola del padre no toca memoria nuestra;
        // falla sin efecto si el padre no tiene consola.
        unsafe { AttachConsole(ATTACH_PARENT_PROCESS) }.ok()?;
        // SAFETY: nombre estatico; la manija se queda abierta hasta salir.
        let h = unsafe {
            CreateFileW(
                w!("CONOUT$"),
                (GENERIC_READ | GENERIC_WRITE).0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            )
        }
        .ok()?;
        let unidades: Vec<u16> = "\r\n".encode_utf16().collect();
        let mut escritos = 0u32;
        // SAFETY: igual que en `escribir_en`.
        let _ = unsafe { WriteConsoleW(h, &unidades, Some(&mut escritos), None) };
        Some(h.0 as isize)
    })
}

/// Si lo que se escriba lo va a ver alguien: la salida va a una tuberia o
/// un fichero, o hay una consola del padre a la que engancharse. Sin eso
/// (un enlace, el menu del Explorador), los fallos se dicen en un cuadro.
pub fn hay_donde_escribir() -> bool {
    salida_redirigida() || consola_del_padre().is_some()
}
