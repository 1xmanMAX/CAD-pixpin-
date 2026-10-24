//! **Un documento para anotar como hoja de la pagina web** (`HojaWeb.Documento`
//! y `motor/DocumentoAnotado.kt` del movil, 20/21-sep-2026).
//!
//! Un Word, un libro o un PDF anotado se exporta **como se ve al abrirlo**:
//! el texto (o las hojas) en una columna de ancho fijo, un margen en blanco a
//! cada lado, lo anotado encima como SVG vectorial y los marcadores con su
//! emoticono. Entra en la pagina web de siempre como una hoja mas
//! (`data-tipo="nota"` con su `.doc-caja`), asi que los mandos le vienen
//! dados: lapiz, resaltador, goma, colores, deshacer, guardar, imprimir,
//! presentar y zoom. El guion es el del movil, ya copiado en `armazon.js`
//! (`crearNota`): aqui no hay ni una linea de JavaScript nueva.
//!
//! **Lo anotado va atado al parrafo, no al pixel.** Otro navegador tiene
//! otras letras y el texto no mide lo mismo de alto; una capa entera clavada
//! en coordenadas se iria despegando hacia abajo. Al exportar se mide a que
//! altura caia cada bloque (`tops`); cada anotacion y cada marcador se apunta
//! al bloque junto al que estaba ([`ancla_de`]), y al abrir la pagina el
//! guion vuelve a medir y corre cada cosa lo que se haya corrido su bloque.

use crate::exportar_svg::escapar;

/// Los bloques que mide el movil en su `WebView`. Se deja por referencia: el
/// PC no mide en un navegador sino con su propia disposicion, y marca cada
/// bloque con `data-b` para que la cuenta de un lado y la del otro casen una
/// a una ([`SELECTOR_MARCADO`]).
pub const SELECTOR: &str = "p,h1,h2,h3,h4,h5,h6,li,tr,img,pre,blockquote,figure,hr";

/// **Los bloques que mide el PC**: los que llevan `data-b`. Con el selector
/// del movil una imagen dentro de un parrafo contaba dos veces (el `p` y el
/// `img`) y el orden de las alturas ya no casaba con el de los bloques del
/// lector; marcandolos al escribir no puede pasar.
pub const SELECTOR_MARCADO: &str = "[data-b]";

/// Una pieza de lo anotado: su SVG y donde iba, en pixeles del documento
/// (el cero en la esquina de la caja, margen izquierdo incluido).
#[derive(Debug, Clone, PartialEq)]
pub struct Pieza {
    pub svg: String,
    pub x: f64,
    pub y: f64,
    pub ancho: f64,
    pub alto: f64,
    /// El bloque al que va atada, o -1 si a ninguno.
    pub ancla: i32,
}

/// Un marcador ya situado: a que altura del documento estaba y junto a que
/// bloque.
#[derive(Debug, Clone, PartialEq)]
pub struct Senal {
    pub emoji: String,
    pub y: f64,
    pub ancla: i32,
    /// La fraccion del alto, para cuando no hay ancla (nunca se midio): el
    /// guion la pone a esa fraccion del alto que tenga el documento alli.
    pub fraccion: f64,
}

/// **La hoja de un documento para anotar.** `estilo` es su CSS **ya
/// acotado a `.doc`** ([`acotar`]); `capa`, lo anotado y los marcadores
/// ([`capa_de`]); `tops`, a que altura caia cada bloque al exportar; `letra`,
/// el cuerpo en px; `bloques`, el selector con el que el guion los vuelve a
/// medir.
#[derive(Debug, Clone, PartialEq)]
pub struct HojaDocumento {
    pub nombre: String,
    pub estilo: String,
    pub cuerpo: String,
    pub capa: String,
    pub columna: u32,
    pub margen: u32,
    pub tops: Vec<f64>,
    pub letra: f64,
    pub fondo: String,
    pub bloques: String,
    /// El riel de los marcadores ([`riel_de`]), que va fuera de la caja.
    pub riel: String,
}

/// **El bloque al que se ata algo que estaba a la altura `y`**: el ultimo que
/// empieza por encima, con un poco de holgura (lo escrito al lado de un
/// titulo suele empezar un pelo antes que el). -1 si no hay ninguno:
/// entonces se queda donde estaba. Es `anclaDe` del movil, y el guion hace
/// la misma cuenta con lo que se raya en el navegador.
pub fn ancla_de(y: f64, tops: &[f64], holgura: f64) -> i32 {
    let mut mejor: i32 = -1;
    for (i, t) in tops.iter().enumerate() {
        if *t <= y + holgura && (mejor < 0 || *t >= tops[mejor as usize]) {
            mejor = i as i32;
        }
    }
    mejor
}

/// La holgura de [`ancla_de`], la del movil y la del guion.
pub const HOLGURA: f64 = 6.0;

/// La caja de un SVG, leida de su `viewBox`: x, y, ancho, alto.
pub fn caja_de(svg: &str) -> Option<[f64; 4]> {
    let desde = svg.find("viewBox=\"")? + "viewBox=\"".len();
    let hasta = desde + svg[desde..].find('"')?;
    let n: Vec<f64> = svg[desde..hasta]
        .split_whitespace()
        .filter_map(|v| v.parse().ok())
        .collect();
    (n.len() == 4).then(|| [n[0], n[1], n[2], n[3]])
}

/// El SVG listo para ir dentro de una pagina: sin la cabecera XML, que ahi
/// es un error.
pub fn sin_cabecera(svg: &str) -> &str {
    match svg.find("?>") {
        Some(i) if svg.trim_start().starts_with("<?xml") => svg[i + 2..].trim(),
        _ => svg.trim(),
    }
}

/// Un numero con un decimal y los enteros sin `.0`, como `num` del movil.
fn num(v: f64) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let r = (v * 10.0).round() / 10.0;
    if r == r.trunc() {
        format!("{}", r as i64)
    } else {
        format!("{r}")
    }
}

/// **Lo anotado, por piezas atadas a su bloque, y los marcadores**
/// (`capaDe` del movil, sin el riel: ver [`riel_de`]). Cada pieza es un
/// `svg.ppa` con `data-i` (su bloque) y `data-y` (donde estaba); cada
/// marcador, un `span.ppm` en el canto derecho de la columna. `clave`
/// distingue los ids si hubiera dos documentos en la misma pagina.
pub fn capa_de(
    piezas: &[Pieza],
    senales: &[Senal],
    columna: u32,
    margen: u32,
    clave: &str,
) -> String {
    let mut s = String::new();
    for p in piezas {
        let svg = sin_cabecera(&p.svg);
        let atributos = format!(
            "<svg class=\"ppa\" data-i=\"{}\" data-y=\"{}\" style=\"left:{}px;top:{}px;width:{}px;height:{}px\" ",
            p.ancla,
            num(p.y),
            num(p.x),
            num(p.y),
            num(p.ancho),
            num(p.alto)
        );
        s.push_str(&svg.replacen("<svg ", &atributos, 1));
        s.push('\n');
    }
    let clave = escapar(clave);
    for (k, m) in senales.iter().enumerate() {
        s.push_str(&format!(
            "<span class=\"ppm\" id=\"ppm-{clave}-{k}\" data-i=\"{}\" data-y=\"{}\"",
            m.ancla,
            num(m.y)
        ));
        if m.ancla < 0 && m.fraccion >= 0.0 {
            s.push_str(&format!(" data-f=\"{}\"", m.fraccion));
        }
        s.push_str(&format!(
            " style=\"left:{}px;top:{}px\">{}</span>",
            margen + columna + 6,
            num(m.y),
            escapar(&m.emoji)
        ));
    }
    s
}

/// **El riel de los marcadores**: un boton por marcador que lleva a su
/// `span.ppm`. Va **fuera** de la caja del documento, en la hoja: la caja se
/// amplia con `transform`, y dentro de algo transformado un `position:fixed`
/// deja de estar fijo a la ventana y el riel se iba con el papel, fuera de la
/// vista. El guion lo busca en toda la hoja, asi que ahi sigue funcionando.
pub fn riel_de(senales: &[Senal], clave: &str) -> String {
    if senales.is_empty() {
        return String::new();
    }
    let clave = escapar(clave);
    let mut s = String::from("<div class=\"pprail\">");
    for (k, m) in senales.iter().enumerate() {
        s.push_str(&format!(
            "<button data-m=\"ppm-{clave}-{k}\" title=\"Ir al marcador\">{}</button>",
            escapar(&m.emoji)
        ));
    }
    s.push_str("</div>");
    s
}

/// **La hoja de estilo del documento, encerrada en `.doc`** (`acotar` del
/// movil). Dice `body{…}`, `p{…}`; suelta en la pagina web le cambiaria la
/// letra y los colores a la barra y a las demas hojas. Cada selector se
/// cuelga de `raiz`, y `html`/`body` pasan a ser la raiz misma. Las reglas de
/// `@media` y `@supports` se acotan por dentro; `@page` y compania, que no
/// tienen a que aplicarse, se van. Es para hojas que escribe la aplicacion,
/// no para CSS cualquiera.
pub fn acotar(css: &str, raiz: &str) -> String {
    let mut sale = String::with_capacity(css.len() + css.len() / 4);
    let b = css.as_bytes();
    let mut i = 0usize;
    while i < css.len() {
        let Some(abre) = css[i..].find('{').map(|k| i + k) else {
            break;
        };
        let selector = css[i..abre].trim();
        // La llave que cierra **esta** regla, contando las de dentro.
        let mut hondo = 1;
        let mut j = abre + 1;
        while j < css.len() && hondo > 0 {
            match b[j] {
                b'{' => hondo += 1,
                b'}' => hondo -= 1,
                _ => {}
            }
            j += 1;
        }
        let dentro = &css[abre + 1..(j - 1).max(abre + 1)];
        if selector.starts_with("@media") || selector.starts_with("@supports") {
            sale.push_str(selector);
            sale.push('{');
            sale.push_str(&acotar(dentro, raiz));
            sale.push('}');
        } else if selector.starts_with('@') {
            // `@page`, `@font-face`...: fuera.
        } else {
            let partes: Vec<String> = selector
                .split(',')
                .map(|uno| {
                    let u = uno.trim();
                    for base in ["html", "body"] {
                        if let Some(resto) = u.strip_prefix(base)
                            && !resto
                                .starts_with(|c: char| c.is_alphanumeric() || c == '-' || c == '_')
                        {
                            return format!("{raiz}{resto}");
                        }
                    }
                    format!("{raiz} {u}")
                })
                .collect();
            sale.push_str(&partes.join(","));
            sale.push('{');
            sale.push_str(dentro);
            sale.push('}');
        }
        i = j;
    }
    sale
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn lo_anotado_se_ata_al_ultimo_bloque_que_empieza_por_encima() {
        let tops = [0.0, 100.0, 250.0];
        assert_eq!(ancla_de(120.0, &tops, HOLGURA), 1);
        assert_eq!(ancla_de(260.0, &tops, HOLGURA), 2);
        // Un pelo por encima de un titulo cuenta como suyo.
        assert_eq!(ancla_de(96.0, &tops, HOLGURA), 1);
        // Caso negativo: por encima de todo no hay bloque, y sin medidas tampoco.
        assert_eq!(ancla_de(-50.0, &tops, HOLGURA), -1);
        assert_eq!(ancla_de(50.0, &[], HOLGURA), -1);
    }

    #[test]
    fn la_caja_sale_del_viewbox_y_sin_el_no_hay_caja() {
        let svg = "<?xml version=\"1.0\"?>\n<svg width=\"3\" viewBox=\"-10 20.5 300 40\"></svg>";
        assert_eq!(caja_de(svg), Some([-10.0, 20.5, 300.0, 40.0]));
        assert_eq!(caja_de("<svg></svg>"), None);
        assert!(sin_cabecera(svg).starts_with("<svg "));
    }

    #[test]
    fn cada_pieza_y_cada_marcador_llevan_su_bloque_y_el_riel_salta_a_ellos() {
        let piezas = [Pieza {
            svg: "<?xml version=\"1.0\"?>\n<svg xmlns=\"x\" viewBox=\"0 0 5 5\"><path/></svg>"
                .into(),
            x: 400.0,
            y: 120.25,
            ancho: 50.0,
            alto: 10.0,
            ancla: 3,
        }];
        let senales = [
            Senal {
                emoji: "⭐".into(),
                y: 300.0,
                ancla: 4,
                fraccion: 0.5,
            },
            Senal {
                emoji: "🔖".into(),
                y: 10.0,
                ancla: -1,
                fraccion: 0.1,
            },
        ];
        let capa = capa_de(&piezas, &senales, 600, 400, "d");
        assert!(capa.contains("<svg class=\"ppa\" data-i=\"3\" data-y=\"120.3\" style=\"left:400px;top:120.3px;width:50px;height:10px\" xmlns=\"x\""));
        assert!(!capa.contains("<?xml"));
        assert!(capa.contains("id=\"ppm-d-0\" data-i=\"4\" data-y=\"300\" style=\"left:1006px"));
        // Sin ancla, la fraccion; con ancla, no hace falta.
        assert!(capa.contains("id=\"ppm-d-1\" data-i=\"-1\" data-y=\"10\" data-f=\"0.1\""));
        assert!(!capa.contains("data-i=\"4\" data-y=\"300\" data-f"));
        let riel = riel_de(&senales, "d");
        assert!(riel.starts_with("<div class=\"pprail\">"));
        assert!(riel.contains("<button data-m=\"ppm-d-0\" title=\"Ir al marcador\">⭐</button>"));
        // El riel no va en la capa: la capa esta dentro de la caja que se amplia.
        assert!(!capa.contains("pprail"));
    }

    #[test]
    fn un_documento_sin_marcadores_no_lleva_riel() {
        assert_eq!(capa_de(&[], &[], 600, 400, "d"), "");
        assert_eq!(riel_de(&[], "d"), "");
    }

    #[test]
    fn la_hoja_de_estilo_se_encierra_en_doc() {
        let css = "body{font:16px/1.5 serif}p,h1{margin:0}html{x:1}bodyx{y:2}@page{size:A4}@media print{body{color:#000}td p{a:b}}";
        let a = acotar(css, ".doc");
        assert_eq!(
            a,
            ".doc{font:16px/1.5 serif}.doc p,.doc h1{margin:0}.doc{x:1}.doc bodyx{y:2}@media print{.doc{color:#000}.doc td p{a:b}}"
        );
        // Caso negativo: nada se escapa sin acotar.
        assert!(!a.contains("@page"));
    }
}
