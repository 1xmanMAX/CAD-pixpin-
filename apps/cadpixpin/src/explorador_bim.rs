//! **El explorador de datos BIM**: una ventana propia (su hilo, su `Gpu` y
//! su `MotorRender`, como la principal) que lee un IFC o un Revit con
//! `cad-bim-datos` en otro hilo y muestra:
//!
//! - el fichero y el proyecto, con la miniatura de Revit;
//! - el arbol Proyecto ▸ Edificio ▸ Nivel ▸ Categoria ▸ Elemento, con cuantos;
//! - las propiedades del elemento elegido, por conjunto;
//! - la tabla de planificacion de un nivel o una categoria (columnas que se
//!   ordenan con un clic);
//! - una busqueda (por nombre, categoria o valor de un parametro);
//! - exportar CSV, JSON, Excel y, de un Revit, IFC y GLB; y verlo en 3D.
//!
//! Las listas solo pintan las filas que se ven: un modelo de cien mil
//! elementos se mueve igual que uno de diez.

use std::cell::RefCell;
use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use cad_bim_datos::exportar::{self, Celda, Tabla};
use cad_bim_datos::{Formato, ModeloBim, Opciones};
use pixpin_cad::gpu::Gpu;
use pixpin_render::{Color, MotorRender, Pintor, RectF};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;
use windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{HSTRING, PCWSTR, w};

use crate::bim::Aviso;
use crate::ui::{dentro, rect};
use crate::{dialogos, tema};

/// **Abre el explorador de un modelo** en su propia ventana y vuelve enseguida.
pub fn abrir(ruta: PathBuf, avisos: Sender<Aviso>) {
    let _ = std::thread::Builder::new()
        .name("bim-explorador".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            // Direct2D, WIC y los dialogos de guardar: COM en este hilo.
            // SAFETY: inicializar COM en un hilo propio que no lo tenia.
            unsafe {
                let _ = windows::Win32::System::Com::CoInitializeEx(
                    None,
                    windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
                );
            }
            if let Err(e) = correr(ruta.clone(), avisos.clone()) {
                let _ = avisos.send(Aviso::Texto(format!(
                    "No se pudo abrir el explorador de {}: {e}",
                    ruta.display()
                )));
            }
        });
}

// ------------------------------------------------------------ la ventana

#[derive(Debug, Clone, Copy)]
enum Ev {
    Mover(f32, f32),
    Bajar(f32, f32),
    /// Cuanto, donde (x) y si va de lado.
    Rueda(f32, f32, bool),
    Tecla(u32),
    Caracter(char),
    Tamano(u32, u32),
    Fuera,
    Cerrar,
}

thread_local! {
    static COLA: RefCell<VecDeque<Ev>> = const { RefCell::new(VecDeque::new()) };
}

fn apuntar(e: Ev) {
    COLA.with(|c| c.borrow_mut().push_back(e));
}

fn xy(lp: LPARAM) -> (f32, f32) {
    (
        (lp.0 & 0xffff) as i16 as f32,
        ((lp.0 >> 16) & 0xffff) as i16 as f32,
    )
}

const WM_MOUSEHWHEEL_: u32 = 0x020E;
const WM_MOUSELEAVE_: u32 = 0x02A3;

unsafe extern "system" fn procedimiento(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_MOUSEMOVE => {
            let (x, y) = xy(lp);
            apuntar(Ev::Mover(x, y));
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
        WM_MOUSELEAVE_ => apuntar(Ev::Fuera),
        WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => {
            let (x, y) = xy(lp);
            apuntar(Ev::Bajar(x, y));
        }
        WM_MOUSEWHEEL | WM_MOUSEHWHEEL_ => {
            let (sx, sy) = xy(lp);
            let mut p = POINT {
                x: sx as i32,
                y: sy as i32,
            };
            // SAFETY: ventana propia; punto local.
            let _ = unsafe { ScreenToClient(hwnd, &mut p) };
            let delta = ((wp.0 >> 16) & 0xffff) as i16 as f32 / 120.0;
            // Con Mayusculas, la rueda va de lado (como en Excel).
            let mayus = (wp.0 & 0x0004) != 0;
            let lado = msg == WM_MOUSEHWHEEL_ || mayus;
            let delta = if msg == WM_MOUSEHWHEEL_ {
                -delta
            } else {
                delta
            };
            apuntar(Ev::Rueda(delta, p.x as f32, lado));
        }
        WM_KEYDOWN => apuntar(Ev::Tecla(wp.0 as u32)),
        WM_CHAR => {
            if let Some(c) = char::from_u32(wp.0 as u32) {
                apuntar(Ev::Caracter(c));
            }
            return LRESULT(0);
        }
        WM_SIZE => apuntar(Ev::Tamano(
            (lp.0 & 0xffff) as u32,
            ((lp.0 >> 16) & 0xffff) as u32,
        )),
        WM_ERASEBKGND => return LRESULT(1),
        WM_CLOSE => {
            apuntar(Ev::Cerrar);
            return LRESULT(0);
        }
        _ => {}
    }
    // SAFETY: lo demas, como siempre.
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

fn crear_ventana(titulo: &str) -> windows::core::Result<HWND> {
    // SAFETY: clase y ventana propias; cadenas vivas durante las llamadas.
    unsafe {
        let inst: windows::Win32::Foundation::HINSTANCE = GetModuleHandleW(None)?.into();
        let c = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_DBLCLKS,
            lpfnWndProc: Some(procedimiento),
            hInstance: inst,
            hIcon: LoadIconW(Some(inst), PCWSTR(1 as _)).unwrap_or_default(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: w!("CadPixpinBim"),
            ..Default::default()
        };
        // Si ya estaba registrada (otro explorador abierto), falla y vale.
        RegisterClassExW(&c);
        let t = HSTRING::from(titulo);
        let hwnd = CreateWindowExW(
            WS_EX_APPWINDOW,
            w!("CadPixpinBim"),
            &t,
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1360,
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
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
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

fn ctrl_pulsada() -> bool {
    // SAFETY: lectura del estado del teclado.
    unsafe {
        windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(
            windows::Win32::UI::Input::KeyboardAndMouse::VK_CONTROL.0 as i32,
        ) < 0
    }
}

/// Lo que llega de los hilos de lectura y de exportar.
enum Llega {
    Avance(String),
    Listo(Box<Result<ModeloBim, String>>),
    Hecho(String),
}

fn correr(ruta: PathBuf, avisos: Sender<Aviso>) -> Result<(), String> {
    let nombre = ruta
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let hwnd = crear_ventana(&format!("Datos BIM — {nombre}")).map_err(|e| e.to_string())?;
    let (w, h) = tamano_cliente(hwnd);
    let gpu = Gpu::nueva(hwnd, w, h).map_err(|e| format!("sin Direct3D: {e}"))?;
    let motor = MotorRender::nuevo(&gpu.dispositivo).ok();
    // SAFETY: ventana propia.
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    let (tx, rx) = channel();
    // La lectura, en otro hilo con pila grande (los IFC recurren hondo).
    {
        let tx = tx.clone();
        let r = ruta.clone();
        let _ = std::thread::Builder::new()
            .name("bim-datos".into())
            .stack_size(64 * 1024 * 1024)
            .spawn(move || {
                let t2 = tx.clone();
                let m = cad_bim_datos::leer(&r, Opciones::default(), &mut |a| {
                    let _ = t2.send(Llega::Avance(a.to_string()));
                });
                let _ = tx.send(Llega::Listo(Box::new(m)));
            });
    }
    let mut x = Explorador {
        hwnd,
        gpu,
        motor,
        v: Vista::nueva(ruta, avisos, tx, dpi as f32 / 96.0, w, h),
        rx,
    };
    let mut sucio = true;
    loop {
        // SAFETY: bucle de mensajes de este hilo.
        unsafe {
            let mut m = MSG::default();
            while PeekMessageW(&mut m, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&m);
                DispatchMessageW(&m);
            }
        }
        let mut cerrar = false;
        while let Some(ev) = COLA.with(|c| c.borrow_mut().pop_front()) {
            match ev {
                Ev::Cerrar => cerrar = true,
                Ev::Tamano(nw, nh) => {
                    if nw > 0 && nh > 0 && (nw, nh) != (x.v.w, x.v.h) {
                        x.v.w = nw;
                        x.v.h = nh;
                        let _ = x.gpu.redimensionar(nw, nh);
                    }
                }
                otro => x.v.atender(otro),
            }
            sucio = true;
        }
        if cerrar {
            break;
        }
        while let Ok(l) = x.rx.try_recv() {
            x.v.llega(l);
            sucio = true;
        }
        if sucio {
            x.pintar();
            sucio = false;
        }
        let espera = if x.v.ocupado() { 40 } else { u32::MAX };
        // SAFETY: esperar mensajes de este hilo.
        unsafe {
            let _ = MsgWaitForMultipleObjectsEx(None, espera, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
        }
    }
    // SAFETY: ventana propia.
    let _ = unsafe { DestroyWindow(hwnd) };
    Ok(())
}

struct Explorador {
    hwnd: HWND,
    gpu: Gpu,
    motor: Option<MotorRender>,
    v: Vista,
    rx: Receiver<Llega>,
}

impl Explorador {
    fn pintar(&mut self) {
        let Some(motor) = self.motor.as_ref() else {
            let _ = self.gpu.dibujar(tema::FONDO_PLANO, &[]);
            return;
        };
        let v = &mut self.v;
        let hwnd = self.hwnd;
        let mut encima =
            |_: &windows::Win32::Graphics::Direct3D11::ID3D11Device,
             tex: &windows::Win32::Graphics::Direct3D11::ID3D11Texture2D| {
                if let Ok(b) = motor.destino_backbuffer(tex)
                    && let Err(err) = motor.dibujar(&b, |p| v.pintar(p, motor))
                {
                    tracing::warn!(?err, "no se pudo pintar el explorador");
                }
            };
        if let Err(err) = self.gpu.dibujar_con(tema::FONDO_PLANO, &[], &mut encima) {
            tracing::warn!(?err, "no se pudo dibujar el explorador");
        }
        // Lo que pidieron los clics del fotograma (dialogos modales: fuera de pintar).
        if let Some(b) = self.v.pendiente.take() {
            self.v.boton(hwnd, b);
            apuntar(Ev::Mover(-1.0, -1.0));
        }
    }
}

// ------------------------------------------------------------ el estado

/// Un nodo del arbol que se abre y se cierra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Clave {
    Proyecto,
    Edificio(usize),
    Nivel(usize),
    Categoria(usize, usize),
}

/// Lo que se ve a la derecha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sel {
    Info,
    Nivel(usize),
    Categoria(usize, usize),
    Elemento(usize),
}

/// Un nivel del arbol (o «sin nivel») con sus categorias y elementos.
struct Grupo {
    nombre: String,
    categorias: Vec<(String, Vec<usize>)>,
}

struct Edificio {
    nombre: String,
    grupos: Vec<usize>,
}

/// El arbol ya agrupado y, de cada elemento, todo su texto en minusculas
/// (la busqueda es un `contains` por elemento).
struct Indice {
    edificios: Vec<Edificio>,
    grupos: Vec<Grupo>,
    pajar: Vec<String>,
}

impl Indice {
    fn de(m: &ModeloBim) -> Indice {
        let mut grupos: Vec<Grupo> = Vec::new();
        let mut por_nivel: Vec<Vec<usize>> = vec![Vec::new(); m.niveles.len() + 1];
        for (i, e) in m.elementos.iter().enumerate() {
            let k = e
                .nivel
                .filter(|&n| n < m.niveles.len())
                .unwrap_or(m.niveles.len());
            por_nivel[k].push(i);
        }
        let mut de_nivel: Vec<Option<usize>> = vec![None; m.niveles.len() + 1];
        for (k, ids) in por_nivel.into_iter().enumerate() {
            if k == m.niveles.len() && ids.is_empty() {
                continue;
            }
            let mut cats: std::collections::BTreeMap<String, Vec<usize>> = Default::default();
            for i in ids {
                cats.entry(m.elementos[i].categoria.clone())
                    .or_default()
                    .push(i);
            }
            let categorias = cats
                .into_iter()
                .map(|(c, mut v)| {
                    v.sort_by(|a, b| {
                        exportar::orden_natural(&m.elementos[*a].nombre, &m.elementos[*b].nombre)
                    });
                    (c, v)
                })
                .collect();
            let nombre = match m.niveles.get(k) {
                Some(n) => match n.cota {
                    Some(c) => format!("{} ({} m)", n.nombre, cad_bim_datos::numero_corto(c)),
                    None => n.nombre.clone(),
                },
                None => "(sin nivel)".into(),
            };
            de_nivel[k] = Some(grupos.len());
            grupos.push(Grupo { nombre, categorias });
        }
        let defecto = m
            .proyecto
            .edificio
            .clone()
            .unwrap_or_else(|| "Edificio".into());
        let mut edificios: Vec<Edificio> = Vec::new();
        for (k, g) in de_nivel.iter().enumerate() {
            let Some(g) = *g else { continue };
            let n = m
                .niveles
                .get(k)
                .and_then(|n| n.edificio.clone())
                .unwrap_or_else(|| defecto.clone());
            match edificios.iter_mut().find(|e| e.nombre == n) {
                Some(e) => e.grupos.push(g),
                None => edificios.push(Edificio {
                    nombre: n,
                    grupos: vec![g],
                }),
            }
        }
        let pajar = m
            .elementos
            .iter()
            .map(|e| {
                let mut s = String::new();
                for t in [
                    &e.nombre,
                    &e.categoria,
                    &e.clase,
                    &e.familia,
                    &e.tipo,
                    &e.id,
                ] {
                    s.push_str(t);
                    s.push('\n');
                }
                if let Some(g) = &e.global_id {
                    s.push_str(g);
                    s.push('\n');
                }
                for x in &e.materiales {
                    s.push_str(x);
                    s.push('\n');
                }
                for c in &e.conjuntos {
                    for p in &c.propiedades {
                        s.push_str(&p.valor.texto());
                        s.push('\n');
                    }
                }
                s.to_lowercase()
            })
            .collect();
        Indice {
            edificios,
            grupos,
            pajar,
        }
    }
}

/// Una fila del arbol tal como se pinta.
struct FilaArbol {
    sangria: u8,
    texto: String,
    cuenta: Option<usize>,
    clave: Option<Clave>,
    elemento: Option<usize>,
    abierta: bool,
}

/// Una fila de la lista de propiedades.
enum FilaProp {
    Titulo(String),
    Par(String, String),
}

/// Lo que hace un clic.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Clic {
    Fila(usize),
    Columna(usize),
    FilaTabla(usize),
    Boton(Boton),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Boton {
    Csv,
    CsvCategorias,
    Json,
    Excel,
    Metrado,
    Ifc,
    Glb,
    Ver3d,
    Volver,
    Limpiar,
}

struct Vista {
    ruta: PathBuf,
    avisos: Sender<Aviso>,
    tx: Sender<Llega>,
    e: f32,
    w: u32,
    h: u32,
    estado: String,
    cargando: bool,
    exportando: usize,
    modelo: Option<Arc<ModeloBim>>,
    indice: Option<Indice>,
    abiertos: HashSet<Clave>,
    busqueda: String,
    coincide: Option<Vec<bool>>,
    filas: Vec<FilaArbol>,
    /// Primera fila visible del arbol.
    desde_arbol: f32,
    sel: Sel,
    /// A donde vuelve «Volver» (la tabla de donde se eligio el elemento).
    anterior: Option<Sel>,
    props: Vec<FilaProp>,
    desde_der: f32,
    tabla: Option<Tabla>,
    orden: Option<(usize, bool)>,
    anchos: Vec<f32>,
    /// Pixeles movidos a la derecha en la tabla.
    lado: f32,
    zonas: Vec<(RectF, Clic)>,
    cursor: Option<(f32, f32)>,
    miniatura: Option<ID2D1Bitmap1>,
    miniatura_hecha: bool,
    /// Un boton que pide un dialogo: se atiende despues de pintar.
    pendiente: Option<Boton>,
}

impl Vista {
    fn nueva(
        ruta: PathBuf,
        avisos: Sender<Aviso>,
        tx: Sender<Llega>,
        e: f32,
        w: u32,
        h: u32,
    ) -> Vista {
        Vista {
            ruta,
            avisos,
            tx,
            e,
            w,
            h,
            estado: "Leyendo…".into(),
            cargando: true,
            exportando: 0,
            modelo: None,
            indice: None,
            abiertos: [Clave::Proyecto].into_iter().collect(),
            busqueda: String::new(),
            coincide: None,
            filas: Vec::new(),
            desde_arbol: 0.0,
            sel: Sel::Info,
            anterior: None,
            props: Vec::new(),
            desde_der: 0.0,
            tabla: None,
            orden: None,
            anchos: Vec::new(),
            lado: 0.0,
            zonas: Vec::new(),
            cursor: None,
            miniatura: None,
            miniatura_hecha: false,
            pendiente: None,
        }
    }

    fn ocupado(&self) -> bool {
        self.cargando || self.exportando > 0
    }

    fn es_revit(&self) -> bool {
        self.modelo
            .as_ref()
            .is_some_and(|m| m.archivo.formato == Formato::Revit)
    }

    fn llega(&mut self, l: Llega) {
        let l = match l {
            Llega::Listo(r) => Ok(*r),
            otra => Err(otra),
        };
        match l {
            Err(Llega::Avance(t)) => self.estado = t,
            Err(Llega::Hecho(t)) => {
                self.exportando = self.exportando.saturating_sub(1);
                self.estado = t;
            }
            Err(Llega::Listo(_)) => {}
            Ok(Ok(m)) => {
                self.cargando = false;
                self.estado = format!(
                    "{} elementos, {} niveles, {} categorias, {} materiales{}",
                    m.elementos.len(),
                    m.niveles.len(),
                    m.categorias.len(),
                    m.materiales.len(),
                    if m.avisos.is_empty() {
                        String::new()
                    } else {
                        format!(" · {} avisos (ver la ficha del proyecto)", m.avisos.len())
                    }
                );
                let ix = Indice::de(&m);
                for k in 0..ix.edificios.len() {
                    self.abiertos.insert(Clave::Edificio(k));
                }
                self.indice = Some(ix);
                self.modelo = Some(Arc::new(m));
                self.rehacer_filas();
                self.elegir(Sel::Info);
            }
            Ok(Err(e)) => {
                self.cargando = false;
                self.estado = format!("No se pudo leer: {e}");
            }
        }
    }

    fn ve(&self, i: usize) -> bool {
        self.coincide.as_ref().is_none_or(|c| c[i])
    }

    fn abierta(&self, k: Clave) -> bool {
        self.coincide.is_some() || self.abiertos.contains(&k)
    }

    /// **Las filas del arbol** segun lo abierto y lo buscado.
    fn rehacer_filas(&mut self) {
        let mut filas = Vec::new();
        let (Some(m), Some(ix)) = (self.modelo.as_ref(), self.indice.as_ref()) else {
            self.filas = filas;
            return;
        };
        let cuenta = |v: &[usize]| v.iter().filter(|&&i| self.ve(i)).count();
        let cuenta_grupo = |g: &Grupo| g.categorias.iter().map(|(_, v)| cuenta(v)).sum::<usize>();
        let total: usize = ix.grupos.iter().map(cuenta_grupo).sum();
        let proyecto = m
            .proyecto
            .nombre
            .clone()
            .unwrap_or_else(|| m.archivo.nombre.clone());
        let ab = self.abierta(Clave::Proyecto);
        filas.push(FilaArbol {
            sangria: 0,
            texto: proyecto,
            cuenta: Some(total),
            clave: Some(Clave::Proyecto),
            elemento: None,
            abierta: ab,
        });
        if ab {
            for (k, ed) in ix.edificios.iter().enumerate() {
                let n: usize = ed.grupos.iter().map(|&g| cuenta_grupo(&ix.grupos[g])).sum();
                if self.coincide.is_some() && n == 0 {
                    continue;
                }
                let ab = self.abierta(Clave::Edificio(k));
                filas.push(FilaArbol {
                    sangria: 1,
                    texto: ed.nombre.clone(),
                    cuenta: Some(n),
                    clave: Some(Clave::Edificio(k)),
                    elemento: None,
                    abierta: ab,
                });
                if !ab {
                    continue;
                }
                for &g in &ed.grupos {
                    let gr = &ix.grupos[g];
                    let n = cuenta_grupo(gr);
                    if self.coincide.is_some() && n == 0 {
                        continue;
                    }
                    let ab = self.abierta(Clave::Nivel(g));
                    filas.push(FilaArbol {
                        sangria: 2,
                        texto: gr.nombre.clone(),
                        cuenta: Some(n),
                        clave: Some(Clave::Nivel(g)),
                        elemento: None,
                        abierta: ab,
                    });
                    if !ab {
                        continue;
                    }
                    for (c, (nombre, ids)) in gr.categorias.iter().enumerate() {
                        let n = cuenta(ids);
                        if self.coincide.is_some() && n == 0 {
                            continue;
                        }
                        let ab = self.abierta(Clave::Categoria(g, c));
                        filas.push(FilaArbol {
                            sangria: 3,
                            texto: nombre.clone(),
                            cuenta: Some(n),
                            clave: Some(Clave::Categoria(g, c)),
                            elemento: None,
                            abierta: ab,
                        });
                        if !ab {
                            continue;
                        }
                        for &i in ids.iter().filter(|&&i| self.ve(i)) {
                            let e = &m.elementos[i];
                            let t = if e.nombre.is_empty() {
                                format!("{} {}", e.clase, e.id)
                            } else {
                                e.nombre.clone()
                            };
                            filas.push(FilaArbol {
                                sangria: 4,
                                texto: t,
                                cuenta: None,
                                clave: None,
                                elemento: Some(i),
                                abierta: false,
                            });
                        }
                    }
                }
            }
        }
        self.filas = filas;
        let max = self.filas.len().saturating_sub(1) as f32;
        self.desde_arbol = self.desde_arbol.min(max).max(0.0);
    }

    fn buscar(&mut self) {
        let t = self.busqueda.trim().to_lowercase();
        self.coincide = match (&self.indice, t.is_empty()) {
            (Some(ix), false) => Some(ix.pajar.iter().map(|s| s.contains(&t)).collect()),
            _ => None,
        };
        self.desde_arbol = 0.0;
        self.rehacer_filas();
        // Una tabla abierta se rehace con lo que coincide.
        if matches!(self.sel, Sel::Nivel(_) | Sel::Categoria(..)) {
            self.elegir(self.sel);
        }
    }

    /// **Elige lo que se ve a la derecha** y rehace su lista o su tabla.
    fn elegir(&mut self, s: Sel) {
        self.sel = s;
        self.desde_der = 0.0;
        self.props.clear();
        let Some(m) = self.modelo.clone() else { return };
        match s {
            Sel::Info => {
                self.tabla = None;
                self.props = filas_de_info(&m);
            }
            Sel::Elemento(i) => {
                self.tabla = None;
                self.props = filas_de_elemento(&m, i);
            }
            Sel::Nivel(_) | Sel::Categoria(..) => {
                let Some(ix) = &self.indice else { return };
                let mut ids: Vec<usize> = match s {
                    Sel::Nivel(g) => ix.grupos[g]
                        .categorias
                        .iter()
                        .flat_map(|(_, v)| v.iter().copied())
                        .collect(),
                    Sel::Categoria(g, c) => ix.grupos[g].categorias[c].1.clone(),
                    _ => Vec::new(),
                };
                ids.retain(|&i| self.ve(i));
                let mut t = exportar::tabla(&m, &ids, true);
                if let Some((c, asc)) = self.orden.filter(|(c, _)| *c < t.columnas.len()) {
                    t.ordenar(c, asc);
                }
                self.tabla = Some(t);
                self.anchos.clear();
                self.lado = 0.0;
            }
        }
    }

    fn atender(&mut self, ev: Ev) {
        let e = self.e;
        match ev {
            Ev::Mover(x, y) => {
                self.cursor = (x >= 0.0).then_some((x, y));
            }
            Ev::Fuera => self.cursor = None,
            Ev::Bajar(x, y) => {
                let Some((_, c)) = self
                    .zonas
                    .iter()
                    .rev()
                    .find(|(r, _)| dentro(r, x, y))
                    .copied()
                else {
                    return;
                };
                self.clic(c);
            }
            Ev::Rueda(d, x, lado) => {
                if x < self.ancho_arbol() {
                    self.desde_arbol = (self.desde_arbol - d * 3.0)
                        .clamp(0.0, self.filas.len().saturating_sub(1) as f32);
                } else if lado && self.tabla.is_some() {
                    let total: f32 = self.anchos.iter().sum();
                    self.lado =
                        (self.lado - d * 120.0 * e).clamp(0.0, (total - 200.0 * e).max(0.0));
                } else {
                    let n = self
                        .tabla
                        .as_ref()
                        .map_or(self.props.len(), |t| t.filas.len());
                    self.desde_der =
                        (self.desde_der - d * 3.0).clamp(0.0, n.saturating_sub(1) as f32);
                }
            }
            Ev::Tecla(vk) => match vk {
                0x1B => {
                    if !self.busqueda.is_empty() {
                        self.busqueda.clear();
                        self.buscar();
                    } else if let Some(a) = self.anterior.take() {
                        self.elegir(a);
                    }
                }
                // Re Pag / Av Pag en lo de la derecha.
                0x21 => self.desde_der = (self.desde_der - 20.0).max(0.0),
                0x22 => {
                    let n = self
                        .tabla
                        .as_ref()
                        .map_or(self.props.len(), |t| t.filas.len());
                    self.desde_der = (self.desde_der + 20.0).min(n.saturating_sub(1) as f32);
                }
                _ => {}
            },
            Ev::Caracter(c) => {
                if c == '\u{8}' {
                    if self.busqueda.pop().is_some() {
                        self.buscar();
                    }
                } else if !c.is_control() && !ctrl_pulsada() && self.busqueda.chars().count() < 80 {
                    self.busqueda.push(c);
                    self.buscar();
                }
            }
            Ev::Tamano(..) | Ev::Cerrar => {}
        }
    }

    fn clic(&mut self, c: Clic) {
        match c {
            Clic::Fila(k) => {
                let Some(f) = self.filas.get(k) else { return };
                if let Some(i) = f.elemento {
                    self.anterior = None;
                    self.elegir(Sel::Elemento(i));
                    return;
                }
                let Some(cl) = f.clave else { return };
                if self.coincide.is_none() {
                    if !self.abiertos.remove(&cl) {
                        self.abiertos.insert(cl);
                    }
                    self.rehacer_filas();
                }
                self.anterior = None;
                self.elegir(match cl {
                    Clave::Proyecto | Clave::Edificio(_) => Sel::Info,
                    Clave::Nivel(g) => Sel::Nivel(g),
                    Clave::Categoria(g, c) => Sel::Categoria(g, c),
                });
            }
            Clic::Columna(c) => {
                let asc = !matches!(self.orden, Some((x, true)) if x == c);
                self.orden = Some((c, asc));
                if let Some(t) = &mut self.tabla {
                    t.ordenar(c, asc);
                }
            }
            Clic::FilaTabla(k) => {
                if let Some(&i) = self.tabla.as_ref().and_then(|t| t.elementos.get(k)) {
                    self.anterior = Some(self.sel);
                    self.elegir(Sel::Elemento(i));
                }
            }
            Clic::Boton(Boton::Volver) => {
                if let Some(a) = self.anterior.take() {
                    self.elegir(a);
                }
            }
            Clic::Boton(Boton::Limpiar) => {
                self.busqueda.clear();
                self.buscar();
            }
            Clic::Boton(b) => self.pendiente = Some(b),
        }
    }

    /// Los botones que abren un dialogo o una ventana.
    fn boton(&mut self, hwnd: HWND, b: Boton) {
        if b == Boton::Ver3d {
            crate::bim::abrir(self.ruta.clone(), self.avisos.clone());
            self.estado = "Abriendo en 3D…".into();
            return;
        }
        let Some(m) = self.modelo.clone() else { return };
        let base = self
            .ruta
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "modelo".into());
        let (titulo, nombre, patron, sugerido) = match b {
            Boton::Csv => ("Exportar todo a CSV", "CSV", "*.csv", format!("{base}.csv")),
            Boton::CsvCategorias => (
                "Exportar un CSV por categoria",
                "CSV",
                "*.csv",
                format!("{base}.csv"),
            ),
            Boton::Json => (
                "Exportar el modelo a JSON",
                "JSON",
                "*.json",
                format!("{base}.json"),
            ),
            Boton::Excel => (
                "Exportar a Excel (una hoja por categoria)",
                "Excel",
                "*.xlsx",
                format!("{base}.xlsx"),
            ),
            Boton::Metrado => (
                "Metrado por nivel, categoria y tipo",
                "CSV",
                "*.csv",
                format!("{base} - metrado.csv"),
            ),
            Boton::Ifc => (
                "Exportar el Revit a IFC4",
                "IFC",
                "*.ifc",
                format!("{base}.ifc"),
            ),
            Boton::Glb => (
                "Exportar el Revit a GLB",
                "glTF binario",
                "*.glb",
                format!("{base}.glb"),
            ),
            _ => return,
        };
        let Some(destino) = dialogos::elegir(
            hwnd,
            true,
            titulo,
            &[(nombre.into(), patron.into())],
            &sugerido,
        ) else {
            return;
        };
        self.exportando += 1;
        self.estado = format!("Exportando {}…", destino.display());
        let tx = self.tx.clone();
        let ruta = self.ruta.clone();
        let _ = std::thread::Builder::new()
            .name("bim-exportar".into())
            .stack_size(64 * 1024 * 1024)
            .spawn(move || {
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match b {
                    Boton::CsvCategorias => exportar::csv_por_categoria(&m, &destino)
                        .map(|v| format!("{} CSV escritos junto a {}", v.len(), destino.display())),
                    Boton::Metrado => {
                        let f = cad_bim_datos::metrado::por_tipo(&m);
                        std::fs::write(&destino, cad_bim_datos::metrado::csv(&f))
                            .map_err(|e| e.to_string())
                            .map(|_| {
                                format!("Metrado de {} partidas en {}", f.len(), destino.display())
                            })
                    }
                    Boton::Ifc => exportar::revit_a_ifc(&ruta)
                        .and_then(|t| std::fs::write(&destino, t).map_err(|e| e.to_string()))
                        .map(|_| format!("IFC4 escrito en {}", destino.display())),
                    Boton::Glb => exportar::revit_a_glb(&ruta)
                        .and_then(|t| std::fs::write(&destino, t).map_err(|e| e.to_string()))
                        .map(|_| format!("GLB escrito en {}", destino.display())),
                    _ => exportar::exportar(&m, &destino)
                        .map(|_| format!("Exportado a {}", destino.display())),
                }))
                .unwrap_or_else(|_| Err("la exportacion se detuvo".into()));
                let _ = tx.send(Llega::Hecho(
                    r.unwrap_or_else(|e| format!("No se pudo exportar: {e}")),
                ));
            });
    }

    fn ancho_arbol(&self) -> f32 {
        (340.0 * self.e).min(self.w as f32 * 0.4)
    }

    // ------------------------------------------------------------ pintar

    fn pintar(&mut self, p: &Pintor, motor: &MotorRender) {
        let e = self.e;
        let (w, h) = (self.w as f32, self.h as f32);
        let mut zonas: Vec<(RectF, Clic)> = Vec::new();
        p.rellenar(rect(0.0, 0.0, w, h), tema::FONDO);
        if !self.miniatura_hecha {
            self.miniatura_hecha = self.modelo.is_some();
            self.miniatura = self
                .modelo
                .as_ref()
                .and_then(|m| m.archivo.vista_previa.as_deref())
                .and_then(cad_bim_datos::png_a_rgba)
                .and_then(|px| {
                    motor
                        .bitmap_desde_pixeles_premultiplicado(px.ancho, px.alto, &px.rgba)
                        .ok()
                });
        }
        let tam = tema::LETRA * e;
        let peq = tema::LETRA_PEQUENA * e;
        let alto_barra = 44.0 * e;
        let alto_estado = 26.0 * e;

        // La barra de arriba: el nombre y los botones.
        p.rellenar(rect(0.0, 0.0, w, alto_barra), tema::PANEL);
        let nombre = self
            .ruta
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        p.texto_linea(
            &nombre,
            14.0 * e,
            13.0 * e,
            14.0 * e,
            300.0 * e,
            tema::TEXTO,
        );
        let listo = self.modelo.is_some() && !self.cargando;
        let mut botones = vec![
            (Boton::Ver3d, "Ver en 3D", true),
            (Boton::Metrado, "Metrado", listo),
            (Boton::Excel, "Excel", listo),
            (Boton::Json, "JSON", listo),
            (Boton::CsvCategorias, "CSV por categoría", listo),
            (Boton::Csv, "CSV", listo),
        ];
        if self.es_revit() {
            botones.insert(1, (Boton::Glb, "GLB", true));
            botones.insert(1, (Boton::Ifc, "IFC", true));
        }
        let mut x = w - 10.0 * e;
        for (b, t, activo) in botones {
            let ancho = p.medir_texto(t, tam).0 + 24.0 * e;
            x -= ancho;
            let r = rect(x, 8.0 * e, ancho, alto_barra - 16.0 * e);
            let sobre = self.cursor.is_some_and(|(cx, cy)| dentro(&r, cx, cy));
            let principal = b == Boton::Ver3d;
            let fondo = if principal {
                Some(if sobre {
                    tema::rgb(0x40, 0x9C, 0xFF)
                } else {
                    tema::ACENTO
                })
            } else if sobre && activo {
                Some(tema::PANEL_ALTO)
            } else {
                None
            };
            if let Some(f) = fondo {
                p.rellenar_redondeado(r, 6.0 * e, f);
            } else {
                p.trazar(r, 1.0, tema::BORDE);
            }
            let c = if activo {
                tema::TEXTO
            } else {
                tema::TEXTO_APAGADO
            };
            let (tw, th) = p.medir_texto(t, tam);
            p.texto(
                t,
                r.x + (r.ancho - tw) / 2.0,
                r.y + (r.alto - th) / 2.0,
                tam,
                c,
            );
            if activo {
                zonas.push((r, Clic::Boton(b)));
            }
            x -= 6.0 * e;
            if b == Boton::Csv {
                p.texto("Exportar:", x - 62.0 * e, 15.0 * e, peq, tema::TEXTO_SUAVE);
                x -= 70.0 * e;
            }
        }

        // La izquierda: la busqueda y el arbol.
        let ai = self.ancho_arbol();
        let y0 = alto_barra;
        let abajo = h - alto_estado;
        p.rellenar(rect(0.0, y0, ai, abajo - y0), tema::PANEL);
        p.linea((ai, y0), (ai, abajo), 1.0, tema::BORDE);
        let rb = rect(10.0 * e, y0 + 10.0 * e, ai - 20.0 * e, 30.0 * e);
        p.rellenar_redondeado(rb, 6.0 * e, tema::FONDO);
        p.trazar(
            rb,
            1.0,
            if self.busqueda.is_empty() {
                tema::BORDE
            } else {
                tema::ACENTO
            },
        );
        if self.busqueda.is_empty() {
            p.texto(
                "Buscar nombre, categoria o valor…",
                rb.x + 10.0 * e,
                rb.y + 7.0 * e,
                tam,
                tema::TEXTO_APAGADO,
            );
        } else {
            let t = format!("{}|", self.busqueda);
            p.texto_linea(
                &t,
                rb.x + 10.0 * e,
                rb.y + 7.0 * e,
                tam,
                rb.ancho - 44.0 * e,
                tema::TEXTO,
            );
            let rx = rect(
                rb.x + rb.ancho - 28.0 * e,
                rb.y + 3.0 * e,
                24.0 * e,
                24.0 * e,
            );
            p.texto(
                "x",
                rx.x + 7.0 * e,
                rx.y + 2.0 * e,
                15.0 * e,
                tema::TEXTO_SUAVE,
            );
            zonas.push((rx, Clic::Boton(Boton::Limpiar)));
        }
        let alto_fila = 22.0 * e;
        let ya = y0 + 50.0 * e;
        let visibles = ((abajo - ya) / alto_fila).ceil() as usize + 1;
        let desde = self.desde_arbol.floor() as usize;
        p.con_recorte(rect(0.0, ya, ai, abajo - ya), |p| {
            for (k, f) in self.filas.iter().enumerate().skip(desde).take(visibles) {
                let y = ya + (k - desde) as f32 * alto_fila;
                let r = rect(0.0, y, ai, alto_fila);
                let elegida = match (self.sel, f.clave, f.elemento) {
                    (Sel::Elemento(i), _, Some(j)) => i == j,
                    (Sel::Nivel(g), Some(Clave::Nivel(h)), _) => g == h,
                    (Sel::Categoria(g, c), Some(Clave::Categoria(h, d)), _) => (g, c) == (h, d),
                    _ => false,
                };
                let sobre = self.cursor.is_some_and(|(cx, cy)| dentro(&r, cx, cy));
                if elegida {
                    p.rellenar(r, tema::ACENTO_SUAVE);
                } else if sobre {
                    p.rellenar(r, tema::PANEL_ALTO);
                }
                let x = 12.0 * e + f.sangria as f32 * 14.0 * e;
                if f.clave.is_some() {
                    let d = if f.abierta { Dir::Abajo } else { Dir::Derecha };
                    triangulo(
                        p,
                        x + 5.0 * e,
                        y + alto_fila / 2.0,
                        4.0 * e,
                        d,
                        tema::TEXTO_SUAVE,
                    );
                }
                let color = if f.elemento.is_some() {
                    tema::TEXTO_SUAVE
                } else {
                    tema::TEXTO
                };
                let cuenta = f.cuenta.map(|n| n.to_string()).unwrap_or_default();
                let cw = p.medir_texto(&cuenta, peq).0;
                p.texto_linea(
                    &f.texto,
                    x + 14.0 * e,
                    y + 3.0 * e,
                    tam,
                    ai - x - 30.0 * e - cw,
                    if elegida { tema::ACENTO } else { color },
                );
                if !cuenta.is_empty() {
                    p.texto(
                        &cuenta,
                        ai - cw - 12.0 * e,
                        y + 4.0 * e,
                        peq,
                        tema::TEXTO_APAGADO,
                    );
                }
                zonas.push((r, Clic::Fila(k)));
            }
        });
        if self.filas.is_empty() {
            let t = if self.cargando {
                "Leyendo el modelo…"
            } else {
                "Sin elementos"
            };
            p.texto(t, 16.0 * e, ya + 8.0 * e, tam, tema::TEXTO_SUAVE);
        }

        // La derecha.
        let xd = ai + 1.0;
        let ad = w - xd;
        p.con_recorte(rect(xd, y0, ad, abajo - y0), |p| {
            if self.tabla.is_some() {
                self.pintar_tabla(p, xd, y0, ad, abajo, &mut zonas);
            } else {
                self.pintar_props(p, xd, y0, ad, abajo, &mut zonas);
            }
        });

        // La linea de estado.
        p.rellenar(rect(0.0, abajo, w, alto_estado), tema::PANEL);
        let mut estado = self.estado.clone();
        if self.coincide.is_some() {
            let n = self
                .coincide
                .as_ref()
                .map_or(0, |c| c.iter().filter(|x| **x).count());
            estado = format!("{n} coinciden con «{}» · {estado}", self.busqueda);
        }
        let color = if self.ocupado() {
            tema::AMARILLO
        } else {
            tema::TEXTO_SUAVE
        };
        p.texto_linea(&estado, 12.0 * e, abajo + 6.0 * e, peq, w - 24.0 * e, color);
        self.zonas = zonas;
    }

    fn pintar_props(
        &self,
        p: &Pintor,
        x0: f32,
        y0: f32,
        ancho: f32,
        abajo: f32,
        zonas: &mut Vec<(RectF, Clic)>,
    ) {
        let e = self.e;
        let tam = tema::LETRA * e;
        let mut y = y0 + 14.0 * e;
        // La cabecera: el elemento o el modelo.
        let titulo = match (self.sel, self.modelo.as_ref()) {
            (Sel::Elemento(i), Some(m)) => {
                let el = &m.elementos[i];
                format!(
                    "{} · {}",
                    el.categoria,
                    if el.nombre.is_empty() {
                        &el.id
                    } else {
                        &el.nombre
                    }
                )
            }
            (_, Some(m)) => m
                .proyecto
                .nombre
                .clone()
                .unwrap_or_else(|| m.archivo.nombre.clone()),
            _ => "Leyendo…".into(),
        };
        let mut xt = x0 + 18.0 * e;
        if self.anterior.is_some() {
            let r = rect(xt, y - 4.0 * e, 70.0 * e, 26.0 * e);
            let sobre = self.cursor.is_some_and(|(cx, cy)| dentro(&r, cx, cy));
            p.rellenar_redondeado(
                r,
                6.0 * e,
                if sobre { tema::PANEL_ALTO } else { tema::PANEL },
            );
            p.texto("< Tabla", r.x + 10.0 * e, r.y + 5.0 * e, tam, tema::TEXTO);
            zonas.push((r, Clic::Boton(Boton::Volver)));
            xt += 80.0 * e;
        }
        p.texto_linea(
            &titulo,
            xt,
            y,
            17.0 * e,
            ancho - (xt - x0) - 20.0 * e,
            tema::TEXTO,
        );
        y += 34.0 * e;
        // La miniatura de Revit, arriba a la derecha de la ficha del modelo.
        let mut ancho_lista = ancho - 36.0 * e;
        if self.sel == Sel::Info
            && let Some(b) = &self.miniatura
        {
            // SAFETY: bitmap vivo de este motor.
            let tamano = unsafe { b.GetSize() };
            let lado = (220.0 * e).min(ancho * 0.35);
            let k = (lado / tamano.width.max(1.0)).min(lado / tamano.height.max(1.0));
            let (bw, bh) = (tamano.width * k, tamano.height * k);
            let r = rect(x0 + ancho - bw - 18.0 * e, y, bw, bh);
            p.rellenar(
                rect(r.x - 4.0, r.y - 4.0, bw + 8.0, bh + 8.0),
                tema::rgb(255, 255, 255),
            );
            p.bitmap(b, r, None, false);
            ancho_lista -= bw + 24.0 * e;
        }
        let alto_fila = 22.0 * e;
        let visibles = ((abajo - y) / alto_fila).ceil() as usize + 1;
        let desde = self.desde_der.floor() as usize;
        let col = (ancho_lista * 0.42).min(300.0 * e);
        for (k, f) in self.props.iter().enumerate().skip(desde).take(visibles) {
            let yy = y + (k - desde) as f32 * alto_fila;
            let xl = x0 + 18.0 * e;
            match f {
                FilaProp::Titulo(t) => {
                    p.rellenar(
                        rect(xl, yy + 1.0, ancho_lista, alto_fila - 2.0),
                        tema::PANEL_ALTO,
                    );
                    p.texto_linea(
                        t,
                        xl + 8.0 * e,
                        yy + 3.0 * e,
                        tam,
                        ancho_lista - 16.0 * e,
                        tema::ACENTO,
                    );
                }
                FilaProp::Par(n, v) => {
                    p.texto_linea(
                        n,
                        xl + 8.0 * e,
                        yy + 3.0 * e,
                        tam,
                        col - 12.0 * e,
                        tema::TEXTO_SUAVE,
                    );
                    p.texto_linea(
                        v,
                        xl + col,
                        yy + 3.0 * e,
                        tam,
                        ancho_lista - col - 8.0 * e,
                        tema::TEXTO,
                    );
                    p.linea(
                        (xl, yy + alto_fila),
                        (xl + ancho_lista, yy + alto_fila),
                        1.0,
                        tema::rgba(0x3A, 0x3A, 0x42, 0.5),
                    );
                }
            }
        }
    }

    fn pintar_tabla(
        &mut self,
        p: &Pintor,
        x0: f32,
        y0: f32,
        ancho: f32,
        abajo: f32,
        zonas: &mut Vec<(RectF, Clic)>,
    ) {
        let e = self.e;
        let tam = tema::LETRA * e;
        let Some(t) = &self.tabla else { return };
        // Los anchos, una vez por tabla: lo que pide la cabecera y las
        // primeras filas, entre 60 y 320 px.
        if self.anchos.len() != t.columnas.len() {
            self.anchos = t
                .columnas
                .iter()
                .enumerate()
                .map(|(c, n)| {
                    let mut a = p.medir_texto(n, tam).0 + 26.0 * e;
                    for f in t.filas.iter().take(40) {
                        if let Some(x) = f.get(c) {
                            a = a.max(p.medir_texto(&x.texto(), tam).0 + 16.0 * e);
                        }
                    }
                    a.clamp(60.0 * e, 320.0 * e)
                })
                .collect();
        }
        let titulo = match (self.sel, &self.indice) {
            (Sel::Nivel(g), Some(ix)) => format!("Tabla de {}", ix.grupos[g].nombre),
            (Sel::Categoria(g, c), Some(ix)) => format!(
                "Tabla de {} · {}",
                ix.grupos[g].categorias[c].0, ix.grupos[g].nombre
            ),
            _ => "Tabla".into(),
        };
        p.texto_linea(
            &format!(
                "{titulo} ({} filas, {} columnas)",
                t.filas.len(),
                t.columnas.len()
            ),
            x0 + 18.0 * e,
            y0 + 14.0 * e,
            15.0 * e,
            ancho - 36.0 * e,
            tema::TEXTO,
        );
        p.texto(
            "Clic en una columna para ordenar · Mayús + rueda para moverse de lado · clic en una fila para ver el elemento",
            x0 + 18.0 * e,
            y0 + 36.0 * e,
            tema::LETRA_PEQUENA * e,
            tema::TEXTO_APAGADO,
        );
        let yc = y0 + 58.0 * e;
        let alto_cab = 26.0 * e;
        let alto_fila = 22.0 * e;
        let xi = x0 + 8.0 * e;
        // Las columnas que se ven.
        let mut cols: Vec<(usize, f32, f32)> = Vec::new();
        let mut x = xi - self.lado;
        for (c, &a) in self.anchos.iter().enumerate() {
            if x + a >= xi && x <= x0 + ancho {
                cols.push((c, x, a));
            }
            x += a;
            if x > x0 + ancho {
                break;
            }
        }
        p.rellenar(rect(x0, yc, ancho, alto_cab), tema::PANEL_ALTO);
        for &(c, x, a) in &cols {
            let r = rect(x, yc, a, alto_cab);
            let n = &t.columnas[c];
            let mut ancho_texto = a - 10.0 * e;
            if let Some((oc, asc)) = self.orden
                && oc == c
            {
                let d = if asc { Dir::Arriba } else { Dir::Abajo };
                triangulo(
                    p,
                    x + a - 10.0 * e,
                    yc + alto_cab / 2.0,
                    4.0 * e,
                    d,
                    tema::ACENTO,
                );
                ancho_texto -= 14.0 * e;
            }
            p.texto_linea(n, x + 6.0 * e, yc + 5.0 * e, tam, ancho_texto, tema::TEXTO);
            p.linea(
                (x + a, yc),
                (x + a, abajo),
                1.0,
                tema::rgba(0x3A, 0x3A, 0x42, 0.6),
            );
            zonas.push((r, Clic::Columna(c)));
        }
        let yf = yc + alto_cab;
        let visibles = ((abajo - yf) / alto_fila).ceil() as usize + 1;
        let desde = self.desde_der.floor() as usize;
        for (k, f) in t.filas.iter().enumerate().skip(desde).take(visibles) {
            let y = yf + (k - desde) as f32 * alto_fila;
            let r = rect(x0, y, ancho, alto_fila);
            let sobre = self.cursor.is_some_and(|(cx, cy)| dentro(&r, cx, cy));
            if sobre {
                p.rellenar(r, tema::PANEL_ALTO);
            } else if k % 2 == 1 {
                p.rellenar(r, tema::rgba(0x26, 0x26, 0x2C, 0.6));
            }
            for &(c, x, a) in &cols {
                let Some(celda) = f.get(c) else { continue };
                let txt = celda.texto();
                if txt.is_empty() {
                    continue;
                }
                let color: Color = match celda {
                    Celda::Numero(_) => tema::PREVIA,
                    _ => tema::TEXTO,
                };
                // Los numeros, a la derecha.
                let tx = if matches!(celda, Celda::Numero(_)) {
                    let tw = p.medir_texto(&txt, tam).0.min(a - 10.0 * e);
                    x + a - tw - 6.0 * e
                } else {
                    x + 6.0 * e
                };
                p.texto_linea(&txt, tx, y + 3.0 * e, tam, a - 10.0 * e, color);
            }
            zonas.push((r, Clic::FilaTabla(k)));
        }
        if t.filas.is_empty() {
            p.texto(
                "Nada que mostrar",
                x0 + 18.0 * e,
                yf + 10.0 * e,
                tam,
                tema::TEXTO_SUAVE,
            );
        }
    }
}

/// La ficha del fichero y del proyecto.
fn filas_de_info(m: &ModeloBim) -> Vec<FilaProp> {
    let mut v = Vec::new();
    let par = |v: &mut Vec<FilaProp>, n: &str, x: Option<&str>| {
        if let Some(x) = x.filter(|x| !x.trim().is_empty()) {
            v.push(FilaProp::Par(n.into(), x.into()));
        }
    };
    let a = &m.archivo;
    v.push(FilaProp::Titulo("Fichero".into()));
    par(&mut v, "Nombre", Some(&a.nombre));
    par(&mut v, "Formato", Some(a.formato.nombre()));
    par(&mut v, "Version", a.version.as_deref());
    par(&mut v, "Compilacion", a.compilacion.as_deref());
    par(&mut v, "Programa", a.aplicacion.as_deref());
    par(&mut v, "Autor", a.autor.as_deref());
    par(&mut v, "Organizacion", a.organizacion.as_deref());
    par(&mut v, "Guardado por", a.guardado_por.as_deref());
    par(&mut v, "Fecha", a.fecha.as_deref());
    par(
        &mut v,
        "Trabajo compartido",
        a.trabajo_compartido.as_deref(),
    );
    par(&mut v, "Modelo central", a.ruta_central.as_deref());
    par(&mut v, "GUID", a.guid.as_deref());
    par(&mut v, "Idioma", a.idioma.as_deref());
    par(&mut v, "OmniClass", a.omniclass.as_deref());
    par(
        &mut v,
        "Tamaño",
        Some(&format!("{:.1} MB", a.bytes as f64 / 1e6)),
    );
    let p = &m.proyecto;
    v.push(FilaProp::Titulo("Proyecto".into()));
    par(&mut v, "Nombre", p.nombre.as_deref());
    par(&mut v, "Numero", p.numero.as_deref());
    par(&mut v, "Cliente", p.cliente.as_deref());
    par(&mut v, "Direccion", p.direccion.as_deref());
    par(&mut v, "Estado", p.estado.as_deref());
    par(&mut v, "Fecha de emision", p.fecha_emision.as_deref());
    par(&mut v, "Organizacion", p.organizacion.as_deref());
    par(&mut v, "Sitio", p.sitio.as_deref());
    par(&mut v, "Edificio", p.edificio.as_deref());
    par(&mut v, "Descripcion", p.descripcion.as_deref());
    v.push(FilaProp::Titulo("Resumen".into()));
    for (n, x) in [
        ("Elementos", m.elementos.len()),
        ("Niveles", m.niveles.len()),
        ("Categorias", m.categorias.len()),
        ("Materiales", m.materiales.len()),
        ("Espacios", m.espacios.len()),
        ("Tipos", m.tipos.len()),
    ] {
        v.push(FilaProp::Par(n.into(), x.to_string()));
    }
    for c in &m.categorias {
        v.push(FilaProp::Par(
            format!("  {}", c.nombre),
            c.elementos.to_string(),
        ));
    }
    if !m.avisos.is_empty() {
        v.push(FilaProp::Titulo("Avisos (lo que no se pudo leer)".into()));
        for x in &m.avisos {
            v.push(FilaProp::Par("·".into(), x.clone()));
        }
    }
    if !m.espacios.is_empty() {
        v.push(FilaProp::Titulo("Habitaciones".into()));
        for s in &m.espacios {
            let area = s
                .area
                .map(|a| format!("{} m²", cad_bim_datos::numero_corto(a)))
                .unwrap_or_default();
            v.push(FilaProp::Par(format!("{} {}", s.numero, s.nombre), area));
        }
    }
    for g in &p.grupos {
        v.push(FilaProp::Titulo(g.nombre.clone()));
        for q in &g.propiedades {
            v.push(FilaProp::Par(q.nombre.clone(), q.texto()));
        }
    }
    if !a.datos.is_empty() {
        v.push(FilaProp::Titulo("Datos del fichero".into()));
        for d in &a.datos {
            v.push(FilaProp::Par(d.clave.clone(), d.valor.clone()));
        }
    }
    if !m.materiales.is_empty() {
        v.push(FilaProp::Titulo("Materiales".into()));
        for x in &m.materiales {
            v.push(FilaProp::Par(
                x.nombre.clone(),
                format!("{} elementos", x.elementos),
            ));
        }
    }
    v
}

/// Las propiedades de un elemento, por conjunto.
fn filas_de_elemento(m: &ModeloBim, i: usize) -> Vec<FilaProp> {
    let el = &m.elementos[i];
    let mut v = vec![FilaProp::Titulo("Elemento".into())];
    let mut par = |n: &str, x: &str| {
        if !x.trim().is_empty() {
            v.push(FilaProp::Par(n.into(), x.into()));
        }
    };
    par("Id", &el.id);
    par("GlobalId", el.global_id.as_deref().unwrap_or(""));
    par("Categoria", &el.categoria);
    par("Clase IFC", &el.clase);
    par("Familia", &el.familia);
    par("Tipo", &el.tipo);
    par("Nombre", &el.nombre);
    par("Nivel", m.nivel_de(el).unwrap_or(""));
    par("Materiales", &el.materiales.join("; "));
    if let Some(c) = el.caja {
        let [a, b, h] = c.medidas();
        let n = cad_bim_datos::numero_corto;
        par(
            "Caja (ancho × fondo × alto)",
            &format!("{} × {} × {} m", n(a), n(b), n(h)),
        );
        par(
            "Caja (min)",
            &format!("{}, {}, {}", n(c.min[0]), n(c.min[1]), n(c.min[2])),
        );
    }
    for c in &el.conjuntos {
        v.push(FilaProp::Titulo(format!(
            "{}  ·  {}",
            c.nombre,
            c.tipo.nombre()
        )));
        for q in &c.propiedades {
            v.push(FilaProp::Par(q.nombre.clone(), q.texto()));
        }
    }
    v
}

#[derive(Clone, Copy)]
enum Dir {
    Derecha,
    Abajo,
    Arriba,
}

/// Un triangulito con centro en `(cx, cy)` (las flechas del arbol y del
/// orden; las letras de flechas no estan en todas las fuentes).
fn triangulo(p: &Pintor, cx: f32, cy: f32, r: f32, d: Dir, color: Color) {
    let n = (r * 2.0).ceil().max(2.0) as i32;
    for k in 0..n {
        // Lineas paralelas que se acortan: un triangulo relleno sin trazados.
        let t = k as f32 / n as f32;
        let l = r * (1.0 - t);
        match d {
            Dir::Derecha => {
                let x = cx - r * 0.6 + t * r * 1.4;
                p.linea((x, cy - l), (x, cy + l), 1.0, color);
            }
            Dir::Abajo => {
                let y = cy - r * 0.6 + t * r * 1.4;
                p.linea((cx - l, y), (cx + l, y), 1.0, color);
            }
            Dir::Arriba => {
                let y = cy + r * 0.6 - t * r * 1.4;
                p.linea((cx - l, y), (cx + l, y), 1.0, color);
            }
        }
    }
}
