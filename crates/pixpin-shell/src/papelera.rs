//! **La papelera de reciclaje de Windows.**
//!
//! Lo que se va solo (las capturas que caducan a la semana, 3-oct) no se
//! borra de verdad: va a la papelera de Windows, de donde el usuario lo
//! saca con «Restaurar» si se fue algo que queria. Es `SHFileOperationW`
//! con `FOF_ALLOWUNDO`, lo mismo que pulsar Supr en el Explorador, pero sin
//! preguntar ni ensenar la barra de progreso.

use std::path::Path;

use windows::Win32::UI::Shell::{
    FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, SHFILEOPSTRUCTW,
    SHFileOperationW,
};
use windows::core::PCWSTR;

/// Manda `ruta` (fichero o carpeta) a la papelera de reciclaje.
///
/// Un fichero que no esta es un error: quien llama decide si le importa.
/// En una unidad sin papelera (una memoria USB, una red) Windows lo borra
/// del todo, como hace el Explorador.
pub fn a_la_papelera_de_reciclaje(ruta: &Path) -> std::io::Result<()> {
    if !ruta.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("no existe {}", ruta.display()),
        ));
    }
    // `pFrom` es una LISTA de rutas acabada en dos ceros: con uno solo,
    // Windows sigue leyendo memoria detras buscando la segunda.
    let mut desde: Vec<u16> = ruta.as_os_str().encode_wide_lossless();
    desde.push(0);
    desde.push(0);
    let mut op = SHFILEOPSTRUCTW {
        wFunc: FO_DELETE,
        pFrom: PCWSTR(desde.as_ptr()),
        fFlags: (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_NOERRORUI | FOF_SILENT).0 as u16,
        ..Default::default()
    };
    // SAFETY: `desde` vive hasta el final de la funcion y acaba en doble
    // cero; la estructura es propia y no se guarda nada de ella.
    let r = unsafe { SHFileOperationW(&mut op) };
    if r != 0 {
        return Err(std::io::Error::other(format!(
            "SHFileOperationW devolvio {r:#x} para {}",
            ruta.display()
        )));
    }
    if op.fAnyOperationsAborted.as_bool() {
        return Err(std::io::Error::other("se cancelo el envio a la papelera"));
    }
    Ok(())
}

trait EnUtf16 {
    fn encode_wide_lossless(&self) -> Vec<u16>;
}

impl EnUtf16 for std::ffi::OsStr {
    fn encode_wide_lossless(&self) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        self.encode_wide().collect()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_fichero_se_va_de_su_carpeta() {
        let dir = std::env::temp_dir().join(format!("pixpin-papelera-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("captura-de-prueba.png");
        std::fs::write(&f, b"x").unwrap();
        a_la_papelera_de_reciclaje(&f).unwrap();
        assert!(!f.exists());
        // Caso negativo: lo que no esta no se finge enviado.
        assert!(a_la_papelera_de_reciclaje(&f).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
