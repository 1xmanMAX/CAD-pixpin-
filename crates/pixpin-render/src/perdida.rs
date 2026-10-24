//! **El dispositivo grafico perdido**, en un solo sitio.
//!
//! La GPU se puede ir debajo de la aplicacion: el driver se actualiza, un
//! TDR la reinicia, se desconecta una pantalla, el equipo se suspende o la
//! grafica dedicada se apaga para ahorrar (4-oct-2026: `0x887A0005 The GPU
//! device instance has been suspended` y, desde ahi, ni un pin ni una
//! captura hasta reiniciar PixPin). Despues de eso TODO lo creado sobre el
//! dispositivo viejo —cadenas de intercambio, bitmaps, el contexto de
//! Direct2D— falla para siempre: no se arregla reintentando, hay que
//! rehacerlo sobre un dispositivo nuevo.
//!
//! Aqui vive lo comun:
//! - [`es_perdida`]: que codigos significan «el dispositivo ya no vale».
//! - [`senalar`]: todo error de Windows que pasa a `ErrorRender` pasa por
//!   aqui (`From` de `motor.rs`), asi que pintar o presentar sobre un
//!   dispositivo muerto lo avisa sin que cada ventana tenga que mirarlo.
//! - El aviso al hilo principal ([`al_perder`]) y su recogida
//!   ([`tomar_aviso`]): quien tiene los recursos compartidos los rehace.
//! - [`motivo`]: si un dispositivo concreto esta perdido y por que.
//! - Un hilo de ventanas propias puede pedir que sus ventanas se cierren
//!   solas al perderse ([`cerrar_ventanas_al_perder`]): su lanzador las
//!   vuelve a abrir con un dispositivo nuevo.
//! - Para las pruebas, un fallo inyectado por dispositivo
//!   ([`inyectar_perdida`]): forzar un TDR de verdad no es algo que una
//!   prueba pueda hacer.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};

use windows::Win32::Graphics::Direct3D11::ID3D11Device;
use windows::core::{HRESULT, Interface};

/// `DXGI_ERROR_DEVICE_REMOVED`: la GPU se fue (driver, TDR, apagada).
pub const DEVICE_REMOVED: HRESULT = HRESULT(0x887A_0005_u32 as i32);
/// `DXGI_ERROR_DEVICE_HUNG`: la GPU se colgo con nuestros comandos.
pub const DEVICE_HUNG: HRESULT = HRESULT(0x887A_0006_u32 as i32);
/// `DXGI_ERROR_DEVICE_RESET`: la GPU se reinicio.
pub const DEVICE_RESET: HRESULT = HRESULT(0x887A_0007_u32 as i32);
/// `DXGI_ERROR_DRIVER_INTERNAL_ERROR`: el driver fallo por dentro.
pub const DRIVER_INTERNAL_ERROR: HRESULT = HRESULT(0x887A_0020_u32 as i32);
/// `D2DERR_RECREATE_TARGET`: Direct2D pide rehacer el destino (lo da
/// `EndDraw` cuando el dispositivo debajo se perdio).
pub const RECREATE_TARGET: HRESULT = HRESULT(0x8899_000C_u32 as i32);

/// Si un codigo significa que el dispositivo ya no sirve y hay que rehacer
/// todo lo que vive en el. Lo demas (memoria, parametros, una ventana que ya
/// no existe) se trata como un fallo corriente: rehacer el dispositivo por
/// eso no arreglaria nada y costaria repintarlo todo.
pub fn es_perdida(hr: HRESULT) -> bool {
    matches!(
        hr,
        DEVICE_REMOVED | DEVICE_HUNG | DEVICE_RESET | DRIVER_INTERNAL_ERROR | RECREATE_TARGET
    )
}

/// El nombre corto de un codigo de perdida, para el registro.
pub fn nombre(hr: HRESULT) -> &'static str {
    match hr {
        DEVICE_REMOVED => "DEVICE_REMOVED",
        DEVICE_HUNG => "DEVICE_HUNG",
        DEVICE_RESET => "DEVICE_RESET",
        DRIVER_INTERNAL_ERROR => "DRIVER_INTERNAL_ERROR",
        RECREATE_TARGET => "RECREATE_TARGET",
        _ => "otro",
    }
}

/// Ya se aviso de una perdida y nadie la ha recogido todavia: un
/// dispositivo muerto hace fallar cada fotograma de cada ventana, y un aviso
/// por fallo inundaria la cola del hilo principal.
static AVISADA: AtomicBool = AtomicBool::new(false);
/// A quien se avisa (lo pone la aplicacion: despertar su bucle principal).
static AVISADOR: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

/// Lo que hay que hacer cuando algo descubre un dispositivo perdido. Una
/// vez por proceso; las siguientes llamadas no cambian nada. Se llama desde
/// el hilo que pinto, que puede ser cualquiera: tiene que ser barato y no
/// bloquear (un `PostMessage`).
pub fn al_perder(avisar: impl Fn() + Send + Sync + 'static) {
    let _ = AVISADOR.set(Box::new(avisar));
}

/// Recoge el aviso pendiente, si lo hay. El siguiente fallo volvera a
/// avisar.
pub fn tomar_aviso() -> bool {
    AVISADA.swap(false, Ordering::AcqRel)
}

std::thread_local! {
    /// Este hilo quiere que sus ventanas se cierren al perder el dispositivo
    /// (ver [`cerrar_ventanas_al_perder`]).
    static CERRAR_AL_PERDER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Y ya se pidio el cierre: una vez basta.
    static CIERRE_PEDIDO: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Las ventanas de ESTE hilo se cierran solas (`WM_CLOSE`) si se pierde el
/// dispositivo. Lo piden los hilos que abren una ventana con su propio
/// dispositivo (galeria, tareas, chat...): su lanzador ve que el bucle
/// acabo por una perdida y la vuelve a abrir sobre uno nuevo. Llamarlo otra
/// vez rearma el cierre para la ventana nueva.
///
/// El hilo principal NO lo pide: sus pines no se cierran, se rehacen en su
/// sitio (`Pin::cambiar_dispositivo`).
pub fn cerrar_ventanas_al_perder(si: bool) {
    CERRAR_AL_PERDER.with(|c| c.set(si));
    CIERRE_PEDIDO.with(|c| c.set(false));
}

/// Mira un error de Windows y, si es una perdida del dispositivo, lo avisa.
/// Devuelve si lo era.
pub fn senalar(e: &windows::core::Error) -> bool {
    senalar_codigo(e.code())
}

/// Como [`senalar`], con el codigo suelto.
pub fn senalar_codigo(hr: HRESULT) -> bool {
    if !es_perdida(hr) {
        return false;
    }
    if !AVISADA.swap(true, Ordering::AcqRel) {
        if let Some(avisar) = AVISADOR.get() {
            avisar();
        }
    }
    if CERRAR_AL_PERDER.with(|c| c.get()) && !CIERRE_PEDIDO.with(|c| c.replace(true)) {
        cerrar_las_ventanas_del_hilo();
    }
    true
}

/// Pide el cierre de las ventanas visibles del hilo que llama.
fn cerrar_las_ventanas_del_hilo() {
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumThreadWindows, IsWindowVisible, PostMessageW, WM_CLOSE,
    };
    unsafe extern "system" fn cada(hwnd: HWND, _: LPARAM) -> windows::core::BOOL {
        // SAFETY: `hwnd` lo da Windows y es del hilo que llama; publicar un
        // WM_CLOSE no toca memoria nuestra. Solo las visibles: las ocultas
        // (la del IME, las de mensajes) no son ventanas del usuario.
        unsafe {
            if IsWindowVisible(hwnd).as_bool() {
                let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
        }
        true.into()
    }
    // SAFETY: enumera las ventanas del hilo actual con un callback que no
    // guarda nada.
    unsafe {
        let _ = EnumThreadWindows(GetCurrentThreadId(), Some(cada), LPARAM(0));
    }
}

/// El fallo inyectado: la direccion del dispositivo y el codigo. Solo para
/// pruebas (y para provocar una perdida a mano al depurar).
static INYECTADO_EN: AtomicUsize = AtomicUsize::new(0);
static INYECTADO_CODIGO: AtomicI32 = AtomicI32::new(0);

/// Hace como si `d3d` se hubiera perdido con `hr` (o lo deja estar, con
/// `None`): pintar sobre el falla con ese codigo y [`motivo`] lo dice. Es la
/// unica forma de probar la recuperacion sin tumbar el driver.
#[doc(hidden)]
pub fn inyectar_perdida(d3d: &ID3D11Device, hr: Option<HRESULT>) {
    match hr {
        Some(hr) => {
            INYECTADO_CODIGO.store(hr.0, Ordering::Release);
            INYECTADO_EN.store(d3d.as_raw() as usize, Ordering::Release);
        }
        None => INYECTADO_EN.store(0, Ordering::Release),
    }
}

/// El fallo inyectado en el dispositivo cuya direccion es `crudo`, si hay.
pub(crate) fn inyectado(crudo: usize) -> Option<HRESULT> {
    let en = INYECTADO_EN.load(Ordering::Acquire);
    (en != 0 && en == crudo).then(|| HRESULT(INYECTADO_CODIGO.load(Ordering::Acquire)))
}

/// Si `d3d` esta perdido, por que (`GetDeviceRemovedReason`). `None` si
/// sigue sano. Es una consulta barata: se puede hacer en cada vuelta del
/// bucle.
pub fn motivo(d3d: &ID3D11Device) -> Option<HRESULT> {
    if let Some(hr) = inyectado(d3d.as_raw() as usize) {
        return Some(hr);
    }
    // SAFETY: consulta de solo lectura sobre un dispositivo vivo; es segura
    // desde cualquier hilo (el dispositivo D3D11 es libre de hilos).
    let hr = unsafe { d3d.GetDeviceRemovedReason() };
    match hr {
        Ok(()) => None,
        Err(e) => Some(e.code()),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_cinco_codigos_de_dispositivo_perdido_cuentan_como_perdida() {
        for hr in [
            DEVICE_REMOVED,
            DEVICE_HUNG,
            DEVICE_RESET,
            DRIVER_INTERNAL_ERROR,
            RECREATE_TARGET,
        ] {
            assert!(es_perdida(hr), "{hr:?} es una perdida");
            assert_ne!(nombre(hr), "otro");
        }
        // Los valores exactos, por si alguien los reescribe a mano.
        assert_eq!(DEVICE_REMOVED.0 as u32, 0x887A_0005);
        assert_eq!(RECREATE_TARGET.0 as u32, 0x8899_000C);
    }

    #[test]
    fn caso_negativo_otros_fallos_no_rehacen_el_dispositivo() {
        use windows::Win32::Foundation::{E_FAIL, E_INVALIDARG, E_OUTOFMEMORY, S_OK};
        // DXGI_ERROR_WAS_STILL_DRAWING (0x887A000A): ocupado, no perdido.
        let ocupado = HRESULT(0x887A_000A_u32 as i32);
        // DXGI_ERROR_INVALID_CALL (0x887A0001): fallo nuestro, no de la GPU.
        let mal_llamado = HRESULT(0x887A_0001_u32 as i32);
        for hr in [
            S_OK,
            E_FAIL,
            E_INVALIDARG,
            E_OUTOFMEMORY,
            ocupado,
            mal_llamado,
        ] {
            assert!(!es_perdida(hr), "{hr:?} no es una perdida");
            assert!(!senalar_codigo(hr));
        }
    }

    #[test]
    fn el_aviso_se_da_una_vez_hasta_que_se_recoge() {
        let _ = tomar_aviso();
        assert!(senalar_codigo(DEVICE_REMOVED));
        assert!(senalar_codigo(RECREATE_TARGET), "sigue siendo perdida");
        assert!(tomar_aviso(), "hay un aviso pendiente");
        assert!(!tomar_aviso(), "recogido, ya no queda");
        // Caso negativo: un fallo corriente no deja aviso.
        assert!(!senalar_codigo(windows::Win32::Foundation::E_FAIL));
        assert!(!tomar_aviso());
    }
}
