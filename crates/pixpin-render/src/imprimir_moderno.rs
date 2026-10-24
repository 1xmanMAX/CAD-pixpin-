//! **El dialogo moderno de imprimir de Windows, con vista previa dentro.**
//!
//! Es el de Windows 10 y 11 (el mismo que abren Edge o Fotos): impresora,
//! papel, orientacion, copias, paginas... y a la izquierda **la hoja tal como
//! va a salir**, que se rehace sola al cambiar el papel o la orientacion. Es
//! lo que pidio el usuario («no me sale la previsualizacion dentro, asi que
//! no se que se imprimira») y es lo que hace el `PrintManager` del movil
//! (`guardados/Imprimir.kt`): el sistema pregunta y ensena, la aplicacion
//! solo pinta.
//!
//! Desde una ventana Win32 se llega por `IPrintManagerInterop` (el mismo
//! camino que `IDataTransferManagerInterop` en `pixpin_shell::compartir`).
//! La aplicacion entrega un `IPrintDocumentSource` hecho a mano con las dos
//! interfaces nativas de `DocumentSource.h`, como la muestra oficial
//! «Direct2D printing» de Microsoft, sin XAML:
//!
//! - `IPrintPreviewPageCollection`: `Paginate` dice cuantas hojas salen con
//!   el papel elegido y `MakePage` pinta una en una textura DXGI que se le
//!   da al dialogo (`IPrintPreviewDxgiPackageTarget::DrawPage`).
//! - `IPrintDocumentPageSource::MakeDocument`: al pulsar «Imprimir», las
//!   mismas hojas van al papel por [`crate::imprimir::imprimir_en`], el
//!   mismo camino que el dialogo clasico.
//!
//! La vista previa y el papel los pinta el **mismo** [`Documento::pintar`]:
//! lo unico que cambia es la escala (pixeles de la miniatura por DIP de
//! papel). Si divergieran, la vista previa mentiria.
//!
//! Los avisos del dialogo llegan en hilos suyos, no en el de la ventana: todo
//! lo que tocan (el motor, el documento) va detras de un cerrojo, de uno en
//! uno. El motor es propio y de usar y tirar, nunca el de la pantalla.
//!
//! Si algo de esto falla (un Windows sin el servicio, una version vieja),
//! [`mostrar`] devuelve el error **antes** de ensenar nada y quien llama abre
//! el dialogo clasico (`pixpin_shell::imprimir`). Si falla despues, ya con
//! el dialogo pedido, [`roto`] lo recuerda para la siguiente vez.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use windows::Foundation::{IReference, PropertyValue, TypedEventHandler};
use windows::Graphics::Printing::OptionDetails::{
    PrintTaskOptionChangedEventArgs, PrintTaskOptionDetails,
};
use windows::Graphics::Printing::{
    IPrintDocumentSource, IPrintDocumentSource_Impl, PrintManager, PrintOrientation, PrintTask,
    PrintTaskOptions, PrintTaskRequestedEventArgs, PrintTaskSourceRequestedArgs,
    PrintTaskSourceRequestedHandler, StandardPrintTaskOptions,
};
use windows::Win32::Foundation::{E_FAIL, E_POINTER, HWND};
use windows::Win32::Graphics::Direct3D11::ID3D11Device;
use windows::Win32::Graphics::Dxgi::IDXGISurface;
use windows::Win32::Graphics::Printing::{FinalPageCount, IPrintPreviewDxgiPackageTarget};
use windows::Win32::Storage::Xps::Printing::IPrintDocumentPackageTarget;
use windows::Win32::System::WinRT::Printing::{
    IPrintDocumentPageSource, IPrintDocumentPageSource_Impl, IPrintManagerInterop,
    IPrintPreviewPageCollection, IPrintPreviewPageCollection_Impl,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetWindowLongPtrW, HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SetWindowPos, WS_EX_TOPMOST,
};
use windows::core::{GUID, HSTRING, IInspectable, Interface, Ref, factory, implement};
use windows_core::IUnknownImpl;
use windows_future::{AsyncOperationCompletedHandler, AsyncStatus, IAsyncOperation};

use crate::fuera_de_pantalla::FueraDePantalla;
use crate::{Color, ErrorRender, MotorRender, Pintor};

/// Una hoja de papel tal como la va a pintar [`Documento::pintar`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pagina {
    /// El papel elegido en el dialogo, en DIP (1/96 de pulgada), ya con su
    /// orientacion.
    pub papel: (f32, f32),
    /// Unidades del destino por DIP de papel: 1 al imprimir; en la vista
    /// previa, lo que mide la miniatura entre lo que mide el papel.
    pub escala: f32,
}

/// Lo que se imprime. Lo implementa la aplicacion (el lienzo); aqui solo se
/// pregunta y se pinta.
pub trait Documento {
    /// Prepara las hojas para `alcance` (el elemento elegido en la opcion
    /// propia, o `None` si no hay) y dice cuantas son. Aqui se suben a la
    /// GPU las imagenes que hagan falta: fuera de `BeginDraw`.
    fn paginar(&mut self, motor: &MotorRender, alcance: Option<&str>) -> usize;
    /// Pinta la hoja `i` (desde cero) en `pagina`. El pintor llega sin
    /// vista puesta; el papel ya esta en blanco.
    fn pintar(&self, i: usize, pagina: Pagina, p: &Pintor);
}

/// Una lista de opciones propia en el dialogo (la de «Que imprimir»).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpcionPropia {
    pub id: String,
    pub titulo: String,
    /// `(id, rotulo)` de cada elemento, en orden.
    pub elementos: Vec<(String, String)>,
    /// El id del elemento elegido al abrir.
    pub inicial: String,
}

/// Lo que se pide al abrir el dialogo.
pub struct Pedido {
    /// Como se llama el trabajo en la cola.
    pub titulo: String,
    /// Si el dialogo sale ya con el papel tumbado (la primera hoja es
    /// apaisada, como hace el movil).
    pub apaisada: bool,
    pub opcion: Option<OpcionPropia>,
    /// Los puntos por pulgada de la miniatura de la vista previa.
    pub ppp_previa: f32,
}

/// Un A4 en DIP: el papel si el dialogo no dice otro (el del movil).
pub const A4: (f32, f32) = (793.7, 1122.5);

/// `JOB_PAGE_APPLICATION_DEFINED` de `DocumentSource.h`: «la que quieras».
const PAGINA_DE_LA_APLICACION: u32 = u32::MAX;

/// Si el dialogo moderno ya fallo una vez en esta sesion: entonces se va
/// derecho al clasico, que no falla por las mismas razones.
static ROTO: AtomicBool = AtomicBool::new(false);

/// Si el dialogo moderno fallo en esta sesion (despues de pedirlo).
pub fn roto() -> bool {
    ROTO.load(Ordering::Relaxed)
}

/// El papel por omision, con la orientacion pedida.
pub fn papel_por_omision(apaisada: bool) -> (f32, f32) {
    if apaisada { (A4.1, A4.0) } else { A4 }
}

/// La hoja (desde cero) que pide el dialogo con su numero de trabajo (desde
/// uno), o `None` si no existe. «La que quieras» es la primera.
pub fn hoja_del_trabajo(n: u32, total: usize) -> Option<usize> {
    if n == PAGINA_DE_LA_APLICACION {
        return (total > 0).then_some(0);
    }
    let i = (n as usize).checked_sub(1)?;
    (i < total).then_some(i)
}

/// Las hojas (desde cero) que entran en los intervalos de paginas del
/// dialogo (desde uno, cerrados). Sin intervalos, todas. Los intervalos que
/// se salen se recortan; los que no tocan ninguna hoja no aportan nada.
pub fn hojas_elegidas(intervalos: &[(i32, i32)], total: usize) -> Vec<usize> {
    if intervalos.is_empty() {
        return (0..total).collect();
    }
    (0..total)
        .filter(|i| {
            let n = *i as i32 + 1;
            intervalos
                .iter()
                .any(|&(a, b)| n >= a.min(b) && n <= a.max(b))
        })
        .collect()
}

/// Los pixeles de la miniatura: `ancho` x `alto` en DIP de la vista previa a
/// `ppp`. Nunca cero, y con tope para no pedir una textura absurda si el
/// dialogo manda algo raro.
pub fn tamano_miniatura(ancho: f32, alto: f32, ppp: f32) -> (u32, u32) {
    let k = (ppp / 96.0).clamp(0.5, 4.0);
    let px = |d: f32| ((d.max(1.0) * k).ceil() as u32).clamp(1, 4096);
    (px(ancho), px(alto))
}

/// Mientras el dialogo este abierto: el documento, el motor propio y el
/// destino de la vista previa.
struct Estado {
    documento: Box<dyn Documento>,
    motor: MotorRender,
    d3d: ID3D11Device,
    previa: Option<IPrintPreviewDxgiPackageTarget>,
    papel: (f32, f32),
    total: usize,
    ppp: f32,
    id_opcion: Option<String>,
    papel_omision: (f32, f32),
}

// SAFETY: el estado solo se toca con el cerrojo de `Compartido` cogido, de
// uno en uno, venga el aviso del dialogo del hilo que venga. El motor es
// propio (nadie mas lo usa) y su fabrica de un hilo solo necesita justo eso:
// que no la usen dos a la vez. Los objetos de impresion de Windows son
// agiles.
unsafe impl Send for Estado {}

type Compartido = Arc<Mutex<Estado>>;

/// Algo de COM que cruza a los avisos del dialogo. Solo se guardan aqui los
/// objetos de impresion de WinRT (agiles) y la fuente propia (que se
/// protege con su cerrojo).
struct Agil<T>(T);
// SAFETY: ver arriba: objetos agiles o protegidos por su propio cerrojo.
unsafe impl<T> Send for Agil<T> {}
// SAFETY: idem.
unsafe impl<T> Sync for Agil<T> {}

impl<T> Agil<T> {
    /// Lo de dentro. Metodo y no `.0`: un cierre que usara `.0` capturaria
    /// solo el campo, sin el envoltorio que lo deja cruzar de hilo.
    fn dentro(&self) -> &T {
        &self.0
    }
}

/// **La fuente del documento** que se le da al dialogo.
#[implement(
    IPrintDocumentSource,
    IPrintDocumentPageSource,
    IPrintPreviewPageCollection
)]
struct Fuente {
    estado: Compartido,
}

impl IPrintDocumentSource_Impl for Fuente_Impl {}

/// El papel y lo elegido en la opcion propia segun las opciones del
/// dialogo. Si no se dejan leer (una prueba, o una version que no las da),
/// el papel por omision y la opcion inicial.
fn leer_opciones(opciones: Option<&IInspectable>, e: &Estado) -> ((f32, f32), Option<String>) {
    let opciones = opciones.and_then(|o| o.cast::<PrintTaskOptions>().ok());
    let papel = opciones
        .as_ref()
        .and_then(|o| o.GetPageDescription(1).ok())
        .map(|d| (d.PageSize.Width, d.PageSize.Height))
        .filter(|(w, h)| *w > 0.0 && *h > 0.0)
        .unwrap_or(e.papel_omision);
    let alcance = match (&opciones, &e.id_opcion) {
        (Some(o), Some(id)) => valor_de_opcion(o, id),
        _ => None,
    };
    (papel, alcance)
}

/// El elemento elegido en una opcion de lista propia.
fn valor_de_opcion(opciones: &PrintTaskOptions, id: &str) -> Option<String> {
    let detalles = PrintTaskOptionDetails::GetFromPrintTaskOptions(opciones).ok()?;
    let valor = detalles
        .Options()
        .ok()?
        .Lookup(&HSTRING::from(id))
        .ok()?
        .Value()
        .ok()?;
    let texto = valor.cast::<IReference<HSTRING>>().ok()?.Value().ok()?;
    Some(texto.to_string_lossy())
}

/// Los intervalos de paginas elegidos en el dialogo, desde uno. Vacio si
/// no hay (o no se dejan leer): todas.
fn intervalos(opciones: Option<&IInspectable>) -> Vec<(i32, i32)> {
    let Some(o) = opciones.and_then(|o| o.cast::<PrintTaskOptions>().ok()) else {
        return Vec::new();
    };
    let Ok(v) = o.CustomPageRanges() else {
        return Vec::new();
    };
    let n = v.Size().unwrap_or(0);
    (0..n)
        .filter_map(|i| v.GetAt(i).ok())
        .filter_map(|r| Some((r.FirstPageNumber().ok()?, r.LastPageNumber().ok()?)))
        .collect()
}

impl IPrintDocumentPageSource_Impl for Fuente_Impl {
    fn GetPreviewPageCollection(
        &self,
        destino: Ref<IPrintDocumentPackageTarget>,
    ) -> windows::core::Result<IPrintPreviewPageCollection> {
        let destino = destino.ok()?;
        // ID_PREVIEWPACKAGETARGET_DXGI es el IID de la interfaz
        // (`PrintPreview.h`: `__uuidof(IPrintPreviewDxgiPackageTarget)`).
        let id: GUID = IPrintPreviewDxgiPackageTarget::IID;
        // SAFETY: destino vivo que da el dialogo; el GUID es local.
        let previa: IPrintPreviewDxgiPackageTarget = unsafe { destino.GetPackageTarget(&id)? };
        if let Ok(mut e) = self.estado.lock() {
            e.previa = Some(previa);
        }
        Ok(self.to_interface())
    }

    fn MakeDocument(
        &self,
        opciones: Ref<IInspectable>,
        destino: Ref<IPrintDocumentPackageTarget>,
    ) -> windows::core::Result<()> {
        let destino = destino.ok()?;
        let mut e = self
            .estado
            .lock()
            .map_err(|_| windows::core::Error::from(E_FAIL))?;
        let (papel, alcance) = leer_opciones(opciones.as_ref(), &e);
        let e = &mut *e;
        let total = e.documento.paginar(&e.motor, alcance.as_deref());
        let elegidas = hojas_elegidas(&intervalos(opciones.as_ref()), total);
        let tamanos = vec![papel; elegidas.len()];
        let documento = &e.documento;
        crate::imprimir::imprimir_en(&e.motor, destino, &tamanos, &mut |j, p| {
            documento.pintar(elegidas[j], Pagina { papel, escala: 1.0 }, p);
        })
        .map_err(a_error)
    }
}

impl IPrintPreviewPageCollection_Impl for Fuente_Impl {
    fn Paginate(&self, _actual: u32, opciones: Ref<IInspectable>) -> windows::core::Result<()> {
        let mut e = self
            .estado
            .lock()
            .map_err(|_| windows::core::Error::from(E_FAIL))?;
        let (papel, alcance) = leer_opciones(opciones.as_ref(), &e);
        let e = &mut *e;
        e.papel = papel;
        e.total = e.documento.paginar(&e.motor, alcance.as_deref());
        if let Some(p) = &e.previa {
            // Una hoja en blanco antes que ninguna: con cero el dialogo no
            // ensena nada y parece roto.
            // SAFETY: destino de la vista previa vivo, del dialogo.
            unsafe {
                p.SetJobPageCount(FinalPageCount, e.total.max(1) as u32)?;
                // Las miniaturas de antes eran de otro papel u otras hojas.
                p.InvalidatePreview()?;
            }
        }
        Ok(())
    }

    fn MakePage(&self, pedida: u32, ancho: f32, alto: f32) -> windows::core::Result<()> {
        let e = self
            .estado
            .lock()
            .map_err(|_| windows::core::Error::from(E_FAIL))?;
        let Some(previa) = e.previa.clone() else {
            return Ok(());
        };
        let numero = if pedida == PAGINA_DE_LA_APLICACION {
            1
        } else {
            pedida
        };
        let superficie =
            miniatura(&e, hoja_del_trabajo(pedida, e.total), ancho, alto).map_err(a_error)?;
        // SAFETY: destino vivo; la superficie es una textura propia recien
        // pintada, que el dialogo retiene mientras la necesite.
        unsafe { previa.DrawPage(numero, &superficie, e.ppp, e.ppp) }
    }
}

/// **Pinta la miniatura de una hoja**: papel blanco y, si la hoja existe,
/// el mismo `Documento::pintar` del papel a la escala de la miniatura.
fn miniatura(
    e: &Estado,
    hoja: Option<usize>,
    ancho: f32,
    alto: f32,
) -> Result<IDXGISurface, ErrorRender> {
    let destino = pintar_miniatura(e, hoja, ancho, alto)?;
    // SAFETY: el bitmap es un destino hecho sobre una textura DXGI.
    Ok(unsafe { destino.destino.GetSurface()? })
}

fn pintar_miniatura(
    e: &Estado,
    hoja: Option<usize>,
    ancho: f32,
    alto: f32,
) -> Result<FueraDePantalla, ErrorRender> {
    let (w, h) = tamano_miniatura(ancho, alto, e.ppp);
    let destino = FueraDePantalla::nuevo(&e.motor, &e.d3d, w, h)?;
    let escala = w as f32 / e.papel.0.max(1.0);
    e.motor.dibujar(&destino.destino, |p| {
        p.limpiar(Color::BLANCO);
        if let Some(i) = hoja {
            e.documento.pintar(
                i,
                Pagina {
                    papel: e.papel,
                    escala,
                },
                p,
            );
        }
    })?;
    Ok(destino)
}

fn a_error(e: ErrorRender) -> windows::core::Error {
    match e {
        ErrorRender::Windows(w) => w,
        otro => windows::core::Error::new(E_FAIL, otro.to_string()),
    }
}

fn estado_nuevo(
    documento: Box<dyn Documento>,
    motor: MotorRender,
    d3d: ID3D11Device,
    pedido: &Pedido,
) -> Estado {
    let papel = papel_por_omision(pedido.apaisada);
    Estado {
        documento,
        motor,
        d3d,
        previa: None,
        papel,
        total: 0,
        ppp: pedido.ppp_previa,
        id_opcion: pedido.opcion.as_ref().map(|o| o.id.clone()),
        papel_omision: papel,
    }
}

/// Las opciones del trabajo: la orientacion de salida, el papel y las
/// paginas a la vista, y la opcion propia con su aviso de cambio.
fn preparar_tarea(
    tarea: &PrintTask,
    apaisada: bool,
    opcion: Option<&OpcionPropia>,
    estado: &Compartido,
) -> windows::core::Result<()> {
    let opciones = tarea.Options()?;
    opciones.SetOrientation(if apaisada {
        PrintOrientation::Landscape
    } else {
        PrintOrientation::Portrait
    })?;
    let mostradas = opciones.DisplayedOptions()?;
    let poner = |id: HSTRING| -> windows::core::Result<()> {
        let mut i = 0;
        if !mostradas.IndexOf(&id, &mut i)? {
            mostradas.Append(&id)?;
        }
        Ok(())
    };
    poner(StandardPrintTaskOptions::MediaSize()?)?;
    poner(StandardPrintTaskOptions::Orientation()?)?;
    // Las paginas sueltas, como el dialogo del movil. Si esta version de
    // Windows no las ofrece, se imprimen todas: no es motivo para no abrir.
    if let Ok(rango) = opciones.PageRangeOptions() {
        let _ = rango.SetAllowAllPages(true);
        let _ = rango.SetAllowCurrentPage(false);
        if rango.SetAllowCustomSetOfPages(true).is_ok() {
            let _ = poner(StandardPrintTaskOptions::CustomPageRanges()?);
        }
    }
    let Some(o) = opcion else {
        return Ok(());
    };
    let detalles = PrintTaskOptionDetails::GetFromPrintTaskOptions(&opciones)?;
    let lista = detalles.CreateItemListOption(&HSTRING::from(&o.id), &HSTRING::from(&o.titulo))?;
    for (id, rotulo) in &o.elementos {
        lista.AddItem(&HSTRING::from(id), &HSTRING::from(rotulo))?;
    }
    lista.TrySetValue(&PropertyValue::CreateString(&HSTRING::from(&o.inicial))?)?;
    poner(HSTRING::from(&o.id))?;
    // Cambiar «que imprimir» cambia las hojas: el dialogo no lo sabe solo.
    let id = o.id.clone();
    let estado = estado.clone();
    detalles.OptionChanged(&TypedEventHandler::<
        PrintTaskOptionDetails,
        PrintTaskOptionChangedEventArgs,
    >::new(move |_, args| {
        let Some(args) = args.as_ref() else {
            return Ok(());
        };
        let cambiada = args.OptionId()?;
        let es_la_nuestra = cambiada
            .cast::<IReference<HSTRING>>()
            .and_then(|r| r.Value())
            .is_ok_and(|s| s.to_string_lossy() == id);
        if es_la_nuestra {
            let previa = estado.lock().ok().and_then(|e| e.previa.clone());
            if let Some(p) = previa {
                // SAFETY: destino de la vista previa vivo, del dialogo; pide
                // volver a paginar (con la opcion nueva) y a pintar.
                unsafe { p.InvalidatePreview()? };
            }
        }
        Ok(())
    }))?;
    Ok(())
}

thread_local! {
    /// El aviso de «piden imprimir» puesto en el `PrintManager` de la
    /// ventana. Windows admite uno por ventana: se quita el anterior antes
    /// de poner otro, por si el de la vez pasada no se llego a quitar.
    static ANTERIOR: std::cell::RefCell<Option<(PrintManager, i64)>> =
        const { std::cell::RefCell::new(None) };
}

fn quitar_anterior() {
    ANTERIOR.with(|a| {
        if let Some((m, t)) = a.borrow_mut().take() {
            let _ = m.RemovePrintTaskRequested(t);
        }
    });
}

/// **Bajar las ventanas de PixPin que van siempre encima** mientras esta el
/// dialogo de imprimir (10-oct-2026, el usuario: «la ventana de impresion
/// desaparece cuando le doy a imprimir porque la ventana del pin esta sobre
/// la pantalla»). El dialogo es de otro proceso: un pin, su barra o
/// cualquier otro pin siempre encima lo tapaban. Devuelve las que se
/// bajaron, para volver a subirlas con [`subir_las_de_encima`] al cerrarlo.
pub fn bajar_las_de_encima() -> Vec<isize> {
    use windows::Win32::Foundation::LPARAM;
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsWindowVisible,
    };
    use windows::core::BOOL;
    struct Busca {
        pid: u32,
        v: Vec<isize>,
    }
    extern "system" fn una(h: HWND, l: LPARAM) -> BOOL {
        // SAFETY: `l` apunta a la `Busca` de abajo, viva durante
        // EnumWindows; lo demas son consultas de solo lectura.
        unsafe {
            let b = &mut *(l.0 as *mut Busca);
            let mut pid = 0;
            GetWindowThreadProcessId(h, Some(&mut pid));
            let estilo = GetWindowLongPtrW(h, GWL_EXSTYLE) as u32;
            if pid == b.pid && IsWindowVisible(h).as_bool() && estilo & WS_EX_TOPMOST.0 != 0 {
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
    for &h in &b.v {
        // SAFETY: solo el orden Z de una ventana de este proceso.
        unsafe {
            let _ = SetWindowPos(
                HWND(h as *mut _),
                Some(HWND_NOTOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }
    b.v
}

/// Las vuelve a poner encima (ver [`bajar_las_de_encima`]).
pub fn subir_las_de_encima(ventanas: &[isize]) {
    for &h in ventanas {
        // SAFETY: solo el orden Z; una ventana que ya no existe falla sin mas.
        unsafe {
            let _ = SetWindowPos(
                HWND(h as *mut _),
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }
}

/// **Abre el dialogo moderno** sobre `ventana` con `documento`. Vuelve en
/// seguida: el dialogo sigue abierto por su cuenta, y lo que haga falta lo
/// pide a la fuente. Hay que llamarla desde el hilo de la ventana, que
/// tiene que seguir atendiendo sus mensajes.
///
/// `Err` si no se pudo ni pedir: entonces no se ha ensenado nada y lo
/// sensato es el dialogo clasico.
pub fn mostrar(
    ventana: HWND,
    pedido: Pedido,
    d3d: ID3D11Device,
    motor: MotorRender,
    documento: Box<dyn Documento>,
) -> Result<(), ErrorRender> {
    let interop: IPrintManagerInterop = factory::<PrintManager, IPrintManagerInterop>()?;
    quitar_anterior();
    // SAFETY: ventana de este proceso, viva mientras dure la llamada.
    let manager: PrintManager = unsafe { interop.GetForWindow(ventana)? };
    let estado: Compartido = Arc::new(Mutex::new(estado_nuevo(documento, motor, d3d, &pedido)));
    let fuente: IPrintDocumentSource = Fuente {
        estado: estado.clone(),
    }
    .into();
    let fuente = Agil(fuente);
    let titulo = HSTRING::from(&pedido.titulo);
    let apaisada = pedido.apaisada;
    let opcion = pedido.opcion.clone();
    let pedida =
        TypedEventHandler::<PrintManager, PrintTaskRequestedEventArgs>::new(move |_, args| {
            let args = args
                .as_ref()
                .ok_or_else(|| windows::core::Error::from(E_POINTER))?;
            let fuente = Agil(fuente.dentro().clone());
            let tarea = args.Request()?.CreatePrintTask(
                &titulo,
                &PrintTaskSourceRequestedHandler::new(
                    move |a: Ref<PrintTaskSourceRequestedArgs>| {
                        let a = a
                            .as_ref()
                            .ok_or_else(|| windows::core::Error::from(E_POINTER))?;
                        a.SetSource(fuente.dentro())
                    },
                ),
            )?;
            // Si alguna opcion no se deja poner (una version de Windows que no
            // la tiene), el trabajo sigue con las de siempre: mejor un dialogo
            // con menos opciones que ninguno.
            let _ = preparar_tarea(&tarea, apaisada, opcion.as_ref(), &estado);
            Ok(())
        });
    let token = manager.PrintTaskRequested(&pedida)?;
    // El editor va siempre encima, y el dialogo es de otro proceso: sin
    // bajarlo, el dialogo podria abrirse detras y parecer que no pasa nada.
    // Se le devuelve su sitio al cerrar.
    // Y no solo ella: los pines y sus barras tambien van siempre encima.
    let bajadas = bajar_las_de_encima();
    // SAFETY: ventana de este proceso; el aviso de arriba esta puesto.
    let op: IAsyncOperation<bool> = match unsafe { interop.ShowPrintUIForWindowAsync(ventana) } {
        Ok(op) => op,
        Err(e) => {
            let _ = manager.RemovePrintTaskRequested(token);
            subir_las_de_encima(&bajadas);
            return Err(e.into());
        }
    };
    ANTERIOR.with(|a| *a.borrow_mut() = Some((manager.clone(), token)));
    let manager = Agil(manager);
    op.SetCompleted(&AsyncOperationCompletedHandler::new(
        move |_: Ref<IAsyncOperation<bool>>, estado: AsyncStatus| {
            let _ = manager.dentro().RemovePrintTaskRequested(token);
            // Cerrado el dialogo, cada una vuelve a ir encima.
            subir_las_de_encima(&bajadas);
            if estado == AsyncStatus::Error {
                ROTO.store(true, Ordering::Relaxed);
            }
            Ok(())
        },
    ))?;
    Ok(())
}

/// **Pinta la miniatura de la vista previa** de la hoja `i` tal como la
/// pediria el dialogo para una vista de `ancho` x `alto` DIP con `papel`,
/// y devuelve sus pixeles RGBA. Es el mismo camino de `MakePage` sin el
/// dialogo: para las muestras y las pruebas.
// Los mismos datos que monta el dialogo, uno por campo de `Estado`; la tupla
// de vuelta es la de `MakePage` (pagina, (ancho, alto, pixeles)).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn miniatura_rgba(
    documento: Box<dyn Documento>,
    motor: MotorRender,
    d3d: ID3D11Device,
    papel: (f32, f32),
    alcance: Option<&str>,
    i: usize,
    ancho: f32,
    ppp: f32,
) -> Result<(usize, (u32, u32, Vec<u8>)), ErrorRender> {
    let mut e = Estado {
        documento,
        motor,
        d3d,
        previa: None,
        papel,
        total: 0,
        ppp,
        id_opcion: None,
        papel_omision: papel,
    };
    let e2 = &mut e;
    e2.total = e2.documento.paginar(&e2.motor, alcance);
    let alto = ancho * papel.1 / papel.0;
    let hoja = hoja_del_trabajo(i as u32 + 1, e.total);
    let destino = pintar_miniatura(&e, hoja, ancho, alto)?;
    Ok((e.total, destino.leer_rgba()?))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_hoja_que_pide_el_dialogo_se_cuenta_desde_uno() {
        assert_eq!(hoja_del_trabajo(1, 3), Some(0));
        assert_eq!(hoja_del_trabajo(3, 3), Some(2));
        // «La que quieras» es la primera, si hay alguna.
        assert_eq!(hoja_del_trabajo(PAGINA_DE_LA_APLICACION, 3), Some(0));
        // Casos negativos: la cero, una que no existe, o ninguna hoja.
        assert_eq!(hoja_del_trabajo(0, 3), None);
        assert_eq!(hoja_del_trabajo(4, 3), None);
        assert_eq!(hoja_del_trabajo(PAGINA_DE_LA_APLICACION, 0), None);
    }

    #[test]
    fn sin_intervalos_se_imprimen_todas_y_con_ellos_solo_las_suyas() {
        assert_eq!(hojas_elegidas(&[], 3), [0, 1, 2]);
        assert_eq!(hojas_elegidas(&[(2, 3)], 5), [1, 2]);
        assert_eq!(hojas_elegidas(&[(1, 1), (4, 4)], 5), [0, 3]);
        // Un intervalo escrito al reves vale igual.
        assert_eq!(hojas_elegidas(&[(3, 2)], 5), [1, 2]);
        // Casos negativos: lo que se sale no inventa hojas.
        assert_eq!(hojas_elegidas(&[(7, 9)], 5), Vec::<usize>::new());
        assert_eq!(hojas_elegidas(&[(4, 99)], 5), [3, 4]);
    }

    #[test]
    fn la_miniatura_sigue_los_ppp_y_no_se_desmanda() {
        assert_eq!(tamano_miniatura(400.0, 566.0, 96.0), (400, 566));
        assert_eq!(tamano_miniatura(400.0, 566.0, 144.0), (600, 849));
        // Casos negativos: tamanos raros no dan cero ni texturas gigantes.
        assert_eq!(tamano_miniatura(0.0, -5.0, 96.0), (1, 1));
        assert_eq!(tamano_miniatura(100000.0, 10.0, 96.0).0, 4096);
    }

    fn d3d() -> ID3D11Device {
        use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
        use windows::Win32::Graphics::Direct3D11::{
            D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice,
        };
        let mut d = None;
        // SAFETY: salida local; sin adaptador concreto ni capas de depuracion.
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                Default::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut d),
                None,
                None,
            )
            .expect("la prueba necesita GPU");
        }
        d.expect("dispositivo")
    }

    /// Un documento de prueba: tantas hojas como diga el alcance («dos» o
    /// una), y cada hoja un cuadro negro en el medio del papel.
    struct Cuadros(std::sync::Arc<Mutex<Vec<String>>>);

    impl Documento for Cuadros {
        fn paginar(&mut self, _m: &MotorRender, alcance: Option<&str>) -> usize {
            self.0.lock().unwrap().push(format!("paginar {alcance:?}"));
            if alcance == Some("dos") { 2 } else { 1 }
        }
        fn pintar(&self, i: usize, pagina: Pagina, p: &Pintor) {
            self.0.lock().unwrap().push(format!(
                "pintar {i} {:.0}x{:.0} {:.3}",
                pagina.papel.0, pagina.papel.1, pagina.escala
            ));
            let k = pagina.escala;
            p.rellenar(
                crate::RectF {
                    x: pagina.papel.0 * k / 4.0,
                    y: pagina.papel.1 * k / 4.0,
                    ancho: pagina.papel.0 * k / 2.0,
                    alto: pagina.papel.1 * k / 2.0,
                },
                Color::NEGRO,
            );
        }
    }

    /// El dialogo de mentira: da su destino de vista previa y apunta que
    /// paginas le pintan, con que tamano y a cuantos ppp.
    #[implement(IPrintDocumentPackageTarget, IPrintPreviewDxgiPackageTarget)]
    struct DialogoFalso {
        visto: std::sync::Arc<Mutex<Vec<String>>>,
    }

    impl windows::Win32::Storage::Xps::Printing::IPrintDocumentPackageTarget_Impl
        for DialogoFalso_Impl
    {
        fn GetPackageTargetTypes(
            &self,
            _n: *mut u32,
            _t: *mut *mut GUID,
        ) -> windows::core::Result<()> {
            Err(E_FAIL.into())
        }
        fn GetPackageTarget(
            &self,
            tipo: *const GUID,
            iid: *const GUID,
            salida: *mut *mut core::ffi::c_void,
        ) -> windows::core::Result<()> {
            // SAFETY: punteros que da quien llama, validos durante la llamada.
            unsafe {
                if *tipo != IPrintPreviewDxgiPackageTarget::IID {
                    return Err(E_FAIL.into());
                }
                let yo: IPrintPreviewDxgiPackageTarget = self.to_interface();
                yo.query(iid, salida).ok()
            }
        }
        fn Cancel(&self) -> windows::core::Result<()> {
            Ok(())
        }
    }

    impl windows::Win32::Graphics::Printing::IPrintPreviewDxgiPackageTarget_Impl for DialogoFalso_Impl {
        fn SetJobPageCount(
            &self,
            _t: windows::Win32::Graphics::Printing::PageCountType,
            n: u32,
        ) -> windows::core::Result<()> {
            self.visto.lock().unwrap().push(format!("hojas {n}"));
            Ok(())
        }
        fn DrawPage(
            &self,
            pagina: u32,
            imagen: Ref<IDXGISurface>,
            ppp_x: f32,
            _ppp_y: f32,
        ) -> windows::core::Result<()> {
            // SAFETY: superficie viva que nos pasan.
            let d = unsafe { imagen.ok()?.GetDesc()? };
            self.visto.lock().unwrap().push(format!(
                "miniatura {pagina} {}x{} {ppp_x}",
                d.Width, d.Height
            ));
            Ok(())
        }
        fn InvalidatePreview(&self) -> windows::core::Result<()> {
            self.visto.lock().unwrap().push("invalidar".into());
            Ok(())
        }
    }

    /// **La vista previa llega al dialogo de verdad por COM**: la fuente da
    /// su coleccion, cuenta las hojas y pinta la miniatura en una textura
    /// del tamano pedido, con el mismo `pintar` y la escala de la
    /// miniatura. Es todo el camino de `MakePage` salvo la ventana.
    #[test]
    fn la_fuente_cuenta_las_hojas_y_da_al_dialogo_una_miniatura_por_hoja() {
        let d = d3d();
        let motor = MotorRender::nuevo(&d).expect("motor");
        let visto = std::sync::Arc::new(Mutex::new(Vec::new()));
        let pedido = Pedido {
            titulo: "prueba".into(),
            apaisada: false,
            opcion: None,
            ppp_previa: 144.0,
        };
        let estado: Compartido = Arc::new(Mutex::new(estado_nuevo(
            Box::new(Cuadros(visto.clone())),
            motor,
            d,
            &pedido,
        )));
        let fuente: IPrintDocumentPageSource = Fuente {
            estado: estado.clone(),
        }
        .into();
        let dialogo: IPrintDocumentPackageTarget = DialogoFalso {
            visto: visto.clone(),
        }
        .into();
        // SAFETY: objetos vivos de esta prueba.
        let coleccion = unsafe { fuente.GetPreviewPageCollection(&dialogo) }.expect("coleccion");
        // Sin opciones (no hay dialogo de verdad): papel A4 y sin alcance.
        // SAFETY: idem.
        unsafe { coleccion.Paginate(1, None) }.expect("pagina");
        // SAFETY: idem; 400 DIP de ancho de miniatura, con la forma del A4.
        unsafe { coleccion.MakePage(1, 400.0, 400.0 * A4.1 / A4.0) }.expect("miniatura");
        // Caso negativo: una hoja que no existe se pinta en blanco, sin
        // llamar a `pintar` y sin error (el dialogo no debe romperse).
        // SAFETY: idem.
        unsafe { coleccion.MakePage(5, 400.0, 566.0) }.expect("en blanco");
        let v = visto.lock().unwrap().clone();
        assert_eq!(v[0], "paginar None");
        assert_eq!(v[1], "hojas 1");
        assert_eq!(v[2], "invalidar");
        // La miniatura a 144 ppp: 400 DIP son 600 pixeles.
        assert_eq!(v[3], "pintar 0 794x1122 0.756");
        assert_eq!(v[4], "miniatura 1 600x849 144");
        assert_eq!(v[5], "miniatura 5 600x849 144");
        assert_eq!(v.len(), 6, "{v:?}");
    }

    #[test]
    fn el_papel_por_omision_es_un_a4_con_la_orientacion_pedida() {
        assert_eq!(papel_por_omision(false), A4);
        assert_eq!(papel_por_omision(true), (A4.1, A4.0));
        assert!(A4.0 < A4.1, "vertical es mas alto que ancho");
    }
}
