//! **Escuchar un documento en voz alta**: las cuentas, sin Windows, para
//! poder comprobarlas. Puerto de `motor/VozAlta.kt` y de la parte de la voz
//! de `motor/Lectura.kt` del movil (22/23-sep-2026).
//!
//! En el movil lee la voz de Google del telefono; aqui, la de Windows
//! (`pixpin_voz::sapi::Lector`), que ya esta en el equipo, no pesa y no
//! manda el texto a ningun sitio. Quien habla con Windows es
//! `apps/pixpin/src/leer_en_voz.rs`; esto decide **en que trozos**, **con
//! que idioma** y **donde se quedo** (el marcador verde).
//!
//! # El marcador verde
//!
//! Donde se dejo de escuchar un documento: **uno solo** por documento, que
//! se mueve con lo que se va oyendo. Se guarda como `parrafo:fraccion`
//! (`Lectura.vozATexto`): el numero del parrafo y en que fraccion del
//! documento empieza. De un adjunto del chat va en `anot-<uid>.voz`, que
//! viaja; vacio es «sin verde» (borrado se tomaria por algo quitado a
//! proposito). El PC y el movil no numeran los parrafos exactamente igual
//! (alli cuenta el `WebView`), asi que al volver se busca **por la
//! fraccion** cuando el numero no cae cerca ([`parrafo_de_la_marca`]).

use std::ops::Range;

use crate::lectura::Marcador;

/// El emoticono del marcador de la voz (`Lectura.EMOJI_DE_VOZ`).
pub const EMOJI_DE_VOZ: &str = "🟢";

/// Las velocidades que se ofrecen; 1 es la normal (`VozAlta.VELOCIDADES`).
pub const VELOCIDADES: [f32; 6] = [0.75, 1.0, 1.25, 1.5, 1.75, 2.0];

/// Lo mas largo que se le da a la voz de una vez (`VozAlta.TOPE`): con
/// trozos cortos empieza a hablar antes y, al saltar, se vuelve a un sitio
/// mas cercano.
pub const TOPE: usize = 600;

/// La velocidad siguiente al pulsar el boton, dando la vuelta al final.
pub fn siguiente_velocidad(ahora: f32) -> f32 {
    let i = VELOCIDADES.iter().position(|v| (v - ahora).abs() < 0.01);
    match i {
        Some(i) => VELOCIDADES[(i + 1) % VELOCIDADES.len()],
        None => VELOCIDADES[1],
    }
}

/// «1×», «1,25×»: como se lee en el boton (`VozAlta.rotulo`).
pub fn rotulo(velocidad: f32) -> String {
    let centesimas = (velocidad * 100.0).round() as i32;
    let entero = centesimas / 100;
    let resto = centesimas % 100;
    if resto == 0 {
        format!("{entero}×")
    } else if resto % 10 == 0 {
        format!("{entero},{}×", resto / 10)
    } else {
        format!("{entero},{resto:02}×")
    }
}

/// **La velocidad como la entiende la voz de Windows** (`ISpVoice::SetRate`,
/// de −10 a 10). SAPI multiplica la velocidad por tres cada diez pasos, asi
/// que el paso es `10·log₃(v)`: 1,5× son 4 pasos y 2× son 6.
pub fn tasa_de_windows(velocidad: f32) -> i32 {
    if !velocidad.is_finite() || velocidad <= 0.0 {
        return 0;
    }
    (10.0 * velocidad.ln() / 3f32.ln())
        .round()
        .clamp(-10.0, 10.0) as i32
}

/// **Un parrafo, en trozos que la voz admita**, como `VozAlta.trozos`: se
/// corta por el final de las frases; una frase mas larga que `tope`, por las
/// comas; y lo que aun sobre, por los espacios. Devuelve los trozos como
/// rangos **en letras** (`char`) del texto original, sin los blancos de los
/// bordes: el lector de PDF los usa para marcar en la hoja lo que suena.
pub fn cortes(texto: &str, tope: usize) -> Vec<Range<usize>> {
    let letras: Vec<char> = texto.chars().collect();
    let tope = tope.max(1);
    let mut salida: Vec<Range<usize>> = Vec::new();
    // Los finales de frase: tras `.!?…;:` (con comillas o parentesis que
    // cierran) y un blanco.
    let frases = partir_detras(&letras, 0..letras.len(), |l, i| {
        es_blanco(l[i]) && i > 0 && cierra_frase(l, i - 1)
    });
    let mut actual: Option<Range<usize>> = None;
    let anadir =
        |pieza: Range<usize>, actual: &mut Option<Range<usize>>, salida: &mut Vec<Range<usize>>| {
            let largo = actual.as_ref().map_or(0, |a| a.end - a.start);
            if largo + (pieza.end - pieza.start) > tope {
                soltar(&letras, actual, salida);
            }
            *actual = Some(match actual.take() {
                Some(a) => a.start..pieza.end,
                None => pieza,
            });
        };
    for frase in frases {
        if frase.end - frase.start <= tope {
            anadir(frase, &mut actual, &mut salida);
            continue;
        }
        let partes = partir_detras(&letras, frase, |l, i| {
            es_blanco(l[i]) && i > 0 && l[i - 1] == ','
        });
        for parte in partes {
            if parte.end - parte.start <= tope {
                anadir(parte, &mut actual, &mut salida);
                continue;
            }
            // Por los espacios, y una «palabra» eterna, a tajos.
            let palabras = partir_detras(&letras, parte, |l, i| es_blanco(l[i]));
            for palabra in palabras {
                let mut desde = palabra.start;
                while desde < palabra.end {
                    let hasta = (desde + tope).min(palabra.end);
                    anadir(desde..hasta, &mut actual, &mut salida);
                    desde = hasta;
                }
            }
        }
    }
    soltar(&letras, &mut actual, &mut salida);
    salida
}

/// Lo juntado hasta ahora, como trozo (sin sus blancos de los bordes).
fn soltar(letras: &[char], actual: &mut Option<Range<usize>>, salida: &mut Vec<Range<usize>>) {
    if let Some(r) = actual.take()
        && let Some(r) = sin_blancos(letras, r)
    {
        salida.push(r);
    }
}

/// Los trozos ya como texto, con los blancos juntados (`VozAlta.trozos`).
pub fn trozos(texto: &str, tope: usize) -> Vec<String> {
    let letras: Vec<char> = texto.chars().collect();
    cortes(texto, tope)
        .into_iter()
        .map(|r| juntar_blancos(&letras[r].iter().collect::<String>()))
        .filter(|t| !t.is_empty())
        .collect()
}

/// Los blancos seguidos, uno; y sin blancos en los bordes.
pub fn juntar_blancos(texto: &str) -> String {
    texto.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn es_blanco(c: char) -> bool {
    c.is_whitespace()
}

/// Si en `i` acaba una frase: un signo de final, quiza seguido de comillas
/// o parentesis que cierran.
fn cierra_frase(l: &[char], mut i: usize) -> bool {
    while matches!(l[i], '"' | '\'' | '»' | '”' | ')') {
        if i == 0 {
            return false;
        }
        i -= 1;
    }
    matches!(l[i], '.' | '!' | '?' | '…' | ';' | ':')
}

/// Parte `rango` justo **detras** de cada letra `i` en la que `corta` dice
/// que si (la letra `i` va con lo de delante). Sin perder nada.
fn partir_detras(
    l: &[char],
    rango: Range<usize>,
    corta: impl Fn(&[char], usize) -> bool,
) -> Vec<Range<usize>> {
    let mut salida = Vec::new();
    let mut desde = rango.start;
    for i in rango.clone() {
        if corta(l, i) {
            salida.push(desde..i + 1);
            desde = i + 1;
        }
    }
    if desde < rango.end {
        salida.push(desde..rango.end);
    }
    salida
}

fn sin_blancos(l: &[char], r: Range<usize>) -> Option<Range<usize>> {
    let mut a = r.start;
    let mut b = r.end;
    while a < b && es_blanco(l[a]) {
        a += 1;
    }
    while b > a && es_blanco(l[b - 1]) {
        b -= 1;
    }
    (a < b).then_some(a..b)
}

// ---------------------------------------------------------------- el idioma

/// «es_ES», «es-es», «ES» → («es», «ES») (`VozAlta.partes`).
pub fn partes(idioma: &str) -> (String, String) {
    let p: Vec<&str> = idioma
        .trim()
        .split(['-', '_'])
        .filter(|s| !s.is_empty())
        .collect();
    let lengua = p.first().map(|s| s.to_lowercase()).unwrap_or_default();
    let pais = p
        .get(1)
        .map(|s| s.to_uppercase())
        .filter(|s| s.len() == 2 || s.len() == 3)
        .unwrap_or_default();
    (lengua, pais)
}

/// Las palabras mas corrientes de cada lengua (`VozAlta.PALABRAS`).
const PALABRAS: [(&str, &[&str]); 6] = [
    (
        "es",
        &[
            "el", "la", "los", "las", "de", "del", "que", "y", "en", "un", "una", "por", "con",
            "para", "es", "se", "no", "lo", "al", "su", "como", "más", "pero", "sus", "le", "ya",
            "o", "este", "sí", "porque", "esta", "entre", "cuando", "muy", "sin", "sobre",
            "también", "hay", "donde", "está",
        ],
    ),
    (
        "en",
        &[
            "the", "of", "and", "to", "in", "is", "that", "it", "was", "for", "on", "are", "as",
            "with", "his", "they", "at", "be", "this", "have", "from", "or", "by", "but", "not",
            "what", "all", "were", "when", "we", "there", "can", "which", "their", "if", "would",
            "been", "has", "an", "she",
        ],
    ),
    (
        "pt",
        &[
            "o", "os", "as", "de", "do", "da", "dos", "das", "que", "e", "em", "um", "uma", "para",
            "com", "não", "no", "na", "por", "mais", "se", "ao", "como", "mas", "foi", "ele",
            "ela", "seu", "sua", "ou", "ser", "quando", "muito", "há", "nos", "já", "está",
            "também", "só", "pelo",
        ],
    ),
    (
        "fr",
        &[
            "le", "la", "les", "de", "des", "du", "et", "un", "une", "est", "en", "que", "qui",
            "dans", "pour", "pas", "sur", "au", "aux", "ne", "il", "elle", "se", "ce", "avec",
            "plus", "par", "son", "sa", "ses", "mais", "nous", "vous", "ont", "été", "cette",
            "sont", "comme", "leur", "tout",
        ],
    ),
    (
        "it",
        &[
            "il", "lo", "la", "gli", "le", "di", "del", "della", "che", "e", "è", "un", "una",
            "per", "con", "non", "in", "si", "da", "al", "sono", "come", "più", "ma", "anche",
            "questo", "nel", "nella", "dei", "delle", "suo", "sua", "ha", "molto", "quando", "già",
            "tra", "cosa", "loro", "essere",
        ],
    ),
    (
        "de",
        &[
            "der", "die", "das", "und", "ist", "nicht", "ein", "eine", "zu", "den", "dem", "mit",
            "sich", "des", "auf", "für", "im", "von", "auch", "es", "an", "werden", "aus", "er",
            "hat", "dass", "sie", "nach", "wird", "bei", "einer", "um", "noch", "wie", "einem",
            "über", "so", "zum", "war", "haben",
        ],
    ),
];

/// **De que idioma es el texto**, por sus palabras mas corrientes
/// (`VozAlta.idiomaDelTexto`). Un libro en ingles leido con la voz
/// castellana no se entiende. Si la muestra no deja claro nada, `por_defecto`.
pub fn idioma_del_texto(muestra: &str, por_defecto: &str) -> String {
    let minus = muestra.to_lowercase();
    let palabras: Vec<&str> = minus
        .split(|c: char| !(c.is_alphabetic() || c == '\''))
        .filter(|p| !p.is_empty())
        .take(600)
        .collect();
    if palabras.len() < 8 {
        return por_defecto.to_string();
    }
    let cuentas: Vec<(&str, usize)> = PALABRAS
        .iter()
        .map(|(lengua, lista)| {
            (
                *lengua,
                palabras.iter().filter(|p| lista.contains(p)).count(),
            )
        })
        .collect();
    // El primero con mas, como `maxByOrNull` (el primero en caso de empate).
    let (mejor, n) = cuentas
        .iter()
        .fold(cuentas[0], |a, b| if b.1 > a.1 { *b } else { a });
    let segundo = cuentas
        .iter()
        .filter(|(l, _)| *l != mejor)
        .map(|(_, c)| *c)
        .max()
        .unwrap_or(0);
    // Claro: al menos un 6 % de las palabras y bastante por delante del siguiente.
    if n as f32 >= palabras.len() as f32 * 0.06 && n as f32 > segundo as f32 * 1.3 {
        mejor.to_string()
    } else {
        por_defecto.to_string()
    }
}

/// **Con que idioma se lee** (`VozAlta.idiomaParaLeer`): el adivinado por el
/// texto, afinado con el acento del documento o del equipo si es la misma
/// lengua; sin nada que adivinar, el del documento, y si no, el del equipo.
pub fn idioma_para_leer(muestra: &str, lang: &str, del_equipo: &str) -> String {
    let adivinado = idioma_del_texto(muestra, "");
    let (suyo, su_pais) = partes(lang);
    if adivinado.is_empty() {
        return if suyo.is_empty() {
            del_equipo.to_string()
        } else {
            lang.to_string()
        };
    }
    if suyo == adivinado && !su_pais.is_empty() {
        lang.to_string()
    } else if partes(del_equipo).0 == adivinado {
        del_equipo.to_string()
    } else {
        adivinado
    }
}

// ------------------------------------------------------- el marcador verde

/// `parrafo:fraccion`, como `Lectura.vozATexto`.
pub fn voz_a_texto(parrafo: usize, fraccion: f32) -> String {
    format!("{parrafo}:{}", fraccion.clamp(0.0, 1.0))
}

/// Al reves (`Lectura.vozDeTexto`); `None` si no se entiende o esta vacio.
pub fn voz_de_texto(texto: &str) -> Option<(usize, f32)> {
    let mut p = texto.trim().split(':');
    let (a, b) = (p.next()?, p.next()?);
    if p.next().is_some() {
        return None;
    }
    let parrafo = a.trim().parse::<i64>().ok().filter(|n| *n >= 0)? as usize;
    let f = b.trim().parse::<f32>().ok().filter(|f| f.is_finite())?;
    Some((parrafo, f.clamp(0.0, 1.0)))
}

/// **Por que parrafo se sigue**: el del numero si su fraccion cae cerca de la
/// apuntada (el mismo documento contado igual); si no, el que empieza mas
/// cerca de esa fraccion (contado por el otro aparato). `fracciones` es
/// donde empieza cada parrafo, en orden.
pub fn parrafo_de_la_marca(fracciones: &[f32], marca: (usize, f32)) -> Option<usize> {
    if fracciones.is_empty() {
        return None;
    }
    let (p, f) = marca;
    if let Some(propia) = fracciones.get(p)
        && (propia - f).abs() <= 0.02
    {
        return Some(p);
    }
    let i = fracciones.partition_point(|x| *x < f);
    let candidatos = [i.checked_sub(1), (i < fracciones.len()).then_some(i)];
    candidatos.into_iter().flatten().min_by(|a, b| {
        let da = (fracciones[*a] - f).abs();
        let db = (fracciones[*b] - f).abs();
        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
    })
}

/// Los marcadores con el verde de la voz en su sitio
/// (`Lectura.conMarcaDeVoz`): nunca mas de un verde.
pub fn con_marca_de_voz(lista: &[Marcador], fraccion: Option<f32>) -> Vec<Marcador> {
    let mut salida: Vec<Marcador> = lista
        .iter()
        .filter(|m| m.emoji != EMOJI_DE_VOZ)
        .cloned()
        .collect();
    if let Some(f) = fraccion {
        salida.push(Marcador {
            id: u64::MAX,
            fraccion: f.clamp(0.0, 1.0),
            emoji: EMOJI_DE_VOZ.to_string(),
        });
    }
    salida.sort_by(|a, b| {
        a.fraccion
            .partial_cmp(&b.fraccion)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_velocidad_da_la_vuelta_y_se_escribe_como_en_el_movil() {
        assert_eq!(siguiente_velocidad(1.0), 1.25);
        assert_eq!(siguiente_velocidad(2.0), 0.75);
        assert_eq!(siguiente_velocidad(3.3), 1.0, "una rara vuelve a la normal");
        assert_eq!(rotulo(1.0), "1×");
        assert_eq!(rotulo(1.5), "1,5×");
        assert_eq!(rotulo(1.25), "1,25×");
        assert_eq!(rotulo(0.75), "0,75×");
    }

    #[test]
    fn la_tasa_de_windows_crece_con_la_velocidad_y_se_queda_en_sus_topes() {
        assert_eq!(tasa_de_windows(1.0), 0);
        assert_eq!(tasa_de_windows(2.0), 6);
        assert_eq!(tasa_de_windows(1.5), 4);
        assert!(tasa_de_windows(0.75) < 0);
        assert_eq!(tasa_de_windows(100.0), 10);
        assert_eq!(tasa_de_windows(0.0), 0);
        let tasas: Vec<i32> = VELOCIDADES.iter().map(|v| tasa_de_windows(*v)).collect();
        assert!(tasas.windows(2).all(|w| w[0] < w[1]), "{tasas:?}");
    }

    #[test]
    fn un_parrafo_corto_es_un_trozo_y_uno_largo_se_corta_por_las_frases() {
        assert_eq!(trozos("  Hola   mundo. ", 600), vec!["Hola mundo."]);
        assert!(trozos("   ", 600).is_empty());
        let t = "Primera frase. Segunda frase! ¿Tercera? Cuarta.";
        let v = trozos(t, 20);
        assert_eq!(
            v,
            vec!["Primera frase.", "Segunda frase!", "¿Tercera? Cuarta."]
        );
        // Nada se pierde: juntos dicen lo mismo.
        assert_eq!(v.join(" "), t);
    }

    #[test]
    fn una_frase_eterna_se_corta_por_las_comas_y_luego_por_los_espacios() {
        let t = "uno, dos, tres, cuatro, cinco";
        let v = trozos(t, 12);
        assert!(v.iter().all(|x| x.chars().count() <= 12), "{v:?}");
        assert_eq!(v.join(" "), t);
        let palabra = "a".repeat(30);
        let v = trozos(&palabra, 10);
        assert_eq!(v.len(), 3);
        assert!(v.iter().all(|x| x.len() == 10));
    }

    #[test]
    fn los_cortes_son_rangos_en_letras_del_texto_original() {
        let t = "Él dijo «sí». Y se fue.";
        let r = cortes(t, 14);
        let letras: Vec<char> = t.chars().collect();
        let textos: Vec<String> = r
            .iter()
            .map(|x| letras[x.clone()].iter().collect())
            .collect();
        assert_eq!(textos, vec!["Él dijo «sí».", "Y se fue."]);
    }

    #[test]
    fn el_idioma_se_adivina_por_las_palabras_y_se_afina_con_el_acento() {
        let ingles = "The quick fox was in the house and it was not what they said of the dog";
        assert_eq!(idioma_del_texto(ingles, "es"), "en");
        let castellano =
            "El perro de la casa no es lo que dicen los vecinos y por eso se fue con su dueño";
        assert_eq!(idioma_del_texto(castellano, "en"), "es");
        assert_eq!(
            idioma_del_texto("hola", "fr"),
            "fr",
            "poca muestra: lo de por defecto"
        );
        assert_eq!(idioma_para_leer(castellano, "", "es-PE"), "es-PE");
        assert_eq!(idioma_para_leer(castellano, "es-MX", "es-PE"), "es-MX");
        assert_eq!(idioma_para_leer(ingles, "", "es-PE"), "en");
        assert_eq!(idioma_para_leer("", "", "es-PE"), "es-PE");
        assert_eq!(partes("es_es"), ("es".to_string(), "ES".to_string()));
    }

    #[test]
    fn el_verde_se_escribe_y_se_lee_como_en_el_movil() {
        assert_eq!(voz_a_texto(12, 0.5), "12:0.5");
        assert_eq!(voz_de_texto("12:0.5"), Some((12, 0.5)));
        // Lo que escribe Kotlin con un `Float` pequeno.
        assert_eq!(voz_de_texto("3:1.0E-4"), Some((3, 0.0001)));
        assert_eq!(voz_de_texto("3:7"), Some((3, 1.0)), "la fraccion se acota");
        assert_eq!(voz_de_texto(""), None, "vacio es sin verde");
        assert_eq!(voz_de_texto("-1:0.2"), None);
        assert_eq!(voz_de_texto("1:2:3"), None);
    }

    #[test]
    fn se_sigue_por_el_numero_si_cuadra_y_si_no_por_la_fraccion() {
        let f = [0.0, 0.1, 0.2, 0.5, 0.9];
        assert_eq!(parrafo_de_la_marca(&f, (3, 0.5)), Some(3));
        // El movil conto otro numero: manda la fraccion.
        assert_eq!(parrafo_de_la_marca(&f, (40, 0.48)), Some(3));
        assert_eq!(parrafo_de_la_marca(&f, (1, 0.85)), Some(4));
        assert_eq!(parrafo_de_la_marca(&[], (0, 0.0)), None);
    }

    #[test]
    fn nunca_hay_mas_de_un_verde() {
        let lista = vec![
            Marcador {
                id: 1,
                fraccion: 0.3,
                emoji: "⭐".into(),
            },
            Marcador {
                id: 2,
                fraccion: 0.6,
                emoji: EMOJI_DE_VOZ.into(),
            },
        ];
        let v = con_marca_de_voz(&lista, Some(0.1));
        assert_eq!(v.iter().filter(|m| m.emoji == EMOJI_DE_VOZ).count(), 1);
        assert_eq!(v[0].emoji, EMOJI_DE_VOZ);
        assert_eq!(v[0].fraccion, 0.1);
        assert_eq!(con_marca_de_voz(&lista, None).len(), 1);
    }
}
