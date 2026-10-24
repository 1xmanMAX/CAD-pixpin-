//! **La pagina web de un solo archivo, la misma que saca el movil.**
//!
//! Es «lo que hace distinta» a la aplicacion segun su README: un `.html` que
//! se manda por donde sea y se abre en cualquier navegador sin instalar nada,
//! con el dibujo dentro y un visor escrito en la propia pagina (acercar,
//! desplazar, lapiz, resaltador, borrador, medir, presentar, imprimir por
//! marcos y guardar lo anotado en el mismo fichero).
//!
//! **El visor no se reescribe: se copia.** La hoja de estilo, el visor del
//! dibujo y el armazon son texto del movil (`motor/ExportarHtml.kt`,
//! `ESTILO`, `VISOR_DIBUJO` y `SHELL`), guardados tal cual en
//! `exportar_html/`. Lo que se porta aqui es solo lo que los envuelve —el
//! `<head>`, el indice de hojas y la barra de botones de `barra()`—, con los
//! mismos ids y clases. Asi la pagina que sale del PC y la que sale del
//! telefono son la misma pagina: se abren igual en los dos, y lo que se anota
//! y se guarda en una se lee en la otra. Si el movil cambia su visor, se
//! vuelven a copiar los tres ficheros y nada mas.
//!
//! Se portan las clases de hoja que el PC sabe producir: el dibujo
//! (`HojaWeb.Dibujo`), el documento ([`documento`]) y la tabla que sigue
//! calculando ([`HojaTabla`], con `VisorTabla` del movil copiado en
//! `exportar_html/visor_tabla.*` y `calculo_tabla.js`). El croquis del
//! espacio vive en otro fichero del movil (`VisorEspacio`) y el armazon solo
//! lo llama si hay hojas de esa clase, asi que no hace falta traerlo.
//!
//! Ademas, **el lienzo viaja dentro** como `.excalidraw` (en un
//! `<script type="application/json" class="excalidraw">`): el visor lo
//! ignora, y quien quiera volver a editar el dibujo, en el PC o en el movil,
//! lo tiene entero y no solo como SVG. El guardar de la pagina no lo toca
//! (solo reescribe los grupos `#croquis`).

use crate::exportar_svg::escapar;

pub mod documento;
pub use documento::HojaDocumento;

/// La hoja de estilo del movil, con `FONDO` por sustituir.
const ESTILO: &str = include_str!("exportar_html/estilo.css");
/// El visor de un dibujo plano (`crearDibujo`).
const VISOR_DIBUJO: &str = include_str!("exportar_html/visor_dibujo.js");
/// El armazon: que hoja se mira, la barra, los cajones y guardar.
const ARMAZON: &str = include_str!("exportar_html/armazon.js");
/// La tabla que sigue calculando (J3): `VisorTabla.ESTILO`, `CALCULO` (el
/// motor de formulas del movil escrito otra vez en JavaScript) y `JS`
/// (`crearTabla`), copiados tal cual del movil. Solo van si hay tablas.
const ESTILO_TABLA: &str = include_str!("exportar_html/visor_tabla.css");
const CALCULO_TABLA: &str = include_str!("exportar_html/calculo_tabla.js");
const VISOR_TABLA: &str = include_str!("exportar_html/visor_tabla.js");

/// El grupo del SVG donde viven las anotaciones. El visor lo busca por este
/// id, y guardar es rellenarlo.
pub const ID_DEL_CROQUIS: &str = "croquis";

/// Los colores del lapiz de la pagina: los del movil, chillones a proposito
/// para que una anotacion se vea encima de cualquier plano.
pub const COLORES: [&str; 9] = [
    "#ff1744", "#ff9100", "#ffea00", "#00e676", "#00e5ff", "#2979ff", "#d500f9", "#ffffff",
    "#111111",
];

/// Los grosores del lapiz, en pixeles de pantalla.
pub const GROSORES: [u32; 3] = [2, 4, 9];

/// Una hoja de la pagina: un dibujo como SVG, con el color de su papel.
#[derive(Debug, Clone, PartialEq)]
pub struct HojaWeb {
    pub nombre: String,
    pub svg: String,
    /// `#rrggbb`. La barra toma el de la primera y el fondo de la ventana
    /// cambia al de la hoja que se mira.
    pub fondo: String,
}

/// Que lleva la pagina (`ExportarHtml.Opciones` del movil). Lo que se apaga
/// no va: ni su boton ni su atajo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Opciones {
    pub lapiz: bool,
    pub resaltador: bool,
    pub borrador: bool,
    pub deshacer: bool,
    pub medir: bool,
    pub paginas: bool,
    pub guardar: bool,
    pub compartir: bool,
}

impl Default for Opciones {
    fn default() -> Self {
        Self {
            lapiz: true,
            resaltador: true,
            borrador: true,
            deshacer: true,
            medir: true,
            paginas: true,
            guardar: true,
            compartir: true,
        }
    }
}

impl Opciones {
    /// Las herramientas que se dejan coger, por su nombre en el visor.
    fn herramientas(&self) -> String {
        let mut v = vec!["mano"];
        if self.lapiz {
            v.push("lapiz");
        }
        if self.resaltador {
            v.push("marcador");
        }
        if self.borrador {
            v.push("goma");
        }
        if self.medir {
            v.push("medir");
        }
        v.push("girar");
        v.push("mover");
        v.join(" ")
    }
}

/// Si un color `#rrggbb` es mas bien oscuro. Con lo que no se entienda,
/// claro. La misma cuenta que el movil.
fn es_oscuro(color: &str) -> bool {
    let hex = color.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return false;
    }
    let Ok(n) = u32::from_str_radix(hex, 16) else {
        return false;
    };
    let (r, g, b) = ((n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff);
    (0.299 * r as f64 + 0.587 * g as f64 + 0.114 * (b as f64)) < 128.0
}

fn icono(camino: &str) -> String {
    format!(
        "<svg viewBox=\"0 0 24 24\" width=\"22\" height=\"22\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\" aria-hidden=\"true\"><path d=\"{camino}\"/></svg>"
    )
}

fn boton(id: &str, titulo: &str, camino: &str, clase: &str) -> String {
    let clase = if clase.is_empty() {
        String::new()
    } else {
        format!(" class=\"{clase}\"")
    };
    format!(
        "<button id=\"{id}\" title=\"{titulo}\" aria-label=\"{titulo}\"{clase}>{}</button>",
        icono(camino)
    )
}

// Los caminos de los iconos, los mismos del movil.
const MANO: &str = "M8 13V5a2 2 0 1 1 4 0v6M12 11V4a2 2 0 1 1 4 0v7M16 12V6a2 2 0 1 1 4 0v8a7 7 0 0 1-7 7h-1a7 7 0 0 1-6-3.4L3.5 13a2 2 0 0 1 3.4-2.1L8 13";
const LAPIZ: &str = "M4 20l4-1L19.5 7.5a2.1 2.1 0 0 0-3-3L5 16l-1 4zM14 6l4 4";
const MARCADOR: &str = "M4 20h16M6 16l8.5-8.5a2.1 2.1 0 0 1 3 3L9 19H6v-3zM13 8l3 3";
const GOMA: &str = "M20 20H8M4.6 14.4l8.8-8.8a2 2 0 0 1 2.8 0l3.2 3.2a2 2 0 0 1 0 2.8L13 18H8.6l-4-4a1 1 0 0 1 0-1.4zM9.5 9.5l5 5";
const ENCAJAR: &str = "M4 9V5a1 1 0 0 1 1-1h4M15 4h4a1 1 0 0 1 1 1v4M20 15v4a1 1 0 0 1-1 1h-4M9 20H5a1 1 0 0 1-1-1v-4";

/// La barra: una sola, abajo y centrada, con los mismos ids que la del
/// movil (`ExportarHtml.barra`), en la variante de un documento de dibujos.
fn barra(varias: bool, o: &Opciones, tabla: bool) -> String {
    let mut s = String::with_capacity(8192);
    s.push_str("<div id=\"estado\" role=\"status\" aria-live=\"polite\"></div>\n");
    // La pastilla de la presentacion.
    s.push_str("<div id=\"presentacion\" hidden>");
    s.push_str(&boton("p-anterior", "Anterior (←)", "M15 5l-7 7 7 7", ""));
    s.push_str("<span id=\"p-cuenta\"></span>");
    s.push_str(&boton("p-siguiente", "Siguiente (→)", "M9 5l7 7-7 7", ""));
    s.push_str("<i class=\"p-sep\"></i>");
    for (m, titulo, camino) in [
        ("mano", "Pasar", MANO),
        ("lapiz", "Lápiz", LAPIZ),
        ("marcador", "Resaltador", MARCADOR),
        ("goma", "Borrador", GOMA),
    ] {
        s.push_str(&format!(
            "<button data-m=\"{m}\" title=\"{titulo}\" aria-label=\"{titulo}\">{}</button>",
            icono(camino)
        ));
    }
    s.push_str("<i class=\"p-sep\"></i>");
    s.push_str(&boton("p-encajar", "Encajar (0)", ENCAJAR, ""));
    s.push_str(&boton("p-salir", "Salir (Esc)", "M6 6l12 12M18 6L6 18", ""));
    s.push_str("</div>\n");
    s.push_str("<div id=\"cajon\" hidden></div>\n");
    // Presentar e imprimir, arriba, lejos de la mano que dibuja.
    s.push_str("<div id=\"arriba\">");
    s.push_str(&boton(
        "presentar",
        "Presentar (F5)",
        "M3 4h18v12H3zM12 16v4M8 20h8M10 8l5 2.5-5 2.5z",
        "",
    ));
    s.push_str(&boton(
        "marcar-zona",
        "Imprimir una zona",
        "M4 8V4h4M16 4h4v4M20 16v4h-4M8 20H4v-4M9 9h6v6H9z",
        "",
    ));
    s.push_str(&boton(
        "imprimir",
        "Imprimir (Ctrl+P)",
        "M7 8V3h10v5M7 17H4v-7h16v7h-3M7 14h10v7H7z",
        "",
    ));
    s.push_str("</div>\n");
    s.push_str("<div id=\"pizarra\"");
    if varias {
        s.push_str(" class=\"varias\"");
    }
    s.push_str(">\n");
    if o.lapiz || o.resaltador {
        s.push_str("<div id=\"paleta\" hidden>");
        s.push_str("<div id=\"colores\" role=\"group\" aria-label=\"Color\">");
        for (i, c) in COLORES.iter().enumerate() {
            let activo = if i == 0 { " activo" } else { "" };
            s.push_str(&format!(
                "<button class=\"color{activo}\" data-color=\"{c}\" style=\"--c:{c}\" title=\"Color\" aria-label=\"Color {c}\"></button>"
            ));
        }
        s.push_str("</div>");
        s.push_str("<div id=\"grosores\" role=\"group\" aria-label=\"Grosor\">");
        for (i, g) in GROSORES.iter().enumerate() {
            let activo = if i == 1 { " activo" } else { "" };
            let lado = 5 + i * 5;
            s.push_str(&format!(
                "<button class=\"grosor{activo}\" data-grosor=\"{g}\" title=\"Grosor\" aria-label=\"Grosor {g}\"><i style=\"width:{lado}px;height:{lado}px\"></i></button>"
            ));
        }
        s.push_str("</div>");
        s.push_str("</div>\n");
    }
    s.push_str("<div id=\"barra\" role=\"toolbar\">");
    let pinta = o.lapiz || o.resaltador || o.borrador;
    if pinta {
        s.push_str("<div class=\"grupo solo-dibujo\">");
        s.push_str(&boton("mano", "Mover (V)", MANO, "activo"));
        if o.lapiz {
            s.push_str(&boton("lapiz", "Lápiz (P)", LAPIZ, ""));
        }
        if o.resaltador {
            s.push_str(&boton("marcador", "Resaltador (M)", MARCADOR, ""));
        }
        if o.borrador {
            s.push_str(&boton("goma", "Borrador (E)", GOMA, ""));
        }
        s.push_str("</div>");
    }
    if o.medir {
        s.push_str("<div class=\"grupo solo-medir\">");
        s.push_str(&boton(
            "medir",
            "Medir (D)",
            "M3 14.5 14.5 3 21 9.5 9.5 21zM7 11l2 2M10 8l2 2M13 5l2 2",
            "",
        ));
        s.push_str("</div>");
    }
    if o.deshacer && pinta {
        s.push_str("<div class=\"grupo solo-dibujo\">");
        s.push_str(&boton(
            "deshacer",
            "Deshacer (Ctrl+Z)",
            "M9 14L4 9l5-5M4 9h9a6 6 0 0 1 0 12h-3",
            "",
        ));
        s.push_str(&boton(
            "rehacer",
            "Rehacer (Ctrl+Y)",
            "M15 14l5-5-5-5M20 9h-9a6 6 0 0 0 0 12h3",
            "",
        ));
        s.push_str("</div>");
    }
    s.push_str("<div class=\"grupo\">");
    s.push_str(&boton("menos", "Alejar (−)", "M5 12h14", "solo-raton"));
    s.push_str(&boton("encajar", "Encajar (0)", ENCAJAR, ""));
    s.push_str(&boton(
        "mas",
        "Acercar (+)",
        "M12 5v14M5 12h14",
        "solo-raton",
    ));
    s.push_str("</div>");
    if tabla {
        // **La tabla**: negrita, Σ, y el portapapeles con boton, que en un
        // telefono no hay Ctrl+C; y el CSV, que abre cualquier hoja de
        // calculo. Deshacer: si ya esta el de la tinta, ese mismo deshace
        // tambien las celdas (`barra` del movil, rama `tabla`).
        let mut primeros = String::new();
        if o.deshacer && !pinta {
            primeros.push_str(&boton(
                "t-deshacer",
                "Deshacer (Ctrl+Z)",
                "M9 14L4 9l5-5M4 9h9a6 6 0 0 1 0 12h-3",
                "",
            ));
            primeros.push_str(&boton(
                "t-rehacer",
                "Rehacer (Ctrl+Y)",
                "M15 14l5-5-5-5M20 9h-9a6 6 0 0 0 0 12h3",
                "",
            ));
        }
        primeros.push_str(&boton(
            "t-negrita",
            "Negrita (Ctrl+B)",
            "M7 5h6a4 4 0 0 1 0 8H7zM7 13h7a4 4 0 0 1 0 8H7z",
            "",
        ));
        primeros.push_str(&boton("t-suma", "Autosuma", "M18 5H6l6 7-6 7h12", ""));
        s.push_str("<div class=\"grupo solo-tabla\">");
        s.push_str(&primeros);
        s.push_str("</div>");
        s.push_str("<div class=\"grupo solo-tabla\">");
        s.push_str(&boton(
            "t-copiar",
            "Copiar (Ctrl+C)",
            "M9 9h11v11H9zM5 15H4V4h11v1",
            "",
        ));
        s.push_str(&boton(
            "t-pegar",
            "Pegar (Ctrl+V)",
            "M9 3h6v4H9zM7 5H5v16h14V5h-2M9 12h6M9 16h4",
            "",
        ));
        s.push_str(&boton(
            "t-csv",
            "Bajar CSV",
            "M12 4v11M7 10l5 5 5-5M5 20h14",
            "",
        ));
        s.push_str("</div>");
    }
    if o.guardar || o.compartir {
        s.push_str("<div class=\"grupo\">");
        if o.guardar {
            s.push_str(&boton(
                "guardar",
                "Guardar (Ctrl+S)",
                "M5 4h11l3 3v13H5zM8 4v5h7V4M8 20v-6h8v6",
                "",
            ));
        }
        if o.compartir {
            s.push_str(&boton(
                "compartir",
                "Compartir",
                "M12 3v12M8 7l4-4 4 4M5 12v7a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-7",
                "",
            ));
        }
        s.push_str("</div>");
    }
    s.push_str("</div>\n");
    s.push_str("<div id=\"aviso\" role=\"status\" hidden></div>\n");
    s.push_str("</div>\n");
    s
}

/// El grupo de las anotaciones, el ultimo del SVG: las notas van encima del
/// dibujo y pasean y se amplian con el. Va ya en el fichero, vacio, para que
/// guardar sea rellenarlo (`conGrupoDeCroquis` del movil).
fn con_grupo_de_croquis(svg: &str) -> String {
    let cuerpo = svg
        .trim()
        .trim_start_matches("<?xml version=\"1.0\" encoding=\"UTF-8\"?>")
        .trim();
    if !cuerpo.ends_with("</svg>") || cuerpo.contains(&format!("id=\"{ID_DEL_CROQUIS}\"")) {
        return cuerpo.to_string();
    }
    format!(
        "{}\n<g id=\"{ID_DEL_CROQUIS}\"></g>\n</svg>",
        cuerpo.trim_end_matches("</svg>").trim_end()
    )
}

/// **La caja de un documento** (la rama `HojaWeb.Documento` del movil): mide
/// la columna mas sus dos margenes; el texto va en `article.doc`, lo anotado
/// en la aplicacion encima y la tinta del navegador **dentro**, asi se
/// desplaza y se amplia con el documento. `data-tops` es a que altura caia
/// cada bloque al exportar: el guion (`crearNota`) corre cada pieza lo que
/// se haya corrido el suyo con las letras de este navegador.
fn caja_de_documento(d: &HojaDocumento, s: &mut String) {
    let tops: Vec<String> = d
        .tops
        .iter()
        .map(|t| format!("{:.1}", (t * 10.0).round() / 10.0))
        .collect();
    let letra = format!("{:.2}", d.letra)
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string();
    s.push_str(&format!(
        "<div class=\"doc-caja\" data-columna=\"{}\" data-margen=\"{}\" data-letra=\"{letra}\" data-bloques=\"{}\" data-tops=\"{}\" style=\"width:{}px\">",
        d.columna,
        d.margen,
        escapar(&d.bloques),
        tops.join(","),
        d.columna + 2 * d.margen
    ));
    s.push_str(&format!(
        "<article class=\"doc\" style=\"width:{}px;margin-left:{}px;font-size:{letra}px\">",
        d.columna, d.margen
    ));
    s.push_str(&d.cuerpo);
    s.push_str("</article>");
    s.push_str(&d.capa);
    s.push_str(&format!(
        "\n<svg class=\"tinta\"><g id=\"{ID_DEL_CROQUIS}\"></g></svg></div>"
    ));
    // Fuera de la caja, que se amplia con `transform`: ver `riel_de`.
    s.push_str(&d.riel);
}

/// **La caja de una tabla** (la rama `HojaWeb.Tabla` del movil): la barra
/// de formula arriba, la tabla ya calculada en medio con la tinta del
/// navegador dentro (asi se desplaza con las celdas) y lo escrito al final.
/// Guardar la pagina reescribe las dos ultimas.
fn caja_de_tabla(t: &HojaTabla, s: &mut String) {
    s.push_str("<div class=\"tabla-fx\"><span class=\"tabla-dir\">A1</span>");
    s.push_str(
        "<input class=\"tabla-fx-in\" type=\"text\" spellcheck=\"false\" autocomplete=\"off\" ",
    );
    s.push_str("autocapitalize=\"off\" aria-label=\"Contenido de la celda\" placeholder=\"Valor o =SUMA(A1:A3)\"/></div>\n");
    s.push_str(&format!(
        "<div class=\"tabla-caja\" tabindex=\"0\"><svg class=\"tinta\"><g class=\"origen\"><g id=\"{ID_DEL_CROQUIS}\"></g></g></svg>"
    ));
    s.push_str(&t.estatica);
    s.push_str("</div>\n");
    s.push_str("<script type=\"application/json\" class=\"tabla\">");
    // El JSON ya viene sin `<`; por si acaso, que no cierre su `script`.
    s.push_str(&t.json.replace('<', "\\u003c"));
    s.push_str("</script>");
}

/// Un texto metido en un `<script>` de datos: el navegador no lo ejecuta y no
/// hay que escapar comillas ni acentos, solo la etiqueta de cierre.
fn como_datos(json: &str) -> String {
    json.replace("</", "<\\/")
}

/// Los ficheros copiados del movil llegan con saltos de Windows si git los
/// convirtio al sacarlos: se dejan como los escribe el movil, para que la
/// pagina sea byte a byte la misma.
fn sin_retornos(s: &str) -> std::borrow::Cow<'_, str> {
    if s.contains('\r') {
        std::borrow::Cow::Owned(s.replace("\r\n", "\n"))
    } else {
        std::borrow::Cow::Borrowed(s)
    }
}

/// **La pagina entera.** `titulo` es el de la ventana; `nombre`, como se
/// llamara el fichero al guardarlo desde el navegador. `excalidraw` es el
/// lienzo que viaja dentro, si se quiere.
///
/// Devuelve `None` sin hojas: un documento sin hojas no es un documento.
pub fn paginas(
    hojas: &[HojaWeb],
    titulo: &str,
    nombre: &str,
    opciones: Opciones,
    excalidraw: Option<&str>,
) -> Option<String> {
    let mixtas: Vec<HojaDeLaPagina<'_>> = hojas.iter().map(HojaDeLaPagina::Dibujo).collect();
    paginas_mixtas(&mixtas, titulo, nombre, opciones, excalidraw)
}

/// **Una hoja de la pagina, de la clase que sea**: un dibujo (SVG) o un
/// documento para anotar (`HojaWeb.Documento` del movil, ver [`documento`]).
/// Va por referencia: las hojas pesan megas y no hay por que copiarlas para
/// escribirlas.
#[derive(Debug, Clone, Copy)]
pub enum HojaDeLaPagina<'a> {
    Dibujo(&'a HojaWeb),
    Documento(&'a HojaDocumento),
    /// Una tabla que sigue calculando en el navegador (J3).
    Tabla(&'a HojaTabla),
}

/// **Una tabla con formulas** (`HojaWeb.Tabla` del movil): viaja lo escrito
/// --el JSON de la tabla, el mismo que guarda el proyecto-- y la tabla ya
/// calculada, para leerse sin guion (hay visores de chat que no los
/// ejecutan). Con guion, `crearTabla` la rehace desde el JSON y a partir de
/// ahi recalcula, se edita, se pega desde Excel y se guarda. Las dos piezas
/// las hace quien sabe de tablas (`pixpin_proyecto::tabla_web`): este crate no
/// las conoce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HojaTabla {
    pub nombre: String,
    pub fondo: String,
    /// El JSON de la tabla, sin un solo `<`.
    pub json: String,
    /// `<table class="calc">` ya calculada.
    pub estatica: String,
    /// El separador decimal con que se ensena lo calculado (`,` o `.`): el
    /// del usuario, para que un total salga como lo escrito a mano. Va a la
    /// pagina como `data-decimal` y el motor de la tabla lo usa.
    pub decimal: char,
}

impl HojaDeLaPagina<'_> {
    fn nombre(&self) -> &str {
        match self {
            HojaDeLaPagina::Dibujo(h) => &h.nombre,
            HojaDeLaPagina::Documento(h) => &h.nombre,
            HojaDeLaPagina::Tabla(h) => &h.nombre,
        }
    }

    fn fondo(&self) -> &str {
        match self {
            HojaDeLaPagina::Dibujo(h) => &h.fondo,
            HojaDeLaPagina::Documento(h) => &h.fondo,
            HojaDeLaPagina::Tabla(h) => &h.fondo,
        }
    }

    fn peso(&self) -> usize {
        match self {
            HojaDeLaPagina::Dibujo(h) => h.svg.len(),
            HojaDeLaPagina::Documento(h) => h.cuerpo.len() + h.capa.len() + h.estilo.len(),
            HojaDeLaPagina::Tabla(h) => h.json.len() + h.estatica.len(),
        }
    }
}

/// **La pagina entera con hojas de cualquier clase** (`paginas` del movil
/// con `HojaWeb` sellada). Un documento va como nota (`data-tipo="nota"`)
/// con su `.doc-caja`: la columna, sus dos margenes, lo anotado atado a sus
/// bloques y la capa donde se sigue rayando en el navegador. Su hoja de
/// estilo, ya acotada a `.doc`, va detras de la de la pagina.
pub fn paginas_mixtas(
    hojas: &[HojaDeLaPagina<'_>],
    titulo: &str,
    nombre: &str,
    opciones: Opciones,
    excalidraw: Option<&str>,
) -> Option<String> {
    let primera = hojas.first()?;
    let fondo = primera.fondo();
    let oscuro = es_oscuro(fondo);
    let tamano: usize = hojas.iter().map(HojaDeLaPagina::peso).sum::<usize>()
        + excalidraw.map_or(0, str::len)
        + 100_000;
    let mut s = String::with_capacity(tamano);
    s.push_str("<!DOCTYPE html>\n<html lang=\"es\">\n<head>\n");
    s.push_str("<meta charset=\"utf-8\"/>\n");
    s.push_str(
        "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1, viewport-fit=cover\"/>\n",
    );
    s.push_str(&format!(
        "<meta name=\"color-scheme\" content=\"{}\"/>\n",
        if oscuro { "dark" } else { "light" }
    ));
    s.push_str("<meta name=\"generator\" content=\"PixPin\"/>\n");
    s.push_str(&format!("<title>{}</title>\n", escapar(titulo)));
    s.push_str("<style>");
    let hay_tabla = hojas.iter().any(|h| matches!(h, HojaDeLaPagina::Tabla(_)));
    s.push_str(&sin_retornos(ESTILO).replace("FONDO", fondo));
    if hay_tabla {
        s.push('\n');
        s.push_str(&sin_retornos(ESTILO_TABLA));
    }
    for h in hojas {
        if let HojaDeLaPagina::Documento(d) = h {
            s.push('\n');
            s.push_str(&d.estilo);
        }
    }
    s.push_str("</style>\n</head>\n");
    s.push_str(&format!(
        "<body data-nombre=\"{}\" data-herramientas=\"{}\"",
        escapar(nombre),
        opciones.herramientas()
    ));
    // El separador decimal de las tablas (el de la primera): el motor de
    // formulas lo lee de aqui al ensenar un numero calculado.
    if let Some(HojaDeLaPagina::Tabla(t)) =
        hojas.iter().find(|h| matches!(h, HojaDeLaPagina::Tabla(_)))
        && t.decimal != '.'
    {
        s.push_str(&format!(
            " data-decimal=\"{}\"",
            escapar(&t.decimal.to_string())
        ));
    }
    if oscuro {
        s.push_str(" class=\"oscuro\"");
    }
    s.push_str(">\n");
    // El indice, a la vista y con enlaces, con varias hojas.
    if hojas.len() > 1 && opciones.paginas {
        let nombre_de = |i: usize, h: &HojaDeLaPagina<'_>| {
            if h.nombre().trim().is_empty() {
                format!("Hoja {}", i + 1)
            } else {
                h.nombre().to_string()
            }
        };
        s.push_str(&format!(
            "<details id=\"indice-fijo\" open><summary><span class=\"titulo\">{}</span><span class=\"cuenta\">1 / {}</span></summary><nav>",
            escapar(&nombre_de(0, primera)),
            hojas.len()
        ));
        for (i, h) in hojas.iter().enumerate() {
            s.push_str(&format!(
                "<a href=\"#hoja-{}\" data-i=\"{i}\"><b>{}</b> {}</a>",
                i + 1,
                i + 1,
                escapar(&nombre_de(i, h))
            ));
        }
        s.push_str("</nav></details>\n");
    }
    s.push_str("<div id=\"lienzo\">\n");
    for (i, h) in hojas.iter().enumerate() {
        let nombre = if h.nombre().trim().is_empty() {
            format!("Hoja {}", i + 1)
        } else {
            h.nombre().to_string()
        };
        let tipo = match h {
            HojaDeLaPagina::Dibujo(_) => "dibujo",
            HojaDeLaPagina::Documento(_) => "nota",
            HojaDeLaPagina::Tabla(_) => "tabla",
        };
        s.push_str(&format!(
            "<div class=\"hoja\" id=\"hoja-{}\" data-tipo=\"{tipo}\" data-fondo=\"{}\" data-oscuro=\"{}\" data-nombre=\"{}\"",
            i + 1,
            escapar(h.fondo()),
            if es_oscuro(h.fondo()) { "1" } else { "0" },
            escapar(&nombre)
        ));
        if i > 0 {
            s.push_str(" hidden");
        }
        s.push_str(">\n");
        match h {
            HojaDeLaPagina::Dibujo(d) => s.push_str(&con_grupo_de_croquis(&d.svg)),
            HojaDeLaPagina::Documento(d) => caja_de_documento(d, &mut s),
            HojaDeLaPagina::Tabla(t) => caja_de_tabla(t, &mut s),
        }
        s.push_str("\n</div>\n");
    }
    s.push_str("</div>\n");
    s.push_str(&barra(
        hojas.len() > 1 && opciones.paginas,
        &opciones,
        hay_tabla,
    ));
    if let Some(json) = excalidraw {
        s.push_str("<script type=\"application/json\" class=\"excalidraw\">");
        s.push_str(&como_datos(json));
        s.push_str("</script>\n");
    }
    s.push_str("<script>");
    s.push_str(&sin_retornos(VISOR_DIBUJO));
    if hay_tabla {
        s.push_str(&sin_retornos(CALCULO_TABLA));
        s.push_str(&sin_retornos(VISOR_TABLA));
    }
    s.push_str(&sin_retornos(ARMAZON));
    s.push_str("</script>\n</body>\n</html>\n");
    Some(s)
}

/// El nombre del fichero que se sugiere al guardar la pagina: sin los
/// caracteres que Windows no admite en un nombre.
pub fn nombre_de_fichero(nombre: &str) -> String {
    let limpio: String = nombre
        .chars()
        .map(|c| if "\\/:*?\"<>|".contains(c) { '-' } else { c })
        .collect();
    let limpio = limpio.trim();
    if limpio.is_empty() {
        "dibujo".into()
    } else {
        limpio.into()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn svg(ancho: u32) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{ancho}\" height=\"200\" viewBox=\"0 0 {ancho} 200\">\n<path d=\"M0 0L9 9\"/>\n</svg>\n"
        )
    }

    fn hoja(nombre: &str) -> HojaWeb {
        HojaWeb {
            nombre: nombre.into(),
            svg: svg(300),
            fondo: "#ffffff".into(),
        }
    }

    fn cuantas(texto: &str, aguja: &str) -> usize {
        texto.matches(aguja).count()
    }

    #[test]
    fn sin_hojas_no_hay_pagina() {
        assert!(paginas(&[], "t", "t", Opciones::default(), None).is_none());
    }

    #[test]
    fn una_hoja_sola_no_lleva_indice_ni_se_esconde() {
        let html = paginas(
            &[hoja("")],
            "Mi dibujo",
            "Mi dibujo",
            Opciones::default(),
            None,
        )
        .expect("hay hoja");
        assert!(html.starts_with("<!DOCTYPE html>\n<html lang=\"es\">"));
        assert_eq!(cuantas(&html, "class=\"hoja\""), 1);
        assert!(!html.contains("class=\"varias\""));
        assert!(!html.contains("<details id=\"indice-fijo\""));
        assert!(!html.contains("data-nombre=\"Hoja 1\" hidden"));
        assert!(html.contains("viewBox=\"0 0 300 200\""));
        // El grupo de las anotaciones, vacio y el ultimo del SVG.
        assert!(!html.contains("<path d=\"M0 0L9 9\"/>\n</svg>"));
        assert!(html.contains("<g id=\"croquis\"></g>\n</svg>"));
        // Y el prologo XML fuera: dentro de un HTML es un error.
        assert!(!html.contains("<?xml"));
    }

    #[test]
    fn varias_hojas_llevan_su_indice_y_las_demas_escondidas() {
        let html = paginas(
            &[hoja("Planta"), hoja("Alzado"), hoja("")],
            "Casa",
            "Casa",
            Opciones::default(),
            None,
        )
        .expect("hay hojas");
        assert_eq!(cuantas(&html, "class=\"hoja\""), 3);
        assert_eq!(cuantas(&html, "data-tipo=\"dibujo\""), 3);
        assert!(html.contains("data-nombre=\"Alzado\" hidden>"));
        assert!(!html.contains("data-nombre=\"Planta\" hidden>"));
        assert!(html.contains("<div id=\"pizarra\" class=\"varias\">"));
        assert!(
            html.contains(
                "<span class=\"titulo\">Planta</span><span class=\"cuenta\">1 / 3</span>"
            )
        );
        assert!(html.contains("href=\"#hoja-2\""));
        assert!(html.contains("data-nombre=\"Hoja 3\""));
        // Solo en el lienzo: el armazon tambien nombra el grupo en su texto.
        let lienzo = &html[..html.find("<div id=\"estado\"").expect("barra")];
        assert_eq!(cuantas(lienzo, "<g id=\"croquis\"></g>"), 3);
    }

    #[test]
    fn la_pagina_lleva_el_visor_y_el_armazon_del_movil_enteros() {
        let html = paginas(&[hoja("")], "t", "t", Opciones::default(), None).expect("hoja");
        assert!(html.contains("function crearDibujo(caja, api){"));
        assert!(
            html.contains("var PLANTILLA='<!DOCTYPE html>\\n'+document.documentElement.outerHTML;")
        );
        assert!(html.contains("window.paginaAnotada=paginaAnotada;"));
        // El fondo de la hoja de estilo, puesto.
        assert!(!html.contains("FONDO"));
        // Los mandos de dibujar con los ids que busca el armazon.
        for id in [
            "lapiz",
            "marcador",
            "goma",
            "medir",
            "deshacer",
            "guardar",
            "imprimir",
            "presentar",
        ] {
            assert!(html.contains(&format!("id=\"{id}\"")), "falta {id}");
        }
        // Ningun retorno de carro: la pagina es la del movil.
        assert!(!html.contains('\r'));
    }

    #[test]
    fn lo_que_se_apaga_no_va_ni_en_la_barra_ni_en_las_herramientas() {
        let o = Opciones {
            lapiz: false,
            resaltador: false,
            borrador: false,
            guardar: false,
            compartir: false,
            ..Opciones::default()
        };
        let html = paginas(&[hoja("")], "t", "t", o, None).expect("hoja");
        assert!(!html.contains("id=\"lapiz\""));
        assert!(!html.contains("id=\"guardar\""));
        assert!(!html.contains("id=\"paleta\""));
        assert!(html.contains("data-herramientas=\"mano medir girar mover\""));
    }

    #[test]
    fn el_lienzo_viaja_dentro_sin_poder_cerrar_su_script() {
        let json = r#"{"type":"excalidraw","elements":[{"text":"</script><b>"}]}"#;
        let html = paginas(&[hoja("")], "t", "t", Opciones::default(), Some(json)).expect("hoja");
        assert!(html.contains("<script type=\"application/json\" class=\"excalidraw\">"));
        assert!(html.contains("<\\/script><b>"));
        // Caso negativo: la etiqueta de cierre del usuario no cierra nada.
        assert!(!html.contains("\"</script><b>"));
    }

    #[test]
    fn un_papel_oscuro_pone_la_pagina_en_modo_noche() {
        let mut h = hoja("");
        h.fondo = "#14161c".into();
        let html = paginas(&[h], "t", "t", Opciones::default(), None).expect("hoja");
        assert!(html.contains("content=\"dark\""));
        assert!(html.contains("class=\"oscuro\""));
        assert!(html.contains("data-oscuro=\"1\""));
        // Caso negativo: un color que no se entiende cuenta como claro.
        assert!(!es_oscuro("rojo"));
    }

    #[test]
    fn el_titulo_y_los_nombres_van_escapados() {
        let html =
            paginas(&[hoja("<b>")], "a & b", "x\"y", Opciones::default(), None).expect("hoja");
        assert!(html.contains("<title>a &amp; b</title>"));
        assert!(html.contains("data-nombre=\"x&quot;y\""));
        assert!(html.contains("data-nombre=\"&lt;b&gt;\""));
    }

    fn documento() -> HojaDocumento {
        HojaDocumento {
            nombre: "Apuntes".into(),
            estilo: ".doc p{margin:0}".into(),
            cuerpo: "<p data-b>Hola</p><p data-b>Adios</p>".into(),
            capa: "<span class=\"ppm\" id=\"ppm-d-0\" data-i=\"1\" data-y=\"40\">⭐</span>".into(),
            columna: 600,
            margen: 400,
            tops: vec![32.0, 57.25],
            letra: 17.5,
            fondo: "#121316".into(),
            bloques: documento::SELECTOR_MARCADO.into(),
            riel: "<div class=\"pprail\"><button data-m=\"ppm-d-0\">⭐</button></div>".into(),
        }
    }

    #[test]
    fn un_documento_va_como_nota_con_su_caja_sus_medidas_y_sus_mandos() {
        let d = documento();
        let html = paginas_mixtas(
            &[HojaDeLaPagina::Documento(&d)],
            "Apuntes",
            "Apuntes (anotado)",
            Opciones::default(),
            None,
        )
        .expect("hoja");
        assert!(html.contains("data-tipo=\"nota\""));
        assert!(html.contains(
            "<div class=\"doc-caja\" data-columna=\"600\" data-margen=\"400\" data-letra=\"17.5\" data-bloques=\"[data-b]\" data-tops=\"32.0,57.3\" style=\"width:1400px\">"
        ));
        assert!(html.contains("<article class=\"doc\" style=\"width:600px;margin-left:400px;font-size:17.5px\"><p data-b>Hola</p>"));
        // Lo anotado y la tinta del navegador, dentro de la caja.
        assert!(html.contains("⭐</span>\n<svg class=\"tinta\"><g id=\"croquis\"></g></svg></div><div class=\"pprail\">"), "el riel, fuera de la caja");
        // Su estilo, en la cabecera; y el papel oscuro, en la pagina.
        assert!(html.contains("\n.doc p{margin:0}</style>"));
        assert!(html.contains("class=\"oscuro\""));
        for id in [
            "lapiz", "marcador", "goma", "deshacer", "guardar", "imprimir", "mas", "menos",
        ] {
            assert!(html.contains(&format!("id=\"{id}\"")), "falta {id}");
        }
        // Caso negativo: un documento no es un dibujo, ni lleva su grupo suelto.
        assert!(!html.contains("data-tipo=\"dibujo\""));
    }

    #[test]
    fn un_dibujo_y_un_documento_conviven_en_la_misma_pagina() {
        let d = documento();
        let h = hoja("Plano");
        let html = paginas_mixtas(
            &[HojaDeLaPagina::Dibujo(&h), HojaDeLaPagina::Documento(&d)],
            "t",
            "t",
            Opciones::default(),
            None,
        )
        .expect("hojas");
        assert!(html.contains(
            "data-tipo=\"dibujo\" data-fondo=\"#ffffff\" data-oscuro=\"0\" data-nombre=\"Plano\">"
        ));
        assert!(html.contains("data-tipo=\"nota\" data-fondo=\"#121316\" data-oscuro=\"1\" data-nombre=\"Apuntes\" hidden>"));
        // La pagina toma el papel de la primera.
        assert!(!html.contains("<body data-nombre=\"t\" data-herramientas=\"mano lapiz marcador goma medir girar mover\" class=\"oscuro\""));
    }

    fn tabla_web() -> HojaTabla {
        HojaTabla {
            nombre: "Gastos".into(),
            fondo: "#ffffff".into(),
            json: r#"{"nombre":"Gastos","celdas":{"A1":"2","A2":"=A1*3 < 9"}}"#.into(),
            estatica: "<table class=\"calc\"><tbody><tr><th>1</th><td class=\"d\">2</td></tr></tbody></table>".into(),
            decimal: ',',
        }
    }

    #[test]
    fn una_tabla_va_con_su_barra_de_formula_su_json_y_el_motor_que_sigue_calculando() {
        let t = tabla_web();
        let html = paginas_mixtas(
            &[HojaDeLaPagina::Tabla(&t)],
            "Gastos",
            "Gastos",
            Opciones::default(),
            None,
        )
        .expect("hoja");
        assert!(html.contains(
            "data-tipo=\"tabla\" data-fondo=\"#ffffff\" data-oscuro=\"0\" data-nombre=\"Gastos\">"
        ));
        assert!(html.contains("<div class=\"tabla-fx\"><span class=\"tabla-dir\">A1</span>"));
        assert!(html.contains("<div class=\"tabla-caja\" tabindex=\"0\"><svg class=\"tinta\"><g class=\"origen\"><g id=\"croquis\"></g></g></svg><table class=\"calc\">"));
        assert!(
            html.contains(
                "<script type=\"application/json\" class=\"tabla\">{\"nombre\":\"Gastos\""
            )
        );
        // El motor de formulas y el visor del movil, y su hoja de estilo.
        assert!(html.contains("var Calculo=(function(){"));
        assert!(html.contains("function crearTabla(d,api){"));
        assert!(html.contains(".tabla-caja{"));
        for id in ["t-negrita", "t-suma", "t-copiar", "t-pegar", "t-csv"] {
            assert!(html.contains(&format!("id=\"{id}\"")), "falta {id}");
        }
        // Lo escrito no puede cerrar su `script`: ningun `</` dentro del JSON.
        let json = html
            .split("class=\"tabla\">")
            .nth(1)
            .unwrap()
            .split("</script>")
            .next()
            .unwrap();
        assert!(!json.contains('<'));
        // La coma del usuario va a la pagina, y el motor la usa al ensenar.
        assert!(html.contains("<body data-nombre=\"Gastos\" data-herramientas=\""));
        assert!(html.contains(" data-decimal=\",\">"));
        assert!(html.contains("function separadorDecimal(){"));
        // Caso negativo: con punto no se escribe nada (es lo de siempre).
        let mut en = tabla_web();
        en.decimal = '.';
        let html = paginas_mixtas(
            &[HojaDeLaPagina::Tabla(&en)],
            "G",
            "G",
            Opciones::default(),
            None,
        )
        .unwrap();
        assert!(!html.contains(" data-decimal=\""));
    }

    #[test]
    fn sin_tablas_la_pagina_no_carga_el_motor_de_las_tablas() {
        let h = hoja("Plano");
        let html = paginas(&[h], "t", "t", Opciones::default(), None).expect("hoja");
        // Caso negativo: 80 kB de guion que nadie va a usar.
        assert!(!html.contains("var Calculo=(function(){"));
        assert!(!html.contains("function crearTabla(d,api){"));
        assert!(!html.contains("id=\"t-csv\""));
    }

    #[test]
    fn el_nombre_de_fichero_no_lleva_lo_que_windows_no_admite() {
        assert_eq!(nombre_de_fichero("a/b:c?"), "a-b-c-");
        assert_eq!(nombre_de_fichero("   "), "dibujo");
    }
}
