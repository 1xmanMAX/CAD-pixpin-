//! **Una nota Markdown exportada a Word (`.docx`)**, escrita a mano (H12,
//! 1-oct-2026). El usuario: «que se noten claro las tablas y todo lo que
//! tiene el Markdown, como el editor de Claude; que sea precision y no
//! falle con los archivos».
//!
//! # Lo que sale
//!
//! Un Office Open XML de los de verdad, no un HTML disfrazado: titulos con
//! los estilos `Heading1-3` (salen en el panel de navegacion de Word),
//! listas con `numbering.xml` (vinetas y numeros que Word sigue al editar,
//! anidadas), casillas ☐/☒, tablas con su rejilla (`gridSpan`/`vMerge` para
//! las combinadas, fondos y color de letra, alineacion, cabecera repetida en
//! cada pagina), fotos y paginas vivas incrustadas a su tamano, enlaces web
//! como hipervinculos, codigo en Consolas sobre gris, citas con su barra, la
//! raya, formulas en Cambria Math y **los comentarios del panel** como
//! comentarios de Word anclados a su texto, con las respuestas en su hilo y
//! los resueltos marcados.
//!
//! # Por que se lee con el mismo analizador que el editor
//!
//! Cada renglon se parte con [`md_vivo::analizar`], el mismo que esconde
//! las marcas en el editor: lo que en la pantalla es negrita, en Word es
//! negrita, y una marca que el editor no entiende sale como texto en los
//! dos sitios. Las tablas, con los lectores de la nota
//! ([`md_tabla::leer_gfm`] y [`md_tabla_html::leer_de_nota`]); las fotos,
//! con [`md_imagen::leer`]. No hay un segundo Markdown que se desvie.
//!
//! # Puro
//!
//! No toca el disco: las fotos las da quien llama ([`Nota::imagen`]) y el
//! resultado son los bytes del ZIP. Asi se prueba entero sin ventanas (y se
//! comprueba que el propio lector de `.docx` de PixPin lo lee igual).

mod cuerpo;
pub mod imagen;
mod partes;

use std::collections::BTreeMap;
use std::io::Write;

use crate::md_comentarios::{self, Comentarios};
use crate::md_tabla::{self, Tabla};
use crate::md_vivo::{self, Estilo};
use crate::{ErrorDocs, md_edicion, md_imagen, md_tabla_html};

/// La letra con que se lee la nota: la preferencia de vista del editor
/// (`vista-de-notas.txt`), que no va en el `.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Letra {
    pub cuerpo: String,
    pub titulos: String,
    /// El tamano del cuerpo en pixeles a 96 ppp (16 es el de siempre).
    pub px: u32,
}

impl Default for Letra {
    fn default() -> Self {
        Letra {
            cuerpo: "Work Sans".into(),
            titulos: "Fraunces".into(),
            px: 16,
        }
    }
}

/// Lo que se exporta.
pub struct Nota<'a> {
    pub markdown: &'a str,
    /// El nombre del documento (propiedades de Word); vacio, el titulo de
    /// la nota.
    pub titulo: &'a str,
    pub letra: Letra,
    pub comentarios: Option<&'a Comentarios>,
    /// Los bytes de la foto de una ruta del Markdown (`pixpin:files/…`,
    /// relativa o de este equipo), o nada si no esta. Lo que no sea PNG,
    /// JPEG, GIF o BMP deberia venir ya pasado a PNG.
    pub imagen: &'a dyn Fn(&str) -> Option<Vec<u8>>,
    /// Quien firma el documento (las propiedades); vacio, «PixPin».
    pub autor: &'a str,
    /// Milisegundos UTC: la fecha de creacion de las propiedades.
    pub ahora_ms: i64,
}

/// **Exporta la nota**: los bytes del `.docx`.
pub fn exportar(n: &Nota) -> Result<Vec<u8>, ErrorDocs> {
    let modelo = modelo(n.markdown);
    let titulo = match n.titulo.trim() {
        "" => md_vivo::titulo(n.markdown),
        t => t.to_string(),
    };
    let comentarios = colocar_comentarios(n.markdown, n.comentarios, &modelo);
    let hecho = cuerpo::escribir(&modelo, &comentarios, &n.letra, n.imagen);
    let mut entradas: Vec<(String, Vec<u8>)> = Vec::new();
    let hay_comentarios = !comentarios.hilos.is_empty();
    entradas.push((
        "[Content_Types].xml".into(),
        partes::tipos(&hecho.medios, hay_comentarios).into_bytes(),
    ));
    entradas.push(("_rels/.rels".into(), partes::RELS_RAIZ.as_bytes().to_vec()));
    entradas.push((
        "docProps/core.xml".into(),
        partes::core(&titulo, n.autor, n.ahora_ms).into_bytes(),
    ));
    entradas.push(("docProps/app.xml".into(), partes::APP.as_bytes().to_vec()));
    entradas.push(("word/document.xml".into(), hecho.documento.into_bytes()));
    entradas.push((
        "word/styles.xml".into(),
        partes::estilos(&n.letra).into_bytes(),
    ));
    entradas.push((
        "word/numbering.xml".into(),
        partes::numeracion(&hecho.numeradas).into_bytes(),
    ));
    entradas.push((
        "word/settings.xml".into(),
        partes::AJUSTES.as_bytes().to_vec(),
    ));
    entradas.push((
        "word/fontTable.xml".into(),
        partes::letras(&n.letra).into_bytes(),
    ));
    if hay_comentarios {
        let (c, ex) = partes::comentarios(&comentarios.hilos);
        entradas.push(("word/comments.xml".into(), c.into_bytes()));
        entradas.push(("word/commentsExtended.xml".into(), ex.into_bytes()));
    }
    entradas.push((
        "word/_rels/document.xml.rels".into(),
        partes::rels_documento(&hecho.medios, &hecho.enlaces, hay_comentarios).into_bytes(),
    ));
    for m in &hecho.medios {
        entradas.push((format!("word/media/{}", m.nombre), m.datos.clone()));
    }
    zip(&entradas)
}

/// El nombre de fichero para guardar la nota: sin lo que Windows no deja
/// (`<>:"/\|?*`, letras de control), sin puntos ni blancos al final, sin
/// los nombres reservados (`CON`, `NUL`, `COM1`…) y no mas de 120 letras.
/// Con `.docx` al final.
pub fn nombre_de_fichero(titulo: &str) -> String {
    let limpio: String = titulo
        .chars()
        .map(|c| {
            if "<>:\"/\\|?*".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .take(120)
        .collect();
    let limpio = limpio
        .trim()
        .trim_end_matches(['.', ' '])
        .trim()
        .to_string();
    let base = limpio
        .split('.')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_uppercase();
    let reservado = matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((base.starts_with("COM") || base.starts_with("LPT"))
            && base.len() == 4
            && base.as_bytes()[3].is_ascii_digit());
    let limpio = if limpio.chars().all(|c| c == '_' || c.is_whitespace()) {
        "Nota".to_string()
    } else if reservado {
        format!("{limpio}_")
    } else {
        limpio
    };
    format!("{limpio}.docx")
}

fn zip(entradas: &[(String, Vec<u8>)]) -> Result<Vec<u8>, ErrorDocs> {
    let mut salida = Vec::new();
    {
        let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut salida));
        for (nombre, datos) in entradas {
            // Lo de dentro de `word/media` ya va comprimido (PNG, JPEG):
            // comprimirlo otra vez solo gasta tiempo.
            let metodo = if nombre.starts_with("word/media/") {
                zip::CompressionMethod::Stored
            } else {
                zip::CompressionMethod::Deflated
            };
            let opciones: zip::write::FileOptions<'_, ()> =
                zip::write::FileOptions::default().compression_method(metodo);
            z.start_file(nombre.as_str(), opciones).map_err(a_disco)?;
            z.write_all(datos)?;
        }
        z.finish().map_err(a_disco)?;
    }
    Ok(salida)
}

fn a_disco(e: zip::result::ZipError) -> ErrorDocs {
    ErrorDocs::Disco(std::io::Error::other(e.to_string()))
}

// ---------------------------------------------------------------------------
// El modelo: la nota partida en bloques con sus letras visibles

/// Lo que lleva una letra encima.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Marcas {
    pub negrita: bool,
    pub cursiva: bool,
    pub tachado: bool,
    pub codigo: bool,
    pub formula: bool,
    /// Lo escrito tras una casilla marcada: tachado y apagado.
    pub hecha: bool,
    /// El enlace, por su numero en [`Modelo::enlaces`].
    pub enlace: Option<usize>,
}

/// Una letra que se ve, con su sitio en el Markdown (UTF-16), para anclar
/// los comentarios. `orden` es su numero en el documento entero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Letra1 {
    pub c: char,
    pub orden: usize,
    pub sitio: Option<(usize, usize)>,
    pub marcas: Marcas,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tipo {
    Normal,
    Titulo(u8),
    Cita,
    Codigo,
    Vineta {
        nivel: u8,
    },
    /// `lista`: el numero de la racha (cada una con su numeracion de Word,
    /// que empieza donde empezaba la del Markdown).
    Numerada {
        nivel: u8,
        lista: usize,
    },
    Casilla {
        nivel: u8,
        hecha: bool,
    },
    /// Un documento, un audio o un mensaje del chat metido en la nota.
    Adjunto,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Parrafo {
    pub tipo: Tipo,
    pub letras: Vec<Letra1>,
}

/// Una celda: sus parrafos (uno por renglon de la celda).
pub(crate) type Celda1 = Vec<Vec<Letra1>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Bloque1 {
    Parrafo(Parrafo),
    /// Un renglon en blanco puesto a proposito (el segundo de una racha).
    Vacio,
    Regla,
    Foto {
        foto: md_imagen::Foto,
        /// Para anclar un comentario puesto sobre la foto.
        orden: usize,
        sitio: (usize, usize),
    },
    Tabla {
        tabla: Tabla,
        celdas: Vec<Vec<Celda1>>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Modelo {
    pub bloques: Vec<Bloque1>,
    pub enlaces: Vec<String>,
    /// El numero con que empieza cada racha numerada.
    pub rachas: Vec<u32>,
    /// Cuantos `orden` se han dado.
    pub letras: usize,
}

struct Constructor {
    m: Modelo,
}

impl Constructor {
    fn siguiente(&mut self) -> usize {
        self.m.letras += 1;
        self.m.letras - 1
    }

    fn enlace(&mut self, url: &str) -> usize {
        match self.m.enlaces.iter().position(|u| u == url) {
            Some(i) => i,
            None => {
                self.m.enlaces.push(url.to_string());
                self.m.enlaces.len() - 1
            }
        }
    }

    /// Las letras que se ven de un trozo de Markdown en linea, con sus
    /// marcas. `base` es donde empieza en la nota (UTF-16), si se sabe.
    fn letras(
        &mut self,
        texto: &str,
        tramos: &[md_vivo::Tramo],
        base: Option<usize>,
    ) -> Vec<Letra1> {
        let mut sal = Vec::new();
        let mut pos = 0usize;
        for c in texto.chars() {
            let largo = c.len_utf16();
            let dentro = |t: &&md_vivo::Tramo| t.desde <= pos && pos < t.hasta;
            let escondida = tramos.iter().filter(dentro).any(|t| {
                matches!(
                    t.estilo,
                    Estilo::Marca | Estilo::Numero | Estilo::Casilla { .. }
                )
            });
            if !escondida && !matches!(c, '\u{FFF9}' | '\u{FFFB}' | '\u{0007}' | '\u{FFFF}') {
                let mut m = Marcas::default();
                for t in tramos.iter().filter(dentro) {
                    match t.estilo {
                        Estilo::Negrita => m.negrita = true,
                        Estilo::Cursiva => m.cursiva = true,
                        Estilo::Tachado => m.tachado = true,
                        Estilo::Codigo => m.codigo = true,
                        Estilo::Formula => m.formula = true,
                        Estilo::Hecha => m.hecha = true,
                        Estilo::Enlace => {
                            if let Some(u) = t.url.as_deref() {
                                m.enlace = Some(self.enlace(u));
                            }
                        }
                        _ => {}
                    }
                }
                let orden = self.siguiente();
                sal.push(Letra1 {
                    c,
                    orden,
                    sitio: base.map(|b| (b + pos, b + pos + largo)),
                    marcas: m,
                });
            }
            pos += largo;
        }
        sal
    }

    /// Una celda de tabla: solo formato de letra (un `# ` dentro no la hace
    /// titulo), como en el editor. `fuente` es el renglon del Markdown
    /// donde se busca su texto para saber su sitio.
    fn celda(&mut self, texto: &str, fuente: &mut Option<(&str, usize, usize)>) -> Celda1 {
        let mut parrafos = Vec::new();
        for renglon in texto.split('\n') {
            let renglon = renglon.trim_end_matches('\r');
            // El analizador lee una fila de tabla con solo formato de letra
            // en cada celda: se le da el renglon como celda unica.
            let fila = format!("{renglon}{}", md_tabla::CELDA);
            let tramos = md_vivo::analizar(&fila);
            let base = buscar_en_fuente(fuente, renglon);
            parrafos.push(self.letras(renglon, &tramos, base));
        }
        parrafos
    }
}

/// Donde esta `aguja` en el renglon fuente, a partir de lo ya usado:
/// UTF-16 de la nota. Asi un comentario puesto dentro de una tabla cae en
/// su celda. Nada si no esta tal cual (una barra escapada, una entidad).
fn buscar_en_fuente(fuente: &mut Option<(&str, usize, usize)>, aguja: &str) -> Option<usize> {
    let (renglon, base, desde) = fuente.as_mut()?;
    if aguja.is_empty() {
        return None;
    }
    let i = renglon.get(*desde..)?.find(aguja)? + *desde;
    *desde = i + aguja.len();
    Some(*base + renglon[..i].encode_utf16().count())
}

/// Lo que mide una sangria de lista, en niveles (como el editor: dos
/// espacios o un tabulador por nivel; hasta 6).
fn nivel(renglon: &str) -> u8 {
    let blancos: u32 = renglon
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .map(|c| if c == '\t' { 2 } else { 1 })
        .sum();
    (blancos / 2).min(6) as u8
}

fn es_valla(r: &str) -> bool {
    let t = r.trim_start();
    t.starts_with("```") || t.starts_with("~~~")
}

/// **La nota partida en bloques.**
// `x` indexa a la vez `inicio` y la funcion `limpio`: el bucle por rango es lo claro.
#[allow(clippy::needless_range_loop)]
pub(crate) fn modelo(md: &str) -> Modelo {
    let renglones: Vec<&str> = md.split('\n').collect();
    // Donde empieza cada renglon (UTF-16), con su salto.
    let mut inicio = Vec::with_capacity(renglones.len() + 1);
    let mut p = 0usize;
    for r in &renglones {
        inicio.push(p);
        p += r.encode_utf16().count() + 1;
    }
    let limpio = |i: usize| renglones[i].trim_end_matches('\r');
    let mut k = Constructor {
        m: Modelo::default(),
    };
    let mut en_codigo = false;
    let mut blancos = 0usize;
    // La racha numerada en curso: (su numero, ultimo renglon de lista).
    let mut racha: Option<usize> = None;
    let mut i = 0;
    while i < renglones.len() {
        let r = limpio(i);
        if en_codigo {
            if es_valla(r) {
                en_codigo = false;
            } else {
                let letras = k.letras(r, &[], Some(inicio[i]));
                k.m.bloques.push(Bloque1::Parrafo(Parrafo {
                    tipo: Tipo::Codigo,
                    letras,
                }));
            }
            i += 1;
            continue;
        }
        if r.trim().is_empty() {
            blancos += 1;
            racha = None;
            i += 1;
            continue;
        }
        // Una racha de renglones en blanco deja los de mas como parrafos
        // vacios: el primero solo separa (lo hace el aire del parrafo).
        if blancos > 1 && !k.m.bloques.is_empty() {
            for _ in 1..blancos {
                k.m.bloques.push(Bloque1::Vacio);
            }
        }
        blancos = 0;
        if es_valla(r) {
            en_codigo = true;
            racha = None;
            i += 1;
            continue;
        }
        // Tabla de barras: una fila con la de guiones debajo.
        if r.trim().starts_with('|')
            && renglones
                .get(i + 1)
                .is_some_and(|s| md_tabla::es_separadora(s.trim_end_matches('\r')))
        {
            let mut j = i;
            while j < renglones.len() && limpio(j).trim().starts_with('|') {
                j += 1;
            }
            let texto: Vec<&str> = (i..j).map(limpio).collect();
            if let Some(t) = md_tabla::leer_gfm(&texto.join("\n")) {
                // Las filas del Markdown, sin la de guiones: de ahi sale el
                // sitio de cada celda.
                let fuentes: Vec<usize> = (i..j).filter(|&x| x != i + 1).collect();
                let celdas = t
                    .filas
                    .iter()
                    .enumerate()
                    .map(|(f, fila)| {
                        let mut fuente = fuentes.get(f).map(|&x| (limpio(x), inicio[x], 0usize));
                        fila.iter().map(|c| k.celda(c, &mut fuente)).collect()
                    })
                    .collect();
                k.m.bloques.push(Bloque1::Tabla { tabla: t, celdas });
                racha = None;
                i = j;
                continue;
            }
        }
        // Tabla en HTML, como la escribe el movil.
        if r.trim_start().to_ascii_lowercase().starts_with("<table") {
            let mut j = i;
            while j < renglones.len() {
                j += 1;
                if limpio(j - 1).to_ascii_lowercase().contains("</table>") {
                    break;
                }
            }
            let texto: Vec<&str> = (i..j).map(limpio).collect();
            if let Some(t) = md_tabla_html::leer_de_nota(&texto.join("\n")) {
                let n = t.columnas();
                let celdas = t
                    .filas
                    .iter()
                    .map(|fila| {
                        (0..n)
                            .map(|c| {
                                let texto = fila.get(c).map(String::as_str).unwrap_or("");
                                // La celda se busca en todo el HTML de la
                                // tabla, por orden.
                                let mut fuente = None;
                                for x in i..j {
                                    if limpio(x).contains(texto) && !texto.is_empty() {
                                        fuente = Some((limpio(x), inicio[x], 0usize));
                                        break;
                                    }
                                }
                                k.celda(texto, &mut fuente)
                            })
                            .collect()
                    })
                    .collect();
                k.m.bloques.push(Bloque1::Tabla { tabla: t, celdas });
                racha = None;
                i = j;
                continue;
            }
        }
        // Una foto sola en su renglon.
        if let Some(foto) = md_imagen::leer(r) {
            let orden = k.siguiente();
            let sitio = (inicio[i], inicio[i] + r.encode_utf16().count());
            k.m.bloques.push(Bloque1::Foto { foto, orden, sitio });
            racha = None;
            i += 1;
            continue;
        }
        let tramos = md_vivo::analizar(r);
        let tiene = |f: &dyn Fn(Estilo) -> bool| tramos.iter().any(|t| f(t.estilo));
        if tiene(&|e| e == Estilo::Regla) {
            k.m.bloques.push(Bloque1::Regla);
            racha = None;
            i += 1;
            continue;
        }
        let nv = nivel(r);
        let tipo = if let Some(n) = tramos.iter().find_map(|t| match t.estilo {
            Estilo::Titulo(n) => Some(n),
            _ => None,
        }) {
            Tipo::Titulo(n.clamp(1, 6))
        } else if tiene(&|e| e == Estilo::Cita) {
            Tipo::Cita
        } else if let Some(hecha) = tramos.iter().find_map(|t| match t.estilo {
            Estilo::Casilla { hecha } => Some(hecha),
            _ => None,
        }) {
            Tipo::Casilla { nivel: nv, hecha }
        } else if tiene(&|e| e == Estilo::Vineta) {
            Tipo::Vineta { nivel: nv }
        } else if let Some(t) = tramos.iter().find(|t| t.estilo == Estilo::Numero) {
            let lista = match racha {
                Some(l) => l,
                None => {
                    let cifras: String = r
                        .encode_utf16()
                        .skip(t.desde)
                        .take(t.hasta - t.desde)
                        .map(|u| u as u8 as char)
                        .collect();
                    let numero: u32 = cifras.trim_end_matches(['.', ')']).parse().unwrap_or(1);
                    k.m.rachas.push(numero.clamp(0, 32_767));
                    k.m.rachas.len() - 1
                }
            };
            Tipo::Numerada { nivel: nv, lista }
        } else if md_edicion::es_incrustado(r) {
            Tipo::Adjunto
        } else {
            Tipo::Normal
        };
        // La racha numerada sigue mientras haya renglones de lista (una
        // vineta anidada no la corta).
        racha = match tipo {
            Tipo::Numerada { lista, .. } => Some(lista),
            Tipo::Vineta { .. } | Tipo::Casilla { .. } => racha,
            _ => None,
        };
        let letras = k.letras(r, &tramos, Some(inicio[i]));
        k.m.bloques.push(Bloque1::Parrafo(Parrafo { tipo, letras }));
        i += 1;
    }
    k.m
}

// ---------------------------------------------------------------------------
// Los comentarios, en su sitio

/// Un comentario de Word: un hilo del panel o una de sus respuestas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComentarioW {
    pub id: usize,
    pub autor: String,
    pub cuando: i64,
    pub texto: String,
    /// La respuesta a quien (su `id`).
    pub padre: Option<usize>,
    pub resuelto: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Colocados {
    pub hilos: Vec<ComentarioW>,
    /// Los comentarios que empiezan antes de la letra `orden`.
    pub empiezan: BTreeMap<usize, Vec<usize>>,
    /// Los que acaban detras de la letra `orden`.
    pub acaban: BTreeMap<usize, Vec<usize>>,
}

/// Cada hilo en su texto: se busca su cita en el Markdown
/// ([`md_comentarios::ubicar`], lo mismo que hace el panel) y se cuelga de
/// la primera y la ultima letra que se ven dentro. Uno que ya no se
/// encuentra no se pierde: va en la primera letra, con su cita delante.
fn colocar_comentarios(md: &str, c: Option<&Comentarios>, m: &Modelo) -> Colocados {
    let mut sal = Colocados::default();
    let Some(c) = c else {
        return sal;
    };
    let texto: Vec<u16> = md.encode_utf16().collect();
    // Las letras con sitio, por orden.
    let mut sitios: Vec<(usize, (usize, usize))> = Vec::new();
    let con_sitio = |l: &Letra1| l.sitio.map(|s| (l.orden, s));
    for b in &m.bloques {
        match b {
            Bloque1::Parrafo(p) => sitios.extend(p.letras.iter().filter_map(con_sitio)),
            Bloque1::Foto { orden, sitio, .. } => sitios.push((*orden, *sitio)),
            Bloque1::Tabla { celdas, .. } => sitios.extend(
                celdas
                    .iter()
                    .flatten()
                    .flatten()
                    .flatten()
                    .filter_map(con_sitio),
            ),
            Bloque1::Vacio | Bloque1::Regla => {}
        }
    }
    sitios.sort_by_key(|x| x.0);
    let primera = sitios.first().map(|x| x.0).unwrap_or(0);
    for h in &c.comentarios {
        let rango = md_comentarios::ubicar(&texto, &h.ancla).and_then(|(a, b)| {
            let dentro: Vec<usize> = sitios
                .iter()
                .filter(|(_, (x, y))| *x < b && *y > a)
                .map(|(o, _)| *o)
                .collect();
            Some((*dentro.first()?, *dentro.last()?))
        });
        let (desde, hasta, perdido) = match rango {
            Some((x, y)) => (x, y, false),
            None => (primera, primera, true),
        };
        let id = sal.hilos.len();
        let texto = if perdido && !h.ancla.cita.trim().is_empty() {
            format!("«{}»\n{}", h.ancla.cita.trim(), h.texto)
        } else {
            h.texto.clone()
        };
        sal.hilos.push(ComentarioW {
            id,
            autor: h.autor.clone(),
            cuando: h.cuando,
            texto,
            padre: None,
            resuelto: h.resuelto,
        });
        let mut ids = vec![id];
        for r in &h.respuestas {
            let rid = sal.hilos.len();
            sal.hilos.push(ComentarioW {
                id: rid,
                autor: r.autor.clone(),
                cuando: r.cuando,
                texto: r.texto.clone(),
                padre: Some(id),
                resuelto: h.resuelto,
            });
            ids.push(rid);
        }
        sal.empiezan
            .entry(desde)
            .or_default()
            .extend(ids.iter().copied());
        sal.acaban.entry(hasta).or_default().extend(ids);
    }
    sal
}

#[cfg(test)]
mod pruebas;
