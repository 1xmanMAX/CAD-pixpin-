//! **La vista previa de Windows dentro de un pin** (10-oct-2026, el «pin de
//! Office»): abrir un Word, un Excel, un PowerPoint, un Visio o un correo
//! `.msg` casi al instante, con el mismo previsualizador que Office registra
//! para el panel de vista previa del Explorador.
//!
//! Todo sale de la documentacion publica de Microsoft sobre los
//! *preview handlers* («Preview Handlers and Shell Preview Host», «Building
//! Preview Handlers», `IPreviewHandler`, `IPreviewHandlerFrame`,
//! `IInitializeWithFile`/`Stream`/`Item`, `AssocQueryString`):
//!
//! 1. **Quien previsualiza una extension**: `AssocQueryStringW` con
//!    `ASSOCSTR_SHELLEXTENSION` y el IID de `IPreviewHandler`; mira la
//!    extension, su ProgID y `SystemFileAssociations`. Se guarda por
//!    extension (el registro no cambia mientras se usa la app).
//! 2. **Office solo mientras hay pin** (el usuario, 10-oct: «no es buena
//!    idea tener Office abierto, consume RAM en segundo plano y va contra la
//!    filosofia de la app»). Los de Office solo traen `LocalServer32` (van
//!    SIEMPRE fuera de nuestro proceso: si el previsualizador se cae, PixPin
//!    sigue). Cada vista pide su fabrica (`CoGetClassObject`) y su objeto en
//!    el momento, y al cerrar se descarga (`Unload`), se suelta TODO y se
//!    llama a `CoFreeUnusedLibrariesEx`: sin pines abiertos el hilo no
//!    sujeta nada de COM y el servidor de Office se va. Nada de fabricas
//!    guardadas ni previsualizadores de repuesto: cada vista espera a que
//!    Office arranque (medido el 10-oct: 5-9 s). Al cerrar, con el objeto
//!    ya suelto, un `LockServer(true)`/`LockServer(false)` sobre la fabrica
//!    de esa vista le hace mirar al servidor si le queda alguien: sin ese
//!    aviso el de Word se quedaba en marcha mas de diez minutos (mas alla
//!    de la limpieza de COM: no era una referencia nuestra); con el, Word y
//!    PowerPoint se van a los 3-8 s y Excel al soltar el objeto.
//! 3. **Iniciar**: con la ruta (`IInitializeWithFile`), si no con un flujo
//!    del fichero (`IInitializeWithStream`), si no con el elemento del Shell
//!    (`IInitializeWithItem`). Luego el marco (`IObjectWithSite::SetSite`
//!    con un `IPreviewHandlerFrame` propio, que recoge Esc y F11 aunque el
//!    teclado lo tenga el previsualizador), `SetWindow`, `DoPreview`, y
//!    `SetRect` al estirar.
//!
//! # El hilo «office»
//!
//! Las ventanas que alojan las vistas y los objetos COM viven en UN hilo
//! propio, STA, con su bucle de mensajes. Un Office colgado solo cuelga ese
//! hilo: el de la interfaz de PixPin sigue. Las ordenes llegan por un canal
//! y despiertan al hilo con un mensaje a su buzon (una ventana
//! `HWND_MESSAGE`); el bucle vacia el canal antes de cada `GetMessage`, asi
//! que lo que llegue mientras una llamada a Office bombea mensajes (el bucle
//! modal de COM) se atiende al volver, nunca a medias.
//!
//! Nunca se mata un proceso de Office ni se le pide `Quit`: si algo tarda,
//! se dice en el pin y el usuario tiene «Abrir con su programa».
//!
//! # Lo que viene de internet
//!
//! Un fichero con la marca de la web (`Zone.Identifier`, zona 3 o mas) no se
//! previsualiza: Windows lo bloquea en el Explorador por seguridad (una
//! vista previa de un fichero de internet puede filtrar credenciales), y
//! aqui se respeta. [`bloqueado_por_internet`] lo dice; el pin lo cuenta y
//! ofrece abrirlo con su programa.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use pixpin_geom::Rect;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, S_FALSE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateFontW, CreateSolidBrush,
    DEFAULT_CHARSET, DT_CALCRECT, DT_CENTER, DT_NOPREFIX, DT_WORDBREAK, DeleteObject, DrawTextW,
    EndPaint, FF_DONTCARE, FW_NORMAL, FillRect, GetDC, HGDIOBJ, InvalidateRect, OUT_DEFAULT_PRECIS,
    PAINTSTRUCT, ReleaseDC, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::System::Com::{
    CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED, CoFreeUnusedLibrariesEx, CoGetClassObject,
    CoInitializeEx, CoUninitialize, IClassFactory, IStream, STGM_READ, STGM_SHARE_DENY_NONE,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Ole::IObjectWithSite;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_ESCAPE, VK_F11};
use windows::Win32::UI::Shell::PropertiesSystem::{IInitializeWithFile, IInitializeWithStream};
use windows::Win32::UI::Shell::{
    ASSOCF_INIT_DEFAULTTOSTAR, ASSOCF_NOTRUNCATE, ASSOCSTR_SHELLEXTENSION, AssocQueryStringW,
    IInitializeWithItem, IPreviewHandler, IPreviewHandlerFrame, IPreviewHandlerFrame_Impl,
    IPreviewHandlerVisuals, IShellItem, PREVIEWHANDLERFRAMEINFO, SHCreateItemFromParsingName,
    SHCreateStreamOnFileEx,
};
use windows::Win32::UI::WindowsAndMessaging::{
    ACCEL, CreateAcceleratorTableW, CreateWindowExW, DefWindowProcW, DestroyWindow,
    DispatchMessageW, FVIRTKEY, GA_ROOT, GW_CHILD, GWL_STYLE, GetAncestor, GetClientRect,
    GetForegroundWindow, GetMessageW, GetWindow, GetWindowLongPtrW, GetWindowThreadProcessId,
    HACCEL, HWND_MESSAGE, IDC_ARROW, IsWindow, KillTimer, LoadCursorW, MSG, PostMessageW,
    RegisterClassW, SWP_NOACTIVATE, SWP_NOZORDER, SetForegroundWindow, SetTimer, SetWindowLongPtrW,
    SetWindowPos, TranslateMessage, WINDOW_EX_STYLE, WM_APP, WM_ERASEBKGND, WM_KEYDOWN, WM_PAINT,
    WM_SYSKEYDOWN, WM_TIMER, WNDCLASSW, WS_CHILD, WS_CLIPCHILDREN, WS_VISIBLE,
};
use windows::core::{GUID, HSTRING, PCWSTR, implement, w};

/// El IID de `IPreviewHandler`, como lo pide `AssocQueryStringW`.
const IID_VISTA_PREVIA: &str = "{8895b1c6-b41f-4c1c-a562-0d564250836f}";
/// Lo que se espera a que el previsualizador diga «listo» antes de avisar en
/// el pin de que tarda (no se corta nada: solo se dice).
pub const TARDA: Duration = Duration::from_secs(4);
/// Metido en una llamada a Office mas que esto: el hilo esta atascado y lo
/// que se abra mientras va por su camino de siempre.
const ATASCADO_MS: u64 = 8_000;
/// El aviso al buzon del hilo: hay ordenes en el canal.
const MSG_ORDEN: u32 = WM_APP + 0x31;
/// El temporizador del «tarda» de cada vista.
const RELOJ_TARDA: usize = 1;

// Los colores del cartel (los del lector: papel oscuro, letra clara). Un
// COLORREF es 0x00BBGGRR.
const FONDO: u32 = 0x0016_1312;
const LETRA: u32 = 0x00e6_e2e4;

// ---------------------------------------------------------------------------
// Quien previsualiza cada extension

static MANEJADORES: Mutex<Option<HashMap<String, Option<GUID>>>> = Mutex::new(None);

/// El GUID de un texto como `{84F66100-FF7C-4fb4-B0C0-02CD7FB668FE}` (con o
/// sin llaves y espacios alrededor).
pub fn guid_de_texto(texto: &str) -> Option<GUID> {
    let t = texto.trim().trim_start_matches('{').trim_end_matches('}');
    GUID::try_from(t).ok()
}

/// La extension de una ruta, en minusculas y sin el punto ("" si no tiene).
pub fn extension_de(ruta: &Path) -> String {
    ruta.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

fn buscar_manejador(ext: &str) -> Option<GUID> {
    let asoc = HSTRING::from(format!(".{ext}"));
    let iid = HSTRING::from(IID_VISTA_PREVIA);
    let mut buf = [0u16; 128];
    let mut largo = buf.len() as u32;
    // SAFETY: las cadenas viven como HSTRING durante la llamada; el buffer es
    // propio y `largo` dice cuantos caracteres caben.
    let hr = unsafe {
        AssocQueryStringW(
            ASSOCF_INIT_DEFAULTTOSTAR | ASSOCF_NOTRUNCATE,
            ASSOCSTR_SHELLEXTENSION,
            &asoc,
            &iid,
            Some(windows::core::PWSTR(buf.as_mut_ptr())),
            &mut largo,
        )
    };
    if hr.is_err() {
        return None;
    }
    let n = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    guid_de_texto(&String::from_utf16_lossy(&buf[..n]))
}

/// **El previsualizador registrado para esta extension** (sin punto), o
/// `None`. Se mira el registro una vez por extension.
pub fn manejador_de_extension(ext: &str) -> Option<GUID> {
    let ext = ext.trim_start_matches('.').to_ascii_lowercase();
    if ext.is_empty() {
        return None;
    }
    if let Ok(cache) = MANEJADORES.lock()
        && let Some(m) = cache.as_ref().and_then(|c| c.get(&ext))
    {
        return *m;
    }
    let m = buscar_manejador(&ext);
    tracing::debug!(%ext, manejador = ?m, "vista previa: manejador buscado");
    if let Ok(mut cache) = MANEJADORES.lock() {
        cache.get_or_insert_with(HashMap::new).insert(ext, m);
    }
    m
}

/// El previsualizador para este fichero, por su extension.
pub fn manejador_de(ruta: &Path) -> Option<GUID> {
    manejador_de_extension(&extension_de(ruta))
}

/// **La ruta como la escribe Windows**: completa y con `\`. El de Word no
/// acepta `E:/carpeta/x.docx` (medido el 10-oct: `0x86420003` con barras
/// `/`, y la misma ruta con `\` se ve); a otros les da igual.
pub fn ruta_para_windows(ruta: &Path) -> PathBuf {
    let completa = std::path::absolute(ruta).unwrap_or_else(|_| ruta.to_path_buf());
    PathBuf::from(completa.to_string_lossy().replace('/', "\\"))
}

// ---------------------------------------------------------------------------
// La marca de la web

/// La zona de un `Zone.Identifier` (`[ZoneTransfer]` / `ZoneId=3`).
pub fn zona_del_texto(texto: &str) -> Option<u32> {
    texto.lines().find_map(|l| {
        let (k, v) = l.split_once('=')?;
        if k.trim().eq_ignore_ascii_case("ZoneId") {
            v.trim().parse().ok()
        } else {
            None
        }
    })
}

/// Si una zona es «de internet» para la vista previa: 3 (Internet) o 4
/// (sitios restringidos), como hace Windows.
pub fn zona_bloquea(zona: Option<u32>) -> bool {
    zona.is_some_and(|z| z >= 3)
}

/// **Si el fichero lleva la marca de la web**: se lee su flujo
/// `Zone.Identifier` (en NTFS; en otro sistema de ficheros no hay y no lo
/// esta). No se toca ni se borra.
pub fn bloqueado_por_internet(ruta: &Path) -> bool {
    let mut flujo = ruta.as_os_str().to_owned();
    flujo.push(":Zone.Identifier");
    let zona = std::fs::read(PathBuf::from(flujo))
        .ok()
        .and_then(|b| zona_del_texto(&String::from_utf8_lossy(&b)));
    zona_bloquea(zona)
}

// ---------------------------------------------------------------------------
// Lo que se comparte con quien abrio la vista

/// En que va una vista previa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Estado {
    /// Pedida; el previsualizador todavia no ha dicho «listo».
    Abriendo,
    /// Pintada. `ms`, lo que tardo desde la orden (con Office arrancando:
    /// no queda ninguno en marcha entre pines).
    Lista { ms: u64 },
    /// No se pudo (el motivo, para el registro).
    Fallo(String),
    /// Solo un cartel (lo de internet): no hay previsualizador.
    Cartel,
}

struct Compartido {
    estado: Mutex<Estado>,
    /// Las teclas que el previsualizador nos paso (Esc, F11).
    teclas: Mutex<Vec<u32>>,
    /// La ventana de quien abrio, para despertarla.
    padre: isize,
    /// Cerrada en el hilo (la ventana del cartel ya no esta).
    cerrada: Mutex<bool>,
}

/// Los textos del cartel, ya traducidos (este crate no sabe de idiomas).
#[derive(Debug, Clone, Default)]
pub struct Carteles {
    pub abriendo: String,
    pub tarda: String,
    pub fallo: String,
}

enum Orden {
    Abrir {
        id: u64,
        padre: isize,
        ruta: PathBuf,
        clsid: Option<GUID>,
        area: Rect,
        carteles: Carteles,
        c: Arc<Compartido>,
    },
    Mover {
        id: u64,
        area: Rect,
    },
    Cerrar {
        id: u64,
    },
    SoltarTodo {
        hecho: Sender<()>,
    },
}

struct Hilo {
    ordenes: Sender<Orden>,
    buzon: isize,
}

static HILO: OnceLock<Option<Hilo>> = OnceLock::new();
static SIGUIENTE: AtomicU64 = AtomicU64::new(1);
/// Desde cuando (ms del reloj de la app) esta el hilo metido en una llamada
/// a un previsualizador; 0, en ninguna.
static EN_LLAMADA_DESDE: AtomicU64 = AtomicU64::new(0);

fn ms_desde_el_arranque() -> u64 {
    static CERO: OnceLock<Instant> = OnceLock::new();
    CERO.get_or_init(Instant::now).elapsed().as_millis() as u64 + 1
}

/// **El hilo de las vistas esta atascado**: una llamada a un previsualizador
/// lleva mas de ocho segundos sin volver. Lo que se abra mientras debe ir a
/// su camino de siempre (el lector propio, su programa).
pub fn atascado() -> bool {
    let desde = EN_LLAMADA_DESDE.load(Ordering::Relaxed);
    desde != 0 && ms_desde_el_arranque().saturating_sub(desde) > ATASCADO_MS
}

fn hilo() -> Option<&'static Hilo> {
    HILO.get_or_init(|| {
        let (tx, rx) = channel::<Orden>();
        let (listo_tx, listo_rx) = channel::<isize>();
        let lanzado = std::thread::Builder::new()
            .name("office".into())
            .spawn(move || bucle(rx, listo_tx));
        if let Err(e) = lanzado {
            tracing::warn!(?e, "vista previa: no se pudo lanzar el hilo");
            return None;
        }
        match listo_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(buzon) if buzon != 0 => Some(Hilo { ordenes: tx, buzon }),
            _ => {
                tracing::warn!("vista previa: el hilo no arranco");
                None
            }
        }
    })
    .as_ref()
}

fn mandar(orden: Orden) -> bool {
    let Some(h) = hilo() else { return false };
    if h.ordenes.send(orden).is_err() {
        return false;
    }
    // SAFETY: PostMessageW a la ventana buzon del hilo; si ya no existe,
    // falla y no pasa nada (la orden se queda en el canal).
    unsafe {
        let _ = PostMessageW(
            Some(HWND(h.buzon as *mut _)),
            MSG_ORDEN,
            WPARAM(0),
            LPARAM(0),
        );
    }
    true
}

/// **Al salir de la app**: descarga y suelta los previsualizadores de los
/// pines que sigan abiertos, para que los servidores de Office se vayan ya
/// y no cuando COM note que el proceso murio (minutos). Espera como mucho `espera`. Si el
/// hilo nunca arranco, no hace nada (ni lo arranca).
pub fn soltar_todo(espera: Duration) {
    if !matches!(HILO.get(), Some(Some(_))) {
        return;
    }
    let (tx, rx) = channel();
    if mandar(Orden::SoltarTodo { hecho: tx }) && rx.recv_timeout(espera).is_err() {
        tracing::warn!("vista previa: no dio tiempo a soltar los previsualizadores al salir");
    }
}

/// Le quita a la ventana del pin el pintar encima de sus hijas
/// (`WS_CLIPCHILDREN`): asi su marco se pinta sin tapar la vista.
fn recortar_hijas(padre: HWND) {
    // SAFETY: lee y escribe el estilo de una ventana de este hilo.
    unsafe {
        let estilo = GetWindowLongPtrW(padre, GWL_STYLE);
        SetWindowLongPtrW(padre, GWL_STYLE, estilo | WS_CLIPCHILDREN.0 as isize);
    }
}

/// **Pinta el fondo de la ventana del pin** (lo que no tapa la vista: el
/// borde por donde se estira) del color del cartel. La ventana es «llana»
/// (sin `WS_EX_NOREDIRECTIONBITMAP`) y nadie mas la pinta.
pub fn pintar_fondo(hwnd: HWND) {
    // SAFETY: GetDC/ReleaseDC emparejados sobre una ventana propia; la
    // brocha se crea y se borra aqui. Con WS_CLIPCHILDREN la vista no se toca.
    unsafe {
        let mut r = RECT::default();
        if GetClientRect(hwnd, &mut r).is_err() {
            return;
        }
        let dc = GetDC(Some(hwnd));
        if dc.is_invalid() {
            return;
        }
        let brocha = CreateSolidBrush(COLORREF(FONDO));
        FillRect(dc, &r, brocha);
        let _ = DeleteObject(HGDIOBJ(brocha.0));
        ReleaseDC(Some(hwnd), dc);
    }
}

/// **Una vista previa abierta** en una ventana de la app. Se cierra con
/// [`VistaPrevia::cerrar`] (o al soltarla, sin esperar).
pub struct VistaPrevia {
    id: u64,
    c: Arc<Compartido>,
    desde: Instant,
    cerrada: bool,
}

impl VistaPrevia {
    /// Abre la vista de `ruta` dentro de `padre`, en `area` (coordenadas de
    /// su zona de cliente). Vuelve en seguida: el cartel «abriendo» sale ya y
    /// la vista llega cuando el previsualizador la tenga. `None` si no hay
    /// previsualizador para esto o el hilo no esta.
    pub fn abrir(padre: HWND, ruta: &Path, area: Rect, carteles: Carteles) -> Option<VistaPrevia> {
        let clsid = manejador_de(ruta)?;
        let ruta = ruta_para_windows(ruta);
        Self::empezar(padre, &ruta, Some(clsid), area, carteles, Estado::Abriendo)
    }

    /// Solo el cartel `texto`, sin previsualizador (lo que viene de internet).
    pub fn solo_cartel(padre: HWND, area: Rect, texto: &str) -> Option<VistaPrevia> {
        let carteles = Carteles {
            abriendo: texto.to_string(),
            tarda: texto.to_string(),
            fallo: texto.to_string(),
        };
        Self::empezar(padre, Path::new(""), None, area, carteles, Estado::Cartel)
    }

    fn empezar(
        padre: HWND,
        ruta: &Path,
        clsid: Option<GUID>,
        area: Rect,
        carteles: Carteles,
        estado: Estado,
    ) -> Option<VistaPrevia> {
        recortar_hijas(padre);
        let id = SIGUIENTE.fetch_add(1, Ordering::Relaxed);
        let c = Arc::new(Compartido {
            estado: Mutex::new(estado),
            teclas: Mutex::new(Vec::new()),
            padre: padre.0 as isize,
            cerrada: Mutex::new(false),
        });
        let orden = Orden::Abrir {
            id,
            padre: padre.0 as isize,
            ruta: ruta.to_path_buf(),
            clsid,
            area,
            carteles,
            c: Arc::clone(&c),
        };
        if !mandar(orden) {
            return None;
        }
        Some(VistaPrevia {
            id,
            c,
            desde: Instant::now(),
            cerrada: false,
        })
    }

    /// La vista, a otro sitio o tamano dentro de la ventana.
    pub fn mover(&self, area: Rect) {
        mandar(Orden::Mover { id: self.id, area });
    }

    pub fn estado(&self) -> Estado {
        self.c
            .estado
            .lock()
            .map(|e| e.clone())
            .unwrap_or(Estado::Fallo("envenenado".into()))
    }

    /// Lo que lleva abierta (o abriendose).
    pub fn edad(&self) -> Duration {
        self.desde.elapsed()
    }

    /// Las teclas que el previsualizador paso al marco (Esc y F11), en orden.
    pub fn tomar_teclas(&self) -> Vec<u32> {
        self.c
            .teclas
            .lock()
            .map(|mut t| std::mem::take(&mut *t))
            .unwrap_or_default()
    }

    /// **Cierra la vista** y espera hasta `espera` a que el hilo la quite.
    /// `false` si no le dio tiempo (Office colgado): entonces quien llama NO
    /// debe destruir su ventana —eso esperaria al hilo atascado—, sino
    /// esconderla.
    pub fn cerrar(mut self, espera: Duration) -> bool {
        self.cerrada = true;
        mandar(Orden::Cerrar { id: self.id });
        let t = Instant::now();
        loop {
            if self.c.cerrada.lock().map(|c| *c).unwrap_or(true) {
                return true;
            }
            if t.elapsed() >= espera {
                tracing::warn!(id = self.id, "vista previa: el hilo no la cerro a tiempo");
                return false;
            }
            // Quitar la ventana hija le MANDA mensajes a la del pin (que es
            // de este hilo): hay que atenderlos o el hilo «office» se queda
            // esperando y aqui nadie avanza. `PeekMessage` sin quitar nada
            // atiende solo esos; lo que haya en la cola sigue para el bucle.
            let mut msg = MSG::default();
            // SAFETY: consulta de la cola de este hilo con un MSG propio.
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::PeekMessageW(
                    &mut msg,
                    None,
                    0,
                    0,
                    windows::Win32::UI::WindowsAndMessaging::PM_NOREMOVE,
                );
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

impl Drop for VistaPrevia {
    fn drop(&mut self) {
        if !self.cerrada {
            mandar(Orden::Cerrar { id: self.id });
        }
    }
}

// ---------------------------------------------------------------------------
// El marco: lo que el previsualizador le pregunta a quien lo aloja

#[implement(IPreviewHandlerFrame)]
struct Marco {
    c: Arc<Compartido>,
    aceleradores: isize,
}

/// Las teclas que el marco quiere: Esc (salir de pantalla completa o
/// cerrar) y F11 (pantalla completa).
const TECLAS_DEL_MARCO: [u16; 2] = [VK_ESCAPE.0, VK_F11.0];

/// Si un mensaje es una de las teclas del marco (`Some(vk)`).
pub fn tecla_del_marco(mensaje: u32, vk: usize) -> Option<u32> {
    if (mensaje == WM_KEYDOWN || mensaje == WM_SYSKEYDOWN)
        && TECLAS_DEL_MARCO.contains(&(vk as u16))
    {
        Some(vk as u32)
    } else {
        None
    }
}

impl IPreviewHandlerFrame_Impl for Marco_Impl {
    fn GetWindowContext(&self) -> windows::core::Result<PREVIEWHANDLERFRAMEINFO> {
        // La tabla de aceleradores dice que teclas nos interesan: el
        // previsualizador solo pasa esas a `TranslateAccelerator`.
        Ok(PREVIEWHANDLERFRAMEINFO {
            haccel: HACCEL(self.aceleradores as *mut _),
            cAccelEntries: TECLAS_DEL_MARCO.len() as u32,
        })
    }

    fn TranslateAccelerator(&self, pmsg: *const MSG) -> windows::core::Result<()> {
        if pmsg.is_null() {
            return Err(windows::core::Error::from_hresult(S_FALSE));
        }
        // SAFETY: el previsualizador pasa un MSG valido durante la llamada.
        let m = unsafe { *pmsg };
        let Some(vk) = tecla_del_marco(m.message, m.wParam.0) else {
            return Err(windows::core::Error::from_hresult(S_FALSE));
        };
        if let Ok(mut t) = self.c.teclas.lock() {
            t.push(vk);
        }
        crate::overlay::despertar(self.c.padre);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// El hilo

/// Una vista viva en el hilo.
struct Viva {
    cartel: HWND,
    manejador: Option<IPreviewHandler>,
    /// La fabrica de la que salio, sujeta SOLO mientras vive la vista (sin
    /// `LockServer`): al cerrar sirve para avisar al servidor de que ya no
    /// queda nadie (ver [`cerrar_en_el_hilo`]) y se suelta con ella.
    fabrica: Option<IClassFactory>,
    c: Arc<Compartido>,
}

/// Lo que vive en el hilo: las vistas abiertas y nada mas (ni fabricas ni
/// repuestos: sin pines, ningun objeto de Office).
struct EstadoHilo {
    vistas: HashMap<u64, Viva>,
    aceleradores: isize,
}

thread_local! {
    /// El cartel de cada ventana: lo que dice, lo que dira si tarda y si
    /// sigue esperando. Lo lee el WndProc; se cambia en tramos cortos.
    static CARTELES: std::cell::RefCell<HashMap<isize, (String, String, bool)>> = std::cell::RefCell::new(HashMap::new());
}

fn poner_cartel(cartel: HWND, texto: &str, esperando: bool) {
    CARTELES.with(|c| {
        if let Some(x) = c.borrow_mut().get_mut(&(cartel.0 as isize)) {
            x.0 = texto.to_string();
            x.2 = esperando;
        }
    });
    // SAFETY: repintar una ventana de este hilo.
    unsafe {
        let _ = InvalidateRect(Some(cartel), None, true);
    }
}

extern "system" fn proc_cartel(
    hwnd: HWND,
    mensaje: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match mensaje {
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            pintar_cartel(hwnd);
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == RELOJ_TARDA => {
            // SAFETY: el temporizador es de esta ventana.
            unsafe {
                let _ = KillTimer(Some(hwnd), RELOJ_TARDA);
            }
            let tarda = CARTELES.with(|c| {
                c.borrow()
                    .get(&(hwnd.0 as isize))
                    .and_then(|(_, t, esperando)| esperando.then(|| t.clone()))
            });
            if let Some(t) = tarda {
                tracing::warn!(
                    "vista previa: el previsualizador tarda mas de {} s",
                    TARDA.as_secs()
                );
                poner_cartel(hwnd, &t, true);
            }
            LRESULT(0)
        }
        // SAFETY: lo demas, a lo de siempre.
        _ => unsafe { DefWindowProcW(hwnd, mensaje, wparam, lparam) },
    }
}

fn pintar_cartel(hwnd: HWND) {
    let texto = CARTELES
        .with(|c| c.borrow().get(&(hwnd.0 as isize)).map(|x| x.0.clone()))
        .unwrap_or_default();
    // SAFETY: BeginPaint/EndPaint emparejados en el WM_PAINT de una ventana
    // de este hilo; la brocha y la letra se crean y se borran aqui, y la
    // letra de antes se vuelve a poner antes de borrar la nuestra.
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let dc = BeginPaint(hwnd, &mut ps);
        let mut r = RECT::default();
        let _ = GetClientRect(hwnd, &mut r);
        let brocha = CreateSolidBrush(COLORREF(FONDO));
        FillRect(dc, &r, brocha);
        let _ = DeleteObject(HGDIOBJ(brocha.0));
        if !texto.is_empty() {
            let dpi = GetDpiForWindow(hwnd).max(96) as i32;
            let letra = CreateFontW(
                -(15 * dpi / 96),
                0,
                0,
                0,
                FW_NORMAL.0 as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                FF_DONTCARE.0 as u32,
                w!("Segoe UI"),
            );
            let antes = SelectObject(dc, HGDIOBJ(letra.0));
            SetBkMode(dc, TRANSPARENT);
            SetTextColor(dc, COLORREF(LETRA));
            let mut t: Vec<u16> = texto.encode_utf16().collect();
            let margen = 32 * dpi / 96;
            let mut caja = RECT {
                left: r.left + margen,
                top: 0,
                right: (r.right - margen).max(r.left + margen + 10),
                bottom: 0,
            };
            DrawTextW(
                dc,
                &mut t,
                &mut caja,
                DT_CALCRECT | DT_CENTER | DT_WORDBREAK | DT_NOPREFIX,
            );
            let alto = caja.bottom - caja.top;
            caja.left = r.left + margen;
            caja.right = (r.right - margen).max(r.left + margen + 10);
            caja.top = (r.bottom - alto) / 2;
            caja.bottom = caja.top + alto;
            DrawTextW(
                dc,
                &mut t,
                &mut caja,
                DT_CENTER | DT_WORDBREAK | DT_NOPREFIX,
            );
            SelectObject(dc, antes);
            let _ = DeleteObject(HGDIOBJ(letra.0));
        }
        let _ = EndPaint(hwnd, &ps);
    }
}

extern "system" fn proc_buzon(hwnd: HWND, mensaje: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // El buzon no hace nada al recibir: las ordenes las lee el bucle (asi no
    // se atienden a medias dentro del bucle modal de una llamada COM).
    // SAFETY: lo de siempre para lo demas.
    unsafe { DefWindowProcW(hwnd, mensaje, wparam, lparam) }
}

fn registrar_clases() -> bool {
    // SAFETY: registro de dos clases con WndProc propio; los campos no
    // usados a cero, que es lo que la API espera.
    unsafe {
        let Ok(modulo) = GetModuleHandleW(None) else {
            return false;
        };
        let cartel = WNDCLASSW {
            lpfnWndProc: Some(proc_cartel),
            hInstance: modulo.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: w!("PixPinVistaPrevia"),
            ..Default::default()
        };
        let buzon = WNDCLASSW {
            lpfnWndProc: Some(proc_buzon),
            hInstance: modulo.into(),
            lpszClassName: w!("PixPinVistaPreviaBuzon"),
            ..Default::default()
        };
        RegisterClassW(&cartel) != 0 && RegisterClassW(&buzon) != 0
    }
}

fn bucle(rx: Receiver<Orden>, listo: Sender<isize>) {
    // SAFETY: el hilo es nuestro; se empareja con CoUninitialize al final.
    let com = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
    if !registrar_clases() {
        let _ = listo.send(0);
        return;
    }
    // SAFETY: ventana de solo mensajes de este hilo, de la clase de arriba.
    let buzon = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("PixPinVistaPreviaBuzon"),
            PCWSTR::null(),
            Default::default(),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            GetModuleHandleW(None).ok().map(|m| m.into()),
            None,
        )
    };
    let Ok(buzon) = buzon else {
        let _ = listo.send(0);
        return;
    };
    // La tabla de aceleradores del marco (Esc y F11), una para todas.
    let acel: Vec<ACCEL> = TECLAS_DEL_MARCO
        .iter()
        .enumerate()
        .map(|(i, vk)| ACCEL {
            fVirt: FVIRTKEY,
            key: *vk,
            cmd: i as u16 + 1,
        })
        .collect();
    // SAFETY: la tabla se copia; el slice vive durante la llamada.
    let aceleradores = unsafe { CreateAcceleratorTableW(&acel) }
        .map(|h| h.0 as isize)
        .unwrap_or(0);
    let _ = listo.send(buzon.0 as isize);
    tracing::info!("vista previa: hilo «office» en marcha");

    let mut e = EstadoHilo {
        vistas: HashMap::new(),
        aceleradores,
    };
    let mut msg = MSG::default();
    loop {
        // Las ordenes, antes de dormir: las que llegaron durante una llamada
        // (cuyo aviso se comio el bucle modal de COM) tambien.
        let mut pendientes: Vec<Orden> = rx.try_iter().collect();
        // De varios «mover» seguidos de una vista, el ultimo basta.
        let mut i = 0;
        while i < pendientes.len() {
            let repetido = matches!(&pendientes[i], Orden::Mover { id, .. }
                if pendientes[i + 1..].iter().any(|o| matches!(o, Orden::Mover { id: otro, .. } if otro == id)));
            if repetido {
                pendientes.remove(i);
            } else {
                i += 1;
            }
        }
        for o in pendientes {
            atender(&mut e, o);
        }
        // SAFETY: bucle de mensajes corriente de este hilo.
        let r = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if r.0 <= 0 {
            break;
        }
        // SAFETY: el mensaje viene de GetMessageW.
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    let ids: Vec<u64> = e.vistas.keys().copied().collect();
    for id in ids {
        if let Some(v) = e.vistas.remove(&id) {
            cerrar_en_el_hilo(v);
        }
    }
    drop(e);
    if com {
        // SAFETY: empareja el CoInitializeEx de arriba; ya no vive nada COM.
        unsafe { CoUninitialize() };
    }
}

fn atender(e: &mut EstadoHilo, orden: Orden) {
    match orden {
        Orden::Abrir {
            id,
            padre,
            ruta,
            clsid,
            area,
            carteles,
            c,
        } => abrir_en_el_hilo(e, id, padre, &ruta, clsid, area, carteles, c),
        Orden::Mover { id, area } => {
            if let Some(v) = e.vistas.get(&id) {
                mover_en_el_hilo(v, area);
            }
        }
        Orden::Cerrar { id } => {
            if let Some(v) = e.vistas.remove(&id) {
                cerrar_en_el_hilo(v);
            }
        }
        Orden::SoltarTodo { hecho } => {
            let ids: Vec<u64> = e.vistas.keys().copied().collect();
            for id in ids {
                if let Some(v) = e.vistas.remove(&id) {
                    cerrar_en_el_hilo(v);
                }
            }
            let _ = hecho.send(());
        }
    }
}

/// Lo que dura una llamada a un previsualizador queda apuntado (para
/// [`atascado`]).
fn en_llamada<T>(f: impl FnOnce() -> T) -> T {
    EN_LLAMADA_DESDE.store(ms_desde_el_arranque(), Ordering::Relaxed);
    let r = f();
    EN_LLAMADA_DESDE.store(0, Ordering::Relaxed);
    r
}

/// Si un error de COM es «el servidor se murio o no esta» (se pide otra vez,
/// una sola).
pub fn servidor_caido(hr: i32) -> bool {
    const RPC_E_SERVERDIED: i32 = 0x8001_0007_u32 as i32;
    const RPC_E_SERVERDIED_DNE: i32 = 0x8001_0012_u32 as i32;
    const RPC_E_DISCONNECTED: i32 = 0x8001_0108_u32 as i32;
    const RPC_S_SERVER_UNAVAILABLE: i32 = 0x8007_06BA_u32 as i32;
    const RPC_S_CALL_FAILED: i32 = 0x8007_06BE_u32 as i32;
    const CO_E_OBJNOTCONNECTED: i32 = 0x8004_01FD_u32 as i32;
    matches!(
        hr,
        RPC_E_SERVERDIED
            | RPC_E_SERVERDIED_DNE
            | RPC_E_DISCONNECTED
            | RPC_S_SERVER_UNAVAILABLE
            | RPC_S_CALL_FAILED
            | CO_E_OBJNOTCONNECTED
    )
}

/// La fabrica del previsualizador, pedida ahora y para esta vista sola (no
/// se guarda ni se sujeta su servidor con `LockServer`).
fn fabrica(clsid: GUID) -> windows::core::Result<IClassFactory> {
    // SAFETY: el GUID vive durante la llamada; servidor local, fuera de
    // nuestro proceso.
    en_llamada(|| unsafe { CoGetClassObject(&clsid, CLSCTX_LOCAL_SERVER, None) })
}

/// Que COM suelte ya lo que pueda de lo que se dejo de usar (los proxys y
/// las DLL de COM sin objetos vivos), para que el servidor de Office se
/// quede sin clientes cuanto antes.
fn soltar_lo_que_sobra() {
    // SAFETY: sin precondiciones; solo libera lo que COM ya no usa.
    unsafe { CoFreeUnusedLibrariesEx(0, Some(0)) };
}

fn rect_de(area: Rect) -> RECT {
    RECT {
        left: 0,
        top: 0,
        right: area.ancho as i32,
        bottom: area.alto as i32,
    }
}

/// Las maneras de darle el fichero a un previsualizador, en el orden en que
/// se prueban (la ruta es lo mas barato).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Manera {
    Ruta,
    Flujo,
    Elemento,
}

impl Manera {
    fn nombre(self) -> &'static str {
        match self {
            Manera::Ruta => "ruta",
            Manera::Flujo => "flujo",
            Manera::Elemento => "elemento",
        }
    }
}

/// Inicia `m` de una manera. `Ok(false)` si no sabe de esa manera.
fn iniciar_de(m: &IPreviewHandler, ruta: &Path, manera: Manera) -> windows::core::Result<bool> {
    use windows::core::Interface;
    let r = HSTRING::from(ruta.as_os_str());
    match manera {
        Manera::Ruta => {
            let Ok(i) = m.cast::<IInitializeWithFile>() else {
                return Ok(false);
            };
            // SAFETY: la ruta vive como HSTRING durante la llamada.
            en_llamada(|| unsafe { i.Initialize(&r, STGM_READ.0) })?;
        }
        Manera::Flujo => {
            let Ok(i) = m.cast::<IInitializeWithStream>() else {
                return Ok(false);
            };
            // SAFETY: idem; el flujo lo sujeta el previsualizador si lo quiere.
            let flujo: IStream = unsafe {
                SHCreateStreamOnFileEx(&r, STGM_READ.0 | STGM_SHARE_DENY_NONE.0, 0, false, None)
            }?;
            // SAFETY: interfaz y flujo validos.
            en_llamada(|| unsafe { i.Initialize(&flujo, STGM_READ.0) })?;
        }
        Manera::Elemento => {
            let Ok(i) = m.cast::<IInitializeWithItem>() else {
                return Ok(false);
            };
            // SAFETY: la ruta vive durante la llamada.
            let elemento: IShellItem = unsafe { SHCreateItemFromParsingName(&r, None) }?;
            // SAFETY: interfaz y elemento validos.
            en_llamada(|| unsafe { i.Initialize(&elemento, STGM_READ.0) })?;
        }
    }
    Ok(true)
}

/// **Un previsualizador nuevo, iniciado con el fichero**: de la fabrica, y
/// por la primera manera que acepte. Si una manera falla se prueba la
/// siguiente con OTRO objeto (`Initialize` se llama una vez por objeto;
/// reintentar sobre el mismo no es fiable).
fn crear_e_iniciar(
    f: &IClassFactory,
    ruta: &Path,
) -> windows::core::Result<(IPreviewHandler, &'static str)> {
    let mut ultimo: Option<windows::core::Error> = None;
    for manera in [Manera::Ruta, Manera::Flujo, Manera::Elemento] {
        // SAFETY: fabrica valida; sin agregacion.
        let m: IPreviewHandler = en_llamada(|| unsafe { f.CreateInstance(None) })?;
        match iniciar_de(&m, ruta, manera) {
            Ok(true) => return Ok((m, manera.nombre())),
            Ok(false) => {}
            Err(err) => {
                // Un servidor caido no se arregla con otra manera.
                if servidor_caido(err.code().0) {
                    return Err(err);
                }
                tracing::info!(%err, manera = manera.nombre(), "vista previa: no acepto el fichero asi");
                ultimo = Some(err);
            }
        }
    }
    Err(ultimo.unwrap_or_else(|| {
        windows::core::Error::from_hresult(windows::Win32::Foundation::E_NOINTERFACE)
    }))
}

#[allow(clippy::too_many_arguments)]
fn abrir_en_el_hilo(
    e: &mut EstadoHilo,
    id: u64,
    padre: isize,
    ruta: &Path,
    clsid: Option<GUID>,
    area: Rect,
    carteles: Carteles,
    c: Arc<Compartido>,
) {
    let t0 = Instant::now();
    let padre = HWND(padre as *mut _);
    // SAFETY: consulta de solo lectura.
    if !unsafe { IsWindow(Some(padre)) }.as_bool() {
        let _ = c.cerrada.lock().map(|mut x| *x = true);
        return;
    }
    // La ventana del cartel: hija de la del pin (de otro hilo), de este hilo.
    // SAFETY: clase registrada en este hilo; el padre existe.
    let cartel = unsafe {
        CreateWindowExW(
            // Sin avisar al padre (de otro hilo) al nacer y morir: menos
            // esperas cruzadas entre los dos hilos.
            windows::Win32::UI::WindowsAndMessaging::WS_EX_NOPARENTNOTIFY,
            w!("PixPinVistaPrevia"),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN,
            area.x,
            area.y,
            area.ancho as i32,
            area.alto as i32,
            Some(padre),
            None,
            GetModuleHandleW(None).ok().map(|m| m.into()),
            None,
        )
    };
    let Ok(cartel) = cartel else {
        tracing::warn!("vista previa: no se pudo crear la ventana del cartel");
        let _ = c
            .estado
            .lock()
            .map(|mut x| *x = Estado::Fallo("sin ventana".into()));
        let _ = c.cerrada.lock().map(|mut x| *x = true);
        return;
    };
    CARTELES.with(|x| {
        x.borrow_mut().insert(
            cartel.0 as isize,
            (
                carteles.abriendo.clone(),
                carteles.tarda.clone(),
                clsid.is_some(),
            ),
        )
    });
    let mut viva = Viva {
        cartel,
        manejador: None,
        fabrica: None,
        c: Arc::clone(&c),
    };
    let Some(clsid) = clsid else {
        e.vistas.insert(id, viva);
        return;
    };
    // SAFETY: temporizador de una ventana de este hilo.
    unsafe {
        SetTimer(Some(cartel), RELOJ_TARDA, TARDA.as_millis() as u32, None);
    }
    // Se pinta ya el «abriendo»: lo de abajo puede tardar.
    // SAFETY: ventana de este hilo.
    unsafe {
        let _ = windows::Win32::Graphics::Gdi::UpdateWindow(cartel);
    }
    // SAFETY: consulta de solo lectura.
    let delante_antes = unsafe { GetForegroundWindow() };

    let ext = extension_de(ruta);
    let mut ms_fabrica = 0;
    let mut intento = 0;
    let hecho: windows::core::Result<(IPreviewHandler, &'static str, IClassFactory)> = loop {
        intento += 1;
        let tf = Instant::now();
        let r = (|| -> windows::core::Result<(IPreviewHandler, &'static str, IClassFactory)> {
            let f = fabrica(clsid)?;
            ms_fabrica = tf.elapsed().as_millis() as u64;
            let (m, como) = crear_e_iniciar(&f, ruta)?;
            Ok((m, como, f))
        })();
        match r {
            // El servidor se fue entre medias (otro cliente lo cerro, se
            // cayo): se pide otro, una vez.
            Err(err) if intento == 1 && servidor_caido(err.code().0) => {
                tracing::info!(%err, "vista previa: el servidor se habia ido, se pide otro");
            }
            // Recien arrancado, el servidor a veces dice «error sin
            // especificar» al primer fichero (medido el 10-oct con Word, y a
            // la segunda lo abrio): una vez mas, con otro objeto.
            // Lo mismo si Office estaba ocupado y rechazo la llamada
            // (RPC_E_CALL_REJECTED, medido con un .rtf mientras otro
            // programa usaba Word).
            Err(err)
                if intento == 1
                    && (err.code() == windows::Win32::Foundation::E_FAIL
                        || err.code().0 == 0x8001_0001_u32 as i32) =>
            {
                tracing::info!(%err, "vista previa: no pudo a la primera, otra vez");
            }
            r => break r,
        }
    };
    let ms_iniciar = t0.elapsed().as_millis() as u64;
    let (manejador, como, fabrica_de_la_vista) = match hecho {
        Ok(x) => x,
        Err(err) => {
            fallar(&viva, &carteles, format!("no se pudo iniciar: {err}"), &ext);
            // Lo que se llego a crear ya se solto: que COM lo libere.
            soltar_lo_que_sobra();
            e.vistas.insert(id, viva);
            return;
        }
    };
    use windows::core::Interface;
    // El marco: las teclas Esc y F11 aunque el foco lo tenga Office.
    let marco: IPreviewHandlerFrame = Marco {
        c: Arc::clone(&c),
        aceleradores: e.aceleradores,
    }
    .into();
    if let Ok(sitio) = manejador.cast::<IObjectWithSite>() {
        // SAFETY: interfaces validas.
        let _ = en_llamada(|| unsafe { sitio.SetSite(&marco) });
    }
    if let Ok(v) = manejador.cast::<IPreviewHandlerVisuals>() {
        // SAFETY: interfaz valida; muchos previsualizadores lo ignoran.
        let _ = en_llamada(|| unsafe {
            let _ = v.SetBackgroundColor(COLORREF(FONDO));
            v.SetTextColor(COLORREF(LETRA))
        });
    }
    let r = rect_de(area);
    // SAFETY: el cartel es una ventana viva de este hilo y el RECT vive
    // durante la llamada.
    let pintado = en_llamada(|| unsafe {
        manejador
            .SetWindow(cartel, &r)
            .and_then(|_| manejador.DoPreview())
    });
    let ms = t0.elapsed().as_millis() as u64;
    viva.manejador = Some(manejador);
    viva.fabrica = Some(fabrica_de_la_vista);
    match pintado {
        Ok(()) => {
            // SAFETY: el temporizador es del cartel.
            unsafe {
                let _ = KillTimer(Some(cartel), RELOJ_TARDA);
            }
            poner_cartel(cartel, "", false);
            let _ = c.estado.lock().map(|mut x| *x = Estado::Lista { ms });
            tracing::info!(%ext, ms, ms_fabrica, ms_iniciar, como, "vista previa: lista");
            devolver_el_primer_plano(delante_antes, cartel, padre);
        }
        Err(err) => fallar(&viva, &carteles, format!("DoPreview: {err}"), &ext),
    }
    e.vistas.insert(id, viva);
}

fn fallar(v: &Viva, carteles: &Carteles, motivo: String, ext: &str) {
    tracing::warn!(%ext, %motivo, "vista previa: no se pudo");
    // SAFETY: el temporizador es del cartel.
    unsafe {
        let _ = KillTimer(Some(v.cartel), RELOJ_TARDA);
    }
    poner_cartel(v.cartel, &carteles.fallo, false);
    let _ = v.c.estado.lock().map(|mut x| *x = Estado::Fallo(motivo));
}

/// **Si el previsualizador se quedo con el primer plano** al pintar (Word lo
/// hace a veces), se le devuelve a quien lo tenia (el Explorador, al abrir
/// con el espacio). Solo si lo tiene ahora el pin o el proceso del
/// previsualizador: si el usuario se fue a otra cosa, no se le molesta.
fn devolver_el_primer_plano(antes: HWND, cartel: HWND, padre: HWND) {
    // SAFETY: consultas de solo lectura y un SetForegroundWindow que, si
    // Windows lo niega, no hace nada.
    unsafe {
        let ahora = GetForegroundWindow();
        if antes.is_invalid() || ahora == antes || !IsWindow(Some(antes)).as_bool() {
            return;
        }
        let mut pid_ahora = 0;
        GetWindowThreadProcessId(ahora, Some(&mut pid_ahora));
        let mut pid_vista = 0;
        if let Ok(hija) = GetWindow(cartel, GW_CHILD) {
            GetWindowThreadProcessId(hija, Some(&mut pid_vista));
        }
        let del_pin = GetAncestor(ahora, GA_ROOT) == padre;
        if del_pin || (pid_vista != 0 && pid_ahora == pid_vista) {
            let _ = SetForegroundWindow(antes);
            tracing::info!("vista previa: el primer plano vuelve a donde estaba");
        }
    }
}

fn mover_en_el_hilo(v: &Viva, area: Rect) {
    let t = Instant::now();
    // SAFETY: el cartel es de este hilo; el RECT vive durante la llamada.
    unsafe {
        let _ = SetWindowPos(
            v.cartel,
            None,
            area.x,
            area.y,
            area.ancho as i32,
            area.alto as i32,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
    let ms_ventana = t.elapsed().as_millis() as u64;
    if let Some(m) = &v.manejador {
        let r = rect_de(area);
        // SAFETY: interfaz valida. RPC_E_CALL_REJECTED (ocupado) se ignora:
        // el siguiente mover lo arregla.
        if let Err(err) = en_llamada(|| unsafe { m.SetRect(&r) })
            && err.code().0 != 0x8001_010D_u32 as i32
        {
            tracing::debug!(%err, "vista previa: SetRect");
        }
    }
    let ms = t.elapsed().as_millis() as u64;
    if ms > 200 {
        tracing::info!(ms, ms_ventana, "vista previa: estirar tardo");
    }
}

/// **Cerrar una vista sin dejar Office detras**, en este orden: `Unload`
/// (el previsualizador suelta el fichero y su ventana), quitarle el marco
/// (`SetSite(None)`: deja de sujetar nuestro objeto), soltar el objeto (la
/// ultima referencia: el servidor se queda sin clientes y se va), quitar la
/// ventana del cartel y pedirle a COM que libere lo que sobre.
fn cerrar_en_el_hilo(v: Viva) {
    let t = Instant::now();
    if let Some(m) = &v.manejador {
        use windows::core::Interface;
        // SAFETY: interfaces validas; los errores al descargar dan igual.
        en_llamada(|| unsafe {
            let _ = m.Unload();
            if let Ok(sitio) = m.cast::<IObjectWithSite>() {
                let _ = sitio.SetSite(None);
            }
        });
    }
    drop(v.manejador);
    // Sin objetos nuestros ya en el servidor, un `LockServer(true)` y su
    // `LockServer(false)` le hacen mirar si le queda alguien: es cuando un
    // servidor COM decide irse. El de Excel ya se fue al soltar el objeto
    // (esto falla sin mas); el de Word, sin el aviso, se quedaba en marcha.
    if let Some(f) = v.fabrica {
        // SAFETY: interfaz valida; si el servidor ya no esta, falla y ya.
        let r = en_llamada(|| unsafe { f.LockServer(true).and_then(|_| f.LockServer(false)) });
        tracing::debug!(ok = r.is_ok(), "vista previa: aviso de cierre al servidor");
        drop(f);
    }
    CARTELES.with(|x| x.borrow_mut().remove(&(v.cartel.0 as isize)));
    // SAFETY: el cartel es de este hilo (si el padre ya murio, ya no existe
    // y esto falla sin mas).
    unsafe {
        let _ = DestroyWindow(v.cartel);
    }
    soltar_lo_que_sobra();
    let _ = v.c.cerrada.lock().map(|mut x| *x = true);
    tracing::info!(
        ms = t.elapsed().as_millis() as u64,
        "vista previa: cerrada y soltada"
    );
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_guid_se_lee_con_y_sin_llaves_y_lo_demas_no() {
        let g = guid_de_texto("{84F66100-FF7C-4fb4-B0C0-02CD7FB668FE}").expect("guid");
        assert_eq!(g, GUID::from_u128(0x84F66100_FF7C_4FB4_B0C0_02CD7FB668FE));
        assert_eq!(
            guid_de_texto(" 00020827-0000-0000-C000-000000000046 "),
            Some(GUID::from_u128(0x00020827_0000_0000_C000_000000000046))
        );
        // Casos negativos: vacio, a medias, con basura.
        assert_eq!(guid_de_texto(""), None);
        assert_eq!(guid_de_texto("{84F66100-FF7C}"), None);
        assert_eq!(
            guid_de_texto("{84F66100-FF7C-4fb4-B0C0-02CD7FB668FX}"),
            None
        );
    }

    #[test]
    fn la_marca_de_la_web_bloquea_desde_la_zona_tres() {
        let texto = "[ZoneTransfer]\r\nZoneId=3\r\nReferrerUrl=https://ejemplo\r\n";
        assert_eq!(zona_del_texto(texto), Some(3));
        assert!(zona_bloquea(zona_del_texto(texto)));
        assert!(zona_bloquea(Some(4)));
        // Casos negativos: intranet o equipo local, sin marca, marca rota.
        assert!(!zona_bloquea(Some(1)));
        assert!(!zona_bloquea(Some(2)));
        assert!(!zona_bloquea(None));
        assert_eq!(zona_del_texto("[ZoneTransfer]\nZoneId=x\n"), None);
        assert_eq!(zona_del_texto("HostUrl=about:internet"), None);
    }

    #[test]
    fn un_fichero_sin_marca_no_esta_bloqueado() {
        let dir = std::env::temp_dir().join(format!("pixpin-vista-previa-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let f = dir.join("a.docx");
        std::fs::write(&f, b"x").unwrap();
        assert!(!bloqueado_por_internet(&f));
        // Con la marca puesta a mano (NTFS: el flujo alterno).
        let mut flujo = f.as_os_str().to_owned();
        flujo.push(":Zone.Identifier");
        if std::fs::write(PathBuf::from(flujo), b"[ZoneTransfer]\r\nZoneId=3\r\n").is_ok() {
            assert!(bloqueado_por_internet(&f));
        }
        assert!(!bloqueado_por_internet(&dir.join("no-existe.docx")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn solo_esc_y_f11_pulsadas_son_del_marco() {
        assert_eq!(
            tecla_del_marco(WM_KEYDOWN, VK_ESCAPE.0 as usize),
            Some(VK_ESCAPE.0 as u32)
        );
        assert_eq!(
            tecla_del_marco(WM_SYSKEYDOWN, VK_F11.0 as usize),
            Some(VK_F11.0 as u32)
        );
        // Casos negativos: otra tecla, o la tecla al soltarla.
        assert_eq!(tecla_del_marco(WM_KEYDOWN, 0x41), None);
        assert_eq!(
            tecla_del_marco(
                windows::Win32::UI::WindowsAndMessaging::WM_KEYUP,
                VK_ESCAPE.0 as usize
            ),
            None
        );
    }

    #[test]
    fn los_errores_de_servidor_caido_se_reconocen_y_los_demas_no() {
        assert!(servidor_caido(0x8001_0108_u32 as i32));
        assert!(servidor_caido(0x8007_06BA_u32 as i32));
        // Casos negativos: «no implementado», «acceso denegado», ocupado.
        assert!(!servidor_caido(0x8000_4001_u32 as i32));
        assert!(!servidor_caido(0x8007_0005_u32 as i32));
        assert!(!servidor_caido(0x8001_010D_u32 as i32));
    }

    #[test]
    fn la_ruta_va_con_barras_de_windows() {
        assert_eq!(
            ruta_para_windows(Path::new("E:/pixpin-capturas/Guia de usuario.docx")),
            PathBuf::from(r"E:\pixpin-capturas\Guia de usuario.docx")
        );
        // Caso negativo: una ruta ya buena no cambia.
        assert_eq!(
            ruta_para_windows(Path::new(r"C:\a\b.xlsx")),
            PathBuf::from(r"C:\a\b.xlsx")
        );
        // Una relativa queda completa.
        assert!(ruta_para_windows(Path::new("b.xlsx")).is_absolute());
    }

    #[test]
    fn la_extension_va_en_minusculas_y_sin_punto() {
        assert_eq!(extension_de(Path::new(r"C:\x\Informe.XLSX")), "xlsx");
        assert_eq!(extension_de(Path::new("sin")), "");
        assert_eq!(manejador_de_extension(""), None);
    }

    /// Lo que dice el registro de este equipo (con Office instalado). No es
    /// una prueba de la logica: se mira a mano con `--ignored --nocapture`.
    #[test]
    #[ignore]
    fn manejadores_de_este_equipo() {
        for ext in [
            "docx", "doc", "rtf", "odt", "xlsx", "xls", "xlsb", "pptx", "ppt", "vsdx", "msg",
            "docm", "xlsm", "csv", "zip", "png", "pdf",
        ] {
            println!(
                "{ext:>5} -> {:?}",
                manejador_de_extension(ext).map(|g| format!("{g:?}"))
            );
        }
    }
}
