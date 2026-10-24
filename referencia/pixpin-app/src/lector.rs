//! **Lo comun a los dos lectores** (el de Word/libros, `visor.rs`, y el de
//! PDF, `lector_pdf.rs`): los colores del papel, el riel de marcadores, los
//! avisos y las cuentas de la rueda. Lo que cambia entre uno y otro —como se
//! mide y pinta el documento— vive en cada uno.
//!
//! # El riel, el mismo del lienzo
//!
//! El movil lo dice asi: los marcadores con emoticono son «la misma
//! interfaz» en el lienzo, en el PDF y en los libros (`RielDeMarcas.kt`).
//! Aqui se usa la colocacion y el picado de `pixpin_ui::riel_marcas` tal
//! cual (la isla arriba a la derecha con dos botones, los puntos debajo en
//! el orden del documento, la tira de emoticonos al poner uno). En la isla
//! el segundo boton es **el lapiz de anotar**, que es lo que en un lector
//! se tiene a mano (en el lienzo es la hojita).

use pixpin_geom::{Punto, Rect};
use pixpin_render::icono::material;
use pixpin_render::{Color, Pintor, RectF};
use pixpin_ui::riel_marcas::{DestinoRiel, PUNTO_CERCA, PUNTO_ELEGIDO, PUNTO_REPOSO, Riel, SALTO};

use crate::caja_dibujo::hex;

// Los colores del «visor limpio»: papel, no interfaz. Son los del modo
// noche del movil (`DocxAHtml.kt`, bloque `prefers-color-scheme`).
pub const FONDO: Color = hex(0x121316);
pub const TEXTO: Color = hex(0xe4e2e6);
pub const APAGADO: Color = hex(0x9a9aa2);
pub const CRISTAL: Color = hex(0x26262c);
pub const RAYA: Color = hex(0x3a3a42);
pub const DORADO: Color = hex(0xe8c06a);
/// El fondo del lector de PDF: el `#1B1B1B` del movil.
pub const FONDO_PDF: Color = hex(0x1b1b1b);
/// El papel del margen para anotar de un PDF (`PAPEL_DEL_MARGEN`).
pub const PAPEL_DEL_MARGEN: Color = hex(0xf3f3f0);
/// Un boton encendido: el azul `#0060DF` de lo pulsado en la barra de los
/// pines (`lector_pin`), el mismo en todos los visores (19-oct-2026).
pub const ENCENDIDO: Color = Color {
    r: 0x00 as f32 / 255.0,
    g: 0x60 as f32 / 255.0,
    b: 0xdf as f32 / 255.0,
    a: 1.0,
};
/// El `#14182B` de los redondeles de las marcas.
const OSCURO: Color = hex(0x14182b);

/// Cuanto vale una muesca de la rueda (`WHEEL_DELTA`). El evento trae el
/// giro en bruto: 120 por muesca, o fracciones en un panel tactil.
pub const MUESCA: f32 = 120.0;
/// Renglones por muesca, como Windows por defecto.
pub const RENGLONES_POR_MUESCA: f32 = 3.0;

/// Los emoticonos de marcar: los del movil, en su orden.
pub const EMOJIS: [&str; 12] = pixpin_motor2d::marcas::EMOJIS;

/// Si PixPin tiene lector para esto: el de PDF o el de Word, libros,
/// paginas y notas. Es lo que mira la entrada «Abrir aqui» del chat.
pub fn tiene_lector(nombre: &str) -> bool {
    crate::lector_pdf::se_abre(nombre)
        // Un Word o un PowerPoint de ahora, con sus paginas de Office.
        || crate::lector_pdf::se_abre_office(nombre)
        || crate::visor::se_abre(nombre)
        // Un PowerPoint «se lee» presentandolo (D10).
        || crate::diapositivas::es_presentacion(nombre)
}

/// Lo que se abre en su lector **con solo tocar la burbuja**: un Word, un
/// libro o un PDF, como en el movil. Un `.txt` o un `.csv` siguen yendo a su
/// aplicacion de siempre (el Bloc de notas, la hoja de calculo): ahi el
/// lector no aporta nada y quitarle al usuario su programa si.
pub fn se_lee_al_tocar(nombre: &str) -> bool {
    matches!(
        pixpin_docs::extension(nombre).as_str(),
        "pdf" | "docx" | "docm" | "epub"
    ) && tiene_lector(nombre)
        // Tocar la burbuja de un PowerPoint lo presenta, como en el movil.
        || crate::diapositivas::es_presentacion(nombre)
        // Un Word o un PowerPoint de ahora (tambien las plantillas), en el
        // lector con sus paginas de Office.
        || crate::lector_pdf::se_abre_office(nombre)
        // Un plano DWG o DXF, en su visor (como un pin).
        || crate::plano_cad::se_abre(nombre)
        // Un modelo BIM (IFC o Revit), en su visor 3D.
        || crate::modelo_bim::se_abre(nombre)
}

/// Como [`se_lee_al_tocar`], mirando tambien dentro del fichero: un
/// LandXML o un fichero de puntos de Civil 3D van al visor 3D (un `.xml`,
/// `.csv` o `.txt` cualquiera no). Para lo que el usuario abre o suelta en
/// PixPin a proposito.
pub fn se_lee_al_tocar_fichero(ruta: &std::path::Path) -> bool {
    se_lee_al_tocar(&pixpin_docs::nombre(ruta))
        || pixpin_bim::es_de_civil(ruta)
        // Un libro de Excel moderno, en el visor de hojas (si es un ZIP).
        || crate::visor_hoja::se_abre_fichero(ruta)
}

/// Para la burbuja del chat: los LandXML si; los `.csv` y `.txt` siguen
/// yendo a su programa aunque sean puntos. (Un Excel del chat no llega
/// aqui: se abre como pin de hoja, `hoja_pin`.)
pub fn se_lee_al_tocar_burbuja(ruta: &std::path::Path) -> bool {
    let nombre = pixpin_docs::nombre(ruta);
    se_lee_al_tocar(&nombre)
        || (pixpin_docs::extension(&nombre) == "xml" && pixpin_bim::es_de_civil(ruta))
        || crate::visor_hoja::se_abre_fichero(ruta)
}

/// Abre `ruta` en el lector que le toque, en su propio hilo. `false` si no
/// hay lector para eso (y entonces no se abre nada).
pub fn abrir_en_su_lector(
    idioma: pixpin_store::Idioma,
    ubicacion: &pixpin_store::Ubicacion,
    ruta: &std::path::Path,
    nombre: &str,
) -> bool {
    // Un Markdown, en el lector de notas, como «Abrir con» (`main::destino_con`):
    // el visor de documentos lo ensenaba en crudo (el usuario, 20-oct-2026:
    // «las tablas… se ven sin su formato, solo con los guiones»).
    if crate::notas_md::es_markdown(ruta) {
        crate::notas_md::abrir(
            idioma,
            ubicacion.clone(),
            crate::notas_md::Destino::Fichero {
                ruta: ruta.to_path_buf(),
            },
        );
        return true;
    }
    // Unos puntos o un LandXML (por su contenido) van al visor 3D antes
    // que al de documentos, que tambien abre los .txt.
    if pixpin_bim::es_de_civil(ruta) {
        crate::modelo_bim::lanzar(idioma, ubicacion.clone(), ruta);
        return true;
    }
    // Un Word o un PowerPoint de ahora: sus paginas tal como las imprime
    // Office, en el lector de paginas (si no se deja leer, el lanzador lo
    // manda a donde iba antes: el lector de documentos o PowerPoint).
    if crate::lector_pdf::se_abre_office(nombre) {
        crate::lector_pdf::lanzar_office(idioma, ubicacion.clone(), ruta);
        return true;
    }
    if crate::lector_pdf::se_abre(nombre) {
        crate::lector_pdf::lanzar(idioma, ubicacion.clone(), ruta);
        true
    } else if crate::visor::se_abre(nombre) {
        crate::visor::lanzar(idioma, ubicacion.clone(), ruta);
        true
    } else if crate::diapositivas::es_presentacion(nombre) {
        crate::diapositivas::lanzar(idioma, ubicacion.clone(), ruta);
        true
    } else if crate::plano_cad::se_abre(nombre) {
        crate::plano_cad::lanzar(idioma, ubicacion.clone(), ruta);
        true
    } else if crate::modelo_bim::se_abre(nombre) {
        crate::modelo_bim::lanzar(idioma, ubicacion.clone(), ruta);
        true
    } else if crate::visor_hoja::se_abre(nombre) {
        // Un .xlsx que no se pueda leer se abre con su programa desde alli.
        crate::visor_hoja::lanzar(idioma, ruta);
        true
    } else {
        false
    }
}

pub fn con_alfa(c: Color, a: f32) -> Color {
    Color { a, ..c }
}

pub fn ahora_ms() -> u64 {
    pixpin_shell::entorno::ahora_utc_ms() as u64
}

/// Si un punto cae en una caja.
pub fn dentro(r: &RectF, x: f32, y: f32) -> bool {
    x >= r.x && x <= r.x + r.ancho && y >= r.y && y <= r.y + r.alto
}

pub fn rf(r: Rect) -> RectF {
    RectF {
        x: r.x as f32,
        y: r.y as f32,
        ancho: r.ancho as f32,
        alto: r.alto as f32,
    }
}

pub fn punto(x: f32, y: f32) -> Punto {
    Punto {
        x: x.round() as i32,
        y: y.round() as i32,
    }
}

/// **La tira de emoticonos lleva el verde al final** (`ElegirEmojiDeMarca`
/// con `conVerde = true` del movil): el marcador de la voz, que es uno solo
/// y se pone «aqui» para que se lea desde ahi.
pub const VERDE_EN_LA_TIRA: usize = EMOJIS.len();

/// El emoticono de la celda `i` de la tira: los de marcar y, el ultimo, el verde.
pub fn emoji_de_la_tira(i: usize) -> &'static str {
    EMOJIS
        .get(i)
        .copied()
        .unwrap_or(pixpin_docs::voz_alta::EMOJI_DE_VOZ)
}

/// Un boton de los mandos de los lados.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotonLado {
    /// Mas espacio (`true`) o menos a la izquierda.
    Izquierda(bool),
    Derecha(bool),
    /// El candado del desplazamiento de lado.
    Candado,
}

/// Si el raton esta en la franja de abajo, donde salen los mandos de los
/// lados (el «tocar para que salgan» del movil, con raton).
pub fn raton_abajo(raton_y: f32, alto: f32, e: f32) -> bool {
    raton_y > alto - 110.0 * e
}

/// **Los mandos de los lados** (`MandosDeLosLadosDeLector` del movil), abajo
/// y pequenos: a la izquierda el espacio de la izquierda (−, ⟵, +); en
/// medio el candado del desplazamiento de lado; a la derecha el espacio de
/// la derecha. `pasos_*` son los pasos puestos y `tope` los que caben (en
/// un PDF, uno: el espacio esta o no). El − solo sale si hay algo que
/// quitar; el + se apaga en el tope.
#[allow(clippy::too_many_arguments)] // lo que se ve depende de los dos lados y del candado
pub fn pintar_mandos_de_los_lados(
    p: &Pintor<'_>,
    ancho: f32,
    alto: f32,
    e: f32,
    pasos_izq: u32,
    pasos_der: u32,
    tope: u32,
    sin_lado: bool,
) -> Vec<(RectF, BotonLado)> {
    let mut botones = Vec::new();
    let lado = 36.0 * e;
    let y = alto - 12.0 * e - lado - 20.0 * e;
    let fondo = con_alfa(OSCURO, 0.55);
    let tam = 16.0 * e;
    let centrado = |p: &Pintor<'_>, t: &str, r: RectF, color: Color| {
        let (w, h) = p.medir_texto(t, tam);
        p.texto(
            t,
            r.x + (r.ancho - w) / 2.0,
            r.y + (r.alto - h) / 2.0,
            tam,
            color,
        );
    };
    let mut pastilla = |p: &Pintor<'_>, izquierda: bool, x: f32, pasos: u32| -> f32 {
        let flecha = if izquierda { "⟵" } else { "⟶" };
        let (wf, _) = p.medir_texto(flecha, tam);
        let menos = pasos > 0;
        let ancho_flecha = wf + if menos { 4.0 } else { 20.0 } * e;
        let total = ancho_flecha + lado + if menos { lado } else { 0.0 };
        let x = if izquierda { x } else { x - total };
        p.rellenar_redondeado(
            RectF {
                x,
                y,
                ancho: total,
                alto: lado,
            },
            lado / 2.0,
            fondo,
        );
        let mut xx = x;
        if menos {
            let r = RectF {
                x: xx,
                y,
                ancho: lado,
                alto: lado,
            };
            centrado(p, "−", r, Color::BLANCO);
            botones.push((
                r,
                if izquierda {
                    BotonLado::Izquierda(false)
                } else {
                    BotonLado::Derecha(false)
                },
            ));
            xx += lado;
        }
        centrado(
            p,
            flecha,
            RectF {
                x: xx,
                y,
                ancho: ancho_flecha,
                alto: lado,
            },
            con_alfa(Color::BLANCO, 0.8),
        );
        xx += ancho_flecha;
        let r = RectF {
            x: xx,
            y,
            ancho: lado,
            alto: lado,
        };
        let lleno = pasos >= tope;
        centrado(
            p,
            "+",
            r,
            con_alfa(Color::BLANCO, if lleno { 0.3 } else { 1.0 }),
        );
        if !lleno {
            botones.push((
                r,
                if izquierda {
                    BotonLado::Izquierda(true)
                } else {
                    BotonLado::Derecha(true)
                },
            ));
        }
        total
    };
    pastilla(p, true, 14.0 * e, pasos_izq);
    pastilla(p, false, ancho - 14.0 * e, pasos_der);
    // El candado, en medio.
    let candado = if sin_lado { "🔒" } else { "🔓" };
    let (wc, _) = p.medir_texto(candado, tam);
    let (wd, _) = p.medir_texto(" ⟷", tam);
    let total = wc + wd + 24.0 * e;
    let r = RectF {
        x: (ancho - total) / 2.0,
        y,
        ancho: total,
        alto: lado,
    };
    p.rellenar_redondeado(r, lado / 2.0, fondo);
    let (_, h) = p.medir_texto(candado, tam);
    p.texto_color(
        candado,
        r.x + 12.0 * e,
        r.y + (lado - h) / 2.0,
        tam,
        if sin_lado { DORADO } else { Color::BLANCO },
    );
    p.texto(
        " ⟷",
        r.x + 12.0 * e + wc,
        r.y + (lado - h) / 2.0,
        tam,
        Color::BLANCO,
    );
    botones.push((r, BotonLado::Candado));
    botones
}

/// El riel colocado para esta ventana. La tira de emoticonos va debajo de
/// la pastilla del nombre (o de la barra de anotar), que es lo que hay
/// arriba en el centro.
pub fn riel(ancho: f32, alto: f32, escala_por_cien: u32, cuantas: usize, tira: bool) -> Riel {
    let e = escala_por_cien as f32 / 100.0;
    Riel::colocar(
        ancho as u32,
        alto as u32,
        escala_por_cien,
        cuantas,
        tira,
        EMOJIS.len() + 1,
        (60.0 * e) as i32,
    )
}

/// Un emoticono sobre su redondel oscuro, centrado en `c`.
pub fn redondel(p: &Pintor<'_>, emoji: &str, c: (f32, f32), radio: f32, alfa: f32) {
    p.circulo(c, radio, con_alfa(OSCURO, alfa));
    let tam = radio * 1.1;
    let (w, h) = p.medir_texto(emoji, tam);
    // `texto_color`: con `texto` el emoticono sale como silueta.
    p.texto_color(emoji, c.0 - w / 2.0, c.1 - h / 2.0, tam, Color::BLANCO);
}

/// **El riel**: la isla (marcar y anotar), los puntos de las marcas y, si
/// se esta poniendo una, la tira de emoticonos. `emojis` son los de las
/// marcas en el orden del riel; `atajo` es el texto «Ctrl+» traducido. Con
/// `enmarcando`, el marco pulsado (se estan poniendo los marcos numerados,
/// `marcos_doc`).
#[allow(clippy::too_many_arguments)] // lo que se ve del riel depende de todo esto
pub fn pintar_riel_con(
    p: &Pintor<'_>,
    riel: &Riel,
    emojis: &[&str],
    encima: DestinoRiel,
    poniendo: bool,
    anotando: bool,
    enmarcando: bool,
    elige: &str,
) {
    let e = riel.escala_por_cien as f32 / 100.0;
    let isla = rf(riel.isla);
    p.rellenar_redondeado(isla, 8.0 * e, con_alfa(CRISTAL, 0.92));
    for (caja, icono, activo, que) in [
        (
            riel.poner_marca,
            &material::BOOKMARK_ADD,
            poniendo,
            DestinoRiel::PonerMarca,
        ),
        (riel.hojita, &material::EDIT, anotando, DestinoRiel::Hojita),
        // El marco para imprimir: igual en todos los visores.
        (
            riel.imprimir,
            &material::CROP_FREE,
            enmarcando,
            DestinoRiel::Imprimir,
        ),
    ] {
        let r = rf(caja);
        if activo {
            p.rellenar_redondeado(r, 6.0 * e, ENCENDIDO);
        } else if encima == que {
            p.rellenar_redondeado(r, 6.0 * e, con_alfa(Color::BLANCO, 0.12));
        }
        let lado = 20.0 * e;
        p.icono(
            icono,
            RectF {
                x: r.x + (r.ancho - lado) / 2.0,
                y: r.y + (r.alto - lado) / 2.0,
                ancho: lado,
                alto: lado,
            },
            if activo { Color::BLANCO } else { TEXTO },
        );
    }

    // El globo del boton bajo el raton: su nombre y su tecla, a su
    // izquierda (19-oct-2026, el usuario: «que me aparezca su atajo»).
    if let Some((caja, clave)) = match encima {
        DestinoRiel::PonerMarca => Some((riel.poner_marca, "riel-marcador")),
        DestinoRiel::Hojita => Some((riel.hojita, "riel-anotar")),
        DestinoRiel::Imprimir => Some((riel.imprimir, "riel-marcos")),
        _ => None,
    } {
        globo_a_la_izquierda(
            p,
            rf(caja),
            &crate::ventana_editor::exportar::textos().t(clave),
            e,
        );
    }

    // Los puntos: en reposo pequenos y medio transparentes para no tapar la
    // lectura; con el raton encima crecen, y el de debajo se adelanta hacia
    // dentro (lo que hace el movil bajo el dedo).
    let con_raton = matches!(encima, DestinoRiel::Marca(_));
    for (i, (emoji, c)) in emojis.iter().zip(&riel.puntos).enumerate() {
        let elegido = encima == DestinoRiel::Marca(i);
        let (diametro, alfa, x) = if elegido {
            (PUNTO_ELEGIDO, 0.9, c.x as f32 - SALTO as f32 * e)
        } else if con_raton {
            (PUNTO_CERCA, 0.5, c.x as f32)
        } else {
            (PUNTO_REPOSO, 0.5, c.x as f32)
        };
        let radio = diametro as f32 * e / 2.0;
        redondel(p, emoji, (x, c.y as f32), radio, alfa);
        if elegido && i < 9 {
            let tecla = format!("Ctrl+{}", i + 1);
            let tam = 11.0 * e;
            let (w, h) = p.medir_texto(&tecla, tam);
            p.texto(
                &tecla,
                x - radio - w - 6.0 * e,
                c.y as f32 - h / 2.0,
                tam,
                TEXTO,
            );
        }
    }

    if let Some(tira) = &riel.tira {
        let marco = rf(tira.marco);
        p.rellenar_redondeado(marco, marco.alto / 2.0, con_alfa(OSCURO, 0.9));
        for (i, celda) in tira.celdas.iter().enumerate() {
            let r = rf(*celda);
            if encima == DestinoRiel::Emoji(i) {
                p.circulo(
                    (r.x + r.ancho / 2.0, r.y + r.alto / 2.0),
                    r.ancho / 2.0,
                    con_alfa(Color::BLANCO, 0.18),
                );
            }
            let tam = 22.0 * e;
            let (w, h) = p.medir_texto(emoji_de_la_tira(i), tam);
            p.texto_color(
                emoji_de_la_tira(i),
                r.x + (r.ancho - w) / 2.0,
                r.y + (r.alto - h) / 2.0,
                tam,
                Color::BLANCO,
            );
        }
        let x = rf(tira.cerrar);
        let lado = 18.0 * e;
        p.icono(
            &material::CLOSE,
            RectF {
                x: x.x + (x.ancho - lado) / 2.0,
                y: x.y + (x.alto - lado) / 2.0,
                ancho: lado,
                alto: lado,
            },
            Color::BLANCO,
        );
        aviso_en(
            p,
            elige,
            marco.x + marco.ancho / 2.0,
            marco.y + marco.alto + 8.0 * e,
            e,
        );
    }
}

/// Una pastilla oscura con una linea de texto, centrada en `cx`.
pub fn aviso_en(p: &Pintor<'_>, texto: &str, cx: f32, y: f32, e: f32) {
    let tam = 13.0 * e;
    let (w, h) = p.medir_texto(texto, tam);
    let caja = RectF {
        x: cx - w / 2.0 - 12.0 * e,
        y,
        ancho: w + 24.0 * e,
        alto: h + 12.0 * e,
    };
    p.rellenar_redondeado(caja, caja.alto / 2.0, con_alfa(OSCURO, 0.9));
    p.texto(
        texto,
        caja.x + 12.0 * e,
        caja.y + 6.0 * e,
        tam,
        Color::BLANCO,
    );
}

/// El aviso de abajo (guardado, no se pudo, la letra fijada...).
pub fn aviso_abajo(p: &Pintor<'_>, texto: &str, ancho: f32, alto: f32, e: f32) {
    aviso_en(p, texto, ancho / 2.0, alto - 110.0 * e, e);
}

/// Que numero de marca pide una tecla de cifra con la tira abierta: 1-9 y
/// 0 para la decima, como en el lienzo.
pub fn emoji_de_cifra(c: char) -> Option<usize> {
    let d = c.to_digit(10)?;
    Some(if d == 0 { 9 } else { d as usize - 1 }.min(EMOJIS.len() - 1))
}

/// Las muescas que trae un giro de la rueda (positivo = hacia arriba).
pub fn muescas(delta: i32) -> f32 {
    delta as f32 / MUESCA
}

/// Lo que se desplaza con un giro, en unidades del documento: tres
/// renglones por muesca, hacia abajo con la rueda hacia abajo.
pub fn desplazamiento_de_rueda(delta: i32, renglon: f32) -> f32 {
    -muescas(delta) * RENGLONES_POR_MUESCA * renglon
}

/// La cruz de un arrastre: menos que esto es un clic, no un arrastre. Un
/// raton nunca se queda quieto del todo al pulsar.
pub const UMBRAL_DE_ARRASTRE: f32 = 4.0;

// ---------------------------------------------------------------------------
// La pagina que sube sola (Android v0.88/v0.92, `MandosDeLector.kt`)
//
// Las cuentas estan en `pixpin_docs::auto_desplazar`; aqui va lo que comparten
// los dos lectores: el boton de la pastilla, la barra de abajo y la velocidad
// que se recuerda en este equipo (`auto:ppm` de las preferencias del movil).

/// `keyboard_double_arrow_down` de Material: el boton «Desplazar sola» de la
/// pastilla del movil. Va aqui y no en `pixpin_render::icono::material`
/// porque solo lo usan los lectores.
pub const DOBLE_FLECHA_ABAJO: pixpin_render::icono::Icono = pixpin_render::icono::Icono {
    vista: (0.0, 0.0, 24.0, 24.0),
    trazos: &[pixpin_render::icono::TrazoIcono {
        d: "M18 6.41 16.59 5 12 9.58 7.41 5 6 6.41l6 6zM18 13l-1.41-1.41L12 16.17l-4.59-4.58L6 13l6 6z",
        relleno: pixpin_render::icono::Pintura::Actual,
        trazo: pixpin_render::icono::Pintura::Nada,
        grosor: 0.0,
        extremo_redondo: false,
        union_redonda: false,
        opacidad: 1.0,
        par_impar: false,
        matriz: None,
        mascara: None,
    }],
};

/// Un boton de la barra de la pagina que sube sola.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotonAuto {
    /// Mas despacio.
    Menos,
    /// Mas deprisa.
    Mas,
    /// Pausa o seguir.
    Alternar,
    /// Dejar de desplazar (la barra se va).
    Cerrar,
}

/// **La velocidad recordada en este equipo**, en palabras por minuto. Es del
/// aparato, no del documento: en el movil va en sus preferencias.
pub fn ppm_guardadas(ubicacion: &pixpin_store::Ubicacion) -> u32 {
    pixpin_docs::auto_desplazar::valida(
        pixpin_store::estado::cargar(ubicacion)
            .palabras_por_minuto
            .unwrap_or(pixpin_docs::auto_desplazar::POR_DEFECTO),
    )
}

/// Guarda la velocidad sin tocar lo demas que se recuerda.
pub fn guardar_ppm(ubicacion: &pixpin_store::Ubicacion, ppm: u32) {
    let mut estado = pixpin_store::estado::cargar(ubicacion);
    estado.palabras_por_minuto = Some(ppm);
    if let Err(e) = pixpin_store::estado::guardar(ubicacion, &estado) {
        tracing::warn!(?e, "no se pudo guardar la velocidad de leer");
    }
}

/// Los minutos desde las 00:00 de hoy, en la hora de este equipo.
pub fn minutos_de_hoy() -> i64 {
    (pixpin_shell::entorno::ahora_local_ms() / 60_000).rem_euclid(24 * 60)
}

/// **«Terminas en 12 min · a las 18:40»** (`rotuloDeLoQueQueda`), para
/// `palabras` a `ppm` con `progreso` de 0 a 1.
pub fn rotulo_de_lo_que_queda(
    textos: &pixpin_store::Catalogo,
    palabras: usize,
    progreso: f32,
    ppm: u32,
    ahora_en_minutos: i64,
) -> String {
    use pixpin_docs::auto_desplazar as auto;
    let minutos = auto::minutos_que_quedan(palabras, progreso, ppm);
    if minutos <= 0.0 {
        return textos.t("lector-auto-terminado");
    }
    let mut args = fluent_bundle::FluentArgs::new();
    args.set(
        "cuanto",
        auto::rotulo_con(minutos, &textos.t("lector-auto-menos-de-un-minuto"), ""),
    );
    args.set("hora", auto::hora_de_terminar(ahora_en_minutos, minutos));
    textos.t_args("lector-auto-quedan", &args)
}

/// La velocidad como se lee en la barra: «230 palabras/min».
pub fn rotulo_de_ppm(textos: &pixpin_store::Catalogo, ppm: u32) -> String {
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("ppm", ppm);
    textos.t_args("lector-auto-ppm", &args)
}

/// **La barra de la pagina que sube sola** (`BarraDeAutoDesplazarDeLector`),
/// abajo y de cristal como la de escuchar: mas despacio, la velocidad, mas
/// deprisa, pausa y cerrar; debajo, `quedan` («Terminas en 12 min…»).
pub fn pintar_barra_de_auto(
    p: &Pintor<'_>,
    ancho: f32,
    alto: f32,
    e: f32,
    velocidad: &str,
    en_marcha: bool,
    quedan: &str,
) -> Vec<(RectF, BotonAuto)> {
    let mut botones = Vec::new();
    let lado = 40.0 * e;
    let tam = 14.0 * e;
    let (w_vel, h_vel) = p.medir_texto(velocidad, tam);
    let ancho_vel = w_vel + 16.0 * e;
    let fila = 4.0 * lado + ancho_vel;
    let tam_debajo = 11.0 * e;
    let (w_debajo, h_debajo) = p.medir_texto(quedan, tam_debajo);
    let total = fila.max(w_debajo + 24.0 * e) + 12.0 * e;
    let caja = RectF {
        x: (ancho - total) / 2.0,
        y: alto - 24.0 * e - lado - h_debajo - 10.0 * e,
        ancho: total,
        alto: lado + h_debajo + 10.0 * e,
    };
    p.rellenar_redondeado(caja, 20.0 * e, con_alfa(CRISTAL, 0.95));
    let mut x = caja.x + (total - fila) / 2.0;
    let y = caja.y;
    let centrado = |p: &Pintor<'_>, t: &str, r: RectF, tam: f32| {
        let (w, h) = p.medir_texto(t, tam);
        p.texto(
            t,
            r.x + (r.ancho - w) / 2.0,
            r.y + (r.alto - h) / 2.0,
            tam,
            TEXTO,
        );
    };
    let icono = |p: &Pintor<'_>, i: &pixpin_render::icono::Icono, r: RectF, l: f32| {
        p.icono(
            i,
            RectF {
                x: r.x + (r.ancho - l) / 2.0,
                y: r.y + (r.alto - l) / 2.0,
                ancho: l,
                alto: l,
            },
            TEXTO,
        );
    };
    let boton = |x: &mut f32, ancho: f32| {
        let r = RectF {
            x: *x,
            y,
            ancho,
            alto: lado,
        };
        *x += ancho;
        r
    };
    let r = boton(&mut x, lado);
    centrado(p, "−", r, 18.0 * e);
    botones.push((r, BotonAuto::Menos));
    let r = boton(&mut x, ancho_vel);
    p.texto(
        velocidad,
        r.x + (r.ancho - w_vel) / 2.0,
        r.y + (r.alto - h_vel) / 2.0,
        tam,
        TEXTO,
    );
    let r = boton(&mut x, lado);
    centrado(p, "+", r, 18.0 * e);
    botones.push((r, BotonAuto::Mas));
    let r = boton(&mut x, lado);
    icono(
        p,
        if en_marcha {
            &material::PAUSE
        } else {
            &material::PLAY_ARROW
        },
        r,
        28.0 * e,
    );
    botones.push((r, BotonAuto::Alternar));
    let r = boton(&mut x, lado);
    icono(p, &material::CLOSE, r, 20.0 * e);
    botones.push((r, BotonAuto::Cerrar));
    p.texto(
        quedan,
        caja.x + (total - w_debajo) / 2.0,
        caja.y + lado,
        tam_debajo,
        con_alfa(TEXTO, 0.7),
    );
    botones
}

/// **El globo de un boton**: su nombre y su atajo, en una pastilla oscura a
/// la izquierda de `junto_a` (pixeles de la ventana), centrada a su altura.
/// El mismo para todos los rieles de los visores.
pub fn globo_a_la_izquierda(p: &Pintor<'_>, junto_a: RectF, texto: &str, e: f32) {
    let tam = 12.0 * e;
    let (w, h) = p.medir_texto(texto, tam);
    let (px, py) = (8.0 * e, 5.0 * e);
    let caja = RectF {
        x: (junto_a.x - 8.0 * e - w - 2.0 * px).max(0.0),
        y: junto_a.y + (junto_a.alto - h - 2.0 * py) / 2.0,
        ancho: w + 2.0 * px,
        alto: h + 2.0 * py,
    };
    p.rellenar_redondeado(
        caja,
        6.0 * e,
        Color {
            r: 0.106,
            g: 0.106,
            b: 0.122,
            a: 0.96,
        },
    );
    p.texto(texto, caja.x + px, caja.y + py, tam, Color::BLANCO);
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn una_muesca_de_rueda_baja_tres_renglones() {
        assert_eq!(desplazamiento_de_rueda(-120, 20.0), 60.0);
        assert_eq!(desplazamiento_de_rueda(120, 20.0), -60.0);
        // Un panel tactil manda trocitos: se respetan, no se redondean a
        // cero (si no, el desplazamiento fino no movería nada).
        assert!((desplazamiento_de_rueda(-12, 20.0) - 6.0).abs() < 1e-4);
    }

    #[test]
    fn al_tocar_la_burbuja_se_leen_word_libros_y_pdf_y_no_una_hoja_csv() {
        assert!(se_lee_al_tocar("plano.pdf"));
        assert!(se_lee_al_tocar("Tema 1.DOCX"));
        assert!(se_lee_al_tocar("novela.epub"));
        assert!(
            !se_lee_al_tocar("gastos.csv"),
            "la hoja de calculo es su programa"
        );
        assert!(!se_lee_al_tocar("notas.txt"));
        assert!(!se_lee_al_tocar("foto.png"));
        assert!(!se_lee_al_tocar("pdf"), "sin extension no es un PDF");
        // «Abrir aqui» si se ofrece para todo lo que el lector sabe leer.
        assert!(tiene_lector("notas.md"));
        assert!(tiene_lector("gastos.csv"));
        assert!(!tiene_lector("foto.png"));
    }

    #[test]
    fn la_tira_acaba_en_el_verde_de_la_voz() {
        assert_eq!(emoji_de_la_tira(0), EMOJIS[0]);
        assert_eq!(
            emoji_de_la_tira(VERDE_EN_LA_TIRA),
            pixpin_docs::voz_alta::EMOJI_DE_VOZ
        );
        let r = riel(1600.0, 900.0, 100, 0, true);
        assert_eq!(r.tira.expect("tira").celdas.len(), EMOJIS.len() + 1);
        assert!(raton_abajo(850.0, 900.0, 1.0));
        assert!(!raton_abajo(400.0, 900.0, 1.0));
    }

    #[test]
    fn las_cifras_eligen_emoticono_y_el_cero_es_el_decimo() {
        assert_eq!(emoji_de_cifra('1'), Some(0));
        assert_eq!(emoji_de_cifra('9'), Some(8));
        assert_eq!(emoji_de_cifra('0'), Some(9));
        assert_eq!(emoji_de_cifra('x'), None);
    }

    #[test]
    fn los_emoticonos_del_lector_son_los_de_los_marcadores_de_libros() {
        // Tres listas que tienen que ser una: la del motor (lienzo y PDF), la
        // de `pixpin_docs::lectura` (Word y libros) y la del movil.
        assert_eq!(EMOJIS, pixpin_docs::lectura::EMOJIS);
    }

    #[test]
    fn el_riel_de_los_lectores_va_a_la_derecha_con_su_tira_bajo_la_pastilla() {
        let r = riel(1600.0, 900.0, 100, 3, true);
        assert!(r.isla.x > 1400);
        assert_eq!(r.puntos.len(), 3);
        let tira = r.tira.expect("tira abierta");
        assert!(
            tira.marco.y >= 60,
            "debajo de la pastilla: {:?}",
            tira.marco
        );
        assert!(!riel(1600.0, 900.0, 100, 0, false).tira.is_some());
    }

    #[test]
    fn un_excel_abierto_con_pixpin_va_al_visor_de_hojas_si_es_un_zip() {
        let dir = std::env::temp_dir().join(format!("pixpin-lector-hoja-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let libro = dir.join("Cuentas.xlsx");
        std::fs::write(&libro, b"PK\x03\x04...").unwrap();
        let viejo = dir.join("Antiguo.xlsx");
        std::fs::write(&viejo, b"\xD0\xCF\x11\xE0...").unwrap();
        assert!(se_lee_al_tocar_fichero(&libro));
        assert!(se_lee_al_tocar_burbuja(&libro));
        // Caso negativo: un .xls renombrado y un .xls siguen su camino de
        // antes (su programa o un pin), y por el nombre solo tampoco.
        assert!(!se_lee_al_tocar_fichero(&viejo));
        assert!(!se_lee_al_tocar_fichero(&dir.join("hoja.xls")));
        assert!(!se_lee_al_tocar("Cuentas.xlsx"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dentro_de_una_caja_incluye_los_bordes_y_nada_mas() {
        let r = RectF {
            x: 10.0,
            y: 10.0,
            ancho: 5.0,
            alto: 5.0,
        };
        assert!(dentro(&r, 10.0, 15.0));
        assert!(!dentro(&r, 9.9, 12.0));
        assert!(!dentro(&r, 12.0, 15.1));
    }
}
