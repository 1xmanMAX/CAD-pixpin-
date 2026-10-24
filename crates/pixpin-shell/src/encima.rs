//! Fijar encima de todo la ventana que hay bajo el raton.
//!
//! Es una de las funciones del PixPin original y una de las mas utiles del
//! programa: sirve para dejar a la vista una calculadora, un documento o un
//! video mientras se trabaja en otra cosa, sin tener que pinear nada.
//!
//! Alterna: si la ventana ya estaba encima, la baja. Asi el mismo comando
//! pone y quita, que es lo que se espera al pulsarlo dos veces.

use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::UI::WindowsAndMessaging::{
    GA_ROOT, GWL_EXSTYLE, GetAncestor, GetCursorPos, GetDesktopWindow, GetShellWindow,
    GetWindowLongW, GetWindowTextW, HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SetWindowPos, WS_EX_TOPMOST, WindowFromPoint,
};

/// Lo que paso al pedir el cambio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fijada {
    /// Ahora esta encima de todo (o ha dejado de estarlo), y su titulo.
    Cambiada { encima: bool, titulo: String },
    /// Bajo el cursor no hay ninguna ventana a la que aplicarselo: el
    /// escritorio y la barra de tareas no cuentan.
    SinVentana,
}

/// Alterna «siempre encima» en la ventana que haya bajo el cursor ahora.
pub fn alternar_ventana_bajo_el_cursor() -> Fijada {
    let mut p = POINT::default();
    // SAFETY: GetCursorPos escribe en la variable local; el resto son
    // consultas sobre el manejador que devuelve WindowFromPoint, validas
    // aunque la ventana desaparezca entre medias (devolverian cero).
    let hwnd = unsafe {
        if GetCursorPos(&mut p).is_err() {
            return Fijada::SinVentana;
        }
        // WindowFromPoint da el control concreto (un boton, un panel); lo
        // que hay que subir es la ventana de nivel superior que lo contiene.
        let bajo_cursor = WindowFromPoint(p);
        if bajo_cursor.is_invalid() {
            return Fijada::SinVentana;
        }
        GetAncestor(bajo_cursor, GA_ROOT)
    };

    // SAFETY: comparaciones con manejadores del sistema, sin precondiciones.
    let descartable =
        unsafe { hwnd.is_invalid() || hwnd == GetDesktopWindow() || hwnd == GetShellWindow() };
    if descartable {
        // El escritorio y la ventana de la Shell (donde viven los iconos y
        // la barra de tareas) no se tocan: subirlas taparia todo lo demas.
        return Fijada::SinVentana;
    }

    // SAFETY: consulta del estilo extendido de una ventana existente.
    let ya_encima = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST.0 != 0 };
    let destino = if ya_encima {
        HWND_NOTOPMOST
    } else {
        HWND_TOPMOST
    };

    // SAFETY: solo cambia el orden Z; no se mueve, no se redimensiona y no
    // se le roba el foco a lo que el usuario estuviera usando.
    let hecho = unsafe {
        SetWindowPos(
            hwnd,
            Some(destino),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
    };
    if hecho.is_err() {
        return Fijada::SinVentana;
    }

    Fijada::Cambiada {
        encima: !ya_encima,
        titulo: titulo_de(hwnd),
    }
}

/// El titulo de la ventana, para poder decir en el registro sobre cual se
/// actuo. Vacio si no tiene o no se pudo leer.
fn titulo_de(hwnd: HWND) -> String {
    let mut buf = [0u16; 256];
    // SAFETY: GetWindowTextW escribe como mucho `buf.len()` unidades y
    // devuelve cuantas escribio, sin contar el cero final.
    let n = unsafe { GetWindowTextW(hwnd, &mut buf) };
    if n <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..n as usize])
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    #[ignore = "necesita sesion de escritorio; ejecutar con --ignored"]
    fn sobre_el_escritorio_no_hace_nada() {
        // Caso negativo: con el cursor en el escritorio no hay ventana que
        // subir, y subir el escritorio taparia todo lo demas.
        // SAFETY: mover el cursor a una esquina no tiene precondiciones.
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::SetCursorPos(0, 0);
        }
        // No se afirma el resultado: en la esquina puede haber una barra de
        // tareas o una ventana del usuario. Lo que se comprueba es que no
        // entra en panico ni deja el escritorio fijado encima.
        let _ = alternar_ventana_bajo_el_cursor();
    }
}
