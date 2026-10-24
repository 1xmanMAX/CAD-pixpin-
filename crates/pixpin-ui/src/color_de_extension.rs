//! **Un color por tipo de archivo** (`motor/ColorDeExtension.kt` del movil,
//! v0.98.4, 30-sep-2026, pedido por el usuario con una captura de Telegram:
//! «rojo para todo PDF, azul para docx y similares, amarillo para dwg y
//! similares»).
//!
//! Telegram solo tiene cuatro colores y reparte el resto por la primera
//! letra. Aqui, como en el movil, cada **familia** tiene el suyo, para que de
//! un vistazo se sepa que es: los parecidos (docx, doc, odt...) comparten
//! color. Lo que no esta en la tabla sale siempre del mismo color, sacado de
//! su extension con la MISMA cuenta que el movil (la suma de sus letras), asi
//! que un `.xyz` es del mismo color en los dos aparatos.
//!
//! La tabla esta copiada tal cual: cambiar aqui un color o una extension
//! haria que el mismo archivo se viera distinto en el PC y en el telefono.

use std::borrow::Cow;

/// Una familia de archivos: su nombre, su color (`0xRRGGBB`) y si la
/// extension escrita encima va en oscuro (sobre el amarillo, el blanco no se
/// lee).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Familia {
    pub nombre: Cow<'static, str>,
    pub color: u32,
    pub texto_oscuro: bool,
}

const fn familia(nombre: &'static str, color: u32, texto_oscuro: bool) -> Familia {
    Familia {
        nombre: Cow::Borrowed(nombre),
        color,
        texto_oscuro,
    }
}

pub const PDF: Familia = familia("PDF", 0xE5484D, false);
pub const WORD: Familia = familia("Documento", 0x2F7BF5, false);
pub const HOJA: Familia = familia("Hoja de calculo", 0x2E9E4F, false);
pub const PRESENTACION: Familia = familia("Presentacion", 0xF2711C, false);
pub const PLANO: Familia = familia("Plano", 0xF5B800, true);
pub const MODELO: Familia = familia("Modelo 3D", 0x0FA3A3, false);
pub const IMAGEN: Familia = familia("Imagen", 0xD6336C, false);
pub const AUDIO: Familia = familia("Audio", 0x8E44AD, false);
pub const VIDEO: Familia = familia("Video", 0x5B3CC4, false);
pub const COMPRIMIDO: Familia = familia("Comprimido", 0x8D6E63, false);
pub const CODIGO: Familia = familia("Codigo y web", 0x1098AD, false);
pub const TEXTO: Familia = familia("Texto", 0x607D8B, false);
pub const LIBRO: Familia = familia("Libro", 0x3B5BDB, false);
pub const APLICACION: Familia = familia("Aplicacion", 0x6CBF5B, false);
pub const PIXPIN: Familia = familia("PixPin", 0x7048E8, false);

/// Lo que no tiene extension: gris, «Archivo».
pub const SIN_EXTENSION: Familia = familia("Archivo", 0x868E96, false);

/// La tabla del movil (`porExtension`), familia por familia.
const TABLA: &[(&Familia, &[&str])] = &[
    (&PDF, &["pdf"]),
    (
        &WORD,
        &[
            "doc", "docx", "docm", "dot", "dotx", "odt", "rtf", "pages", "wps",
        ],
    ),
    (
        &HOJA,
        &[
            "xls", "xlsx", "xlsm", "xlsb", "csv", "tsv", "ods", "numbers",
        ],
    ),
    (
        &PRESENTACION,
        &[
            "ppt", "pptx", "pptm", "pps", "ppsx", "pot", "potx", "odp", "key",
        ],
    ),
    (
        &PLANO,
        &[
            "dwg", "dxf", "dwf", "dwfx", "dgn", "dwt", "plt", "rvt", "rfa", "ifc", "nwd", "nwc",
        ],
    ),
    (
        &MODELO,
        &[
            "obj", "stl", "fbx", "3ds", "blend", "skp", "gltf", "glb", "dae", "ply", "step", "stp",
            "iges", "igs", "3dm", "usdz",
        ],
    ),
    (
        &IMAGEN,
        &[
            "jpg", "jpeg", "png", "webp", "gif", "heic", "heif", "bmp", "tif", "tiff", "svg",
            "psd", "ai", "raw", "dng", "avif", "ico",
        ],
    ),
    (
        &AUDIO,
        &[
            "mp3", "m4a", "aac", "wav", "ogg", "oga", "opus", "flac", "amr", "wma", "mid", "midi",
        ],
    ),
    (
        &VIDEO,
        &[
            "mp4", "mov", "avi", "mkv", "webm", "3gp", "m4v", "wmv", "flv",
        ],
    ),
    (
        &COMPRIMIDO,
        &["zip", "rar", "7z", "tar", "gz", "tgz", "bz2", "xz", "zst"],
    ),
    (
        &CODIGO,
        &[
            "html", "htm", "css", "js", "ts", "json", "xml", "kt", "kts", "java", "py", "c", "h",
            "cpp", "hpp", "cs", "rs", "go", "sh", "sql", "yml", "yaml", "tex", "m", "ipynb",
        ],
    ),
    (&TEXTO, &["txt", "md", "log", "ini", "cfg", "srt", "vtt"]),
    (
        &LIBRO,
        &["epub", "mobi", "azw", "azw3", "fb2", "djvu", "cbz", "cbr"],
    ),
    (
        &APLICACION,
        &[
            "apk", "aab", "xapk", "apks", "exe", "msi", "dmg", "deb", "appimage",
        ],
    ),
    (&PIXPIN, &["pixpin", "excalidraw"]),
];

/// Para lo que no esta en la tabla: colores que se distinguen de los de las
/// familias (`RESTO` del movil, en su orden: el orden es parte de la cuenta).
const RESTO: [u32; 8] = [
    0x5C7CFA, 0x20C997, 0xFF6B6B, 0xFAB005, 0x845EF7, 0x15AABF, 0xE64980, 0x82C91E,
];

/// La extension de `nombre`, en minusculas, o `None` si no tiene.
///
/// Lo de despues del ultimo punto del ultimo tramo de la ruta, como el movil
/// (`substringAfterLast('/')` y luego `'.'`); aqui tambien se corta por `\`,
/// que es la barra de las rutas de Windows y en el movil no aparece. Mas de
/// diez letras no es una extension, es un nombre con un punto en medio.
pub fn extension(nombre: &str) -> Option<String> {
    let tramo = nombre.rsplit(['/', '\\']).next().unwrap_or(nombre);
    let (_, ext) = tramo.rsplit_once('.')?;
    let ext = ext.to_lowercase();
    // `length <= 10` de Kotlin cuenta unidades UTF-16.
    (!ext.trim().is_empty() && ext.encode_utf16().count() <= 10).then_some(ext)
}

/// La familia de `nombre`: la de su extension, o una hecha a su medida si no
/// esta en la tabla.
pub fn de(nombre: &str) -> Familia {
    let Some(ext) = extension(nombre) else {
        return SIN_EXTENSION;
    };
    if let Some((f, _)) = TABLA.iter().find(|(_, exts)| exts.contains(&ext.as_str())) {
        return (*f).clone();
    }
    // Siempre el mismo para la misma extension: la suma de sus letras
    // (`sumOf { it.code }`, unidades UTF-16), igual que el movil.
    let suma: u64 = ext.encode_utf16().map(u64::from).sum();
    let color = RESTO[(suma % RESTO.len() as u64) as usize];
    Familia {
        nombre: Cow::Owned(ext.to_uppercase()),
        color,
        texto_oscuro: color == 0xFAB005,
    }
}

/// Lo que se escribe en el icono: la extension en minusculas, como Telegram,
/// cortada a 4 letras. Vacio si no tiene.
pub fn rotulo(nombre: &str) -> String {
    extension(nombre)
        .map(|e| e.chars().take(4).collect())
        .unwrap_or_default()
}

/// El color de la familia en `(r, g, b)` de 0 a 1, para pintar.
pub fn rgb(f: &Familia) -> (f32, f32, f32) {
    let c = |d: u32| ((f.color >> d) & 0xFF) as f32 / 255.0;
    (c(16), c(8), c(0))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    // Las constantes de la tabla tambien se comprueban: es lo que se prueba.
    #[allow(clippy::assertions_on_constants)]
    fn cada_familia_tiene_su_color_como_en_el_movil() {
        assert_eq!(de("Tesis final.PDF"), PDF);
        assert_eq!(de("informe.docx"), WORD);
        assert_eq!(de("viejo.doc"), WORD);
        assert_eq!(de("Planta baja.dwg"), PLANO);
        assert_eq!(de("corte.dxf"), PLANO);
        assert_eq!(de("cuentas.xlsx"), HOJA);
        assert_eq!(de("charla.pptx"), PRESENTACION);
        assert_eq!(de("/storage/Download/PixPin-0.98.4.apk"), APLICACION);
        assert_eq!(PDF.color, 0xE5484D);
        assert!(PLANO.texto_oscuro, "sobre el amarillo el texto va oscuro");
        assert!(!PDF.texto_oscuro);
    }

    #[test]
    fn una_ruta_de_windows_mira_solo_el_nombre_del_fichero() {
        assert_eq!(de(r"C:\Users\Max\Planos\nave.rvt"), PLANO);
        // Caso negativo: un punto en una carpeta no es la extension.
        assert_eq!(de(r"C:\obra.v2\LEEME"), SIN_EXTENSION);
    }

    #[test]
    fn lo_que_no_esta_en_la_tabla_sale_siempre_igual_y_con_la_cuenta_del_movil() {
        let una = de("a.xyz");
        assert_eq!(una.color, de("otro nombre.XYZ").color);
        assert_eq!(una.nombre, "XYZ");
        // x+y+z = 120+121+122 = 363; 363 % 8 = 3 -> 0xFAB005, el amarillo,
        // que lleva la letra oscura como en el movil.
        assert_eq!(una.color, 0xFAB005);
        assert!(una.texto_oscuro);
        // «abc»: 97+98+99 = 294; 294 % 8 = 6 -> 0xE64980, letra blanca.
        let otra = de("a.abc");
        assert_eq!(otra.color, 0xE64980);
        assert!(!otra.texto_oscuro);
    }

    #[test]
    fn sin_extension_es_un_archivo_gris_sin_rotulo() {
        assert_eq!(de("sin extension").nombre, "Archivo");
        assert_eq!(de("sin extension").color, 0x868E96);
        assert_eq!(de("acaba en punto."), SIN_EXTENSION);
        assert_eq!(de(""), SIN_EXTENSION);
        // Mas de diez letras despues del punto no es una extension.
        assert_eq!(de("nota.esto-no-es-extension"), SIN_EXTENSION);
        assert_eq!(rotulo("LEEME"), "");
    }

    #[test]
    fn el_rotulo_es_la_extension_corta_en_minusculas() {
        assert_eq!(rotulo("a.PDF"), "pdf");
        assert_eq!(rotulo("a.xlsx"), "xlsx");
        assert_eq!(rotulo("proyecto.pixpin"), "pixp");
        assert_eq!(rotulo("dibujo.excalidraw"), "exca");
    }

    #[test]
    fn el_color_se_pasa_a_rgb_sin_perder_nada() {
        let (r, g, b) = rgb(&PDF);
        assert_eq!(
            (
                (r * 255.0).round(),
                (g * 255.0).round(),
                (b * 255.0).round()
            ),
            (229.0, 72.0, 77.0)
        );
    }
}
