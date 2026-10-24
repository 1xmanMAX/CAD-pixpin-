//! Registro de los atajos globales.
//!
//! **Un atajo ocupado no es un error fatal.** Otra aplicacion puede tener ya
//! Ctrl+Alt+X, y cerrarse por eso seria desproporcionado: se registra todo lo
//! que se pueda, se devuelve la lista de los que fallaron, y el ejecutable
//! avisa al usuario para que elija otro.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, RegisterHotKey, UnregisterHotKey,
};

use crate::atajo::Atajo;

pub const ID_REGION: u32 = 1;
pub const ID_COPIAR: u32 = 2;
pub const ID_SCROLL: u32 = 3;
pub const ID_CUENTAGOTAS: u32 = 4;
/// Recortar y dejar flotando como pin (S2, D26).
pub const ID_PIN: u32 = 5;
/// Abrir la capa viva para anotar sobre la pantalla (S3-C, D49).
pub const ID_ANOTAR: u32 = 7;
/// Anotar sobre la pantalla congelada (S3-C, D56).
pub const ID_ANOTAR_CONGELADA: u32 = 8;

/// Mientras esto viva, los atajos siguen registrados.
pub struct AtajosRegistrados {
    hwnd: HWND,
    ids: Vec<u32>,
}

impl Drop for AtajosRegistrados {
    fn drop(&mut self) {
        for id in &self.ids {
            // SAFETY: cada id se registro con exito sobre este mismo hwnd y no
            // se ha liberado antes; este tipo no es Clone ni Copy.
            unsafe {
                let _ = UnregisterHotKey(Some(self.hwnd), *id as i32);
            }
        }
    }
}

/// Registra todas las peticiones que pueda.
///
/// Devuelve el guardia con las que funcionaron y la lista de las que no.
pub fn registrar(
    hwnd: HWND,
    peticiones: &[(u32, Atajo)],
) -> (AtajosRegistrados, Vec<(u32, Atajo)>) {
    let mut ids = Vec::new();
    let mut fallidos = Vec::new();

    for (id, atajo) in peticiones {
        // SAFETY: el llamante debe garantizar que `hwnd` es una ventana
        // valida de este hilo; esta funcion lo recibe sin comprobarlo. Los
        // codigos vienen de `Atajo`, que solo produce combinaciones bien
        // formadas.
        let ok = unsafe {
            RegisterHotKey(
                Some(hwnd),
                *id as i32,
                HOT_KEY_MODIFIERS(atajo.modificadores_win32()),
                atajo.tecla_win32(),
            )
        };

        if ok.is_ok() {
            ids.push(*id);
        } else {
            fallidos.push((*id, *atajo));
        }
    }

    (AtajosRegistrados { hwnd, ids }, fallidos)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::ventana::VentanaMensajes;

    #[test]
    // RegisterHotKey necesita una sesion de escritorio interactiva (una
    // estacion de ventanas con acceso de entrada); un runner de CI hospedado
    // normalmente no la tiene, asi que este test se deja fuera de la
    // ejecucion normal. Se ejecuta a mano con `cargo test -- --ignored`.
    #[ignore = "necesita una sesion de escritorio interactiva. cargo test -- --ignored"]
    fn registra_y_libera_una_combinacion_poco_usada() {
        let v = VentanaMensajes::nueva().unwrap();
        // Combinacion rara a proposito para no chocar con nada real.
        let raro: Atajo = "Ctrl+Alt+Shift+F24".parse().unwrap();

        let (registrados, fallidos) = registrar(v.handle(), &[(ID_REGION, raro)]);

        assert!(fallidos.is_empty(), "no deberia fallar: {fallidos:?}");
        drop(registrados);

        // Tras liberar, la misma combinacion debe poder registrarse otra vez.
        let (otros, fallidos) = registrar(v.handle(), &[(ID_REGION, raro)]);
        assert!(fallidos.is_empty(), "tras liberar deberia poder repetirse");
        drop(otros);
    }

    #[test]
    #[ignore = "necesita una sesion de escritorio interactiva. cargo test -- --ignored"]
    fn un_atajo_ocupado_se_informa_en_vez_de_abortar() {
        // Registrar dos veces la misma combinacion: la segunda choca.
        let v = VentanaMensajes::nueva().unwrap();
        let raro: Atajo = "Ctrl+Alt+Shift+F23".parse().unwrap();

        let (primeros, _) = registrar(v.handle(), &[(ID_REGION, raro)]);
        let (segundos, fallidos) = registrar(v.handle(), &[(ID_COPIAR, raro)]);

        assert_eq!(fallidos.len(), 1, "el choque debe reportarse");
        assert_eq!(fallidos[0].0, ID_COPIAR);

        drop(segundos);
        drop(primeros);
    }
}
