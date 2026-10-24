//! **Espacio en el Explorador: vista rapida** (10-oct-2026, el usuario: «una
//! nueva funcion de abrir archivos soportados con solo darle espacio a los
//! archivos seleccionados, que puedan abrirse de forma rapida y sencilla»).
//!
//! Un gancho de teclado de bajo nivel mira **solo la barra espaciadora**. Si
//! lo que esta delante es el Explorador (o el escritorio) y el foco esta en
//! su lista de ficheros —no en el cuadro de cambiar el nombre, ni en la
//! barra de direcciones, ni en el buscador—, se traga la pulsacion y se la
//! pasa a un hilo de trabajo, que lee la seleccion ([`crate::explorador`]) y
//! decide (lo hace la app: que se abre y que no). Si la app dice que no era
//! para ella, el hilo devuelve el espacio al Explorador tal cual.
//!
//! El gancho no pregunta nada a otro proceso: solo la ventana de delante, su
//! clase y la del foco (microsegundos). Windows quita un gancho de bajo
//! nivel que tarde, y uno lento haria ir a tirones todo el teclado.

use std::cell::Cell;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
    VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT, VK_SPACE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GUITHREADINFO, GetAncestor, GetClassNameW, GetForegroundWindow,
    GetGUIThreadInfo, GetParent, GetWindowThreadProcessId, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT,
    LLKHF_INJECTED, SetWindowsHookExW, UnhookWindowsHookEx, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN,
    WM_SYSKEYUP,
};

/// Encendido o no (el interruptor de Ajustes, en vivo).
static ACTIVO: AtomicBool = AtomicBool::new(true);
static TRABAJO: Mutex<Option<Sender<isize>>> = Mutex::new(None);
/// Se trago un espacio y falta su soltada (y las repeticiones de tenerlo
/// pulsado): tambien se tragan.
static TRAGANDO: AtomicBool = AtomicBool::new(false);
/// Lo que llevan las pulsaciones que devolvemos, para no volver a cogerlas.
const MARCA_PROPIA: usize = 0x5049_5853; // "PIXS"

/// Encender o apagar la vista rapida (sin quitar el gancho).
pub fn activar(si: bool) {
    ACTIVO.store(si, Ordering::Relaxed);
}

/// Las ventanas de delante que son el Explorador o el escritorio.
const DELANTE: [&str; 4] = ["CabinetWClass", "ExploreWClass", "Progman", "WorkerW"];

/// **La decision del gancho**, sin Windows (para probarla): el espacio solo,
/// con el Explorador delante y el foco en su lista de ficheros. `foco` es la
/// clase de la ventana con el foco y `antepasados` las de sus padres.
pub fn es_para_la_vista_rapida(
    modificadores: bool,
    delante: &str,
    foco: &str,
    antepasados: &[String],
) -> bool {
    if modificadores || !DELANTE.contains(&delante) {
        return false;
    }
    // La lista de ficheros: «DirectUIHWND» (Explorador) o «SysListView32»
    // (escritorio), siempre dentro de «SHELLDLL_DefView». Un «Edit» (cambiar
    // el nombre, la direccion) o el buscador de Windows 11 no lo son.
    matches!(foco, "DirectUIHWND" | "SysListView32")
        && antepasados.iter().any(|a| a == "SHELLDLL_DefView")
}

fn pulsada(tecla: VIRTUAL_KEY) -> bool {
    // SAFETY: GetAsyncKeyState solo lee el estado de una tecla.
    (unsafe { GetAsyncKeyState(tecla.0 as i32) } as u16 & 0x8000) != 0
}

fn clase(h: HWND) -> String {
    let mut b = [0u16; 64];
    // SAFETY: escribe como mucho `b.len()` caracteres en el buffer propio.
    let n = unsafe { GetClassNameW(h, &mut b) };
    String::from_utf16_lossy(&b[..n.max(0) as usize])
}

thread_local! {
    /// La ultima ventana de delante y si es del Explorador (la clase no
    /// cambia: no se pregunta en cada espacio que se escribe en Word).
    static CACHE: Cell<(isize, bool)> = const { Cell::new((0, false)) };
}

/// Mira la ventana de delante y la del foco; `Some(delante)` si el espacio
/// es nuestro.
fn mirar() -> Option<HWND> {
    // SAFETY: llamadas de solo lectura sobre ventanas que pueden no ser
    // nuestras; si alguna ya no existe, devuelven cero o una cadena vacia.
    unsafe {
        let delante = GetForegroundWindow();
        let k = delante.0 as isize;
        let (ultima, era) = CACHE.with(Cell::get);
        let es_explorador = if k == ultima && k != 0 {
            era
        } else {
            let es = DELANTE.contains(&clase(delante).as_str());
            CACHE.with(|c| c.set((k, es)));
            es
        };
        if !es_explorador {
            return None;
        }
        let hilo = GetWindowThreadProcessId(delante, None);
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        GetGUIThreadInfo(hilo, &mut info).ok()?;
        let foco = info.hwndFocus;
        if foco.0.is_null() {
            return None;
        }
        let mut antepasados = Vec::new();
        let mut p = GetParent(foco).unwrap_or_default();
        for _ in 0..8 {
            if p.0.is_null() {
                break;
            }
            antepasados.push(clase(p));
            p = GetParent(p).unwrap_or_default();
        }
        let raiz = GetAncestor(delante, windows::Win32::UI::WindowsAndMessaging::GA_ROOT);
        let delante_clase = clase(if raiz.0.is_null() { delante } else { raiz });
        let mods = pulsada(VK_CONTROL)
            || pulsada(VK_MENU)
            || pulsada(VK_SHIFT)
            || pulsada(VK_LWIN)
            || pulsada(VK_RWIN);
        es_para_la_vista_rapida(mods, &delante_clase, &clase(foco), &antepasados).then_some(delante)
    }
}

extern "system" fn procedimiento(codigo: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if codigo == HC_ACTION as i32 {
        // SAFETY: con HC_ACTION, lparam apunta a un KBDLLHOOKSTRUCT valido
        // durante la llamada (contrato de WH_KEYBOARD_LL).
        let k = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        let nuestra = (k.flags.0 & LLKHF_INJECTED.0) != 0 && k.dwExtraInfo == MARCA_PROPIA;
        if k.vkCode == VK_SPACE.0 as u32 && !nuestra {
            let m = wparam.0 as u32;
            let abajo = m == WM_KEYDOWN || m == WM_SYSKEYDOWN;
            let arriba = m == WM_KEYUP || m == WM_SYSKEYUP;
            if TRAGANDO.load(Ordering::Relaxed) {
                if arriba {
                    TRAGANDO.store(false, Ordering::Relaxed);
                }
                return LRESULT(1);
            }
            if abajo
                && ACTIVO.load(Ordering::Relaxed)
                && let Some(delante) = mirar()
            {
                let enviado = TRABAJO
                    .lock()
                    .ok()
                    .and_then(|t| t.as_ref().map(|t| t.send(delante.0 as isize).is_ok()))
                    .unwrap_or(false);
                if enviado {
                    TRAGANDO.store(true, Ordering::Relaxed);
                    return LRESULT(1);
                }
            }
        }
    }
    // SAFETY: pasar el evento al siguiente gancho, como pide Windows.
    unsafe { CallNextHookEx(None, codigo, wparam, lparam) }
}

/// Devuelve un espacio al Explorador (la app dijo que no era para ella).
fn devolver_espacio() {
    let tecla = |arriba: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_SPACE,
                wScan: 0x39,
                dwFlags: if arriba {
                    KEYEVENTF_KEYUP
                } else {
                    Default::default()
                },
                time: 0,
                dwExtraInfo: MARCA_PROPIA,
            },
        },
    };
    // SAFETY: SendInput con un arreglo local de dos INPUT bien formados.
    unsafe {
        SendInput(
            &[tecla(false), tecla(true)],
            std::mem::size_of::<INPUT>() as i32,
        );
    }
}

/// El gancho instalado (se quita al soltarlo).
pub struct GanchoEspacio {
    hilo: Option<std::thread::JoinHandle<()>>,
    id_hilo: u32,
}

impl GanchoEspacio {
    /// `al_pulsar(delante)` corre en un hilo propio (con COM iniciado) y dice
    /// si el espacio era para la vista rapida; si no, se devuelve.
    pub fn instalar(
        al_pulsar: impl Fn(isize) -> bool + Send + 'static,
    ) -> windows::core::Result<Self> {
        use windows::Win32::System::Threading::GetCurrentThreadId;
        use windows::Win32::UI::WindowsAndMessaging::{GetMessageW, MSG};
        let error = |e: String| windows::core::Error::new(windows::core::HRESULT(-1), e);
        let (tx, rx) = channel::<isize>();
        std::thread::Builder::new()
            .name("vista-rapida".into())
            .spawn(move || trabajar(rx, al_pulsar))
            .map_err(|e| error(e.to_string()))?;
        if let Ok(mut t) = TRABAJO.lock() {
            *t = Some(tx);
        }
        let (listo_tx, listo_rx) = channel::<windows::core::Result<u32>>();
        let hilo = std::thread::Builder::new()
            .name("gancho-espacio".into())
            .spawn(move || {
                // SAFETY: gancho global de teclado con un procedimiento
                // `extern "system"` estatico; se quita en este mismo hilo.
                let gancho: HHOOK = match unsafe {
                    SetWindowsHookExW(
                        windows::Win32::UI::WindowsAndMessaging::WH_KEYBOARD_LL,
                        Some(procedimiento),
                        None,
                        0,
                    )
                } {
                    Ok(g) => g,
                    Err(e) => {
                        let _ = listo_tx.send(Err(e));
                        return;
                    }
                };
                // SAFETY: id del hilo propio.
                let _ = listo_tx.send(Ok(unsafe { GetCurrentThreadId() }));
                let mut msg = MSG::default();
                // SAFETY: bucle de mensajes de este hilo (el gancho de bajo
                // nivel lo necesita para que Windows lo llame).
                while unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 > 0 {}
                // SAFETY: el gancho es de este hilo.
                unsafe {
                    let _ = UnhookWindowsHookEx(gancho);
                }
            })
            .map_err(|e| error(e.to_string()))?;
        let id_hilo = listo_rx.recv().map_err(|e| error(e.to_string()))??;
        Ok(Self {
            hilo: Some(hilo),
            id_hilo,
        })
    }
}

impl Drop for GanchoEspacio {
    fn drop(&mut self) {
        use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT};
        if let Ok(mut t) = TRABAJO.lock() {
            *t = None;
        }
        // SAFETY: WM_QUIT al hilo del gancho, que es nuestro.
        let avisado = unsafe { PostThreadMessageW(self.id_hilo, WM_QUIT, WPARAM(0), LPARAM(0)) };
        if let (Ok(()), Some(hilo)) = (avisado, self.hilo.take()) {
            let _ = hilo.join();
        }
    }
}

fn trabajar(rx: Receiver<isize>, al_pulsar: impl Fn(isize) -> bool) {
    // COM en este hilo, para leer la seleccion del Explorador.
    let _com = crate::explorador::ComDelHilo::iniciar();
    while let Ok(delante) = rx.recv() {
        // Varios espacios seguidos: solo cuenta el ultimo.
        let mut delante = delante;
        while let Ok(otro) = rx.try_recv() {
            delante = otro;
        }
        let t = std::time::Instant::now();
        let era = al_pulsar(delante);
        tracing::info!(
            era,
            ms = t.elapsed().as_millis() as u64,
            "vista rapida: espacio en el Explorador"
        );
        if !era {
            devolver_espacio();
        }
    }
}

/// Las ventanas de este proceso de primer nivel, visibles y sin dueno (las de los
/// visores; sus barras van con ellas).
pub fn ventanas_propias() -> Vec<isize> {
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GW_OWNER, GetWindow, GetWindowThreadProcessId, IsWindowVisible,
    };
    use windows::core::BOOL;
    struct Busca {
        pid: u32,
        v: Vec<isize>,
    }
    extern "system" fn una(h: HWND, l: LPARAM) -> BOOL {
        // SAFETY: `l` apunta a la `Busca` de abajo, viva durante EnumWindows;
        // las demas son consultas de solo lectura.
        unsafe {
            let b = &mut *(l.0 as *mut Busca);
            let mut pid = 0;
            GetWindowThreadProcessId(h, Some(&mut pid));
            if pid == b.pid
                && IsWindowVisible(h).as_bool()
                && GetWindow(h, GW_OWNER).unwrap_or_default().0.is_null()
            {
                b.v.push(h.0 as isize);
            }
        }
        BOOL(1)
    }
    let mut b = Busca {
        pid: std::process::id(),
        v: Vec::new(),
    };
    // SAFETY: EnumWindows llama a `una` en este hilo, con el puntero a `b`.
    let _ = unsafe { EnumWindows(Some(una), LPARAM(&mut b as *mut Busca as isize)) };
    b.v
}

pub fn viva(h: isize) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{IsWindow, IsWindowVisible};
    let h = HWND(h as *mut _);
    // SAFETY: consultas de solo lectura; una ventana que ya no existe da false.
    unsafe { IsWindow(Some(h)).as_bool() && IsWindowVisible(h).as_bool() }
}

pub fn cerrar(ventanas: &[isize]) {
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};
    for &h in ventanas {
        // SAFETY: WM_CLOSE a una ventana de este proceso (la abrio la vista
        // rapida); cada visor lo atiende como su boton de cerrar.
        let _ = unsafe { PostMessageW(Some(HWND(h as *mut _)), WM_CLOSE, WPARAM(0), LPARAM(0)) };
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn a(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn el_espacio_es_de_la_vista_rapida_solo_en_la_lista_de_ficheros() {
        let lista = a(&["SHELLDLL_DefView", "DUIViewWndClassName", "CabinetWClass"]);
        assert!(es_para_la_vista_rapida(
            false,
            "CabinetWClass",
            "DirectUIHWND",
            &lista
        ));
        // El escritorio.
        assert!(es_para_la_vista_rapida(
            false,
            "Progman",
            "SysListView32",
            &a(&["SHELLDLL_DefView", "Progman"])
        ));
        // Casos negativos: cambiando el nombre (un Edit), con Ctrl, en el
        // buscador o la barra de direcciones, o en otro programa.
        assert!(!es_para_la_vista_rapida(
            false,
            "CabinetWClass",
            "Edit",
            &lista
        ));
        assert!(!es_para_la_vista_rapida(
            true,
            "CabinetWClass",
            "DirectUIHWND",
            &lista
        ));
        assert!(!es_para_la_vista_rapida(
            false,
            "CabinetWClass",
            "DirectUIHWND",
            &a(&["CtrlNotifySink", "CabinetWClass"])
        ));
        assert!(!es_para_la_vista_rapida(
            false,
            "OpusApp",
            "DirectUIHWND",
            &lista
        ));
    }
}
