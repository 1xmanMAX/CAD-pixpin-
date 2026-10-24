//! **La ventana de CAD-pixpin** y su bucle. Como en el visor de PixPin, el
//! procedimiento de la ventana solo apunta lo que pasa en una cola; el bucle
//! lo atiende con todo el estado a mano y pinta si hace falta (asi un
//! dialogo modal, que tambien despacha mensajes, nunca pisa ese estado).

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};

use cad_nucleo::Dibujo;
use cad_nucleo::opencadcodec::CadDocument;
use cad_nucleo::ordenes::Id;
use pixpin_cad::gpu::Gpu;
use pixpin_render::MotorRender;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, VK_CONTROL, VK_SHIFT,
};
use windows::Win32::UI::Shell::{DragAcceptFiles, DragFinish, DragQueryFileW, HDROP};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{HSTRING, PCWSTR, w};

use crate::bim::{self, Aviso};
use crate::mesa::Mesa;
use crate::ui::{Accion, Interruptor, Zona};
use crate::{dialogos, inicio, recientes, tema};

#[derive(Debug, Clone)]
enum Evento {
    Mover(i32, i32),
    Bajar(i32, i32, bool),
    Subir(i32, i32, bool),
    DobleCentral,
    Derecho,
    Rueda(i32, i32, i32),
    Tecla(u32),
    Caracter(char),
    Tamano(u32, u32),
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
        WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => {
            let (x, y) = xy(lp);
            // SAFETY: ventana propia.
            let _ = unsafe { SetCapture(hwnd) };
            apuntar(Evento::Bajar(x, y, false));
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

fn tecla_pulsada(vk: u16) -> bool {
    // SAFETY: lectura del estado del teclado.
    unsafe { GetKeyState(vk as i32) < 0 }
}

enum Pantalla {
    Inicio,
    Mesa(Box<Mesa>),
}

/// Un plano que se esta leyendo en otro hilo.
struct Abriendo {
    ruta: PathBuf,
    llega: Receiver<Result<CadDocument, String>>,
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
    abriendo: Option<Abriendo>,
    avisos: (Sender<Aviso>, Receiver<Aviso>),
    /// Lo que se dice abajo en el inicio (errores al abrir…).
    aviso_inicio: Option<String>,
    titulo: String,
    salir: bool,
}

/// **Arranca la app** y no vuelve hasta cerrarla.
pub fn correr(abrir: Option<PathBuf>) -> Result<(), String> {
    let hwnd = crear_ventana().map_err(|e| e.to_string())?;
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
        abriendo: None,
        avisos: channel(),
        aviso_inicio: None,
        titulo: String::new(),
        salir: false,
    };
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
        while let Some(ev) = COLA.with(|c| c.borrow_mut().pop_front()) {
            app.atender(ev);
            sucio = true;
        }
        if app.recibir() {
            sucio = true;
        }
        if sucio && !app.salir {
            app.pintar();
            sucio = false;
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
        self.abriendo.is_some()
            || matches!(&self.pantalla, Pantalla::Mesa(m) if m.plano.esperando())
    }

    /// Lo que llega de otros hilos. Dice si hay que pintar.
    fn recibir(&mut self) -> bool {
        let mut cambio = false;
        while let Ok(Aviso::Texto(t)) = self.avisos.1.try_recv() {
            self.decir(t);
            cambio = true;
        }
        if let Some(a) = &self.abriendo
            && let Ok(r) = a.llega.try_recv()
        {
            let a = self.abriendo.take().unwrap();
            match r {
                Ok(doc) => {
                    recientes::anadir(&a.ruta);
                    self.recientes = recientes::leer();
                    self.pantalla = Pantalla::Mesa(Box::new(Mesa::nueva(Dibujo::de_documento(
                        doc,
                        Some(a.ruta),
                    ))));
                }
                Err(e) => {
                    self.aviso_inicio = Some(format!("No se pudo abrir {}: {e}", a.ruta.display()))
                }
            }
            cambio = true;
        }
        if let Pantalla::Mesa(m) = &mut self.pantalla {
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
            Evento::DobleCentral => self.accion(Accion::Encuadre),
            Evento::Derecho => {
                if let Pantalla::Mesa(m) = &mut self.pantalla {
                    m.derecho();
                }
            }
            Evento::Rueda(delta, x, y) => {
                if let Pantalla::Mesa(m) = &mut self.pantalla {
                    m.rueda(delta as f32 / 120.0, x as f32, y as f32, w, h);
                }
            }
            Evento::Tecla(vk) => self.tecla(vk),
            Evento::Caracter(c) => {
                // Los Ctrl+letra llegan como caracteres de control y la mesa
                // los ignora; no se mira Ctrl aqui, porque AltGr (la @ del
                // teclado espanol) tambien lo enciende.
                if let Pantalla::Mesa(m) = &mut self.pantalla {
                    m.caracter(c);
                }
            }
            Evento::Soltar(v) => {
                if let Some(r) = v.into_iter().next() {
                    self.abrir_ruta(r);
                }
            }
            Evento::Cerrar => {
                if self.puede_dejar_el_dibujo() {
                    self.salir = true;
                }
            }
        }
    }

    fn tecla(&mut self, vk: u32) {
        let ctrl = tecla_pulsada(VK_CONTROL.0);
        let shift = tecla_pulsada(VK_SHIFT.0);
        let Pantalla::Mesa(m) = &mut self.pantalla else {
            match (ctrl, vk) {
                (true, 0x4F) => self.accion(Accion::AbrirPlano),
                (true, 0x4E) => self.accion(Accion::PlanoNuevo),
                _ => {}
            }
            return;
        };
        match (ctrl, vk) {
            (_, 0x1B) => m.escape(),
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
            return true;
        }
        let nombre = m
            .ruta()
            .and_then(|r| r.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "el plano nuevo".into());
        if !pixpin_shell::dialogo::preguntar(
            self.hwnd,
            "CAD-pixpin",
            &format!("¿Guardar los cambios de {nombre}?"),
        ) {
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
            match dialogos::guardar(hwnd, &sugerido) {
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

    fn abrir_ruta(&mut self, ruta: PathBuf) {
        if bim::se_abre(&ruta) {
            recientes::anadir(&ruta);
            self.recientes = recientes::leer();
            self.decir(format!("Abriendo {} en 3D…", ruta.display()));
            bim::abrir(ruta, self.avisos.0.clone());
            return;
        }
        if !self.puede_dejar_el_dibujo() {
            return;
        }
        let (tx, rx) = channel();
        let r2 = ruta.clone();
        let _ = std::thread::Builder::new()
            .name("plano-abrir".into())
            .stack_size(64 * 1024 * 1024)
            .spawn(move || {
                let r = std::panic::catch_unwind(|| cad_nucleo::dibujo::leer(&r2))
                    .unwrap_or_else(|_| Err("el plano esta dañado".into()));
                let _ = tx.send(r);
            });
        self.pantalla = Pantalla::Inicio;
        self.aviso_inicio = Some(format!(
            "Abriendo {}…",
            ruta.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        ));
        self.abriendo = Some(Abriendo { ruta, llega: rx });
    }

    fn accion(&mut self, a: Accion) {
        let (w, h, e) = (self.w as f32, self.h as f32, self.e);
        match a {
            Accion::PlanoNuevo => {
                if self.puede_dejar_el_dibujo() {
                    self.pantalla = Pantalla::Mesa(Box::new(Mesa::nueva(Dibujo::nuevo())));
                }
            }
            Accion::AbrirPlano => {
                if let Some(r) = dialogos::abrir(self.hwnd, dialogos::Que::Plano) {
                    self.abrir_ruta(r);
                }
            }
            Accion::AbrirModelo => {
                if let Some(r) = dialogos::abrir(self.hwnd, dialogos::Que::Modelo) {
                    self.abrir_ruta(r);
                }
            }
            Accion::AbrirReciente(r) => self.abrir_ruta(r),
            Accion::MetradoBim => self.metrado_bim(),
            Accion::Inicio => {
                if self.puede_dejar_el_dibujo() {
                    self.pantalla = Pantalla::Inicio;
                    self.aviso_inicio = None;
                }
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
        let Some(destino) = dialogos::guardar_csv(hwnd, &format!("{base} - metrado.csv")) else {
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
        let Some(modelo) = dialogos::abrir(self.hwnd, dialogos::Que::Modelo) else {
            return;
        };
        let base = modelo
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let Some(destino) = dialogos::guardar_csv(self.hwnd, &format!("{base} - metrado BIM.csv"))
        else {
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
        let aviso = self.aviso_inicio.clone();
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
                zonas = inicio::pintar(p, w, h, e, recientes, cursor);
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
                zonas = m.pintar(p, w, h, e);
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
            m.caja_plano = m
                .plano
                .gpu
                .as_ref()
                .map(|(modelo, _)| crate::mesa::caja_de_modelo(modelo));
            tomado = m.plano.gpu.take();
        }
        let vista = match (&self.pantalla, &tomado) {
            (Pantalla::Mesa(m), Some((modelo, _))) => {
                Some(crate::plano::vista_de(modelo, &m.camara, self.w, self.h))
            }
            _ => None,
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
        let r = match (&tomado, vista) {
            (Some((_, pg)), Some(v)) => {
                self.gpu
                    .dibujar_con(tema::FONDO_PLANO, &[(pg, v)], &mut encima)
            }
            _ => self.gpu.dibujar_con(tema::FONDO_PLANO, &[], &mut encima),
        };
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
