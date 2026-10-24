//! **Texto pegado en el lienzo**: lo que se hace con un texto del
//! portapapeles antes de convertirlo en un elemento de texto del motor.
//!
//! Es el `addTextFromPaste` de Excalidraw en su variante de «pegado plano»
//! (un solo elemento con todo el texto), que es lo que pidio el usuario:
//! copiar un texto en cualquier programa y pegarlo en el lienzo. El movil no
//! tiene este camino (alli el lienzo no recibe Ctrl+V; el texto se pega
//! dentro del cuadro de escribir, que es el del sistema), asi que se sigue
//! a Excalidraw, de donde sale el motor de los dos:
//!
//! - los finales de linea se unifican y los tabuladores pasan a espacios
//!   (`normalizeText`): un tabulador no se pinta ni se mide igual en todas
//!   las letras;
//! - se quitan los espacios y renglones vacios de los extremos (`trim`);
//! - un renglon mas ancho que `ancho_maximo` se parte por palabras
//!   (`wrapText` con `maxTextWidth`, la mitad de lo que se ve, entre 200 y
//!   800): un parrafo de mil palabras en un solo renglon no se podria leer.
//!
//! **Lo que cambia respecto a Excalidraw, y por que.** Aqui un texto suelto
//! no tiene ancho fijo (siempre es `autoResize`), asi que el ajuste se hace
//! con saltos de linea de verdad. Por eso nunca se parte una palabra: una
//! URL partida dejaria de servir al copiarla de vuelta. Una palabra mas
//! ancha que el maximo se queda sola en su renglon, entera.
//!
//! Y hay un tope ([`TOPE_CARACTERES`]): pegar un libro entero dejaria un
//! elemento que DirectWrite tendria que maquetar en cada repintado. Lo que
//! pasa del tope se corta y se marca con «…».
//!
//! Aqui no hay escena ni portapapeles: solo cadenas. Quien lo coloca es
//! `Gesto::pegar_texto`.

/// Lo mas largo que se pega de una vez, en caracteres. Unas diez paginas
/// de texto corrido: de sobra para una nota, y aun ligero de pintar.
pub const TOPE_CARACTERES: usize = 20_000;

/// Un tabulador, en espacios: lo que mide la tecla Tab en Excalidraw.
const TABULADOR: &str = "    ";

/// El ancho maximo de un texto pegado, en unidades del documento, segun lo
/// que se ve del lienzo: la mitad, entre 200 y 800 (`maxTextWidth` de
/// Excalidraw).
pub fn ancho_maximo(ancho_visible: f32) -> f32 {
    if !ancho_visible.is_finite() {
        return 800.0;
    }
    (ancho_visible * 0.5).clamp(200.0, 800.0)
}

/// **Limpia un texto del portapapeles** para pegarlo: finales de linea,
/// tabuladores, caracteres de control, extremos y tope. `None` si no queda
/// nada que pegar (vacio o solo espacios).
pub fn preparar(crudo: &str) -> Option<String> {
    let limpio = normalizar(crudo);
    // Los espacios al final de cada renglon no se ven, y engordan la caja:
    // fuera.
    let renglones: Vec<&str> = limpio.split('\n').map(str::trim_end).collect();
    let junto = renglones.join("\n");
    // Los extremos: los renglones vacios de arriba y de abajo, y el espacio
    // de delante del primero (`trim` de Excalidraw). La sangria de los demas
    // se respeta: puede ser codigo.
    let recortado = junto.trim_matches('\n').trim_start_matches(' ');
    let recortado = recortado.trim_end();
    if recortado.is_empty() {
        return None;
    }
    Some(cortar_al_tope(recortado))
}

/// **Para meterlo dentro de un texto que se escribe**: como [`preparar`],
/// pero sin recortar nada (un espacio delante de lo pegado es del usuario,
/// como en cualquier cuadro de texto). `None` si no queda nada.
pub fn preparar_para_insertar(crudo: &str) -> Option<String> {
    let limpio = normalizar(crudo);
    if limpio.is_empty() {
        return None;
    }
    Some(cortar_al_tope(&limpio))
}

/// Finales de linea, tabuladores y caracteres de control.
fn normalizar(crudo: &str) -> String {
    let mut limpio = String::with_capacity(crudo.len().min(TOPE_CARACTERES * 4));
    let mut chars = crudo.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                // CRLF de Windows y CR suelto del Mac viejo: los dos son un
                // salto.
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                limpio.push('\n');
            }
            '\n' => limpio.push('\n'),
            '\t' => limpio.push_str(TABULADOR),
            // El separador de parrafo y de linea de Unicode (Word los deja
            // a veces) tambien son saltos.
            '\u{2028}' | '\u{2029}' => limpio.push('\n'),
            // Lo demas que sea de control (NUL, el ^V que trae algun
            // terminal, el BOM) no se ve y estorbaria al medir.
            c if c.is_control() || c == '\u{FEFF}' => {}
            c => limpio.push(c),
        }
    }
    limpio
}

/// Corta en [`TOPE_CARACTERES`] (en caracteres, nunca a mitad de una letra) y
/// marca el corte.
fn cortar_al_tope(texto: &str) -> String {
    match texto.char_indices().nth(TOPE_CARACTERES) {
        None => texto.to_string(),
        Some((byte, _)) => {
            let mut s = texto[..byte].trim_end().to_string();
            s.push('…');
            s
        }
    }
}

/// **Parte en renglones** lo que pase de `ancho` (medido con `mide`), solo
/// por los espacios: una palabra no se parte nunca (ve la cabecera). Los
/// saltos que ya traia el texto se respetan.
pub fn ajustar(texto: &str, ancho: f32, mide: &dyn Fn(&str) -> f32) -> String {
    if ancho <= 0.0 {
        return texto.to_string();
    }
    let mut salida = String::with_capacity(texto.len() + texto.len() / 40);
    for (i, parrafo) in texto.split('\n').enumerate() {
        if i > 0 {
            salida.push('\n');
        }
        if mide(parrafo) <= ancho {
            salida.push_str(parrafo);
            continue;
        }
        // La sangria del parrafo va con la primera palabra.
        let sangria = parrafo.len() - parrafo.trim_start_matches(' ').len();
        let mut linea = String::from(&parrafo[..sangria]);
        let mut vacia = true;
        for palabra in parrafo[sangria..].split(' ').filter(|p| !p.is_empty()) {
            if vacia {
                linea.push_str(palabra);
                vacia = false;
                continue;
            }
            let largo = linea.len();
            linea.push(' ');
            linea.push_str(palabra);
            if mide(&linea) > ancho {
                linea.truncate(largo);
                salida.push_str(&linea);
                salida.push('\n');
                linea.clear();
                linea.push_str(palabra);
            }
        }
        salida.push_str(&linea);
    }
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Diez por caracter: cuentas faciles.
    fn diez(s: &str) -> f32 {
        s.chars().count() as f32 * 10.0
    }

    #[test]
    fn unifica_los_finales_de_linea_y_quita_los_extremos() {
        assert_eq!(
            preparar("\r\n\r\n  hola\r\nmundo  \rfin\n\n").as_deref(),
            Some("hola\nmundo\nfin")
        );
        assert_eq!(preparar("a\tb").as_deref(), Some("a    b"));
        // La sangria de los renglones de dentro se queda (puede ser codigo).
        assert_eq!(
            preparar("fn x() {\n    y\n}").as_deref(),
            Some("fn x() {\n    y\n}")
        );
    }

    #[test]
    fn dentro_de_un_texto_no_se_recorta_nada() {
        assert_eq!(
            preparar_para_insertar(" mundo\tya\r\n").as_deref(),
            Some(" mundo    ya\n")
        );
        assert_eq!(preparar_para_insertar("\u{0}"), None);
        assert_eq!(preparar_para_insertar(" ").as_deref(), Some(" "));
    }

    #[test]
    fn tildes_y_emojis_pasan_tal_cual() {
        assert_eq!(
            preparar("Canción ñandú 😀👍🏽").as_deref(),
            Some("Canción ñandú 😀👍🏽")
        );
    }

    #[test]
    fn sin_nada_visible_no_hay_pegado() {
        assert_eq!(preparar(""), None);
        assert_eq!(preparar("   \r\n\t \n"), None);
        assert_eq!(preparar("\u{0}\u{FEFF}\u{16}"), None);
    }

    #[test]
    fn un_texto_enorme_se_corta_en_el_tope_sin_partir_letras() {
        let enorme = "ñ".repeat(TOPE_CARACTERES + 500);
        let p = preparar(&enorme).unwrap();
        assert_eq!(p.chars().count(), TOPE_CARACTERES + 1);
        assert!(p.ends_with('…'));
        // Justo en el tope no se corta.
        let justo = "a".repeat(TOPE_CARACTERES);
        assert_eq!(preparar(&justo).unwrap(), justo);
    }

    #[test]
    fn el_ancho_maximo_es_la_mitad_de_lo_que_se_ve_entre_200_y_800() {
        assert_eq!(ancho_maximo(1000.0), 500.0);
        assert_eq!(ancho_maximo(100.0), 200.0);
        assert_eq!(ancho_maximo(10_000.0), 800.0);
        assert_eq!(ancho_maximo(f32::INFINITY), 800.0);
    }

    #[test]
    fn ajusta_por_palabras_y_respeta_los_saltos() {
        assert_eq!(ajustar("uno dos tres", 80.0, &diez), "uno dos\ntres");
        assert_eq!(
            ajustar("corto\nuno dos tres", 80.0, &diez),
            "corto\nuno dos\ntres"
        );
        // Lo que cabe no se toca.
        assert_eq!(ajustar("uno dos", 80.0, &diez), "uno dos");
    }

    #[test]
    fn una_url_larga_no_se_parte_nunca() {
        let url = "https://ejemplo.com/una/ruta/muy/larga?con=parametros";
        let t = format!("mira {url} ya");
        let a = ajustar(&t, 100.0, &diez);
        assert!(a.lines().any(|l| l == url), "{a:?}");
        // Sin espacios no hay donde partir: se queda entera.
        assert_eq!(ajustar(url, 100.0, &diez), url);
    }
}
