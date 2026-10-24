use super::*;

/// La posicion UTF-16 de la primera vez que sale `trozo`.
fn en(t: &str, trozo: &str) -> usize {
    t[..t.find(trozo).unwrap()].encode_utf16().count()
}

#[test]
fn el_cursor_no_se_queda_delante_de_la_marca_de_un_titulo_ni_dentro_de_una_marca() {
    assert_eq!(cursor_valido("# Titulo", 0), 2);
    assert_eq!(cursor_valido("# Titulo", 1), 2);
    assert_eq!(cursor_valido("uno\n- [ ] tarea", 4), 10);
    // Entre los dos asteriscos de `**`: detras de la marca.
    assert_eq!(cursor_valido("a **b** c", 3), 4);
    // Casos negativos: a un lado de la marca o en texto llano, se queda.
    assert_eq!(cursor_valido("a **b** c", 2), 2);
    assert_eq!(cursor_valido("a **b** c", 7), 7);
    assert_eq!(cursor_valido("hola", 2), 2);
    // Dentro de un bloque de codigo no hay marcas.
    assert_eq!(cursor_valido("```\n# no\n```", 4), 4);
}

#[test]
fn borrar_la_ultima_letra_de_una_negrita_no_deja_asteriscos_sueltos() {
    // Con el cursor delante o detras de la marca de cierre: lo mismo.
    assert_eq!(tecla_borrar("a **b** c", 5, true), Some(("a  c".into(), 2)));
    assert_eq!(tecla_borrar("a **b** c", 7, true), Some(("a  c".into(), 2)));
    // Con mas letras dentro, la marca se queda.
    assert_eq!(tecla_borrar("**ab**", 4, true), Some(("**a**".into(), 3)));
    // Cursiva dentro de negrita: se van las dos.
    assert_eq!(
        tecla_borrar("x ***y*** z", 6, true),
        Some(("x  z".into(), 2))
    );
    // Un enlace sin texto se va entero, direccion incluida.
    assert_eq!(
        tecla_borrar("ver [a](https://x.es)", 6, true),
        Some(("ver ".into(), 4))
    );
}

#[test]
fn supr_borra_la_letra_que_se_ve_saltando_las_marcas() {
    assert_eq!(tecla_borrar("a**b**", 1, false), Some(("a".into(), 1)));
    assert_eq!(
        tecla_borrar("a**bc**", 1, false),
        Some(("a**c**".into(), 1))
    );
}

#[test]
fn retroceso_al_principio_de_un_titulo_o_una_lista_lo_vuelve_parrafo() {
    assert_eq!(tecla_borrar("# Hola", 2, true), Some(("Hola".into(), 0)));
    assert_eq!(
        tecla_borrar("uno\n- dos", 6, true),
        Some(("uno\ndos".into(), 4))
    );
    assert_eq!(
        tecla_borrar("- [x] hecha", 6, true),
        Some(("hecha".into(), 0))
    );
    // Ya parrafo: se junta con el de arriba.
    assert_eq!(
        tecla_borrar("uno\ndos", 4, true),
        Some(("unodos".into(), 3))
    );
    // Caso negativo: al principio de la nota no hay nada que hacer.
    assert_eq!(tecla_borrar("hola", 0, true), None);
}

#[test]
fn supr_al_final_de_un_renglon_junta_el_de_abajo_sin_su_marca() {
    assert_eq!(
        tecla_borrar("uno\n# dos", 3, false),
        Some(("unodos".into(), 3))
    );
    assert_eq!(
        tecla_borrar("uno\n- [ ] dos", 3, false),
        Some(("unodos".into(), 3))
    );
    // Caso negativo: al final de la nota.
    assert_eq!(tecla_borrar("uno", 3, false), None);
}

#[test]
fn retroceso_tras_una_foto_o_una_raya_la_quita_entera() {
    let t = "![x](a.png)\ntexto";
    assert_eq!(
        tecla_borrar(t, en(t, "texto"), true),
        Some(("texto".into(), 0))
    );
    assert_eq!(
        tecla_borrar("uno\n---\ndos", 8, true),
        Some(("uno\ndos".into(), 4))
    );
}

#[test]
fn una_tabla_la_borra_el_control_y_una_valla_no_se_junta() {
    use crate::md_tabla::{CELDA, FILA_ABRE, FILA_CIERRA};
    let t = format!("{FILA_ABRE}\ra{CELDA}b{CELDA}{FILA_CIERRA}\rfin");
    assert_eq!(tecla_borrar(&t, 1 + 1 + 1, true), None);
    let c = "```\ncodigo\n```";
    assert_eq!(tecla_borrar(c, 4, true), Some((c.to_string(), 4)));
}

#[test]
fn un_emoji_se_borra_entero() {
    assert_eq!(tecla_borrar("a😀", 3, true), Some(("a".into(), 1)));
    assert_eq!(tecla_borrar("a😀", 1, false), Some(("a".into(), 1)));
}

#[test]
fn borrar_lo_elegido_quita_solo_lo_que_se_ve() {
    let t = "a **bold** c";
    // «ld c» a la vista: la negrita se queda con «bo».
    assert_eq!(
        borrar(t, en(t, "ld"), t.encode_utf16().count()),
        ("a **bo**".into(), 6)
    );
    assert_eq!(borrar(t, 0, en(t, "ld")), ("**ld** c".into(), 0));
    // Todo: no queda nada.
    assert_eq!(borrar(t, 0, 12), (String::new(), 0));
    // Borrar el salto se lleva la marca del titulo de abajo.
    assert_eq!(borrar("uno\n# dos", 2, 7), ("unos".into(), 2));
}

#[test]
fn borrar_la_letra_escapada_se_lleva_su_barra() {
    let t = r"precio \*5";
    assert_eq!(borrar(t, 8, 9), ("precio 5".into(), 7));
}

#[test]
fn la_negrita_se_pone_sobre_lo_elegido_sin_sus_blancos() {
    assert_eq!(
        alternar("hola mundo", 5, 10, "**"),
        Some(("hola **mundo**".into(), 7, 12))
    );
    // Doble clic: la palabra con su espacio.
    assert_eq!(
        alternar("hola mundo fin", 5, 11, "**"),
        Some(("hola **mundo** fin".into(), 7, 12))
    );
    assert_eq!(
        alternar("# Hola", 0, 6, "*"),
        Some(("# *Hola*".into(), 3, 7))
    );
    // Un renglon cada vez.
    assert_eq!(
        alternar("uno\ndos", 0, 7, "~~"),
        Some(("~~uno~~\n~~dos~~".into(), 2, 13))
    );
    // Caso negativo: solo blancos.
    assert_eq!(alternar("a   b", 1, 4, "**"), None);
    // Caso negativo: una marca que no es de letra.
    assert_eq!(alternar("hola", 0, 4, "#"), None);
}

#[test]
fn la_negrita_se_quita_si_ya_la_tiene_con_o_sin_sus_marcas_elegidas() {
    let t = "hola **mundo**";
    assert_eq!(alternar(t, 7, 12, "**"), Some(("hola mundo".into(), 5, 10)));
    assert_eq!(alternar(t, 5, 14, "**"), Some(("hola mundo".into(), 5, 10)));
}

#[test]
fn quitar_la_negrita_a_un_trozo_parte_el_tramo() {
    let t = "**hola mundo**";
    let (n, a, b) = alternar(t, en(t, "mundo"), 12, "**").unwrap();
    assert_eq!(n, "**hola** mundo");
    assert_eq!(&n[a..b], "mundo");
    let (n, a, b) = alternar("**abc**", 3, 4, "**").unwrap();
    assert_eq!(n, "**a**b**c**");
    assert_eq!(&n[a..b], "b");
}

#[test]
fn poner_negrita_junto_a_otra_las_junta() {
    let t = "**ab**cd";
    assert_eq!(alternar(t, 3, 8, "**"), Some(("**abcd**".into(), 2, 6)));
}

#[test]
fn sin_nada_elegido_quita_el_formato_del_tramo_del_cursor() {
    assert_eq!(
        alternar("a **bc** d", 5, 5, "**"),
        Some(("a bc d".into(), 3, 3))
    );
    // Caso negativo: fuera de un tramo no hay nada que hacer.
    assert_eq!(alternar("a **bc** d", 9, 9, "**"), None);
    assert_eq!(alternar("a *bc* d", 4, 4, "**"), None);
}

#[test]
fn quitar_formato_deja_el_texto_y_los_enlaces() {
    let t = "a **b** *c* ~~d~~ `e` [f](https://x.es)";
    let n = t.encode_utf16().count();
    let (q, _, _) = quitar_formato(t, 0, n).unwrap();
    assert_eq!(q, "a b c d e [f](https://x.es)");
    assert_eq!(quitar_formato("sin nada", 0, 8), None);
}

#[test]
fn corchetes_al_principio_se_vuelven_casilla_al_vuelo() {
    assert_eq!(convertir("[] ", 3), Some(("- [ ] ".into(), 6)));
    assert_eq!(
        convertir("uno\n[ ] x", 8),
        Some(("uno\n- [ ] x".into(), 10))
    );
    assert_eq!(convertir("[x] ", 4), Some(("- [x] ".into(), 6)));
    // Casos negativos: en mitad del renglon, o ya en una lista.
    assert_eq!(convertir("ver [] ", 7), None);
    assert_eq!(convertir("- [] ", 5), None);
}

#[test]
fn el_bloque_de_cada_renglon() {
    assert_eq!(bloque_de("# a\n- [ ] b\nc\n1. d", 0), "# ");
    assert_eq!(bloque_de("# a\n- [ ] b\nc\n1. d", 1), "- [ ] ");
    assert_eq!(bloque_de("# a\n- [ ] b\nc\n1. d", 2), "");
    assert_eq!(bloque_de("# a\n- [ ] b\nc\n1. d", 3), "1. ");
    assert_eq!(bloque_de("x", 7), "");
}

#[test]
fn se_esconden_las_marcas_las_rayas_y_el_texto_de_las_fotos() {
    let t = "a **b**\n---\n![x](a.png)";
    let v = escondidas(t);
    let visto: String = t
        .chars()
        .zip(&v)
        .filter(|(_, e)| !**e)
        .map(|(c, _)| c)
        .collect();
    assert_eq!(visto, "a b\n\n");
    // Caso negativo: un parrafo llano no esconde nada.
    assert!(escondidas("hola mundo").iter().all(|e| !e));
}

#[test]
fn escribir_una_letra_que_cierra_o_abre_una_marca_cambia_lo_escondido() {
    // Cerrar una negrita, abrir un titulo, una lista o una casilla con el
    // espacio, la tercera raya de un separador.
    assert!(letra_cambia_lo_escondido("a **b*", 6, '*'));
    assert!(letra_cambia_lo_escondido("#", 1, ' '));
    assert!(letra_cambia_lo_escondido("x\n-", 3, ' '));
    assert!(letra_cambia_lo_escondido("--", 2, '-'));
    assert!(letra_cambia_lo_escondido("1.", 2, ' '));
    // Casos negativos: una letra normal en un parrafo, dentro de una
    // negrita ya cerrada o detras de un titulo no cambia nada.
    assert!(!letra_cambia_lo_escondido("hola", 4, 'a'));
    assert!(!letra_cambia_lo_escondido("a **bc** d", 5, 'x'));
    assert!(!letra_cambia_lo_escondido("# Ti", 4, 't'));
    assert!(!letra_cambia_lo_escondido("uno\ndos", 7, ' '));
}
