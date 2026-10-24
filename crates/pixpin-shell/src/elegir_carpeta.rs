//! El dialogo del sistema para elegir una carpeta.
//!
//! Lo pide «Guardar en…» de un proyecto: el usuario elige donde vive todo lo
//! suyo. Es el mismo `IFileOpenDialog` de `elegir`, con `FOS_PICKFOLDERS`,
//! y no el viejo `SHBrowseForFolder`: aquel es un arbol sin barra de
//! direcciones ni accesos rapidos, y aqui el usuario ya conoce este.
//!
//! Sin test automatico, como los demas dialogos: uno modal espera a un
//! humano. Se declara en vez de disimularlo.

use std::path::PathBuf;

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree};
use windows::Win32::UI::Shell::{
    FOS_FORCEFILESYSTEM, FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, FileOpenDialog, IFileOpenDialog,
    SIGDN_FILESYSPATH,
};
use windows::core::HSTRING;

/// Pide una carpeta. `None` si cancela o si el dialogo falla: no poder
/// abrirlo no puede tumbar la ventana que lo pidio.
pub fn pedir_carpeta(hwnd_padre: HWND, titulo: &str) -> Option<PathBuf> {
    // SAFETY: COM ya esta inicializado en el hilo de interfaz (lo hace la
    // bandeja); el dialogo es un objeto local que muere al salir de aqui, y
    // la cadena que devuelve se libera con CoTaskMemFree antes de cualquier
    // salida temprana.
    unsafe {
        let dialogo: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        // Solo carpetas del sistema de ficheros: una «Biblioteca» o «Este
        // equipo» no tienen ruta donde poner nada.
        let opciones = dialogo.GetOptions().unwrap_or_default();
        dialogo
            .SetOptions(opciones | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST)
            .ok()?;
        let titulo = HSTRING::from(titulo);
        let _ = dialogo.SetTitle(&titulo);
        // Show devuelve Err al cancelar: es el camino normal, no un fallo.
        dialogo.Show(Some(hwnd_padre)).ok()?;
        let item = dialogo.GetResult().ok()?;
        let ruta = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let texto = ruta.to_string().ok();
        CoTaskMemFree(Some(ruta.as_ptr() as *const _));
        texto.map(PathBuf::from)
    }
}
