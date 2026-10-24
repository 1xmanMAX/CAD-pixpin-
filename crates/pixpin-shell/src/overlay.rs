//! La ventana de overlay: una por monitor, clase y WndProc PROPIOS.
//!
//! La retrospectiva de S1-A lo exige: el WndProc de VentanaMensajes llama a
//! PostQuitMessage ante cualquier WM_DESTROY. Si los overlays lo
//! compartieran, cerrar uno mataria la aplicacion. Este WndProc no llama a
//! PostQuitMessage NUNCA: el bucle modal termina porque el callback lo dice,
//! no porque una ventana muera.
//!
//! WS_EX_NOREDIRECTIONBITMAP: la ventana no tiene superficie GDI; todo lo
//! visible lo presenta la Superficie de pixpin-render por DirectComposition.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::sync::Once;

use pixpin_geom::{Punto, Rect};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{InvalidateRect, ValidateRect};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
// En windows 0.62, AttachThreadInput vive en System::Threading.
use windows::Win32::System::Threading::AttachThreadInput;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, SetFocus, VK_CONTROL, VK_MENU, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

/// WM_APP+1: otros hilos lo PostMessage-an para despertar el bucle modal.
pub const MSG_DESPIERTA: u32 = WM_APP + 1;

/// Despierta a la ventana de otro hilo: le llega `EventoOverlay::Despierta`.
/// Recibe el HWND como entero para poder cruzar hilos (`HWND` no es `Send`).
pub fn despertar(hwnd: isize) {
    // SAFETY: PostMessageW no toca memoria nuestra; si la ventana ya no
    // existe, falla y se ignora, que es lo correcto para un aviso.
    unsafe {
        let _ = PostMessageW(
            Some(HWND(hwnd as *mut _)),
            MSG_DESPIERTA,
            WPARAM(0),
            LPARAM(0),
        );
    }
}

/// Si la ventana de otro hilo sigue viva (aunque este minimizada).
pub fn existe(hwnd: isize) -> bool {
    // SAFETY: consulta de solo lectura; un handle muerto responde «no».
    unsafe { IsWindow(Some(HWND(hwnd as *mut _))).as_bool() }
}

/// **Lo de dentro de una ventana, en pixeles de pantalla**: `hwnd`, o la
/// activa de este hilo si es `None`. Devuelve tambien cual era. Lo usa el
/// marco para imprimir de los visores.
pub fn area_cliente(hwnd: Option<isize>) -> Option<(isize, Rect)> {
    use windows::Win32::Foundation::{POINT, RECT};
    use windows::Win32::Graphics::Gdi::ClientToScreen;
    use windows::Win32::UI::Input::KeyboardAndMouse::GetActiveWindow;
    use windows::Win32::UI::WindowsAndMessaging::GetClientRect;
    // SAFETY: consultas de solo lectura de una ventana; las estructuras son
    // locales y un handle muerto solo hace fallar la llamada.
    unsafe {
        let h = match hwnd {
            Some(h) if h != 0 => HWND(h as *mut _),
            _ => GetActiveWindow(),
        };
        let mut r = RECT::default();
        if h.is_invalid() || GetClientRect(h, &mut r).is_err() {
            return None;
        }
        let mut esquina = POINT { x: 0, y: 0 };
        let _ = ClientToScreen(h, &mut esquina);
        Some((
            h.0 as isize,
            Rect {
                x: esquina.x,
                y: esquina.y,
                ancho: (r.right - r.left).max(1) as u32,
                alto: (r.bottom - r.top).max(1) as u32,
            },
        ))
    }
}

/// **Un clic de mentira para las pruebas**: manda a `hwnd` (una ventana
/// PROPIA) pulsar (0), mover con el boton pulsado (1) o soltar (2) en `x, y`
/// de su zona cliente, como si fuera el raton. No mueve el raton de verdad.
pub fn simular_raton(hwnd: isize, que: u8, x: i32, y: i32) {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        PostMessageW, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
    };
    let mensaje = match que {
        0 => WM_LBUTTONDOWN,
        1 => WM_MOUSEMOVE,
        _ => WM_LBUTTONUP,
    };
    let lparam = ((y as u16 as isize) << 16) | (x as u16 as isize);
    let boton = if que == 2 { 0 } else { 1 };
    // SAFETY: solo deja un mensaje en la cola de esa ventana; un handle
    // muerto hace fallar la llamada sin mas.
    unsafe {
        let _ = PostMessageW(
            Some(HWND(hwnd as *mut _)),
            mensaje,
            WPARAM(boton),
            LPARAM(lparam),
        );
    }
}

/// Espera a que el compositor haya presentado lo ultimo que se dibujo.
/// Dos vueltas: la primera cierra el fotograma en curso, la segunda
/// garantiza que el nuestro ya esta en pantalla y, por tanto, en la
/// captura que venga despues (D59).
pub fn esperar_composicion() {
    use windows::Win32::Graphics::Dwm::DwmFlush;
    // SAFETY: sin precondiciones; un fallo (sin DWM) solo significa no
    // esperar.
    unsafe {
        let _ = DwmFlush();
        let _ = DwmFlush();
    }
}

thread_local! {
    /// La mitad alta de un par subrogado UTF-16 a la espera de su mitad
    /// baja (WM_CHAR entrega los dos por separado).
    static MITAD_ALTA: Cell<Option<u16>> = const { Cell::new(None) };
    /// Lo ultimo que se solto encima, a la espera de que lo recojan. Va
    /// aparte de la cola de eventos porque son rutas y el evento es `Copy`.
    static SOLTADOS: std::cell::RefCell<Vec<std::path::PathBuf>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Recoge las rutas de lo ultimo que se solto, y deja el sitio vacio.
///
/// Se llama tras recibir `EventoOverlay::FicherosSoltados`. Si no se recoge,
/// lo siguiente que se suelte se anade detras: nada se pierde en silencio.
pub fn ficheros_soltados() -> Vec<std::path::PathBuf> {
    SOLTADOS.with(|s| std::mem::take(&mut *s.borrow_mut()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventoOverlay {
    RatonMovido(Punto),
    BotonPulsado(Punto),
    BotonSoltado(Punto),
    /// Boton central (WM_MBUTTONDOWN / WM_MBUTTONUP). El editor lo usa para
    /// desplazar el lienzo como Excalidraw (D136). Escritorio virtual, como
    /// `BotonPulsado`.
    BotonCentralPulsado(Punto),
    /// Boton derecho. Va aparte del izquierdo porque el chat abre con el su
    /// menu contextual; la captura, en cambio, lo trata igual que el
    /// izquierdo (el gesto de Alt + derecho arranca una seleccion).
    BotonDerechoPulsado(Punto),
    BotonCentralSoltado(Punto),
    Tecla {
        vk: u32,
        shift: bool,
        /// Ctrl mantenido: para `Ctrl+A` (seleccionar todo) y `Ctrl+Z`.
        ctrl: bool,
        /// Alt mantenido: en el editor, escalar desde el centro en vez de
        /// desde el ancla.
        alt: bool,
    },
    Pintar,
    CambioDpi,
    /// MSG_DESPIERTA recibido: hay trabajo de otro hilo esperando.
    Despierta,
    /// WM_CLOSE (Alt+F4): el usuario quiere cerrar; tratar como cancelar.
    Cerrar,
    /// Rueda del raton, positivo hacia arriba. La capa viva la usa para el
    /// grosor del trazo y el aumento de la lupa (D55).
    Rueda(i32),
    /// El giro HORIZONTAL (`WM_MOUSEHWHEEL`), positivo hacia la derecha. Es
    /// lo que manda un panel tactil al mover dos dedos de lado: Windows
    /// parte ese gesto en dos mensajes, uno por eje, y sin este el lienzo
    /// solo se dejaba llevar arriba y abajo.
    RuedaHorizontal(i32),
    /// Un caracter escrito, ya compuesto (WM_CHAR, IME incluido) (D57).
    Caracter(char),
    /// Se han soltado ficheros encima (WM_DROPFILES). Las rutas se recogen
    /// con `ficheros_soltados`, no van aqui: este enum es `Copy` y meterle
    /// un `Vec` obligaria a clonar en cada evento de raton.
    FicherosSoltados,
    /// Una tecla soltada (WM_KEYUP). La capa viva lo usa para el pasante
    /// temporal con Ctrl mantenido (D50).
    TeclaSoltada(u32),
    /// Un atajo global pulsado MIENTRAS el overlay esta abierto. Llega aqui
    /// en vez de quedarse en la cola de la ventana principal, donde se
    /// atenderia al cerrar y volveria a abrir el overlay.
    Atajo(u32),
    /// Un punto del trazo que no es el del `WM_MOUSEMOVE`: uno que Windows
    /// fusiono, o uno del lapiz con presion. Llega ANTES del `RatonMovido`
    /// al que precede, y solo a ventanas que llamaron a
    /// `pedir_entrada_fina`. Coordenadas del escritorio virtual.
    Muestra(crate::puntero::Muestra),
    /// La rueda con su hora y sus modificadores, en vez de `Rueda` y
    /// `RuedaHorizontal`, SOLO para las ventanas que llamaron a
    /// `pedir_gestos_tactiles`: quien la recibe decide si es un raton o un
    /// panel tactil viejo (`gestos_tactiles::RuedaCruda`).
    RuedaFina(crate::gestos_tactiles::RuedaCruda),
    /// Dos dedos en el panel tactil de precision (DirectManipulation), solo
    /// para las ventanas que llamaron a `pedir_gestos_tactiles`.
    DeslizTactil(crate::gestos_tactiles::Desliz),
    /// Pellizco en el panel tactil de precision, idem.
    PellizcoTactil(crate::gestos_tactiles::Pellizco),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormaCursorWin {
    Cruz,
    Mover,
    RedimNS,
    RedimEO,
    RedimNeSo,
    RedimNoSe,
    /// La barra vertical de escribir (herramienta de texto).
    Texto,
    /// La flecha normal (seleccionar / deshacer).
    Flecha,
    /// Girar lo seleccionado.
    ///
    /// Windows no trae cursor de giro: los ocho `IDC_*` estandar son
    /// flechas, cruz, barra de texto y poco mas. Se usa `IDC_HAND` porque
    /// al menos se distingue de los de redimension y no miente sobre lo
    /// que va a pasar. Un cursor de giro de verdad necesitaria un recurso
    /// propio, y eso no entra en esta entrega.
    Giro,
}

#[derive(Debug, thiserror::Error)]
pub enum ErrorOverlay {
    #[error("no se pudo crear la ventana de overlay: {0}")]
    Creacion(#[source] windows::core::Error),
}

thread_local! {
    /// Cola de eventos del hilo de interfaz. Solo la toca el WndProc (que
    /// corre en este hilo) y bucle_modal. Los hilos ajenos usan PostMessage.
    static PENDIENTES_OVERLAY: RefCell<VecDeque<(HWND, EventoOverlay)>> =
        const { RefCell::new(VecDeque::new()) };
    /// Cursor vigente por ventana; WM_SETCURSOR lo consulta.
    static CURSOR: RefCell<Vec<(HWND, FormaCursorWin)>> = const { RefCell::new(Vec::new()) };
    /// Hueco por ventana, en coordenadas del escritorio virtual. Los clics
    /// que caen dentro atraviesan la ventana y llegan a lo que hay debajo.
    static HUECO: RefCell<Vec<(HWND, Rect)>> = const { RefCell::new(Vec::new()) };
    /// Ventanas que quieren todos los puntos: su historial de raton y el
    /// estado del ultimo WM_POINTERUPDATE de lapiz (el tercer campo). Sin
    /// ese tercer campo, un WM_MOUSEMOVE sintetizado por el tacto (misma
    /// firma que el lapiz salvo un bit), por un lapiz cuyo
    /// GetPointerPenInfoHistory fallo, o por el borrador del lapiz (D107) no
    /// se distinguiria de uno que el camino del lapiz ya cubrio de verdad.
    /// Ventanas que pidieron gestos tactiles, con su DirectManipulation si
    /// Windows lo dio (`None`: solo la rueda con hora, `RuedaFina`).
    static GESTOS: RefCell<Vec<(HWND, Option<crate::gestos_tactiles::GestosTactiles>)>> =
        const { RefCell::new(Vec::new()) };
    static ENTRADA_FINA: RefCell<Vec<(HWND, crate::puntero::HistorialRaton, crate::puntero::EstadoLapiz)>> =
        const { RefCell::new(Vec::new()) };
}

static REGISTRO: Once = Once::new();

pub struct VentanaOverlay {
    hwnd: HWND,
    area: Rect,
}

impl VentanaOverlay {
    pub fn nueva(area: Rect) -> Result<Self, ErrorOverlay> {
        REGISTRO.call_once(registrar_clase);
        // SAFETY: la clase quedo registrada en call_once; los estilos son
        // constantes documentadas y el modulo es el propio.
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOOLWINDOW,
                w!("PixPinOverlay"),
                w!(""),
                WS_POPUP,
                area.x,
                area.y,
                area.ancho as i32,
                area.alto as i32,
                None,
                None,
                Some(
                    GetModuleHandleW(None)
                        .map_err(ErrorOverlay::Creacion)?
                        .into(),
                ),
                None,
            )
            .map_err(ErrorOverlay::Creacion)?
        };
        Ok(Self { hwnd, area })
    }

    /// Una ventana de aplicacion corriente, con la misma clase y el mismo
    /// bombeo que el overlay, pero SIN estar siempre encima y con su boton
    /// en la barra de tareas (D141).
    ///
    /// Es la de la ventana de chat, que es la interfaz principal: el usuario
    /// la quiere como cualquier otra ventana, que se tapa al cambiar de
    /// programa. `WS_MINIMIZEBOX | WS_SYSMENU` son los que dejan minimizarla
    /// y recuperarla pulsando su boton de la barra de tareas; sin ellos una
    /// `WS_POPUP` no responde a ese clic.
    pub fn nueva_normal(area: Rect, titulo: &str) -> Result<Self, ErrorOverlay> {
        REGISTRO.call_once(registrar_clase);
        let titulo: Vec<u16> = titulo.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: la clase quedo registrada en call_once; el titulo es una
        // cadena terminada en cero que vive hasta despues de la llamada.
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_NOREDIRECTIONBITMAP | WS_EX_APPWINDOW,
                w!("PixPinOverlay"),
                windows::core::PCWSTR(titulo.as_ptr()),
                WS_POPUP | WS_MINIMIZEBOX | WS_SYSMENU,
                area.x,
                area.y,
                area.ancho as i32,
                area.alto as i32,
                None,
                None,
                Some(
                    GetModuleHandleW(None)
                        .map_err(ErrorOverlay::Creacion)?
                        .into(),
                ),
                None,
            )
            .map_err(ErrorOverlay::Creacion)?
        };
        Ok(Self { hwnd, area })
    }

    /// Como `nueva_normal`, pero SIN `WS_EX_NOREDIRECTIONBITMAP`.
    ///
    /// Esa bandera le dice a Windows que la ventana no tiene superficie de
    /// redibujado porque la pinta DirectComposition: es lo correcto para
    /// todo lo que dibuja PixPin con Direct2D, y lo que hace que no parpadee.
    /// Pero una ventana asi es mal sitio para meter dentro un control de
    /// Windows —un WebView2, por ejemplo—: el control tiene que componer por
    /// su cuenta contra una ventana que no le ofrece donde, y se nota en el
    /// desplazamiento. Para esos casos, esta.
    pub fn nueva_llana(area: Rect, titulo: &str) -> Result<Self, ErrorOverlay> {
        REGISTRO.call_once(registrar_clase);
        let titulo: Vec<u16> = titulo.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: la clase quedo registrada en call_once; el titulo es una
        // cadena terminada en cero que vive hasta despues de la llamada.
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_APPWINDOW,
                w!("PixPinOverlay"),
                windows::core::PCWSTR(titulo.as_ptr()),
                WS_POPUP | WS_MINIMIZEBOX | WS_SYSMENU,
                area.x,
                area.y,
                area.ancho as i32,
                area.alto as i32,
                None,
                None,
                Some(
                    GetModuleHandleW(None)
                        .map_err(ErrorOverlay::Creacion)?
                        .into(),
                ),
                None,
            )
            .map_err(ErrorOverlay::Creacion)?
        };
        Ok(Self { hwnd, area })
    }

    /// Si esta minimizada, la devuelve a su sitio; y la trae al frente. Es
    /// lo que hace pedir la ventana de chat cuando ya esta abierta.
    pub fn restaurar_de_hwnd(hwnd: HWND) {
        // SAFETY: consultas y cambios de estado sobre una ventana de este
        // proceso; si ya no existe, las llamadas fallan sin efecto.
        unsafe {
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            let _ = SetForegroundWindow(hwnd);
        }
    }

    /// Lleva la ventana a otro sitio y otro tamano sin rehacerla.
    ///
    /// El marco de la grabacion se mueve y se estira mientras el usuario
    /// arrastra: recrear la ventana y su superficie en cada paso costaria
    /// mas que el arrastre entero.
    pub fn mover(&mut self, area: Rect) {
        // SAFETY: SetWindowPos sobre la ventana propia, viva mientras
        // exista este objeto. SWP_NOACTIVATE mantiene el foco donde este:
        // el usuario sigue trabajando con la aplicacion que va a grabar.
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                None,
                area.x,
                area.y,
                area.ancho as i32,
                area.alto as i32,
                SWP_NOACTIVATE | SWP_NOZORDER,
            );
        }
        self.area = area;
    }

    /// **Siempre encima, o como una ventana mas** (el pin de los lectores):
    /// con `false` deja de estar por encima de las demas aplicaciones.
    pub fn poner_siempre_encima(&self, si: bool) {
        // SAFETY: SetWindowPos sobre la ventana propia y viva; solo el orden Z.
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                Some(if si { HWND_TOPMOST } else { HWND_NOTOPMOST }),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }

    /// **Al frente, pero como una ventana mas** (un visor de archivos al
    /// nacer, 13-oct-2026): delante de las aplicaciones normales y sin
    /// «siempre encima», sin activarla. `HWND_NOTOPMOST` solo no basta: en
    /// una ventana que no es TOPMOST no hace nada, y si `enfocar` no pudo
    /// robar el primer plano (Windows a veces no deja) la ventana nueva
    /// quedaria debajo de la que se estaba usando. Subirla a TOPMOST y
    /// bajarla en seguida la deja la primera de las normales.
    pub fn al_frente_sin_fijar(&self) {
        self.poner_siempre_encima(true);
        self.poner_siempre_encima(false);
    }

    /// Si esta «siempre encima» ahora (`WS_EX_TOPMOST`).
    pub fn siempre_encima(&self) -> bool {
        self.estilo_extendido() & WS_EX_TOPMOST.0 != 0
    }

    /// La pone por delante de las demas ventanas TOPMOST sin activarla ni
    /// moverla (D145). Crearla despues suele bastar, pero un pin que se
    /// reordene (su paleta, un clic) quedaria encima del lienzo.
    pub fn traer_encima(&self) {
        // SAFETY: SetWindowPos sobre la ventana propia y viva; solo cambia el
        // orden Z (sin mover, sin redimensionar, sin robar el foco).
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }

    pub fn handle(&self) -> HWND {
        self.hwnd
    }

    pub fn area(&self) -> Rect {
        self.area
    }

    /// El rectangulo que la ventana ocupa DE VERDAD ahora mismo, preguntado
    /// a Windows. `None` si no se puede preguntar.
    ///
    /// `area()` devuelve lo ULTIMO que pidio la aplicacion, que no siempre es
    /// lo que hay: Windows cambia el tamano por su cuenta al pasar a un
    /// monitor con otro DPI, al ajustar la ventana a un lado con Win+flecha o
    /// al maximizarla desde la barra de tareas. Quedarse con el valor viejo
    /// deja la ventana pintando y repartiendo los clics en un sitio distinto
    /// del que ocupa, que es como se queda «sin poder pulsar nada».
    pub fn rect_del_sistema(&self) -> Option<Rect> {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
        let mut r = RECT::default();
        // SAFETY: GetWindowRect escribe en un RECT propio; la ventana es
        // propia y esta viva mientras exista este objeto.
        unsafe { GetWindowRect(self.hwnd, &mut r) }.ok()?;
        let (ancho, alto) = ((r.right - r.left).max(0), (r.bottom - r.top).max(0));
        (ancho > 0 && alto > 0).then_some(Rect {
            x: r.left,
            y: r.top,
            ancho: ancho as u32,
            alto: alto as u32,
        })
    }

    pub fn mostrar(&self) {
        // SIN activar: la activacion de SW_SHOW sincroniza con el shell y
        // costaba 25-40 ms medidos. El foco lo toma enfocar() despues, una
        // sola vez y fuera del camino critico de "visible".
        // SAFETY: la ventana es propia y esta viva.
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
        }
    }

    /// Deja pasar los clics a la aplicacion de abajo, o vuelve a
    /// recogerlos (D50).
    ///
    /// Es lo que hace posible la capa viva: dibujas encima de lo que estas
    /// haciendo, y cuando quieres seguir trabajando la vuelves pasante — el
    /// dibujo se sigue viendo, pero el raton lo atraviesa como si no
    /// estuviera. Sin esto, una capa a pantalla completa secuestra el
    /// escritorio entero.
    ///
    /// `WS_EX_TRANSPARENT` afecta SOLO al raton. El teclado sigue llegando
    /// mientras la ventana tenga el foco, que es lo que permite volver a
    /// activar el dibujo con el mismo atajo.
    pub fn poner_pasante(&self, pasante: bool) {
        use windows::Win32::UI::WindowsAndMessaging::{
            GWL_EXSTYLE, GetWindowLongPtrW, SetWindowLongPtrW, WS_EX_LAYERED, WS_EX_TRANSPARENT,
        };
        // WS_EX_TRANSPARENT solo deja pasar el raton en una ventana LAYERED:
        // sin el segundo bit, el estilo se lee como puesto pero los clics
        // siguen llegando a la capa. Lo encontro la prueba de extremo a
        // extremo: un clic "pasante" abria un texto in situ.
        let bits = WS_EX_LAYERED.0 | WS_EX_TRANSPARENT.0;
        // SAFETY: lee y escribe el estilo extendido de una ventana propia.
        unsafe {
            let actual = GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE) as u32;
            let nuevo = if pasante {
                actual | bits
            } else {
                actual & !bits
            };
            SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, nuevo as isize);
        }
    }

    /// Que pulsarla NO la active (`WS_EX_NOACTIVATE`): los clics le llegan,
    /// pero el foco y el teclado se quedan en la aplicacion donde estaban.
    /// Es lo que necesita la pastilla del anotador de pantalla con el clic a
    /// traves puesto (el `FLAG_NOT_FOCUSABLE` de la barra de
    /// `CapaPantalla.kt` del movil): se pulsa para volver a dibujar sin
    /// quitarle el teclado a nadie.
    pub fn poner_sin_activar(&self) {
        use windows::Win32::UI::WindowsAndMessaging::{
            GWL_EXSTYLE, GetWindowLongPtrW, SetWindowLongPtrW, WS_EX_NOACTIVATE,
        };
        // SAFETY: lee y escribe el estilo extendido de una ventana propia.
        unsafe {
            let actual = GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE) as u32;
            SetWindowLongPtrW(
                self.hwnd,
                GWL_EXSTYLE,
                (actual | WS_EX_NOACTIVATE.0) as isize,
            );
        }
    }

    /// **Que no salga en ninguna captura** (`WDA_EXCLUDEFROMCAPTURE`): la
    /// barra de grabar una clase esta encima de lo que se fotografia y no
    /// tiene que salir en la foto. Antes de Windows 10 2004 falla sin mas y
    /// la ventana sigue igual.
    pub fn excluir_de_capturas(&self) {
        // SAFETY: afinidad de una ventana propia y viva.
        unsafe {
            if let Err(e) = SetWindowDisplayAffinity(self.hwnd, WDA_EXCLUDEFROMCAPTURE) {
                tracing::warn!(?e, "la ventana no se pudo excluir de las capturas");
            }
        }
    }

    /// **La hace propiedad de `dueno`**: Windows mantiene SIEMPRE una ventana
    /// poseida por encima de su dueno, aunque el dueno se active, se reordene
    /// o se vuelva pasante. Es lo que necesita la pastilla del anotador de
    /// pantalla (2026-09-29): las dos son TOPMOST y, sin dueno, activar el
    /// anotador (el primer clic para dibujar cuando `enfocar` no pudo robar
    /// el primer plano, que abriendo con el gesto pasa) lo subia al tope de
    /// las TOPMOST y la pastilla quedaba DEBAJO de una ventana que cubre la
    /// pantalla entera: el usuario no veia el clic a traves. `traer_encima`
    /// una vez no basta; el dueno lo garantiza el sistema.
    ///
    /// `GWLP_HWNDPARENT` en una ventana sin `WS_CHILD` cambia el DUENO, no el
    /// padre (es el camino conocido para ponerlo despues de crearla).
    pub fn poner_dueno(&self, dueno: HWND) {
        use windows::Win32::UI::WindowsAndMessaging::{GWLP_HWNDPARENT, SetWindowLongPtrW};
        // SAFETY: cambia el dueno de una ventana propia y viva por otra ventana
        // del mismo hilo, viva mientras lo este esta (la pastilla muere antes).
        unsafe {
            SetWindowLongPtrW(self.hwnd, GWLP_HWNDPARENT, dueno.0 as isize);
        }
    }

    /// El dueno de la ventana, si tiene (ver [`Self::poner_dueno`]).
    pub fn dueno(&self) -> Option<HWND> {
        use windows::Win32::UI::WindowsAndMessaging::{GW_OWNER, GetWindow};
        // SAFETY: consulta de solo lectura sobre una ventana propia.
        unsafe { GetWindow(self.hwnd, GW_OWNER) }
            .ok()
            .filter(|h| !h.is_invalid())
    }

    /// Coloca la ventana de composicion del IME donde se escribe (D57):
    /// sin esto el japones o el chino se componen en la esquina de la
    /// pantalla, lejos de donde mira el usuario. `p` es local a la ventana.
    pub fn poner_posicion_ime(&self, p: Punto) {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::UI::Input::Ime::{
            CFS_POINT, COMPOSITIONFORM, ImmGetContext, ImmReleaseContext, ImmSetCompositionWindow,
        };
        // SAFETY: contexto del IME de una ventana propia, tomado y devuelto
        // en la misma llamada; la estructura es local y valida.
        unsafe {
            let ctx = ImmGetContext(self.hwnd);
            if ctx.is_invalid() {
                return;
            }
            let forma = COMPOSITIONFORM {
                dwStyle: CFS_POINT,
                ptCurrentPos: POINT { x: p.x, y: p.y },
                ..Default::default()
            };
            let _ = ImmSetCompositionWindow(ctx, &forma);
            let _ = ImmReleaseContext(self.hwnd, ctx);
        }
    }

    /// Si ahora mismo los clics la atraviesan.
    /// Deja pasar los clics que caigan dentro de `hueco`, en coordenadas
    /// del escritorio virtual, y los atiende fuera de el.
    ///
    /// Es lo que hace falta para un marco: el borde se puede agarrar para
    /// mover y redimensionar, pero por el centro se sigue trabajando con
    /// la aplicacion de debajo. `poner_pasante` no sirve aqui porque es
    /// todo o nada para la ventana entera.
    pub fn poner_hueco(&self, hueco: Option<Rect>) {
        HUECO.with(|h| {
            let mut lista = h.borrow_mut();
            lista.retain(|(w, _)| *w != self.hwnd);
            if let Some(r) = hueco {
                lista.push((self.hwnd, r));
            }
        });
    }

    /// El estilo extendido de ahora (`WS_EX_*`), para comprobar sin ensenar
    /// nada lo que decide el orden Z y el foco.
    pub fn estilo_extendido(&self) -> u32 {
        use windows::Win32::UI::WindowsAndMessaging::{GWL_EXSTYLE, GetWindowLongPtrW};
        // SAFETY: consulta de solo lectura sobre ventana propia.
        unsafe { GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE) as u32 }
    }

    pub fn es_pasante(&self) -> bool {
        use windows::Win32::UI::WindowsAndMessaging::{
            GWL_EXSTYLE, GetWindowLongPtrW, WS_EX_TRANSPARENT,
        };
        // SAFETY: consulta de solo lectura sobre ventana propia.
        let actual = unsafe { GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE) as u32 };
        actual & WS_EX_TRANSPARENT.0 != 0
    }

    /// Esconde la ventana sin destruirla. El overlay retiene sus ventanas
    /// entre capturas porque crearlas (con su DComp y su swapchain) costaba
    /// ~90 ms de los 50 permitidos; una ventana oculta no recibe entrada ni
    /// se dibuja, asi que retenerla no cuesta nada.
    /// La manda a la barra de tareas. La ventana de chat la usa desde su
    /// propia barra de titulo, que no es la del sistema.
    pub fn minimizar(&self) {
        use windows::Win32::UI::WindowsAndMessaging::{SW_MINIMIZE, ShowWindow};
        // SAFETY: ventana propia y viva.
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_MINIMIZE);
        }
    }

    /// Deja que se suelten ficheros encima (WM_DROPFILES).
    ///
    /// Apagado por omision: un overlay de captura que aceptara ficheros
    /// robaria el soltar a la ventana que tiene debajo.
    pub fn aceptar_ficheros(&self, si: bool) {
        use windows::Win32::UI::Shell::DragAcceptFiles;
        // SAFETY: ventana propia y viva.
        unsafe {
            DragAcceptFiles(self.hwnd, si);
        }
    }

    pub fn ocultar(&self) {
        // SAFETY: la ventana es propia y esta viva.
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    /// Toma el foco de teclado. Sin esto, Esc/Enter/flechas van a la
    /// aplicacion que estaba activa y el overlay es sordo al teclado — el
    /// primer bug real que encontro la prueba de extremo a extremo.
    ///
    /// `SetForegroundWindow` a secas es NO DETERMINISTA: Windows deniega
    /// robar el primer plano segun quien tuviera la ultima entrada (funciono
    /// a las 21:49 y fallo a las 21:52 con el mismo binario). El adjunto
    /// temporal al hilo del primer plano es el remedio clasico: mientras
    /// dura, ambos hilos comparten el permiso de foco.
    pub fn enfocar(&self) {
        use windows::Win32::System::Threading::GetCurrentThreadId;
        // SAFETY: consultas de solo lectura sobre el estado global, adjunto
        // simetrico (true/false) entre hilos vivos, y foco sobre ventana
        // propia; si el primer plano cambia entre medias, lo peor es que el
        // foco no llegue, que es el estado del que partiamos.
        unsafe {
            let primer_plano = GetForegroundWindow();
            if primer_plano.is_invalid() {
                let _ = SetForegroundWindow(self.hwnd);
                return;
            }
            let hilo_fg = GetWindowThreadProcessId(primer_plano, None);
            let hilo_yo = GetCurrentThreadId();
            if hilo_fg != hilo_yo {
                let _ = AttachThreadInput(hilo_yo, hilo_fg, true);
            }
            let _ = SetForegroundWindow(self.hwnd);
            let _ = SetFocus(Some(self.hwnd));
            if hilo_fg != hilo_yo {
                let _ = AttachThreadInput(hilo_yo, hilo_fg, false);
            }
        }
    }

    pub fn invalidar(&self) {
        // SAFETY: InvalidateRect sobre ventana propia; None invalida entera.
        unsafe {
            let _ = InvalidateRect(Some(self.hwnd), None, false);
        }
    }

    /// Da la ventana por pintada: quien acaba de presentar un fotograma
    /// entero cancela asi el `WM_PAINT` que `invalidar` hubiera dejado
    /// pendiente, y la tanda de muestras no se pinta dos veces.
    pub fn validar(&self) {
        // SAFETY: ValidateRect sobre ventana propia; None valida entera.
        unsafe {
            let _ = ValidateRect(Some(self.hwnd), None);
        }
    }

    /// Toma la captura del raton como si el boton se hubiera pulsado sobre
    /// el overlay. Para los gestos con Alt: el boton se pulso ANTES de que
    /// el overlay existiera, y sin captura el arrastre se perderia al soltar
    /// fuera de la ventana (o al soltar sin haberse movido nunca encima).
    /// Suelta la captura del raton tomada con `capturar_raton`. Sin esto,
    /// el resto del escritorio se queda sin raton hasta que la ventana
    /// muera.
    pub fn soltar_raton(&self) {
        // SAFETY: libera la captura de este hilo; si no habia ninguna, no
        // hace nada.
        unsafe {
            let _ = ReleaseCapture();
        }
    }

    pub fn capturar_raton(&self) {
        // SAFETY: SetCapture sobre ventana propia.
        unsafe { SetCapture(self.hwnd) };
    }

    pub fn poner_cursor(&self, f: FormaCursorWin) {
        CURSOR.with(|c| {
            let mut c = c.borrow_mut();
            match c.iter_mut().find(|(h, _)| *h == self.hwnd) {
                Some(par) => par.1 = f,
                None => c.push((self.hwnd, f)),
            }
        });
    }

    /// Pide todos los puntos del trazo (`EventoOverlay::Muestra`). Solo para
    /// las ventanas de dibujo: el resto sigue recibiendo un movimiento por
    /// mensaje, que es lo que esperan.
    /// Pide los gestos del panel tactil para mover un lienzo: dos dedos
    /// llegan como `DeslizTactil`, el pellizco como `PellizcoTactil`, y la
    /// rueda como `RuedaFina` (ya no como `Rueda`/`RuedaHorizontal`). Solo
    /// para lienzos: el lector PDF, los pines y el resto siguen con su
    /// rueda de siempre porque no lo piden.
    ///
    /// Si Windows no da DirectManipulation, queda la `RuedaFina`: el panel
    /// sigue llegando como rueda y se distingue del raton como se pueda
    /// (`pixpin::navegacion`). Devuelve si hay DirectManipulation.
    pub fn pedir_gestos_tactiles(&self) -> bool {
        if let Some(hay) = GESTOS.with(|g| {
            g.borrow()
                .iter()
                .find(|(h, _)| *h == self.hwnd)
                .map(|(_, d)| d.is_some())
        }) {
            return hay;
        }
        let hwnd = self.hwnd;
        let avisar: crate::gestos_tactiles::Aviso = Box::new(move |s| {
            let ev = match s {
                crate::gestos_tactiles::Salida::Desliz { dx, dy } => {
                    EventoOverlay::DeslizTactil(crate::gestos_tactiles::Desliz::nuevo(dx, dy))
                }
                crate::gestos_tactiles::Salida::Pellizco { factor } => {
                    let mut p = windows::Win32::Foundation::POINT::default();
                    // SAFETY: consulta pura de la posicion del cursor.
                    unsafe {
                        let _ = GetCursorPos(&mut p);
                    }
                    EventoOverlay::PellizcoTactil(crate::gestos_tactiles::Pellizco::nuevo(
                        factor,
                        Punto { x: p.x, y: p.y },
                    ))
                }
            };
            PENDIENTES_OVERLAY.with(|p| p.borrow_mut().push_back((hwnd, ev)));
        });
        let dm = match crate::gestos_tactiles::GestosTactiles::nuevo(
            hwnd,
            self.area.ancho as i32,
            self.area.alto as i32,
            avisar,
        ) {
            Ok(g) => Some(g),
            Err(err) => {
                tracing::warn!(
                    ?err,
                    "sin DirectManipulation: el panel tactil ira como rueda"
                );
                None
            }
        };
        let hay = dm.is_some();
        GESTOS.with(|g| g.borrow_mut().push((hwnd, dm)));
        hay
    }

    pub fn pedir_entrada_fina(&self) {
        ENTRADA_FINA.with(|e| {
            let mut e = e.borrow_mut();
            if !e.iter().any(|(h, _, _)| *h == self.hwnd) {
                e.push((
                    self.hwnd,
                    crate::puntero::HistorialRaton::nuevo(),
                    crate::puntero::EstadoLapiz::SinDatos,
                ));
            }
        });
    }
}

impl Drop for VentanaOverlay {
    fn drop(&mut self) {
        CURSOR.with(|c| c.borrow_mut().retain(|(h, _)| *h != self.hwnd));
        HUECO.with(|h| h.borrow_mut().retain(|(w, _)| *w != self.hwnd));
        PENDIENTES_OVERLAY.with(|p| p.borrow_mut().retain(|(h, _)| *h != self.hwnd));
        ENTRADA_FINA.with(|e| e.borrow_mut().retain(|(h, _, _)| *h != self.hwnd));
        // Se saca de la lista ANTES de soltarlo: al soltarse, DirectManipulation
        // puede avisar todavia, y el aviso no debe encontrar la lista cogida.
        let gestos = GESTOS.with(|g| {
            let mut g = g.borrow_mut();
            g.iter()
                .position(|(h, _)| *h == self.hwnd)
                .map(|i| g.remove(i))
        });
        drop(gestos);
        // SAFETY: destruir una ventana propia desde su hilo es valido; si ya
        // fue destruida por el sistema, DestroyWindow falla y se ignora.
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

/// Bombea mensajes hasta que el callback devuelva Continuar::No.
///
/// No filtra por ventana: el teclado y MSG_DESPIERTA pueden llegar a
/// cualquiera de los overlays. Las ventanas viven en el llamante; el
/// parametro existe para atar tiempos de vida y dejar claro el contrato.
/// Atiende los mensajes que haya pendientes y vuelve enseguida, sin
/// esperar a que llegue ninguno.
///
/// Es lo que necesita un bucle que hace su propio trabajo —la grabacion en
/// GIF, que captura a su ritmo— para no dejar la aplicacion muerta
/// mientras dura: sin esto Windows la marca como «no responde», las
/// ventanas propias no llegan a pintarse y el usuario cree que se colgo.
/// `bucle_modal` no sirve aqui porque se queda esperando.
pub fn bombear_pendientes() {
    use windows::Win32::UI::WindowsAndMessaging::{PM_REMOVE, PeekMessageW};
    let mut msg = MSG::default();
    // SAFETY: PeekMessage y compania sobre la cola del hilo actual; el
    // mensaje es una estructura local. Se acota el numero de mensajes por
    // vuelta para que una tormenta de eventos no robe el turno sin fin.
    unsafe {
        for _ in 0..64 {
            if !PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                break;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// Se lleva los eventos que el WndProc haya encolado, para quien bombea a
/// mano con `bombear_pendientes` en vez de usar `bucle_modal`.
///
/// La grabacion de GIF es ese caso: tiene su propio reloj y no puede
/// cederle el hilo a `GetMessage`, que se duerme hasta que llega algo y
/// perderia el ritmo de captura.
pub fn tomar_eventos_pendientes() -> Vec<(HWND, EventoOverlay)> {
    PENDIENTES_OVERLAY.with(|p| p.borrow_mut().drain(..).collect())
}

/// Duerme el hilo hasta que llegue un mensaje o pase `tope_ms`.
///
/// Es el sustituto de un `sleep` fijo en los bucles que bombean a mano:
/// en reposo cuesta 0 % de CPU y, en cuanto llega un movimiento, vuelve sin
/// esperar a que termine ningun turno. `MWMO_INPUTAVAILABLE` hace que vuelva
/// tambien si ya habia mensajes en la cola que `bombear_pendientes` dejo sin
/// sacar (tope de 64 por vuelta).
pub fn esperar_eventos(tope_ms: Option<u32>) {
    use windows::Win32::System::Threading::INFINITE;
    use windows::Win32::UI::WindowsAndMessaging::{
        MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, QS_ALLINPUT,
    };
    // SAFETY: sin handles; solo espera a la cola de mensajes del hilo.
    unsafe {
        MsgWaitForMultipleObjectsEx(
            None,
            tope_ms.unwrap_or(INFINITE),
            QS_ALLINPUT,
            MWMO_INPUTAVAILABLE,
        );
    }
}

/// Como `esperar_eventos`, pero despierta tambien con `senal` (la de «ya se
/// puede presentar» de una swapchain de baja latencia). Devuelve `true` si
/// desperto la senal: entonces toca leer el raton y pintar YA.
/// Si la senal ya esta disparada, sin esperar (y consumiendola). Hace falta
/// porque con el raton en marcha `esperar_eventos_o_senal` despierta SIEMPRE
/// por la entrada, que va primero, y la senal no llegaba a leerse: el editor
/// dejaba de pintar hasta que el raton se paraba (tirones de 200-350 ms
/// medidos al cablearlo).
pub fn senal_disparada(senal: isize) -> bool {
    use windows::Win32::Foundation::{HANDLE, WAIT_OBJECT_0};
    use windows::Win32::System::Threading::WaitForSingleObjectEx;
    // SAFETY: consulta sin espera de un handle vivo del llamante.
    unsafe { WaitForSingleObjectEx(HANDLE(senal as *mut _), 0, false) == WAIT_OBJECT_0 }
}

/// Un temporizador de espera de alta resolucion, para dormir tramos cortos
/// con precision de menos de un milisegundo.
///
/// **Para que.** El plan B1 del 2026-09-19: el fotograma no se empieza nada
/// mas poder, sino en `plazo de DWM − lo que cuesta pintar − margen`, para
/// que los puntos del lapiz que entran en el sean los mas frescos posibles.
/// Eso exige dormir 6, 8 u 11 ms **de verdad**: un `Sleep` normal se rige
/// por el reloj del planificador (15,6 ms sin `timeBeginPeriod`), asi que
/// «duerme 8» y despierta a los 15, que es justo perder el refresco que se
/// venia a ganar.
///
/// `CREATE_WAITABLE_TIMER_HIGH_RESOLUTION` (Windows 10 1803+) da esa
/// precision **sin** subir la resolucion global del sistema con
/// `timeBeginPeriod(1)`, que la sube para todos los procesos y gasta
/// bateria. En un Windows mas viejo la bandera falla y `nuevo` devuelve
/// `None`: el llamante se queda con el comportamiento de siempre.
pub struct TemporizadorFino {
    handle: windows::Win32::Foundation::HANDLE,
}

impl Drop for TemporizadorFino {
    fn drop(&mut self) {
        // SAFETY: el handle lo creo `nuevo`, es nuestro y se cierra una vez.
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}

impl TemporizadorFino {
    /// `None` si el sistema no tiene temporizadores de alta resolucion.
    pub fn nuevo() -> Option<Self> {
        use windows::Win32::System::Threading::{
            CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, CreateWaitableTimerExW, TIMER_ALL_ACCESS,
        };
        // SAFETY: sin atributos ni nombre; la bandera es la documentada y el
        // handle que sale es nuestro (lo cierra `Drop`).
        let handle = unsafe {
            CreateWaitableTimerExW(
                None,
                None,
                CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
                TIMER_ALL_ACCESS.0,
            )
            .ok()?
        };
        Some(Self { handle })
    }

    /// Duerme `ms` milisegundos sin sacar nada de la cola de mensajes.
    ///
    /// Que NO se saque nada es el objetivo, no un descuido: lo que llegue
    /// mientras se duerme se queda en la cola de Windows y lo recoge la
    /// vuelta siguiente del bucle, que es la que pinta. Esa es toda la idea
    /// de B1: retrasar la lectura hasta el ultimo momento.
    ///
    /// Aun asi la espera es `MsgWaitForMultipleObjectsEx` y no un
    /// `WaitForSingleObject` a secas: con `QS_SENDMESSAGE` vuelve tambien si
    /// otro hilo le manda un mensaje SINCRONO a esta ventana (un
    /// `SendMessage` del sistema, por ejemplo al cambiar de tema o al
    /// consultar accesibilidad). Bloquear ahi cuelga al que envia hasta que
    /// pase el tope, y con el editor dibujando eso pasaria sesenta veces por
    /// segundo.
    pub fn dormir(&self, ms: f32) {
        use windows::Win32::Foundation::WAIT_OBJECT_0;
        use windows::Win32::System::Threading::{INFINITE, SetWaitableTimer};
        use windows::Win32::UI::WindowsAndMessaging::{
            MSG, MSG_WAIT_FOR_MULTIPLE_OBJECTS_EX_FLAGS, MsgWaitForMultipleObjectsEx, PM_NOREMOVE,
            PeekMessageW, QS_SENDMESSAGE,
        };
        if !ms.is_finite() || ms <= 0.0 {
            return;
        }
        // En unidades de 100 ns y en negativo: tiempo RELATIVO, que es lo
        // que documenta `SetWaitableTimer`.
        let plazo = -((ms as f64 * 10_000.0) as i64);
        // SAFETY: el handle es nuestro y esta vivo; `plazo` vive durante la
        // llamada; sin rutina de finalizacion.
        if unsafe { SetWaitableTimer(self.handle, &plazo, 0, None, None, false) }.is_err() {
            return;
        }
        let handles = [self.handle];
        // Tope de vueltas: si algo mandara mensajes sincronos sin parar, esto
        // no puede girar para siempre dentro de un bucle de fotograma.
        for _ in 0..64 {
            // SAFETY: el handle vive durante la espera y la cola es la de
            // este hilo.
            let r = unsafe {
                MsgWaitForMultipleObjectsEx(
                    Some(&handles),
                    INFINITE,
                    QS_SENDMESSAGE,
                    MSG_WAIT_FOR_MULTIPLE_OBJECTS_EX_FLAGS(0),
                )
            };
            if r == WAIT_OBJECT_0 {
                return;
            }
            // Un `PeekMessage` con `PM_NOREMOVE` basta: al mirar la cola,
            // Windows entrega los mensajes sincronos pendientes. No se saca
            // ninguno de los nuestros.
            let mut msg = MSG::default();
            // SAFETY: `msg` es local y valido; no se retira nada.
            unsafe {
                let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
            }
        }
    }
}

pub fn esperar_eventos_o_senal(senal: Option<isize>, tope_ms: Option<u32>) -> bool {
    use windows::Win32::Foundation::{HANDLE, WAIT_OBJECT_0};
    use windows::Win32::System::Threading::INFINITE;
    use windows::Win32::UI::WindowsAndMessaging::{
        MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, QS_ALLINPUT,
    };
    let Some(s) = senal else {
        esperar_eventos(tope_ms);
        return false;
    };
    let handles = [HANDLE(s as *mut _)];
    // SAFETY: el handle es de la swapchain del llamante, vivo mientras dura
    // la espera; la cola es la del hilo actual.
    let r = unsafe {
        MsgWaitForMultipleObjectsEx(
            Some(&handles),
            tope_ms.unwrap_or(INFINITE),
            QS_ALLINPUT,
            MWMO_INPUTAVAILABLE,
        )
    };
    r == WAIT_OBJECT_0
}

pub fn bucle_modal(
    ventanas: &[VentanaOverlay],
    mut callback: impl FnMut(HWND, EventoOverlay) -> crate::ventana::Continuar,
) {
    let _ = ventanas;
    let mut msg = MSG::default();
    loop {
        // SAFETY: GetMessageW con punteros locales validos; <= 0 es salida.
        let r = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if r.0 <= 0 {
            return;
        }
        // SAFETY: protocolo estandar de bombeo.
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        // Los atajos globales que el WndProc de la ventana principal haya
        // encolado durante el Dispatch se entregan AQUI: si se quedaran en
        // su cola, se atenderian al cerrar el overlay y lo reabririan.
        for id in crate::ventana::tomar_atajos_pendientes() {
            if callback(HWND::default(), EventoOverlay::Atajo(id)) == crate::ventana::Continuar::No
            {
                return;
            }
        }
        // Drenar lo que el WndProc haya encolado durante el Dispatch.
        loop {
            let siguiente = PENDIENTES_OVERLAY.with(|p| p.borrow_mut().pop_front());
            let Some((hwnd, evento)) = siguiente else {
                break;
            };
            if callback(hwnd, evento) == crate::ventana::Continuar::No {
                return;
            }
        }
    }
}

/// Presta el historial y el estado del lapiz de una ventana con entrada fina.
/// `None` si la ventana no la pidio.
fn con_entrada_fina<R>(
    hwnd: HWND,
    f: impl FnOnce(&mut crate::puntero::HistorialRaton, &mut crate::puntero::EstadoLapiz) -> R,
) -> Option<R> {
    ENTRADA_FINA.with(|e| {
        e.borrow_mut()
            .iter_mut()
            .find(|(w, _, _)| *w == hwnd)
            .map(|(_, h, estado)| f(h, estado))
    })
}

/// Si este `WM_POINTER*` es de un contacto de borrador y hay que tragarlo
/// (D107). Deja el estado de la ventana al dia. Solo se pregunta al lapiz
/// al bajar: despues manda el estado, que es lo que decidio ese pulsado.
fn tragar_goma(hwnd: HWND, mensaje: u32, wparam: WPARAM) -> bool {
    use crate::puntero::FaseContacto;
    let fase = match mensaje {
        WM_POINTERDOWN => FaseContacto::Baja,
        WM_POINTERUP => FaseContacto::Sube,
        _ => FaseContacto::Actualiza,
    };
    let goma = fase == FaseContacto::Baja
        && crate::puntero::indicadores_de_lapiz(wparam).is_some_and(crate::puntero::es_goma);
    con_entrada_fina(hwnd, |h, estado| {
        let nuevo = crate::puntero::decidir_goma(fase, goma, *estado)?;
        *estado = nuevo;
        // Nada del raton de antes o de durante el borrador vale para el
        // trazo siguiente.
        h.olvidar();
        Some(())
    })
    .flatten()
    .is_some()
}

fn registrar_clase() {
    // SAFETY: registro unico (Once) de una clase con WndProc propio; los
    // campos no usados quedan a cero, que es lo que la API espera.
    unsafe {
        let instancia: windows::Win32::Foundation::HINSTANCE =
            GetModuleHandleW(None).expect("modulo propio").into();
        // El icono de PixPin (recurso 1 del ejecutable, `pixpinmax.rc`): sin
        // el, el chat, los visores y los pines salian con el icono generico
        // en la barra de tareas y en Alt+Tab (20-oct-2026, el usuario: «su
        // icono no aparece de la app»).
        let icono = windows::Win32::UI::WindowsAndMessaging::LoadIconW(
            Some(instancia),
            windows::core::PCWSTR(1 as _),
        )
        .unwrap_or_default();
        let clase = WNDCLASSW {
            lpfnWndProc: Some(procedimiento_overlay),
            hInstance: instancia,
            hIcon: icono,
            lpszClassName: w!("PixPinOverlay"),
            ..Default::default()
        };
        RegisterClassW(&clase);
    }
}

extern "system" fn procedimiento_overlay(
    hwnd: HWND,
    mensaje: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let encolar = |e: EventoOverlay| {
        PENDIENTES_OVERLAY.with(|p| p.borrow_mut().push_back((hwnd, e)));
    };
    // Una ventana que pidio gestos tactiles recibe la rueda con su hora y
    // sus teclas; las demas, como siempre.
    let rueda = |delta: i32, horizontal: bool, (ctrl, shift): (bool, bool)| {
        let con_gestos = GESTOS.with(|g| g.borrow().iter().any(|(h, _)| *h == hwnd));
        let ev = if con_gestos {
            EventoOverlay::RuedaFina(crate::gestos_tactiles::RuedaCruda {
                delta,
                horizontal,
                // SAFETY: hora del mensaje que se esta atendiendo.
                ms: unsafe { GetMessageTime() } as u32,
                ctrl,
                shift,
            })
        } else if horizontal {
            EventoOverlay::RuedaHorizontal(delta)
        } else {
            EventoOverlay::Rueda(delta)
        };
        PENDIENTES_OVERLAY.with(|p| p.borrow_mut().push_back((hwnd, ev)));
    };
    // Cliente -> escritorio virtual: la ventana vive en su esquina del
    // escritorio, asi que basta sumar su origen real.
    let punto = |lparam: LPARAM| {
        let mut r = RECT::default();
        // SAFETY: GetWindowRect sobre la ventana del propio WndProc.
        unsafe {
            let _ = GetWindowRect(hwnd, &mut r);
        }
        Punto {
            x: r.left + (lparam.0 & 0xFFFF) as i16 as i32,
            y: r.top + ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
        }
    };

    match mensaje {
        // El hueco: dentro de el la ventana no existe para el raton, asi
        // que el clic llega a la aplicacion de debajo. El lparam de este
        // mensaje ya viene en coordenadas de pantalla, al reves que el
        // resto de mensajes de raton.
        WM_NCHITTEST => {
            let p = Punto {
                x: (lparam.0 & 0xFFFF) as i16 as i32,
                y: ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
            };
            let dentro = HUECO.with(|h| {
                h.borrow()
                    .iter()
                    .find(|(w, _)| *w == hwnd)
                    .is_some_and(|(_, r)| r.contiene(p))
            });
            if dentro {
                LRESULT(HTTRANSPARENT as isize)
            } else {
                LRESULT(HTCLIENT as isize)
            }
        }
        WM_MOUSEMOVE => {
            let p = punto(lparam);
            const MK_LBUTTON: usize = 0x0001;
            let boton = wparam.0 & MK_LBUTTON != 0;
            let fina = ENTRADA_FINA.with(|e| e.borrow().iter().any(|(h, _, _)| *h == hwnd));
            if fina {
                // `descartar_movimiento` (pura, con su propia tabla de
                // pruebas) decide si este WM_MOUSEMOVE sintetizado sobra: el
                // camino del lapiz (WM_POINTERUPDATE) ya lo cubre, o es su
                // borrador y no debe reaparecer como tinta (D107).
                let origen = crate::puntero::origen_actual();
                // SAFETY: tiempo del mensaje que se esta atendiendo.
                let tiempo = unsafe { GetMessageTime() } as u32;
                let (descartar, perdidos) = con_entrada_fina(hwnd, |h, estado| {
                    if crate::puntero::descartar_movimiento(boton, origen, *estado) {
                        // Tirado, pero no ignorado: si el historial no se
                        // olvidara, el siguiente movimiento (un SinDatos a
                        // mitad del trazo) recuperaria desde un punto viejo
                        // y repetiria puntos ya entregados por el lapiz.
                        h.olvidar();
                        return (true, Vec::new());
                    }
                    if !boton {
                        // Se acabo el trazo (o nunca hubo boton): el
                        // siguiente no debe heredar el estado del lapiz de
                        // este, o un fallo puntual del historial del
                        // siguiente trazo se confundiria con un ConMuestras
                        // viejo y le tiraria el primer movimiento. Un
                        // contacto de borrador en curso se respeta: su
                        // WM_POINTERUP es quien lo cierra.
                        h.olvidar();
                        if *estado != crate::puntero::EstadoLapiz::ContactoDeGoma {
                            *estado = crate::puntero::EstadoLapiz::SinDatos;
                        }
                        return (false, Vec::new());
                    }
                    if !crate::puntero::usa_historial_raton(origen)
                        || crate::puntero::olvida_historial_raton(*estado)
                    {
                        // Un movimiento del lapiz no se recupera del
                        // historial del raton (M3): sus puntos buenos
                        // vienen por WM_POINTERUPDATE.
                        h.olvidar();
                        return (false, Vec::new());
                    }
                    (false, h.recuperar(p.x, p.y, tiempo))
                })
                .unwrap_or_default();
                if descartar {
                    return LRESULT(0);
                }
                for m in perdidos {
                    encolar(EventoOverlay::Muestra(m));
                }
            }
            encolar(EventoOverlay::RatonMovido(p));
            LRESULT(0)
        }
        WM_MOUSEHWHEEL => {
            let delta = ((wparam.0 >> 16) & 0xFFFF) as i16 as i32;
            avisar_de_horizontal("WM_MOUSEHWHEEL", delta);
            rueda(delta, true, teclas_de_rueda(wparam));
            LRESULT(0)
        }
        // El camino VIEJO del desplazamiento de lado: muchos controladores de
        // panel tactil (Synaptics, ELAN) no mandan `WM_MOUSEHWHEEL` sino una
        // barra de desplazamiento fingida. Sin atenderlo, en esos equipos el
        // lienzo solo sube y baja, que es lo que reporto el usuario.
        WM_HSCROLL => {
            let delta = match (wparam.0 & 0xFFFF) as u32 {
                0 => -40,  // SB_LINELEFT
                1 => 40,   // SB_LINERIGHT
                2 => -120, // SB_PAGELEFT
                3 => 120,  // SB_PAGERIGHT
                _ => 0,
            };
            avisar_de_horizontal("WM_HSCROLL", delta);
            if delta != 0 {
                // Este wparam no trae teclas: es el codigo de la barra.
                rueda(delta, true, (false, false));
            }
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            // La palabra alta del wparam trae el giro con signo.
            let delta = ((wparam.0 >> 16) & 0xFFFF) as i16 as i32;
            rueda(delta, false, teclas_de_rueda(wparam));
            LRESULT(0)
        }
        // El panel tactil de precision pone dos dedos: si esta ventana pidio
        // gestos tactiles, DirectManipulation se queda el contacto y el gesto
        // ya no llega como rueda (`gestos_tactiles`).
        m if m == crate::gestos_tactiles::DM_POINTERHITTEST => {
            GESTOS.with(|g| {
                if let Some((_, Some(dm))) = g.borrow().iter().find(|(h, _)| *h == hwnd) {
                    dm.tocar(wparam);
                }
            });
            // SAFETY: reenvio estandar del mensaje recibido.
            unsafe { DefWindowProcW(hwnd, mensaje, wparam, lparam) }
        }
        WM_TIMER if wparam.0 == crate::gestos_tactiles::ID_TEMPORIZADOR => {
            GESTOS.with(|g| {
                if let Some((_, Some(dm))) = g.borrow().iter().find(|(h, _)| *h == hwnd) {
                    dm.latido();
                }
            });
            LRESULT(0)
        }
        // El boton derecho vale lo mismo que el izquierdo: el gesto de
        // «Alt + derecho y arrastrar» pinea directo, y la seleccion que
        // arranca con el derecho se termina con el derecho.
        WM_RBUTTONDOWN => {
            // SAFETY: SetCapture sobre ventana propia, como el izquierdo.
            unsafe { SetCapture(hwnd) };
            encolar(EventoOverlay::BotonDerechoPulsado(punto(lparam)));
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            // SAFETY: SetCapture sobre ventana propia: el arrastre no se
            // pierde al salir del borde.
            unsafe { SetCapture(hwnd) };
            let p = punto(lparam);
            // El pulsado es el "anterior" del primer movimiento: sin
            // sembrarlo, ese movimiento no recupera los puntos fusionados
            // mientras el editor gasta un fotograma en el pulsado, y cada
            // trazo rapido empezaba con una cuerda recta. Un pulsado
            // sintetizado por el lapiz no siembra: sus puntos buenos vienen
            // por WM_POINTERUPDATE.
            if crate::puntero::usa_historial_raton(crate::puntero::origen_actual()) {
                // SAFETY: tiempo del mensaje que se esta atendiendo.
                let tiempo = unsafe { GetMessageTime() } as u32;
                con_entrada_fina(hwnd, |h, _| h.sembrar(p.x, p.y, tiempo));
            }
            encolar(EventoOverlay::BotonPulsado(p));
            LRESULT(0)
        }
        WM_LBUTTONUP | WM_RBUTTONUP => {
            // SAFETY: libera la captura tomada arriba.
            unsafe {
                let _ = ReleaseCapture();
            }
            // Se olvida aqui, no solo en el siguiente WM_MOUSEMOVE: si se
            // suelta el boton sin volver a mover (un simple clic), ningun
            // WM_MOUSEMOVE con boton=false llegaria a limpiarlo, y el
            // estado del lapiz de este trazo se colaria en el siguiente.
            con_entrada_fina(hwnd, |h, estado| {
                h.olvidar();
                *estado = crate::puntero::EstadoLapiz::SinDatos;
            });
            encolar(EventoOverlay::BotonSoltado(punto(lparam)));
            LRESULT(0)
        }
        WM_MBUTTONDOWN => {
            // SAFETY: SetCapture sobre ventana propia: arrastrar el lienzo
            // con el boton central no se pierde al salir del borde.
            unsafe { SetCapture(hwnd) };
            encolar(EventoOverlay::BotonCentralPulsado(punto(lparam)));
            LRESULT(0)
        }
        WM_MBUTTONUP => {
            // SAFETY: libera la captura tomada arriba.
            unsafe {
                let _ = ReleaseCapture();
            }
            encolar(EventoOverlay::BotonCentralSoltado(punto(lparam)));
            LRESULT(0)
        }
        // Las dos: con Alt mantenido, Windows manda WM_SYSKEYDOWN en vez de
        // WM_KEYDOWN. Sin atender el segundo, la ventana de ajustes no
        // podria grabar ningun atajo con Alt, que son la mitad de los que
        // trae el programa de fabrica.
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            // SAFETY: GetKeyState es una consulta sin precondiciones.
            let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0;
            // SAFETY: GetKeyState es una consulta sin precondiciones.
            let ctrl = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
            // SAFETY: GetKeyState es una consulta sin precondiciones.
            let alt = unsafe { GetKeyState(VK_MENU.0 as i32) } < 0;
            encolar(EventoOverlay::Tecla {
                vk: wparam.0 as u32,
                shift,
                ctrl,
                alt,
            });
            LRESULT(0)
        }
        WM_KEYUP | WM_SYSKEYUP => {
            encolar(EventoOverlay::TeclaSoltada(wparam.0 as u32));
            LRESULT(0)
        }
        WM_CHAR => {
            // WM_CHAR trae unidades UTF-16: un emoji llega en dos mensajes.
            // La mitad alta se guarda hasta que llega la baja. El IME
            // entrega por aqui el texto ya compuesto, asi que el japones o
            // el chino no necesitan nada mas.
            let unidad = wparam.0 as u16;
            let caracter = MITAD_ALTA.with(|alta| {
                if (0xD800..0xDC00).contains(&unidad) {
                    alta.set(Some(unidad));
                    None
                } else if (0xDC00..0xE000).contains(&unidad) {
                    let a = alta.take()?;
                    char::decode_utf16([a, unidad]).next()?.ok()
                } else {
                    alta.set(None);
                    char::from_u32(unidad as u32)
                }
            });
            if let Some(c) = caracter.filter(|c| es_letra_de_wm_char(*c)) {
                encolar(EventoOverlay::Caracter(c));
            }
            LRESULT(0)
        }
        WM_DROPFILES => {
            use windows::Win32::UI::Shell::{DragFinish, DragQueryFileW, HDROP};
            let arrastre = HDROP(wparam.0 as *mut _);
            let mut rutas = Vec::new();
            // SAFETY: `wparam` de WM_DROPFILES ES un HDROP valido hasta que se
            // llama a DragFinish, que se llama justo debajo pase lo que pase.
            unsafe {
                // Con 0xFFFF_FFFF devuelve cuantos hay, no una ruta.
                let cuantos = DragQueryFileW(arrastre, 0xFFFF_FFFF, None);
                for i in 0..cuantos {
                    // Primero se pregunta el largo (sin el cero final) y
                    // luego se pide; pedir a ciegas con un buffer fijo
                    // truncaria las rutas largas.
                    let largo = DragQueryFileW(arrastre, i, None) as usize;
                    if largo == 0 {
                        continue;
                    }
                    let mut buffer = vec![0u16; largo + 1];
                    let escritos = DragQueryFileW(arrastre, i, Some(&mut buffer)) as usize;
                    if escritos == 0 {
                        continue;
                    }
                    rutas.push(std::path::PathBuf::from(String::from_utf16_lossy(
                        &buffer[..escritos],
                    )));
                }
                DragFinish(arrastre);
            }
            if !rutas.is_empty() {
                SOLTADOS.with(|s| s.borrow_mut().extend(rutas));
                encolar(EventoOverlay::FicherosSoltados);
            }
            LRESULT(0)
        }
        WM_PAINT => {
            // SAFETY: ValidateRect marca la ventana como pintada; el dibujo
            // real lo hace el swapchain, no GDI.
            unsafe {
                let _ = ValidateRect(Some(hwnd), None);
            }
            encolar(EventoOverlay::Pintar);
            LRESULT(0)
        }
        WM_DPICHANGED => {
            encolar(EventoOverlay::CambioDpi);
            LRESULT(0)
        }
        WM_SETCURSOR => {
            let forma =
                CURSOR.with(|c| c.borrow().iter().find(|(h, _)| *h == hwnd).map(|(_, f)| *f));
            let id = match forma.unwrap_or(FormaCursorWin::Cruz) {
                FormaCursorWin::Cruz => IDC_CROSS,
                FormaCursorWin::Mover => IDC_SIZEALL,
                FormaCursorWin::RedimNS => IDC_SIZENS,
                FormaCursorWin::RedimEO => IDC_SIZEWE,
                FormaCursorWin::RedimNeSo => IDC_SIZENESW,
                FormaCursorWin::RedimNoSe => IDC_SIZENWSE,
                FormaCursorWin::Texto => IDC_IBEAM,
                FormaCursorWin::Flecha => IDC_ARROW,
                FormaCursorWin::Giro => IDC_HAND,
            };
            // SAFETY: LoadCursorW de un cursor del sistema y SetCursor son
            // llamadas sin precondiciones sobre recursos compartidos.
            unsafe {
                if let Ok(cursor) = LoadCursorW(None, id) {
                    SetCursor(Some(cursor));
                }
            }
            LRESULT(1)
        }
        WM_CLOSE => {
            // Alt+F4 sobre el overlay: cancelar limpiamente, NUNCA destruir
            // la ventana por debajo del bucle modal — dejaria un overlay
            // zombi invisible con el bucle vivo.
            encolar(EventoOverlay::Cerrar);
            LRESULT(0)
        }
        m if m == MSG_DESPIERTA => {
            encolar(EventoOverlay::Despierta);
            LRESULT(0)
        }
        // El borrador del lapiz (D107): un contacto que empieza con el se
        // traga entero, sin DefWindowProc, para que Windows no sintetice un
        // WM_LBUTTONDOWN que el editor convertiria en un lapiz y la capa en
        // un punto. `decidir_goma` (pura, con su tabla) dice cuando.
        WM_POINTERDOWN | WM_POINTERUPDATE | WM_POINTERUP
            if ENTRADA_FINA.with(|e| e.borrow().iter().any(|(h, _, _)| *h == hwnd))
                && tragar_goma(hwnd, mensaje, wparam) =>
        {
            LRESULT(0)
        }
        WM_POINTERUPDATE
            if ENTRADA_FINA.with(|e| e.borrow().iter().any(|(h, _, _)| *h == hwnd)) =>
        {
            let muestras = crate::puntero::muestras_de_lapiz(wparam);
            // Se recuerda el estado (no un simple bool: Goma y SinDatos
            // llegaban antes como el mismo "vector vacio", y sin
            // distinguirlos el borrador colaba su WM_MOUSEMOVE como si
            // fuera tinta de verdad — ver descartar_movimiento).
            let estado = match &muestras {
                crate::puntero::MuestrasLapiz::SinDatos => crate::puntero::EstadoLapiz::SinDatos,
                crate::puntero::MuestrasLapiz::Goma => crate::puntero::EstadoLapiz::Goma,
                crate::puntero::MuestrasLapiz::Muestras(_) => {
                    crate::puntero::EstadoLapiz::ConMuestras
                }
            };
            con_entrada_fina(hwnd, |h, e| {
                *e = estado;
                // El lapiz respondio: el ultimo punto del raton recordado ya
                // no es el ultimo entregado (M3).
                if crate::puntero::olvida_historial_raton(estado) {
                    h.olvidar();
                }
            });
            if let crate::puntero::MuestrasLapiz::Muestras(muestras) = muestras {
                for m in muestras {
                    encolar(EventoOverlay::Muestra(m));
                }
            }
            // Se deja pasar a DefWindowProc: asi Windows sigue sintetizando
            // los clics del lapiz, que es como se pulsa y se suelta.
            // SAFETY: reenvio estandar del mensaje recibido.
            unsafe { DefWindowProcW(hwnd, mensaje, wparam, lparam) }
        }
        // NUNCA PostQuitMessage aqui: ver el comentario de modulo.
        _ => {
            // SAFETY: delegacion estandar al procedimiento por defecto.
            unsafe { DefWindowProcW(hwnd, mensaje, wparam, lparam) }
        }
    }
}

/// Ctrl y Shift segun el propio `WM_MOUSEWHEEL` (`MK_CONTROL`, `MK_SHIFT`
/// en la palabra baja). El pellizco de un panel sin DirectManipulation
/// llega como rueda con Ctrl fingido: el mensaje lo dice seguro, sondear
/// el teclado despues quiza no.
fn teclas_de_rueda(wparam: WPARAM) -> (bool, bool) {
    const MK_SHIFT: usize = 0x0004;
    const MK_CONTROL: usize = 0x0008;
    (wparam.0 & MK_CONTROL != 0, wparam.0 & MK_SHIFT != 0)
}

/// Apunta en el registro las primeras veces que llega un desplazamiento de
/// lado, y por que camino. Es para diagnosticar en el equipo del usuario: que
/// mensaje manda su panel tactil no se puede saber desde aqui, y sin verlo se
/// arregla a ciegas. Solo las primeras, para no llenar el registro.
fn avisar_de_horizontal(camino: &'static str, delta: i32) {
    use std::sync::atomic::{AtomicU32, Ordering};
    static VECES: AtomicU32 = AtomicU32::new(0);
    if VECES.fetch_add(1, Ordering::Relaxed) < 8 {
        tracing::info!(camino, delta, "desplazamiento de lado recibido");
    }
}

/// Si lo que trae WM_CHAR es una letra para escribir.
///
/// **Ctrl+Retroceso** manda su WM_KEYDOWN (VK_BACK con Ctrl, que es lo que
/// borra la palabra) y DETRAS un WM_CHAR con `0x7F` (DEL), que no es ninguna
/// letra. Las cajas que solo miraban `c >= ' '` lo escribian: un caracter
/// invisible detras de la rayita, y el siguiente Ctrl+Retroceso se gastaba
/// en borrarlo a el en vez de la palabra (el usuario, 17-oct-2026: «solo se
/// borra la primera palabra y luego ya no funciona»). Se quita aqui, en el
/// unico sitio por el que entra lo tecleado, y vale para todas las cajas.
/// Los demas mandos (Intro `\r`, Retroceso `\x08`, Tab) siguen llegando:
/// hay quien los atiende como letra.
pub fn es_letra_de_wm_char(c: char) -> bool {
    c != '\u{7f}'
}

/// Donde esta el raton, en coordenadas del escritorio virtual.
pub fn raton_en_pantalla() -> Punto {
    let mut p = windows::Win32::Foundation::POINT::default();
    // SAFETY: escribe en la variable local.
    let _ = unsafe { GetCursorPos(&mut p) };
    Punto { x: p.x, y: p.y }
}

/// La ventana de primer nivel que se ve en `p` (la que recibiria un clic),
/// o una nula. Para saber si una ventana que no esta «siempre encima» esta
/// tapada por otra aplicacion justo donde esta el raton.
pub fn ventana_bajo(p: Punto) -> HWND {
    // SAFETY: consultas de solo lectura; un manejador que desaparezca entre
    // medias da una ventana nula, que no es de nadie.
    unsafe {
        let h = WindowFromPoint(windows::Win32::Foundation::POINT { x: p.x, y: p.y });
        if h.is_invalid() {
            h
        } else {
            GetAncestor(h, GA_ROOT)
        }
    }
}

/// El monitor donde cae `p`: su area entera y la de trabajo (sin la barra
/// de tareas).
pub fn monitor_de(p: Punto) -> (Rect, Rect) {
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
    };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: estructura local con su tamano.
    unsafe {
        let m = MonitorFromPoint(
            windows::Win32::Foundation::POINT { x: p.x, y: p.y },
            MONITOR_DEFAULTTONEAREST,
        );
        let _ = GetMonitorInfoW(m, &mut info);
    }
    let r = |r: RECT| Rect {
        x: r.left,
        y: r.top,
        ancho: (r.right - r.left).max(1) as u32,
        alto: (r.bottom - r.top).max(1) as u32,
    };
    (r(info.rcMonitor), r(info.rcWork))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_geom::Rect;

    #[test]
    fn el_del_de_ctrl_retroceso_no_es_una_letra() {
        // Lo que manda Windows detras de Ctrl+Retroceso: fuera.
        assert!(!es_letra_de_wm_char('\u{7f}'));
        // Caso negativo: las letras y los mandos de siempre siguen pasando.
        for c in ['a', 'ñ', ' ', '€', '\r', '\u{8}', '\t'] {
            assert!(es_letra_de_wm_char(c), "{c:?}");
        }
    }

    #[test]
    fn ctrl_y_shift_de_la_rueda_salen_del_propio_mensaje() {
        // Palabra alta: el giro; baja: MK_*. 120 con MK_CONTROL (0x08).
        assert_eq!(teclas_de_rueda(WPARAM((120 << 16) | 0x0008)), (true, false));
        assert_eq!(teclas_de_rueda(WPARAM((120 << 16) | 0x0004)), (false, true));
        // Caso negativo: el boton izquierdo pulsado (MK_LBUTTON) no es Ctrl,
        // ni lo es un giro cuyo valor tenga el bit 3 puesto en la palabra alta.
        assert_eq!(teclas_de_rueda(WPARAM((8 << 16) | 0x0001)), (false, false));
    }

    #[test]
    #[ignore = "necesita sesion de escritorio; ejecutar con --ignored"]
    fn el_modo_pasante_se_activa_y_se_quita() {
        // D50: es lo que permite dibujar encima de lo que estas haciendo y
        // luego seguir trabajando sin cerrar la capa. Se comprueba contra el
        // estilo REAL de la ventana, no contra una variable propia: si
        // Windows no aplicara el cambio, un booleano nuestro mentiria.
        let v = VentanaOverlay::nueva(Rect {
            x: 0,
            y: 0,
            ancho: 300,
            alto: 200,
        })
        .expect("la ventana deberia crearse");

        assert!(!v.es_pasante(), "una capa nace recogiendo el raton");
        v.poner_pasante(true);
        assert!(v.es_pasante(), "los clics deberian atravesarla");
        v.poner_pasante(false);
        assert!(!v.es_pasante(), "y volver a recogerse");
    }

    #[test]
    #[ignore = "necesita sesion de escritorio; ejecutar con --ignored"]
    fn poner_pasante_no_borra_los_demas_estilos() {
        // Caso negativo de la escritura del estilo: un SetWindowLongPtrW que
        // asignara el estilo en vez de combinarlo dejaria la ventana sin
        // TOPMOST ni NOREDIRECTIONBITMAP, y la capa dejaria de componerse.
        use windows::Win32::UI::WindowsAndMessaging::{
            GWL_EXSTYLE, GetWindowLongPtrW, WS_EX_TOPMOST,
        };
        let v = VentanaOverlay::nueva(Rect {
            x: 0,
            y: 0,
            ancho: 300,
            alto: 200,
        })
        .unwrap();
        v.poner_pasante(true);
        // SAFETY: consulta de solo lectura sobre la ventana del test.
        let estilo = unsafe { GetWindowLongPtrW(v.handle(), GWL_EXSTYLE) as u32 };
        assert!(
            estilo & WS_EX_TOPMOST.0 != 0,
            "la capa perdio el TOPMOST al volverse pasante"
        );
    }

    #[test]
    #[ignore = "necesita sesion de escritorio; ejecutar con --ignored"]
    fn crear_y_destruir_un_overlay_no_toca_al_resto() {
        // La mina de la retrospectiva: si el WndProc del overlay llamara a
        // PostQuitMessage en WM_DESTROY, destruir la primera ventana
        // contaminaria el bucle de la segunda. Se destruye una y se
        // comprueba que la otra sigue viva y funcional.
        let a = VentanaOverlay::nueva(Rect {
            x: 0,
            y: 0,
            ancho: 200,
            alto: 200,
        })
        .expect("primera ventana");
        let b = VentanaOverlay::nueva(Rect {
            x: 200,
            y: 0,
            ancho: 200,
            alto: 200,
        })
        .expect("segunda ventana");

        drop(a);

        // SAFETY: IsWindow es una consulta sin precondiciones.
        let viva = unsafe {
            windows::Win32::UI::WindowsAndMessaging::IsWindow(Some(b.handle())).as_bool()
        };
        assert!(viva, "destruir un overlay no debe llevarse a los demas");
    }

    #[test]
    #[ignore = "necesita sesion de escritorio; ejecutar con --ignored"]
    fn el_bucle_modal_termina_cuando_el_callback_dice_no() {
        let v = VentanaOverlay::nueva(Rect {
            x: 0,
            y: 0,
            ancho: 100,
            alto: 100,
        })
        .unwrap();
        // Se autoenvia el mensaje de despertar; el callback corta al verlo.
        // Si bucle_modal no drenara la cola ni tradujera MSG_DESPIERTA, este
        // test se quedaria colgado (el timeout del harness lo delata).
        // SAFETY: PostMessageW a una ventana propia y viva.
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                Some(v.handle()),
                MSG_DESPIERTA,
                Default::default(),
                Default::default(),
            )
            .unwrap();
        }
        let mut despertares = 0;
        bucle_modal(std::slice::from_ref(&v), |_hwnd, evento| {
            if matches!(evento, EventoOverlay::Pintar) {
                return crate::ventana::Continuar::Si;
            }
            despertares += 1;
            crate::ventana::Continuar::No
        });
        assert_eq!(despertares, 1);
    }

    #[test]
    fn la_tecla_lleva_alt_ademas_de_shift_y_ctrl() {
        // Alt escala desde el centro. Sin este campo, el editor no puede
        // distinguir un arrastre normal de uno desde el centro.
        let t = EventoOverlay::Tecla {
            vk: 65,
            shift: true,
            ctrl: false,
            alt: true,
        };
        let EventoOverlay::Tecla { alt, .. } = t else {
            panic!()
        };
        assert!(alt);
    }

    #[test]
    fn hay_cursor_de_giro() {
        // Que exista la variante. Cual dibuja Windows se comprueba a mano: el
        // mapeo a IDC_* necesita una sesion de escritorio.
        let formas = [
            FormaCursorWin::Cruz,
            FormaCursorWin::Mover,
            FormaCursorWin::RedimNS,
            FormaCursorWin::RedimEO,
            FormaCursorWin::RedimNeSo,
            FormaCursorWin::RedimNoSe,
            FormaCursorWin::Texto,
            FormaCursorWin::Flecha,
            FormaCursorWin::Giro,
        ];
        assert_eq!(formas.len(), 9);
    }

    #[test]
    fn el_boton_central_viaja_como_evento_copiable_y_comparable() {
        // `EventoOverlay` cruza colas por valor y se compara en las pruebas
        // del editor: las variantes nuevas no pueden quitarle Copy ni Eq.
        fn copiable<T: Copy + Eq>(_: T) {}
        let p = EventoOverlay::BotonCentralPulsado(Punto { x: 3, y: 4 });
        copiable(p);
        assert_ne!(p, EventoOverlay::BotonCentralSoltado(Punto { x: 3, y: 4 }));
        assert_ne!(p, EventoOverlay::BotonPulsado(Punto { x: 3, y: 4 }));
    }

    #[test]
    #[ignore = "necesita sesion de escritorio; ejecutar con --ignored"]
    fn traer_encima_pone_la_ventana_sobre_otra_topmost_creada_despues() {
        // D145: el lienzo tiene que quedar por encima de los pines, que
        // tambien son TOPMOST. Se crea `b` despues (queda encima) y se
        // comprueba que `traer_encima` pone `a` por delante.
        use windows::Win32::UI::WindowsAndMessaging::{GW_HWNDPREV, GetWindow};
        let rect = Rect {
            x: 0,
            y: 0,
            ancho: 100,
            alto: 100,
        };
        let a = VentanaOverlay::nueva(rect).unwrap();
        let b = VentanaOverlay::nueva(rect).unwrap();
        a.traer_encima();
        let mut actual = b.handle();
        let mut a_por_encima = false;
        for _ in 0..10_000 {
            // SAFETY: consulta del orden Z; si la cadena se corta, se para.
            match unsafe { GetWindow(actual, GW_HWNDPREV) } {
                Ok(h) if !h.is_invalid() => {
                    if h == a.handle() {
                        a_por_encima = true;
                        break;
                    }
                    actual = h;
                }
                _ => break,
            }
        }
        assert!(a_por_encima, "la ventana traida encima sigue debajo");
    }
}
