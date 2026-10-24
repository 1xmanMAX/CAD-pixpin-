//! Una sola PixPin Max a la vez.
//!
//! Sin esto, dos copias pelearian por los mismos atajos globales: la segunda
//! fallaria al registrarlos y el usuario veria una aplicacion que "a veces no
//! responde al atajo", que es de los fallos mas dificiles de diagnosticar.

use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;
use windows::core::w;

/// Errores posibles al pedir la instancia unica.
///
/// Se distinguen a proposito dos casos que no tienen nada que ver entre si:
/// que ya haya otra copia corriendo es el resultado esperado la mayoria de
/// las veces (el usuario probo a abrir PixPin Max dos veces); que
/// `CreateMutexW` falle por otra razon (permisos, recursos agotados) es un
/// fallo real que hay que reportar. Confundir el segundo caso con el primero
/// dejaria a quien depure buscando una segunda copia que no existe.
#[derive(Debug, thiserror::Error)]
pub enum ErrorInstanciaUnica {
    /// Ya hay otra copia de PixPin Max en marcha: no es un fallo.
    #[error("ya hay otra instancia de PixPin Max en marcha")]
    YaHayOtraInstancia,

    /// `CreateMutexW` fallo por una razon distinta a que el mutex ya
    /// existiera.
    #[error("no se pudo crear el mutex de instancia unica: {0}")]
    Windows(#[from] windows::core::Error),
}

/// Mientras este valor viva, ninguna otra copia puede arrancar.
pub struct InstanciaUnica {
    handle: HANDLE,
}

impl Drop for InstanciaUnica {
    fn drop(&mut self) {
        // SAFETY: `handle` viene de CreateMutexW, no se ha cerrado antes, y
        // este tipo no es Clone ni Copy, asi que se cierra exactamente una vez.
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

pub fn adquirir_instancia_unica() -> Result<InstanciaUnica, ErrorInstanciaUnica> {
    // El prefijo Local\ limita el ambito a la sesion del usuario: dos usuarios
    // distintos en el mismo equipo si pueden tener cada uno su PixPin Max.
    adquirir(w!(r"Local\PixPinMax-instancia-unica"))
}

/// **Solo para probar** (`PIXPIN_INSTANCIA`, ver `main.rs`): otra instancia
/// unica con su propio nombre, que no choca con la PixPin del usuario.
pub fn adquirir_instancia_de_prueba(nombre: &str) -> Result<InstanciaUnica, ErrorInstanciaUnica> {
    let limpio: String = nombre
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .take(40)
        .collect();
    let nombre =
        windows::core::HSTRING::from(format!(r"Local\PixPinMax-instancia-prueba-{limpio}"));
    adquirir(windows::core::PCWSTR(nombre.as_ptr()))
}

fn adquirir(nombre: windows::core::PCWSTR) -> Result<InstanciaUnica, ErrorInstanciaUnica> {
    // SAFETY: `nombre` es una cadena UTF-16 terminada en cero que vive
    // durante la llamada (un literal o el HSTRING de quien llama).
    // CreateMutexW devuelve un handle valido o un error; GetLastError se
    // consulta inmediatamente despues, antes de cualquier otra llamada.
    let (handle, ya_existia) = unsafe {
        let handle = CreateMutexW(None, true, nombre)?;
        let ya_existia = GetLastError() == ERROR_ALREADY_EXISTS;
        (handle, ya_existia)
    };

    if ya_existia {
        // SAFETY: handle recien creado y valido; se cierra una sola vez.
        unsafe {
            let _ = CloseHandle(handle);
        }
        return Err(ErrorInstanciaUnica::YaHayOtraInstancia);
    }

    Ok(InstanciaUnica { handle })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    // Este test usa un mutex con nombre global (Local\...): si otro test se
    // ejecuta a la vez, ambos compiten por el mismo nombre y el resultado se
    // vuelve intermitente. Por eso el crate se prueba con
    // `--test-threads=1`; no quitar ese flag sin sustituirlo por algo
    // equivalente.
    #[test]
    fn la_segunda_adquisicion_falla_mientras_viva_la_primera() {
        let primera = adquirir_instancia_unica().expect("la primera debe funcionar");

        assert!(
            adquirir_instancia_unica().is_err(),
            "la segunda instancia debe ser rechazada"
        );

        drop(primera);

        // Tras soltar la primera, el nombre queda libre otra vez.
        assert!(
            adquirir_instancia_unica().is_ok(),
            "al liberarse la primera, el nombre debe quedar disponible"
        );
    }

    #[test]
    fn un_fallo_real_de_windows_no_se_confunde_con_ya_hay_otra_instancia() {
        // No forzamos un fallo real de CreateMutexW: conseguirlo de verdad
        // (agotar el limite de handles del proceso, revocar permisos sobre
        // el objeto con nombre...) exige manipular el entorno del proceso de
        // pruebas de forma artificial y muy fragil. En su lugar probamos la
        // conversion `From<windows::core::Error> for ErrorInstanciaUnica`,
        // que es exactamente la que usa `adquirir_instancia_unica` en su
        // camino de error via `?` cuando `CreateMutexW` falla de verdad.
        let error_de_windows =
            windows::core::Error::from_hresult(windows::Win32::Foundation::E_ACCESSDENIED);
        let error: ErrorInstanciaUnica = error_de_windows.into();

        assert!(
            matches!(error, ErrorInstanciaUnica::Windows(_)),
            "un fallo real de Windows no debe convertirse en YaHayOtraInstancia"
        );
        assert_ne!(
            error.to_string(),
            ErrorInstanciaUnica::YaHayOtraInstancia.to_string(),
            "los dos casos deben tener mensajes distintos para no confundir a quien depure"
        );
    }
}
