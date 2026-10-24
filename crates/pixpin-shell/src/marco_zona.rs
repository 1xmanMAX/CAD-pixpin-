//! El recuadro de la zona de un pin en vivo.
//!
//! El usuario lo pidio asi: «que la zona que se esta proyectando se ponga
//! con un recuadro para saber que zona es». Con el pin lejos de su zona, y
//! mas aun manejandola a distancia, no se sabia de donde salia la imagen.
//!
//! Son cuatro tiras finas POR FUERA de la zona, no una ventana encima:
//!
//! - lo de dentro sigue siendo la aplicacion de verdad, sin nada nuestro
//!   delante. El clic a distancia pregunta que ventana hay en el punto
//!   (`entrada::punto_es_nuestro`) y una ventana nuestra encima de toda la
//!   zona se los comeria todos;
//! - las tiras no salen en ninguna captura (`WDA_EXCLUDEFROMCAPTURE`), ni en
//!   la del propio pin ni en las del usuario, y el raton las atraviesa
//!   (`WS_EX_TRANSPARENT`);
//! - se cuidan solas: la primera mira cada poco a su pin y se esconde con el
//!   (Ctrl+2) o se cierra, con las otras tres, cuando el pin ya no existe.
//!   Asi no hace falta acordarse de quitarlas en cada sitio que cierra o
//!   congela un pin en vivo.

use std::sync::Once;

use pixpin_geom::Rect;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::CreateSolidBrush;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GWLP_USERDATA, GetWindowLongPtrW, IsWindow,
    IsWindowVisible, KillTimer, LWA_ALPHA, RegisterClassExW, SW_HIDE, SW_SHOWNOACTIVATE,
    SetLayeredWindowAttributes, SetTimer, SetWindowDisplayAffinity, SetWindowLongPtrW, ShowWindow,
    WDA_EXCLUDEFROMCAPTURE, WM_NCDESTROY, WM_TIMER, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::w;

const CLASE: windows::core::PCWSTR = w!("PixPinMaxMarcoZona");
const TEMPORIZADOR: usize = 1;
/// Cada cuanto mira la primera tira a su pin. Un cuarto de segundo de
/// recuadro de mas tras cerrar el pin no se nota; mirar mas a menudo, si.
const CADA_MS: u32 = 250;
/// El azul de acento de la aplicacion (`Color::ACENTO`), en `0x00BBGGRR`.
const AZUL: u32 = 0x00F2_8C21;
/// Casi opaco: el recuadro tiene que verse sobre cualquier fondo.
const OPACIDAD: u8 = 230;

/// **Pura**: las cuatro tiras de grosor `g` que rodean `zona` por fuera,
/// arriba, abajo, izquierda y derecha. Las de arriba y abajo cubren tambien
/// las esquinas.
pub fn tiras(zona: Rect, g: u32) -> [Rect; 4] {
    let gi = g as i32;
    let (w, h) = (zona.ancho as i32, zona.alto as i32);
    [
        Rect {
            x: zona.x - gi,
            y: zona.y - gi,
            ancho: zona.ancho + 2 * g,
            alto: g,
        },
        Rect {
            x: zona.x - gi,
            y: zona.y + h,
            ancho: zona.ancho + 2 * g,
            alto: g,
        },
        Rect {
            x: zona.x - gi,
            y: zona.y,
            ancho: g,
            alto: zona.alto,
        },
        Rect {
            x: zona.x + w,
            y: zona.y,
            ancho: g,
            alto: zona.alto,
        },
    ]
}

/// Lo que la primera tira recuerda: su pin y las otras tres.
struct Vigia {
    pin: HWND,
    otras: Vec<HWND>,
}

fn registrar() {
    static UNA_VEZ: Once = Once::new();
    UNA_VEZ.call_once(|| {
        // SAFETY: registro de una clase propia con un procedimiento valido;
        // el pincel vive lo que el proceso, como la clase.
        unsafe {
            let instancia = GetModuleHandleW(None).unwrap_or_default();
            let clase = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(procedimiento),
                hInstance: instancia.into(),
                hbrBackground: CreateSolidBrush(COLORREF(AZUL)),
                lpszClassName: CLASE,
                ..Default::default()
            };
            let _ = RegisterClassExW(&clase);
        }
    });
}

/// Pone el recuadro alrededor de `zona` (pixeles fisicos) mientras exista
/// la ventana `pin`. `escala_por_cien` es la del monitor de la zona.
pub fn poner(zona: Rect, escala_por_cien: u32, pin: HWND) {
    registrar();
    let g = (3 * escala_por_cien / 100).max(2);
    let mut hechas = Vec::with_capacity(4);
    for r in tiras(zona, g) {
        // SAFETY: ventana emergente de una clase propia ya registrada; los
        // demas parametros son constantes o medidas propias.
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST
                    | WS_EX_TOOLWINDOW
                    | WS_EX_NOACTIVATE
                    | WS_EX_LAYERED
                    | WS_EX_TRANSPARENT,
                CLASE,
                None,
                WS_POPUP,
                r.x,
                r.y,
                r.ancho as i32,
                r.alto as i32,
                None,
                None,
                GetModuleHandleW(None).ok().map(Into::into),
                None,
            )
        };
        let Ok(hwnd) = hwnd else {
            tracing::warn!(?zona, "no se pudo crear el recuadro de la zona en vivo");
            continue;
        };
        // SAFETY: llamadas sobre una ventana propia recien creada.
        unsafe {
            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), OPACIDAD, LWA_ALPHA);
            // Anterior a Windows 10 2004 falla sin mas: el recuadro sigue,
            // solo que saldria en las capturas que lo pillen.
            let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        hechas.push(hwnd);
    }
    let Some((&jefa, otras)) = hechas.split_first() else {
        return;
    };
    let vigia = Box::new(Vigia {
        pin,
        otras: otras.to_vec(),
    });
    // SAFETY: el puntero se guarda en la ventana y se libera en su
    // WM_NCDESTROY; el temporizador es de esa misma ventana.
    unsafe {
        SetWindowLongPtrW(jefa, GWLP_USERDATA, Box::into_raw(vigia) as isize);
        SetTimer(Some(jefa), TEMPORIZADOR, CADA_MS, None);
    }
}

unsafe extern "system" fn procedimiento(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TIMER if wparam.0 == TEMPORIZADOR => {
            // SAFETY: el puntero lo puso `poner` y vive hasta WM_NCDESTROY.
            let Some(v) =
                (unsafe { (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Vigia).as_ref() })
            else {
                return LRESULT(0);
            };
            // SAFETY: consultas y cambios sobre ventanas propias; `IsWindow`
            // tolera un pin ya destruido.
            unsafe {
                if !IsWindow(Some(v.pin)).as_bool() {
                    // DestroyWindow de la jefa cierra tambien las otras
                    // (ver WM_NCDESTROY).
                    let _ = DestroyWindow(hwnd);
                    return LRESULT(0);
                }
                let a_la_vista = IsWindowVisible(v.pin).as_bool();
                if a_la_vista != IsWindowVisible(hwnd).as_bool() {
                    let modo = if a_la_vista {
                        SW_SHOWNOACTIVATE
                    } else {
                        SW_HIDE
                    };
                    for &t in std::iter::once(&hwnd).chain(&v.otras) {
                        let _ = ShowWindow(t, modo);
                    }
                }
            }
            LRESULT(0)
        }
        WM_NCDESTROY => {
            // SAFETY: se recupera el Box de `poner` una sola vez; tras
            // ponerlo a cero, un segundo WM_NCDESTROY no lo libera otra vez.
            unsafe {
                let _ = KillTimer(Some(hwnd), TEMPORIZADOR);
                let p = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *mut Vigia;
                if !p.is_null() {
                    let v = Box::from_raw(p);
                    for t in v.otras {
                        let _ = DestroyWindow(t);
                    }
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }
        // SAFETY: el resto, como cualquier ventana.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_tiras_rodean_la_zona_sin_pisarla() {
        let zona = Rect {
            x: 100,
            y: 50,
            ancho: 400,
            alto: 300,
        };
        let t = tiras(zona, 3);
        for r in t {
            assert!(r.interseccion(zona).is_none(), "{r:?} pisa la zona");
        }
        // Arriba y abajo cubren las esquinas; los lados, solo la altura.
        assert_eq!(
            t[0],
            Rect {
                x: 97,
                y: 47,
                ancho: 406,
                alto: 3
            }
        );
        assert_eq!(t[1].y, 350);
        assert_eq!((t[2].x, t[2].alto), (97, 300));
        assert_eq!(t[3].x, 500);
    }
}
