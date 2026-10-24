//! Entrada sintetizada y sondeo de teclas para la captura con scroll (D75).
//!
//! Windows entrega la rueda a la ventana que hay bajo el cursor sin
//! activarla (ajuste por defecto desde Windows 10), asi que hacer scroll en
//! la ventana de abajo es mover el cursor a la region y enviar la rueda con
//! `SendInput`. Y como el overlay esta OCULTO mientras se hace scroll, el
//! Escape del usuario no llega por ningun WndProc: se sondea.

use pixpin_geom::Punto;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_WHEEL, MOUSEINPUT, SendInput,
    VK_ESCAPE,
};
use windows::Win32::UI::WindowsAndMessaging::SetCursorPos;

/// Una muesca de rueda estandar.
const WHEEL_DELTA: i32 = 120;

/// Coloca el cursor en `p` (escritorio virtual) y envia `muescas` de rueda
/// hacia abajo (negativas: hacia arriba).
pub fn rueda_en(p: Punto, muescas: i32) {
    let entrada = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                // La palabra baja lleva el giro con signo; negativo = abajo.
                mouseData: (-(WHEEL_DELTA * muescas)) as u32,
                dwFlags: MOUSEEVENTF_WHEEL,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    // SAFETY: SetCursorPos no tiene precondiciones; SendInput recibe un
    // array propio de un elemento con el tamano correcto de la estructura.
    unsafe {
        let _ = SetCursorPos(p.x, p.y);
        let _ = SendInput(&[entrada], size_of::<INPUT>() as i32);
    }
}

/// Un punto del escritorio virtual en las coordenadas «absolutas» de
/// `SendInput`: de 0 a 65535 sobre TODO el escritorio virtual
/// (`MOUSEEVENTF_VIRTUALDESK`), no sobre el monitor principal.
///
/// `escritorio` es (x, y, ancho, alto) del escritorio virtual. Pura y
/// aparte porque es justo la cuenta que se tuerce con un monitor a la
/// izquierda (x negativa) y que no se puede mirar sin mover el raton.
pub fn normalizar(p: Punto, escritorio: (i32, i32, i32, i32)) -> (i32, i32) {
    let (ex, ey, ancho, alto) = escritorio;
    // Entre (ancho - 1): el ultimo pixel tiene que caer en 65535, no pasarse.
    let eje = |v: i32, origen: i32, largo: i32| -> i32 {
        let largo = (largo - 1).max(1) as i64;
        let dentro = ((v - origen) as i64).clamp(0, largo);
        // Redondeo al mas cercano: truncar desviaria un pixel los clics de
        // la mitad derecha de la pantalla.
        ((dentro * 65535 + largo / 2) / largo) as i32
    };
    (eje(p.x, ex, ancho), eje(p.y, ey, alto))
}

fn escritorio_virtual() -> (i32, i32, i32, i32) {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
        SM_YVIRTUALSCREEN,
    };
    // SAFETY: consultas puras de las medidas del sistema.
    unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    }
}

/// Hace `en_medio` con el cursor en `p` y lo devuelve a donde estaba.
///
/// Todo viaja en UNA llamada a `SendInput`: ir, actuar y volver. Con
/// `SetCursorPos` alrededor habria una carrera —`SendInput` solo encola, y
/// el cursor podria haber vuelto antes de que el sistema procesara el clic,
/// que entonces caeria en el pin y no en la zona—; dentro de un mismo lote
/// el orden esta garantizado.
/// Un paso de lo que se hace con el cursor ya en su sitio.
enum Paso {
    /// Un boton o la rueda: las banderas y su dato (el giro de la rueda).
    Accion(
        windows::Win32::UI::Input::KeyboardAndMouse::MOUSE_EVENT_FLAGS,
        u32,
    ),
    /// Llevar el cursor a otro punto sin soltar lo que este pulsado.
    Mover(Punto),
}

fn ir_actuar_y_volver(p: Punto, en_medio: &[Paso]) {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_MOVE, MOUSEEVENTF_VIRTUALDESK,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    let mut antes = POINT::default();
    // SAFETY: GetCursorPos escribe en un POINT propio.
    if unsafe { GetCursorPos(&mut antes) }.is_err() {
        return;
    }
    let escritorio = escritorio_virtual();
    let evento = |dx: i32, dy: i32, datos: u32, flags| INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: datos,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let mover = MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK;
    let (ix, iy) = normalizar(p, escritorio);
    let (vx, vy) = normalizar(
        Punto {
            x: antes.x,
            y: antes.y,
        },
        escritorio,
    );
    let mut lote = vec![evento(ix, iy, 0, mover)];
    lote.extend(en_medio.iter().map(|paso| match paso {
        Paso::Accion(flags, datos) => evento(0, 0, *datos, *flags),
        Paso::Mover(q) => {
            let (qx, qy) = normalizar(*q, escritorio);
            evento(qx, qy, 0, mover)
        }
    }));
    lote.push(evento(vx, vy, 0, mover));
    // SAFETY: SendInput recibe un array propio con el tamano correcto de la
    // estructura.
    unsafe {
        let _ = SendInput(&lote, size_of::<INPUT>() as i32);
    }
}

/// Un clic izquierdo de verdad en `p` (escritorio virtual), dejando el
/// cursor donde estaba. Para manejar a distancia la zona de un pin en vivo.
///
/// Entrada sintetizada y no `PostMessage` a la ventana de destino: los
/// navegadores, las aplicaciones modernas y los juegos ignoran un
/// `WM_LBUTTONDOWN` que no viene del sistema de entrada.
pub fn clic_en(p: Punto) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP};
    ir_actuar_y_volver(
        p,
        &[
            Paso::Accion(MOUSEEVENTF_LEFTDOWN, 0),
            Paso::Accion(MOUSEEVENTF_LEFTUP, 0),
        ],
    );
}

/// Cuantos puntos intermedios lleva un arrastre a distancia. Muchos
/// programas no dan por empezado un arrastre hasta ver moverse el raton con
/// el boton abajo, y una barra deslizante salta si solo ve el principio y el
/// final; ocho bastan para las dos cosas sin alargar el lote.
const PASOS_ARRASTRE: i32 = 8;

/// Los puntos por los que pasa un arrastre de `desde` a `hasta`, SIN el
/// primero (ahi ya esta el cursor al pulsar) y CON el ultimo exacto.
pub fn camino_de_arrastre(desde: Punto, hasta: Punto) -> Vec<Punto> {
    (1..=PASOS_ARRASTRE)
        .map(|i| Punto {
            x: desde.x + (hasta.x - desde.x) * i / PASOS_ARRASTRE,
            y: desde.y + (hasta.y - desde.y) * i / PASOS_ARRASTRE,
        })
        .collect()
}

/// Pulsar en `desde`, arrastrar hasta `hasta` y soltar, dejando el cursor
/// donde estaba. Para las barras deslizantes y las selecciones de la zona de
/// un pin en vivo.
pub fn arrastrar_a_distancia(desde: Punto, hasta: Punto) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP};
    let mut pasos = vec![Paso::Accion(MOUSEEVENTF_LEFTDOWN, 0)];
    pasos.extend(
        camino_de_arrastre(desde, hasta)
            .into_iter()
            .map(Paso::Mover),
    );
    pasos.push(Paso::Accion(MOUSEEVENTF_LEFTUP, 0));
    ir_actuar_y_volver(desde, &pasos);
}

/// La rueda en `p`, dejando el cursor donde estaba. `delta` es el de
/// `WM_MOUSEWHEEL` tal cual (120 por muesca, positivo hacia arriba).
pub fn rueda_a_distancia(p: Punto, delta: i32) {
    ir_actuar_y_volver(p, &[Paso::Accion(MOUSEEVENTF_WHEEL, delta as u32)]);
}

/// Si lo que hay en `p` es una ventana de este mismo proceso.
///
/// El clic a distancia lo pregunta antes de actuar: si el pin tapa su
/// propia zona, el clic caeria en el pin, que lo reenviaria otra vez.
pub fn punto_es_nuestro(p: Punto) -> bool {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::System::Threading::GetCurrentProcessId;
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowThreadProcessId, WindowFromPoint};
    // SAFETY: consultas puras sobre la ventana bajo un punto.
    unsafe {
        let hwnd = WindowFromPoint(POINT { x: p.x, y: p.y });
        if hwnd.is_invalid() {
            return false;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        pid == GetCurrentProcessId()
    }
}

/// Que modificadores estan pulsados ahora mismo.
///
/// Para grabar un atajo en la ventana de ajustes: el evento de tecla trae
/// Ctrl y Shift, pero no Alt ni Win, y sondearlos en el momento de la
/// pulsacion es mas fiable que arrastrar cuatro banderas por el WndProc.
pub fn modificadores_pulsados() -> crate::atajo::Modificadores {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
    };
    // SAFETY: consultas puras del estado del teclado.
    let pulsada = |vk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY| unsafe {
        (GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000) != 0
    };
    crate::atajo::Modificadores {
        ctrl: pulsada(VK_CONTROL),
        alt: pulsada(VK_MENU),
        shift: pulsada(VK_SHIFT),
        win: pulsada(VK_LWIN) || pulsada(VK_RWIN),
    }
}

/// Si Escape esta pulsado ahora mismo. Para el bucle de scroll, que no
/// tiene ventana con foco a la que le llegue la tecla (D76).
pub fn escape_pulsado() -> bool {
    // SAFETY: consulta pura del estado del teclado.
    unsafe { (GetAsyncKeyState(VK_ESCAPE.0 as i32) as u16 & 0x8000) != 0 }
}

/// Si la tecla o el boton de raton con este codigo virtual esta pulsado
/// AHORA MISMO, no en el ultimo evento que llego a la ventana.
///
/// Generaliza `modificadores_pulsados` y `boton_del_raton_pulsado` a
/// cualquier codigo: hacen falta dos casos donde el evento que cerraria un
/// gesto nunca llega (D136, editor avanzado): el espacio se suelta con un
/// Alt+Tab sin mandar `WM_KEYUP` a esta ventana, o Windows le quita la
/// captura al raton a mitad de un arrastre y el boton-arriba tampoco llega.
pub fn tecla_pulsada_ahora(vk: u32) -> bool {
    // SAFETY: consulta pura del estado del teclado o del raton.
    unsafe { (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 }
}

/// Si algun boton principal del raton esta pulsado ahora mismo. El gancho
/// de gestos se traga la pulsacion, pero el estado asincrono del sistema si
/// la refleja, asi que sirve de segunda opinion cuando la bandera del
/// gancho no basta.
pub fn boton_del_raton_pulsado() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{VK_LBUTTON, VK_MBUTTON, VK_RBUTTON};
    // SAFETY: consulta pura del estado de los botones.
    unsafe {
        (GetAsyncKeyState(VK_LBUTTON.0 as i32) as u16 & 0x8000) != 0
            || (GetAsyncKeyState(VK_RBUTTON.0 as i32) as u16 & 0x8000) != 0
            || (GetAsyncKeyState(VK_MBUTTON.0 as i32) as u16 & 0x8000) != 0
    }
}

/// Si estan pulsadas ahora mismo las mayusculas y Alt, en ese orden. Las
/// consulta el anotador antes de cada evento de puntero: mayusculas
/// restringe angulos y proporciones, y Alt duplica al arrastrar.
pub fn modificadores() -> (bool, bool) {
    // Sale de `modificadores_pulsados` y no de su propio sondeo: eran dos
    // copias de la misma consulta, y dos copias de una cosa asi acaban
    // discrepando el dia que una se arregle y la otra no.
    let m = modificadores_pulsados();
    (m.shift, m.alt)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    #[ignore = "necesita sesion de escritorio; ejecutar con --ignored"]
    fn sin_nadie_pulsando_escape_no_esta_pulsado() {
        // Caso negativo del sondeo: si esto diera true, la captura con
        // scroll se pararia sola en el primer paso.
        assert!(!escape_pulsado());
    }

    #[test]
    fn las_esquinas_del_escritorio_caen_en_cero_y_en_65535() {
        let escritorio = (0, 0, 1920, 1080);
        assert_eq!(normalizar(Punto { x: 0, y: 0 }, escritorio), (0, 0));
        assert_eq!(
            normalizar(Punto { x: 1919, y: 1079 }, escritorio),
            (65535, 65535),
            "el ultimo pixel es 65535 justo, no se pasa"
        );
    }

    #[test]
    fn con_un_monitor_a_la_izquierda_la_x_negativa_cuenta_desde_su_borde() {
        // El escritorio del usuario: un monitor a la izquierda del principal.
        // Una cuenta hecha sobre el principal mandaria el clic a otro sitio.
        let escritorio = (-1920, 0, 5880, 2235);
        assert_eq!(normalizar(Punto { x: -1920, y: 0 }, escritorio).0, 0);
        let (x, _) = normalizar(Punto { x: 0, y: 0 }, escritorio);
        // El origen del principal esta a 1920 de 5879 del borde izquierdo.
        assert_eq!(x, ((1920i64 * 65535 + 5879 / 2) / 5879) as i32);
        assert_eq!(
            normalizar(Punto { x: 3959, y: 2234 }, escritorio),
            (65535, 65535)
        );
    }

    #[test]
    fn un_punto_fuera_del_escritorio_se_arrima_al_borde_y_no_desborda() {
        // Caso negativo: nunca un valor fuera de 0..=65535, que SendInput
        // interpretaria a su manera.
        let escritorio = (0, 0, 1920, 1080);
        assert_eq!(
            normalizar(Punto { x: -500, y: 99999 }, escritorio),
            (0, 65535)
        );
        // Ni una division por cero con un escritorio degenerado.
        assert_eq!(
            normalizar(Punto { x: 5, y: 5 }, (0, 0, 1, 1)),
            (65535, 65535)
        );
        assert_eq!(normalizar(Punto { x: 0, y: 0 }, (0, 0, 0, 0)), (0, 0));
    }

    #[test]
    fn el_arrastre_pasa_por_el_medio_y_acaba_justo_donde_se_solto() {
        let camino = camino_de_arrastre(Punto { x: 100, y: 200 }, Punto { x: 900, y: 200 });
        assert_eq!(camino.len(), PASOS_ARRASTRE as usize);
        assert_eq!(
            camino[0],
            Punto { x: 200, y: 200 },
            "no repite el punto de pulsar"
        );
        assert_eq!(
            camino.last(),
            Some(&Punto { x: 900, y: 200 }),
            "el ultimo es exacto: una barra deslizante se queda donde se solto"
        );
        assert!(camino.windows(2).all(|w| w[1].x > w[0].x), "siempre avanza");
        // Caso negativo: hacia atras y en diagonal tambien llega, sin pasarse.
        let atras = camino_de_arrastre(Punto { x: 50, y: 50 }, Punto { x: -1900, y: 7 });
        assert_eq!(atras.last(), Some(&Punto { x: -1900, y: 7 }));
    }

    #[test]
    fn ida_y_vuelta_acierta_el_pixel() {
        // Lo que de verdad importa: que Windows, al deshacer la cuenta, caiga
        // en el MISMO pixel. Un pixel de error es fallar un boton pequeno.
        let escritorio = (-1920, 0, 5880, 2235);
        for x in [-1920, -1, 0, 1, 777, 2879, 3959] {
            let (n, _) = normalizar(Punto { x, y: 0 }, escritorio);
            let vuelta = (n as i64 * 5879 + 32767) / 65535 - 1920;
            assert_eq!(vuelta as i32, x, "x={x} normalizada={n}");
        }
    }
}
