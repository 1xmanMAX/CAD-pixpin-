//! Lo que hay que preguntarle al sistema antes de arrancar.
//!
//! Este modulo existe para que los crates con `forbid(unsafe_code)` no tengan
//! que llamar al sistema: reciben estos valores ya resueltos como parametros.
//!
//! En Windows cada respuesta sale de Win32, como siempre. Fuera de Windows
//! (Mac, iPhone/iPad, el Linux de la CI) va una version portable: la app de
//! Apple podra pasar sus propios valores (el tema y la region de macOS) por
//! encima, pero los crates de logica ya compilan y se prueban sin Windows.
//!
//! (`posicion_del_cursor` sigue en `pixpin_shell::entorno`: es del
//! escritorio, no de la logica.)

use std::path::PathBuf;

/// Directorio donde vive el ejecutable (`pixpinmax.exe` en Windows).
pub fn directorio_del_ejecutable() -> std::io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    exe.parent()
        .map(PathBuf::from)
        .ok_or_else(|| std::io::Error::other("el ejecutable no tiene directorio padre"))
}

/// La carpeta de datos del usuario: `%APPDATA%` (la itinerante) en Windows.
///
/// Se usa `SHGetKnownFolderPath` y no la variable de entorno porque la
/// variable se puede manipular y no siempre esta presente en sesiones de
/// servicio.
#[cfg(windows)]
pub fn appdata() -> std::io::Result<PathBuf> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{
        FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, SHGetKnownFolderPath,
    };
    use windows::core::PWSTR;

    // SAFETY: SHGetKnownFolderPath devuelve un puntero a cadena UTF-16
    // terminada en cero que hay que liberar con CoTaskMemFree exactamente una
    // vez. Por eso la conversion a String se hace primero y se guarda en una
    // variable local (sin propagar el error todavia): CoTaskMemFree se llama
    // despues, en todos los caminos, tanto si la conversion salio bien como
    // si no. Tras liberar no queda ninguna referencia viva al puntero. Solo
    // estas tres llamadas son realmente inseguras; decidir si propagar el
    // error de la conversion es logica normal y vive fuera del bloque.
    let convertida = unsafe {
        let ruta: PWSTR = SHGetKnownFolderPath(&FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, None)
            .map_err(|e| std::io::Error::other(format!("SHGetKnownFolderPath fallo: {e}")))?;
        let convertida = ruta.to_string();
        CoTaskMemFree(Some(ruta.0 as *const _));
        convertida
    };
    let texto = convertida.map_err(std::io::Error::other)?;
    Ok(PathBuf::from(texto))
}

/// La carpeta de datos del usuario: `~/Library/Application Support` en Mac
/// (dentro del contenedor de la app en iPhone/iPad, que es su `HOME`) y
/// `$XDG_DATA_HOME` o `~/.local/share` en Linux.
#[cfg(not(windows))]
pub fn appdata() -> std::io::Result<PathBuf> {
    let casa = || {
        std::env::var_os("HOME")
            .filter(|h| !h.is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| std::io::Error::other("no hay HOME"))
    };
    if cfg!(target_vendor = "apple") {
        return Ok(casa()?.join("Library").join("Application Support"));
    }
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME").filter(|x| !x.is_empty()) {
        return Ok(PathBuf::from(xdg));
    }
    Ok(casa()?.join(".local").join("share"))
}

/// Etiqueta de idioma del usuario, por ejemplo `es-ES`.
///
/// Si Windows no la devuelve se asume `en-US`: es preferible una interfaz en
/// ingles a no arrancar.
#[cfg(windows)]
pub fn locale_del_sistema() -> String {
    use windows::Win32::Globalization::GetUserDefaultLocaleName;
    const MAX: usize = 85; // LOCALE_NAME_MAX_LENGTH
    let mut buffer = [0u16; MAX];

    // SAFETY: se pasa un buffer propio de tamaño conocido y la funcion
    // devuelve cuantos u16 escribio, incluido el cero final.
    let escritos = unsafe { GetUserDefaultLocaleName(&mut buffer) };

    if escritos <= 1 {
        return "en-US".to_string();
    }
    String::from_utf16_lossy(&buffer[..(escritos as usize - 1)])
}

/// Etiqueta de idioma del usuario, sacada de `LC_ALL`/`LC_MESSAGES`/`LANG`
/// (`es_ES.UTF-8` → `es-ES`). Sin nada util, `en-US`, como en Windows.
#[cfg(not(windows))]
pub fn locale_del_sistema() -> String {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .find_map(|v| etiqueta_de_posix(&v))
        .unwrap_or_else(|| "en-US".to_string())
}

/// `es_ES.UTF-8@euro` → `es-ES`. `C` y `POSIX` no son ningun idioma.
#[cfg_attr(windows, allow(dead_code))]
fn etiqueta_de_posix(valor: &str) -> Option<String> {
    let base = valor.split(['.', '@']).next()?.trim();
    if base.is_empty() || base == "C" || base == "POSIX" {
        return None;
    }
    let etiqueta = base.replace('_', "-");
    etiqueta
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-')
        .then_some(etiqueta)
}

/// **El separador decimal** que usa este usuario (Configuracion regional
/// de Windows, `LOCALE_SDECIMAL`): `,` en `es-ES`, `.` en `en-US` o
/// `es-PE`. Lo que la tabla ensena de una formula va con el mismo que lo
/// que el usuario escribe a mano. Si el sistema no dice nada, el punto.
pub fn separador_decimal() -> char {
    separador_decimal_de(None)
}

/// Lo mismo para una region concreta (`"es-ES"`); `None`, la del usuario.
#[cfg(windows)]
pub fn separador_decimal_de(region: Option<&str>) -> char {
    use windows::Win32::Globalization::{GetLocaleInfoEx, LOCALE_SDECIMAL};
    let nombre = region.map(windows::core::HSTRING::from);
    let mut buffer = [0u16; 8];
    // SAFETY: buffer propio de tamano conocido; el nombre (o NULL, la
    // region del usuario) vive durante la llamada.
    let escritos = unsafe {
        match &nombre {
            Some(n) => GetLocaleInfoEx(n, LOCALE_SDECIMAL, Some(&mut buffer)),
            None => GetLocaleInfoEx(
                windows::core::PCWSTR::null(),
                LOCALE_SDECIMAL,
                Some(&mut buffer),
            ),
        }
    };
    if escritos <= 1 {
        return '.';
    }
    match String::from_utf16_lossy(&buffer[..escritos as usize - 1])
        .chars()
        .next()
    {
        // Solo los dos que se entienden al volver a leer un numero.
        Some(',') => ',',
        _ => '.',
    }
}

/// Lo mismo sin Windows: una tabla corta de CLDR, por idioma y con las
/// regiones que se apartan de su idioma (Mexico, Peru y Centroamerica
/// escriben `1.5` aunque Espana escriba `1,5`). Es el respaldo: la app de
/// Apple le pasara el de `Locale.current.decimalSeparator`.
#[cfg(not(windows))]
pub fn separador_decimal_de(region: Option<&str>) -> char {
    let etiqueta = match region {
        Some(r) => r.to_string(),
        None => locale_del_sistema(),
    };
    separador_de_etiqueta(&etiqueta)
}

#[cfg_attr(windows, allow(dead_code))]
fn separador_de_etiqueta(etiqueta: &str) -> char {
    // Paises que usan el punto aunque su idioma, en general, use la coma.
    const PAISES_CON_PUNTO: &[&str] = &[
        "MX", "PE", "GT", "HN", "NI", "PA", "SV", "DO", "PR", "US", "GB", "IE", "AU", "NZ", "CH",
        "LI",
    ];
    // Idiomas que, sin mas, escriben la coma decimal.
    const IDIOMAS_CON_COMA: &[&str] = &[
        "es", "pt", "fr", "de", "it", "nl", "ru", "pl", "tr", "sv", "da", "nb", "nn", "no", "fi",
        "cs", "sk", "hu", "ro", "bg", "el", "uk", "hr", "sl", "sr", "ca", "eu", "gl", "id", "vi",
        "lt", "lv", "et",
    ];
    let mut partes = etiqueta.split(['-', '_']);
    let idioma = partes.next().unwrap_or("").to_ascii_lowercase();
    let pais = partes
        .find(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_alphabetic()))
        .map(|p| p.to_ascii_uppercase());
    if pais
        .as_deref()
        .is_some_and(|p| PAISES_CON_PUNTO.contains(&p))
    {
        return '.';
    }
    if IDIOMAS_CON_COMA.contains(&idioma.as_str()) {
        ','
    } else {
        '.'
    }
}

/// `true` si Windows esta en tema claro para las aplicaciones.
///
/// Decide el lienzo de las notas y las fichas (D30). Si la clave no existe
/// —Windows anteriores a la opcion, o una politica que la borro— se asume
/// claro, que es el aspecto por defecto del sistema.
#[cfg(windows)]
pub fn tema_claro() -> bool {
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
    use windows::core::w;

    let mut valor: u32 = 1;
    let mut tam = size_of::<u32>() as u32;
    // SAFETY: rutas constantes terminadas en cero; se pide un DWORD y se
    // entrega un u32 propio con su tamano exacto. RegGetValueW no guarda
    // ninguno de los dos punteros.
    let estado = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut valor as *mut u32 as *mut _),
            Some(&mut tam),
        )
    };
    if estado != ERROR_SUCCESS {
        return true;
    }
    valor != 0
}

/// Sin Windows, claro: el aspecto por defecto. En Mac la interfaz es nativa
/// y sigue sola el tema del sistema; esto solo lo miran los crates de logica.
#[cfg(not(windows))]
pub fn tema_claro() -> bool {
    true
}

/// La hora local ahora mismo, en milisegundos desde 1970 **ya corridos al
/// huso del usuario**. Asi lo de arriba puede comparar dias sin saber nada
/// de husos: dos instantes caen el mismo dia si dividen igual entre 86400000.
#[cfg(windows)]
pub fn ahora_local_ms() -> i64 {
    use windows::Win32::Foundation::{FILETIME, SYSTEMTIME};
    use windows::Win32::System::SystemInformation::GetLocalTime;
    use windows::Win32::System::Time::SystemTimeToFileTime;

    // SAFETY: `GetLocalTime` solo escribe el SYSTEMTIME que devuelve, y
    // `SystemTimeToFileTime` lee ese y escribe el FILETIME que se le pasa.
    let ft = unsafe {
        let t: SYSTEMTIME = GetLocalTime();
        let mut ft = FILETIME::default();
        if SystemTimeToFileTime(&t, &mut ft).is_err() {
            return 0;
        }
        ft
    };
    // FILETIME cuenta de cien en cien nanosegundos desde 1601.
    const A_1970: i64 = 116_444_736_000_000_000;
    let cien_ns = ((ft.dwHighDateTime as i64) << 32) | ft.dwLowDateTime as i64;
    (cien_ns - A_1970) / 10_000
}

/// Lo mismo en Mac/Linux: UTC mas el desfase que da `localtime_r` (que lee
/// el huso del sistema y sabe del horario de verano).
#[cfg(unix)]
pub fn ahora_local_ms() -> i64 {
    let utc = ahora_utc_ms();
    let segundos = (utc / 1000) as libc::time_t;
    // SAFETY: `tm` es una estructura de C sin punteros que se puede poner a
    // cero; `localtime_r` (la reentrante) solo lee `segundos` y escribe en
    // `tm`, sin retener ninguno de los dos.
    let desfase = unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&segundos, &mut tm).is_null() {
            return utc;
        }
        tm.tm_gmtoff as i64
    };
    utc + desfase * 1000
}

/// En un sistema que no es ni Windows ni Unix no hay huso que preguntar.
#[cfg(not(any(windows, unix)))]
pub fn ahora_local_ms() -> i64 {
    ahora_utc_ms()
}

/// Ahora, en milisegundos desde 1970 en UTC: la hora que se GUARDA.
///
/// Es la de `System.currentTimeMillis()` del movil. Lo que se guarda con la
/// hora local viaja al movil corrido por el huso (unas cinco horas en Peru),
/// se coloca en el chat donde no toca y, al decidir si un proyecto borrado
/// alli se toco aqui despues, puede parecer anterior de lo que fue.
pub fn ahora_utc_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

/// Cuanto hay que sumarle a una hora UTC para pintarla en la del usuario.
///
/// Se redondea al cuarto de hora: los husos van de cuarto en cuarto y asi
/// los pocos milisegundos entre las dos lecturas del reloj no se cuelan.
/// Se recalcula a cada llamada porque es barato y el cambio de horario de
/// verano tiene que notarse sin reiniciar la aplicacion.
pub fn desfase_local_ms() -> i64 {
    const CUARTO: i64 = 15 * 60_000;
    let d = ahora_local_ms() - ahora_utc_ms();
    (d as f64 / CUARTO as f64).round() as i64 * CUARTO
}

/// Una hora guardada (UTC) vista en la hora del usuario, para pintarla.
pub fn a_local(utc_ms: i64) -> i64 {
    utc_ms + desfase_local_ms()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_desfase_del_huso_va_en_cuartos_de_hora_y_a_local_lo_suma() {
        let d = desfase_local_ms();
        assert_eq!(d % (15 * 60_000), 0, "desfase {d}");
        assert!(d.abs() <= 14 * 3_600_000, "ningun huso pasa de 14 h");
        assert_eq!(a_local(1_000), 1_000 + d);
        // Caso negativo: la hora UTC no es la local corrida, salvo en UTC.
        let (utc, local) = (ahora_utc_ms(), ahora_local_ms());
        assert!((local - utc - d).abs() < 60_000);
    }

    #[test]
    fn el_tema_se_responde_sin_reventar() {
        // No se puede afirmar claro u oscuro (depende del equipo), pero si
        // que la consulta al registro no entra en panico ni cuelga, que es
        // lo unico que puede romper el arranque de un pin.
        let _ = tema_claro();
    }

    #[test]
    fn el_directorio_del_ejecutable_existe() {
        let dir = directorio_del_ejecutable().unwrap();
        assert!(dir.is_dir(), "{dir:?} deberia ser un directorio");
    }

    #[test]
    fn appdata_es_una_ruta_absoluta() {
        let dir = appdata().unwrap();
        assert!(dir.is_absolute(), "{dir:?} deberia ser absoluta");
        // En Windows existe siempre; fuera, en una CI recien creada, puede
        // no existir todavia (se crea al guardar lo primero).
        #[cfg(windows)]
        assert!(dir.is_dir(), "{dir:?} deberia ser un directorio");
    }

    #[test]
    fn el_locale_tiene_forma_de_etiqueta_de_idioma() {
        let l = locale_del_sistema();
        assert!(!l.is_empty(), "el locale no puede venir vacio");
        assert!(
            l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'),
            "locale con forma rara: {l}"
        );
    }

    #[test]
    fn las_variables_de_posix_se_vuelven_etiquetas() {
        assert_eq!(etiqueta_de_posix("es_ES.UTF-8").as_deref(), Some("es-ES"));
        assert_eq!(etiqueta_de_posix("de_DE@euro").as_deref(), Some("de-DE"));
        assert_eq!(etiqueta_de_posix("en").as_deref(), Some("en"));
        // Caso negativo: `C`, `POSIX` y lo vacio no son ningun idioma.
        assert_eq!(etiqueta_de_posix("C.UTF-8"), None);
        assert_eq!(etiqueta_de_posix("POSIX"), None);
        assert_eq!(etiqueta_de_posix(""), None);
    }

    #[test]
    fn la_tabla_portable_del_separador_sigue_a_cldr() {
        assert_eq!(separador_de_etiqueta("es-ES"), ',');
        assert_eq!(separador_de_etiqueta("pt-BR"), ',');
        assert_eq!(separador_de_etiqueta("fr_FR"), ',');
        assert_eq!(separador_de_etiqueta("es-PE"), '.');
        assert_eq!(separador_de_etiqueta("es-MX"), '.');
        assert_eq!(separador_de_etiqueta("de-CH"), '.');
        assert_eq!(separador_de_etiqueta("en-US"), '.');
        // Caso negativo: lo que no se conoce no inventa la coma.
        assert_eq!(separador_de_etiqueta("xx-NOPE"), '.');
        assert_eq!(separador_de_etiqueta(""), '.');
    }

    #[test]
    fn el_separador_decimal_es_el_de_la_region() {
        assert_eq!(separador_decimal_de(Some("es-ES")), ',');
        assert_eq!(separador_decimal_de(Some("de-DE")), ',');
        assert_eq!(separador_decimal_de(Some("en-US")), '.');
        assert_eq!(separador_decimal_de(Some("es-MX")), '.');
        // Caso negativo: una region que no existe no inventa nada.
        assert_eq!(separador_decimal_de(Some("xx-NOPE")), '.');
        assert!(matches!(separador_decimal(), '.' | ','));
    }
}
