//! **Un PowerPoint pasado a PDF con el PowerPoint que ya haya instalado**
//! (D10).
//!
//! En el movil, `guardados/DiapositivasAPdf.kt` lee el `.pptx` a mano
//! (`motor/Diapositivas.kt`) y pinta cada diapositiva en una pagina de PDF.
//! Es un maquetador entero, con sus limites dichos en su cabecera (sin
//! graficos, SmartArt, degradados ni fuentes incrustadas). En un PC con
//! Office no hace falta: el propio PowerPoint sabe guardar como PDF
//! (`SaveAs` con `ppSaveAsPDF = 32`) y lo hace **exacto** —las fuentes, los
//! graficos, el SmartArt—, con cero codigo de maquetacion aqui. Es la regla
//! de la casa: usar lo que Windows (y lo que el usuario ya tiene) trae.
//!
//! Se habla con el por **automatizacion COM** (`IDispatch`), sin
//! bibliotecas de tipos: los nombres se piden con `GetIDsOfNames`, como hace
//! un guion de VBScript. Sin PowerPoint, [`hay_powerpoint`] dice que no y
//! quien llama lo explica y ofrece «Abrir con otra app».
//!
//! # Las dos trampas
//!
//! - **PowerPoint es de una sola instancia**: `CoCreateInstance` devuelve el
//!   que el usuario ya tenga abierto. Cerrarlo con `Quit` le cerraria sus
//!   presentaciones. Por eso se mira antes con `GetActiveObject` si ya
//!   estaba, y solo se cierra lo que se abrio aqui.
//! - **Nada de ventanas**: la presentacion se abre con `WithWindow = falso`
//!   y de solo lectura, y la aplicacion no se hace visible. Un cuadro de
//!   dialogo de PowerPoint esperando a nadie colgaria el hilo.
//!
//! Sin prueba automatica que arranque PowerPoint en la bateria normal: la
//! hay, marcada `#[ignore]`, para correrla a mano en un equipo con Office.

use std::path::Path;

use windows::Win32::System::Com::{
    CLSCTX_LOCAL_SERVER, CLSIDFromProgID, CoCreateInstance, DISPATCH_FLAGS, DISPATCH_METHOD,
    DISPATCH_PROPERTYGET, DISPATCH_PROPERTYPUT, DISPPARAMS, IDispatch,
};
use windows::Win32::System::Ole::{DISPID_PROPERTYPUT, GetActiveObject};
use windows::Win32::System::Variant::{VARIANT, VT_BSTR, VT_DISPATCH, VT_I4, VariantClear};
use windows::core::{BSTR, GUID, HSTRING, PCWSTR};

/// `ppSaveAsPDF` de la enumeracion `PpSaveAsFileType`.
const PP_SAVE_AS_PDF: i32 = 32;
/// `msoTrue` y `msoFalse`: los booleanos de Office son enteros.
const MSO_TRUE: i32 = -1;
const MSO_FALSE: i32 = 0;
/// `ppAlertsNone`: que no salga ningun aviso esperando a nadie.
const PP_ALERTS_NONE: i32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum ErrorPowerPoint {
    #[error("PowerPoint no esta instalado")]
    NoHay,
    #[error("PowerPoint no pudo abrir la presentacion: {0}")]
    NoAbre(String),
    #[error("PowerPoint no pudo guardarla como PDF: {0}")]
    NoGuarda(String),
    #[error("error de COM al hablar con PowerPoint: {0}")]
    Com(#[from] windows::core::Error),
}

fn clsid() -> Option<GUID> {
    // SAFETY: cadena propia viva durante la llamada; CLSIDFromProgID solo
    // lee el registro.
    unsafe { CLSIDFromProgID(&HSTRING::from("PowerPoint.Application")).ok() }
}

/// Si hay un PowerPoint registrado en este equipo. Solo mira el registro
/// (`PowerPoint.Application`): no arranca nada.
pub fn hay_powerpoint() -> bool {
    clsid().is_some()
}

/// Un `VARIANT` que se limpia solo al salir de su ambito.
struct Var(VARIANT);

impl Drop for Var {
    fn drop(&mut self) {
        // SAFETY: el VARIANT es propio y esta bien formado (lo crearon las
        // funciones de aqui o lo relleno `Invoke`).
        unsafe {
            let _ = VariantClear(&mut self.0);
        }
    }
}

impl Var {
    fn vacio() -> Var {
        Var(VARIANT::default())
    }

    fn entero(n: i32) -> Var {
        let mut v = VARIANT::default();
        // SAFETY: se escribe el discriminante y el campo que le corresponde.
        unsafe {
            let dentro = &mut v.Anonymous.Anonymous;
            dentro.vt = VT_I4;
            dentro.Anonymous.lVal = n;
        }
        Var(v)
    }

    fn texto(s: &str) -> Var {
        let mut v = VARIANT::default();
        // SAFETY: igual; el BSTR pasa a ser del VARIANT y lo suelta
        // `VariantClear` en el `Drop`.
        unsafe {
            let dentro = &mut v.Anonymous.Anonymous;
            dentro.vt = VT_BSTR;
            dentro.Anonymous.bstrVal = std::mem::ManuallyDrop::new(BSTR::from(s));
        }
        Var(v)
    }

    fn despacho(&self) -> Option<IDispatch> {
        // SAFETY: se lee el campo que dice el discriminante; se clona la
        // interfaz (AddRef) y el VARIANT sigue siendo dueno de la suya.
        unsafe {
            let dentro = &self.0.Anonymous.Anonymous;
            if dentro.vt != VT_DISPATCH {
                return None;
            }
            (*dentro.Anonymous.pdispVal).clone()
        }
    }
}

/// Llama a `nombre` de `objeto`. `args` en el orden natural (el de VBA): se
/// dan la vuelta aqui, que `DISPPARAMS` los quiere del ultimo al primero.
fn llamar(
    objeto: &IDispatch,
    nombre: &str,
    tipo: DISPATCH_FLAGS,
    args: Vec<Var>,
) -> windows::core::Result<Var> {
    let ancho = HSTRING::from(nombre);
    let nombres = [PCWSTR(ancho.as_ptr())];
    let mut id = 0i32;
    // SAFETY: un nombre, un id; punteros validos durante la llamada.
    unsafe { objeto.GetIDsOfNames(&GUID::zeroed(), nombres.as_ptr(), 1, 0, &mut id)? };
    let mut crudos: Vec<VARIANT> = args
        .iter()
        .rev()
        .map(|v| {
            // SAFETY: copia bit a bit; los originales en `args` siguen siendo
            // los duenos y se limpian una sola vez, al final.
            unsafe { std::mem::transmute_copy(&v.0) }
        })
        .collect();
    let mut nombrado = DISPID_PROPERTYPUT;
    let parametros = DISPPARAMS {
        rgvarg: if crudos.is_empty() {
            std::ptr::null_mut()
        } else {
            crudos.as_mut_ptr()
        },
        rgdispidNamedArgs: if tipo == DISPATCH_PROPERTYPUT {
            &mut nombrado
        } else {
            std::ptr::null_mut()
        },
        cArgs: crudos.len() as u32,
        cNamedArgs: u32::from(tipo == DISPATCH_PROPERTYPUT),
    };
    let mut resultado = Var::vacio();
    // SAFETY: `parametros` apunta a memoria viva hasta despues de la
    // llamada; el resultado es un VARIANT propio que limpia `Var`.
    let hecho = unsafe {
        objeto.Invoke(
            id,
            &GUID::zeroed(),
            0,
            tipo,
            &parametros,
            Some(&mut resultado.0),
            None,
            None,
        )
    };
    // Las copias no se limpian (serian dos `VariantClear`): se olvidan.
    for c in crudos.drain(..) {
        std::mem::forget(c);
    }
    drop(args);
    hecho.map(|_| resultado)
}

fn propiedad(objeto: &IDispatch, nombre: &str) -> windows::core::Result<IDispatch> {
    llamar(objeto, nombre, DISPATCH_PROPERTYGET, Vec::new())?
        .despacho()
        .ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_NOINTERFACE))
}

/// Si hay un PowerPoint en marcha (el del usuario, casi siempre).
fn abierto(clsid: &GUID) -> bool {
    let mut previo: Option<windows::core::IUnknown> = None;
    // SAFETY: GetActiveObject solo consulta la tabla de objetos activos.
    unsafe { GetActiveObject(clsid, None, &mut previo) }.is_ok() && previo.is_some()
}

/// **`origen` (un `.pptx`, `.ppt`, `.odp`... lo que abra PowerPoint) a PDF
/// en `destino`**, una pagina por diapositiva visible. Bloquea hasta que
/// acaba: llamarla desde un hilo propio. Inicia COM en el hilo si hace falta.
pub fn a_pdf(origen: &Path, destino: &Path) -> Result<(), ErrorPowerPoint> {
    let _com = crate::ComDelHilo::iniciar();
    let clsid = clsid().ok_or(ErrorPowerPoint::NoHay)?;
    // ¿Estaba ya abierto? Entonces es del usuario y no se cierra.
    let ya_estaba = abierto(&clsid);
    // SAFETY: CoCreateInstance con un CLSID del registro; servidor local
    // porque PowerPoint es un .exe aparte.
    let app: IDispatch = unsafe { CoCreateInstance(&clsid, None, CLSCTX_LOCAL_SERVER)? };
    if !ya_estaba {
        // Que no pregunte nada a nadie. Si falla, se sigue: es un seguro.
        let _ = llamar(
            &app,
            "DisplayAlerts",
            DISPATCH_PROPERTYPUT,
            vec![Var::entero(PP_ALERTS_NONE)],
        );
    }
    let resultado = convertir(&app, origen, destino);
    if !ya_estaba {
        // Solo si no le queda nada abierto: otra presentacion que el
        // usuario abriera mientras tanto no se le cierra.
        let quedan = llamar(&app, "Presentations", DISPATCH_PROPERTYGET, Vec::new())
            .ok()
            .and_then(|v| v.despacho())
            .and_then(|p| llamar(&p, "Count", DISPATCH_PROPERTYGET, Vec::new()).ok())
            // SAFETY: se lee `lVal` solo si el discriminante es VT_I4.
            .map(|v| unsafe {
                let d = &v.0.Anonymous.Anonymous;
                if d.vt == VT_I4 { d.Anonymous.lVal } else { 1 }
            })
            .unwrap_or(0);
        if quedan == 0 {
            let _ = llamar(&app, "Quit", DISPATCH_METHOD, Vec::new());
        }
    }
    resultado
}

fn convertir(app: &IDispatch, origen: &Path, destino: &Path) -> Result<(), ErrorPowerPoint> {
    let presentaciones = propiedad(app, "Presentations")?;
    let abierta = llamar(
        &presentaciones,
        "Open",
        DISPATCH_METHOD,
        vec![
            Var::texto(&origen.to_string_lossy()),
            // ReadOnly, Untitled (asi no se engancha al fichero), WithWindow.
            Var::entero(MSO_TRUE),
            Var::entero(MSO_TRUE),
            Var::entero(MSO_FALSE),
        ],
    )
    .map_err(|e| ErrorPowerPoint::NoAbre(e.message().to_string()))?
    .despacho()
    .ok_or_else(|| ErrorPowerPoint::NoAbre("sin presentacion".into()))?;
    let guardado = llamar(
        &abierta,
        "SaveAs",
        DISPATCH_METHOD,
        vec![
            Var::texto(&destino.to_string_lossy()),
            Var::entero(PP_SAVE_AS_PDF),
        ],
    );
    // Se cierra siempre, haya salido bien o no.
    let _ = llamar(&abierta, "Close", DISPATCH_METHOD, Vec::new());
    guardado.map_err(|e| ErrorPowerPoint::NoGuarda(e.message().to_string()))?;
    if !destino.is_file() {
        return Err(ErrorPowerPoint::NoGuarda("no aparecio el PDF".into()));
    }
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn preguntar_si_hay_powerpoint_no_arranca_nada_y_no_falla() {
        // Solo mira el registro: da lo mismo que diga si o no, pero no puede
        // colgarse ni dejar un POWERPNT.EXE vivo.
        let _ = hay_powerpoint();
    }

    /// Arranca PowerPoint de verdad: a mano, en un equipo con Office
    /// (`cargo test -p pixpin-shell powerpoint -- --ignored`).
    #[test]
    #[ignore]
    fn una_presentacion_de_dos_diapositivas_sale_como_pdf() {
        let _com = crate::ComDelHilo::iniciar();
        // Sin PowerPoint no hay nada que probar; con el del usuario abierto,
        // tampoco: esta prueba no toca las aplicaciones de nadie.
        let Some(id) = clsid() else { return };
        if abierto(&id) {
            return;
        }
        let dir = std::env::temp_dir().join("pixpin-pp-prueba");
        let _ = std::fs::create_dir_all(&dir);
        let pptx = dir.join("dos.pptx");
        let pdf = dir.join("dos.pdf");
        let _ = std::fs::remove_file(&pdf);
        {
            // La presentacion de prueba la hace el propio PowerPoint.
            // SAFETY: como en `a_pdf`.
            let app: IDispatch =
                unsafe { CoCreateInstance(&clsid().unwrap(), None, CLSCTX_LOCAL_SERVER).unwrap() };
            let pres = propiedad(&app, "Presentations").unwrap();
            let nueva = llamar(&pres, "Add", DISPATCH_METHOD, vec![Var::entero(MSO_FALSE)])
                .unwrap()
                .despacho()
                .unwrap();
            let diapos = propiedad(&nueva, "Slides").unwrap();
            for i in 1..=2 {
                // ppLayoutBlank = 12
                llamar(
                    &diapos,
                    "Add",
                    DISPATCH_METHOD,
                    vec![Var::entero(i), Var::entero(12)],
                )
                .unwrap();
            }
            // ppSaveAsOpenXMLPresentation = 24
            llamar(
                &nueva,
                "SaveAs",
                DISPATCH_METHOD,
                vec![Var::texto(&pptx.to_string_lossy()), Var::entero(24)],
            )
            .unwrap();
            let _ = llamar(&nueva, "Close", DISPATCH_METHOD, Vec::new());
            // Se cierra aqui: si siguiera vivo, `a_pdf` lo tomaria por el
            // del usuario y lo dejaria abierto.
            let _ = llamar(&app, "Quit", DISPATCH_METHOD, Vec::new());
        }
        a_pdf(&pptx, &pdf).unwrap();
        let bytes = std::fs::read(&pdf).unwrap();
        assert!(bytes.starts_with(b"%PDF"), "tiene que ser un PDF");
        // Y no deja PowerPoint vivo: lo arranco el y lo cierra el.
        std::thread::sleep(std::time::Duration::from_secs(3));
        assert!(!abierto(&id), "PowerPoint se quedo abierto");
    }

    #[test]
    #[ignore]
    fn un_fichero_que_no_existe_no_se_convierte_y_lo_dice() {
        if !hay_powerpoint() {
            return;
        }
        let dir = std::env::temp_dir().join("pixpin-pp-no-existe");
        let r = a_pdf(&dir.join("no-esta.pptx"), &dir.join("no-esta.pdf"));
        assert!(matches!(r, Err(ErrorPowerPoint::NoAbre(_))), "{r:?}");
    }
}
