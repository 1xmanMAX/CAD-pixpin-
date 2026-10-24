//! **Las ventanas que se ven, con su nombre**, de delante a atras.
//!
//! Para el modo «Ventana» de la captura (v2-captura): se fotografian ANTES
//! de ensenar el overlay, porque con el delante la ventana de debajo del
//! raton seria siempre el propio overlay. Con la lista en la mano, el
//! overlay solo tiene que buscar la primera que contenga el cursor: es la de
//! mas arriba, la que el usuario ve.
//!
//! Se quedan fuera las que no se ven como ventana: invisibles, minimizadas,
//! de herramientas, con dueno, sin titulo y las «encapotadas» por DWM (las
//! aplicaciones de la tienda suspendidas y las de otros escritorios
//! virtuales, que Windows da por visibles aunque no se vean).

use pixpin_geom::Rect;
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{
    DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GW_OWNER, GWL_EXSTYLE, GetClassNameW, GetWindow, GetWindowLongPtrW,
    InternalGetWindowText, IsIconic, IsWindowVisible, WS_EX_TOOLWINDOW,
};

/// Una ventana de primer nivel que se ve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VentanaVisible {
    /// El marco que se ve (sin la sombra invisible de Windows 10/11).
    pub rect: Rect,
    pub titulo: String,
}

/// Las ventanas visibles con titulo, en orden Z: la primera es la de
/// delante. Las del overlay de captura no salen nunca.
pub fn ventanas_visibles() -> Vec<VentanaVisible> {
    unsafe extern "system" fn cada(hwnd: HWND, l: LPARAM) -> windows::core::BOOL {
        // SAFETY: `l` es el puntero al Vec que `ventanas_visibles` presta
        // durante la llamada sincrona a EnumWindows.
        let v = unsafe { &mut *(l.0 as *mut Vec<VentanaVisible>) };
        // SAFETY: `hwnd` lo da EnumWindows; `leer` solo hace consultas de
        // lectura que aceptan un handle muerto.
        if let Some(w) = unsafe { leer(hwnd) } {
            v.push(w);
        }
        true.into()
    }
    let mut v: Vec<VentanaVisible> = Vec::new();
    // SAFETY: el Vec vive hasta que EnumWindows vuelve; la devolucion solo
    // lee de cada ventana y empuja.
    unsafe {
        let _ = EnumWindows(
            Some(cada),
            LPARAM(&mut v as *mut Vec<VentanaVisible> as isize),
        );
    }
    v
}

/// La ventana `hwnd` si cuenta como visible.
///
/// # Safety
/// `hwnd` puede haber muerto: todas las llamadas son de solo lectura y
/// Windows contesta «no» a un handle muerto.
unsafe fn leer(hwnd: HWND) -> Option<VentanaVisible> {
    // SAFETY: solo consultas de lectura sobre `hwnd` (ver arriba) y buferes
    // locales con su tamano.
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
            return None;
        }
        if GetWindow(hwnd, GW_OWNER).is_ok_and(|o| !o.0.is_null()) {
            return None;
        }
        if (GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0) != 0 {
            return None;
        }
        let mut clase = [0u16; 32];
        let n = GetClassNameW(hwnd, &mut clase);
        if n > 0 && String::from_utf16_lossy(&clase[..n as usize]) == "PixPinOverlay" {
            return None;
        }
        let mut encapotada: u32 = 0;
        if DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut encapotada as *mut u32 as *mut _,
            size_of::<u32>() as u32,
        )
        .is_ok()
            && encapotada != 0
        {
            return None;
        }
        let mut buf = [0u16; 256];
        // `InternalGetWindowText` y no `GetWindowTextW`: con una ventana de
        // ESTE proceso, la segunda le MANDA `WM_GETTEXT` y espera a que su
        // hilo conteste; si ese hilo esta parado esperando algo del que
        // pregunta (el overlay corre en el hilo principal), no vuelve nunca.
        // La primera lee el titulo guardado sin mandar nada.
        let largo = InternalGetWindowText(hwnd, &mut buf);
        if largo <= 0 {
            return None;
        }
        let titulo = String::from_utf16_lossy(&buf[..largo as usize]);
        let mut r = RECT::default();
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut r as *mut RECT as *mut _,
            size_of::<RECT>() as u32,
        )
        .ok()?;
        let rect = Rect {
            x: r.left,
            y: r.top,
            ancho: (r.right - r.left).max(0) as u32,
            alto: (r.bottom - r.top).max(0) as u32,
        };
        if rect.esta_vacio() {
            return None;
        }
        Some(VentanaVisible { rect, titulo })
    }
}

/// La de mas arriba que contiene `p`: la primera de la lista, que ya viene
/// de delante a atras.
pub fn ventana_en(ventanas: &[VentanaVisible], p: pixpin_geom::Punto) -> Option<&VentanaVisible> {
    ventanas.iter().find(|v| v.rect.contiene(p))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_geom::Punto;

    fn v(x: i32, y: i32, ancho: u32, alto: u32, t: &str) -> VentanaVisible {
        VentanaVisible {
            rect: Rect { x, y, ancho, alto },
            titulo: t.into(),
        }
    }

    #[test]
    fn gana_la_de_delante_aunque_la_de_atras_sea_mas_pequena() {
        let lista = [v(0, 0, 800, 600, "delante"), v(100, 100, 50, 50, "detras")];
        assert_eq!(
            ventana_en(&lista, Punto { x: 120, y: 120 }).map(|w| w.titulo.as_str()),
            Some("delante")
        );
        // Caso negativo: fuera de todas, ninguna.
        assert_eq!(ventana_en(&lista, Punto { x: 900, y: 900 }), None);
    }
}
