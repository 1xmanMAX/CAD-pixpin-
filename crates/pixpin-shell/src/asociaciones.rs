//! **PixPin Max como app para abrir imagenes y videos** (el usuario, 3-oct:
//! «que pixpin sea la app predeterminada que abra imagenes y videos, todos en
//! formato de pin»).
//!
//! Y los audios (el usuario, 3-oct: «fotos de todos los formatos comunes,
//! videos y AUDIOS»), que suenan en el reproductor flotante sin abrir la app.
//!
//! Se registra en `HKCU` (sin permisos de administrador) como lo hace
//! cualquier visor: tres tipos propios (`PixPinMax.Imagen`,
//! `PixPinMax.Video` y `PixPinMax.Audio`) que abren con
//! `"pixpinmax.exe" "%1"`, la app en
//! «Abrir con» de cada extension y sus capacidades en
//! `RegisteredApplications`, que es lo que ensena Configuracion →
//! Aplicaciones predeterminadas.
//!
//! **Lo que no se hace: ponerse predeterminada sola.** Desde Windows 8 la
//! eleccion del usuario (`UserChoice`) va firmada y Windows la deshace si la
//! escribe un programa; la unica forma buena es que el usuario pulse
//! «Establecer como predeterminada» en Configuracion. [`abrir_configuracion`]
//! lleva ahi, a la pagina de PixPin Max.
//!
//! En modo portable no se toca, por lo mismo que el arranque con Windows
//! (`arranque.rs`): cero rastro en el equipo.

use std::path::Path;

use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
    RegCreateKeyExW, RegSetValueExW,
};
use windows::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify};
use windows::core::HSTRING;

/// El nombre con que sale en Configuracion.
pub const NOMBRE: &str = "PixPin Max";
const IMAGEN: &str = "PixPinMax.Imagen";
const VIDEO: &str = "PixPinMax.Video";
const AUDIO: &str = "PixPinMax.Audio";
const CAPACIDADES: &str = r"Software\PixPinMax\Capabilities";

/// Las imagenes que el pin abre (`pixpin_codec::imagen`: las de `image` y,
/// de reserva, las que lee Windows). HEIC/HEIF/AVIF/JPEG XL van aunque el
/// equipo no tenga su extension de la tienda: abrirlas sin ella saca un pin
/// que dice cual instalar, que es mejor que el «no se puede abrir» del Visor.
pub const IMAGENES: [&str; 15] = [
    "png", "jpg", "jpeg", "jfif", "gif", "bmp", "webp", "tif", "tiff", "ico", "heic", "heif",
    "avif", "jxl", "jxr",
];
/// Los videos que el pin reproduce (`pixpin_pin::contenido`).
pub const VIDEOS: [&str; 10] = [
    "mp4", "mkv", "webm", "mov", "avi", "m4v", "wmv", "mpg", "mpeg", "ts",
];
/// Los audios que suenan en el reproductor flotante (los mismos que el
/// plugin de Flow Launcher, `pixpin_lanzador::resultados::es_audio`).
pub const AUDIOS: [&str; 10] = [
    "mp3", "m4a", "wav", "aac", "flac", "ogg", "opus", "wma", "3gp", "amr",
];

/// Si `ruta` es un audio de [`AUDIOS`], por su extension (sin tocar disco).
pub fn es_audio(ruta: &Path) -> bool {
    ruta.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| AUDIOS.iter().any(|a| a.eq_ignore_ascii_case(e)))
}

/// Los tres tipos y sus extensiones, en un solo sitio para escribir y borrar.
const TIPOS: [(&str, &str, &[&str]); 3] = [
    (IMAGEN, "Imagen (PixPin Max)", &IMAGENES),
    (VIDEO, "Video (PixPin Max)", &VIDEOS),
    (AUDIO, "Audio (PixPin Max)", &AUDIOS),
];

/// Una clave abierta (o creada) para escribir; se cierra al soltarla.
struct Clave(HKEY);

impl Clave {
    fn crear(ruta: &str) -> windows::core::Result<Clave> {
        let mut h = HKEY::default();
        // SAFETY: `ruta` vive en un HSTRING propio durante la llamada y `h` es
        // una variable propia que la llamada rellena si sale bien.
        unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                &HSTRING::from(ruta),
                None,
                None,
                REG_OPTION_NON_VOLATILE,
                KEY_WRITE,
                None,
                &mut h,
                None,
            )
            .ok()?;
        }
        Ok(Clave(h))
    }

    /// Escribe un texto; `nombre` vacio es el valor «(predeterminado)».
    fn texto(&self, nombre: &str, valor: &str) -> windows::core::Result<()> {
        let v = HSTRING::from(valor);
        // SAFETY: el buffer UTF-16 de `v` sigue vivo hasta el final de la
        // funcion y `HSTRING` pone un cero tras `len()` unidades, asi que
        // `(len + 1) * 2` bytes caen dentro de lo inicializado.
        let bytes: &[u8] =
            unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, (v.len() + 1) * 2) };
        let n = HSTRING::from(nombre);
        // SAFETY: `self.0` es una clave abierta para escribir; `n` y `bytes`
        // viven durante la llamada.
        unsafe { RegSetValueExW(self.0, &n, None, REG_SZ, Some(bytes)).ok() }
    }
}

impl Drop for Clave {
    fn drop(&mut self) {
        // SAFETY: la clave la abrio `crear` y solo se cierra aqui, una vez.
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

/// Lo que se escribe, como pares (clave, valor, texto): puro para poder
/// probarlo sin tocar el registro.
pub fn entradas(exe: &Path) -> Vec<(String, String, String)> {
    let exe = exe.display().to_string();
    let abrir = format!("\"{exe}\" \"%1\"");
    let mut v = Vec::new();
    let mut e = |clave: &str, valor: &str, texto: &str| {
        v.push((clave.to_string(), valor.to_string(), texto.to_string()));
    };
    for (tipo, nombre, _) in TIPOS {
        let base = format!(r"Software\Classes\{tipo}");
        e(&base, "", nombre);
        e(&format!(r"{base}\DefaultIcon"), "", &format!("\"{exe}\",0"));
        e(&format!(r"{base}\shell\open\command"), "", &abrir);
    }
    e(CAPACIDADES, "ApplicationName", NOMBRE);
    e(
        CAPACIDADES,
        "ApplicationDescription",
        "Abre imagenes y videos como pines flotantes y los audios en un reproductor flotante",
    );
    for (tipo, _, exts) in TIPOS {
        for ext in exts {
            e(
                &format!(r"Software\Classes\.{ext}\OpenWithProgids"),
                tipo,
                "",
            );
            e(
                &format!(r"{CAPACIDADES}\FileAssociations"),
                &format!(".{ext}"),
                tipo,
            );
        }
    }
    e(r"Software\RegisteredApplications", NOMBRE, CAPACIDADES);
    v
}

/// Registra PixPin Max para abrir imagenes y videos con `exe`. Se puede
/// llamar cada vez que arranca: escribe lo mismo, y si el .exe se movio deja
/// la ruta nueva.
pub fn registrar(exe: &Path) -> windows::core::Result<()> {
    for (clave, valor, texto) in entradas(exe) {
        Clave::crear(&clave)?.texto(&valor, &texto)?;
    }
    // Que el Explorador se entere sin cerrar sesion.
    // SAFETY: sin punteros: avisa de un cambio global de asociaciones.
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };
    Ok(())
}

/// Borra lo que escribio [`registrar`]: con «Abrir con» apagado en Ajustes no
/// puede quedar rastro (la misma regla que `abrir_con::desinscribir`). Lo que
/// ya no esta no es un fallo.
pub fn quitar() {
    use windows::Win32::System::Registry::{RegDeleteKeyValueW, RegDeleteTreeW};
    let arbol = |ruta: &str| {
        // SAFETY: la ruta vive en un HSTRING propio durante la llamada.
        let _ = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, &HSTRING::from(ruta)) };
    };
    let valor = |ruta: &str, nombre: &str| {
        // SAFETY: las dos cadenas viven en HSTRING propios durante la llamada.
        let _ = unsafe {
            RegDeleteKeyValueW(
                HKEY_CURRENT_USER,
                &HSTRING::from(ruta),
                &HSTRING::from(nombre),
            )
        };
    };
    for (tipo, _, _) in TIPOS {
        arbol(&format!(r"Software\Classes\{tipo}"));
    }
    for (tipo, _, exts) in TIPOS {
        for ext in exts {
            valor(&format!(r"Software\Classes\.{ext}\OpenWithProgids"), tipo);
        }
    }
    arbol(r"Software\PixPinMax");
    valor(r"Software\RegisteredApplications", NOMBRE);
    // SAFETY: sin punteros: avisa de un cambio global de asociaciones.
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };
}

/// Abre Configuracion en la pagina de PixPin Max, donde el usuario la hace
/// predeterminada con un boton.
pub fn abrir_configuracion() {
    let uri = format!(
        "ms-settings:defaultapps?registeredAppUser={}",
        NOMBRE.replace(' ', "%20")
    );
    if let Err(e) = crate::abrir::abrir(Path::new(&uri)) {
        tracing::warn!(?e, "no se pudo abrir Configuracion de apps predeterminadas");
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn abre_con_el_exe_y_cubre_imagenes_y_videos() {
        let v = entradas(Path::new(r"C:\Apps\PixPinMax\pixpinmax.exe"));
        let valor = |clave: &str, nombre: &str| {
            v.iter()
                .find(|(c, n, _)| c == clave && n == nombre)
                .map(|(_, _, t)| t.as_str())
        };
        assert_eq!(
            valor(r"Software\Classes\PixPinMax.Imagen\shell\open\command", ""),
            Some(r#""C:\Apps\PixPinMax\pixpinmax.exe" "%1""#)
        );
        assert_eq!(
            valor(&format!(r"{CAPACIDADES}\FileAssociations"), ".png"),
            Some(IMAGEN)
        );
        assert_eq!(
            valor(&format!(r"{CAPACIDADES}\FileAssociations"), ".mp4"),
            Some(VIDEO)
        );
        assert_eq!(
            valor(r"Software\RegisteredApplications", NOMBRE),
            Some(CAPACIDADES)
        );
        // Caso negativo: un PDF o un texto no se toman (tienen su programa).
        assert_eq!(
            valor(&format!(r"{CAPACIDADES}\FileAssociations"), ".pdf"),
            None
        );
        assert_eq!(
            valor(&format!(r"{CAPACIDADES}\FileAssociations"), ".txt"),
            None
        );
    }

    #[test]
    fn los_audios_tienen_su_tipo_y_las_fotos_nuevas_van_con_las_imagenes() {
        let v = entradas(Path::new(r"C:\Apps\PixPinMax\pixpinmax.exe"));
        let valor = |clave: &str, nombre: &str| {
            v.iter()
                .find(|(c, n, _)| c == clave && n == nombre)
                .map(|(_, _, t)| t.clone())
        };
        let asociado = |ext: &str| valor(&format!(r"{CAPACIDADES}\FileAssociations"), ext);
        for a in [".mp3", ".m4a", ".wav", ".flac", ".ogg", ".opus", ".amr"] {
            assert_eq!(asociado(a).as_deref(), Some(AUDIO), "{a}");
            assert_eq!(
                valor(&format!(r"Software\Classes\{a}\OpenWithProgids"), AUDIO).as_deref(),
                Some(""),
                "{a} en Abrir con"
            );
        }
        assert_eq!(
            valor(r"Software\Classes\PixPinMax.Audio\shell\open\command", "").as_deref(),
            Some(r#""C:\Apps\PixPinMax\pixpinmax.exe" "%1""#)
        );
        for f in [".heic", ".heif", ".avif", ".jfif", ".jxl"] {
            assert_eq!(asociado(f).as_deref(), Some(IMAGEN), "{f}");
        }
        // Caso negativo: un audio no cae en el tipo de video ni al reves, y
        // un .mid (no lo reproduce) no se toma.
        assert_eq!(asociado(".mp4").as_deref(), Some(VIDEO));
        assert_ne!(asociado(".mp3").as_deref(), Some(VIDEO));
        assert_eq!(asociado(".mid"), None);
    }

    #[test]
    fn es_audio_por_la_extension_sin_mirar_mayusculas() {
        assert!(es_audio(Path::new(r"C:\musica\Cancion.MP3")));
        assert!(es_audio(Path::new("nota de voz.m4a")));
        // Caso negativo: ni un video ni algo sin extension.
        assert!(!es_audio(Path::new("clip.mp4")));
        assert!(!es_audio(Path::new("mp3")));
    }

    #[test]
    fn ninguna_extension_esta_en_dos_tipos() {
        // Una extension en dos tipos dejaria a Windows elegir uno al azar.
        let todas: Vec<&str> = TIPOS
            .iter()
            .flat_map(|(_, _, e)| e.iter().copied())
            .collect();
        let mut unicas = todas.clone();
        unicas.sort_unstable();
        unicas.dedup();
        assert_eq!(unicas.len(), todas.len());
    }
}
