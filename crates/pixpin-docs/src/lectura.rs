//! **Leer a gusto**: el tamano de la letra y los marcadores con emoticono.
//! Puerto de `motor/Lectura.kt` del movil (v0.65-v0.66).
//!
//! Un marcador guarda **en que fraccion del documento** esta lo que habia
//! arriba de la pantalla al ponerlo, no un numero de pixeles: asi sigue
//! cayendo (casi) en el mismo parrafo aunque luego se agrande la letra y
//! todo el documento se alargue.
//!
//! Lo que el movil guarda en sus ajustes, aqui se guarda **junto al
//! documento**, en un fichero hermano `<nombre>.pixpin-lectura`. Es lo que
//! pidio el usuario, y ademas tiene una ventaja: los marcadores viajan con
//! el documento si se copia la carpeta, y no se quedan en un ajuste que
//! nadie sabe donde esta. Una linea por cosa, sin JSON que arrastrar, igual
//! que el movil.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct Marcador {
    /// Cuando se puso, en milisegundos. Sirve de identidad.
    pub id: u64,
    /// De 0 a 1.
    pub fraccion: f32,
    pub emoji: String,
}

pub const TAMANO_MIN: u32 = 70;
pub const TAMANO_MAX: u32 = 250;

/// Los tamanos de la barra de puntos, en tanto por ciento: cada punto, uno.
pub const TAMANOS: [u32; 9] = [80, 90, 100, 115, 130, 150, 175, 200, 230];

/// Los emoticonos con los que se marca, en el orden del movil.
pub const EMOJIS: [&str; 12] = [
    "🔖", "⭐", "❤️", "❗", "❓", "💡", "📌", "✅", "🔥", "👀", "✏️", "🏁",
];

/// Mas de esto no se guardan: la tira lateral deja de distinguirse.
pub const MARCADORES: usize = 24;

pub fn tamano_valido(t: u32) -> u32 {
    t.clamp(TAMANO_MIN, TAMANO_MAX)
}

/// El siguiente tamano de la barra de puntos, hacia arriba o hacia abajo.
pub fn tamano_vecino(actual: u32, hacia_arriba: bool) -> u32 {
    let i = punto_del_tamano(actual);
    let siguiente = if hacia_arriba {
        (i + 1).min(TAMANOS.len() - 1)
    } else {
        i.saturating_sub(1)
    };
    TAMANOS[siguiente]
}

/// Que punto de la barra corresponde al tamano de ahora.
pub fn punto_del_tamano(actual: u32) -> usize {
    TAMANOS
        .iter()
        .position(|t| *t >= actual)
        .unwrap_or(TAMANOS.len() - 1)
}

/// **Los tipos de letra, los cuatro del movil** (`Lectura.LETRAS`, en su
/// orden): serif, sans, ancho fijo y cursiva. Son los mismos indices que
/// viajan en la maqueta (`anot-<uid>.maqueta`), asi que la letra con la que
/// se anoto en un aparato es la misma en el otro (K16). Antes el PC tenia
/// dos (Segoe UI y Consolas) y los traducia a ojo: con otra letra el texto
/// se parte en otros renglones y lo anotado cae en otra palabra.
pub const TIPOS: usize = 4;

/// Los grosores, los cuatro del movil (`Lectura.GROSORES`): 300, 400, 600 y
/// 800 de CSS.
pub const GROSORES: usize = 4;

/// Los pesos de CSS de cada grosor.
pub const PESOS: [u16; GROSORES] = [300, 400, 600, 800];

/// La letra y el grosor con los que abre un documento que nunca se toco:
/// los del movil (`prefsDeLectura.getInt("tipo", 0)` y `("grosor", 1)`).
pub const TIPO_DE_FABRICA: u8 = 0;
pub const GROSOR_DE_FABRICA: u8 = 1;

/// La version del fichero. La 1 es la del PC de antes de K16: dos letras
/// (0 Segoe UI, 1 Consolas) y dos grosores (0 normal, 1 gruesa), y la tinta
/// en la maqueta propia del PC. Se lee y se traduce ([`de_texto`]).
const VERSION: u32 = 2;

/// Todo lo que se recuerda de un documento entre una lectura y la
/// siguiente.
#[derive(Debug, Clone, PartialEq)]
pub struct Ajustes {
    pub tamano: u32,
    /// Por donde se iba, de 0 a 1 (Word, libro, pagina).
    pub sitio: f32,
    pub marcadores: Vec<Marcador>,
    /// Que letra, de `0..TIPOS` (0 serif, 1 sans, 2 ancho fijo, 3 cursiva).
    pub tipo: u8,
    /// Que grosor, de `0..GROSORES` (0 fina, 1 normal, 2 gruesa, 3 negra).
    pub grosor: u8,
    /// **La columna de texto con la que se anoto por primera vez**, en
    /// pixeles logicos; 0 si nunca se anoto. Es `columnaDeAnotar` del
    /// movil: desde que hay tinta, el texto no puede volver a partirse en
    /// otras lineas o lo anotado quedaria encima de otra palabra. Con la
    /// columna se fijan tambien el tamano, el tipo y el grosor de la letra.
    pub columna: u32,
    /// El aumento propio del documento (1 = la columna a su tamano).
    pub zoom: f32,
    /// **Los espacios para anotar del PDF**, en bits como el movil
    /// (`ESPACIO_IZQUIERDA = 1`, `ESPACIO_DERECHA = 2`).
    pub espacios: u8,
    /// Por donde se iba en un PDF: **la pagina mas la fraccion** de su alto
    /// (2,5 = la mitad de la tercera), la misma cuenta que las marcas.
    pub pagina: f64,
    /// Las marcas del PDF, en la linea del movil (`id:x:y:emoji|…`). Se
    /// guardan tal cual: quien las entiende es `pixpin_motor2d::marcas`, y
    /// este crate no depende del motor.
    pub marcas: String,
    /// **Ya se recogieron los marcadores del indice del PDF** (`/Outlines`,
    /// Android v0.98.2): el lector los recoge la primera vez que lo abre, y
    /// despues no, o los quitados a mano volverian.
    pub indice: bool,
    /// **El espacio en blanco de cada lado de la columna de un Word o un
    /// libro**, en pixeles de lectura (`espacioIzq`/`espacioDer` del movil,
    /// los `izq`/`der` de la maqueta). `None`: los dos tercios de siempre.
    /// Solo cuentan con la columna fijada.
    pub lados: Option<(u32, u32)>,
    /// **El marcador verde de la voz**: el parrafo y la fraccion donde se
    /// dejo de escuchar (`pixpin_docs::voz_alta`). De un adjunto del chat va
    /// en su `anot-<uid>.voz`; aqui, para lo que no lo es.
    pub voz: Option<(usize, f32)>,
    /// **El desplazamiento de lado, bloqueado** (el candado de los mandos
    /// de abajo, `sinLado` del movil): la vista solo sube y baja.
    pub sin_lado: bool,
    /// **Se leyo de un fichero de antes de K16** (version 1): la tinta que
    /// hubiera junto al documento se hizo sobre la maqueta vieja del PC y el
    /// lector la pasa una vez a la del movil. No se escribe: al guardar, el
    /// fichero ya es de la version nueva.
    pub de_antes: bool,
}

impl Default for Ajustes {
    fn default() -> Self {
        Self {
            tamano: 100,
            sitio: 0.0,
            marcadores: Vec::new(),
            tipo: TIPO_DE_FABRICA,
            grosor: GROSOR_DE_FABRICA,
            columna: 0,
            zoom: 1.0,
            espacios: 0,
            pagina: 0.0,
            marcas: String::new(),
            indice: false,
            lados: None,
            voz: None,
            sin_lado: false,
            de_antes: false,
        }
    }
}

impl Ajustes {
    /// Si el documento ya tiene tinta encima: entonces la letra no se toca.
    pub fn letra_fijada(&self) -> bool {
        self.columna > 0
    }
}

/// Uno mas, **en el orden del documento**, que es el de los puntos del
/// lateral.
pub fn con_marcador(lista: &[Marcador], fraccion: f32, emoji: &str, ahora: u64) -> Vec<Marcador> {
    let mut salida = lista.to_vec();
    salida.push(Marcador {
        id: ahora,
        fraccion: fraccion.clamp(0.0, 1.0),
        emoji: if emoji.trim().is_empty() {
            EMOJIS[0].to_string()
        } else {
            emoji.to_string()
        },
    });
    ordenar(&mut salida);
    // Los mas viejos del documento se van si sobran, como en el movil
    // (`takeLast`).
    if salida.len() > MARCADORES {
        salida.drain(..salida.len() - MARCADORES);
    }
    salida
}

fn ordenar(lista: &mut [Marcador]) {
    lista.sort_by(|a, b| {
        a.fraccion
            .partial_cmp(&b.fraccion)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

/// El fichero hermano donde vive todo esto.
pub fn ruta_de_ajustes(documento: &Path) -> PathBuf {
    let mut nombre = documento
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "documento".into());
    nombre.push_str(".pixpin-lectura");
    documento.with_file_name(nombre)
}

/// Lee los ajustes de un documento. Si no hay fichero, o esta roto, salen
/// los de fabrica: no leer es tan normal como leer.
pub fn leer(documento: &Path) -> Ajustes {
    match std::fs::read_to_string(ruta_de_ajustes(documento)) {
        Ok(t) => de_texto(&t),
        Err(_) => Ajustes::default(),
    }
}

/// Guarda los ajustes junto al documento. Si no se puede escribir (un
/// documento en un sitio de solo lectura), se dice y no pasa nada mas: la
/// lectura no depende de esto.
pub fn escribir(documento: &Path, a: &Ajustes) -> std::io::Result<()> {
    std::fs::write(ruta_de_ajustes(documento), a_texto(a))
}

pub fn a_texto(a: &Ajustes) -> String {
    let marcadores = a
        .marcadores
        .iter()
        .map(|m| format!("{}:{}:{}", m.id, m.fraccion, m.emoji))
        .collect::<Vec<_>>()
        .join("|");
    // Las lineas nuevas van detras: un PixPin anterior las salta (lee por
    // clave e ignora lo que no conoce) y el fichero le sigue sirviendo.
    let mut t = format!(
        "pixpin-lectura {VERSION}\ntamano {}\nsitio {}\nmarcadores {marcadores}\ntipo {}\ngrosor {}\ncolumna {}\nzoom {}\nespacios {}\npagina {}\nmarcas {}\n",
        a.tamano,
        a.sitio,
        a.tipo,
        a.grosor,
        a.columna,
        a.zoom,
        a.espacios,
        a.pagina,
        // Una linea por clave: un salto de linea colado en las marcas
        // partiria el fichero y lo de detras se leeria como otra clave.
        a.marcas.replace(['\n', '\r'], "")
    );
    // Solo si es verdad: los ficheros de siempre no cambian.
    if a.indice {
        t.push_str("indice 1\n");
    }
    if let Some((izq, der)) = a.lados {
        t.push_str(&format!("lados {izq} {der}\n"));
    }
    if let Some((p, f)) = a.voz {
        t.push_str(&format!("voz {}\n", crate::voz_alta::voz_a_texto(p, f)));
    }
    if a.sin_lado {
        t.push_str("sinlado 1\n");
    }
    t
}

pub fn de_texto(texto: &str) -> Ajustes {
    let mut a = Ajustes::default();
    // Con la 1, el fichero es del PC de antes (sus letras y su maqueta): el
    // PC siempre escribio la cabecera, asi que sin ella es un fichero roto y
    // se lee como de hoy. Uno de una version futura, con las claves de hoy.
    let version = texto
        .lines()
        .next()
        .and_then(|l| l.strip_prefix("pixpin-lectura "))
        .and_then(|v| v.trim().parse::<u32>().ok())
        .unwrap_or(VERSION);
    let vieja = version < VERSION;
    a.de_antes = vieja;
    if vieja {
        // Lo que no diga el fichero viejo era Segoe UI normal: la sans del
        // movil a su peso normal, no la serif de fabrica de ahora.
        a.tipo = 1;
        a.grosor = 1;
    }
    for linea in texto.lines() {
        let (clave, valor) = linea.split_once(' ').unwrap_or((linea, ""));
        match clave {
            // Las dos letras y los dos grosores del PC de antes, a los del
            // movil: Segoe UI a la sans, Consolas a la de ancho fijo; la
            // normal a la normal y la gruesa a la gruesa.
            "tipo" if vieja => a.tipo = if valor.trim() == "1" { 2 } else { 1 },
            "grosor" if vieja => a.grosor = if valor.trim() == "1" { 2 } else { 1 },
            "tamano" => a.tamano = tamano_valido(valor.trim().parse().unwrap_or(100)),
            "sitio" => a.sitio = valor.trim().parse::<f32>().unwrap_or(0.0).clamp(0.0, 1.0),
            "marcadores" => a.marcadores = marcadores_de_texto(valor),
            "tipo" => {
                a.tipo = valor
                    .trim()
                    .parse::<u8>()
                    .unwrap_or(TIPO_DE_FABRICA)
                    .min(TIPOS as u8 - 1)
            }
            "grosor" => {
                a.grosor = valor
                    .trim()
                    .parse::<u8>()
                    .unwrap_or(GROSOR_DE_FABRICA)
                    .min(GROSORES as u8 - 1)
            }
            // Una columna absurda solo puede venir de un fichero roto: se
            // acota para que el texto no quede en una linea kilometrica.
            "columna" => a.columna = valor.trim().parse::<u32>().unwrap_or(0).min(20_000),
            "zoom" => {
                a.zoom = valor
                    .trim()
                    .parse::<f32>()
                    .ok()
                    .filter(|z| z.is_finite() && *z > 0.0)
                    .unwrap_or(1.0)
                    .clamp(crate::vista::ZOOM_MINIMO, crate::vista::ZOOM_MAXIMO)
            }
            "espacios" => a.espacios = valor.trim().parse::<u8>().unwrap_or(0) & 3,
            "pagina" => {
                a.pagina = valor
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|p| p.is_finite() && *p >= 0.0)
                    .unwrap_or(0.0)
            }
            "marcas" => a.marcas = valor.trim().to_string(),
            "indice" => a.indice = valor.trim() == "1",
            "lados" => {
                let mut n = valor.split_whitespace().map(|x| x.parse::<u32>().ok());
                if let (Some(Some(i)), Some(Some(d))) = (n.next(), n.next()) {
                    a.lados = Some((i.min(20_000), d.min(20_000)));
                }
            }
            "voz" => a.voz = crate::voz_alta::voz_de_texto(valor),
            "sinlado" => a.sin_lado = valor.trim() == "1",
            _ => {}
        }
    }
    a
}

/// Guardados en una linea: `id:fraccion:emoji` separados por `|`, igual que
/// en el movil.
pub fn marcadores_de_texto(texto: &str) -> Vec<Marcador> {
    let mut salida: Vec<Marcador> = texto
        .split('|')
        .filter_map(|trozo| {
            // Solo dos cortes: un emoticono puede traer dos puntos dentro
            // (los hay con variantes).
            let (id, resto) = trozo.split_once(':')?;
            let (fraccion, emoji) = resto.split_once(':').unwrap_or((resto, ""));
            Some(Marcador {
                id: id.trim().parse().ok()?,
                fraccion: fraccion.trim().parse::<f32>().ok()?.clamp(0.0, 1.0),
                emoji: if emoji.trim().is_empty() {
                    EMOJIS[0].to_string()
                } else {
                    emoji.to_string()
                },
            })
        })
        .collect();
    ordenar(&mut salida);
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn m(id: u64, f: f32) -> Marcador {
        Marcador {
            id,
            fraccion: f,
            emoji: "🔖".into(),
        }
    }

    #[test]
    fn un_marcador_nuevo_entra_por_donde_le_toca_en_el_documento() {
        let lista = vec![m(1, 0.1), m(2, 0.8)];
        let con = con_marcador(&lista, 0.5, "⭐", 3);
        let f: Vec<f32> = con.iter().map(|x| x.fraccion).collect();
        assert_eq!(f, vec![0.1, 0.5, 0.8]);
        assert_eq!(con[1].emoji, "⭐");
    }

    #[test]
    fn una_fraccion_imposible_se_mete_en_el_documento() {
        let con = con_marcador(&[], 5.0, "⭐", 1);
        assert_eq!(con[0].fraccion, 1.0);
        let con = con_marcador(&[], -3.0, "⭐", 1);
        assert_eq!(con[0].fraccion, 0.0);
    }

    #[test]
    fn nunca_se_guardan_mas_de_veinticuatro() {
        let mut lista: Vec<Marcador> = (0..MARCADORES as u64)
            .map(|i| m(i, i as f32 / 100.0))
            .collect();
        lista = con_marcador(&lista, 0.99, "🏁", 999);
        assert_eq!(lista.len(), MARCADORES);
        assert_eq!(
            lista.last().map(|x| x.id),
            Some(999),
            "el nuevo tiene que estar; el que sobra es el primero"
        );
    }

    #[test]
    fn se_guardan_y_se_leen_igual() {
        let a = Ajustes {
            tamano: 130,
            sitio: 0.25,
            marcadores: vec![m(1, 0.1), m(2, 0.9)],
            tipo: 1,
            grosor: 1,
            columna: 720,
            zoom: 0.6,
            espacios: 3,
            pagina: 4.5,
            marcas: "1:0.5:2.25:⭐|2:0.5:7:🔖".into(),
            indice: true,
            lados: Some((0, 240)),
            voz: Some((17, 0.5)),
            sin_lado: true,
            de_antes: false,
        };
        let vuelta = de_texto(&a_texto(&a));
        assert_eq!(vuelta, a);
    }

    #[test]
    fn un_fichero_de_antes_se_sigue_leyendo_y_sale_sin_tinta() {
        // Lo que escribia el visor antes de poder anotar: sin las claves
        // nuevas, todo lo nuevo sale de fabrica.
        let a = de_texto("pixpin-lectura 1\ntamano 115\nsitio 0.5\nmarcadores 1:0.2:🔖\n");
        assert_eq!(a.tamano, 115);
        assert_eq!(a.marcadores.len(), 1);
        assert_eq!(a.columna, 0);
        assert!(!a.letra_fijada());
        // Sin la clave, el indice del PDF aun no se recogio; y no se escribe
        // mientras no sea verdad.
        assert!(!a.indice);
        assert!(!a_texto(&Ajustes::default()).contains("indice"));
        assert_eq!(a.zoom, 1.0);
        assert_eq!(a.espacios, 0);
    }

    #[test]
    fn un_fichero_del_pc_de_antes_traduce_su_letra_a_la_del_movil_y_avisa_de_su_tinta() {
        // Segoe UI gruesa en la version 1: la sans del movil, gruesa.
        let a = de_texto("pixpin-lectura 1\ntamano 230\ntipo 0\ngrosor 1\ncolumna 860\n");
        assert_eq!((a.tipo, a.grosor), (1, 2));
        assert!(a.de_antes, "su tinta es de la maqueta vieja del PC");
        // Consolas normal: la de ancho fijo, normal.
        let a = de_texto("pixpin-lectura 1\ntipo 1\ngrosor 0\n");
        assert_eq!((a.tipo, a.grosor), (2, 1));
        // Sin decir letra, la de antes (Segoe UI normal), no la de fabrica.
        let a = de_texto("pixpin-lectura 1\ntamano 100\n");
        assert_eq!((a.tipo, a.grosor), (1, 1));
        // Caso negativo: lo que ya es de la version 2 se lee tal cual, y al
        // guardarlo deja de ser «de antes».
        let b = de_texto("pixpin-lectura 2\ntipo 0\ngrosor 3\n");
        assert_eq!((b.tipo, b.grosor, b.de_antes), (0, 3, false));
        let vuelta = de_texto(&a_texto(&a));
        assert!(!vuelta.de_antes);
        assert_eq!((vuelta.tipo, vuelta.grosor), (1, 1));
        // Y sin fichero, la letra de fabrica del movil: serif normal.
        assert_eq!((Ajustes::default().tipo, Ajustes::default().grosor), (0, 1));
        assert_eq!(PESOS[Ajustes::default().grosor as usize], 400);
    }

    #[test]
    fn valores_imposibles_de_lo_nuevo_no_rompen_nada() {
        let a = de_texto(
            "pixpin-lectura 2\ntipo 9\ngrosor 7\ncolumna 999999\nzoom NaN\nespacios 255\npagina -3\n",
        );
        assert_eq!(a.tipo, (TIPOS - 1) as u8);
        assert_eq!(a.grosor, (GROSORES - 1) as u8);
        assert_eq!(a.columna, 20_000);
        assert_eq!(
            a.zoom, 1.0,
            "un aumento que no es numero vuelve al de siempre"
        );
        assert_eq!(a.espacios, 3, "solo hay dos lados");
        assert_eq!(a.pagina, 0.0);
        assert_eq!(
            de_texto("zoom 0\n").zoom,
            1.0,
            "aumento cero dejaria el documento invisible"
        );
    }

    #[test]
    fn un_salto_de_linea_en_las_marcas_no_parte_el_fichero() {
        let a = Ajustes {
            marcas: "1:0:0:🔖\ntamano 250".into(),
            ..Ajustes::default()
        };
        assert_eq!(
            de_texto(&a_texto(&a)).tamano,
            100,
            "lo de detras del salto no puede colarse como otra clave"
        );
    }

    #[test]
    fn un_fichero_roto_no_rompe_la_lectura() {
        let a = de_texto("basura\ntamano no-es-un-numero\nsitio lejos\nmarcadores ;;;");
        assert_eq!(a, Ajustes::default());
        assert_eq!(de_texto(""), Ajustes::default());
    }

    #[test]
    fn un_marcador_a_medio_escribir_se_tira_y_los_demas_no() {
        let lista = marcadores_de_texto("1:0.2:🔖|basura|3:0.5:⭐|:0.7:x");
        assert_eq!(lista.len(), 2);
        assert_eq!(lista[0].id, 1);
        assert_eq!(lista[1].id, 3);
    }

    #[test]
    fn un_marcador_sin_emoticono_se_queda_con_el_de_siempre() {
        let lista = marcadores_de_texto("1:0.2:");
        assert_eq!(lista[0].emoji, EMOJIS[0]);
    }

    #[test]
    fn el_tamano_se_queda_entre_el_minimo_y_el_maximo() {
        assert_eq!(tamano_valido(10), TAMANO_MIN);
        assert_eq!(tamano_valido(999), TAMANO_MAX);
        assert_eq!(tamano_valido(120), 120);
    }

    #[test]
    fn la_barra_de_puntos_no_se_sale_por_los_extremos() {
        assert_eq!(tamano_vecino(TAMANOS[0], false), TAMANOS[0]);
        assert_eq!(
            tamano_vecino(TAMANOS[TAMANOS.len() - 1], true),
            TAMANOS[TAMANOS.len() - 1]
        );
        assert_eq!(tamano_vecino(100, true), 115);
        assert_eq!(tamano_vecino(100, false), 90);
    }

    #[test]
    fn el_fichero_de_ajustes_es_hermano_del_documento() {
        let r = ruta_de_ajustes(Path::new("C:/apuntes/tema 1.docx"));
        assert_eq!(
            r.file_name().unwrap().to_string_lossy(),
            "tema 1.docx.pixpin-lectura"
        );
        assert_eq!(r.parent(), Path::new("C:/apuntes/tema 1.docx").parent());
    }
}
