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
        let d: IFileSaveDialog = match CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)
        {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!(?e, "sin dialogo de guardar");
                return None;
            }
        };
        if let Err(e) = d.SetFileTypes(filtros) {
            tracing::warn!(?e, "SetFileTypes");
            return None;
        }
        if let Err(e) = d.SetDefaultExtension(ext) {
            tracing::warn!(?e, "SetDefaultExtension");
            return None;
        }
        let nombre = HSTRING::from(sugerido);
        let _ = d.SetFileName(&nombre);
        let op = d.GetOptions().ok()?;
        d.SetOptions(op | FOS_OVERWRITEPROMPT).ok()?;
        if let Err(e) = d.Show(Some(padre)) {
            tracing::warn!(?e, "Show");
            return None;
        }
        let item = d.GetResult().ok()?;
        let p = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let ruta = p.to_string().ok().map(PathBuf::from);
        CoTaskMemFree(Some(p.0 as *const _));
        ruta
    }
}

/// **El dialogo que pide una orden** ([`cad_nucleo::herramienta::PedidoApp::Archivo`]):
/// abrir o guardar, con sus filtros (`("PDF", "*.pdf")`).
pub fn elegir(
    padre: HWND,
    guardar: bool,
    titulo: &str,
    filtros: &[(String, String)],
    sugerido: &str,
) -> Option<PathBuf> {
    // Las cadenas tienen que vivir mientras el dialogo las usa.
    let nombres: Vec<HSTRING> = filtros
        .iter()
        .map(|f| HSTRING::from(f.0.as_str()))
        .collect();
    let patrones: Vec<HSTRING> = filtros
        .iter()
        .map(|f| HSTRING::from(f.1.as_str()))
        .collect();
    let mut specs: Vec<COMDLG_FILTERSPEC> = nombres
        .iter()
        .zip(&patrones)
        .map(|(n, p)| COMDLG_FILTERSPEC {
            pszName: PCWSTR(n.as_ptr()),
            pszSpec: PCWSTR(p.as_ptr()),
        })
        .collect();
    if specs.is_empty() {
        specs.push(COMDLG_FILTERSPEC {
            pszName: w!("Todos"),
            pszSpec: w!("*.*"),
        });
    }
    let titulo = HSTRING::from(titulo);
    // SAFETY: como en `abrir`; las cadenas de arriba viven hasta el final.
    unsafe {
        if guardar {
            let d: IFileSaveDialog =
                CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER).ok()?;
            d.SetFileTypes(&specs).ok()?;
            let _ = d.SetTitle(&titulo);
            // La extension por defecto: la del primer patron.
            let ext = filtros
                .first()
                .and_then(|f| f.1.split(';').next())
                .and_then(|p| p.rsplit('.').next())
                .filter(|e| *e != "*")
                .map(HSTRING::from);
            if let Some(ext) = &ext {
                let _ = d.SetDefaultExtension(ext);
            }
            let _ = d.SetFileName(&HSTRING::from(sugerido));
            let op = d.GetOptions().ok()?;
            d.SetOptions(op | FOS_OVERWRITEPROMPT).ok()?;
            d.Show(Some(padre)).ok()?;
            let item = d.GetResult().ok()?;
            let p = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
            let ruta = p.to_string().ok().map(PathBuf::from);
            CoTaskMemFree(Some(p.0 as *const _));
            ruta
        } else {
            let d: IFileOpenDialog =
                CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
            d.SetFileTypes(&specs).ok()?;
            let _ = d.SetTitle(&titulo);
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
}
