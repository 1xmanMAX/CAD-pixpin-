//! **Los dialogos de Abrir y Guardar** de Windows, con los filtros de CAD y
//! BIM (los de `pixpin-shell` son para fotos y videos).

use std::path::PathBuf;

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FOS_FILEMUSTEXIST, FOS_OVERWRITEPROMPT, FileOpenDialog, FileSaveDialog, IFileOpenDialog,
    IFileSaveDialog, SIGDN_FILESYSPATH,
};
use windows::core::{HSTRING, PCWSTR, w};

/// Que se abre.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Que {
    Plano,
    Modelo,
}

/// Pide un archivo para abrir. `None` si se cancela o el dialogo falla.
pub fn abrir(padre: HWND, que: Que) -> Option<PathBuf> {
    let planos = [
        COMDLG_FILTERSPEC {
            pszName: w!("Planos (DWG, DXF)"),
            pszSpec: w!("*.dwg;*.dxf"),
        },
        COMDLG_FILTERSPEC {
            pszName: w!("Todos"),
            pszSpec: w!("*.*"),
        },
    ];
    let modelos = [
        COMDLG_FILTERSPEC {
            pszName: w!("Modelos BIM (IFC, Revit, LandXML, puntos)"),
            pszSpec: w!("*.ifc;*.rvt;*.xml;*.csv;*.txt;*.pnt"),
        },
        COMDLG_FILTERSPEC {
            pszName: w!("Todos"),
            pszSpec: w!("*.*"),
        },
    ];
    let filtros: &[COMDLG_FILTERSPEC] = if que == Que::Plano { &planos } else { &modelos };
    // SAFETY: COM inicializado en este hilo (main); el dialogo es local y la
    // cadena que devuelve se libera con CoTaskMemFree.
    unsafe {
        let d: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        d.SetFileTypes(filtros).ok()?;
        let op = d.GetOptions().ok()?;
        d.SetOptions(op | FOS_FILEMUSTEXIST).ok()?;
        d.Show(Some(padre)).ok()?;
        let item = d.GetResult().ok()?;
        let p = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let ruta = p.to_string().ok().map(PathBuf::from);
        CoTaskMemFree(Some(p.0 as *const _));
        ruta
    }
}

/// Pide donde guardar un plano (DWG o DXF por la extension elegida).
pub fn guardar(padre: HWND, sugerido: &str) -> Option<PathBuf> {
    let filtros = [
        COMDLG_FILTERSPEC {
            pszName: w!("Dibujo de AutoCAD (DWG)"),
            pszSpec: w!("*.dwg"),
        },
        COMDLG_FILTERSPEC {
            pszName: w!("Intercambio (DXF)"),
            pszSpec: w!("*.dxf"),
        },
    ];
    guardar_con(padre, sugerido, &filtros, w!("dwg"))
}

/// Pide donde guardar una tabla de metrados.
pub fn guardar_csv(padre: HWND, sugerido: &str) -> Option<PathBuf> {
    let filtros = [COMDLG_FILTERSPEC {
        pszName: w!("Tabla (CSV)"),
        pszSpec: w!("*.csv"),
    }];
    guardar_con(padre, sugerido, &filtros, w!("csv"))
}

fn guardar_con(
    padre: HWND,
    sugerido: &str,
    filtros: &[COMDLG_FILTERSPEC],
    ext: PCWSTR,
) -> Option<PathBuf> {
    // SAFETY: como en `abrir`.
    unsafe {
        let d: IFileSaveDialog =
            CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        d.SetFileTypes(filtros).ok()?;
        d.SetDefaultExtension(ext).ok()?;
        let nombre = HSTRING::from(sugerido);
        let _ = d.SetFileName(&nombre);
        let op = d.GetOptions().ok()?;
        d.SetOptions(op | FOS_OVERWRITEPROMPT).ok()?;
        d.Show(Some(padre)).ok()?;
        let item = d.GetResult().ok()?;
        let p = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let ruta = p.to_string().ok().map(PathBuf::from);
        CoTaskMemFree(Some(p.0 as *const _));
        ruta
    }
}
