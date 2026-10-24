//! **La configuracion regional del usuario** (separadores de numeros y de
//! fechas, fecha corta), tal como la tiene puesta en Windows. La usa el
//! visor de hojas de Excel: Excel pinta el `.` y la `,` de un formato con
//! los separadores del sistema, y la `/` de una fecha con el suyo.
//!
//! Solo lee: `GetLocaleInfoEx` con la configuracion del usuario
//! (`LOCALE_NAME_USER_DEFAULT`, que es el puntero nulo).

use windows::Win32::Globalization::{
    GetLocaleInfoEx, LOCALE_SCURRENCY, LOCALE_SDATE, LOCALE_SDECIMAL, LOCALE_SSHORTDATE,
    LOCALE_STHOUSAND,
};
use windows::core::PCWSTR;

/// Lo que dice Windows, en crudo (cadenas vacias si no se pudo leer).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegionalDelSistema {
    /// `LOCALE_SDECIMAL`: «.» o «,».
    pub decimal: String,
    /// `LOCALE_STHOUSAND`: «,», «.», un espacio fino...
    pub miles: String,
    /// `LOCALE_SDATE`: el separador de la fecha corta («/», «-», «.»).
    pub separador_fecha: String,
    /// `LOCALE_SSHORTDATE`: el patron de la fecha corta (`dd/MM/yyyy`).
    pub fecha_corta: String,
}

fn leer(tipo: u32) -> String {
    let mut buf = [0u16; 96];
    // SAFETY: el bufer es nuestro y vive durante la llamada; el puntero nulo
    // es `LOCALE_NAME_USER_DEFAULT`, documentado. Devuelve los caracteres
    // escritos con el nulo final, o 0 si falla.
    let n = unsafe { GetLocaleInfoEx(PCWSTR::null(), tipo, Some(&mut buf)) };
    if n <= 0 {
        return String::new();
    }
    let n = (n as usize).min(buf.len());
    let fin = buf[..n].iter().position(|&c| c == 0).unwrap_or(n);
    String::from_utf16_lossy(&buf[..fin])
}

/// La configuracion regional del usuario, leida ahora.
pub fn del_usuario() -> RegionalDelSistema {
    RegionalDelSistema {
        decimal: leer(LOCALE_SDECIMAL),
        miles: leer(LOCALE_STHOUSAND),
        separador_fecha: leer(LOCALE_SDATE),
        fecha_corta: leer(LOCALE_SSHORTDATE),
    }
}

/// El simbolo de la moneda del usuario (`LOCALE_SCURRENCY`: «S/» en el Peru,
/// «€» en Espana); `$` si no se pudo leer. Lo usa el formato «Moneda» de la
/// hoja del pin.
pub fn simbolo_moneda() -> String {
    let s = leer(LOCALE_SCURRENCY);
    if s.trim().is_empty() { "$".into() } else { s }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn windows_siempre_tiene_un_separador_decimal() {
        let r = del_usuario();
        assert!(!r.decimal.is_empty(), "{r:?}");
        // Caso negativo: un tipo que no existe da cadena vacia, no basura.
        assert_eq!(leer(0xFFFF_FFF0), "");
    }
}
