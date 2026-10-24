//! «Abrir con…»: el dialogo «Abrir con» de Windows, el de verdad.
//!
//! Es lo que en el movil hace el selector del sistema (`ui/AbrirCon.kt`):
//! elegir CON QUE se abre este fichero, no abrirlo con lo de siempre. Antes
//! el chat llamaba a `abrir::abrir`, que lo abre con la aplicacion asociada;
//! para un PDF que uno quiere firmar con otro programa eso no servia.
//!
//! `SHOpenWithDialog` es el mismo cuadro que sale en el Explorador con el
//! boton derecho > «Abrir con» > «Elegir otra aplicacion»: lista lo que hay
//! instalado y sabe abrir ese formato, deja buscar en la tienda y, marcado,
//! recuerda la eleccion. No pesa nada: es de shell32 y ya esta cargado.
//!
//! **Por que fallaba** (13-oct-2026, el usuario: «al darle a abrir en otra
//! app sale un mensaje»; el registro: `0x80070057`, parametro incorrecto, en
//! cada «Abrir con otra app» del chat; reproducido en la prueba
//! `el_shell_entiende_la_ruta_arreglada`). Dos cosas:
//! - Las rutas de los adjuntos del chat son la carpeta del proyecto (con
//!   `\`) mas la relativa del movil (con `/`): `...\android\proyectos/doc.pdf`.
//!   El shell no acepta la mezcla (lo mismo que ya rompio el panel
//!   Compartir, ver `compartir.rs`) y el cuadro contestaba «parametro
//!   incorrecto» sin salir. Se le pasa la ruta absoluta y con `\`
//!   ([`ruta_para_el_dialogo`]).
//! - Se pedia ademas `OAIF_REGISTER_EXT` (que el propio programa escriba la
//!   asociacion), cosa que desde Windows 8 solo hace el sistema. Ahora van
//!   las banderas justas ([`BANDERAS`]): la casilla «Usar siempre» activa y
//!   el cuadro abre el fichero con lo elegido.
//!
//! Y un fichero que ya no esta se dice con su nombre ([`ErrorAbrirCon::NoExiste`])
//! en vez de dejar que Windows conteste con un codigo.
//!
//! El cuadro en si no tiene prueba automatica, como `elegir` y `guardar`: es
//! modal y espera a una persona. Lo que se le pasa, si.

use std::path::{Path, PathBuf};

use windows::Win32::Foundation::{ERROR_CANCELLED, HWND};
use windows::Win32::UI::Shell::{
    OAIF_ALLOW_REGISTRATION, OAIF_EXEC, OPEN_AS_INFO_FLAGS, OPENASINFO, SHOpenWithDialog,
};
use windows::core::{HRESULT, HSTRING, PCWSTR};

/// Las banderas del cuadro: la casilla «Usar siempre esta aplicacion»
/// disponible (la asociacion la guarda Windows, no nosotros) y, al aceptar,
/// el propio cuadro abre el fichero con lo elegido.
pub const BANDERAS: OPEN_AS_INFO_FLAGS =
    OPEN_AS_INFO_FLAGS(OAIF_ALLOW_REGISTRATION.0 | OAIF_EXEC.0);

/// Por que no salio el cuadro.
#[derive(Debug)]
pub enum ErrorAbrirCon {
    /// El fichero no esta (borrado, movido, un adjunto que aun no bajo).
    NoExiste(PathBuf),
    /// Windows no quiso abrir el cuadro.
    Windows(windows::core::Error),
}

impl std::fmt::Display for ErrorAbrirCon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorAbrirCon::NoExiste(r) => write!(f, "no existe el fichero {}", r.display()),
            ErrorAbrirCon::Windows(e) => write!(f, "Windows no abrio el cuadro «Abrir con»: {e}"),
        }
    }
}

impl std::error::Error for ErrorAbrirCon {}

/// La ruta tal y como la quiere el cuadro: absoluta, con `\` en todas
/// partes y de un fichero que existe.
pub fn ruta_para_el_dialogo(ruta: &Path) -> Result<PathBuf, ErrorAbrirCon> {
    let con_barras = PathBuf::from(ruta.as_os_str().to_string_lossy().replace('/', "\\"));
    let absoluta = std::path::absolute(&con_barras).unwrap_or(con_barras);
    if absoluta.is_file() {
        Ok(absoluta)
    } else {
        Err(ErrorAbrirCon::NoExiste(absoluta))
    }
}

/// Pregunta con que abrir `ruta` y lo abre con lo elegido.
///
/// `Ok(true)` si se abrio, `Ok(false)` si el usuario cerro el cuadro sin
/// elegir (eso no es un fallo: es cambiar de idea), y el error en otro caso.
pub fn abrir_con_otra(padre: HWND, ruta: &Path) -> Result<bool, ErrorAbrirCon> {
    let ruta = ruta_para_el_dialogo(ruta)?;
    let fichero = HSTRING::from(ruta.as_os_str());
    let info = OPENASINFO {
        pcszFile: PCWSTR(fichero.as_ptr()),
        pcszClass: PCWSTR::null(),
        oaifInFlags: BANDERAS,
    };
    // El cuadro es COM: en un hilo sin COM se inicia aqui (y se suelta al
    // volver); en uno que ya lo tiene no cambia nada.
    let _com = crate::ComDelHilo::iniciar();
    let padre = (!padre.is_invalid()).then_some(padre);
    // SAFETY: `info` y la cadena a la que apunta viven durante toda la
    // llamada (es sincrona: vuelve cuando se cierra el cuadro) y el padre es
    // una ventana de este hilo o ninguna.
    match unsafe { SHOpenWithDialog(padre, &info) } {
        Ok(()) => Ok(true),
        Err(e) if e.code() == HRESULT::from_win32(ERROR_CANCELLED.0) => Ok(false),
        Err(e) => Err(ErrorAbrirCon::Windows(e)),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn carpeta(nombre: &str) -> PathBuf {
        let c =
            std::env::temp_dir().join(format!("pixpin-abrir-con-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&c);
        std::fs::create_dir_all(c.join("proyectos")).unwrap();
        c
    }

    #[test]
    fn la_ruta_mezclada_del_chat_llega_con_barras_de_windows() {
        let c = carpeta("mezcla");
        std::fs::write(c.join("proyectos").join("doc-1.pdf"), b"%PDF").unwrap();
        // Como la arma el chat: la carpeta del PC mas la relativa del movil.
        let mezclada = PathBuf::from(format!("{}\\proyectos/doc-1.pdf", c.display()));
        let r = ruta_para_el_dialogo(&mezclada).unwrap();
        let texto = r.to_string_lossy();
        assert!(!texto.contains('/'), "{texto}");
        assert!(r.is_absolute());
        assert_eq!(r, c.join("proyectos").join("doc-1.pdf"));
        let _ = std::fs::remove_dir_all(&c);
    }

    #[test]
    fn un_fichero_que_no_esta_da_un_aviso_claro_y_no_abre_nada() {
        // Caso negativo: nunca se llega a Windows (ni al cuadro) con un
        // fichero que no existe; el error dice cual es.
        let c = carpeta("falta");
        let falta = c.join("proyectos").join("no-esta.xlsx");
        let e = abrir_con_otra(HWND::default(), &falta).unwrap_err();
        assert!(
            matches!(&e, ErrorAbrirCon::NoExiste(r) if r == &falta),
            "{e:?}"
        );
        assert!(e.to_string().contains("no-esta.xlsx"));
        // Una carpeta tampoco es un fichero que abrir.
        assert!(matches!(
            ruta_para_el_dialogo(&c),
            Err(ErrorAbrirCon::NoExiste(_))
        ));
        let _ = std::fs::remove_dir_all(&c);
    }

    #[test]
    fn las_banderas_son_las_del_explorador_sin_registrar_por_su_cuenta() {
        assert_eq!(BANDERAS.0, OAIF_ALLOW_REGISTRATION.0 | OAIF_EXEC.0);
        // Caso negativo: nada de escribir la asociacion desde aqui.
        assert_eq!(
            BANDERAS.0 & windows::Win32::UI::Shell::OAIF_REGISTER_EXT.0,
            0
        );
        assert_eq!(
            BANDERAS.0 & windows::Win32::UI::Shell::OAIF_FORCE_REGISTRATION.0,
            0
        );
    }

    #[test]
    fn el_shell_entiende_la_ruta_arreglada() {
        // Sin abrir ningun cuadro: el analizador de rutas del shell con la
        // ruta que se le pasa ahora, y lo que dice de la que llegaba del
        // chat (a la salida de la prueba, para el diagnostico).
        use windows::Win32::UI::Shell::{IShellItem, SHCreateItemFromParsingName};
        let c = carpeta("shell");
        std::fs::write(c.join("proyectos").join("doc-2.pdf"), b"%PDF").unwrap();
        let mezclada = format!("{}\\proyectos/doc-2.pdf", c.display());
        let _com = crate::ComDelHilo::iniciar();
        let analizar = |s: &str| -> windows::core::Result<IShellItem> {
            // SAFETY: cadena viva durante la llamada; sin contexto de enlace.
            unsafe { SHCreateItemFromParsingName(&HSTRING::from(s), None) }
        };
        let arreglada = ruta_para_el_dialogo(Path::new(&mezclada)).unwrap();
        assert!(
            analizar(&arreglada.to_string_lossy()).is_ok(),
            "la ruta arreglada se entiende"
        );
        // Caso negativo, la causa del fallo reproducida: la ruta como
        // llegaba da el mismo 0x80070057 que salia en el registro.
        let antes = analizar(&mezclada).err().map(|e| e.code());
        assert_eq!(
            antes,
            Some(windows::Win32::Foundation::E_INVALIDARG),
            "la mezcla de barras"
        );
        let _ = std::fs::remove_dir_all(&c);
    }
}
