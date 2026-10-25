//! **Los archivos recientes** de la pantalla de inicio: una ruta por linea en
//! `%APPDATA%\CAD-pixpin\recientes.txt`, el mas nuevo arriba.

use std::path::{Path, PathBuf};

const CUANTOS: usize = 12;

pub fn carpeta() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("CAD-pixpin")
}

fn fichero() -> PathBuf {
    carpeta().join("recientes.txt")
}

/// Los recientes que aun existen.
pub fn leer() -> Vec<PathBuf> {
    de_texto(&std::fs::read_to_string(fichero()).unwrap_or_default())
        .into_iter()
        .filter(|p| p.is_file())
        .collect()
}

pub fn de_texto(t: &str) -> Vec<PathBuf> {
    t.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// Pone `ruta` arriba del todo (sin repetirla).
pub fn anadir_a(lista: &[PathBuf], ruta: &Path) -> Vec<PathBuf> {
    let clave = |p: &Path| p.to_string_lossy().to_lowercase();
    let mut v = vec![ruta.to_path_buf()];
    v.extend(lista.iter().filter(|p| clave(p) != clave(ruta)).cloned());
    v.truncate(CUANTOS);
    v
}

pub fn anadir(ruta: &Path) {
    let v = anadir_a(&leer(), ruta);
    let texto: String = v.iter().map(|p| format!("{}\n", p.display())).collect();
    if std::fs::create_dir_all(carpeta()).is_ok()
        && let Err(e) = std::fs::write(fichero(), texto)
    {
        tracing::warn!(?e, "no se pudieron guardar los recientes");
    }
}

/// La etiqueta del tipo de archivo (la pastilla de color de la lista).
pub fn tipo(ruta: &Path) -> &'static str {
    match ruta
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .as_deref()
    {
        Some("dwg") => "DWG",
        Some("dxf") => "DXF",
        Some("ifc") => "IFC",
        Some("rvt") => "RVT",
        Some("xml") => "XML",
        _ => "CAD",
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_nuevo_arriba_sin_repetir() {
        let l = de_texto("C:\\a.dwg\n\nC:\\b.dxf\n");
        let v = anadir_a(&l, Path::new("c:\\B.DXF"));
        assert_eq!(v.len(), 2);
        assert_eq!(v[0], PathBuf::from("c:\\B.DXF"));
        assert_eq!(v[1], PathBuf::from("C:\\a.dwg"));
    }
}
