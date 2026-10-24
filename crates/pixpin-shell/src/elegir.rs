//! El dialogo "Abrir" del sistema, con seleccion multiple.
//!
//! Es lo que en PixPin Android hace el SAF al adjuntar: elegir ficheros para
//! copiarlos dentro de la aplicacion. Aqui la copia la hace quien llama;
//! esto solo devuelve las rutas.
//!
//! Va aparte de `abrir`, que es lo contrario: aquel abre un fichero CON el
//! sistema, este se lo pide AL sistema.
//!
//! Sin test automatico, como el de guardar: un dialogo modal espera a un
//! humano. Se declara en vez de disimularlo.

use std::path::PathBuf;

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FOS_ALLOWMULTISELECT, FOS_FILEMUSTEXIST, FileOpenDialog, IFileOpenDialog, SIGDN_FILESYSPATH,
};
use windows::core::w;

/// Pide ficheros al usuario, de cualquier clase.
pub fn pedir_ficheros(hwnd_padre: HWND) -> Vec<PathBuf> {
    pedir(hwnd_padre, Clase::Todo)
}

/// Pide fotos o videos.
///
/// El filtro solo estrecha lo que el dialogo ensena de entrada; el usuario
/// siempre puede cambiarlo a «todos». Por eso quien llama sigue teniendo que
/// tragar con lo que venga: esto es comodidad, no una garantia.
pub fn pedir_imagenes(hwnd_padre: HWND) -> Vec<PathBuf> {
    pedir(hwnd_padre, Clase::FotosYVideos)
}

/// Pide UNA foto, sin videos: el logo de un proyecto es una imagen fija y
/// solo cabe una. Como arriba, el filtro es comodidad: quien llama tiene que
/// aguantar un fichero que no se deje leer.
pub fn pedir_una_imagen(hwnd_padre: HWND) -> Option<PathBuf> {
    pedir(hwnd_padre, Clase::UnaFoto).into_iter().next()
}

/// Que se le pide al usuario.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Clase {
    Todo,
    FotosYVideos,
    UnaFoto,
}

/// Lista vacia si cancela o si el dialogo falla: no poder abrirlo no puede
/// tumbar la ventana que lo pidio.
fn pedir(hwnd_padre: HWND, clase: Clase) -> Vec<PathBuf> {
    // SAFETY: COM ya esta inicializado en el hilo de interfaz (lo hace la
    // bandeja); el dialogo es un objeto local que muere al salir de aqui, y
    // cada cadena que devuelve se libera con CoTaskMemFree.
    unsafe {
        let Ok(dialogo) =
            CoCreateInstance::<_, IFileOpenDialog>(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
        else {
            return Vec::new();
        };
        // Varios a la vez, y solo ficheros que existan: adjuntar algo que no
        // esta no tiene sentido.
        let opciones = dialogo.GetOptions().unwrap_or_default();
        let opciones = if clase == Clase::UnaFoto {
            opciones
        } else {
            opciones | FOS_ALLOWMULTISELECT
        };
        let _ = dialogo.SetOptions(opciones | FOS_FILEMUSTEXIST);
        if clase != Clase::Todo {
            // Las cadenas tienen que vivir hasta despues de la llamada: el
            // dialogo guarda los punteros, no copia.
            let (fotos, patron) = if clase == Clase::UnaFoto {
                (w!("Imagenes"), w!("*.png;*.jpg;*.jpeg;*.webp;*.bmp;*.gif"))
            } else {
                (
                    w!("Fotos y videos"),
                    w!("*.png;*.jpg;*.jpeg;*.gif;*.bmp;*.webp;*.avif;*.mp4;*.mov;*.mkv;*.webm"),
                )
            };
            let (todos, estrella) = (w!("Todos los archivos"), w!("*.*"));
            let filtros = [
                COMDLG_FILTERSPEC {
                    pszName: fotos,
                    pszSpec: patron,
                },
                COMDLG_FILTERSPEC {
                    pszName: todos,
                    pszSpec: estrella,
                },
            ];
            let _ = dialogo.SetFileTypes(&filtros);
        }
        if dialogo.Show(Some(hwnd_padre)).is_err() {
            // Cancelar llega como error; no es un fallo que anotar.
            return Vec::new();
        }
        let Ok(elegidos) = dialogo.GetResults() else {
            return Vec::new();
        };
        let cuantos = elegidos.GetCount().unwrap_or(0);
        let mut rutas = Vec::with_capacity(cuantos as usize);
        for i in 0..cuantos {
            // Uno que falle no puede llevarse los demas que se eligieron.
            let Ok(elemento) = elegidos.GetItemAt(i) else {
                continue;
            };
            let Ok(ancha) = elemento.GetDisplayName(SIGDN_FILESYSPATH) else {
                continue;
            };
            if ancha.is_null() {
                continue;
            }
            if let Ok(texto) = ancha.to_string() {
                rutas.push(PathBuf::from(texto));
            }
            CoTaskMemFree(Some(ancha.0 as *const _));
        }
        rutas
    }
}
