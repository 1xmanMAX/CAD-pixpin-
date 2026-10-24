//! **Subir un fichero por HTTPS como formulario** (`multipart/form-data`),
//! con lo que ya trae Windows (WinHTTP), para «Enlace» de la hoja de
//! compartir (8-oct-2026).
//!
//! Es el `SubirArchivo.subir` del movil: los campos del formulario, el
//! fichero detras, y lo que conteste el servicio. Como alli, el fichero se
//! manda a trozos con el largo dicho por delante (`setFixedLengthStreamingMode`
//! del movil; aqui `WinHttpSendRequest` con el total y `WinHttpWriteData`):
//! un PDF de cincuenta megas no se carga entero en memoria antes de salir.
//!
//! No sabe de servicios: eso lo decide quien llama (`compartir::enlace`).
//! Este modulo habla con el sistema operativo: cada `unsafe` lleva su
//! `// SAFETY:`.

use std::ffi::c_void;
use std::io::Read;
use std::path::Path;

use windows::Win32::Networking::WinHttp::{
    INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
    WINHTTP_ACCESS_TYPE_DEFAULT_PROXY, WINHTTP_FLAG_SECURE, WINHTTP_OPEN_REQUEST_FLAGS,
    WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE, WinHttpCloseHandle, WinHttpConnect,
    WinHttpOpen, WinHttpOpenRequest, WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse,
    WinHttpSendRequest, WinHttpSetTimeouts, WinHttpWriteData,
};
use windows::core::{HSTRING, PCWSTR, w};

/// Un asa de WinHTTP que se cierra sola.
struct Asa(*mut c_void);

impl Drop for Asa {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: el asa la devolvio WinHTTP, es de este `Asa` y no se
            // ha cerrado antes.
            let _ = unsafe { WinHttpCloseHandle(self.0) };
        }
    }
}

/// Lo que dijo el servidor: su codigo y lo que escribio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Respuesta {
    pub codigo: u32,
    pub cuerpo: String,
}

/// La cabeza y la cola del formulario, alrededor del fichero. Pura, para
/// probarla sin red.
pub fn formulario(
    frontera: &str,
    campos: &[(&str, &str)],
    campo_fichero: &str,
    nombre: &str,
    tipo: &str,
) -> (Vec<u8>, Vec<u8>) {
    let mut cabeza = String::new();
    for (k, v) in campos {
        cabeza.push_str(&format!(
            "--{frontera}\r\nContent-Disposition: form-data; name=\"{k}\"\r\n\r\n{v}\r\n"
        ));
    }
    cabeza.push_str(&format!(
        "--{frontera}\r\nContent-Disposition: form-data; name=\"{campo_fichero}\"; filename=\"{nombre}\"\r\nContent-Type: {}\r\n\r\n",
        if tipo.is_empty() {
            "application/octet-stream"
        } else {
            tipo
        }
    ));
    (
        cabeza.into_bytes(),
        format!("\r\n--{frontera}--\r\n").into_bytes(),
    )
}

/// `https://servidor/ruta` en sus dos piezas. Solo HTTPS.
fn partir_url(url: &str) -> Option<(&str, &str)> {
    let resto = url.strip_prefix("https://")?;
    let (servidor, ruta) = match resto.find('/') {
        Some(i) => (&resto[..i], &resto[i..]),
        None => (resto, "/"),
    };
    (!servidor.is_empty() && !servidor.contains(':')).then_some((servidor, ruta))
}

/// **Sube `fichero`** a `url` como el campo `campo_fichero` del formulario,
/// con `campos` delante. Bloquea: llamarlo desde un hilo de fondo. `avance`
/// va de 0 a 1 con lo que lleva mandado.
#[allow(clippy::too_many_arguments)] // el formulario entero, por piezas
pub fn subir(
    url: &str,
    campos: &[(&str, &str)],
    campo_fichero: &str,
    fichero: &Path,
    nombre: &str,
    tipo: &str,
    avance: &mut dyn FnMut(f32),
) -> Result<Respuesta, String> {
    let (servidor, ruta) = partir_url(url).ok_or_else(|| format!("direccion no valida: {url}"))?;
    let largo = std::fs::metadata(fichero)
        .map_err(|e| format!("no se pudo leer el fichero: {e}"))?
        .len();
    let frontera = format!(
        "----pixpin{:x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let (cabeza, cola) = formulario(&frontera, campos, campo_fichero, nombre, tipo);
    let total = cabeza.len() as u64 + largo + cola.len() as u64;
    if total > u32::MAX as u64 {
        return Err("pesa demasiado para subirlo de una vez".into());
    }
    let mut entrada =
        std::fs::File::open(fichero).map_err(|e| format!("no se pudo abrir el fichero: {e}"))?;
    let cabeceras = HSTRING::from(format!(
        "Content-Type: multipart/form-data; boundary={frontera}\r\n"
    ));
    let error = |que: &str| format!("{que}: {}", windows::core::Error::from_thread().message());
    // SAFETY: todas las cadenas son literales o `HSTRING` vivos durante cada
    // llamada; cada asa se envuelve en `Asa` en cuanto existe, asi que se
    // cierra en todos los caminos y en orden inverso (peticion, conexion,
    // sesion). Los bufferes que se pasan a WinHTTP viven mientras dura cada
    // llamada, que es sincrona.
    unsafe {
        let mut sesion = Asa(WinHttpOpen(
            w!("PixPin"),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        ));
        if sesion.0.is_null() {
            sesion = Asa(WinHttpOpen(
                w!("PixPin"),
                WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                PCWSTR::null(),
                PCWSTR::null(),
                0,
            ));
        }
        if sesion.0.is_null() {
            return Err(error("sin conexion"));
        }
        // Como el movil: 20 s para conectar, 120 s esperando la respuesta.
        let _ = WinHttpSetTimeouts(sesion.0, 20_000, 20_000, 120_000, 120_000);
        let conexion = Asa(WinHttpConnect(
            sesion.0,
            &HSTRING::from(servidor),
            INTERNET_DEFAULT_HTTPS_PORT,
            0,
        ));
        if conexion.0.is_null() {
            return Err(error("no se pudo conectar"));
        }
        let peticion = Asa(WinHttpOpenRequest(
            conexion.0,
            w!("POST"),
            &HSTRING::from(ruta),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            WINHTTP_OPEN_REQUEST_FLAGS(WINHTTP_FLAG_SECURE.0),
        ));
        if peticion.0.is_null() {
            return Err(error("no se pudo preparar la subida"));
        }
        WinHttpSendRequest(peticion.0, Some(&cabeceras), None, 0, total as u32, 0)
            .map_err(|e| format!("no se pudo empezar a subir: {}", e.message()))?;
        let mandar = |trozo: &[u8]| -> Result<(), String> {
            let mut hecho = 0usize;
            while hecho < trozo.len() {
                let mut n: u32 = 0;
                WinHttpWriteData(
                    peticion.0,
                    Some(trozo[hecho..].as_ptr().cast()),
                    (trozo.len() - hecho) as u32,
                    &mut n,
                )
                .map_err(|e| format!("se corto la subida: {}", e.message()))?;
                if n == 0 {
                    return Err("se corto la subida".into());
                }
                hecho += n as usize;
            }
            Ok(())
        };
        mandar(&cabeza)?;
        let mut puestos = cabeza.len() as u64;
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = entrada
                .read(&mut buf)
                .map_err(|e| format!("no se pudo leer el fichero: {e}"))?;
            if n == 0 {
                break;
            }
            mandar(&buf[..n])?;
            puestos += n as u64;
            avance((puestos as f32 / total as f32).min(0.99));
        }
        mandar(&cola)?;
        WinHttpReceiveResponse(peticion.0, std::ptr::null_mut())
            .map_err(|e| format!("el servicio no contesto: {}", e.message()))?;
        let mut codigo: u32 = 0;
        let mut largo_codigo = std::mem::size_of::<u32>() as u32;
        let _ = WinHttpQueryHeaders(
            peticion.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some((&mut codigo as *mut u32).cast()),
            &mut largo_codigo,
            std::ptr::null_mut(),
        );
        let mut cuerpo = Vec::new();
        loop {
            let mut n: u32 = 0;
            if WinHttpReadData(
                peticion.0,
                buf.as_mut_ptr().cast(),
                buf.len() as u32,
                &mut n,
            )
            .is_err()
                || n == 0
            {
                break;
            }
            cuerpo.extend_from_slice(&buf[..n as usize]);
            // Un enlace no ocupa mas: lo que pase de aqui es una pagina de
            // error entera, y con el principio basta para decir que paso.
            if cuerpo.len() > 64 * 1024 {
                break;
            }
        }
        avance(1.0);
        Ok(Respuesta {
            codigo,
            cuerpo: String::from_utf8_lossy(&cuerpo).into_owned(),
        })
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_formulario_lleva_los_campos_y_el_fichero_con_su_nombre_y_tipo() {
        let (cabeza, cola) = formulario(
            "XX",
            &[("reqtype", "fileupload"), ("time", "72h")],
            "fileToUpload",
            "lista.pdf",
            "application/pdf",
        );
        let c = String::from_utf8(cabeza).unwrap();
        assert!(c.starts_with(
            "--XX\r\nContent-Disposition: form-data; name=\"reqtype\"\r\n\r\nfileupload\r\n"
        ));
        assert!(c.contains("name=\"time\"\r\n\r\n72h\r\n"));
        assert!(c.ends_with(
            "name=\"fileToUpload\"; filename=\"lista.pdf\"\r\nContent-Type: application/pdf\r\n\r\n"
        ));
        assert_eq!(String::from_utf8(cola).unwrap(), "\r\n--XX--\r\n");
        // Caso negativo: sin tipo, el generico.
        let (c, _) = formulario("XX", &[], "file", "x", "");
        assert!(
            String::from_utf8(c)
                .unwrap()
                .contains("application/octet-stream")
        );
    }

    #[test]
    fn solo_https_y_sin_puerto() {
        assert_eq!(
            partir_url("https://litterbox.catbox.moe/resources/internals/api.php"),
            Some(("litterbox.catbox.moe", "/resources/internals/api.php"))
        );
        assert_eq!(partir_url("http://temp.sh/upload"), None);
        assert_eq!(partir_url("https://x:8080/a"), None);
    }
}
