//! El dialogo de imprimir de Windows (`PrintDlgEx`), el de siempre: elegir
//! impresora, papel, orientacion, copias y que paginas.
//!
//! Es el equivalente del `PrintManager` del movil (`guardados/Imprimir.kt`):
//! el sistema pregunta y el programa solo pinta. Devuelve lo elegido en datos
//! planos —el nombre de la impresora, su `DEVMODE` en bytes y el tamano del
//! papel en DIP— para que quien imprime (`pixpin_render::imprimir`) no tenga
//! que saber nada de manejadores de memoria de Win32.
//!
//! Sin prueba automatica, como `guardar.rs`: un dialogo modal espera a una
//! persona. Lo que se puede probar sin ella —convertir lo que devuelve en
//! paginas— esta en funciones puras aparte.

use windows::Win32::Foundation::{GlobalFree, HGLOBAL, HWND};
use windows::Win32::Graphics::Gdi::{
    DEVMODEW, DM_ORIENTATION, DMORIENT_LANDSCAPE, DMORIENT_PORTRAIT, DeleteDC, GetDeviceCaps,
    LOGPIXELSX, LOGPIXELSY, PHYSICALHEIGHT, PHYSICALWIDTH,
};
use windows::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};
use windows::Win32::UI::Controls::Dialogs::{
    DEVNAMES, PD_NOCURRENTPAGE, PD_NOPAGENUMS, PD_NOSELECTION, PD_PAGENUMS, PD_RESULT_PRINT,
    PD_RETURNDC, PD_SELECTION, PD_USEDEVMODECOPIESANDCOLLATE, PRINTDLGEXW, PRINTPAGERANGE,
    PrintDlgExW, START_PAGE_GENERAL,
};

/// Que paginas se eligieron en el dialogo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rango {
    Todas,
    /// «Seleccion»: solo lo elegido en el lienzo.
    Seleccion,
    /// Intervalos de paginas, desde uno y cerrados por los dos lados.
    Paginas(Vec<(u32, u32)>),
}

impl Rango {
    /// Si la pagina `n` (desde uno) entra en lo elegido.
    pub fn incluye(&self, n: u32) -> bool {
        match self {
            Rango::Todas | Rango::Seleccion => true,
            Rango::Paginas(v) => v.iter().any(|(a, b)| n >= *a && n <= *b),
        }
    }
}

/// Lo elegido en el dialogo.
#[derive(Debug, Clone)]
pub struct Impresion {
    pub impresora: String,
    /// El `DEVMODEW` del controlador, entero, en bytes.
    pub devmode: Vec<u8>,
    /// El papel, en DIP (1/96 de pulgada), ya con la orientacion elegida.
    pub papel: (f32, f32),
    pub rango: Rango,
}

/// Cuantos intervalos de paginas se aceptan. Diez es lo que deja escribir
/// con holgura el cuadro de «Paginas» sin reservar de mas.
const MAX_INTERVALOS: usize = 10;

/// De pixeles de impresora a DIP.
pub fn a_dip(pixeles: i32, ppp: i32) -> f32 {
    if ppp <= 0 {
        return 0.0;
    }
    pixeles as f32 * 96.0 / ppp as f32
}

/// El DEVMODE inicial: solo la orientacion, para que el dialogo salga ya
/// tumbado si la primera hoja es apaisada (lo que hace el movil al abrir
/// su dialogo). El resto lo completa el controlador.
fn devmode_inicial(apaisada: bool) -> Option<HGLOBAL> {
    let mut dm = DEVMODEW {
        dmSize: size_of::<DEVMODEW>() as u16,
        dmFields: DM_ORIENTATION,
        ..Default::default()
    };
    dm.Anonymous1.Anonymous1.dmOrientation = if apaisada {
        DMORIENT_LANDSCAPE
    } else {
        DMORIENT_PORTRAIT
    } as i16;
    // SAFETY: bloque movil nuevo del tamano de la estructura; se copia con
    // el bloque bloqueado y se desbloquea en seguida. Si algo falla se
    // libera antes de devolver.
    unsafe {
        let h = GlobalAlloc(GMEM_MOVEABLE, size_of::<DEVMODEW>()).ok()?;
        let p = GlobalLock(h) as *mut DEVMODEW;
        if p.is_null() {
            let _ = GlobalFree(Some(h));
            return None;
        }
        p.write(dm);
        let _ = GlobalUnlock(h);
        Some(h)
    }
}

/// Los bytes de un bloque de memoria global, sin soltarlo.
///
/// # Safety
/// `h` tiene que ser un HGLOBAL vivo.
unsafe fn bytes_de(h: HGLOBAL) -> Vec<u8> {
    // SAFETY: lo garantiza quien llama; se desbloquea antes de salir.
    unsafe {
        let largo = GlobalSize(h);
        let p = GlobalLock(h) as *const u8;
        if p.is_null() || largo == 0 {
            return Vec::new();
        }
        let v = std::slice::from_raw_parts(p, largo).to_vec();
        let _ = GlobalUnlock(h);
        v
    }
}

/// El nombre de la impresora de un DEVNAMES.
///
/// # Safety
/// `h` tiene que ser el HGLOBAL de un DEVNAMES vivo.
unsafe fn nombre_de(h: HGLOBAL) -> Option<String> {
    // SAFETY: lo garantiza quien llama; las cadenas del DEVNAMES terminan en
    // cero dentro del bloque, y el desplazamiento va en caracteres anchos.
    unsafe {
        let p = GlobalLock(h) as *const u16;
        if p.is_null() {
            return None;
        }
        let dn = &*(p as *const DEVNAMES);
        let inicio = p.add(dn.wDeviceOffset as usize);
        let mut largo = 0;
        while *inicio.add(largo) != 0 {
            largo += 1;
        }
        let s = String::from_utf16_lossy(std::slice::from_raw_parts(inicio, largo));
        let _ = GlobalUnlock(h);
        Some(s)
    }
}

/// **Abre el dialogo de imprimir.** `paginas` es cuantas hay (una por marco,
/// o una si no hay marcos); `hay_seleccion` enciende la opcion «Seleccion»;
/// `apaisada` pone la orientacion de salida.
///
/// `None` si se cancela o el dialogo falla: no se imprime nada.
pub fn pedir_impresion(
    propietaria: HWND,
    paginas: u32,
    hay_seleccion: bool,
    apaisada: bool,
) -> Option<Impresion> {
    // Con la orientacion puesta y, si el controlador no acepta ese DEVMODE
    // a medias (hay alguno que no), con lo que la impresora traiga: mejor
    // un dialogo derecho que ninguno.
    match un_intento(
        propietaria,
        paginas,
        hay_seleccion,
        devmode_inicial(apaisada),
    ) {
        Ok(r) => r,
        Err(()) => un_intento(propietaria, paginas, hay_seleccion, None)
            .ok()
            .flatten(),
    }
}

/// Una apertura del dialogo. `Err` es que el dialogo mismo fallo (no que se
/// cancelara), para volver a probar sin el DEVMODE inicial.
fn un_intento(
    propietaria: HWND,
    paginas: u32,
    hay_seleccion: bool,
    devmode: Option<HGLOBAL>,
) -> Result<Option<Impresion>, ()> {
    let mut intervalos = [PRINTPAGERANGE::default(); MAX_INTERVALOS];
    let mut flags = PD_RETURNDC | PD_USEDEVMODECOPIESANDCOLLATE | PD_NOCURRENTPAGE;
    if !hay_seleccion {
        flags |= PD_NOSELECTION;
    }
    if paginas <= 1 {
        flags |= PD_NOPAGENUMS;
    }
    let mut pd = PRINTDLGEXW {
        lStructSize: size_of::<PRINTDLGEXW>() as u32,
        hwndOwner: propietaria,
        hDevMode: devmode.unwrap_or_default(),
        Flags: flags,
        nMaxPageRanges: MAX_INTERVALOS as u32,
        lpPageRanges: intervalos.as_mut_ptr(),
        nMinPage: 1,
        nMaxPage: paginas.max(1),
        nCopies: 1,
        nStartPage: START_PAGE_GENERAL,
        ..Default::default()
    };
    // Lo de PixPin que va siempre encima (pines, barras) taparia el dialogo.
    let bajadas = bajar_las_de_encima();
    // SAFETY: la estructura esta rellena y vive durante la llamada; los
    // intervalos son un arreglo local del tamano declarado. El dialogo es
    // modal sobre la ventana propietaria, que es del llamante.
    let hecho = unsafe { PrintDlgExW(&mut pd) };
    subir_las_de_encima(&bajadas);
    let resultado = pd.dwResultAction;
    let hdc = pd.hDC;
    let (h_devmode, h_devnames) = (pd.hDevMode, pd.hDevNames);
    // Lo que haya que leer, antes de soltar nada.
    let salida = if hecho.is_ok() && resultado == PD_RESULT_PRINT && !hdc.is_invalid() {
        // SAFETY: el dialogo devolvio un DC de impresora vivo.
        let (w, h, px, py) = unsafe {
            (
                GetDeviceCaps(Some(hdc), PHYSICALWIDTH),
                GetDeviceCaps(Some(hdc), PHYSICALHEIGHT),
                GetDeviceCaps(Some(hdc), LOGPIXELSX),
                GetDeviceCaps(Some(hdc), LOGPIXELSY),
            )
        };
        // SAFETY: con PD_RESULT_PRINT los dos bloques vienen rellenos.
        let impresora = unsafe { nombre_de(h_devnames) };
        // SAFETY: idem.
        let devmode = unsafe { bytes_de(h_devmode) };
        let rango = if pd.Flags.0 & PD_SELECTION.0 != 0 {
            Rango::Seleccion
        } else if pd.Flags.0 & PD_PAGENUMS.0 != 0 {
            Rango::Paginas(
                intervalos[..(pd.nPageRanges as usize).min(MAX_INTERVALOS)]
                    .iter()
                    .map(|r| (r.nFromPage, r.nToPage))
                    .collect(),
            )
        } else {
            Rango::Todas
        };
        impresora.map(|impresora| Impresion {
            impresora,
            devmode,
            papel: (a_dip(w, px), a_dip(h, py)),
            rango,
        })
    } else {
        None
    };
    // SAFETY: el DC y los bloques son del dialogo y ahora nuestros; se
    // sueltan una vez, existan o no.
    unsafe {
        if !hdc.is_invalid() {
            let _ = DeleteDC(hdc);
        }
        if !h_devmode.is_invalid() {
            let _ = GlobalFree(Some(h_devmode));
        }
        if !h_devnames.is_invalid() {
            let _ = GlobalFree(Some(h_devnames));
        }
    }
    if hecho.is_err() {
        return Err(());
    }
    Ok(salida.filter(|i| i.papel.0 > 0.0 && i.papel.1 > 0.0))
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
    use windows::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, HWND_NOTOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
        SetWindowPos, WS_EX_TOPMOST,
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
    use windows::Win32::UI::WindowsAndMessaging::{
        HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
    };
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

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_a4_a_600_ppp_son_794_por_1123_dip() {
        // 4960 x 7016 pixeles son un A4 a 600 ppp.
        assert!((a_dip(4960, 600) - 793.6).abs() < 0.1);
        assert!((a_dip(7016, 600) - 1122.6).abs() < 0.1);
        // Caso negativo: un controlador que dice cero ppp no divide por cero.
        assert_eq!(a_dip(100, 0), 0.0);
    }

    #[test]
    fn el_rango_de_paginas_dice_que_hoja_entra() {
        let r = Rango::Paginas(vec![(1, 2), (5, 5)]);
        assert!(r.incluye(1) && r.incluye(2) && r.incluye(5));
        assert!(!r.incluye(3) && !r.incluye(6));
        assert!(Rango::Todas.incluye(99));
    }
}
