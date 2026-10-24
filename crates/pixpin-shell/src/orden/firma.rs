//! **Los enlaces firmados**: los que hace esta misma PixPin.
//!
//! Un enlace `pixpin://` lo puede lanzar cualquier pagina web (el navegador
//! pregunta antes, pero pregunta «¿abrir PixPin?», no «¿borrar tu lista?»),
//! y por eso lo que no es inofensivo pide confirmacion (`orden::seguridad`).
//! Pero los botones de los avisos de la propia app («Hecha», «+10 min»,
//! «Conservar») tambien son enlaces, y preguntarle al usuario si de verdad
//! quiere lo que acaba de pulsar en su propio aviso seria absurdo.
//!
//! Asi que la app firma los suyos: HMAC-SHA256 con una clave de 32 bytes
//! al azar que vive en `%LOCALAPPDATA%\PixPinMax\enlaces.clave` y que una
//! web no puede conocer. Lo que se firma no es el texto del enlace sino su
//! forma canonica (`PartesEnlace::canonico`: ya descodificado y con los
//! campos en orden), para que un programa que reescriba el enlace por el
//! camino (`%20` por `+`, una barra final) no lo invalide.
//!
//! Un programa del propio equipo podria leer la clave, si; pero ese ya
//! puede mandar pedidos por `WM_COPYDATA` sin preguntar a nadie: la firma
//! protege de lo que viene de fuera, no del propio usuario.

use std::path::PathBuf;

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// El nombre del campo que lleva la firma.
pub const CAMPO: &str = "firma";

/// Donde vive la clave. `LOCALAPPDATA` y no `APPDATA`: es de este equipo y
/// no debe viajar con un perfil movil.
pub fn ruta_de_la_clave() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(|d| PathBuf::from(d).join("PixPinMax").join("enlaces.clave"))
}

/// La firma de un texto canonico con una clave, en hexadecimal.
pub fn firma_con(clave: &[u8], canonico: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(clave).expect("HMAC acepta claves de cualquier largo");
    mac.update(canonico.as_bytes());
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Si `firma` (hex) es la de `canonico` con esa clave. En tiempo constante
/// (`verify_slice`): comparar texto con `==` diria por cuanto se fallo.
pub fn verificar_con(clave: &[u8], canonico: &str, firma: &str) -> bool {
    let Some(bytes) = de_hex(firma) else {
        return false;
    };
    let mut mac = HmacSha256::new_from_slice(clave).expect("HMAC acepta claves de cualquier largo");
    mac.update(canonico.as_bytes());
    mac.verify_slice(&bytes).is_ok()
}

fn de_hex(t: &str) -> Option<Vec<u8>> {
    if t.len() != 64 || !t.is_ascii() {
        return None;
    }
    (0..t.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&t[i..i + 2], 16).ok())
        .collect()
}

/// Si la firma vale con la clave de este equipo. Sin clave (nunca se firmo
/// nada aqui) no vale ninguna.
pub fn verificar(canonico: &str, firma: &str) -> bool {
    let Some(ruta) = ruta_de_la_clave() else {
        return false;
    };
    match std::fs::read(&ruta) {
        Ok(clave) if clave.len() == 32 => verificar_con(&clave, canonico, firma),
        _ => false,
    }
}

/// La clave de este equipo; la crea la primera vez.
pub fn clave() -> std::io::Result<Vec<u8>> {
    let ruta = ruta_de_la_clave().ok_or_else(|| std::io::Error::other("sin LOCALAPPDATA"))?;
    if let Ok(c) = std::fs::read(&ruta)
        && c.len() == 32
    {
        return Ok(c);
    }
    if let Some(d) = ruta.parent() {
        std::fs::create_dir_all(d)?;
    }
    let nueva = al_azar()?;
    // `create_new`: si otra copia la escribio a la vez, gana la suya y se lee.
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&ruta)
    {
        Ok(mut f) => {
            use std::io::Write;
            f.write_all(&nueva)?;
            Ok(nueva.to_vec())
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let c = std::fs::read(&ruta)?;
            if c.len() == 32 {
                Ok(c)
            } else {
                // Una clave rota (cortada a medias) se rehace: los enlaces
                // firmados con ella, de todas formas, ya no valdrian.
                std::fs::write(&ruta, nueva)?;
                Ok(nueva.to_vec())
            }
        }
        Err(e) => Err(e),
    }
}

fn al_azar() -> std::io::Result<[u8; 32]> {
    use windows::Win32::Security::Cryptography::{
        BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom,
    };
    let mut b = [0u8; 32];
    // SAFETY: el buffer es nuestro y vive durante la llamada; sin algoritmo,
    // con la bandera del generador del sistema, es el uso documentado.
    let r = unsafe { BCryptGenRandom(None, &mut b, BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
    if r.is_ok() {
        Ok(b)
    } else {
        Err(std::io::Error::other(format!("BCryptGenRandom: {r:?}")))
    }
}

/// **Firma un enlace** con la clave de este equipo: el mismo enlace con
/// `&firma=…` al final. Es lo que usan los avisos con botones para que su
/// enlace se haga sin preguntar.
pub fn firmar(url: &str) -> std::io::Result<String> {
    let clave = clave()?;
    firmar_con(&clave, url).map_err(|f| std::io::Error::other(f.motivo))
}

/// [`firmar`] con una clave dada (las pruebas).
pub fn firmar_con(clave: &[u8], url: &str) -> Result<String, super::Fallo> {
    let partes = super::partir_enlace(url)?;
    let f = firma_con(clave, &partes.canonico());
    // Si ya llevaba una firma (vieja), fuera: la buena es la nueva.
    let base = match url.find("firma=") {
        Some(i) if i > 0 && matches!(url.as_bytes()[i - 1], b'?' | b'&') => {
            url[..i - 1].to_string()
        }
        _ => url.to_string(),
    };
    let sep = if base.contains('?') { '&' } else { '?' };
    Ok(format!("{base}{sep}{CAMPO}={f}"))
}
