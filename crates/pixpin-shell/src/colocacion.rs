//! **Donde esta una ventana y como devolverla alli** (H9, grupos de
//! ventanas): `GetWindowPlacement`/`SetWindowPlacement` y lo necesario para
//! encontrar las ventanas de un hilo sin que quien las crea sepa nada.
//!
//! Vive aqui porque el ejecutable no admite `unsafe`; lo que decide que se
//! guarda y que se abre esta en `apps/pixpin/src/grupos_ventanas.rs`.

use serde::{Deserialize, Serialize};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONULL, MonitorFromRect};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumThreadWindows, GW_OWNER, GWL_EXSTYLE, GWL_STYLE, GetWindow, GetWindowLongPtrW,
    GetWindowPlacement, IsWindow, IsWindowVisible, SW_HIDE, SW_SHOWNORMAL, SetWindowPlacement,
    WINDOWPLACEMENT, WINDOWPLACEMENT_FLAGS, WPF_ASYNCWINDOWPLACEMENT, WS_EX_TOOLWINDOW,
    WS_THICKFRAME,
};

/// El `WINDOWPLACEMENT` de Windows, en algo que se puede escribir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Colocacion {
    /// `showCmd`: normal, minimizada o maximizada.
    pub mostrar: u32,
    pub banderas: u32,
    /// El rectangulo «normal» (el de cuando no esta minimizada ni
    /// maximizada), en coordenadas del area de trabajo.
    pub izquierda: i32,
    pub arriba: i32,
    pub derecha: i32,
    pub abajo: i32,
    pub min_x: i32,
    pub min_y: i32,
    pub max_x: i32,
    pub max_y: i32,
}

impl Colocacion {
    fn de_windows(w: &WINDOWPLACEMENT) -> Colocacion {
        let r = w.rcNormalPosition;
        Colocacion {
            mostrar: w.showCmd,
            banderas: w.flags.0,
            izquierda: r.left,
            arriba: r.top,
            derecha: r.right,
            abajo: r.bottom,
            min_x: w.ptMinPosition.x,
            min_y: w.ptMinPosition.y,
            max_x: w.ptMaxPosition.x,
            max_y: w.ptMaxPosition.y,
        }
    }

    fn a_windows(&self) -> WINDOWPLACEMENT {
        WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            flags: WINDOWPLACEMENT_FLAGS(self.banderas),
            showCmd: self.mostrar,
            ptMinPosition: POINT {
                x: self.min_x,
                y: self.min_y,
            },
            ptMaxPosition: POINT {
                x: self.max_x,
                y: self.max_y,
            },
            rcNormalPosition: self.rectangulo(),
        }
    }

    fn rectangulo(&self) -> RECT {
        RECT {
            left: self.izquierda,
            top: self.arriba,
            right: self.derecha,
            bottom: self.abajo,
        }
    }

    fn tamano(&self) -> (i32, i32) {
        (self.derecha - self.izquierda, self.abajo - self.arriba)
    }
}

/// El id del hilo que llama.
pub fn hilo_actual() -> u32 {
    // SAFETY: sin argumentos; solo lee el id del hilo actual.
    unsafe { windows::Win32::System::Threading::GetCurrentThreadId() }
}

/// Las ventanas de primer nivel de un hilo, en el orden de Windows.
pub fn ventanas_del_hilo(hilo: u32) -> Vec<isize> {
    unsafe extern "system" fn cada(hwnd: HWND, l: LPARAM) -> windows::core::BOOL {
        // SAFETY: `l` es el puntero al Vec que `ventanas_del_hilo` presta
        // durante la llamada sincrona a EnumThreadWindows.
        let v = unsafe { &mut *(l.0 as *mut Vec<isize>) };
        v.push(hwnd.0 as isize);
        true.into()
    }
    let mut v: Vec<isize> = Vec::new();
    // SAFETY: el Vec vive hasta que EnumThreadWindows vuelve; la devolucion
    // solo empuja handles. Un hilo sin ventanas devuelve falso y nada mas.
    unsafe {
        let _ = EnumThreadWindows(hilo, Some(cada), LPARAM(&mut v as *mut Vec<isize> as isize));
    }
    v
}

/// Las ventanas de primer nivel de este proceso (de todos sus hilos).
pub fn ventanas_del_proceso() -> Vec<isize> {
    unsafe extern "system" fn cada(hwnd: HWND, l: LPARAM) -> windows::core::BOOL {
        // SAFETY: `l` es el puntero al par que `ventanas_del_proceso` presta
        // durante la llamada sincrona a EnumWindows.
        let (yo, v) = unsafe { &mut *(l.0 as *mut (u32, Vec<isize>)) };
        let mut suyo = 0u32;
        // SAFETY: consulta de solo lectura; escribe en una variable local.
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(
                hwnd,
                Some(&mut suyo),
            );
        }
        if suyo == *yo {
            v.push(hwnd.0 as isize);
        }
        true.into()
    }
    // SAFETY: sin argumentos; solo lee el id del proceso.
    let yo = unsafe { windows::Win32::System::Threading::GetCurrentProcessId() };
    let mut par: (u32, Vec<isize>) = (yo, Vec::new());
    // SAFETY: el par vive hasta que EnumWindows vuelve.
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::EnumWindows(
            Some(cada),
            LPARAM(&mut par as *mut _ as isize),
        );
    }
    par.1
}

/// **Lo que se abra ahora, delante** (los pedidos de fuera, como el plugin
/// de Flow Launcher): durante `espera` mira las ventanas nuevas de este
/// proceso que no estaban en `antes`, y la primera que se vea como ventana
/// la saca si nacio minimizada (un lienzo abierto con el chat minimizado
/// salia asi, medido el 2-oct) y la trae al frente. En su hilo: no para a
/// nadie. El permiso de pasar al frente lo da quien pidio
/// (`AllowSetForegroundWindow`); sin el, Windows solo hace parpadear su boton.
pub fn al_frente_lo_nuevo(antes: Vec<isize>, espera: std::time::Duration) {
    let _ = std::thread::Builder::new()
        .name("al-frente".into())
        .spawn(move || {
            let desde = std::time::Instant::now();
            while desde.elapsed() < espera {
                if let Some(h) = ventanas_del_proceso()
                    .into_iter()
                    .find(|h| !antes.contains(h) && es_principal(*h))
                {
                    crate::overlay::VentanaOverlay::restaurar_de_hwnd(HWND(h as *mut _));
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(80));
            }
        });
}

/// Si una ventana es de las que se ven como ventana: viva, visible, sin
/// duena y no de herramientas (menus y paneles flotantes no cuentan).
pub fn es_principal(h: isize) -> bool {
    let hwnd = HWND(h as *mut _);
    // SAFETY: consultas de solo lectura sobre un handle que puede haber
    // muerto ya; Windows responde «no» a un handle muerto.
    unsafe {
        IsWindow(Some(hwnd)).as_bool()
            && IsWindowVisible(hwnd).as_bool()
            && GetWindow(hwnd, GW_OWNER)
                .map(|o| o.0.is_null())
                .unwrap_or(true)
            && (GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0) == 0
    }
}

/// Donde esta una ventana ahora. `None` si ya no existe.
pub fn leer(h: isize) -> Option<Colocacion> {
    let mut w = WINDOWPLACEMENT {
        length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
        ..Default::default()
    };
    // SAFETY: estructura propia con su `length`; un handle muerto da error.
    unsafe { GetWindowPlacement(HWND(h as *mut _), &mut w) }.ok()?;
    Some(Colocacion::de_windows(&w))
}

/// La colocacion que de verdad se aplica.
///
/// Los lectores y el lienzo son ventanas del tamano del monitor que no se
/// redimensionan (su superficie de dibujo nace con ese tamano). A una de
/// esas solo se la mueve si lo guardado mide lo mismo que ella: si no, se
/// le deja su rectangulo y solo se le pone su estado (minimizada o no). Y
/// lo que cae fuera de todos los monitores de ahora (se desenchufo una
/// pantalla) se queda donde esta: perder una ventana fuera de la vista es
/// peor que no moverla.
pub fn a_aplicar(
    guardada: &Colocacion,
    actual: &Colocacion,
    redimensionable: bool,
    cabe: bool,
) -> Colocacion {
    let mut c = *guardada;
    if !cabe || !(redimensionable || guardada.tamano() == actual.tamano()) {
        c.izquierda = actual.izquierda;
        c.arriba = actual.arriba;
        c.derecha = actual.derecha;
        c.abajo = actual.abajo;
    }
    // Una ventana guardada oculta no se reabre oculta: seria abrirla para nada.
    if c.mostrar == SW_HIDE.0 as u32 {
        c.mostrar = SW_SHOWNORMAL.0 as u32;
    }
    c
}

/// Devuelve una ventana a donde estaba (ver [`a_aplicar`]). Tambien la
/// ensena, con el estado guardado. Asincrono si es de otro hilo: esperar a
/// que ese hilo la atienda podria dejar parado a quien coloca.
pub fn poner(h: isize, guardada: &Colocacion) -> bool {
    let hwnd = HWND(h as *mut _);
    let Some(actual) = leer(h) else {
        return false;
    };
    // SAFETY: solo lecturas sobre la ventana y un rectangulo propio.
    let (redimensionable, cabe) = unsafe {
        let estilo = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let r = guardada.rectangulo();
        (
            estilo & WS_THICKFRAME.0 != 0,
            !MonitorFromRect(&r, MONITOR_DEFAULTTONULL).is_invalid(),
        )
    };
    let mut nueva = a_aplicar(guardada, &actual, redimensionable, cabe).a_windows();
    nueva.flags |= WPF_ASYNCWINDOWPLACEMENT;
    // SAFETY: estructura propia y completa; un handle muerto solo da error.
    unsafe { SetWindowPlacement(hwnd, &nueva) }.is_ok()
}

/// Un menu llano (ver [`crate::menu_llano`]) cuando no hay ventana duena a
/// mano, como desde la bandeja: se usa una oculta de un momento.
pub fn menu_sin_duena(entradas: &[(u32, String)]) -> Option<u32> {
    use windows::Win32::UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow, WS_POPUP};
    // SAFETY: la clase «STATIC» es del sistema; la ventana nace oculta y se
    // destruye antes de salir.
    unsafe {
        let h = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            windows::core::w!("STATIC"),
            windows::core::w!(""),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            None,
            None,
        )
        .ok()?;
        let r = crate::menu_llano(h, entradas);
        let _ = DestroyWindow(h);
        r
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn c(x: i32, ancho: i32) -> Colocacion {
        Colocacion {
            mostrar: SW_SHOWNORMAL.0 as u32,
            banderas: 0,
            izquierda: x,
            arriba: 0,
            derecha: x + ancho,
            abajo: 600,
            min_x: -1,
            min_y: -1,
            max_x: -1,
            max_y: -1,
        }
    }

    #[test]
    fn una_ventana_redimensionable_vuelve_entera_a_su_sitio() {
        assert_eq!(a_aplicar(&c(900, 500), &c(0, 800), true, true), c(900, 500));
    }

    #[test]
    fn a_una_de_tamano_fijo_solo_se_la_mueve_si_mide_lo_mismo() {
        // Mismo tamano, otro monitor: se mueve.
        assert_eq!(
            a_aplicar(&c(1920, 800), &c(0, 800), false, true),
            c(1920, 800)
        );
        // Otro tamano: se queda donde esta (su superficie no se estira).
        assert_eq!(
            a_aplicar(&c(1920, 1024), &c(0, 800), false, true),
            c(0, 800)
        );
    }

    #[test]
    fn lo_que_cae_fuera_de_las_pantallas_no_se_mueve_y_lo_oculto_se_ve() {
        assert_eq!(a_aplicar(&c(9000, 500), &c(0, 800), true, false), c(0, 800));
        let mut oculta = c(0, 800);
        oculta.mostrar = SW_HIDE.0 as u32;
        assert_eq!(
            a_aplicar(&oculta, &c(0, 800), true, true).mostrar,
            SW_SHOWNORMAL.0 as u32
        );
    }

    #[test]
    fn la_colocacion_va_y_vuelve_de_windows_igual() {
        let x = c(10, 300);
        assert_eq!(Colocacion::de_windows(&x.a_windows()), x);
    }

    #[test]
    fn un_hilo_sin_ventanas_no_tiene_ninguna_y_una_muerta_no_es_principal() {
        let h = std::thread::spawn(hilo_actual).join().unwrap();
        assert!(ventanas_del_hilo(h).is_empty());
        assert!(!es_principal(0));
        assert_eq!(leer(0), None);
    }
}
