//! **La ventana de CAD-pixpin** y su bucle. Como en el visor de PixPin, el
//! procedimiento de la ventana solo apunta lo que pasa en una cola; el bucle
//! lo atiende con todo el estado a mano y pinta si hace falta (asi un
//! dialogo modal, que tambien despacha mensajes, nunca pisa ese estado).

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use cad_nucleo::Dibujo;
use cad_nucleo::herramientas::portapapeles as pp;
use cad_nucleo::opencadcodec::CadDocument;
use cad_nucleo::ordenes::Id;
use pixpin_cad::gpu::Gpu;
use pixpin_render::MotorRender;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow, ScreenToClient,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, VK_CONTROL, VK_SHIFT,
};
use windows::Win32::UI::Shell::{DragAcceptFiles, DragFinish, DragQueryFileW, HDROP};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{HSTRING, PCWSTR, w};

use crate::autoguardado::{self, Recuperable};
use crate::bim::{self, Aviso};
use crate::documentos::{self, Documento, Documentos};
use crate::mesa::Mesa;
use crate::ui::{Accion, Interruptor, Zona};
use crate::{dialogos, inicio, recientes, tema};

#[derive(Debug, Clone)]
enum Evento {
    Mover(i32, i32),
    Bajar(i32, i32, bool),
    /// Doble clic con el izquierdo (en vez del segundo `Bajar`).
    Doble(i32, i32),
    Subir(i32, i32, bool),
    DobleCentral,
    Derecho,
    Rueda(i32, i32, i32),
    Tecla(u32),
    Caracter(char),
    Tamano(u32, u32),
    /// Cambio de pantalla con otra escala: los puntos por pulgada nuevos.
    Dpi(u32),
    Fuera,
    Soltar(Vec<PathBuf>),
    Cerrar,
}

thread_local! {
    static COLA: RefCell<VecDeque<Evento>> = const { RefCell::new(VecDeque::new()) };
    /// Si el raton esta sobre el plano: la flecha se esconde (se pinta la cruz).
    static SIN_FLECHA: Cell<bool> = const { Cell::new(false) };
}

fn apuntar(e: Evento) {
    COLA.with(|c| c.borrow_mut().push_back(e));
}

/// **El siguiente evento**; de varios movimientos del raton seguidos, solo
/// el ultimo (cada uno buscaria enganches y pintaria para nada).
fn sacar_evento(cola: &mut VecDeque<Evento>) -> Option<Evento> {
    let mut ev = cola.pop_front()?;
    while matches!(ev, Evento::Mover(..)) && matches!(cola.front(), Some(Evento::Mover(..))) {
        ev = cola.pop_front()?;
    }
    Some(ev)
}

fn xy(lp: LPARAM) -> (i32, i32) {
    (
        (lp.0 & 0xffff) as i16 as i32,
        ((lp.0 >> 16) & 0xffff) as i16 as i32,
    )
}

unsafe extern "system" fn procedimiento(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_MOUSEMOVE => {
            let (x, y) = xy(lp);
            apuntar(Evento::Mover(x, y));
            // Para saber cuando sale de la ventana.
            let mut t = windows::Win32::UI::Input::KeyboardAndMouse::TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<
                    windows::Win32::UI::Input::KeyboardAndMouse::TRACKMOUSEEVENT,
                >() as u32,
                dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            // SAFETY: estructura local valida durante la llamada.
            let _ = unsafe { windows::Win32::UI::Input::KeyboardAndMouse::TrackMouseEvent(&mut t) };
            return LRESULT(0);
        }
        WM_MOUSELEAVE => apuntar(Evento::Fuera),
        WM_LBUTTONDOWN => {
            let (x, y) = xy(lp);
            // SAFETY: ventana propia.
            let _ = unsafe { SetCapture(hwnd) };
            apuntar(Evento::Bajar(x, y, false));
        }
        WM_LBUTTONDBLCLK => {
            let (x, y) = xy(lp);
            // SAFETY: ventana propia.
            let _ = unsafe { SetCapture(hwnd) };
            apuntar(Evento::Doble(x, y));
        }
        WM_LBUTTONUP => {
            let (x, y) = xy(lp);
            // SAFETY: soltar la captura propia.
            let _ = unsafe { ReleaseCapture() };
            apuntar(Evento::Subir(x, y, false));
        }
        WM_MBUTTONDOWN => {
            let (x, y) = xy(lp);
            // SAFETY: ventana propia.
            let _ = unsafe { SetCapture(hwnd) };
            apuntar(Evento::Bajar(x, y, true));
        }
        WM_MBUTTONUP => {
            let (x, y) = xy(lp);
            // SAFETY: soltar la captura propia.
            let _ = unsafe { ReleaseCapture() };
            apuntar(Evento::Subir(x, y, true));
        }
        WM_MBUTTONDBLCLK => apuntar(Evento::DobleCentral),
        WM_RBUTTONUP => apuntar(Evento::Derecho),
        WM_MOUSEWHEEL => {
            // La rueda llega en coordenadas de pantalla.
            let (sx, sy) = xy(lp);
            let mut p = POINT { x: sx, y: sy };
            // SAFETY: ventana propia; punto local.
            let _ = unsafe { ScreenToClient(hwnd, &mut p) };
            let delta = ((wp.0 >> 16) & 0xffff) as i16 as i32;
            apuntar(Evento::Rueda(delta, p.x, p.y));
        }
        WM_KEYDOWN | WM_SYSKEYDOWN if wp.0 as u32 != VK_MENU_ALT => {
            apuntar(Evento::Tecla(wp.0 as u32));
            if msg == WM_SYSKEYDOWN && wp.0 as u32 == 0x79 {
                // F10 no abre el menu de la ventana: es el polar.
                return LRESULT(0);
            }
        }
        WM_CHAR => {
            if let Some(c) = char::from_u32(wp.0 as u32) {
                apuntar(Evento::Caracter(c));
            }
            return LRESULT(0);
        }
        WM_SIZE => apuntar(Evento::Tamano(
            (lp.0 & 0xffff) as u32,
            ((lp.0 >> 16) & 0xffff) as u32,
        )),
        WM_SETCURSOR if (lp.0 & 0xffff) as u32 == HTCLIENT => {
            // SAFETY: cursores del sistema.
            unsafe {
                if SIN_FLECHA.with(Cell::get) {
                    SetCursor(None);
                } else {
                    SetCursor(LoadCursorW(None, IDC_ARROW).ok());
                }
            }
            return LRESULT(1);
        }
        WM_DROPFILES => {
            let h = HDROP(wp.0 as *mut _);
            let mut v = Vec::new();
            // SAFETY: el HDROP es valido hasta DragFinish; bufer local.
            unsafe {
                let n = DragQueryFileW(h, u32::MAX, None);
                for i in 0..n {
                    let mut b = [0u16; 1024];
                    let l = DragQueryFileW(h, i, Some(&mut b)) as usize;
                    v.push(PathBuf::from(String::from_utf16_lossy(
                        &b[..l.min(b.len())],
                    )));
                }
                DragFinish(h);
            }
            apuntar(Evento::Soltar(v));
            return LRESULT(0);
        }
        WM_DPICHANGED => {
            // El tamano que propone Windows para la escala nueva.
            let r = lp.0 as *const RECT;
            // SAFETY: con WM_DPICHANGED, lParam apunta a un RECT valido.
            if let Some(r) = unsafe { r.as_ref() } {
                // SAFETY: ventana propia.
                let _ = unsafe {
                    SetWindowPos(
                        hwnd,
                        None,
                        r.left,
                        r.top,
                        r.right - r.left,
                        r.bottom - r.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    )
                };
            }
            apuntar(Evento::Dpi((wp.0 & 0xffff) as u32));
            return LRESULT(0);
        }
        WM_ERASEBKGND => return LRESULT(1),
        WM_CLOSE => {
            apuntar(Evento::Cerrar);
            return LRESULT(0);
        }
        _ => {}
    }
    // SAFETY: lo demas, como siempre.
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

/// Alt sola no hace nada propio.
const VK_MENU_ALT: u32 = 0x12;
/// El raton salio de la ventana (lo pide `TrackMouseEvent`).
const WM_MOUSELEAVE: u32 = 0x02A3;
/// Esperar sin limite.
const INFINITE: u32 = u32::MAX;

fn crear_ventana() -> windows::core::Result<HWND> {
    // SAFETY: clase y ventana propias; cadenas estaticas.
    unsafe {
        let inst: windows::Win32::Foundation::HINSTANCE = GetModuleHandleW(None)?.into();
        let c = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_DBLCLKS,
            lpfnWndProc: Some(procedimiento),
            hInstance: inst,
            hIcon: LoadIconW(Some(inst), PCWSTR(1 as _)).unwrap_or_default(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: w!("CadPixpin"),
            ..Default::default()
        };
        RegisterClassExW(&c);
        let hwnd = CreateWindowExW(
            WS_EX_APPWINDOW | WS_EX_ACCEPTFILES,
            w!("CadPixpin"),
            w!("CAD-pixpin"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1400,
            860,
            None,
            None,
            Some(inst),
            None,
        )?;
        let si: i32 = 1;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &si as *const i32 as *const _,
            4,
        );
        DragAcceptFiles(hwnd, true);
        let _ = ShowWindow(hwnd, SW_SHOWMAXIMIZED);
        Ok(hwnd)
    }
}

fn tamano_cliente(hwnd: HWND) -> (u32, u32) {
    let mut r = RECT::default();
    // SAFETY: ventana propia; rect local.
    let _ = unsafe { GetClientRect(hwnd, &mut r) };
    (
        (r.right - r.left).max(1) as u32,
        (r.bottom - r.top).max(1) as u32,
    )
}

fn poner_titulo(hwnd: HWND, t: &str) {
    let h = HSTRING::from(t);
    // SAFETY: ventana propia; cadena viva durante la llamada.
    let _ = unsafe { SetWindowTextW(hwnd, &h) };
}

/// **Devuelve el teclado a la ventana** tras un dialogo (abrir, guardar,
/// preguntar): sin esto, lo que se escribia despues se perdia hasta hacer
/// clic en la ventana.
fn recuperar_foco(hwnd: HWND) {
    // SAFETY: ventana propia.
    unsafe {
        let _ = SetForegroundWindow(hwnd);
        let _ = windows::Win32::UI::Input::KeyboardAndMouse::SetFocus(Some(hwnd));
    }
}

fn tecla_pulsada(vk: u16) -> bool {
    // SAFETY: lectura del estado del teclado.
    unsafe { GetKeyState(vk as i32) < 0 }
}

/// Los planos abiertos, cada uno en su pestana (se usa como la mesa activa).
type Planos = Documentos<Documento>;

enum Pantalla {
    Inicio,
    Mesa(Planos),
}

/// Cada cuanto se mira si cambiaron los ficheros de las referencias externas.
const CADA_VIGIA: Duration = Duration::from_secs(3);

/// Un plano que se esta leyendo en otro hilo (con lo que haya que decir de
/// sus referencias externas).
struct Abriendo {
    ruta: PathBuf,
    llega: Receiver<Result<(CadDocument, Vec<String>), String>>,
    /// Si es una copia del autoguardado: de que original (si tenia nombre).
    recuperado: Option<Option<PathBuf>>,
}

/// **Todo el estado de la app.**
struct App {
    hwnd: HWND,
    gpu: Gpu,
    motor: Option<MotorRender>,
    w: u32,
    h: u32,
    e: f32,
    pantalla: Pantalla,
    recientes: Vec<PathBuf>,
    zonas: Vec<Zona>,
    cursor: Option<(f32, f32)>,
    /// Los planos que se estan leyendo (se pueden soltar varios a la vez).
    abriendo: Vec<Abriendo>,
    /// Los planos abiertos mientras se ve el inicio (vuelven con su pestana).
    aparcados: Option<Planos>,
    /// Cuando se miraron por ultima vez las referencias externas.
    vigia: Instant,
    avisos: (Sender<Aviso>, Receiver<Aviso>),
    /// Lo que se dice abajo en el inicio (errores al abrir…).
    aviso_inicio: Option<String>,
    titulo: String,
    salir: bool,
    /// Las copias del autoguardado que ofrece el inicio.
    recuperables: Vec<Recuperable>,
    /// Cada cuanto se autoguarda (`None`: nunca).
    cada_autoguardado: Option<Duration>,
    /// En pantalla completa (F11): el estilo y el sitio de antes.
    completa: Option<(i32, WINDOWPLACEMENT)>,
}

/// **Arranca la app** y no vuelve hasta cerrarla.
pub fn correr(abrir: Option<PathBuf>) -> Result<(), String> {
    let hwnd = crear_ventana().map_err(|e| e.to_string())?;
    recuperar_foco(hwnd);
    let (w, h) = tamano_cliente(hwnd);
    let gpu = Gpu::nueva(hwnd, w, h).map_err(|e| format!("sin Direct3D: {e}"))?;
    let motor = MotorRender::nuevo(&gpu.dispositivo).ok();
    // SAFETY: ventana propia.
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    let mut app = App {
        hwnd,
        gpu,
        motor,
        w,
        h,
        e: dpi as f32 / 96.0,
        pantalla: Pantalla::Inicio,
        recientes: recientes::leer(),
        zonas: Vec::new(),
        cursor: None,
        abriendo: Vec::new(),
        aparcados: None,
        vigia: Instant::now(),
        avisos: channel(),
        aviso_inicio: None,
        titulo: String::new(),
        salir: false,
        recuperables: autoguardado::buscar(),
        cada_autoguardado: autoguardado::intervalo(),
        completa: None,
    };
    // Un tic cada 30 s despierta el bucle (que espera sin limite) para el
    // autoguardado.
    // SAFETY: ventana propia; sin TIMERPROC (el WM_TIMER va a DefWindowProc).
    unsafe {
        SetTimer(Some(hwnd), 1, 30_000, None);
        // Y otro cada 3 s para mirar las referencias externas.
        SetTimer(Some(hwnd), 2, CADA_VIGIA.as_millis() as u32, None);
    }
    if let Some(r) = abrir {
        app.abrir_ruta(r);
    }
    let mut sucio = true;
    while !app.salir {
        // SAFETY: bucle de mensajes de este hilo.
        unsafe {
            let mut m = MSG::default();
            while PeekMessageW(&mut m, None, 0, 0, PM_REMOVE).as_bool() {
                if m.message == WM_QUIT {
                    app.salir = true;
                }
                let _ = TranslateMessage(&m);
                DispatchMessageW(&m);
            }
        }
        let t0 = Instant::now();
        let mut ultimo = None;
        while let Some(ev) = COLA.with(|c| sacar_evento(&mut c.borrow_mut())) {
            ultimo = Some(format!("{ev:?}"));
            app.atender(ev);
            app.atender_pedidos();
            sucio = true;
        }
        let t1 = Instant::now();
        if app.recibir() {
            sucio = true;
        }
        app.autoguardar();
        if app.vigilar() {
            sucio = true;
        }
        if sucio && !app.salir {
            app.pintar();
            sucio = false;
        }
        // Las vueltas lentas, al registro (para buscar donde se va el tiempo).
        let (atender, total) = (t1 - t0, t0.elapsed());
        if total > Duration::from_millis(60) {
            tracing::info!(
                "vuelta lenta: {} ms (eventos {} ms, recibir y pintar {} ms) tras {}",
                total.as_millis(),
                atender.as_millis(),
                (total - atender).as_millis(),
                ultimo.unwrap_or_default()
            );
        }
        let espera = if app.ocupada() { 30 } else { INFINITE };
        // SAFETY: esperar mensajes de este hilo.
        unsafe {
            let _ = MsgWaitForMultipleObjectsEx(None, espera, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
        }
    }
    // SAFETY: ventana propia.
    let _ = unsafe { DestroyWindow(app.hwnd) };
    Ok(())
}

impl App {
    fn ocupada(&self) -> bool {
        !self.abriendo.is_empty()
            || matches!(&self.pantalla, Pantalla::Mesa(m) if m.plano.esperando() || m.animando())
    }

    /// Lo que llega de otros hilos. Dice si hay que pintar.
    fn recibir(&mut self) -> bool {
        let mut cambio = false;
        while let Ok(Aviso::Texto(t)) = self.avisos.1.try_recv() {
            self.decir(t);
            cambio = true;
        }
        let mut i = 0;
        while i < self.abriendo.len() {
            let Ok(r) = self.abriendo[i].llega.try_recv() else {
                i += 1;
                continue;
            };
            let a = self.abriendo.remove(i);
            match r {
                Ok((doc, avisos)) if a.recuperado.is_some() => {
                    let original = a.recuperado.flatten();
                    let mut d = Dibujo::de_documento(doc, original);
                    // Lo recuperado aun no esta guardado en su sitio.
                    d.tocar();
                    let mut m = Mesa::nueva(d);
                    m.autoguardado = autoguardado::Estado::recuperado(a.ruta.clone());
                    // No es un plano nuevo: sin el saludo de uno.
                    m.sesion.registro.clear();
                    m.sesion.avisar(
                        "Recuperado del autoguardado: guarde (Ctrl+S) para quedarse con esta version",
                    );
                    for t in avisos {
                        m.sesion.avisar(t);
                    }
                    self.recuperables.retain(|r| r.copia != a.ruta);
                    self.abrir_mesa(m);
                }
                Ok((doc, avisos)) => {
                    recientes::anadir(&a.ruta);
                    self.recientes = recientes::leer();
                    let mut m = Mesa::nueva(Dibujo::de_documento(doc, Some(a.ruta)));
                    for t in avisos {
                        m.sesion.avisar(t);
                    }
                    self.abrir_mesa(m);
                }
                Err(e) => self.decir(format!("No se pudo abrir {}: {e}", a.ruta.display())),
            }
            cambio = true;
        }
        if let Pantalla::Mesa(m) = &mut self.pantalla {
            if m.atender_vista(self.w as f32, self.h as f32, self.e) || m.animando() {
                cambio = true;
            }
            m.al_dia();
            if m.plano.recibir(&self.gpu) {
                cambio = true;
            }
        }
        cambio
    }

    fn decir(&mut self, t: String) {
        match &mut self.pantalla {
            Pantalla::Mesa(m) => m.sesion.avisar(t),
            Pantalla::Inicio => self.aviso_inicio = Some(t),
        }
    }

    fn atender(&mut self, ev: Evento) {
        let (w, h, e) = (self.w as f32, self.h as f32, self.e);
        match ev {
            Evento::Tamano(nw, nh) => {
                if nw > 0 && nh > 0 && (nw, nh) != (self.w, self.h) {
                    self.w = nw;
                    self.h = nh;
                    let _ = self.gpu.redimensionar(nw, nh);
                }
            }
            Evento::Dpi(dpi) => {
                self.e = dpi.max(96) as f32 / 96.0;
            }
            Evento::Mover(x, y) => {
                let (x, y) = (x as f32, y as f32);
                self.cursor = Some((x, y));
                let en_plano = match &mut self.pantalla {
                    Pantalla::Mesa(m) => {
                        m.mover(x, y, w, h, e);
                        m.en_el_dibujo(x, y, w, h, e)
                    }
                    Pantalla::Inicio => false,
                };
                SIN_FLECHA.with(|c| c.set(en_plano));
            }
            Evento::Fuera => {
                self.cursor = None;
                if let Pantalla::Mesa(m) = &mut self.pantalla {
                    m.cursor = None;
                }
                SIN_FLECHA.with(|c| c.set(false));
            }
            Evento::Bajar(x, y, central) => {
                let (x, y) = (x as f32, y as f32);
                // Con un menu o una lista abiertos, un clic fuera solo los cierra.
                if let Pantalla::Mesa(m) = &mut self.pantalla
                    && m.emergente.is_some()
                    && (central || !m.en_emergente(x, y))
                {
                    m.emergente = None;
                    return;
                }
                if let Pantalla::Mesa(m) = &mut self.pantalla
                    && m.en_el_dibujo(x, y, w, h, e)
                {
                    m.bajar(x, y, central);
                    return;
                }
                if !central
                    && let Some(z) = self
                        .zonas
                        .iter()
                        .rev()
                        .find(|z| crate::ui::dentro(&z.r, x, y))
                {
                    let a = z.accion.clone();
                    self.accion(a);
                }
            }
            Evento::Subir(x, y, central) => {
                if let Pantalla::Mesa(m) = &mut self.pantalla {
                    m.subir(x as f32, y as f32, central, w, h, e);
                }
            }
            Evento::Doble(x, y) => {
                let (xf, yf) = (x as f32, y as f32);
                if let Pantalla::Mesa(m) = &mut self.pantalla
                    && m.en_el_dibujo(xf, yf, w, h, e)
                    && m.doble_clic(xf, yf, w, h, e)
                {
                    return;
                }
                // Si no, como un clic mas.
                self.atender(Evento::Bajar(x, y, false));
            }
            Evento::DobleCentral => self.accion(Accion::Encuadre),
            Evento::Derecho => {
                if let Pantalla::Mesa(m) = &mut self.pantalla {
                    m.derecho();
                }
            }
            Evento::Rueda(delta, x, y) => {
                if let Pantalla::Mesa(m) = &mut self.pantalla {
                    let (xf, yf) = (x as f32, y as f32);
                    if m.en_emergente(xf, yf) {
                        m.rueda_emergente(delta as f32 / 120.0);
                    } else if m.en_capas(xf, yf, w, h, e) {
                        m.rueda_capas(delta as f32 / 120.0);
                    } else {
                        m.rueda(delta as f32 / 120.0, xf, yf, w, h);
                    }
                }
            }
            Evento::Tecla(vk) => self.tecla(vk),
            Evento::Caracter(c) => {
                // Los Ctrl+letra llegan como caracteres de control y la mesa
                // los ignora; no se mira Ctrl aqui, porque AltGr (la @ del
                // teclado espanol) tambien lo enciende.
                // En el inicio, escribir una orden empieza un plano nuevo con
                // ella (lo que promete el pie del inicio).
                if matches!(self.pantalla, Pantalla::Inicio)
                    && self.abriendo.is_empty()
                    && !c.is_control()
                    && !c.is_whitespace()
                {
                    self.accion(Accion::PlanoNuevo);
                }
                if let Pantalla::Mesa(m) = &mut self.pantalla {
                    m.caracter(c);
                }
            }
            Evento::Soltar(v) => {
                // Cada plano soltado, en su pestana.
                for r in v {
                    self.abrir_ruta(r);
                }
            }
            Evento::Cerrar => {
                if self.puede_salir() {
                    self.salir = true;
                }
            }
        }
    }

    fn tecla(&mut self, vk: u32) {
        let ctrl = tecla_pulsada(VK_CONTROL.0);
        let shift = tecla_pulsada(VK_SHIFT.0);
        if vk == 0x7A {
            self.pantalla_completa();
            return;
        }
        let Pantalla::Mesa(planos) = &mut self.pantalla else {
            match (ctrl, vk) {
                (true, 0x4F) => self.accion(Accion::AbrirPlano),
                (true, 0x4E) => self.accion(Accion::PlanoNuevo),
                // Volver a los planos abiertos.
                (true, 0x09) => self.volver_a_los_planos(),
                _ => {}
            }
            return;
        };
        // Los planos abiertos: Ctrl+Tab, Ctrl+Mayus+Tab y Ctrl+W (o Ctrl+F4).
        if ctrl && vk == 0x09 {
            let cambio = if shift {
                planos.anterior()
            } else {
                planos.siguiente()
            };
            if cambio {
                self.al_cambiar_de_plano();
            }
            return;
        }
        if ctrl && (vk == 0x57 || vk == 0x73) {
            self.cerrar_plano(None);
            return;
        }
        let m = &mut *planos.mesa;
        match (ctrl, vk) {
            (_, 0x1B) => m.escape(),
            (false, 0x26) => m.historial(true),
            (false, 0x28) => m.historial(false),
            (false, 0x09) => m.tabulador(shift),
            (false, 0x7B) => {
                m.entrada_dinamica = !m.entrada_dinamica;
                m.sesion.avisar(if m.entrada_dinamica {
                    "<Entrada dinamica activada>"
                } else {
                    "<Entrada dinamica desactivada>"
                });
            }
            (false, 0x71) => {
                m.historial_alto = !m.historial_alto;
            }
            (false, 0x76) => {
                m.rejilla_visible = !m.rejilla_visible;
                m.sesion.avisar(if m.rejilla_visible {
                    "<Rejilla visible activada>"
                } else {
                    "<Rejilla visible desactivada>"
                });
            }
            (true, 0x5A) if shift => m.sesion.orden(Id::Rehacer),
            (true, 0x43) if shift => m.sesion.orden(Id::Otra(pp::COPIABASE)),
            (true, 0x43) => m.sesion.orden(Id::Otra(pp::COPIAR)),
            (true, 0x58) => m.sesion.orden(Id::Otra(pp::CORTAR)),
            (true, 0x56) => m.sesion.orden(Id::Otra(pp::PEGAR)),
            (false, 0x2E) => m.sesion.borrar_seleccion(),
            (false, 0x72) => m.alternar(Interruptor::Enganche),
            (false, 0x77) => m.alternar(Interruptor::Orto),
            (false, 0x78) => m.alternar(Interruptor::Rejilla),
            (false, 0x79) => m.alternar(Interruptor::Polar),
            (true, 0x5A) => m.sesion.orden(Id::Deshacer),
            (true, 0x59) => m.sesion.orden(Id::Rehacer),
            (true, 0x41) => m.sesion.elegir_todo(),
            (true, 0x53) if shift => self.accion(Accion::GuardarComo),
            (true, 0x53) => self.accion(Accion::Guardar),
            (true, 0x4F) => self.accion(Accion::AbrirPlano),
            (true, 0x4E) => self.accion(Accion::PlanoNuevo),
            _ => {}
        }
    }

    /// Si se puede dejar el dibujo abierto (guardandolo si se quiere).
    fn puede_dejar_el_dibujo(&mut self) -> bool {
        let Pantalla::Mesa(m) = &mut self.pantalla else {
            return true;
        };
        if !m.sesion.dibujo.sucio() {
            m.autoguardado.olvidar();
            return true;
        }
        let nombre = m
            .ruta()
            .and_then(|r| r.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "el plano nuevo".into());
        let si = pixpin_shell::dialogo::preguntar(
            self.hwnd,
            "CAD-pixpin",
            &format!("¿Guardar los cambios de {nombre}?"),
        );
        recuperar_foco(self.hwnd);
        if !si {
            // No se quieren los cambios: tampoco su copia.
            m.autoguardado.olvidar();
            return true;
        }
        self.guardar(false)
    }

    fn guardar(&mut self, como: bool) -> bool {
        let hwnd = self.hwnd;
        let Pantalla::Mesa(m) = &mut self.pantalla else {
            return false;
        };
        let ruta = if como || m.ruta().is_none() {
            let sugerido = m
                .ruta()
                .and_then(|r| r.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Plano.dwg".into());
            let r = dialogos::guardar(hwnd, &sugerido);
            recuperar_foco(hwnd);
            match r {
                Some(r) => Some(r),
                None => return false,
            }
        } else {
            None
        };
        match m.guardar(ruta) {
            Ok(r) => {
                recientes::anadir(&r);
                self.recientes = recientes::leer();
                true
            }
            Err(e) => {
                m.sesion.avisar(format!("No se pudo guardar: {e}"));
                false
            }
        }
    }

    /// **Lo que las ordenes le pidieron a la app** (ver
    /// [`cad_nucleo::herramienta::PedidoApp`]), en orden.
    fn atender_pedidos(&mut self) {
        use cad_nucleo::herramienta::PedidoApp;
        loop {
            let Pantalla::Mesa(m) = &mut self.pantalla else {
                return;
            };
            if m.sesion.memoria.pedidos.is_empty() {
                return;
            }
            let pedido = m.sesion.memoria.pedidos.remove(0);
            match pedido {
                PedidoApp::Archivo {
                    guardar,
                    titulo,
                    filtros,
                    sugerido,
                } => {
                    let r = dialogos::elegir(self.hwnd, guardar, &titulo, &filtros, &sugerido);
                    recuperar_foco(self.hwnd);
                    let Pantalla::Mesa(m) = &mut self.pantalla else {
                        return;
                    };
                    match r {
                        Some(ruta) => m.sesion.escribir(&ruta.to_string_lossy()),
                        None => m.sesion.escape(),
                    }
                }
                PedidoApp::Abrir(ruta) => self.abrir_ruta(ruta),
                PedidoApp::Ver3d(Some(ruta)) => {
                    bim::abrir(ruta, self.avisos.0.clone());
                }
                PedidoApp::Ver3d(None) => {
                    if let Pantalla::Mesa(m) = &self.pantalla {
                        let d = &m.sesion.dibujo;
                        let titulo = d.ruta.as_ref().and_then(|r| r.file_name()).map_or_else(
                            || "Dibujo en 3D".to_string(),
                            |n| n.to_string_lossy().into_owned(),
                        );
                        crate::vista3d::abrir(d.doc.clone(), titulo, self.avisos.0.clone());
                    }
                }
                PedidoApp::ExplorarBim(ruta) => {
                    self.decir(format!(
                        "El explorador BIM aun no esta listo ({})",
                        ruta.display()
                    ));
                }
                PedidoApp::Aviso(t) => {
                    pixpin_shell::dialogo::avisar_al_frente("CAD-pixpin", &t);
                    recuperar_foco(self.hwnd);
                }
            }
        }
    }

    fn abrir_ruta(&mut self, ruta: PathBuf) {
        if bim::se_abre(&ruta) {
            recientes::anadir(&ruta);
            self.recientes = recientes::leer();
            self.decir(format!("Abriendo {} en 3D…", ruta.display()));
            bim::abrir(ruta, self.avisos.0.clone());
            return;
        }
        // Si ya esta abierto, se va a su pestana; si se esta leyendo, nada.
        if self.ir_al_plano(&ruta)
            || self
                .abriendo
                .iter()
                .any(|a| a.recuperado.is_none() && documentos::es_la_misma(&a.ruta, &ruta))
        {
            return;
        }
        let (tx, rx) = channel();
        let r2 = ruta.clone();
        let _ = std::thread::Builder::new()
            .name("plano-abrir".into())
            .stack_size(64 * 1024 * 1024)
            .spawn(move || {
                let r = std::panic::catch_unwind(|| cad_nucleo::xref::leer_con_referencias(&r2))
                    .unwrap_or_else(|_| Err("el plano esta dañado".into()));
                let _ = tx.send(r);
            });
        self.decir(format!(
            "Abriendo {}…",
            ruta.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        ));
        self.abriendo.push(Abriendo {
            ruta,
            llega: rx,
            recuperado: None,
        });
    }

    /// **Pone una mesa nueva en su pestana** (al final) y la deja a la vista.
    fn abrir_mesa(&mut self, m: Mesa) {
        let d = Documento::nuevo(m);
        let planos = match std::mem::replace(&mut self.pantalla, Pantalla::Inicio) {
            Pantalla::Mesa(mut p) => {
                p.anadir(d);
                p
            }
            Pantalla::Inicio => match self.aparcados.take() {
                Some(mut p) => {
                    p.anadir(d);
                    p
                }
                None => Documentos::con(d),
            },
        };
        self.pantalla = Pantalla::Mesa(planos);
        self.aviso_inicio = None;
        self.al_cambiar_de_plano();
    }

    /// Los planos abiertos (los de la mesa o los aparcados en el inicio).
    fn planos_mut(&mut self) -> Option<&mut Planos> {
        match &mut self.pantalla {
            Pantalla::Mesa(p) => Some(p),
            Pantalla::Inicio => self.aparcados.as_mut(),
        }
    }

    /// Si `ruta` ya esta abierta, se va a su pestana (y dice que si).
    fn ir_al_plano(&mut self, ruta: &Path) -> bool {
        let Some(p) = self.planos_mut() else {
            return false;
        };
        let Some(i) = p.buscar(|d| d.ruta().is_some_and(|r| documentos::es_la_misma(r, ruta)))
        else {
            return false;
        };
        p.activar(i);
        self.volver_a_los_planos();
        self.al_cambiar_de_plano();
        true
    }

    /// Del inicio a los planos que habia abiertos.
    fn volver_a_los_planos(&mut self) {
        if matches!(self.pantalla, Pantalla::Inicio)
            && let Some(p) = self.aparcados.take()
        {
            self.pantalla = Pantalla::Mesa(p);
            self.aviso_inicio = None;
            self.al_cambiar_de_plano();
        }
    }

    /// Al ver otro plano: el raton esta donde esta (no donde lo dejo en esa
    /// pestana).
    fn al_cambiar_de_plano(&mut self) {
        let cursor = self.cursor;
        if let Pantalla::Mesa(m) = &mut self.pantalla {
            m.cursor = cursor;
        }
    }

    /// **Cierra el plano `i`** (o el activo), preguntando si guardar. Con el
    /// ultimo se vuelve al inicio.
    fn cerrar_plano(&mut self, i: Option<usize>) {
        let Pantalla::Mesa(p) = &mut self.pantalla else {
            return;
        };
        if let Some(i) = i {
            p.activar(i);
        }
        if !self.puede_dejar_el_dibujo() {
            return;
        }
        let Pantalla::Mesa(p) = &mut self.pantalla else {
            return;
        };
        if p.cuantos() == 1 {
            self.pantalla = Pantalla::Inicio;
            self.aviso_inicio = None;
        } else {
            let i = p.indice_activo();
            p.quitar(i);
            self.al_cambiar_de_plano();
        }
    }

    /// **Al cerrar la app**: cada plano con cambios pregunta si guardar (con
    /// su pestana a la vista). Si se cancela uno, no se sale.
    fn puede_salir(&mut self) -> bool {
        self.volver_a_los_planos();
        let n = match &self.pantalla {
            Pantalla::Mesa(p) => p.cuantos(),
            Pantalla::Inicio => return true,
        };
        for i in 0..n {
            if let Pantalla::Mesa(p) = &mut self.pantalla {
                p.activar(i);
            }
            if !self.puede_dejar_el_dibujo() {
                return false;
            }
        }
        true
    }

    /// **Mira las referencias externas** de los planos abiertos cada
    /// [`CADA_VIGIA`] (leer fechas de ficheros no se hace en cada vuelta).
    /// Dice si hay que pintar.
    fn vigilar(&mut self) -> bool {
        if self.vigia.elapsed() < CADA_VIGIA {
            return false;
        }
        self.vigia = Instant::now();
        let mut dijo = false;
        if let Some(p) = self.planos_mut() {
            for d in p.todos_mut() {
                dijo |= d.revisar_referencias();
            }
        }
        dijo
    }

    /// **Abre una copia del autoguardado** como si fuera su original (con
    /// sus cambios sin guardar).
    fn recuperar(&mut self, copia: PathBuf) {
        let Some(r) = self.recuperables.iter().find(|r| r.copia == copia).cloned() else {
            return;
        };
        let (tx, rx) = channel();
        let leer = r.copia.clone();
        let original = r.original.clone();
        let _ = std::thread::Builder::new()
            .name("plano-recuperar".into())
            .stack_size(64 * 1024 * 1024)
            .spawn(move || {
                let res = std::panic::catch_unwind(|| {
                    // Las referencias se buscan desde donde estaba el original.
                    let mut doc = cad_nucleo::dibujo::leer(&leer)?;
                    let avisos = cad_nucleo::xref::cargar_todas(&mut doc, original.as_deref());
                    Ok((doc, avisos))
                })
                .unwrap_or_else(|_| Err("la copia esta dañada".into()));
                let _ = tx.send(res);
            });
        self.decir(format!("Recuperando {}…", r.nombre()));
        self.abriendo.push(Abriendo {
            ruta: r.copia,
            llega: rx,
            recuperado: Some(r.original),
        });
    }

    /// Hace la copia del autoguardado si toca (se mira en cada vuelta: es
    /// comparar dos numeros y una hora).
    fn autoguardar(&mut self) {
        let Some(cada) = self.cada_autoguardado else {
            return;
        };
        let Some(p) = self.planos_mut() else {
            return;
        };
        // Todos los abiertos, no solo el que se ve.
        let ahora = Instant::now();
        for d in p.todos_mut() {
            let m = &mut *d.mesa;
            if m.autoguardado.toca(&m.sesion.dibujo, ahora, cada) {
                m.autoguardado.lanzar(&m.sesion.dibujo);
            }
        }
    }

    /// F11: pantalla completa (sin marco, el monitor entero) o como estaba.
    fn pantalla_completa(&mut self) {
        let hwnd = self.hwnd;
        // SAFETY: ventana propia; estructuras locales validas durante las llamadas.
        unsafe {
            if let Some((estilo, sitio)) = self.completa.take() {
                SetWindowLongW(hwnd, GWL_STYLE, estilo);
                let _ = SetWindowPlacement(hwnd, &sitio);
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_FRAMECHANGED,
                );
                return;
            }
            let estilo = GetWindowLongW(hwnd, GWL_STYLE);
            let mut sitio = WINDOWPLACEMENT {
                length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
                ..Default::default()
            };
            let mut mi = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            if GetWindowPlacement(hwnd, &mut sitio).is_err()
                || !GetMonitorInfoW(monitor, &mut mi).as_bool()
            {
                return;
            }
            SetWindowLongW(hwnd, GWL_STYLE, estilo & !(WS_OVERLAPPEDWINDOW.0 as i32));
            let r = mi.rcMonitor;
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOP),
                r.left,
                r.top,
                r.right - r.left,
                r.bottom - r.top,
                SWP_NOOWNERZORDER | SWP_FRAMECHANGED,
            );
            self.completa = Some((estilo, sitio));
        }
    }

    fn accion(&mut self, a: Accion) {
        let (w, h, e) = (self.w as f32, self.h as f32, self.e);
        match a {
            Accion::PlanoNuevo => self.abrir_mesa(Mesa::nueva(Dibujo::nuevo())),
            Accion::Documento(i) => {
                self.volver_a_los_planos();
                if let Pantalla::Mesa(p) = &mut self.pantalla
                    && p.activar(i)
                {
                    self.al_cambiar_de_plano();
                }
            }
            Accion::CerrarDocumento(i) => {
                self.volver_a_los_planos();
                self.cerrar_plano(Some(i));
            }
            Accion::AbrirPlano => {
                let r = dialogos::abrir(self.hwnd, dialogos::Que::Plano);
                recuperar_foco(self.hwnd);
                if let Some(r) = r {
                    self.abrir_ruta(r);
                }
            }
            Accion::AbrirModelo => {
                let r = dialogos::abrir(self.hwnd, dialogos::Que::Modelo);
                recuperar_foco(self.hwnd);
                if let Some(r) = r {
                    self.abrir_ruta(r);
                }
            }
            Accion::AbrirReciente(r) => self.abrir_ruta(r),
            Accion::Recuperar(c) => self.recuperar(c),
            Accion::DescartarCopia(c) => {
                autoguardado::descartar(&c);
                self.recuperables.retain(|r| r.copia != c);
            }
            Accion::MetradoBim => self.metrado_bim(),
            Accion::Inicio => {
                // Los planos no se cierran: esperan en sus pestanas.
                if let Pantalla::Mesa(p) = std::mem::replace(&mut self.pantalla, Pantalla::Inicio) {
                    self.aparcados = Some(p);
                }
                self.aviso_inicio = None;
            }
            Accion::Guardar => {
                self.guardar(false);
            }
            Accion::GuardarComo => {
                self.guardar(true);
            }
            Accion::MetradoPlano => self.metrado_plano(),
            otra => {
                let Pantalla::Mesa(m) = &mut self.pantalla else {
                    return;
                };
                // Lo que se elige en un menu o una lista los cierra.
                m.emergente = None;
                match otra {
                    Accion::Pestana(g) => m.pestana = g,
                    Accion::Orden(id) => m.sesion.orden(id),
                    Accion::Encuadre => m.encuadrar(w, h, e),
                    Accion::CapaActual(c) => m.sesion.dibujo.poner_capa_actual(&c),
                    Accion::CapaVisible(c) => {
                        m.sesion.dibujo.con_capa(&c, |l| {
                            if l.is_off() {
                                l.turn_on()
                            } else {
                                l.turn_off()
                            }
                        });
                    }
                    Accion::CapaBloqueo(c) => {
                        m.sesion
                            .dibujo
                            .con_capa(&c, |l| if l.is_locked() { l.unlock() } else { l.lock() });
                    }
                    Accion::PasarACapa(c) => m.sesion.poner_capa(&c),
                    Accion::Alternar(i) => m.alternar(i),
                    Accion::Enganche(t) => {
                        m.sesion.ajustes.enganche = true;
                        m.sesion.ajustes.modos.alternar(t);
                    }
                    Accion::Desplegar(campo, y) => {
                        m.emergente = Some(crate::mesa::Emergente::Lista { campo, y, desde: 0 });
                    }
                    Accion::Propiedad(c) => m.cambiar_propiedad(&c),
                    Accion::Repetir => m.repetir(),
                    Accion::Cancelar => m.cancelar(),
                    Accion::VerPropiedades => m.ver_propiedades(),
                    _ => {}
                }
            }
        }
    }

    fn metrado_plano(&mut self) {
        let hwnd = self.hwnd;
        let Pantalla::Mesa(m) = &mut self.pantalla else {
            return;
        };
        let (csv, filas) = m.metrado_csv();
        let base = m
            .ruta()
            .and_then(|r| r.file_stem())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Plano".into());
        let destino = dialogos::guardar_csv(hwnd, &format!("{base} - metrado.csv"));
        recuperar_foco(hwnd);
        let Some(destino) = destino else {
            return;
        };
        match escribir_csv(&destino, &csv) {
            Ok(()) => m.sesion.avisar(format!(
                "Metrado de {filas} capas guardado en {}",
                destino.display()
            )),
            Err(e) => m
                .sesion
                .avisar(format!("No se pudo guardar el metrado: {e}")),
        }
    }

    fn metrado_bim(&mut self) {
        let modelo = dialogos::abrir(self.hwnd, dialogos::Que::Modelo);
        recuperar_foco(self.hwnd);
        let Some(modelo) = modelo else {
            return;
        };
        let base = modelo
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let destino = dialogos::guardar_csv(self.hwnd, &format!("{base} - metrado BIM.csv"));
        recuperar_foco(self.hwnd);
        let Some(destino) = destino else {
            return;
        };
        self.decir(format!("Midiendo {}…", modelo.display()));
        let avisos = self.avisos.0.clone();
        let _ = std::thread::Builder::new()
            .name("bim-metrado".into())
            .spawn(move || {
                let t = match bim::metrado_csv(&modelo)
                    .and_then(|(csv, n)| escribir_csv(&destino, &csv).map(|()| n))
                {
                    Ok(n) => format!(
                        "Metrado BIM de {n} partidas guardado en {}",
                        destino.display()
                    ),
                    Err(e) => format!("No se pudo hacer el metrado: {e}"),
                };
                let _ = avisos.send(Aviso::Texto(t));
            });
    }

    fn pintar(&mut self) {
        let (w, h, e) = (self.w as f32, self.h as f32, self.e);
        let titulo = match &self.pantalla {
            Pantalla::Mesa(m) => m.titulo(),
            Pantalla::Inicio => "CAD-pixpin".to_string(),
        };
        if titulo != self.titulo {
            poner_titulo(self.hwnd, &titulo);
            self.titulo = titulo;
        }
        let Some(motor) = self.motor.as_ref() else {
            let _ = self.gpu.dibujar(tema::FONDO_PLANO, &[]);
            return;
        };
        let mut zonas = Vec::new();
        let cursor = self.cursor;
        let recientes = &self.recientes;
        let recuperables = &self.recuperables;
        let aviso = self.aviso_inicio.clone();
        // Las pestanas de los planos que esperan mientras se ve el inicio.
        let aparcadas = self.aparcados.as_ref().map(documentos::etiquetas);
        // El globo del boton bajo el raton (de las zonas del fotograma anterior).
        let globo = cursor.and_then(|(x, y)| {
            self.zonas
                .iter()
                .rev()
                .find(|z| crate::ui::dentro(&z.r, x, y))
                .map(|z| (z.ayuda.clone(), x, y))
        });
        let mut pintar_encima = |p: &pixpin_render::Pintor, pantalla: &mut Pantalla| match pantalla
        {
            Pantalla::Inicio => {
                zonas = inicio::pintar(p, w, h, e, recientes, recuperables, cursor);
                if let Some(v) = &aparcadas {
                    let r = crate::ui::rect(0.0, 0.0, w, tema::ALTO_DOCUMENTOS * e);
                    documentos::pintar(p, r, e, v, None, cursor, &mut zonas);
                }
                if let Some(a) = &aviso {
                    p.texto_con_fondo(
                        a,
                        24.0 * e,
                        h - 56.0 * e,
                        12.5 * e,
                        tema::TEXTO,
                        tema::PANEL_ALTO,
                    );
                }
            }
            Pantalla::Mesa(m) => {
                let t = Instant::now();
                zonas = m.pintar(p, w, h, e);
                let ms = t.elapsed().as_millis();
                if ms > 30 {
                    tracing::info!("la mesa tardo {ms} ms en pintarse");
                }
                let r = m.disp(w, h, e).documentos;
                let v = documentos::etiquetas(m);
                documentos::pintar(p, r, e, &v, Some(m.indice_activo()), cursor, &mut zonas);
                if let Some((t, x, y)) = &globo {
                    crate::ui::globo(p, t, *x, *y, e, w);
                }
            }
        };
        // El plano sale de la mesa mientras se pinta (la interfaz necesita la
        // mesa entera) y vuelve despues.
        let mut tomado = None;
        if let Pantalla::Mesa(m) = &mut self.pantalla {
            m.hay_plano = m.plano.gpu.is_some();
            m.caja_plano = m.plano.gpu.as_ref().and_then(|g| g.caja);
            m.encuadrar_si_falta(w, h, e);
            tomado = m.plano.gpu.take();
        }
        // Los cubos que se ven, cada uno con su vista.
        let planos = match (&self.pantalla, &tomado) {
            (Pantalla::Mesa(m), Some(g)) => g.planos(&m.camara, self.w, self.h),
            _ => Vec::new(),
        };
        let pantalla = &mut self.pantalla;
        let mut encima =
            |_: &windows::Win32::Graphics::Direct3D11::ID3D11Device,
             tex: &windows::Win32::Graphics::Direct3D11::ID3D11Texture2D| {
                if let Ok(b) = motor.destino_backbuffer(tex)
                    && let Err(err) = motor.dibujar(&b, |p| pintar_encima(p, pantalla))
                {
                    tracing::warn!(?err, "no se pudo pintar la interfaz");
                }
            };
        let r = self
            .gpu
            .dibujar_con(tema::FONDO_PLANO, &planos, &mut encima);
        drop(planos);
        if let Pantalla::Mesa(m) = &mut self.pantalla
            && tomado.is_some()
            && m.plano.gpu.is_none()
        {
            m.plano.gpu = tomado;
        }
        if let Err(err) = r {
            tracing::warn!(?err, "no se pudo dibujar");
        }
        self.zonas = zonas;
    }
}

fn escribir_csv(destino: &Path, csv: &str) -> Result<(), String> {
    // Con la marca BOM: Excel lo abre con las tildes bien.
    let mut b = vec![0xEF, 0xBB, 0xBF];
    b.extend_from_slice(csv.as_bytes());
    std::fs::write(destino, b).map_err(|e| e.to_string())
}
