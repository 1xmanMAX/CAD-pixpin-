//! Salir en «Abrir con» de Windows para imagenes y videos.
//!
//! Se inscribe bajo `HKEY_CURRENT_USER\Software\Classes\Applications`, que
//! es el sitio donde un programa dice «se abrir esto» SIN reclamar ser el
//! que lo abre por defecto.
//!
//! Esa distincion es la que importa: quedarse con las extensiones de un
//! usuario a sus espaldas es de las cosas que mas molestan de un programa,
//! y ademas Windows 10 y 11 ya no dejan hacerlo en silencio — devuelven el
//! valor y ensenan un aviso diciendo que una aplicacion lo intento. Asi que
//! PixPin Max solo se ofrece, y elegirlo es cosa del usuario.
//!
//! Y siempre en HKEY_CURRENT_USER, nunca en HKEY_LOCAL_MACHINE: no hace
//! falta ser administrador y no se le toca nada a los demas usuarios del
//! equipo. Es la misma regla que sigue `arranque.rs`.

use std::path::Path;

use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
    RegCreateKeyExW, RegSetValueExW,
};
use windows::core::{HSTRING, PCWSTR};

/// Las extensiones que decimos saber abrir.
///
/// Imagenes y videos, que es lo que el pin sabe ensenar; el proyecto entero
/// del movil; y **los documentos que PixPin ya sabe leer**: el PDF (su
/// lector continuo, con marcadores y tinta) y el Word y el libro (el visor).
/// Antes no estaban porque el pin solo los ensenaba como una ficha con su
/// icono, y ofrecerse para algo que no se hace bien es peor que no
/// ofrecerse; ahora se leen de verdad. Texto, CSV y demas siguen fuera: su
/// programa de siempre los hace mejor.
pub const EXTENSIONES: &[&str] = &[
    ".png",
    ".jpg",
    ".jpeg",
    ".bmp",
    ".gif",
    ".webp",
    ".tif",
    ".tiff",
    ".mp4",
    ".mkv",
    ".avi",
    ".mov",
    ".webm",
    ".wmv",
    ".m4v",
    ".mpg",
    ".mpeg",
    ".ts",
    // Las fotos que lee Windows por debajo (`pixpin_codec::wic`): las del
    // iPhone, AVIF, JPEG XL... (lista entera en `asociaciones::IMAGENES`).
    ".jfif",
    ".ico",
    ".heic",
    ".heif",
    ".avif",
    ".jxl",
    ".jxr",
    // Los audios, que suenan en el reproductor flotante
    // (`asociaciones::AUDIOS`).
    ".mp3",
    ".m4a",
    ".wav",
    ".aac",
    ".flac",
    ".ogg",
    ".opus",
    ".wma",
    ".3gp",
    ".amr",
    // Y el proyecto entero del movil.
    ".pixpin", // Los documentos con lector propio.
    ".pdf",
    ".docx",
    ".epub",
    // Y el Markdown, que se abre en el editor de notas.
    ".md",
    ".markdown",
];

#[derive(Debug, thiserror::Error)]
pub enum ErrorAbrirCon {
    #[error("no se pudo escribir en el registro: {0}")]
    Registro(#[source] windows::core::Error),
}

/// Crea una clave y devuelve su handle. Quien llama la cierra.
fn crear(padre: HKEY, ruta: &str) -> Result<HKEY, ErrorAbrirCon> {
    let mut clave = HKEY::default();
    let nombre = HSTRING::from(ruta);
    // SAFETY: `nombre` vive durante la llamada; `clave` es local y se
    // rellena o se queda a cero.
    let r = unsafe {
        RegCreateKeyExW(
            padre,
            PCWSTR(nombre.as_ptr()),
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut clave,
            None,
        )
    };
    r.ok().map_err(ErrorAbrirCon::Registro)?;
    Ok(clave)
}

/// Escribe un valor de texto. `None` como nombre es el valor por defecto.
fn poner_texto(clave: HKEY, nombre: Option<&str>, valor: &str) -> Result<(), ErrorAbrirCon> {
    let n = nombre.map(HSTRING::from);
    let v = HSTRING::from(valor);
    // El tamano va en BYTES e INCLUYE el cero final. Sin contar el cero,
    // quien lea el valor sigue leyendo hasta topar con uno: es el fallo
    // clasico de escribir cadenas en el registro, y no se nota hasta que
    // alguien lee la clave y le sale basura pegada al final.
    //
    // SAFETY: la rodaja apunta al buffer de `v`, que es UTF-16 terminado en
    // cero, y `v` sigue viva hasta el final de la funcion.
    let bytes = unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, (v.len() + 1) * 2) };
    // SAFETY: `n` y `v` viven durante la llamada.
    let r = unsafe {
        RegSetValueExW(
            clave,
            n.as_ref()
                .map(|s| PCWSTR(s.as_ptr()))
                .unwrap_or(PCWSTR::null()),
            None,
            REG_SZ,
            Some(bytes),
        )
    };
    r.ok().map_err(ErrorAbrirCon::Registro)
}

/// La orden que Windows ejecutara al abrir un fichero con nosotros.
///
/// `"%1"` entre comillas y no `%1` a secas: sin ellas, una ruta con
/// espacios llegaria partida en varios argumentos y el fichero no se
/// encontraria. Es el fallo que hace que un programa funcione con
/// `foto.png` y no con `mi foto.png`.
/// La clave del registro donde vive nuestra inscripcion.
///
/// En un solo sitio a proposito: escrita dos veces, el dia que cambie una y
/// no la otra se estaria creando una clave y borrando otra distinta, y el
/// rastro quedaria para siempre sin que nadie se entere.
fn clave_de(nombre_exe: &str) -> String {
    format!(r"Software\Classes\Applications\{nombre_exe}")
}

pub fn orden_de_apertura(ruta_exe: &Path) -> String {
    format!("\"{}\" \"%1\"", ruta_exe.display())
}

/// Se inscribe como aplicacion capaz de abrir imagenes y videos.
///
/// Es idempotente: se puede llamar en cada arranque sin acumular nada. Un
/// fallo no puede impedir arrancar — quedarse sin salir en «Abrir con» es
/// una molestia, no un motivo para no funcionar.
pub fn inscribir(ruta_exe: &Path) -> Result<(), ErrorAbrirCon> {
    let Some(nombre_exe) = ruta_exe.file_name().and_then(|n| n.to_str()) else {
        return Ok(());
    };
    let base = clave_de(nombre_exe);

    let clave = crear(HKEY_CURRENT_USER, &format!(r"{base}\shell\open\command"))?;
    let escrito = poner_texto(clave, None, &orden_de_apertura(ruta_exe));
    // SAFETY: la clave viene de RegCreateKeyExW y se cierra una sola vez.
    unsafe {
        let _ = RegCloseKey(clave);
    }
    escrito?;

    // El nombre que se lee en la lista, en vez del del ejecutable.
    let app = crear(HKEY_CURRENT_USER, &base)?;
    let escrito = poner_texto(app, Some("FriendlyAppName"), "PixPin Max");
    // SAFETY: igual que arriba.
    unsafe {
        let _ = RegCloseKey(app);
    }
    escrito?;

    // Y que tipos aceptamos, para que Windows nos ofrezca SOLO con ellos.
    // Sin esto salimos en «Abrir con» de cualquier cosa, incluido un .exe o
    // un .zip.
    let tipos = crear(HKEY_CURRENT_USER, &format!(r"{base}\SupportedTypes"))?;
    let mut fallo = None;
    for extension in EXTENSIONES {
        if let Err(e) = poner_texto(tipos, Some(extension), "") {
            fallo = Some(e);
            break;
        }
    }
    // SAFETY: igual que arriba.
    unsafe {
        let _ = RegCloseKey(tipos);
    }
    match fallo {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// Borra la inscripcion, dejando el registro como estaba.
///
/// Existe por el modo portable. El proyecto promete no dejar rastro en el
/// equipo, y salir en «Abrir con» es lo unico que obliga a escribir en el
/// registro: sin una forma de deshacerlo, esa promesa se rompe para
/// siempre en cuanto alguien active esto una vez.
///
/// Que la clave no exista NO es un error: apagar algo que ya estaba
/// apagado es exito, no fallo.
pub fn desinscribir(ruta_exe: &Path) -> Result<(), ErrorAbrirCon> {
    use windows::Win32::System::Registry::RegDeleteTreeW;
    let Some(nombre_exe) = ruta_exe.file_name().and_then(|n| n.to_str()) else {
        return Ok(());
    };
    let ruta = HSTRING::from(clave_de(nombre_exe));
    // SAFETY: `ruta` vive durante la llamada. RegDeleteTreeW borra la clave
    // y todo lo que cuelga de ella; que no exista devuelve un codigo que se
    // ignora abajo.
    let r = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(ruta.as_ptr())) };
    if r.is_ok() || r == windows::Win32::Foundation::ERROR_FILE_NOT_FOUND {
        Ok(())
    } else {
        Err(ErrorAbrirCon::Registro(r.ok().unwrap_err()))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_ruta_va_entre_comillas() {
        // Sin las comillas, «C:\Mis fotos\a.png» llegaria partido en dos
        // argumentos y el fichero no se encontraria: el programa
        // funcionaria con `foto.png` y no con `mi foto.png`, que es de los
        // fallos mas confusos que puede tener.
        let orden = orden_de_apertura(Path::new(r"C:\Archivos de programa\PixPin\pixpinmax.exe"));
        assert!(orden.ends_with("\"%1\""), "{orden}");
        assert!(orden.starts_with('"'), "{orden}");
        assert!(orden.contains(r"Archivos de programa"));
        // Dos comillas para el ejecutable y dos para el argumento.
        assert_eq!(orden.matches('"').count(), 4);
    }

    #[test]
    fn solo_se_ofrece_lo_que_se_ensena_o_se_lee_de_verdad() {
        // Caso negativo: ofrecerse para abrir un ejecutable, un comprimido o
        // un texto seria ensuciar el menu del usuario con algo que no
        // hacemos mejor que su programa.
        for malo in [".exe", ".zip", ".txt", ".csv", ".doc"] {
            assert!(!EXTENSIONES.contains(&malo), "sobra {malo}");
        }
        for bueno in [
            ".png", ".mp4", ".pixpin", ".pdf", ".docx", ".epub", ".heic", ".mp3", ".m4a",
        ] {
            assert!(EXTENSIONES.contains(&bueno), "falta {bueno}");
        }
    }

    #[test]
    fn todo_lo_que_se_asocia_sale_tambien_en_abrir_con() {
        // Lo que PixPin pide ser predeterminada tiene que poder elegirse a
        // mano en «Abrir con»; al reves no hace falta (el PDF sale en «Abrir
        // con» sin pedir ser su predeterminada).
        let asociadas = crate::asociaciones::IMAGENES
            .iter()
            .chain(&crate::asociaciones::VIDEOS)
            .chain(&crate::asociaciones::AUDIOS);
        let faltan: Vec<String> = asociadas
            .map(|e| format!(".{e}"))
            .filter(|e| !EXTENSIONES.contains(&e.as_str()))
            .collect();
        // Caso negativo implicito: si alguien anade un tipo a `asociaciones`
        // y no aqui, esta lista deja de estar vacia.
        assert!(faltan.is_empty(), "faltan en Abrir con: {faltan:?}");
    }

    #[test]
    fn todas_las_extensiones_empiezan_por_punto_y_van_en_minusculas() {
        // Windows las quiere asi en SupportedTypes; sin el punto, la entrada
        // se guarda y no la mira nadie, que es el peor de los fallos porque
        // no se queja.
        for e in EXTENSIONES {
            assert!(e.starts_with('.'), "{e} sin punto");
            assert_eq!(&e.to_lowercase(), e, "{e} deberia ir en minusculas");
        }
    }

    #[test]
    fn la_clave_es_la_misma_al_crear_y_al_borrar() {
        // Si esta ruta se escribiera en dos sitios y una cambiara, se
        // crearia una clave y se borraria otra: el rastro se quedaria en el
        // registro para siempre sin que nadie se entere, y en modo portable
        // eso es romper la promesa del programa.
        let clave = clave_de("pixpinmax.exe");
        assert_eq!(clave, r"Software\Classes\Applications\pixpinmax.exe");
        // Con barras de verdad, no pegado: sin ellas seria una sola clave
        // con un nombre larguisimo colgando de la raiz, y borrarla no
        // limpiaria nada.
        assert_eq!(clave.matches('\\').count(), 3);
    }

    #[test]
    fn no_hay_extensiones_repetidas() {
        // Una repetida no rompe nada, pero es senal de que la lista se ha
        // editado a ciegas y conviene enterarse.
        let mut vistas: Vec<&str> = EXTENSIONES.to_vec();
        vistas.sort_unstable();
        let antes = vistas.len();
        vistas.dedup();
        assert_eq!(vistas.len(), antes, "hay extensiones repetidas");
    }
}
