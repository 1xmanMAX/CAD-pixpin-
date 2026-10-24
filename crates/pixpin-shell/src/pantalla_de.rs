//! **En que monitor esta una ventana** (`MonitorFromWindow`).
//!
//! Lo pide el telepronter (B8): se abre a pantalla completa **en el otro
//! monitor**, el que no tiene el chat delante, que es como se usa un
//! telepronter de verdad (el texto en la pantalla que mira la camara, los
//! mandos en la de trabajo). Para saber cual es «el otro» hace falta saber
//! en cual esta la ventana desde la que se abrio.

use pixpin_geom::Rect;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

/// El rectangulo del monitor en que esta `hwnd` (el mas cercano si esta
/// entre dos), o `None` si Windows no lo sabe.
pub fn monitor_de_ventana(hwnd: HWND) -> Option<Rect> {
    // SAFETY: consultas sin efectos sobre una ventana cualquiera; si ya no
    // existe, `MonitorFromWindow` da el mas cercano y nada se rompe.
    unsafe {
        let m = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(m, &mut info).as_bool() {
            return None;
        }
        let r = info.rcMonitor;
        Some(Rect {
            x: r.left,
            y: r.top,
            ancho: (r.right - r.left).max(0) as u32,
            alto: (r.bottom - r.top).max(0) as u32,
        })
    }
}

/// El monitor de la ventana que tiene el foco ahora: la que acaba de pedir
/// abrir algo.
pub fn monitor_de_la_ventana_activa() -> Option<Rect> {
    // SAFETY: consulta sin precondiciones.
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_invalid() {
        return None;
    }
    monitor_de_ventana(hwnd)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn una_ventana_que_no_existe_cae_en_el_monitor_mas_cercano_o_en_ninguno() {
        // Con `MONITOR_DEFAULTTONEAREST` hasta un HWND nulo da un monitor;
        // lo que no puede es devolver un rectangulo vacio.
        if let Some(r) = monitor_de_ventana(HWND(std::ptr::null_mut())) {
            assert!(r.ancho > 0 && r.alto > 0, "{r:?}");
        }
    }
}
