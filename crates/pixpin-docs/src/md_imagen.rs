//! **Las imagenes de una nota, con su ancho, y las paginas vivas** (H12):
//! las cuentas puras sobre el renglon `![texto](ruta)`, sin ventanas.
//!
//! # El ancho, en el texto de la foto
//!
//! El movil pinta una foto a todo lo ancho y su texto debajo, de pie
//! (`MarkdownText.MedioUi`), y reconoce la foto solo si el renglon es
//! exactamente `![texto](ruta)` con la ruta acabada en `.png`, `.jpg`…
//! (`Markdown.MEDIO`, `claseDeMedio`). Por eso **ni** `{width=…}` **ni**
//! `<img width>` **ni** un titulo `"…"` detras de la ruta: el movil no
//! entiende el HTML (solo `<table`), y un titulo o un `?ancho=` se quedan
//! pegados a la ruta, que deja de ser una foto y ademas la sincronizacion
//! (`Rutas.enTexto`, que corta en `"` o `)`) ya no encuentra el fichero.
//!
//! El ancho va **en el texto**, con la forma de Obsidian:
//! `![Planta|320](ruta)`. El movil la sigue viendo como foto y lo unico que
//! cambia alli es el pie (`Planta|320`) hasta que aprenda a quitarlo (ver
//! `docs/investigacion/2026-09-30-paginas-vivas-android.md`). Solo se
//! escribe si el usuario cambia el tamano: una foto a su tamano no lleva
//! nada.
//!
//! # La pagina viva, en el nombre del fichero
//!
//! Una hoja de un proyecto metida como imagen que se actualiza es una foto
//! normal cuyo fichero se llama `vivo-<hoja>.png`, con `<hoja>` el codigo
//! unico de la hoja (diez signos, `codigos::LARGO`, el mismo en el movil y
//! aqui). El PC la vuelve a pintar cuando la hoja cambia; el movil ve la
//! ultima copia, que viaja como cualquier foto de la nota. Asi no hace falta
//! ninguna marca nueva en el Markdown.
//!
//! # El enlace a una hoja
//!
//! `[Planta baja](pixpin:hoja=<proyecto>/<hoja>)`: un enlace de los de
//! siempre, con los codigos unicos del proyecto y de la hoja.

use crate::md_vivo;

/// El principio del nombre de fichero de una pagina viva.
pub const PREFIJO_VIVA: &str = "vivo-";
/// El esquema de los enlaces a una hoja.
pub const ESQUEMA_HOJA: &str = "pixpin:hoja=";
/// Lo mas estrecha que se deja una foto, en pixeles a 96 ppp.
pub const ANCHO_MINIMO: u32 = 48;
/// El ancho de la columna de texto, a 96 ppp: una foto asi de ancha o mas
/// es «a su tamano» y no lleva ancho.
pub const ANCHO_COLUMNA: u32 = 720;

/// Una foto de la nota: su texto (sin el ancho), su ancho si se cambio y
/// su ruta tal cual va en el Markdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Foto {
    pub alt: String,
    pub ancho: Option<u32>,
    pub ruta: String,
}

/// Separa `Planta|320` en el texto y el ancho. Solo cuenta un numero de
/// verdad tras la ultima barra: `a|b` o `cota 1|2 m` siguen siendo texto.
fn partir_alt(alt: &str) -> (String, Option<u32>) {
    if let Some((a, n)) = alt.rsplit_once('|')
        && !n.is_empty()
        && n.len() <= 5
        && n.bytes().all(|b| b.is_ascii_digit())
        && let Ok(v) = n.parse::<u32>()
        && v > 0
    {
        return (a.trim_end().to_string(), Some(v));
    }
    (alt.to_string(), None)
}

/// La foto de un renglon, si es uno de foto (`md_vivo::imagen_de`).
pub fn leer(renglon: &str) -> Option<Foto> {
    let (alt, ruta) = md_vivo::imagen_de(renglon)?;
    let (alt, ancho) = partir_alt(&alt);
    Some(Foto { alt, ancho, ruta })
}

/// Sin lo que romperia el `![…]`: corchetes y saltos de renglon.
fn texto_limpio(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '[' | ']' | '\n' | '\r'))
        .collect::<String>()
        .trim()
        .to_string()
}

/// El renglon de una foto.
pub fn escribir(f: &Foto) -> String {
    let ancho = f.ancho.map(|a| format!("|{a}")).unwrap_or_default();
    format!("![{}{ancho}]({})", texto_limpio(&f.alt), f.ruta.trim())
}

/// El ancho que se guarda para uno pedido: dentro de los topes, y nada si
/// llega a la columna (es el tamano de siempre).
pub fn ancho_guardado(pedido: u32) -> Option<u32> {
    let a = pedido.max(ANCHO_MINIMO);
    (a < ANCHO_COLUMNA).then_some(a)
}

/// Las fotos de la nota, con su renglon (desde 0). Las de dentro de un
/// bloque de codigo no cuentan (`md_vivo::imagenes`).
pub fn fotos(texto: &str) -> Vec<(usize, Foto)> {
    let renglones: Vec<&str> = texto.split(['\n', '\r']).collect();
    md_vivo::imagenes(texto)
        .into_iter()
        .filter_map(|(n, _)| Some((n, leer(renglones.get(n)?)?)))
        .collect()
}

/// Cambia el renglon `linea` de `texto` (que tiene que ser una foto) con
/// `f`. Conserva la sangria que tuviera. `None` si ese renglon no es una
/// foto.
pub fn cambiar(texto: &str, linea: usize, f: impl FnOnce(&mut Foto)) -> Option<String> {
    let mut renglones: Vec<String> = texto.split('\n').map(str::to_string).collect();
    let r = renglones.get_mut(linea)?;
    let mut foto = leer(r)?;
    f(&mut foto);
    let sangria: String = r.chars().take_while(|c| c.is_whitespace()).collect();
    *r = format!("{sangria}{}", escribir(&foto));
    Some(renglones.join("\n"))
}

/// Pone (o quita, con `None`) el ancho de la foto del renglon `linea`.
pub fn con_ancho(texto: &str, linea: usize, ancho: Option<u32>) -> Option<String> {
    cambiar(texto, linea, |f| f.ancho = ancho.and_then(ancho_guardado))
}

/// Quita el renglon de la foto `linea`, con su salto. `None` si no es una
/// foto.
pub fn quitar(texto: &str, linea: usize) -> Option<String> {
    let mut renglones: Vec<&str> = texto.split('\n').collect();
    leer(renglones.get(linea)?)?;
    renglones.remove(linea);
    Some(renglones.join("\n"))
}

/// Si un nombre de fichero es de una foto que la nota sabe ensenar.
pub fn es_foto(nombre: &str) -> bool {
    md_vivo::imagen_de(&format!("![x]({})", nombre.replace([' ', '(', ')'], "_"))).is_some()
}

/// El nombre de fichero de la pagina viva de una hoja.
pub fn fichero_de_viva(hoja: &str) -> String {
    let limpio: String = hoja
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{PREFIJO_VIVA}{limpio}.png")
}

/// La hoja de una pagina viva, por la ruta de su foto: `…/vivo-<hoja>.png`.
/// `None` si es una foto de las de siempre.
pub fn hoja_de_viva(ruta: &str) -> Option<String> {
    let nombre = ruta.rsplit(['/', '\\']).next()?;
    let hoja = nombre.strip_prefix(PREFIJO_VIVA)?.strip_suffix(".png")?;
    (!hoja.is_empty()
        && hoja
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
    .then(|| hoja.to_string())
}

/// La direccion de un enlace a una hoja.
pub fn direccion_de_hoja(proyecto: &str, hoja: &str) -> String {
    format!("{ESQUEMA_HOJA}{proyecto}/{hoja}")
}

/// El proyecto y la hoja de un enlace `pixpin:hoja=<proyecto>/<hoja>`.
pub fn hoja_del_enlace(url: &str) -> Option<(String, String)> {
    let resto = url.trim().strip_prefix(ESQUEMA_HOJA)?;
    let (p, h) = resto.split_once('/')?;
    let bien = |s: &str| !s.is_empty() && !s.contains(['/', '\\', ' ', ')', '(']);
    (bien(p) && bien(h)).then(|| (p.to_string(), h.to_string()))
}

/// El enlace Markdown a una hoja, con su nombre de texto.
pub fn enlace_a_hoja(nombre: &str, proyecto: &str, hoja: &str) -> String {
    let nombre = match texto_limpio(nombre) {
        n if n.is_empty() => hoja.to_string(),
        n => n,
    };
    format!("[{nombre}]({})", direccion_de_hoja(proyecto, hoja))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const R: &str = "pixpin:files/guardados/pc/p1/notas/1-planta.png";

    #[test]
    fn el_ancho_va_tras_la_ultima_barra_del_texto_y_solo_si_es_un_numero() {
        assert_eq!(
            leer(&format!("![Planta|320]({R})")),
            Some(Foto {
                alt: "Planta".into(),
                ancho: Some(320),
                ruta: R.into()
            })
        );
        // Casos negativos: barras que son texto.
        assert_eq!(leer(&format!("![a|b]({R})")).unwrap().ancho, None);
        assert_eq!(
            leer(&format!("![cota 1|2 m]({R})")).unwrap().alt,
            "cota 1|2 m"
        );
        assert_eq!(leer(&format!("![x|0]({R})")).unwrap().ancho, None);
        assert_eq!(leer(&format!("![x|]({R})")).unwrap().alt, "x|");
        // Ni una foto en mitad de una frase ni un PDF.
        assert_eq!(leer(&format!("mira ![x]({R})")), None);
        assert_eq!(leer("![x](plano.pdf)"), None);
    }

    #[test]
    fn leer_y_escribir_una_foto_da_el_mismo_renglon() {
        for r in [
            format!("![Planta|320]({R})"),
            format!("![Planta]({R})"),
            format!("![]({R})"),
        ] {
            assert_eq!(escribir(&leer(&r).unwrap()), r);
        }
    }

    #[test]
    fn el_renglon_sigue_siendo_una_foto_para_el_movil() {
        // Lo que mira `Markdown.MEDIO` del movil: `^!\[([^]]*)]\(([^)]*)\)$`
        // y la extension de la ruta. Con ancho, la ruta no cambia.
        let r = escribir(&Foto {
            alt: "Planta [baja]".into(),
            ancho: Some(300),
            ruta: R.into(),
        });
        assert_eq!(r, format!("![Planta baja|300]({R})"));
        let (alt, ruta) = r.strip_prefix("![").unwrap().split_once("](").unwrap();
        assert!(!alt.contains(']'));
        assert_eq!(ruta.strip_suffix(')').unwrap(), R);
        assert!(!ruta.contains(['"', ' ', '?']));
    }

    #[test]
    fn el_ancho_tiene_topes_y_a_la_columna_no_se_escribe() {
        assert_eq!(ancho_guardado(10), Some(ANCHO_MINIMO));
        assert_eq!(ancho_guardado(400), Some(400));
        assert_eq!(ancho_guardado(ANCHO_COLUMNA), None);
        assert_eq!(ancho_guardado(5000), None);
    }

    #[test]
    fn cambiar_el_ancho_toca_solo_ese_renglon_y_conserva_la_sangria() {
        let t = format!("# Obra\n  ![Planta]({R})\nfin");
        assert_eq!(
            con_ancho(&t, 1, Some(300)).unwrap(),
            format!("# Obra\n  ![Planta|300]({R})\nfin")
        );
        let con = format!("![Planta|300]({R})");
        assert_eq!(con_ancho(&con, 0, None).unwrap(), format!("![Planta]({R})"));
        // Casos negativos: un renglon que no es foto, o que no existe.
        assert_eq!(con_ancho(&t, 0, Some(300)), None);
        assert_eq!(con_ancho(&t, 9, Some(300)), None);
    }

    #[test]
    fn quitar_una_foto_quita_su_renglon_y_nada_mas() {
        let t = format!("a\n![x]({R})\nb");
        assert_eq!(quitar(&t, 1).unwrap(), "a\nb");
        assert_eq!(quitar(&t, 0), None);
    }

    #[test]
    fn las_fotos_de_la_nota_traen_su_ancho_y_no_las_del_codigo() {
        let t = format!("![a|200]({R})\n```\n![b]({R})\n```\ntexto\n![c]({R})");
        let v = fotos(&t);
        assert_eq!(v.len(), 2);
        assert_eq!((v[0].0, v[0].1.ancho), (0, Some(200)));
        assert_eq!((v[1].0, v[1].1.alt.as_str()), (5, "c"));
    }

    #[test]
    fn una_pagina_viva_se_reconoce_por_el_nombre_de_su_fichero() {
        assert_eq!(fichero_de_viva("K7Q2ABCDEF"), "vivo-K7Q2ABCDEF.png");
        assert_eq!(
            hoja_de_viva("pixpin:files/guardados/pc/p1/notas/vivo-K7Q2ABCDEF.png"),
            Some("K7Q2ABCDEF".into())
        );
        assert_eq!(hoja_de_viva("adjuntos\\vivo-AB_2.png"), Some("AB_2".into()));
        // Casos negativos: una foto de las de siempre, otra extension, vacia.
        assert_eq!(hoja_de_viva("notas/1-vivo-x.png"), None);
        assert_eq!(hoja_de_viva("notas/vivo-x.jpg"), None);
        assert_eq!(hoja_de_viva("notas/vivo-.png"), None);
        // Un codigo con letras raras no rompe el nombre.
        assert_eq!(fichero_de_viva("a/b:c"), "vivo-a_b_c.png");
    }

    #[test]
    fn el_enlace_a_una_hoja_va_y_vuelve() {
        let e = enlace_a_hoja("Planta [baja]", "P9", "K7Q2");
        assert_eq!(e, "[Planta baja](pixpin:hoja=P9/K7Q2)");
        assert_eq!(
            hoja_del_enlace("pixpin:hoja=P9/K7Q2"),
            Some(("P9".into(), "K7Q2".into()))
        );
        // El enlace se lee como un enlace de siempre.
        assert_eq!(
            md_vivo::enlace_en(&e, 2).as_deref(),
            Some("pixpin:hoja=P9/K7Q2")
        );
        // Casos negativos.
        assert_eq!(hoja_del_enlace("https://x.es"), None);
        assert_eq!(hoja_del_enlace("pixpin:hoja=P9"), None);
        assert_eq!(hoja_del_enlace("pixpin:hoja=/K7"), None);
        assert_eq!(hoja_del_enlace("pixpin:hoja=P9/a/b"), None);
        assert_eq!(enlace_a_hoja("", "P9", "K7"), "[K7](pixpin:hoja=P9/K7)");
    }

    #[test]
    fn solo_se_toman_por_fotos_los_ficheros_de_imagen() {
        assert!(es_foto("captura (1).PNG"));
        assert!(es_foto("obra.jpeg"));
        assert!(!es_foto("plano.pdf"));
        assert!(!es_foto("sin-extension"));
    }
}
