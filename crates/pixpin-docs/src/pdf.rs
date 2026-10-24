//! **Un Word o un libro pasados a PDF**, que es el «Word a PDF» del movil
//! (v0.63, `android/print/PdfDesdeWeb.kt`).
//!
//! En Android eso lo hace el servicio de impresion del sistema, que sabe
//! maquetar una pagina web. En Windows no hay nada equivalente que se pueda
//! llamar sin abrir un cuadro de dialogo ni arrastrar un navegador entero,
//! asi que el PDF se escribe aqui: A4, margenes de dos centimetros, y el
//! texto del documento con su negrita, su cursiva y sus titulos.
//!
//! **Que NO lleva, y a proposito:** imagenes, colores, tablas con rejilla y
//! nada que no sea texto. Esto es «tener el Word en PDF para anotarlo
//! encima en el proyecto», que es para lo que el movil lo hacia, no un
//! maquetador. Quien quiera la pagina tal cual la ve, guarda el HTML y
//! usa «Imprimir → Guardar como PDF» del navegador, que si maqueta.
//!
//! Las letras son las catorce de siempre (Helvetica y Courier), que todo
//! lector de PDF trae puestas: asi el fichero no tiene que incrustar
//! ninguna tipografia y pesa lo que pesa el texto.

use crate::documento::{Alineacion, Bloque, Clase, Documento, Estilo, MARCA_IMAGEN, Trozo};

/// A4 en puntos (1/72 de pulgada), que es la unidad del PDF.
const ANCHO: f32 = 595.28;
const ALTO: f32 = 841.89;
/// Dos centimetros.
const MARGEN: f32 = 56.7;

/// Las cinco letras que se usan, con el numero de objeto que les toca y el
/// nombre con el que se piden dentro de la pagina.
const LETRAS: [(&str, &str); 5] = [
    ("F1", "Helvetica"),
    ("F2", "Helvetica-Bold"),
    ("F3", "Helvetica-Oblique"),
    ("F4", "Helvetica-BoldOblique"),
    ("F5", "Courier"),
];

/// El documento entero como PDF.
pub fn de_documento(d: &Documento) -> Vec<u8> {
    let paginas = maquetar(d);
    escribir(&paginas)
}

/// Una linea ya colocada: de donde arranca y que hay en ella.
struct Pieza {
    texto: String,
    /// Indice en [`LETRAS`].
    letra: usize,
    tamano: f32,
    x: f32,
    y: f32,
}

type Pagina = Vec<Pieza>;

/// El tamano y la letra de cada clase de bloque.
fn pinta(clase: Clase) -> (f32, bool, bool, bool) {
    // (tamano, negrita, cursiva, monoespaciada)
    match clase {
        Clase::Titulo(1) => (20.0, true, false, false),
        Clase::Titulo(2) => (16.0, true, false, false),
        Clase::Titulo(3) => (13.5, true, false, false),
        Clase::Titulo(_) => (11.5, true, false, false),
        Clase::Cita => (11.0, false, true, false),
        Clase::Codigo => (9.5, false, false, true),
        Clase::Nota => (10.0, false, true, false),
        _ => (11.0, false, false, false),
    }
}

fn maquetar(d: &Documento) -> Vec<Pagina> {
    let mut paginas: Vec<Pagina> = vec![Vec::new()];
    let mut y = ALTO - MARGEN;
    let util = ANCHO - MARGEN * 2.0;

    let mut nueva_pagina = |paginas: &mut Vec<Pagina>, y: &mut f32| {
        paginas.push(Vec::new());
        *y = ALTO - MARGEN;
    };

    if !d.titulo.trim().is_empty() {
        let bloque = Bloque::nuevo(Clase::Titulo(1), vec![Trozo::llano(d.titulo.trim())]);
        colocar(&bloque, util, &mut y, &mut paginas, &mut nueva_pagina);
        if !d.autor.trim().is_empty() {
            let autor = Bloque::nuevo(Clase::Nota, vec![Trozo::llano(d.autor.trim())]);
            colocar(&autor, util, &mut y, &mut paginas, &mut nueva_pagina);
        }
    }

    for b in &d.bloques {
        if b.clase == Clase::Capitulo {
            // Cada capitulo, en pagina nueva: es lo que hace la hoja de
            // impresion del movil.
            if !paginas.last().is_some_and(|p| p.is_empty()) {
                nueva_pagina(&mut paginas, &mut y);
            }
            continue;
        }
        if b.clase == Clase::Nota && b.texto() == MARCA_IMAGEN {
            let aviso = Bloque::nota("[imagen — el PDF de PixPin es solo texto]");
            colocar(&aviso, util, &mut y, &mut paginas, &mut nueva_pagina);
            continue;
        }
        colocar(b, util, &mut y, &mut paginas, &mut nueva_pagina);
    }
    if paginas.len() > 1 && paginas.last().is_some_and(|p| p.is_empty()) {
        paginas.pop();
    }
    paginas
}

fn colocar(
    b: &Bloque,
    util: f32,
    y: &mut f32,
    paginas: &mut Vec<Pagina>,
    nueva_pagina: &mut impl FnMut(&mut Vec<Pagina>, &mut f32),
) {
    let (tamano, negrita, cursiva, mono) = pinta(b.clase);
    let alto_de_linea = tamano * 1.35;
    // Aire antes: un titulo respira mas que un parrafo.
    let antes = match b.clase {
        Clase::Titulo(_) => tamano * 0.8,
        Clase::Regla => tamano,
        _ => tamano * 0.35,
    };
    *y -= antes;

    if b.clase == Clase::Regla || b.vacio() {
        return;
    }

    let sangria = if b.clase == Clase::Lista { 18.0 } else { 0.0 };
    let disponible = util - sangria;
    let mut piezas = trocear(b, tamano, negrita, cursiva, mono);
    if b.clase == Clase::Lista {
        piezas.insert(
            0,
            Trocito {
                texto: "• ".into(),
                letra: indice_de_letra(negrita, cursiva, mono),
                ancho: ancho_de("• ", tamano, indice_de_letra(negrita, cursiva, mono)),
            },
        );
    }

    for linea in partir(&piezas, disponible) {
        if *y < MARGEN + alto_de_linea {
            nueva_pagina(paginas, y);
            *y -= antes.min(tamano);
        }
        *y -= alto_de_linea;
        let ancho_linea: f32 = linea.iter().map(|p| p.ancho).sum();
        let mut x = MARGEN
            + sangria
            + match b.alineacion {
                Alineacion::Izquierda => 0.0,
                Alineacion::Centro => (disponible - ancho_linea).max(0.0) / 2.0,
                Alineacion::Derecha => (disponible - ancho_linea).max(0.0),
            };
        for p in linea {
            if !p.texto.trim().is_empty() {
                paginas
                    .last_mut()
                    .expect("siempre hay al menos una pagina")
                    .push(Pieza {
                        texto: p.texto.clone(),
                        letra: p.letra,
                        tamano,
                        x,
                        y: *y,
                    });
            }
            x += p.ancho;
        }
    }
}

/// Un trozo de texto con una sola letra y su ancho ya medido.
#[derive(Clone)]
struct Trocito {
    texto: String,
    letra: usize,
    ancho: f32,
}

fn indice_de_letra(negrita: bool, cursiva: bool, mono: bool) -> usize {
    if mono {
        return 4;
    }
    match (negrita, cursiva) {
        (true, true) => 3,
        (true, false) => 1,
        (false, true) => 2,
        (false, false) => 0,
    }
}

/// El bloque partido en palabras y espacios, cada uno con su letra: asi el
/// corte de linea cae siempre entre palabras y la negrita no se arrastra.
fn trocear(b: &Bloque, tamano: f32, negrita: bool, cursiva: bool, mono: bool) -> Vec<Trocito> {
    let mut salida = Vec::new();
    for t in &b.trozos {
        let letra = indice_de_letra(
            negrita || t.estilo.negrita,
            cursiva || t.estilo.cursiva || estilo_de_enlace(t.estilo),
            mono || t.estilo.mono,
        );
        let mut palabra = String::new();
        let empujar = |palabra: &mut String, salida: &mut Vec<Trocito>| {
            if !palabra.is_empty() {
                salida.push(Trocito {
                    ancho: ancho_de(palabra, tamano, letra),
                    texto: std::mem::take(palabra),
                    letra,
                });
            }
        };
        for c in t.texto.chars() {
            if c.is_whitespace() {
                empujar(&mut palabra, &mut salida);
                // El salto de linea de un `<br>` corta de verdad.
                let hueco = if c == '\n' { "\n" } else { " " };
                salida.push(Trocito {
                    texto: hueco.into(),
                    letra,
                    ancho: if c == '\n' {
                        0.0
                    } else {
                        ancho_de(" ", tamano, letra)
                    },
                });
                continue;
            }
            palabra.push(c);
        }
        empujar(&mut palabra, &mut salida);
    }
    salida
}

/// Un enlace se marca en cursiva: el PDF no lleva colores.
fn estilo_de_enlace(e: Estilo) -> bool {
    e.enlace
}

/// Reparte los trocitos en lineas que quepan.
fn partir(piezas: &[Trocito], disponible: f32) -> Vec<Vec<Trocito>> {
    let mut lineas: Vec<Vec<Trocito>> = Vec::new();
    let mut linea: Vec<Trocito> = Vec::new();
    let mut ancho = 0.0f32;
    for p in piezas {
        if p.texto == "\n" {
            lineas.push(std::mem::take(&mut linea));
            ancho = 0.0;
            continue;
        }
        // Un espacio al principio de la linea no cuenta.
        if linea.is_empty() && p.texto == " " {
            continue;
        }
        if ancho + p.ancho > disponible && !linea.is_empty() {
            // Se corta aqui; los espacios del final no viajan.
            while linea.last().is_some_and(|u| u.texto == " ") {
                linea.pop();
            }
            lineas.push(std::mem::take(&mut linea));
            ancho = 0.0;
            if p.texto == " " {
                continue;
            }
        }
        ancho += p.ancho;
        linea.push(p.clone());
    }
    if !linea.is_empty() {
        lineas.push(linea);
    }
    if lineas.is_empty() {
        lineas.push(Vec::new());
    }
    lineas
}

/// Cuanto mide un texto, en puntos.
///
/// Los anchos de Helvetica son **aproximados**: una tabla por clase de
/// letra, no la tabla exacta de la tipografia. La unica consecuencia es que
/// el margen derecho queda algo mas desigual que en un maquetador de
/// verdad; nunca se sale de la pagina, porque la estimacion peca por
/// arriba. Courier si es exacto: todas sus letras miden 600 milesimas.
fn ancho_de(texto: &str, tamano: f32, letra: usize) -> f32 {
    let gordo = matches!(letra, 1 | 3);
    let milesimas: f32 = texto
        .chars()
        .map(|c| {
            if letra == 4 {
                return 600.0;
            }
            let base = match c {
                'i' | 'j' | 'l' | 'I' | '.' | ',' | ':' | ';' | '\'' | '|' | '!' | '`' => 250.0,
                ' ' | '(' | ')' | '[' | ']' | '{' | '}' | '/' | 't' | 'f' | 'r' => 300.0,
                'm' | 'M' | 'W' | 'w' | '—' => 880.0,
                'A'..='Z' | '@' | '&' | '%' => 700.0,
                _ => 560.0,
            };
            if gordo { base * 1.06 } else { base }
        })
        .sum();
    milesimas / 1000.0 * tamano
}

/// Escribe el PDF entero: cabecera, objetos, tabla de referencias y cola.
fn escribir(paginas: &[Pagina]) -> Vec<u8> {
    // 1 catalogo, 2 arbol de paginas, 3..7 las letras, y luego dos objetos
    // por pagina (la pagina y su contenido).
    let primera = 3 + LETRAS.len();
    let mut objetos: Vec<String> = Vec::with_capacity(primera + paginas.len() * 2);

    let kids: Vec<String> = (0..paginas.len())
        .map(|i| format!("{} 0 R", primera + i * 2))
        .collect();
    objetos.push("<< /Type /Catalog /Pages 2 0 R >>".into());
    objetos.push(format!(
        "<< /Type /Pages /Kids [{}] /Count {} >>",
        kids.join(" "),
        paginas.len()
    ));
    for (_, real) in LETRAS {
        objetos.push(format!(
            "<< /Type /Font /Subtype /Type1 /BaseFont /{real} /Encoding /WinAnsiEncoding >>"
        ));
    }
    let recursos = format!(
        "<< /Font << {} >> >>",
        LETRAS
            .iter()
            .enumerate()
            .map(|(i, (nombre, _))| format!("/{nombre} {} 0 R", 3 + i))
            .collect::<Vec<_>>()
            .join(" ")
    );
    for (i, pagina) in paginas.iter().enumerate() {
        let contenido = primera + i * 2 + 1;
        objetos.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {ANCHO:.2} {ALTO:.2}] \
             /Resources {recursos} /Contents {contenido} 0 R >>"
        ));
        let flujo = corriente(pagina);
        objetos.push(format!(
            "<< /Length {} >>\nstream\n{flujo}\nendstream",
            flujo.len()
        ));
    }

    let mut salida: Vec<u8> = b"%PDF-1.4\n".to_vec();
    let mut desplazamientos = Vec::with_capacity(objetos.len());
    for (i, cuerpo) in objetos.iter().enumerate() {
        desplazamientos.push(salida.len());
        salida.extend_from_slice(format!("{} 0 obj\n{cuerpo}\nendobj\n", i + 1).as_bytes());
    }
    let inicio = salida.len();
    let total = objetos.len() + 1;
    salida.extend_from_slice(format!("xref\n0 {total}\n").as_bytes());
    // La entrada cero es la cabeza de la lista de huecos y siempre es esta.
    salida.extend_from_slice(b"0000000000 65535 f \n");
    for d in &desplazamientos {
        // Diez cifras, cinco de generacion y el espacio final: cada linea
        // de la tabla mide exactamente veinte bytes o el lector no la sigue.
        salida.extend_from_slice(format!("{d:010} 00000 n \n").as_bytes());
    }
    salida.extend_from_slice(
        format!("trailer\n<< /Size {total} /Root 1 0 R >>\nstartxref\n{inicio}\n%%EOF\n")
            .as_bytes(),
    );
    salida
}

/// Las ordenes de dibujo de una pagina.
fn corriente(pagina: &Pagina) -> String {
    let mut s = String::with_capacity(pagina.len() * 48);
    for p in pagina {
        let (nombre, _) = LETRAS[p.letra];
        s.push_str(&format!(
            "BT /{nombre} {:.1} Tf {:.2} {:.2} Td ({}) Tj ET\n",
            p.tamano,
            p.x,
            p.y,
            texto_pdf(&p.texto)
        ));
    }
    s
}

/// El texto como cadena de PDF: los parentesis y la barra se escapan, y
/// cada letra se pasa a WinAnsi, que es la codificacion que se le declaro a
/// la tipografia.
fn texto_pdf(s: &str) -> String {
    let mut salida = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '(' => salida.push_str("\\("),
            ')' => salida.push_str("\\)"),
            '\\' => salida.push_str("\\\\"),
            _ => {
                let b = win_ansi(c);
                // Fuera del ASCII imprimible se escribe en octal: asi el
                // fichero sigue siendo texto y ningun lector se pierde.
                if (0x20..0x7F).contains(&b) {
                    salida.push(b as char);
                } else {
                    salida.push_str(&format!("\\{b:03o}"));
                }
            }
        }
    }
    salida
}

/// Una letra en WinAnsi. Lo que no cabe (un emoji, un alfabeto que no sea
/// el latino) sale como `?`: el PDF lleva las letras de siempre y no
/// incrusta ninguna tipografia.
fn win_ansi(c: char) -> u8 {
    // Los sitios que WinAnsi usa para la puntuacion fina y que no coinciden
    // con Latin-1.
    match c {
        '€' => return 0x80,
        '‚' => return 0x82,
        '„' => return 0x84,
        '…' => return 0x85,
        '†' => return 0x86,
        '‡' => return 0x87,
        '‰' => return 0x89,
        '‹' => return 0x8B,
        '‘' => return 0x91,
        '’' => return 0x92,
        '“' => return 0x93,
        '”' => return 0x94,
        '•' => return 0x95,
        '–' => return 0x96,
        '—' => return 0x97,
        '™' => return 0x99,
        '›' => return 0x9B,
        '\u{2011}' => return b'-',
        '\u{A0}' => return b' ',
        '\t' => return b' ',
        _ => {}
    }
    let n = c as u32;
    if (0x20..=0xFF).contains(&n) {
        n as u8
    } else {
        b'?'
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::documento::Trozo;

    fn doc(bloques: Vec<Bloque>) -> Documento {
        Documento {
            titulo: "Prueba".into(),
            bloques,
            ..Default::default()
        }
    }

    fn texto_del_pdf(bytes: &[u8]) -> String {
        String::from_utf8_lossy(bytes).to_string()
    }

    #[test]
    fn el_pdf_empieza_y_acaba_como_manda_la_norma() {
        let p = de_documento(&doc(vec![Bloque::nuevo(
            Clase::Parrafo,
            vec![Trozo::llano("hola")],
        )]));
        assert!(p.starts_with(b"%PDF-1."), "no lleva la marca de PDF");
        assert!(texto_del_pdf(&p).ends_with("%%EOF\n"));
        assert!(texto_del_pdf(&p).contains("/Type /Catalog"));
    }

    #[test]
    fn la_tabla_de_referencias_apunta_a_donde_empieza_cada_objeto() {
        // Es lo unico que se rompe siempre al escribir un PDF a mano: si un
        // desplazamiento no cae en su «N 0 obj», ningun lector abre nada.
        let p = de_documento(&doc(vec![Bloque::nuevo(
            Clase::Parrafo,
            vec![Trozo::llano("hola")],
        )]));
        let texto = texto_del_pdf(&p);
        let inicio: usize = texto
            .rsplit_once("startxref\n")
            .and_then(|(_, c)| c.trim().lines().next().map(|n| n.trim().to_string()))
            .and_then(|n| n.parse().ok())
            .expect("tiene que haber un startxref");
        assert!(
            texto[inicio..].starts_with("xref\n"),
            "el xref no cae donde dice"
        );
        // `xref`, `0 N`, y la entrada cero; a partir de ahi, el objeto 1.
        let lineas: Vec<&str> = texto[inicio..].lines().skip(3).collect();
        let mut comprobados = 0;
        for (i, linea) in lineas.iter().enumerate() {
            if linea.starts_with("trailer") {
                break;
            }
            let d: usize = linea
                .split_whitespace()
                .next()
                .and_then(|n| n.parse().ok())
                .unwrap_or_else(|| panic!("linea de xref rara: {linea:?}"));
            let cabecera = format!("{} 0 obj", i + 1);
            assert!(
                texto[d..].starts_with(&cabecera),
                "el objeto {} no empieza en {d}, sino: {:?}",
                i + 1,
                &texto[d..(d + 20).min(texto.len())]
            );
            comprobados += 1;
        }
        assert!(
            comprobados >= 9,
            "solo se comprobaron {comprobados} objetos"
        );
    }

    #[test]
    fn un_texto_largo_ocupa_varias_paginas() {
        let bloques: Vec<Bloque> = (0..200)
            .map(|i| Bloque::nuevo(Clase::Parrafo, vec![Trozo::llano(format!("linea {i}"))]))
            .collect();
        let p = de_documento(&doc(bloques));
        let cuantas = texto_del_pdf(&p).matches("/Type /Page\n").count()
            + texto_del_pdf(&p).matches("/Type /Page ").count();
        assert!(cuantas > 3, "salieron {cuantas} paginas");
    }

    #[test]
    fn los_parentesis_y_la_barra_se_escapan_o_el_pdf_queda_roto() {
        assert_eq!(texto_pdf("a(b)c\\d"), "a\\(b\\)c\\\\d");
    }

    #[test]
    fn las_letras_acentuadas_van_en_octal_y_un_emoji_sale_como_interrogante() {
        assert_eq!(texto_pdf("año"), "a\\361o");
        assert_eq!(texto_pdf("🙂"), "?");
        assert_eq!(texto_pdf("«guion»"), "\\253guion\\273");
    }

    #[test]
    fn un_documento_vacio_da_un_pdf_de_una_pagina_que_se_abre() {
        let p = de_documento(&Documento::default());
        assert!(p.starts_with(b"%PDF-1."));
        assert!(
            texto_del_pdf(&p).contains("/Count 1"),
            "{}",
            texto_del_pdf(&p)
        );
    }

    #[test]
    fn una_palabra_mas_larga_que_la_pagina_no_deja_la_linea_en_blanco_para_siempre() {
        // Sin el «y la linea no esta vacia» del corte, una palabra que no
        // cabe daria vueltas sin colocarse nunca.
        let larga = "a".repeat(500);
        let p = de_documento(&doc(vec![Bloque::nuevo(
            Clase::Parrafo,
            vec![Trozo::llano(larga)],
        )]));
        assert!(p.len() > 400);
    }

    #[test]
    fn cada_capitulo_de_un_libro_empieza_en_pagina_nueva() {
        let p = de_documento(&doc(vec![
            Bloque::nuevo(Clase::Parrafo, vec![Trozo::llano("uno")]),
            Bloque::nuevo(Clase::Capitulo, vec![Trozo::llano(" ")]),
            Bloque::nuevo(Clase::Parrafo, vec![Trozo::llano("dos")]),
        ]));
        assert!(
            texto_del_pdf(&p).contains("/Count 2"),
            "{}",
            texto_del_pdf(&p)
        );
    }

    #[test]
    fn la_negrita_pide_la_letra_gorda() {
        let mut trozo = Trozo::llano("gordo");
        trozo.estilo.negrita = true;
        let p = de_documento(&doc(vec![Bloque::nuevo(Clase::Parrafo, vec![trozo])]));
        assert!(
            texto_del_pdf(&p).contains("/F2 "),
            "no se uso Helvetica-Bold"
        );
    }
}
