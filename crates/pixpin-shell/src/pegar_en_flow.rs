//! **Ctrl+V de una imagen en Flow Launcher** (2026-10-03).
//!
//! La caja de Flow es texto de WPF: Ctrl+V con una imagen en el portapapeles
//! no pega nada, y el plugin (`pixpin-lanzador`) no ve las teclas. PixPin
//! esta siempre abierta, asi que lo hace ella: un gancho de teclado de bajo
//! nivel (`WH_KEYBOARD_LL`) ve el Ctrl+V antes que Flow y, si toca, se lo
//! traga, y en otro hilo la app guarda la imagen y escribe ` [img 01] ` en
//! la caja con `SendInput` (letras Unicode), como si se tecleara.
//!
//! Toca solo si TODO esto se cumple ([`interceptar`]): es la V con Ctrl (sin
//! Alt ni Win), la ventana de delante es de `Flow.Launcher.exe`, y el
//! portapapeles tiene una imagen (un mapa de bits o un PNG) o ficheros
//! copiados en el Explorador (4-oct: la imagen va como `[img NN]` y otro
//! fichero como `[archivo NN]`, adjunto del mensaje del chat) y NO tiene
//! texto: con texto, Flow pega el texto como siempre.
//!
//! # Lo que cuesta
//!
//! Cada tecla del sistema pasa por aqui, asi que el procedimiento es lo
//! minimo: si no es la V (casi todas) sale en la primera comparacion. Con la
//! V mira los modificadores (`GetAsyncKeyState`); solo con Ctrl+V mira que
//! programa esta delante (cacheado por ventana: el nombre del ejecutable se
//! pregunta una vez por ventana), y solo con Flow delante mira el
//! portapapeles (`IsClipboardFormatAvailable`, que no lo abre; un fichero
//! copiado si obliga a abrirlo un momento). Guardar la imagen y escribir van
//! en otro hilo, nunca dentro del gancho.
//!
//! # Por que en su propio hilo
//!
//! Como el gancho del raton ([`crate::gestos::GanchoRaton`]): un gancho de
//! bajo nivel corre en el hilo que lo instalo, y si ese hilo tarda Windows lo
//! retira sin avisar. En un hilo que solo bombea sus mensajes no tarda nunca.

use std::cell::Cell;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, SendInput, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetForegroundWindow, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, SetWindowsHookExW,
    UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

/// El ejecutable de Flow Launcher, en minusculas.
pub const EXE_FLOW: &str = "flow.launcher.exe";
/// La tecla V.
pub const VK_V: u32 = 0x56;

/// Una pulsacion, lo que el gancho sabe sin preguntar nada caro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pulsacion {
    pub vk: u32,
    pub ctrl: bool,
    pub alt: bool,
    pub win: bool,
}

/// Lo que dice el portapapeles: `(hay imagen o ficheros, hay texto)`.
pub type Contenido = (bool, bool);

/// **¿Se traga esta pulsacion?** Pura: lo caro llega como funciones que solo
/// se llaman si hacen falta, en este orden (y las pruebas lo comprueban):
/// el programa de delante solo con Ctrl+V, y el portapapeles solo con Flow
/// delante.
pub fn interceptar(
    p: Pulsacion,
    programa_delante: impl FnOnce() -> Option<String>,
    portapapeles: impl FnOnce() -> Contenido,
) -> bool {
    if p.vk != VK_V || !p.ctrl || p.alt || p.win {
        return false;
    }
    if !programa_delante().is_some_and(|e| e.eq_ignore_ascii_case(EXE_FLOW)) {
        return false;
    }
    let (imagen, texto) = portapapeles();
    imagen && !texto
}

// ------------------------------------------------------------------ el gancho

/// Lo que se manda al hilo de trabajo: la ventana de Flow que tenia el foco.
static TRABAJO: Mutex<Option<Sender<isize>>> = Mutex::new(None);
/// La V de un Ctrl+V tragado sigue abajo: sus repeticiones y su soltada
/// tambien se tragan (Flow no vio la pulsacion; media tecla no le sirve).
static TRAGANDO_V: AtomicBool = AtomicBool::new(false);
/// Marca de lo que escribe este modulo con `SendInput`.
const MARCA_PROPIA: usize = 0x5049_5856; // "PIXV"

thread_local! {
    /// La ultima ventana mirada y si era de Flow: el nombre del ejecutable
    /// se pregunta una vez por ventana. Solo la toca el hilo del gancho.
    static CACHE: Cell<(isize, bool)> = const { Cell::new((0, false)) };
}

fn pulsada(tecla: VIRTUAL_KEY) -> bool {
    // SAFETY: consulta del estado del teclado, sin precondiciones.
    (unsafe { GetAsyncKeyState(tecla.0 as i32) } as u16 & 0x8000) != 0
}

/// Si la ventana es de Flow, con la cache.
fn es_flow(hwnd: HWND) -> bool {
    let clave = hwnd.0 as isize;
    let (ultima, era) = CACHE.with(Cell::get);
    if clave == ultima && clave != 0 {
        return era;
    }
    let es = crate::primer_plano::programa_de(hwnd).is_some_and(|e| e == EXE_FLOW);
    CACHE.with(|c| c.set((clave, es)));
    es
}

/// Que hay en el portapapeles, sin abrirlo salvo para un fichero copiado.
fn contenido() -> Contenido {
    use windows::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
        RegisterClipboardFormatW,
    };
    use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};
    use windows::core::w;
    const CF_UNICODETEXT: u32 = 13;
    const CF_DIB: u32 = 8;
    const CF_HDROP: u32 = 15;
    const CF_DIBV5: u32 = 17;
    // SAFETY: consultas sin precondiciones; un formato 0 no se pregunta.
    let hay = |f: u32| f != 0 && unsafe { IsClipboardFormatAvailable(f) }.is_ok();
    let texto = hay(CF_UNICODETEXT);
    if texto {
        return (false, true);
    }
    // SAFETY: registrar un nombre ya registrado solo devuelve su numero.
    let png = unsafe {
        [
            RegisterClipboardFormatW(w!("PNG")),
            RegisterClipboardFormatW(w!("image/png")),
        ]
    };
    if [CF_DIBV5, CF_DIB, png[0], png[1]].into_iter().any(hay) {
        return (true, false);
    }
    if !hay(CF_HDROP) {
        return (false, false);
    }
    // Ficheros copiados (en el Explorador): cualquiera vale desde el 4-oct
    // (una imagen va como `[img NN]`, otro fichero como `[archivo NN]`, al
    // chat como adjunto). Hay que abrir para contarlos.
    // SAFETY: se abre sin ventana y se cierra en el mismo bloque; el HDROP
    // es del portapapeles y solo se lee mientras esta abierto.
    let imagen = unsafe {
        if OpenClipboard(None).is_err() {
            return (false, false);
        }
        let hay_ficheros = GetClipboardData(CF_HDROP)
            .is_ok_and(|h| DragQueryFileW(HDROP(h.0), u32::MAX, None) > 0);
        let _ = CloseClipboard();
        hay_ficheros
    };
    (imagen, false)
}

extern "system" fn procedimiento(codigo: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if codigo == HC_ACTION as i32 {
        // SAFETY: con HC_ACTION, lparam apunta a un KBDLLHOOKSTRUCT valido
        // durante la llamada.
        let k = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        if k.vkCode == VK_V {
            let m = wparam.0 as u32;
            let abajo = m == WM_KEYDOWN || m == WM_SYSKEYDOWN;
            let arriba = m == WM_KEYUP || m == WM_SYSKEYUP;
            if TRAGANDO_V.load(Ordering::Relaxed) {
                if arriba {
                    TRAGANDO_V.store(false, Ordering::Relaxed);
                }
                return LRESULT(1);
            }
            if abajo {
                let p = Pulsacion {
                    vk: VK_V,
                    ctrl: pulsada(VK_CONTROL),
                    alt: pulsada(VK_MENU),
                    win: pulsada(VK_LWIN) || pulsada(VK_RWIN),
                };
                // SAFETY: sin precondiciones; puede ser nula.
                let delante = unsafe { GetForegroundWindow() };
                let si = interceptar(
                    p,
                    || es_flow(delante).then(|| EXE_FLOW.to_string()),
                    contenido,
                );
                if si {
                    let enviado = TRABAJO
                        .lock()
                        .ok()
                        .and_then(|t| t.as_ref().map(|t| t.send(delante.0 as isize).is_ok()))
                        .unwrap_or(false);
                    if enviado {
                        TRAGANDO_V.store(true, Ordering::Relaxed);
                        return LRESULT(1);
                    }
                }
            }
        }
    }
    // SAFETY: pasar el evento al siguiente gancho es obligatorio.
    unsafe { CallNextHookEx(None, codigo, wparam, lparam) }
}

/// El gancho de Ctrl+V en Flow, vivo mientras viva el valor.
pub struct GanchoPegarEnFlow {
    hilo: Option<std::thread::JoinHandle<()>>,
    id_hilo: u32,
}

impl GanchoPegarEnFlow {
    /// Instala el gancho. `al_pegar` corre en un hilo de trabajo cada vez
    /// que se traga un Ctrl+V: guarda la imagen y devuelve lo que hay que
    /// escribir en Flow (` [img 01] `), o `None` para no escribir nada (la
    /// imagen ya estaba: una imagen, un nombre).
    pub fn instalar(
        al_pegar: impl Fn() -> Option<String> + Send + 'static,
    ) -> windows::core::Result<Self> {
        use windows::Win32::System::Threading::GetCurrentThreadId;
        use windows::Win32::UI::WindowsAndMessaging::{GetMessageW, MSG};

        let error = |e: String| windows::core::Error::new(windows::core::HRESULT(-1), e);
        let (tx, rx) = channel::<isize>();
        std::thread::Builder::new()
            .name("pegar-en-flow".into())
            .spawn(move || trabajar(rx, al_pegar))
            .map_err(|e| error(e.to_string()))?;
        if let Ok(mut t) = TRABAJO.lock() {
            *t = Some(tx);
        }
        let (listo_tx, listo_rx) = channel::<windows::core::Result<u32>>();
        let hilo = std::thread::Builder::new()
            .name("gancho-teclado".into())
            .spawn(move || {
                // SAFETY: gancho global de bajo nivel con un procedimiento de
                // este modulo; corre en ESTE hilo, que bombea justo debajo.
                let gancho: HHOOK = match unsafe {
                    SetWindowsHookExW(WH_KEYBOARD_LL, Some(procedimiento), None, 0)
                } {
                    Ok(g) => g,
                    Err(e) => {
                        let _ = listo_tx.send(Err(e));
                        return;
                    }
                };
                // SAFETY: consulta del identificador del hilo actual.
                let _ = listo_tx.send(Ok(unsafe { GetCurrentThreadId() }));
                let mut msg = MSG::default();
                // SAFETY: GetMessageW escribe en un MSG propio; sale con el
                // WM_QUIT de `Drop`.
                while unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 > 0 {}
                // SAFETY: desinstala el gancho que instalo este mismo hilo.
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

impl Drop for GanchoPegarEnFlow {
    fn drop(&mut self) {
        use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT};
        // Sin emisor, el hilo de trabajo acaba solo.
        if let Ok(mut t) = TRABAJO.lock() {
            *t = None;
        }
        // SAFETY: publicar WM_QUIT en la cola del hilo del gancho.
        let avisado = unsafe { PostThreadMessageW(self.id_hilo, WM_QUIT, WPARAM(0), LPARAM(0)) };
        if let (Ok(()), Some(hilo)) = (avisado, self.hilo.take()) {
            let _ = hilo.join();
        }
    }
}

/// El hilo de trabajo: por cada Ctrl+V tragado, la app guarda la imagen y
/// lo que devuelva se escribe en Flow.
fn trabajar(rx: Receiver<isize>, al_pegar: impl Fn() -> Option<String>) {
    while let Ok(ventana) = rx.recv() {
        let Some(texto) = al_pegar() else { continue };
        // Con Ctrl aun abajo, Flow tomaria las letras por atajos: se espera a
        // que se suelte (es lo normal en un instante) y, si no, se suelta.
        let inicio = Instant::now();
        while pulsada(VK_CONTROL) && inicio.elapsed() < Duration::from_millis(1500) {
            std::thread::sleep(Duration::from_millis(10));
        }
        // SAFETY: sin precondiciones.
        let delante = unsafe { GetForegroundWindow() };
        if delante.0 as isize != ventana {
            tracing::info!("pegar en Flow: Flow ya no esta delante; no se escribe");
            continue;
        }
        if pulsada(VK_CONTROL) {
            enviar(&[tecla(VK_CONTROL, true)]);
        }
        escribir(&texto);
    }
}

fn tecla(vk: VIRTUAL_KEY, arriba: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if arriba {
                    KEYEVENTF_KEYUP
                } else {
                    Default::default()
                },
                time: 0,
                dwExtraInfo: MARCA_PROPIA,
            },
        },
    }
}

fn enviar(entradas: &[INPUT]) {
    // SAFETY: entradas locales y bien formadas; el tamano es el de INPUT.
    unsafe {
        SendInput(entradas, std::mem::size_of::<INPUT>() as i32);
    }
}

/// Escribe `texto` en la ventana de delante, letra a letra (Unicode: vale
/// cualquier distribucion de teclado).
pub fn escribir(texto: &str) {
    let mut v = Vec::new();
    for u in texto.encode_utf16() {
        for arriba in [false, true] {
            v.push(INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(0),
                        wScan: u,
                        dwFlags: if arriba {
                            KEYEVENTF_UNICODE | KEYEVENTF_KEYUP
                        } else {
                            KEYEVENTF_UNICODE
                        },
                        time: 0,
                        dwExtraInfo: MARCA_PROPIA,
                    },
                },
            });
        }
    }
    enviar(&v);
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::cell::Cell;

    fn ctrl_v() -> Pulsacion {
        Pulsacion {
            vk: VK_V,
            ctrl: true,
            alt: false,
            win: false,
        }
    }

    /// La tabla de verdad, contando a quien se pregunta.
    fn decide(p: Pulsacion, exe: Option<&str>, imagen: bool, texto: bool) -> (bool, u32, u32) {
        let (pregunta_exe, pregunta_pp) = (Cell::new(0), Cell::new(0));
        let si = interceptar(
            p,
            || {
                pregunta_exe.set(pregunta_exe.get() + 1);
                exe.map(str::to_string)
            },
            || {
                pregunta_pp.set(pregunta_pp.get() + 1);
                (imagen, texto)
            },
        );
        (si, pregunta_exe.get(), pregunta_pp.get())
    }

    #[test]
    fn ctrl_v_en_flow_con_una_imagen_y_sin_texto_se_traga() {
        assert_eq!(
            decide(ctrl_v(), Some("flow.launcher.exe"), true, false),
            (true, 1, 1)
        );
        // El nombre, sin distinguir mayusculas.
        assert!(decide(ctrl_v(), Some("Flow.Launcher.exe"), true, false).0);
    }

    #[test]
    fn caso_negativo_la_tabla_de_verdad_de_lo_que_no_se_traga() {
        let flow = Some("flow.launcher.exe");
        // (pulsacion, programa, imagen, texto) -> (traga, preguntas al
        // programa, preguntas al portapapeles)
        let casos = [
            // Con texto en el portapapeles, Flow pega el texto.
            (ctrl_v(), flow, true, true, (false, 1, 1)),
            // Sin imagen, nada que hacer.
            (ctrl_v(), flow, false, false, (false, 1, 1)),
            (ctrl_v(), flow, false, true, (false, 1, 1)),
            // Otro programa delante: ni se mira el portapapeles.
            (ctrl_v(), Some("notepad.exe"), true, false, (false, 1, 0)),
            (ctrl_v(), None, true, false, (false, 1, 0)),
            // Otra tecla, o la V con otros modificadores: no se pregunta nada.
            (
                Pulsacion {
                    vk: 0x43,
                    ..ctrl_v()
                },
                flow,
                true,
                false,
                (false, 0, 0),
            ),
            (
                Pulsacion {
                    ctrl: false,
                    ..ctrl_v()
                },
                flow,
                true,
                false,
                (false, 0, 0),
            ),
            (
                Pulsacion {
                    alt: true,
                    ..ctrl_v()
                },
                flow,
                true,
                false,
                (false, 0, 0),
            ),
            (
                Pulsacion {
                    win: true,
                    ..ctrl_v()
                },
                flow,
                true,
                false,
                (false, 0, 0),
            ),
        ];
        for (p, exe, imagen, texto, esperado) in casos {
            assert_eq!(
                decide(p, exe, imagen, texto),
                esperado,
                "{p:?} {exe:?} imagen={imagen} texto={texto}"
            );
        }
    }
}
