//! **El menu de la barra** (`/`) del editor de notas (H12): se escribe `/`
//! al principio de un renglon, sale la lista de bloques, se filtra segun se
//! teclea (`/tab` deja la tabla) y al elegir, la barra y lo tecleado se
//! cambian por el bloque. Es el puerto de `motormd/Comandos.kt` +
//! `motormd/Bloques.kt` del movil (que copian el `RichCommand` del editor de
//! Telegram) con los bloques que el PC sabe poner.
//!
//! La barra solo cuenta **al principio del renglon**: asi una fecha `12/03`
//! o una ruta `a/b` no abren nada en mitad de una frase.
//!
//! Aqui solo cuentas sobre el texto (UTF-16, como el `RichEdit`); el menu lo
//! pinta el editor.

/// Lo que se puede poner desde el menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bloque {
    Titulo1,
    Titulo2,
    Titulo3,
    Lista,
    Numerada,
    Casillas,
    Cita,
    Codigo,
    Tabla,
    Imagen,
    Fecha,
    Separador,
    /// Una hoja de un proyecto como imagen que se actualiza (pagina viva,
    /// ver `md_imagen`).
    Pagina,
    /// Un enlace a una hoja de un proyecto (`pixpin:hoja=`).
    EnlaceHoja,
    /// Un documento de fuera (PDF, Word, plano...), copiado junto a la nota
    /// y pintado como su tarjeta del chat (el `ARCHIVO` del movil).
    Documento,
    /// Un mensaje del chat del proyecto, como su burbuja.
    DelChat,
    /// Un audio de fuera, con su reproductor (el `AUDIO` del movil).
    Audio,
}

/// Los bloques en el orden en que salen, cada uno con sus atajos: el de
/// Markdown de siempre y palabras en los dos idiomas, para que valga tanto
/// si uno se acuerda del simbolo como si solo de la palabra
/// (`Bloques.todos`).
pub const CATALOGO: [(Bloque, &[&str]); 17] = [
    (
        Bloque::Titulo1,
        &["#", "t1", "h1", "titulo", "heading", "encabezado"],
    ),
    (Bloque::Titulo2, &["##", "t2", "h2", "subtitulo"]),
    (Bloque::Titulo3, &["###", "t3", "h3"]),
    (Bloque::Lista, &["-", "lista", "vinetas", "bullet", "list"]),
    (Bloque::Numerada, &["1.", "numerada", "numbered", "ol"]),
    (
        Bloque::Casillas,
        &["[]", "casillas", "tareas", "checklist", "todo"],
    ),
    (Bloque::Cita, &[">", "cita", "quote"]),
    (Bloque::Codigo, &["```", "codigo", "code", "pre"]),
    (Bloque::Tabla, &["tabla", "table"]),
    (Bloque::Imagen, &["imagen", "foto", "image", "picture"]),
    (Bloque::Fecha, &["fecha", "hoy", "date", "today"]),
    (
        Bloque::Separador,
        &["---", "separador", "raya", "divider", "rule"],
    ),
    (
        Bloque::Pagina,
        &[
            "pagina", "hoja", "lienzo", "proyecto", "page", "sheet", "canvas",
        ],
    ),
    (Bloque::EnlaceHoja, &["enlace", "vinculo", "link"]),
    // Los de `motormd/Bloques.kt` del movil (`/archivo`, `/adjunto`,
    // `/audio`, `/musica`), mas el chat, que alli no esta.
    (
        Bloque::Documento,
        &["documento", "archivo", "adjunto", "pdf", "document", "file"],
    ),
    (Bloque::DelChat, &["chat", "mensaje", "message"]),
    (
        Bloque::Audio,
        &["audio", "voz", "musica", "sonido", "voice"],
    ),
];

/// Lo tecleado tras la barra si el cursor esta escribiendo un comando: la
/// posicion de la barra y lo que va detras. `None` si no.
pub fn consulta(texto: &str, cursor: usize) -> Option<(usize, String)> {
    let u: Vec<u16> = texto.encode_utf16().collect();
    let cursor = cursor.min(u.len());
    let mut i = cursor;
    while i > 0 && es_de_comando(u[i - 1]) {
        i -= 1;
    }
    if i == 0 || u[i - 1] != b'/' as u16 {
        return None;
    }
    let barra = i - 1;
    if barra > 0 && u[barra - 1] != b'\n' as u16 && u[barra - 1] != b'\r' as u16 {
        return None;
    }
    Some((barra, String::from_utf16_lossy(&u[i..cursor])))
}

/// Letras, cifras y lo poco mas que llevan los atajos (`t1`, `1.`, `[]`).
fn es_de_comando(u: u16) -> bool {
    char::from_u32(u as u32).is_some_and(|c| {
        c.is_alphanumeric() || matches!(c, '_' | '#' | '.' | '-' | '[' | ']' | '>' | '`')
    })
}

/// Sin tildes ni mayusculas, para que «titulo» encuentre «Título».
fn plano(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            otra => otra,
        })
        .collect()
}

/// Si `q` encaja con un bloque: por el principio de cada palabra de su
/// nombre o de cada atajo (`Bloque.encaja`). Por el principio y no «que
/// contenga»: con dos letras basta y no sale media lista.
pub fn encaja(nombre: &str, atajos: &[&str], q: &str) -> bool {
    let q = plano(q.trim_start_matches('/'));
    if q.is_empty() {
        return true;
    }
    plano(nombre).split_whitespace().any(|p| p.starts_with(&q))
        || atajos.iter().any(|a| plano(a).starts_with(&q))
}

/// Los bloques que encajan con lo tecleado, en el orden del catalogo.
/// `nombre` da el nombre de cada uno en el idioma de la aplicacion.
pub fn buscar(q: &str, nombre: &dyn Fn(Bloque) -> String) -> Vec<Bloque> {
    CATALOGO
        .iter()
        .filter(|(b, atajos)| encaja(&nombre(*b), atajos, q))
        .map(|(b, _)| *b)
        .collect()
}

/// Lo que se escribe al elegir un bloque de texto: lo de antes del cursor y
/// lo de despues. `None` para los que no son texto (tabla, imagen, fecha),
/// que pone el editor.
pub fn plantilla(b: Bloque) -> Option<(&'static str, &'static str)> {
    Some(match b {
        Bloque::Titulo1 => ("# ", ""),
        Bloque::Titulo2 => ("## ", ""),
        Bloque::Titulo3 => ("### ", ""),
        Bloque::Lista => ("- ", ""),
        Bloque::Numerada => ("1. ", ""),
        Bloque::Casillas => ("- [ ] ", ""),
        Bloque::Cita => ("> ", ""),
        Bloque::Codigo => ("```\n", "\n```"),
        Bloque::Separador => ("---\n", ""),
        Bloque::Tabla
        | Bloque::Imagen
        | Bloque::Fecha
        | Bloque::Pagina
        | Bloque::EnlaceHoja
        | Bloque::Documento
        | Bloque::DelChat
        | Bloque::Audio => return None,
    })
}

/// Cambia lo que va de `desde` a `hasta` (la barra y lo tecleado) por
/// `antes` + `despues`, con el cursor entre los dos (`Comandos.insertar`).
/// Devuelve el texto nuevo y el cursor.
pub fn poner(
    texto: &str,
    desde: usize,
    hasta: usize,
    antes: &str,
    despues: &str,
) -> (String, usize) {
    let u: Vec<u16> = texto.encode_utf16().collect();
    let a = desde.min(u.len());
    let b = hasta.clamp(a, u.len());
    let cabeza = format!("{}{antes}", String::from_utf16_lossy(&u[..a]));
    let cursor = cabeza.encode_utf16().count();
    (
        format!("{cabeza}{despues}{}", String::from_utf16_lossy(&u[b..])),
        cursor,
    )
}

/// La fecha de hoy como la pone el movil (`d MMM yyyy`): «29 sept 2026».
/// `meses` son las doce abreviaturas del idioma.
pub fn fecha(dia: u32, mes: u32, anio: i32, meses: &[String]) -> String {
    let m = meses
        .get(mes.saturating_sub(1) as usize)
        .cloned()
        .unwrap_or_else(|| format!("{mes:02}"));
    format!("{dia} {m} {anio}")
}

/// Dia, mes y ano de unos milisegundos desde 1970 ya en hora local
/// (`pixpin_shell::entorno::ahora_local_ms`). Calendario civil de Howard
/// Hinnant: sin tablas ni dependencias.
pub fn dia_de(ms_local: i64) -> (u32, u32, i32) {
    let dias = ms_local.div_euclid(86_400_000);
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = (yoe + era * 400 + if m <= 2 { 1 } else { 0 }) as i32;
    (d, m, y)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn nombre(b: Bloque) -> String {
        match b {
            Bloque::Titulo1 => "Título 1",
            Bloque::Titulo2 => "Título 2",
            Bloque::Titulo3 => "Título 3",
            Bloque::Lista => "Lista con viñetas",
            Bloque::Numerada => "Lista numerada",
            Bloque::Casillas => "Casillas",
            Bloque::Cita => "Cita",
            Bloque::Codigo => "Código",
            Bloque::Tabla => "Tabla",
            Bloque::Imagen => "Imagen",
            Bloque::Fecha => "Fecha de hoy",
            Bloque::Separador => "Separador",
            Bloque::Pagina => "Página de un proyecto",
            Bloque::EnlaceHoja => "Enlace a una hoja",
            Bloque::Documento => "Documento",
            Bloque::DelChat => "Del chat",
            Bloque::Audio => "Audio",
        }
        .into()
    }

    #[test]
    fn la_barra_al_principio_del_renglon_abre_el_menu() {
        assert_eq!(consulta("/", 1), Some((0, String::new())));
        assert_eq!(consulta("hola\n/tab", 9), Some((5, "tab".into())));
        assert_eq!(consulta("hola\r/t1", 8), Some((5, "t1".into())));
    }

    #[test]
    fn una_barra_en_mitad_de_la_frase_no_abre_nada() {
        assert_eq!(consulta("el 12/03", 8), None);
        assert_eq!(consulta("ver a/b", 7), None);
        assert_eq!(
            consulta("/tabla y mas", 12),
            None,
            "con un espacio ya no es comando"
        );
        assert_eq!(consulta("sin barra", 4), None);
    }

    #[test]
    fn lo_tecleado_filtra_por_el_principio_de_cada_palabra() {
        assert_eq!(buscar("tab", &nombre), vec![Bloque::Tabla]);
        assert_eq!(
            buscar("titulo", &nombre),
            vec![Bloque::Titulo1, Bloque::Titulo2, Bloque::Titulo3]
        );
        assert_eq!(buscar("num", &nombre), vec![Bloque::Numerada]);
        assert_eq!(buscar("hoy", &nombre), vec![Bloque::Fecha]);
        assert_eq!(buscar("", &nombre).len(), CATALOGO.len());
        // En ingles tambien, aunque la aplicacion este en espanol.
        assert_eq!(buscar("table", &nombre), vec![Bloque::Tabla]);
        assert_eq!(buscar("check", &nombre), vec![Bloque::Casillas]);
    }

    #[test]
    fn documento_del_chat_y_audio_salen_en_la_barra_como_en_el_movil() {
        assert_eq!(buscar("archivo", &nombre), vec![Bloque::Documento]);
        assert_eq!(buscar("pdf", &nombre), vec![Bloque::Documento]);
        assert_eq!(buscar("chat", &nombre), vec![Bloque::DelChat]);
        assert_eq!(buscar("musica", &nombre), vec![Bloque::Audio]);
        assert_eq!(buscar("voz", &nombre), vec![Bloque::Audio]);
        // Los pone el editor, no son texto.
        assert_eq!(plantilla(Bloque::Documento), None);
        assert_eq!(plantilla(Bloque::Audio), None);
        // Caso negativo: la tabla sigue sola con «tab».
        assert_eq!(buscar("tab", &nombre), vec![Bloque::Tabla]);
    }

    #[test]
    fn la_pagina_de_un_proyecto_y_su_enlace_salen_en_la_barra() {
        assert_eq!(buscar("pag", &nombre), vec![Bloque::Pagina]);
        assert_eq!(buscar("lienzo", &nombre), vec![Bloque::Pagina]);
        assert_eq!(buscar("sheet", &nombre), vec![Bloque::Pagina]);
        assert_eq!(buscar("enlace", &nombre), vec![Bloque::EnlaceHoja]);
        assert_eq!(plantilla(Bloque::Pagina), None);
        // Caso negativo: «hoy» sigue siendo solo la fecha.
        assert_eq!(buscar("hoy", &nombre), vec![Bloque::Fecha]);
    }

    #[test]
    fn lo_que_no_encaja_deja_la_lista_vacia() {
        assert!(buscar("zzz", &nombre).is_empty());
        // «ista» esta dentro de «Lista» pero no al principio.
        assert!(buscar("ista", &nombre).is_empty());
    }

    #[test]
    fn elegir_cambia_la_barra_por_el_bloque() {
        let (t, c) = poner("hola\n/cas", 5, 9, "- [ ] ", "");
        assert_eq!(t, "hola\n- [ ] ");
        assert_eq!(c, 11);
        let (t, c) = poner("/co", 0, 3, "```\n", "\n```");
        assert_eq!(t, "```\n\n```");
        assert_eq!(c, 4);
        assert_eq!(plantilla(Bloque::Tabla), None);
    }

    #[test]
    fn la_fecha_va_como_la_pone_el_movil() {
        let meses: Vec<String> = "ene feb mar abr may jun jul ago sept oct nov dic"
            .split(' ')
            .map(String::from)
            .collect();
        assert_eq!(fecha(29, 9, 2026, &meses), "29 sept 2026");
        assert_eq!(fecha(1, 13, 2026, &meses), "1 13 2026");
    }

    #[test]
    fn el_dia_sale_de_los_milisegundos() {
        assert_eq!(dia_de(0), (1, 1, 1970));
        // 2026-09-29 12:00 UTC.
        assert_eq!(dia_de(1_790_683_200_000), (29, 9, 2026));
        assert_eq!(dia_de(951_782_400_000), (29, 2, 2000));
    }
}
