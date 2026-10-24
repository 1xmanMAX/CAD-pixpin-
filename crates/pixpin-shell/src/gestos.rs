//! Gestos de raton con Alt en cualquier punto de la pantalla (D81).
//!
//! `Alt + boton izquierdo` y arrastrar selecciona y copia; `Alt + boton
//! derecho` y arrastrar selecciona y pinea. Los pidio el usuario en lugar de
//! los atajos de teclado `Ctrl+Alt+X` y `Ctrl+Alt+D`.
//!
//! Un gancho de raton de bajo nivel (`WH_MOUSE_LL`) ve la pulsacion antes
//! que la ventana de debajo. Si Alt esta pulsado (y ningun otro modificador),
//! se traga la pulsacion y se ESPERA: el gesto es arrastrar, no pulsar. Solo
//! cuando el cursor se aleja `UMBRAL_ARRASTRE` pixeles con el boton abajo se
//! avisa a la ventana de mensajes con `WM_GESTO`, y el bucle principal abre
//! el overlay ya con el arrastre en marcha desde el punto de la pulsacion.
//!
//! El usuario lo pidio asi tras probarlo: con el gesto disparado por la
//! pulsacion, elegir una ventana con el raton en el Alt+Tab (Alt sigue
//! abajo) abria un recorte. Si el boton se suelta sin haber arrastrado era
//! un clic corriente, y se le DEVUELVE a la ventana de debajo sintetizandolo:
//! la pulsacion ya se la habia tragado el gancho, y dejar pasar solo la
//! soltada entregaria medio clic.
//!
//! La soltada de un gesto ya anunciado NO se traga: para entonces el overlay
//! esta delante con la captura del raton y es quien tiene que recibirla.
//!
//! Mientras un overlay esta abierto el gancho se suspende: un Alt+clic
//! dentro del overlay es del overlay, no un gesto nuevo.
//!
//! El cuarto gesto, **Alt + doble clic central** (2026-09-28), abre el
//! anotador de pantalla; con el anotador abierto, alterna su clic a traves
//! (`DobleCentral`, `EscuchaAnotador`). El primer clic del par es un clic
//! corriente (se devuelve como cualquier Alt+clic sin arrastre); solo el
//! segundo, pulsacion y soltada, se lo traga el gancho.

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicIsize, AtomicU32, AtomicUsize, Ordering};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_MOUSE, MOUSE_EVENT_FLAGS, MOUSEEVENTF_LEFTDOWN,
    MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_RIGHTDOWN,
    MOUSEEVENTF_RIGHTUP, MOUSEINPUT, SendInput, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN,
    VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, HC_ACTION, HHOOK, MSLLHOOKSTRUCT, PostMessageW, SetWindowsHookExW,
    UnhookWindowsHookEx, WH_MOUSE_LL, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP,
    WM_MOUSEMOVE, WM_RBUTTONDOWN, WM_RBUTTONUP,
};

use crate::ventana::WM_GESTO;

/// La ventana que recibe `WM_GESTO`, como entero para poder vivir en un
/// estatico (el gancho es una funcion libre sin estado propio).
static DESTINO: AtomicIsize = AtomicIsize::new(0);
/// Con el gancho suspendido las pulsaciones pasan sin tocarse.
static SUSPENDIDO: AtomicBool = AtomicBool::new(false);
/// Hay un boton tragado por el gancho y todavia sin soltar. Se lleva aqui
/// y no con `GetAsyncKeyState`: una pulsacion que el gancho se traga nunca
/// llega al estado del teclado del sistema, asi que para Windows el boton
/// jamas estuvo abajo.
static EN_CURSO: AtomicBool = AtomicBool::new(false);
/// Cuanto hay que alejarse de la pulsacion, en pixeles fisicos, para que sea
/// un arrastre. Ocho: por debajo, el temblor de la mano al hacer clic ya lo
/// cruza (Windows usa 4 para arrastrar iconos, pero ahi equivocarse no abre
/// nada encima de la pantalla); por encima, el recorte empezaria a notarse
/// tarde en una seleccion pequena.
pub const UMBRAL_ARRASTRE: i32 = 8;
/// Sin boton a la espera.
const NINGUNO: usize = usize::MAX;
/// El `wParam` de `WM_GESTO` para Alt + doble clic central (0, 1 y 2 son
/// los arrastres con cada boton).
pub const GESTO_DOBLE_CENTRAL: usize = 3;
/// El boton tragado que todavia no se sabe si es clic o arrastre (0, 1 o 2),
/// o `NINGUNO`.
static PENDIENTE: AtomicUsize = AtomicUsize::new(NINGUNO);
/// Donde se pulso el boton pendiente: el gesto arranca AHI, no donde se
/// cruzo el umbral, o a la seleccion le faltarian los primeros pixeles.
static ORIGEN_X: AtomicI32 = AtomicI32::new(0);
static ORIGEN_Y: AtomicI32 = AtomicI32::new(0);
/// El gesto pendiente ya se aviso con `WM_GESTO`.
static ANUNCIADO: AtomicBool = AtomicBool::new(false);
/// La hora de la pulsacion del ultimo gesto (`MSLLHOOKSTRUCT::time`, el
/// reloj de `GetTickCount`): de ahi se mide cuanto tardo el overlay en salir.
static ORIGEN_T: AtomicU32 = AtomicU32::new(0);
/// El gesto anunciado ya se solto, y donde. Es lo que permite completar un
/// arrastre que acabo ANTES de que el overlay llegara a salir (2-oct: con el
/// overlay tardando, salia con el boton ya suelto, abria en exploracion y el
/// clic siguiente empezaba el recorte en otro sitio: captura incompleta).
static SOLTADO: AtomicBool = AtomicBool::new(false);
static SOLTADO_X: AtomicI32 = AtomicI32::new(0);
static SOLTADO_Y: AtomicI32 = AtomicI32::new(0);
/// Cuando se solto (`MSLLHOOKSTRUCT::time`).
static SOLTADO_T: AtomicU32 = AtomicU32::new(0);
/// Marca de los clics que sintetiza este modulo, para dejarlos pasar en vez
/// de volver a tragarselos (Alt sigue pulsado cuando se devuelven).
const MARCA_PROPIA: usize = 0x5049_5850; // "PIXP"
/// Cuantos editores de lienzo hay abiertos. Dentro del editor, Alt + arrastrar
/// duplica lo elegido (D93); si el gancho se lo tragara, abriria una captura
/// encima del dibujo. Es un contador porque el editor puede estar abierto
/// desde un pin y desde el chat, en hilos distintos.
static EDITORES: AtomicUsize = AtomicUsize::new(0);

/// Mientras viva, los gestos con Alt no se interceptan. La toma el editor.
pub struct PausaGestos(());

impl PausaGestos {
    pub fn tomar() -> Self {
        EDITORES.fetch_add(1, Ordering::SeqCst);
        PausaGestos(())
    }
}

impl Drop for PausaGestos {
    fn drop(&mut self) {
        EDITORES.fetch_sub(1, Ordering::SeqCst);
    }
}

// ------------------------------------------------ Alt + doble clic central

/// **Reconoce Alt + doble clic central** (el cuarto gesto, 2026-09-28: abre
/// el anotador de pantalla, y con el anotador abierto alterna el «clic a
/// traves»). Puro: el gancho le da la hora (`MSLLHOOKSTRUCT::time`, en ms) y
/// el sitio de cada pulsacion central.
///
/// Las reglas son las de Windows para un doble clic, para que se sienta
/// igual que cualquier otro: el segundo clic dentro de `GetDoubleClickTime`
/// y dentro del rectangulo `SM_CXDOUBLECLK x SM_CYDOUBLECLK` centrado en el
/// primero. Y los dos con Alt solo: un doble clic central sin Alt es de la
/// aplicacion de debajo y pasa tal cual.
#[derive(Debug, Default, Clone, Copy)]
pub struct DobleCentral {
    /// La pulsacion anterior con Alt que todavia puede ser la primera.
    primera: Option<(u32, i32, i32)>,
}

impl DobleCentral {
    /// Una pulsacion central. `true` si completa el doble clic; entonces se
    /// olvida, y un tercer clic empieza otro (como hace Windows).
    pub fn pulsacion(
        &mut self,
        t: u32,
        (x, y): (i32, i32),
        con_alt: bool,
        tiempo_doble: u32,
        (hx, hy): (i32, i32),
    ) -> bool {
        if !con_alt {
            self.primera = None;
            return false;
        }
        if let Some((t0, x0, y0)) = self.primera
            // `wrapping_sub`: el reloj de milisegundos da la vuelta.
            && t.wrapping_sub(t0) <= tiempo_doble
            && (x - x0).abs() * 2 <= hx
            && (y - y0).abs() * 2 <= hy
        {
            self.primera = None;
            return true;
        }
        self.primera = Some((t, x, y));
        false
    }

    /// Otro boton en medio: ya no es un doble clic.
    pub fn olvidar(&mut self) {
        self.primera = None;
    }
}

thread_local! {
    /// El reconocedor vive en el hilo del gancho, el unico que lo toca.
    static DOBLE: std::cell::Cell<DobleCentral> = const { std::cell::Cell::new(DobleCentral { primera: None }) };
}
/// La soltada del segundo clic central tambien se traga: la pulsacion ya se
/// la trago el gancho, y dejar pasar solo la soltada entregaria medio clic.
static TRAGAR_SOLTADA_CENTRAL: AtomicBool = AtomicBool::new(false);
/// El anotador de pantalla abierto que escucha Alt + doble clic central
/// (su ventana), o 0. Mientras haya uno el gesto es SUYO, este el gancho
/// suspendido o no: con el clic a traves puesto, el raton es de la
/// aplicacion de debajo y el gesto es la unica manera de volver a dibujar
/// sin buscar la pastilla.
static ANOTADOR: AtomicIsize = AtomicIsize::new(0);
/// El mensaje con que se despierta a esa ventana.
static MENSAJE_ANOTADOR: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
/// El anotador tiene un Alt + doble clic central sin leer.
static PEDIDO_ANOTADOR: AtomicBool = AtomicBool::new(false);

/// Mientras viva, Alt + doble clic central va al anotador de pantalla
/// `hwnd` (despertandolo con `mensaje`) en vez de abrir otro.
pub struct EscuchaAnotador(());

impl EscuchaAnotador {
    pub fn tomar(hwnd: HWND, mensaje: u32) -> Self {
        PEDIDO_ANOTADOR.store(false, Ordering::SeqCst);
        MENSAJE_ANOTADOR.store(mensaje, Ordering::SeqCst);
        ANOTADOR.store(hwnd.0 as isize, Ordering::SeqCst);
        EscuchaAnotador(())
    }
}

impl Drop for EscuchaAnotador {
    fn drop(&mut self) {
        ANOTADOR.store(0, Ordering::SeqCst);
        PEDIDO_ANOTADOR.store(false, Ordering::SeqCst);
    }
}

/// Si llego un Alt + doble clic central para el anotador. Se lee una vez.
pub fn tomar_doble_central_del_anotador() -> bool {
    PEDIDO_ANOTADOR.swap(false, Ordering::SeqCst)
}

/// El tiempo y la holgura de doble clic de Windows, como los tenga puestos
/// el usuario (Panel de control > Mouse).
fn reglas_de_doble_clic() -> (u32, (i32, i32)) {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CXDOUBLECLK, SM_CYDOUBLECLK,
    };
    // SAFETY: consultas de configuracion del sistema, sin precondiciones.
    unsafe {
        (
            GetDoubleClickTime(),
            (
                GetSystemMetrics(SM_CXDOUBLECLK),
                GetSystemMetrics(SM_CYDOUBLECLK),
            ),
        )
    }
}

/// Si el boton del ultimo gesto sigue pulsado. El overlay lo consulta al
/// abrirse para arrancar ya arrastrando.
pub fn gesto_en_curso() -> bool {
    EN_CURSO.load(Ordering::SeqCst)
}

/// Donde se solto el boton del ultimo gesto ANUNCIADO, si ya se solto.
///
/// El overlay lo mira al abrirse: si el usuario ya acabo el arrastre, el
/// recorte va de la pulsacion a la soltada, sin esperar un clic que nunca
/// iba a llegar. Una pulsacion nueva lo borra.
pub fn soltada_del_gesto() -> Option<(i32, i32)> {
    SOLTADO.load(Ordering::SeqCst).then(|| {
        (
            SOLTADO_X.load(Ordering::SeqCst),
            SOLTADO_Y.load(Ordering::SeqCst),
        )
    })
}

/// Pasado esto sin atenderse, un gesto ya no se reproduce: cuanto puede
/// esperar en la cola del hilo principal.
///
/// **Existe por un fallo real (10-oct-2026).** Con la ventana de ajustes
/// abierta en el hilo principal, tres arrastres con Alt se quedaron en la
/// cola; al cerrarla salieron TODOS, uno tras otro, de 2 a 9 segundos tarde y
/// con recortes que ya no tenian que ver con nada: seis pines que nadie
/// pidio. Un gesto es «quiero recortar esto AHORA»; segundos despues, la mano
/// ya esta en otra cosa.
pub const CADUCIDAD_GESTO_MS: u32 = 1500;

/// Si un gesto anunciado a la hora `hora` ya no debe abrir nada a la hora
/// `ahora` (reloj de `GetTickCount`). `sigue_en_curso`: es el gesto cuyo
/// boton sigue abajo, y ese se atiende aunque llegue tarde (la mano sigue
/// en el, y el overlay abrira arrastrando desde la pulsacion).
///
/// La resta se lee con signo: una hora un pelo POR DELANTE de `ahora` (dos
/// lecturas del mismo reloj en hilos distintos) es reciente, no de hace 49
/// dias.
pub fn gesto_caducado(hora: u32, ahora: u32, sigue_en_curso: bool) -> bool {
    !sigue_en_curso && (ahora.wrapping_sub(hora) as i32) > CADUCIDAD_GESTO_MS as i32
}

/// Si el gesto que arranco en `(x, y)` es el ultimo anunciado y su boton
/// sigue abajo. Un gesto viejo de la cola nunca lo es: cada pulsacion nueva
/// pisa el origen.
pub fn es_el_gesto_en_curso(x: i32, y: i32) -> bool {
    EN_CURSO.load(Ordering::SeqCst)
        && ANUNCIADO.load(Ordering::SeqCst)
        && ORIGEN_X.load(Ordering::SeqCst) == x
        && ORIGEN_Y.load(Ordering::SeqCst) == y
}

/// Hace cuantos milisegundos se solto el ultimo gesto anunciado, si ya se
/// solto. Lo usa el overlay para darse cuenta de que la soltada que el
/// gancho SI vio nunca le llego a el (ver `overlay::soltada_perdida`).
pub fn edad_de_la_soltada() -> Option<u32> {
    SOLTADO
        .load(Ordering::SeqCst)
        // Con signo: una hora un pelo por delante del reloj es «ahora».
        .then(|| (reloj_ms().wrapping_sub(SOLTADO_T.load(Ordering::SeqCst)) as i32).max(0) as u32)
}

/// La hora (reloj de `GetTickCount`) de la pulsacion del ultimo gesto.
pub fn hora_de_la_pulsacion() -> u32 {
    ORIGEN_T.load(Ordering::SeqCst)
}

/// La hora del mensaje que se esta atendiendo (`GetMessageTime`): para un
/// atajo, el instante en que se pulso la combinacion.
pub fn hora_del_mensaje() -> u32 {
    // SAFETY: consulta del hilo actual, sin precondiciones.
    unsafe { windows::Win32::UI::WindowsAndMessaging::GetMessageTime() as u32 }
}

/// El reloj de `GetTickCount`, el mismo de las dos horas de arriba.
pub fn reloj_ms() -> u32 {
    // SAFETY: consulta del reloj del sistema, sin precondiciones.
    unsafe { windows::Win32::System::SystemInformation::GetTickCount() }
}

/// Milisegundos de `desde` a `hasta` en ese reloj, que da la vuelta cada
/// 49 dias.
pub fn ms_entre(desde: u32, hasta: u32) -> u32 {
    hasta.wrapping_sub(desde)
}

/// El gancho instalado. Al soltarlo se desinstala y su hilo termina.
///
/// # Por que el gancho tiene un hilo para el solo
///
/// Un gancho de bajo nivel se ejecuta en el hilo que lo instalo, cuando ese
/// hilo bombea mensajes, y Windows le pone un tope: si el hilo tarda en
/// atender un evento, se lo salta, y a la de varias **retira el gancho sin
/// avisar**. Vivia en el hilo principal, y al llegar la pila de capturas el
/// trabajo que ese hilo hace al cerrar un recorte (bajar la imagen de la GPU,
/// el portapapeles, la miniatura, crear la ventana del icono) paso del tope
/// con el raton todavia moviendose: la primera captura salia y despues los
/// gestos morian sin dejar ni una linea en el registro, con el programa
/// respondiendo con normalidad. Medido el 2026-09-20 con gestos sinteticos.
///
/// En un hilo que solo bombea sus propios mensajes nada puede retrasarlo. El
/// procedimiento solo toca atomicos y publica `WM_GESTO` con `PostMessageW`,
/// que se puede llamar desde cualquier hilo, asi que no hizo falta cambiar
/// nada mas.
pub struct GanchoRaton {
    hilo: Option<std::thread::JoinHandle<()>>,
    id_hilo: u32,
}

impl GanchoRaton {
    /// Instala el gancho; los gestos llegan a `destino` como `WM_GESTO`.
    pub fn instalar(destino: HWND) -> windows::core::Result<Self> {
        use windows::Win32::System::Threading::GetCurrentThreadId;
        use windows::Win32::UI::WindowsAndMessaging::{GetMessageW, MSG};

        DESTINO.store(destino.0 as isize, Ordering::SeqCst);
        SUSPENDIDO.store(false, Ordering::SeqCst);
        let (tx, rx) = std::sync::mpsc::channel::<windows::core::Result<u32>>();
        let hilo = std::thread::Builder::new()
            .name("gancho-raton".into())
            .spawn(move || {
                // SAFETY: gancho global de bajo nivel con un procedimiento de
                // este modulo; sin hmodule ni hilo porque corre en ESTE hilo,
                // que es el que bombea justo debajo.
                let gancho: HHOOK =
                    match unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(procedimiento), None, 0) } {
                        Ok(g) => g,
                        Err(e) => {
                            let _ = tx.send(Err(e));
                            return;
                        }
                    };
                // SAFETY: consulta del identificador del hilo actual.
                let _ = tx.send(Ok(unsafe { GetCurrentThreadId() }));
                let mut msg = MSG::default();
                // El bucle no reparte nada: este hilo no tiene ventanas. Solo
                // existe para que Windows tenga donde ejecutar el gancho, y
                // sale con el WM_QUIT que manda `Drop`.
                // SAFETY: GetMessageW escribe en un MSG propio.
                while unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 > 0 {}
                // SAFETY: desinstala el gancho que instalo este mismo hilo.
                unsafe {
                    let _ = UnhookWindowsHookEx(gancho);
                }
            })
            .map_err(|e| windows::core::Error::new(windows::core::HRESULT(-1), e.to_string()))?;
        let id_hilo = rx
            .recv()
            .map_err(|e| windows::core::Error::new(windows::core::HRESULT(-1), e.to_string()))??;
        Ok(Self {
            hilo: Some(hilo),
            id_hilo,
        })
    }

    /// Deja pasar (o vuelve a interceptar) las pulsaciones con Alt.
    pub fn suspender(&self, suspendido: bool) {
        SUSPENDIDO.store(suspendido, Ordering::SeqCst);
    }
}

impl Drop for GanchoRaton {
    fn drop(&mut self) {
        use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT};
        DESTINO.store(0, Ordering::SeqCst);
        // SAFETY: publicar WM_QUIT en la cola del hilo del gancho; si el hilo
        // ya no existe, falla y no hay nada que esperar.
        let avisado = unsafe { PostThreadMessageW(self.id_hilo, WM_QUIT, WPARAM(0), LPARAM(0)) };
        // El gancho lo desinstala su propio hilo al salir del bucle. Solo se
        // espera si el aviso llego: esperar a un hilo que no va a enterarse
        // colgaria el cierre del programa.
        if let (Ok(()), Some(hilo)) = (avisado, self.hilo.take()) {
            let _ = hilo.join();
        }
    }
}

fn pulsada(tecla: VIRTUAL_KEY) -> bool {
    // SAFETY: consulta del estado del teclado, sin precondiciones.
    unsafe { (GetAsyncKeyState(tecla.0 as i32) as u16 & 0x8000) != 0 }
}

/// Si la combinacion de modificadores es exactamente Alt.
fn solo_alt() -> bool {
    pulsada(VK_MENU)
        && !pulsada(VK_CONTROL)
        && !pulsada(VK_SHIFT)
        && !pulsada(VK_LWIN)
        && !pulsada(VK_RWIN)
}

/// Empaqueta un punto en un `LPARAM` como hacen `MAKELPARAM` y los mensajes
/// de raton: x en la palabra baja, y en la alta, ambas con signo.
pub fn empaquetar_punto(x: i32, y: i32) -> LPARAM {
    LPARAM(((x & 0xFFFF) | ((y & 0xFFFF) << 16)) as isize)
}

/// Si de `origen` a `p` hay ya un arrastre y no el temblor de un clic.
///
/// Por ejes y no por distancia euclidea: es lo que hace Windows con
/// `SM_CXDRAG`, y asi no hay raices ni desbordamientos que pensar.
pub fn es_arrastre(origen: (i32, i32), p: (i32, i32)) -> bool {
    (p.0 - origen.0).abs() >= UMBRAL_ARRASTRE || (p.1 - origen.1).abs() >= UMBRAL_ARRASTRE
}

/// Devuelve a la ventana de debajo el clic que el gancho se habia tragado.
fn devolver_clic(boton: usize) {
    let (abajo, arriba): (MOUSE_EVENT_FLAGS, MOUSE_EVENT_FLAGS) = match boton {
        0 => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
        1 => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
        _ => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
    };
    let evento = |flags| INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: MARCA_PROPIA,
            },
        },
    };
    // SAFETY: SendInput recibe un array propio con el tamano correcto de la
    // estructura. Sin coordenadas: el clic cae donde esta el cursor, que no
    // se ha movido del sitio (por eso no fue un arrastre).
    unsafe {
        let _ = SendInput(&[evento(abajo), evento(arriba)], size_of::<INPUT>() as i32);
    }
}

extern "system" fn procedimiento(codigo: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let mensaje = wparam.0 as u32;
    if codigo != HC_ACTION as i32 {
        // SAFETY: pasar el evento al siguiente gancho de la cadena.
        return unsafe { CallNextHookEx(None, codigo, wparam, lparam) };
    }
    // SAFETY: con HC_ACTION, lParam apunta a un MSLLHOOKSTRUCT valido durante
    // la llamada.
    let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
    // El clic que este mismo modulo devuelve pasa sin mirarlo: Alt sigue
    // abajo, y tragarselo otra vez seria un bucle.
    if info.dwExtraInfo == MARCA_PROPIA {
        // SAFETY: pasar el evento al siguiente gancho de la cadena.
        return unsafe { CallNextHookEx(None, codigo, wparam, lparam) };
    }

    // **Alt + doble clic central** (ver `DobleCentral`). Va antes que lo
    // demas: su segunda pulsacion no es un clic pendiente ni un arrastre.
    if mensaje == WM_MBUTTONUP && TRAGAR_SOLTADA_CENTRAL.swap(false, Ordering::SeqCst) {
        return LRESULT(1);
    }
    if matches!(mensaje, WM_LBUTTONDOWN | WM_RBUTTONDOWN) {
        DOBLE.with(|d| {
            let mut v = d.get();
            v.olvidar();
            d.set(v);
        });
    }
    if mensaje == WM_MBUTTONDOWN {
        let (tiempo, holgura) = reglas_de_doble_clic();
        let con_alt = solo_alt();
        let doble = DOBLE.with(|d| {
            let mut v = d.get();
            let doble = v.pulsacion(info.time, (info.pt.x, info.pt.y), con_alt, tiempo, holgura);
            d.set(v);
            doble
        });
        if doble {
            let anotador = ANOTADOR.load(Ordering::SeqCst);
            let destino = DESTINO.load(Ordering::SeqCst);
            if anotador != 0 {
                // El anotador ya esta abierto: alterna su clic a traves. Da
                // igual que el gancho este suspendido (lo esta, con el
                // anotador delante): el gesto es suyo.
                PEDIDO_ANOTADOR.store(true, Ordering::SeqCst);
                TRAGAR_SOLTADA_CENTRAL.store(true, Ordering::SeqCst);
                tracing::info!("gesto: Alt + doble clic central, al anotador abierto");
                // SAFETY: publicar un mensaje propio en una ventana propia.
                unsafe {
                    let _ = PostMessageW(
                        Some(HWND(anotador as *mut _)),
                        MENSAJE_ANOTADOR.load(Ordering::SeqCst),
                        WPARAM(0),
                        LPARAM(0),
                    );
                }
                return LRESULT(1);
            }
            if !SUSPENDIDO.load(Ordering::SeqCst)
                && EDITORES.load(Ordering::SeqCst) == 0
                && destino != 0
            {
                // El primer clic ya se devolvio (se solto sin arrastrar); este
                // segundo no es un clic pendiente: es el gesto.
                PENDIENTE.store(NINGUNO, Ordering::SeqCst);
                EN_CURSO.store(false, Ordering::SeqCst);
                TRAGAR_SOLTADA_CENTRAL.store(true, Ordering::SeqCst);
                tracing::info!(
                    x = info.pt.x,
                    y = info.pt.y,
                    "gesto: Alt + doble clic central"
                );
                // SAFETY: publicar un mensaje propio en una ventana propia.
                unsafe {
                    let _ = PostMessageW(
                        Some(HWND(destino as *mut _)),
                        WM_GESTO,
                        WPARAM(GESTO_DOBLE_CENTRAL),
                        empaquetar_punto(info.pt.x, info.pt.y),
                    );
                }
                return LRESULT(1);
            }
        }
    }

    let soltado = match mensaje {
        WM_LBUTTONUP => Some(0usize),
        WM_RBUTTONUP => Some(1usize),
        WM_MBUTTONUP => Some(2usize),
        _ => None,
    };
    // Las soltadas se atienden este suspendido el gancho o no: cierran el
    // gesto, y el overlay necesita recibir la suya.
    if let Some(boton) = soltado {
        EN_CURSO.store(false, Ordering::SeqCst);
        if PENDIENTE.load(Ordering::SeqCst) == boton {
            PENDIENTE.store(NINGUNO, Ordering::SeqCst);
            if ANUNCIADO.load(Ordering::SeqCst) {
                // El arrastre acabo. Si el overlay aun no ha salido, con esto
                // sabra donde terminaba el recorte.
                SOLTADO_X.store(info.pt.x, Ordering::SeqCst);
                SOLTADO_Y.store(info.pt.y, Ordering::SeqCst);
                SOLTADO_T.store(info.time, Ordering::SeqCst);
                SOLTADO.store(true, Ordering::SeqCst);
            }
            if !ANUNCIADO.load(Ordering::SeqCst) {
                tracing::info!(
                    boton,
                    dx = info.pt.x - ORIGEN_X.load(Ordering::SeqCst),
                    dy = info.pt.y - ORIGEN_Y.load(Ordering::SeqCst),
                    "gesto: soltado sin arrastrar, se devuelve el clic"
                );
                // Se solto sin arrastrar: era un clic de la ventana de debajo.
                // La soltada se traga tambien, porque el clic entero se
                // devuelve sintetizado; dejarla pasar daria dos soltadas.
                devolver_clic(boton);
                return LRESULT(1);
            }
        }
    }

    // El arrastre que convierte la pulsacion pendiente en gesto.
    if mensaje == WM_MOUSEMOVE {
        let boton = PENDIENTE.load(Ordering::SeqCst);
        let destino = DESTINO.load(Ordering::SeqCst);
        let origen = (
            ORIGEN_X.load(Ordering::SeqCst),
            ORIGEN_Y.load(Ordering::SeqCst),
        );
        if boton != NINGUNO
            && destino != 0
            && !ANUNCIADO.load(Ordering::SeqCst)
            && es_arrastre(origen, (info.pt.x, info.pt.y))
        {
            ANUNCIADO.store(true, Ordering::SeqCst);
            // Un arrastre no es la primera mitad de un doble clic.
            DOBLE.with(|d| {
                let mut v = d.get();
                v.olvidar();
                d.set(v);
            });
            tracing::info!(boton, "gesto: arrastre, se anuncia");
            // SAFETY: publicar un mensaje propio en una ventana propia.
            unsafe {
                let _ = PostMessageW(
                    Some(HWND(destino as *mut _)),
                    WM_GESTO,
                    WPARAM(boton),
                    empaquetar_punto(origen.0, origen.1),
                );
            }
        }
    }

    let pulsado = match mensaje {
        WM_LBUTTONDOWN => Some(0usize),
        WM_RBUTTONDOWN => Some(1usize),
        WM_MBUTTONDOWN => Some(2usize),
        _ => None,
    };
    if let Some(boton) = pulsado
        && pulsada(VK_MENU)
    {
        // Cuanto tardo el evento en llegar aqui. El gancho corre en el hilo
        // principal: si ese hilo esta ocupado, Windows entrega tarde (y pasado
        // su tope, NO entrega), y un gesto perdido asi no deja mas rastro.
        // SAFETY: consulta del reloj del sistema, sin precondiciones.
        let ahora = unsafe { windows::Win32::System::SystemInformation::GetTickCount() };
        let retraso_ms = ahora.wrapping_sub(info.time);
        let suspendido = SUSPENDIDO.load(Ordering::SeqCst);
        let editores = EDITORES.load(Ordering::SeqCst);
        let destino = DESTINO.load(Ordering::SeqCst);
        let solo = solo_alt();
        if !suspendido && editores == 0 && destino != 0 && solo {
            // Un pendiente viejo NO bloquea: si su soltada se perdio (el
            // gancho no la vio por el tope de Windows), quedarse esperandola
            // dejaria los gestos muertos hasta el proximo clic normal.
            let viejo = PENDIENTE.swap(boton, Ordering::SeqCst);
            ORIGEN_X.store(info.pt.x, Ordering::SeqCst);
            ORIGEN_Y.store(info.pt.y, Ordering::SeqCst);
            ANUNCIADO.store(false, Ordering::SeqCst);
            SOLTADO.store(false, Ordering::SeqCst);
            ORIGEN_T.store(info.time, Ordering::SeqCst);
            EN_CURSO.store(true, Ordering::SeqCst);
            tracing::info!(
                boton,
                x = info.pt.x,
                y = info.pt.y,
                retraso_ms,
                pendiente_viejo = viejo != NINGUNO,
                "gesto: pulsacion retenida"
            );
            // Tragar la pulsacion: si acaba en arrastre, la ventana de debajo
            // no debe haber reaccionado; si acaba en clic, se le devuelve.
            return LRESULT(1);
        }
        // Con Alt abajo y sin gesto: que quede dicho por que. Es el unico
        // rastro que deja un gesto que «no funciona».
        tracing::info!(
            boton,
            suspendido,
            editores,
            sin_destino = destino == 0,
            ctrl = pulsada(VK_CONTROL),
            shift = pulsada(VK_SHIFT),
            win = pulsada(VK_LWIN) || pulsada(VK_RWIN),
            retraso_ms,
            "gesto: pulsacion con Alt NO retenida"
        );
    }
    // SAFETY: pasar el evento al siguiente gancho de la cadena.
    unsafe { CallNextHookEx(None, codigo, wparam, lparam) }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// El tiempo y la holgura de fabrica de Windows: 500 ms y 4x4 pixeles.
    const T: u32 = 500;
    const H: (i32, i32) = (4, 4);

    #[test]
    fn dos_clics_centrales_con_alt_juntos_y_a_tiempo_abren_el_anotador() {
        let mut d = DobleCentral::default();
        assert!(
            !d.pulsacion(1000, (300, 300), true, T, H),
            "el primero solo espera"
        );
        assert!(
            d.pulsacion(1300, (301, 299), true, T, H),
            "el segundo lo completa"
        );
        // Justo en el borde del tiempo y de la holgura tambien vale.
        let mut d = DobleCentral::default();
        d.pulsacion(0, (-1900, 40), true, T, H);
        assert!(
            d.pulsacion(500, (-1898, 42), true, T, H),
            "monitor de la izquierda"
        );
    }

    #[test]
    fn sin_alt_lentos_o_lejos_no_es_el_gesto() {
        // Sin Alt: el doble clic central de siempre, que pasa tal cual.
        let mut d = DobleCentral::default();
        d.pulsacion(1000, (300, 300), false, T, H);
        assert!(!d.pulsacion(1200, (300, 300), false, T, H));
        // El primero con Alt y el segundo sin el tampoco.
        let mut d = DobleCentral::default();
        d.pulsacion(1000, (300, 300), true, T, H);
        assert!(!d.pulsacion(1200, (300, 300), false, T, H));
        // Lentos: dos clics sueltos.
        let mut d = DobleCentral::default();
        d.pulsacion(1000, (300, 300), true, T, H);
        assert!(!d.pulsacion(1501, (300, 300), true, T, H));
        // Lejos: fuera del rectangulo de doble clic.
        let mut d = DobleCentral::default();
        d.pulsacion(1000, (300, 300), true, T, H);
        assert!(!d.pulsacion(1100, (303, 300), true, T, H));
        let mut d = DobleCentral::default();
        d.pulsacion(1000, (300, 300), true, T, H);
        assert!(!d.pulsacion(1100, (300, 297), true, T, H));
    }

    #[test]
    fn un_tercer_clic_no_vuelve_a_disparar_y_otro_boton_en_medio_lo_corta() {
        let mut d = DobleCentral::default();
        d.pulsacion(0, (10, 10), true, T, H);
        assert!(d.pulsacion(100, (10, 10), true, T, H));
        assert!(
            !d.pulsacion(200, (10, 10), true, T, H),
            "el tercero empieza otro"
        );
        assert!(
            d.pulsacion(300, (10, 10), true, T, H),
            "y el cuarto lo cierra"
        );
        // Un clic izquierdo entre los dos centrales ya no es un doble clic.
        let mut d = DobleCentral::default();
        d.pulsacion(0, (10, 10), true, T, H);
        d.olvidar();
        assert!(!d.pulsacion(100, (10, 10), true, T, H));
        // El reloj de Windows da la vuelta cada 49 dias: un doble clic justo
        // en ese momento sigue valiendo (y no da una resta negativa).
        let mut d = DobleCentral::default();
        d.pulsacion(u32::MAX - 100, (10, 10), true, T, H);
        assert!(
            d.pulsacion(100, (10, 10), true, T, H),
            "a traves de la vuelta, 201 ms"
        );
    }

    #[test]
    fn la_peticion_del_anotador_se_entrega_una_sola_vez() {
        let _ = tomar_doble_central_del_anotador();
        assert!(!tomar_doble_central_del_anotador());
        PEDIDO_ANOTADOR.store(true, Ordering::SeqCst);
        assert!(tomar_doble_central_del_anotador());
        assert!(!tomar_doble_central_del_anotador(), "leida, se borra");
    }

    #[test]
    fn la_pausa_del_editor_se_suelta_al_cerrarlo() {
        let antes = EDITORES.load(Ordering::SeqCst);
        {
            let _a = PausaGestos::tomar();
            let _b = PausaGestos::tomar();
            assert_eq!(EDITORES.load(Ordering::SeqCst), antes + 2, "dos editores");
        }
        assert_eq!(
            EDITORES.load(Ordering::SeqCst),
            antes,
            "cerrados los dos, los gestos vuelven"
        );
    }

    #[test]
    fn un_clic_con_alt_no_es_un_gesto_y_un_arrastre_si() {
        // Lo que fallaba: elegir una ventana con el raton en el Alt+Tab
        // abria un recorte. El temblor de un clic no cruza el umbral.
        assert!(!es_arrastre((500, 500), (500, 500)));
        assert!(!es_arrastre((500, 500), (503, 497)));
        assert!(!es_arrastre((500, 500), (507, 507)), "justo por debajo");
        // Arrastrar en cualquier direccion si, tambien en un solo eje y
        // hacia arriba o hacia la izquierda.
        assert!(es_arrastre((500, 500), (508, 500)));
        assert!(es_arrastre((500, 500), (500, 492)));
        assert!(
            es_arrastre((-1900, 40), (-1950, 40)),
            "monitor de la izquierda"
        );
    }

    #[test]
    fn la_cuenta_de_milisegundos_sobrevive_a_la_vuelta_del_reloj() {
        assert_eq!(ms_entre(1_000, 1_250), 250);
        assert_eq!(ms_entre(u32::MAX - 9, 10), 20, "a traves de la vuelta");
    }

    /// El fallo del 10-oct: gestos de hace 2 a 9 segundos reproducidos al
    /// cerrar los ajustes. Ninguno de esos debe abrir nada; uno reciente si.
    #[test]
    fn un_gesto_que_espero_demasiado_en_la_cola_ya_no_se_reproduce() {
        let ahora = 1_000_000;
        assert!(!gesto_caducado(ahora - 100, ahora, false), "reciente");
        assert!(
            !gesto_caducado(ahora - CADUCIDAD_GESTO_MS, ahora, false),
            "justo en el tope"
        );
        // Los del registro: 2016, 3797 y 9157 ms en la cola.
        for espera in [2016, 3797, 9157] {
            assert!(gesto_caducado(ahora - espera, ahora, false), "{espera} ms");
        }
        // El boton sigue abajo: la mano sigue en el gesto, se atiende.
        assert!(!gesto_caducado(ahora - 9157, ahora, true));
        // Una hora un pelo por delante (otro hilo leyo el reloj despues).
        assert!(!gesto_caducado(ahora + 3, ahora, false));
        // A traves de la vuelta del reloj.
        assert!(gesto_caducado(u32::MAX - 999, 1_000, false));
        assert!(!gesto_caducado(u32::MAX - 9, 10, false));
    }

    #[test]
    fn el_punto_empaquetado_se_desempaqueta_con_signo() {
        // Un monitor a la izquierda del principal tiene x negativa; la
        // ventana de mensajes lo desempaqueta igual que aqui.
        let l = empaquetar_punto(-120, 45);
        let x = (l.0 & 0xFFFF) as u16 as i16 as i32;
        let y = ((l.0 >> 16) & 0xFFFF) as u16 as i16 as i32;
        assert_eq!((x, y), (-120, 45));

        let l = empaquetar_punto(2999, 1999);
        let x = (l.0 & 0xFFFF) as u16 as i16 as i32;
        let y = ((l.0 >> 16) & 0xFFFF) as u16 as i16 as i32;
        assert_eq!((x, y), (2999, 1999));
    }
}
