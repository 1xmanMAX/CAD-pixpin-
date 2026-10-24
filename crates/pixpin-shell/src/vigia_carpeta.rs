//! **Saber si cambio algo bajo una carpeta**, sin recorrerla: Windows
//! (`FindFirstChangeNotificationW`) marca el aviso y quien vigila lo mira
//! cuando le viene bien (sin esperar).
//!
//! Lo usa el indice de documentos del asistente
//! (`apps/pixpin/src/asistente/documentos.rs`) para ponerse al dia solo
//! cuando entra o cambia algo en `proyectos\`. Aqui y no en la aplicacion
//! porque la aplicacion prohibe `unsafe`.

use std::path::Path;

use windows::Win32::Foundation::{HANDLE, WAIT_OBJECT_0};
use windows::Win32::Storage::FileSystem::{
    FILE_NOTIFY_CHANGE_FILE_NAME, FILE_NOTIFY_CHANGE_LAST_WRITE, FILE_NOTIFY_CHANGE_SIZE,
    FindCloseChangeNotification, FindFirstChangeNotificationW, FindNextChangeNotification,
};
use windows::Win32::System::Threading::WaitForSingleObject;
use windows::core::HSTRING;

/// Un aviso de cambios en una carpeta y sus subcarpetas.
pub struct VigiaCarpeta(HANDLE);

// SAFETY: el HANDLE de una notificacion de cambios se puede usar desde
// cualquier hilo (FindNextChangeNotification y WaitForSingleObject no estan
// atados al que lo creo), y solo lo cierra el Drop de su dueno.
unsafe impl Send for VigiaCarpeta {}

impl VigiaCarpeta {
    /// `None` si la carpeta no esta o Windows no deja vigilarla.
    pub fn nueva(carpeta: &Path) -> Option<VigiaCarpeta> {
        if !carpeta.is_dir() {
            return None;
        }
        // SAFETY: una ruta valida; el HANDLE se cierra en `drop`.
        let h = unsafe {
            FindFirstChangeNotificationW(
                &HSTRING::from(carpeta.as_os_str()),
                true,
                FILE_NOTIFY_CHANGE_FILE_NAME
                    | FILE_NOTIFY_CHANGE_LAST_WRITE
                    | FILE_NOTIFY_CHANGE_SIZE,
            )
        }
        .ok()?;
        Some(VigiaCarpeta(h))
    }

    /// Si cambio algo desde la ultima vez que se pregunto. No espera.
    pub fn cambio(&self) -> bool {
        // SAFETY: HANDLE vivo de `FindFirstChangeNotificationW`.
        unsafe {
            if WaitForSingleObject(self.0, 0) == WAIT_OBJECT_0 {
                let _ = FindNextChangeNotification(self.0);
                return true;
            }
        }
        false
    }

    /// Espera hasta que cambie algo o pase `tope`. `true` si cambio. Para
    /// un hilo que solo vive para vigilar (la impresora «PixPin»).
    pub fn esperar(&self, tope: std::time::Duration) -> bool {
        let ms = u32::try_from(tope.as_millis()).unwrap_or(u32::MAX - 1);
        // SAFETY: HANDLE vivo de `FindFirstChangeNotificationW`.
        unsafe {
            if WaitForSingleObject(self.0, ms) == WAIT_OBJECT_0 {
                let _ = FindNextChangeNotification(self.0);
                return true;
            }
        }
        false
    }
}

impl Drop for VigiaCarpeta {
    fn drop(&mut self) {
        // SAFETY: HANDLE de `FindFirstChangeNotificationW`, cerrado una vez.
        unsafe {
            let _ = FindCloseChangeNotification(self.0);
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn avisa_de_un_fichero_nuevo_y_no_antes() {
        let d = std::env::temp_dir().join(format!("pixpin-vigia-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("sub")).unwrap();
        let v = VigiaCarpeta::nueva(&d).unwrap();
        assert!(!v.cambio());
        std::fs::write(d.join("sub").join("x.txt"), "hola").unwrap();
        let mut visto = false;
        for _ in 0..50 {
            if v.cambio() {
                visto = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(visto);
        // Una carpeta que no esta: nada, sin panico.
        assert!(VigiaCarpeta::nueva(&d.join("no-esta")).is_none());
        let _ = std::fs::remove_dir_all(&d);
    }
}
