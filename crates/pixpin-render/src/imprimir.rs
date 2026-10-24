//! **Imprimir con Direct2D**: las mismas primitivas que pintan la pantalla,
//! mandadas a la impresora como paginas de vectores.
//!
//! Es la via que trae Windows para esto (`ID2D1PrintControl`): cada pagina
//! se graba en una lista de ordenes de Direct2D (`ID2D1CommandList`) con el
//! mismo [`Pintor`] de siempre, y el control de impresion la convierte a XPS
//! y la entrega al trabajo de la cola (`IPrintDocumentPackageTarget`). Nada
//! se rasteriza por el camino salvo lo que de verdad es un mapa de bits, asi
//! que un lienzo sale a la resolucion de la impresora y no a la de la
//! pantalla. Lo que se ve y lo que se imprime lo pinta el mismo codigo: si
//! divergieran, lo impreso no se pareceria a lo que se ve (`DrawExport.kt`).
//!
//! El papel, la impresora, las copias y la orientacion los elige el dialogo
//! de imprimir de Windows (`pixpin_shell::imprimir`), que devuelve el
//! `DEVMODE` del controlador; aqui se convierte en el *print ticket* del
//! trabajo con el proveedor del sistema (`PTConvertDevModeToPrintTicket`),
//! que es como Direct2D sabe lo que se eligio.
//!
//! Con `fichero` puesto, la salida del trabajo va a ese fichero en vez de al
//! papel: es lo que permite probar el camino entero con «Microsoft Print to
//! PDF» sin mandar nada a una impresora de verdad.

use std::path::Path;

use windows::Win32::Foundation::HGLOBAL;
use windows::Win32::Graphics::Direct2D::Common::D2D_SIZE_F;
use windows::Win32::Graphics::Direct2D::{
    D2D1_COLOR_SPACE_SRGB, D2D1_PRINT_CONTROL_PROPERTIES, D2D1_PRINT_FONT_SUBSET_MODE_DEFAULT,
};
use windows::Win32::Graphics::Imaging::{CLSID_WICImagingFactory, IWICImagingFactory};
use windows::Win32::Graphics::Printing::PrintTicket::{
    PTCloseProvider, PTConvertDevModeToPrintTicket, PTOpenProvider, kPTJobScope,
};
use windows::Win32::Storage::Xps::Printing::{
    IPrintDocumentPackageTarget, IPrintDocumentPackageTargetFactory,
    PrintDocumentPackageTargetFactory,
};
use windows::Win32::System::Com::StructuredStorage::{CreateStreamOnHGlobal, GetHGlobalFromStream};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize, IStream, STATFLAG_NONAME, STATSTG, STREAM_SEEK_SET,
};
use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
use windows::core::HSTRING;

use crate::{ErrorRender, MotorRender, Pintor};

/// Resolucion a la que Direct2D rasteriza lo que no puede ir en vectores
/// (los mapas de bits con efectos). 150 ppp es lo que usa el ejemplo de
/// Microsoft y se ve bien en papel sin inflar el trabajo.
const PPP_RASTER: f32 = 150.0;

/// Lo que se pide para imprimir.
pub struct Trabajo<'a> {
    /// El nombre de la impresora, tal cual lo da el dialogo.
    pub impresora: &'a str,
    /// Como se llama el trabajo en la cola.
    pub nombre: &'a str,
    /// El `DEVMODEW` que devolvio el dialogo, en bytes. Sin el, lo que la
    /// impresora tenga por defecto.
    pub devmode: Option<&'a [u8]>,
    /// El tamano de cada pagina en DIP (1/96 de pulgada).
    pub paginas: &'a [(f32, f32)],
    /// Si se pone, la salida del trabajo se escribe aqui y no en el papel.
    pub fichero: Option<&'a Path>,
}

/// COM en este hilo mientras dure la impresion. Si ya estaba iniciado con
/// otro modelo (`RPC_E_CHANGED_MODE`) se usa el que habia y no se cierra.
struct Com(bool);

impl Com {
    fn iniciar() -> Com {
        // SAFETY: iniciar COM no tiene precondiciones; el resultado dice si
        // hay que emparejarlo con un CoUninitialize.
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        Com(hr.is_ok())
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: empareja el CoInitializeEx que tuvo exito.
            unsafe { CoUninitialize() };
        }
    }
}

/// El *print ticket* del trabajo a partir del `DEVMODE` del dialogo.
fn ticket(impresora: &HSTRING, devmode: &[u8]) -> Result<IStream, ErrorRender> {
    // SAFETY: flujo en memoria nuevo; se libera solo al soltarlo.
    let flujo = unsafe { CreateStreamOnHGlobal(HGLOBAL::default(), true)? };
    // SAFETY: nombre de impresora vivo durante la llamada; version 1 es la
    // unica que existe del esquema de print tickets.
    let proveedor = unsafe { PTOpenProvider(impresora, 1)? };
    // SAFETY: `devmode` son los bytes de un DEVMODEW entero (los copio el
    // dialogo de su HGLOBAL); el proveedor no retiene el puntero. La firma de
    // `windows` dice DEVMODEA, pero la funcion de prntvpt.dll es la unica que
    // hay y lee el DEVMODE ancho que devuelven los dialogos Unicode.
    let hecho = unsafe {
        PTConvertDevModeToPrintTicket(
            proveedor,
            devmode.len() as u32,
            devmode.as_ptr() as *const _,
            kPTJobScope,
            &flujo,
        )
    };
    // SAFETY: cierra el proveedor abierto arriba, haya salido bien o no.
    unsafe {
        let _ = PTCloseProvider(proveedor);
    }
    hecho?;
    // SAFETY: flujo propio; se rebobina para que lo lea quien lo reciba.
    unsafe { flujo.Seek(0, STREAM_SEEK_SET, None)? };
    Ok(flujo)
}

/// Lo que haya escrito el trabajo en su flujo de salida.
fn bytes_del_flujo(flujo: &IStream) -> Result<Vec<u8>, ErrorRender> {
    let mut stat = STATSTG::default();
    // SAFETY: estructura local de salida; sin nombre, no hay memoria COM que
    // liberar.
    unsafe { flujo.Stat(&mut stat, STATFLAG_NONAME)? };
    let largo = stat.cbSize as usize;
    // SAFETY: el flujo se creo sobre un HGLOBAL propio (CreateStreamOnHGlobal).
    let memoria = unsafe { GetHGlobalFromStream(flujo)? };
    // SAFETY: bloqueo del HGLOBAL del flujo mientras se copia; se
    // desbloquea antes de salir.
    unsafe {
        let p = GlobalLock(memoria) as *const u8;
        if p.is_null() {
            return Ok(Vec::new());
        }
        let v = std::slice::from_raw_parts(p, largo).to_vec();
        let _ = GlobalUnlock(memoria);
        Ok(v)
    }
}

/// **Imprime** `trabajo`: cada pagina se pinta con `pintar(indice, pintor)`
/// en coordenadas de DIP de la pagina, con el origen arriba a la izquierda.
///
/// Usa el dispositivo de `motor`, pero no toca lo que este pintando: el
/// destino del contexto se deja desligado al acabar, como `dibujar`.
pub fn imprimir(
    motor: &MotorRender,
    trabajo: &Trabajo<'_>,
    pintar: &mut dyn FnMut(usize, &Pintor),
) -> Result<(), ErrorRender> {
    let _com = Com::iniciar();
    let impresora = HSTRING::from(trabajo.impresora);
    let nombre = HSTRING::from(trabajo.nombre);
    // SAFETY: fabrica de destinos de impresion del sistema, en proceso.
    let fabrica: IPrintDocumentPackageTargetFactory = unsafe {
        CoCreateInstance(
            &PrintDocumentPackageTargetFactory,
            None,
            CLSCTX_INPROC_SERVER,
        )?
    };
    let ticket = match trabajo.devmode {
        Some(d) if !d.is_empty() => Some(ticket(&impresora, d)?),
        _ => None,
    };
    let salida = match trabajo.fichero {
        // SAFETY: flujo en memoria nuevo, liberado al soltarlo.
        Some(_) => Some(unsafe { CreateStreamOnHGlobal(HGLOBAL::default(), true)? }),
        None => None,
    };
    // SAFETY: cadenas vivas durante la llamada; los flujos opcionales son
    // propios y siguen vivos mientras viva el destino.
    let destino = unsafe {
        fabrica.CreateDocumentPackageTargetForPrintJob(
            &impresora,
            &nombre,
            salida.as_ref(),
            ticket.as_ref(),
        )?
    };
    imprimir_en(motor, &destino, trabajo.paginas, pintar)?;
    if let (Some(ruta), Some(flujo)) = (trabajo.fichero, salida.as_ref()) {
        esperar_y_guardar(ruta, flujo)?;
    }
    Ok(())
}

/// **Pinta las paginas en un destino de impresion ya abierto**: el que crea
/// [`imprimir`] tras el dialogo clasico, o el que entrega el dialogo moderno
/// de Windows en `MakeDocument` (`imprimir_moderno`). Lo que va al papel lo
/// pinta siempre esto, venga de donde venga el destino.
pub fn imprimir_en(
    motor: &MotorRender,
    destino: &IPrintDocumentPackageTarget,
    paginas: &[(f32, f32)],
    pintar: &mut dyn FnMut(usize, &Pintor),
) -> Result<(), ErrorRender> {
    // SAFETY: fabrica de WIC del sistema, en proceso.
    let wic: IWICImagingFactory =
        unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)? };
    let ctx = motor.contexto();
    // SAFETY: el contexto esta vivo; su dispositivo es el del motor.
    let dispositivo = unsafe { ctx.GetDevice()? };
    let propiedades = D2D1_PRINT_CONTROL_PROPERTIES {
        fontSubset: D2D1_PRINT_FONT_SUBSET_MODE_DEFAULT,
        rasterDPI: PPP_RASTER,
        colorSpace: D2D1_COLOR_SPACE_SRGB,
    };
    // SAFETY: fabrica WIC y destino vivos; las propiedades son locales.
    let control = unsafe { dispositivo.CreatePrintControl(&wic, destino, Some(&propiedades))? };
    for (i, &(ancho, alto)) in paginas.iter().enumerate() {
        // SAFETY: lista de ordenes nueva del mismo contexto.
        let lista = unsafe { ctx.CreateCommandList()? };
        motor.fotograma.set(motor.fotograma.get() + 1);
        // SAFETY: protocolo de D2D sobre un contexto vivo, como `dibujar`:
        // el destino se desliga tambien si EndDraw falla, porque va antes
        // del `?`.
        let fin = unsafe {
            ctx.SetTarget(&lista);
            ctx.BeginDraw();
            ctx.SetTransform(&windows_numerics::Matrix3x2::identity());
            pintar(i, &Pintor { motor });
            let fin = ctx.EndDraw(None, None);
            ctx.SetTarget(None);
            fin
        };
        fin?;
        // SAFETY: la lista ya no es destino de nadie; cerrarla es lo que la
        // deja lista para el control.
        unsafe { lista.Close()? };
        // SAFETY: lista cerrada del mismo dispositivo; sin ticket por pagina.
        unsafe {
            control.AddPage(
                &lista,
                D2D_SIZE_F {
                    width: ancho,
                    height: alto,
                },
                None,
                None,
                None,
            )?
        };
    }
    // SAFETY: cierra el documento: a partir de aqui el trabajo es de la cola.
    unsafe { control.Close()? };
    Ok(())
}

/// Espera a que el trabajo acabe de escribir su salida y la guarda en `ruta`.
fn esperar_y_guardar(ruta: &Path, flujo: &IStream) -> Result<(), ErrorRender> {
    {
        // El trabajo escribe su salida al cerrarse el documento, pero la
        // cola lo termina a su ritmo: se espera a que deje de crecer, con un
        // tope para no colgar a quien llama si la impresora no contesta.
        let mut visto = 0usize;
        let mut quieto = 0u32;
        let limite = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while std::time::Instant::now() < limite {
            std::thread::sleep(std::time::Duration::from_millis(100));
            let ahora = bytes_del_flujo(flujo)?.len();
            if ahora > 0 && ahora == visto {
                quieto += 1;
                if quieto >= 5 {
                    break;
                }
            } else {
                quieto = 0;
            }
            visto = ahora;
        }
        std::fs::write(ruta, bytes_del_flujo(flujo)?).map_err(|e| {
            ErrorRender::Windows(windows::core::Error::new(
                windows::Win32::Foundation::E_FAIL,
                e.to_string(),
            ))
        })?;
    }
    Ok(())
}
