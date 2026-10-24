//! **El portapapeles de una tabla** (J2): lo que dejan Excel, Google Sheets y
//! LibreOffice al copiar unas celdas, y lo que se deja para pegarlas en ellos.
//!
//! Las tres dejan dos versiones: texto con tabuladores (`CF_UNICODETEXT`, los
//! valores como se ven) y una tabla HTML (el formato registrado
//! `HTML Format`, CF_HTML), que es la que interesa porque puede traer las
//! formulas y la negrita. Aqui solo se leen y se escriben los dos formatos;
//! entenderlos es de `pixpin_proyecto::portapapeles_tabla`, que es el
//! `motor/PortapapelesDeTabla.kt` del movil.

use windows::Win32::Foundation::HGLOBAL;
use windows::Win32::System::DataExchange::{
    GetClipboardData, OpenClipboard, RegisterClipboardFormatW,
};
use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows::core::w;

use super::{CF_UNICODETEXT, GuardiaPortapapeles, leer_texto, publicar};
use crate::imagen::ErrorCodec;

/// El numero de `HTML Format` en esta sesion: se registra por nombre y
/// Windows da siempre el mismo mientras dure.
fn formato_html() -> u32 {
    // SAFETY: cadena literal terminada en NUL; la funcion solo la lee.
    unsafe { RegisterClipboardFormatW(w!("HTML Format")) }
}

/// Lo que hay para pegar en una tabla: el fragmento HTML (si lo hay) y el
/// texto. `None` si el portapapeles no se abre o no trae ninguno de los dos.
pub fn leer_tabla() -> Option<(Option<String>, Option<String>)> {
    // SAFETY: si abre, el guardia garantiza el cierre.
    unsafe { OpenClipboard(None) }.ok()?;
    let _guardia = GuardiaPortapapeles;
    let html = leer_html().map(|b| fragmento_de(&b));
    let texto = leer_texto();
    if html.is_none() && texto.is_none() {
        return None;
    }
    Some((html, texto))
}

/// Como [`leer_tabla`], y ademas **el HTML entero** (fragmento, entero,
/// texto): Excel pone el estilo de sus celdas en clases cuyo `<style>` va
/// fuera del fragmento (lo usa el editor de notas para los colores).
pub fn leer_tabla_con_estilos() -> Option<(Option<String>, Option<String>, Option<String>)> {
    // SAFETY: si abre, el guardia garantiza el cierre.
    unsafe { OpenClipboard(None) }.ok()?;
    let _guardia = GuardiaPortapapeles;
    let crudo = leer_html();
    let texto = leer_texto();
    if crudo.is_none() && texto.is_none() {
        return None;
    }
    let fragmento = crudo.as_deref().map(fragmento_de);
    let entero = crudo.map(|b| String::from_utf8_lossy(&b).into_owned());
    Some((fragmento, entero, texto))
}

/// Los bytes de `HTML Format`, con el portapapeles ya abierto.
pub(super) fn leer_html() -> Option<Vec<u8>> {
    let formato = formato_html();
    if formato == 0 {
        return None;
    }
    // SAFETY: portapapeles abierto; el handle es suyo y solo se lee mientras
    // dura el bloqueo, sin pasar de `GlobalSize`.
    unsafe {
        let handle = GetClipboardData(formato).ok()?;
        let bloque = HGLOBAL(handle.0);
        let tamano = GlobalSize(bloque);
        let datos = GlobalLock(bloque) as *const u8;
        if datos.is_null() {
            return None;
        }
        let bytes = std::slice::from_raw_parts(datos, tamano.min(64 * 1024 * 1024));
        let fin = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        let copia = bytes[..fin].to_vec();
        let _ = GlobalUnlock(bloque);
        Some(copia)
    }
}

/// **Deja unas celdas para pegar fuera**: el texto con tabuladores y la
/// tabla HTML, las dos en una sola sesion del portapapeles.
pub fn copiar_tabla(tsv: &str, html: &str) -> Result<(), ErrorCodec> {
    let texto: Vec<u8> = tsv
        .encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(u16::to_le_bytes)
        .collect();
    let formato = formato_html();
    if formato == 0 {
        return publicar(&[(CF_UNICODETEXT, &texto)]);
    }
    let cf_html = envolver_html(html);
    publicar(&[(CF_UNICODETEXT, &texto), (formato, &cf_html)])
}

/// Lo de dentro de un CF_HTML: entre `StartFragment` y `EndFragment` si la
/// cabecera los trae bien, y si no, todo lo que va tras la cabecera. Los
/// desplazamientos son **bytes** de UTF-8, no letras.
pub fn fragmento_de(bytes: &[u8]) -> String {
    let texto = String::from_utf8_lossy(bytes);
    let numero = |clave: &str| -> Option<usize> {
        let i = texto.find(clave)? + clave.len();
        texto[i..].lines().next()?.trim().parse().ok()
    };
    if let (Some(a), Some(b)) = (numero("StartFragment:"), numero("EndFragment:"))
        && a <= b
        && b <= bytes.len()
    {
        return String::from_utf8_lossy(&bytes[a..b]).into_owned();
    }
    match (numero("StartHTML:"), numero("EndHTML:")) {
        (Some(a), Some(b)) if a <= b && b <= bytes.len() => {
            String::from_utf8_lossy(&bytes[a..b]).into_owned()
        }
        _ => texto.into_owned(),
    }
}

/// **Un fragmento HTML con la cabecera de CF_HTML**, con los desplazamientos
/// en bytes y a diez cifras, como los escribe Office.
pub fn envolver_html(fragmento: &str) -> Vec<u8> {
    const CABECERA: &str = "Version:0.9\r\nStartHTML:0000000000\r\nEndHTML:0000000000\r\nStartFragment:0000000000\r\nEndFragment:0000000000\r\n";
    let antes = "<html><body>\r\n<!--StartFragment-->";
    let despues = "<!--EndFragment-->\r\n</body></html>";
    let inicio_html = CABECERA.len();
    let inicio_fragmento = inicio_html + antes.len();
    let fin_fragmento = inicio_fragmento + fragmento.len();
    let fin_html = fin_fragmento + despues.len();
    let cabecera = format!(
        "Version:0.9\r\nStartHTML:{inicio_html:010}\r\nEndHTML:{fin_html:010}\r\nStartFragment:{inicio_fragmento:010}\r\nEndFragment:{fin_fragmento:010}\r\n"
    );
    let mut s = String::with_capacity(fin_html);
    s.push_str(&cabecera);
    s.push_str(antes);
    s.push_str(fragmento);
    s.push_str(despues);
    s.into_bytes()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_cf_html_se_escribe_y_se_vuelve_a_leer_con_sus_desplazamientos_en_bytes() {
        // Con acentos: los desplazamientos son de bytes, y contar letras
        // cortaria el fragmento a destiempo.
        let fragmento = "<table><tr><td>Año</td><td>ñandú</td></tr></table>";
        let bytes = envolver_html(fragmento);
        assert_eq!(fragmento_de(&bytes), fragmento);
        let texto = String::from_utf8(bytes).unwrap();
        assert!(texto.starts_with("Version:0.9\r\nStartHTML:0000000105\r\n"));
    }

    #[test]
    fn un_cf_html_de_excel_se_lee_por_su_fragmento() {
        let fragmento = "<tr><td>1</td></tr>";
        let mut crudo =
            String::from("Version:1.0\r\nStartHTML:0000000000\r\nEndHTML:0000000000\r\n");
        crudo.push_str("StartFragment:XXXXXXXXXX\r\nEndFragment:YYYYYYYYYY\r\n<html><body><!--StartFragment-->");
        let a = crudo.len();
        crudo.push_str(fragmento);
        let b = crudo.len();
        crudo.push_str("<!--EndFragment--></body></html>");
        let crudo = crudo
            .replace("XXXXXXXXXX", &format!("{a:010}"))
            .replace("YYYYYYYYYY", &format!("{b:010}"));
        assert_eq!(fragmento_de(crudo.as_bytes()), fragmento);
    }

    #[test]
    fn una_cabecera_rota_no_pierde_la_tabla() {
        // Caso negativo: desplazamientos que se salen o no son numeros; se
        // devuelve todo, que la tabla sigue dentro y se buscara alli.
        let roto = "Version:0.9\r\nStartFragment:99999\r\nEndFragment:x\r\n<table><tr><td>1</td></tr></table>";
        assert!(fragmento_de(roto.as_bytes()).contains("<table>"));
        assert_eq!(fragmento_de(b""), "");
    }
}
