//! **La linea de ordenes de PixPin** (15-oct-2026): `pixpinmax <verbo> …`,
//! `pixpinmax --pedido '<json>'` y los enlaces `pixpin://…`.
//!
//! Hasta ahora, fuera de Flow Launcher y de un script que supiera mandar un
//! `WM_COPYDATA`, nadie podia pedirle nada a PixPin: un acceso directo, un
//! Stream Deck, el Programador de tareas, «Enviar a» del Explorador o un
//! enlace en una nota de Obsidian no tenian como. Esto es esa puerta, y es
//! **solo un adaptador**: todo acaba en un pedido del protocolo de siempre
//! (`docs/protocolo-pedidos.md`) que se manda por `mensajero` a la copia que
//! corre. Nada se hace aqui que no pudiera pedir ya el plugin de Flow.
//!
//! - Los verbos legibles son **una tabla** ([`VERBOS`]), no un `match` por
//!   verbo, como las herramientas del asistente (`pixpin-ia/acciones.rs`):
//!   la ayuda, los enlaces y la consola salen de la misma fila.
//! - Un enlace `pixpin://<verbo>?campo=valor` es **el mismo verbo** con sus
//!   campos con nombre; `pixpin://atajo/<verbo>/<id>?…` es la seña de los
//!   atajos de Android (`atajos/AtajoActivity.kt`), para que un mismo enlace
//!   sirva en el movil y en el PC.
//! - Por enlace solo pasa directo lo inofensivo ([`seguridad`]): lo demas
//!   pregunta, y lo que abre una ruta de red no pasa nunca. Por la consola
//!   pasa todo: quien la escribe ya esta sentado delante del PC.
//!
//! Todo lo de aqui es puro (texto entra, pedidos salen) para probarlo sin
//! ventanas; lo que toca el sistema (mandar, la consola, el OCR, los
//! cuadros) lo hace quien llama: `apps/pixpin/src/orden.rs`.

pub mod firma;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

/// Los codigos de salida de una orden. Los cinco primeros son los mismos de
/// `pixpin-lanzador pedido` (y los `2`/`3` son las respuestas de la propia
/// ventana, `mensajero::respuesta`), para que un script no tenga que
/// distinguir quien le contesto.
pub mod codigo {
    /// La app acepto el pedido (o la orden local salio bien).
    pub const ACEPTADO: i32 = 0;
    /// Fallo aqui mismo: un fichero que no esta, un OCR que no leyo nada.
    pub const FALLO: i32 = 1;
    /// La app no entiende esa version del protocolo.
    pub const VERSION: i32 = 2;
    /// La orden, el enlace o el JSON no se entienden.
    pub const NO_SE_ENTIENDE: i32 = 3;
    /// PixPin no esta abierta y no se pudo abrir.
    pub const SIN_APP: i32 = 4;
    /// La app no contesto a tiempo (o contesto algo que no es del protocolo).
    pub const SIN_RESPUESTA: i32 = 5;
    /// No se hizo a proposito: un enlace que no puede pedir eso, o el
    /// usuario dijo que no en el cuadro de confirmar.
    pub const RECHAZADO: i32 = 6;
}

/// Lo mas largo que se acepta en un enlace. Un enlace es una orden, no un
/// documento: el tope evita que una pagina meta megas por la puerta.
pub const TOPE_ENLACE: usize = 16 * 1024;

/// Las extensiones que se tratan como imagen (las de `imagenes` del
/// protocolo: `chat`, `anadir_tarea`, `enviar_al_movil`…).
pub const EXTENSIONES_IMAGEN: [&str; 7] = ["png", "jpg", "jpeg", "bmp", "gif", "webp", "tif"];

/// Por que no se pudo entender una orden, con el codigo con que salir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fallo {
    pub codigo: i32,
    pub motivo: String,
}

impl Fallo {
    fn no_se_entiende(motivo: impl Into<String>) -> Self {
        Fallo {
            codigo: codigo::NO_SE_ENTIENDE,
            motivo: motivo.into(),
        }
    }
}

/// Lo que hay que hacer.
#[derive(Debug, Clone, PartialEq)]
pub enum Orden {
    /// Pedidos del protocolo, en orden, para la copia que corre.
    Pedidos(Vec<Value>),
    /// Leer el texto de unas imagenes aqui mismo y escribirlo en la consola
    /// (y en el portapapeles con `copiar`). No necesita la app abierta.
    Ocr { rutas: Vec<PathBuf>, copiar: bool },
    /// Preguntar al asistente por unos ficheros: falta la pregunta, que se
    /// pide en una caja antes de mandar nada (el menu del Explorador).
    Preguntar {
        rutas: Vec<PathBuf>,
        proyecto: Option<String>,
    },
    /// La ayuda.
    Ayuda,
}

/// De donde vino la orden: decide cuanto se confia en ella.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origen {
    /// La linea de ordenes, «Enviar a», el menu del Explorador, un script.
    Consola,
    /// Un enlace `pixpin://`. `firmado`: lo hizo esta misma PixPin (los
    /// botones de sus avisos), ver [`firma`].
    Enlace { firmado: bool },
}

/// Una orden ya leida.
#[derive(Debug, Clone, PartialEq)]
pub struct Llamada {
    pub orden: Orden,
    pub origen: Origen,
    /// `--sin-arrancar`: si PixPin no esta abierta, no abrirla (salir con 4).
    pub sin_arrancar: bool,
}

/// Lo que se le pasa a cada fila de la tabla.
pub struct Entrada<'a> {
    /// Lo que va suelto detras del verbo (o los trozos del camino del enlace).
    pub sueltos: Vec<String>,
    /// `--clave valor` en la consola; `?clave=valor` en un enlace.
    pub campos: BTreeMap<String, String>,
    /// Si una ruta es un fichero que existe. En un enlace siempre `false`:
    /// alli una ruta solo entra con su nombre (`ruta=`), y asi pasa por
    /// [`seguridad`].
    pub es_fichero: &'a dyn Fn(&Path) -> bool,
    /// Si viene de un enlace.
    pub enlace: bool,
}

/// Una fila de la tabla de verbos.
pub struct Verbo {
    /// El nombre y sus alias (el primero es el que sale en la ayuda). Van
    /// los de Android (`Atajos.kt`) para que `pixpin://atajo/…` coincida.
    pub nombres: &'static [&'static str],
    /// Como se escribe, para la ayuda.
    pub uso: &'static str,
    /// Que hace, en una linea.
    pub ayuda: &'static str,
    /// Las `--opciones` que admite (sin los guiones). Las banderas sin
    /// valor van en [`BANDERAS`].
    pub opciones: &'static [&'static str],
    /// Si se puede pedir por enlace (`pixpin://`).
    pub por_enlace: bool,
    armar: fn(&Entrada) -> Result<Orden, Fallo>,
}

/// Las opciones que no llevan valor.
pub const BANDERAS: [&str; 2] = ["copiar", "sin-arrancar"];

/// **La tabla.** El orden es el de la ayuda.
pub const VERBOS: &[Verbo] = &[
    Verbo {
        nombres: &["capturar", "capture"],
        uso: "capturar",
        ayuda: "Captura una zona de la pantalla, como el atajo general.",
        opciones: &[],
        por_enlace: true,
        armar: |_| pedidos(vec![pedido("capturar", json!({}))]),
    },
    Verbo {
        nombres: &["pinear", "pin"],
        uso: "pinear <imagen|fichero>… | --codigo <id> [--proyecto P]",
        ayuda: "Saca a la pantalla como pin cada fichero (o el del mensaje <id> del chat).",
        opciones: &["codigo", "proyecto", "ruta"],
        por_enlace: true,
        armar: armar_pinear,
    },
    Verbo {
        nombres: &["pinear-ultima", "ultima"],
        uso: "pinear-ultima",
        ayuda: "Saca como pin la captura mas reciente.",
        opciones: &[],
        por_enlace: true,
        armar: |_| pedidos(vec![pedido("pinear_ultima", json!({}))]),
    },
    Verbo {
        nombres: &["archivo"],
        uso: "archivo <id del mensaje> [--proyecto P]",
        ayuda: "Saca a la pantalla el fichero de ese mensaje del chat (pixpin://atajo/archivo/<id> de Android).",
        opciones: &["proyecto", "id"],
        por_enlace: true,
        armar: armar_archivo,
    },
    Verbo {
        nombres: &["chat", "mensaje"],
        uso: "chat [texto] [fichero…] [--proyecto P]",
        ayuda: "Escribe en el chat (sin proyecto, en Mensajes guardados) el texto y los ficheros; sin nada, abre ese chat.",
        opciones: &["proyecto", "texto"],
        por_enlace: true,
        armar: armar_chat,
    },
    Verbo {
        nombres: &["mensajes", "guardados"],
        uso: "mensajes",
        ayuda: "Abre Mensajes guardados.",
        opciones: &[],
        por_enlace: true,
        armar: |_| {
            pedidos(vec![pedido(
                "abrir",
                json!({ "que": { "tipo": "proyecto", "proyecto": null } }),
            )])
        },
    },
    Verbo {
        nombres: &["proyectos"],
        uso: "proyectos",
        ayuda: "Abre la ventana principal (el chat de proyectos).",
        opciones: &[],
        por_enlace: true,
        armar: |_| pedidos(vec![pedido("ventana_principal", json!({}))]),
    },
    Verbo {
        nombres: &["proyecto"],
        uso: "proyecto <id|nombre>",
        ayuda: "Abre el chat de ese proyecto.",
        opciones: &["id", "proyecto"],
        por_enlace: true,
        armar: armar_proyecto,
    },
    Verbo {
        nombres: &["tarea", "task", "todo"],
        uso: "tarea \"<texto>\" [--proyecto P] [--lista <id>]",
        ayuda: "Apunta una tarea (sin proyecto ni lista, en el Inbox); sin texto, abre las tareas.",
        opciones: &["proyecto", "lista", "texto"],
        por_enlace: true,
        armar: armar_tarea,
    },
    Verbo {
        nombres: &["tareas", "tasks"],
        uso: "tareas",
        ayuda: "Abre la ventana de tareas.",
        opciones: &[],
        por_enlace: true,
        armar: |_| ventana("tareas"),
    },
    Verbo {
        nombres: &["capturas", "galeria"],
        uso: "capturas",
        ayuda: "Abre la galeria de capturas.",
        opciones: &[],
        por_enlace: true,
        armar: |_| ventana("galeria"),
    },
    Verbo {
        nombres: &["nota", "note"],
        uso: "nota [texto] [imagen…] [--proyecto P]",
        ayuda: "Abre una nota nueva en el editor, empezada con el texto.",
        opciones: &["proyecto", "texto"],
        por_enlace: true,
        armar: armar_nota,
    },
    Verbo {
        nombres: &["lienzo", "dibujo"],
        uso: "lienzo [nombre] [--proyecto P]",
        ayuda: "Abre un lienzo nuevo.",
        opciones: &["proyecto", "nombre"],
        por_enlace: true,
        armar: armar_lienzo,
    },
    Verbo {
        nombres: &["al-lienzo", "a-lienzo"],
        uso: "al-lienzo <nombre> [texto] [imagen…] [--proyecto P]",
        ayuda: "Manda el texto y las imagenes al lienzo de ese nombre, apilados en una columna libre (si no existe, lo crea; si esta abierto, entra en vivo). Con espacios en el nombre: --lienzo \"Plano casa\".",
        opciones: &["proyecto", "lienzo", "texto"],
        por_enlace: true,
        armar: armar_al_lienzo,
    },
    Verbo {
        nombres: &["lista"],
        uso: "lista [titulo] [--proyecto P]",
        ayuda: "Crea una lista de tareas nueva.",
        opciones: &["proyecto", "titulo"],
        por_enlace: true,
        armar: armar_lista,
    },
    Verbo {
        nombres: &["leccion"],
        uso: "leccion [texto] [imagen…] [--proyecto P]",
        ayuda: "Abre la ficha de una leccion aprendida nueva.",
        opciones: &["proyecto", "texto"],
        por_enlace: true,
        armar: armar_leccion,
    },
    Verbo {
        nombres: &["lecciones"],
        uso: "lecciones [buscar] [--proyecto P]",
        ayuda: "Abre la lista de lecciones (ya buscando, con texto).",
        opciones: &["proyecto", "consulta"],
        por_enlace: true,
        armar: armar_lecciones,
    },
    Verbo {
        nombres: &["grabar", "voz"],
        uso: "grabar [nombre] [--proyecto P]",
        ayuda: "Saca el microfono flotante y graba una nota de voz.",
        opciones: &["proyecto", "nombre"],
        por_enlace: true,
        armar: armar_grabar,
    },
    Verbo {
        nombres: &["soltar"],
        uso: "soltar [--proyecto P]",
        ayuda: "Saca la caja flotante a la que soltar ficheros para ese chat.",
        opciones: &["proyecto"],
        por_enlace: true,
        armar: armar_soltar,
    },
    Verbo {
        nombres: &["sincronizar", "sync"],
        uso: "sincronizar [todos|<aparato>]",
        ayuda: "Sin nada, abre Sincronizar; con «todos» o un aparato, sincroniza ya.",
        opciones: &["aparato"],
        por_enlace: true,
        armar: armar_sincronizar,
    },
    Verbo {
        nombres: &["al-movil", "movil"],
        uso: "al-movil <fichero>… [--aparato <id>]",
        ayuda: "Manda los ficheros al movil (a todos si no se dice): imagenes al lienzo, lo demas al chat.",
        opciones: &["aparato"],
        por_enlace: false,
        armar: armar_al_movil,
    },
    Verbo {
        nombres: &["ocr", "texto"],
        uso: "ocr <imagen>… [--copiar]",
        ayuda: "Escribe en la consola el texto de las imagenes (y lo copia con --copiar). No necesita PixPin abierta.",
        opciones: &[],
        por_enlace: false,
        armar: armar_ocr,
    },
    Verbo {
        nombres: &["copiar-texto"],
        uso: "copiar-texto <imagen>…",
        ayuda: "PixPin lee el texto de las imagenes, lo copia y lo dice en su aviso (el menu del Explorador).",
        opciones: &[],
        por_enlace: false,
        armar: armar_copiar_texto,
    },
    Verbo {
        nombres: &["leer", "leer-en-voz"],
        uso: "leer \"<texto>\" [--titulo T]",
        ayuda: "Lee el texto en voz alta con la voz de PixPin, sin abrir nada (se para con las teclas multimedia).",
        opciones: &["texto", "titulo"],
        por_enlace: true,
        armar: armar_leer,
    },
    Verbo {
        nombres: &["asistente", "ia", "preguntar"],
        uso: "asistente [\"pedido\"] [fichero…] [--proyecto P]",
        ayuda: "Le pide algo al asistente sin abrir el chat. Con ficheros y sin pedido, pregunta que hacer con ellos.",
        opciones: &["proyecto", "texto"],
        por_enlace: true,
        armar: armar_asistente,
    },
    Verbo {
        nombres: &["abrir", "open"],
        uso: "abrir <fichero>… [--pagina N] [--buscar T]",
        ayuda: "Abre cada fichero como «Abrir con PixPin» (lector, editor o pin).",
        opciones: &[
            "pagina",
            "buscar",
            "ruta",
            "tipo",
            "proyecto",
            "codigo",
            "referencia",
        ],
        por_enlace: true,
        armar: armar_abrir,
    },
    Verbo {
        nombres: &["ventana"],
        uso: "ventana tareas|galeria|lecciones|sincronizar",
        ayuda: "Abre esa ventana de la bandeja.",
        opciones: &["cual"],
        por_enlace: true,
        armar: armar_ventana,
    },
    Verbo {
        nombres: &["pedido"],
        uso: "pedido '<json>'",
        ayuda: "Manda un pedido del protocolo tal cual (igual que --pedido).",
        opciones: &["json"],
        por_enlace: true,
        armar: armar_pedido,
    },
    Verbo {
        nombres: &["ayuda", "help"],
        uso: "ayuda",
        ayuda: "Esta ayuda.",
        opciones: &[],
        por_enlace: false,
        armar: |_| Ok(Orden::Ayuda),
    },
];

/// La fila de un verbo, sin mirar mayusculas.
pub fn verbo(nombre: &str) -> Option<&'static Verbo> {
    let n = nombre.trim().to_lowercase();
    VERBOS.iter().find(|v| v.nombres.contains(&n.as_str()))
}

// ---------------------------------------------------------------------------
// La linea de ordenes
// ---------------------------------------------------------------------------

/// **Lee la linea de ordenes** (sin el ejecutable). `None` si no es una
/// orden: es el arranque de siempre, o ficheros de «Abrir con», o una orden
/// interna (`--atajo`, `--cad-convertir`…), y entonces el `main` sigue como
/// antes.
pub fn de_argumentos(
    args: &[String],
    es_fichero: &dyn Fn(&Path) -> bool,
) -> Option<Result<Llamada, Fallo>> {
    let sin_arrancar = args.iter().any(|a| a == "--sin-arrancar");
    let args: Vec<&String> = args.iter().filter(|a| *a != "--sin-arrancar").collect();
    let primero = args.first()?.as_str();
    let llamada = |r: Result<Orden, Fallo>, origen: Origen| {
        r.map(|orden| Llamada {
            orden,
            origen,
            sin_arrancar,
        })
    };
    let resto: Vec<String> = args.iter().skip(1).map(|s| s.to_string()).collect();
    let r = match primero {
        "--pedido" => llamada(
            resto
                .first()
                .ok_or_else(|| Fallo::no_se_entiende("falta el JSON detras de --pedido"))
                .and_then(|j| de_json(j)),
            Origen::Consola,
        ),
        "--pedido-archivo" => llamada(
            resto
                .first()
                .ok_or_else(|| Fallo::no_se_entiende("falta la ruta detras de --pedido-archivo"))
                .and_then(|r| leer_pedido_de(r))
                .and_then(|j| de_json(&j)),
            Origen::Consola,
        ),
        "--url" => match resto.first() {
            Some(u) => de_enlace(u).map(|l| Llamada { sin_arrancar, ..l }),
            None => Err(Fallo::no_se_entiende("falta el enlace detras de --url")),
        },
        // `--atajo url "<enlace>"`: como registraban el protocolo los avisos
        // con botones (`notificacion`, 15-oct) antes de unirse aqui. Es un
        // alias de `--url`: el mismo traductor y la misma lista blanca.
        "--atajo" if resto.first().map(String::as_str) == Some(crate::jump_list::URL) => {
            match resto.get(1) {
                Some(u) => de_enlace(u).map(|l| Llamada { sin_arrancar, ..l }),
                None => Err(Fallo::no_se_entiende(
                    "falta el enlace detras de --atajo url",
                )),
            }
        }
        "--ayuda" | "--help" | "-h" | "-?" | "/?" => Ok(Llamada {
            orden: Orden::Ayuda,
            origen: Origen::Consola,
            sin_arrancar,
        }),
        u if es_enlace(u) => de_enlace(u).map(|l| Llamada { sin_arrancar, ..l }),
        // Las ordenes internas de la app no son nuestras.
        o if o.starts_with("--") => return None,
        v => {
            // Una palabra que no es un verbo es el arranque de siempre (una
            // ruta de «Abrir con», normalmente): no se le toca.
            let fila = verbo(v)?;
            llamada(armar_de_consola(fila, &resto, es_fichero), Origen::Consola)
        }
    };
    Some(r)
}

/// Los esquemas que se entienden. `pixpin:` es el de Android; en el PC lo
/// puede tener ya el PixPin original de DepthPixel (que se instala con el),
/// y entonces el nuestro es `pixpinmax:` (`herramientas/integraciones.ps1`
/// no le quita el suyo a nadie). Los dos dan lo mismo.
pub const ESQUEMAS: [&str; 2] = ["pixpinmax:", "pixpin:"];

/// El largo del esquema si un texto es un enlace nuestro.
fn esquema(texto: &str) -> Option<usize> {
    ESQUEMAS
        .iter()
        .find(|e| {
            texto.len() >= e.len()
                && texto.is_char_boundary(e.len())
                && texto[..e.len()].eq_ignore_ascii_case(e)
        })
        .map(|e| e.len())
}

/// Si un texto es un enlace `pixpin:` (o `pixpinmax:`).
pub fn es_enlace(texto: &str) -> bool {
    esquema(texto).is_some()
}

fn armar_de_consola(
    fila: &Verbo,
    resto: &[String],
    es_fichero: &dyn Fn(&Path) -> bool,
) -> Result<Orden, Fallo> {
    let mut sueltos = Vec::new();
    let mut campos = BTreeMap::new();
    let mut i = 0;
    let mut solo_sueltos = false;
    while i < resto.len() {
        let a = &resto[i];
        i += 1;
        if solo_sueltos || !a.starts_with("--") || a.len() == 2 {
            if a == "--" && !solo_sueltos {
                solo_sueltos = true;
                continue;
            }
            sueltos.push(a.clone());
            continue;
        }
        let (clave, valor) = match a[2..].split_once('=') {
            Some((c, v)) => (c.to_lowercase(), Some(v.to_string())),
            None => (a[2..].to_lowercase(), None),
        };
        if BANDERAS.contains(&clave.as_str()) {
            campos.insert(clave, "1".into());
            continue;
        }
        if !fila.opciones.contains(&clave.as_str()) {
            return Err(Fallo::no_se_entiende(format!(
                "«{}» no tiene la opcion --{clave} (pixpinmax ayuda)",
                fila.nombres[0]
            )));
        }
        let valor = match valor {
            Some(v) => v,
            None => {
                let v = resto
                    .get(i)
                    .cloned()
                    .ok_or_else(|| Fallo::no_se_entiende(format!("falta el valor de --{clave}")))?;
                i += 1;
                v
            }
        };
        campos.insert(clave, valor);
    }
    // Las rutas relativas de la consola son de la carpeta donde se escribio.
    let sueltos = sueltos
        .into_iter()
        .map(|s| {
            let p = Path::new(&s);
            if !p.is_absolute() && es_fichero(p) {
                std::path::absolute(p)
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or(s)
            } else {
                s
            }
        })
        .collect();
    (fila.armar)(&Entrada {
        sueltos,
        campos,
        es_fichero,
        enlace: false,
    })
}

/// Lee un pedido de un fichero (`-` es la entrada estandar).
fn leer_pedido_de(ruta: &str) -> Result<String, Fallo> {
    use std::io::Read;
    let mut texto = String::new();
    let leido = if ruta == "-" {
        std::io::stdin()
            .take(crate::mensajero::TOPE_PEDIDO as u64 + 1)
            .read_to_string(&mut texto)
            .map(|_| ())
    } else {
        std::fs::File::open(ruta)
            .and_then(|f| {
                f.take(crate::mensajero::TOPE_PEDIDO as u64 + 1)
                    .read_to_string(&mut texto)
            })
            .map(|_| ())
    };
    leido.map_err(|e| Fallo {
        codigo: codigo::FALLO,
        motivo: format!("no se pudo leer {ruta}: {e}"),
    })?;
    // El BOM que deja el «Out-File» de PowerShell 5 no es parte del JSON.
    Ok(texto.trim_start_matches('\u{feff}').trim().to_string())
}

/// Un pedido escrito a mano: se mira como lo miraria la ventana, para
/// contestar 2 o 3 sin molestar a la app (ni abrirla).
fn de_json(json: &str) -> Result<Orden, Fallo> {
    use crate::mensajero::{respuesta, validar_pedido};
    match validar_pedido(json) {
        respuesta::ACEPTADO => {}
        respuesta::VERSION_NO => {
            return Err(Fallo {
                codigo: codigo::VERSION,
                motivo: "esa version del protocolo no existe".into(),
            });
        }
        _ => {
            return Err(Fallo::no_se_entiende(
                "el pedido no es JSON del protocolo, o su accion no existe (docs/protocolo-pedidos.md)",
            ));
        }
    }
    if json.len() > crate::mensajero::TOPE_PEDIDO {
        return Err(Fallo::no_se_entiende("el pedido pasa de 64 KB"));
    }
    let v: Value = serde_json::from_str(json).map_err(|e| Fallo::no_se_entiende(e.to_string()))?;
    Ok(Orden::Pedidos(vec![v]))
}

// ---------------------------------------------------------------------------
// Los enlaces
// ---------------------------------------------------------------------------

/// **Lee un enlace `pixpin://`.** Valen `pixpin://<verbo>/<id>?campo=valor`
/// y la seña de Android `pixpin://atajo/<verbo>/<id>?campo=valor`; las
/// mayusculas del esquema y del verbo dan igual, y la barra final que
/// anaden algunos programas tambien.
pub fn de_enlace(url: &str) -> Result<Llamada, Fallo> {
    let partes = partir_enlace(url)?;
    let fila = verbo(&partes.verbo).ok_or_else(|| {
        Fallo::no_se_entiende(format!("PixPin no conoce «{}» por enlace", partes.verbo))
    })?;
    if !fila.por_enlace {
        return Err(Fallo {
            codigo: codigo::RECHAZADO,
            motivo: format!("«{}» no se puede pedir por enlace", fila.nombres[0]),
        });
    }
    let firmado = partes
        .firma
        .as_deref()
        .is_some_and(|f| firma::verificar(&partes.canonico(), f));
    let nadie = |_: &Path| false;
    let orden = (fila.armar)(&Entrada {
        sueltos: partes.sueltos,
        campos: partes.campos,
        es_fichero: &nadie,
        enlace: true,
    })?;
    Ok(Llamada {
        orden,
        origen: Origen::Enlace { firmado },
        sin_arrancar: false,
    })
}

/// Un enlace en trozos, ya descodificado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartesEnlace {
    pub verbo: String,
    pub sueltos: Vec<String>,
    pub campos: BTreeMap<String, String>,
    pub firma: Option<String>,
}

impl PartesEnlace {
    /// Lo que se firma: el verbo, los trozos y los campos ya descodificados
    /// y en orden. Asi la firma no depende de como escriba el enlace quien lo
    /// pase (`%20` o `+`, una barra final, el orden de los campos).
    pub fn canonico(&self) -> String {
        let mut s = self.verbo.clone();
        for t in &self.sueltos {
            s.push('\n');
            s.push_str(t);
        }
        for (k, v) in &self.campos {
            s.push('\n');
            s.push_str(k);
            s.push('=');
            s.push_str(v);
        }
        s
    }
}

/// Parte un enlace en verbo, trozos y campos.
pub fn partir_enlace(url: &str) -> Result<PartesEnlace, Fallo> {
    let url = url.trim();
    if url.len() > TOPE_ENLACE {
        return Err(Fallo::no_se_entiende("el enlace es demasiado largo"));
    }
    let Some(largo) = esquema(url) else {
        return Err(Fallo::no_se_entiende("no es un enlace pixpin:"));
    };
    let resto = &url[largo..];
    let resto = resto.split('#').next().unwrap_or("");
    let resto = resto.trim_start_matches('/');
    let (camino, consulta) = match resto.split_once('?') {
        Some((c, q)) => (c, q),
        None => (resto, ""),
    };
    let mut trozos = Vec::new();
    for t in camino.split('/').filter(|t| !t.is_empty()) {
        trozos.push(descodificar(t, false)?);
    }
    if trozos
        .first()
        .is_some_and(|t| t.eq_ignore_ascii_case("atajo"))
    {
        trozos.remove(0);
    }
    if trozos.is_empty() {
        return Err(Fallo::no_se_entiende("el enlace no dice que hacer"));
    }
    let verbo = trozos.remove(0).to_lowercase();
    let mut campos = BTreeMap::new();
    let mut firma = None;
    for par in consulta.split('&').filter(|p| !p.is_empty()) {
        let (k, v) = par.split_once('=').unwrap_or((par, ""));
        let k = descodificar(k, true)?.to_lowercase();
        let v = descodificar(v, true)?;
        if k == "firma" {
            firma = Some(v);
            continue;
        }
        // El primero gana: un campo repetido no puede cambiar lo que ya se
        // leyo (ni lo que se firmo).
        campos.entry(k).or_insert(v);
    }
    Ok(PartesEnlace {
        verbo,
        sueltos: trozos,
        campos,
        firma,
    })
}

/// `%XX` a bytes y bytes a UTF-8; en la consulta, `+` es un espacio. Un `%`
/// sin dos cifras o un UTF-8 roto es un enlace roto, no algo que adivinar.
pub fn descodificar(t: &str, consulta: bool) -> Result<String, Fallo> {
    let b = t.as_bytes();
    let mut fuera = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' => {
                let h = b
                    .get(i + 1..i + 3)
                    .and_then(|h| std::str::from_utf8(h).ok())
                    .and_then(|h| u8::from_str_radix(h, 16).ok());
                let Some(byte) = h else {
                    return Err(Fallo::no_se_entiende("el enlace tiene un % mal escrito"));
                };
                fuera.push(byte);
                i += 3;
            }
            b'+' if consulta => {
                fuera.push(b' ');
                i += 1;
            }
            c => {
                fuera.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8(fuera).map_err(|_| Fallo::no_se_entiende("el enlace no es UTF-8"))
}

/// Lo contrario de [`descodificar`], para armar enlaces (la ayuda, los
/// avisos con botones): todo lo que no sea letra, cifra o `-_.~`, en `%XX`.
pub fn codificar(t: &str) -> String {
    let mut s = String::with_capacity(t.len());
    for b in t.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            s.push(b as char);
        } else {
            s.push_str(&format!("%{b:02X}"));
        }
    }
    s
}

/// **El esquema de la casa**: con el que la app arma sus enlaces
/// ([`enlace`], los botones de los avisos de `notificacion`). Es `pixpin`
/// (el de Android) cuando `pixpin://` es nuestro o esta libre; si lo tiene
/// otro programa —en este PC (15-oct) lo registra el PixPin original de
/// DepthPixel, `…\Programs\PixPin\PixPin.exe`—, `pixpinmax`, que siempre es
/// nuestro: un boton con `pixpin://` abriria ese otro programa. Los dos
/// pasan por el mismo traductor ([`de_enlace`]) y la misma lista blanca
/// ([`seguridad`]). Se mira el registro una vez por proceso.
pub fn esquema_de_la_casa() -> &'static str {
    static ESQUEMA: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();
    ESQUEMA.get_or_init(|| {
        if pixpin_es_nuestro_o_libre() {
            "pixpin"
        } else {
            "pixpinmax"
        }
    })
}

/// Si `pixpin://` esta libre o lo abre un `pixpinmax.exe` (lo mismo que
/// mira `herramientas/integraciones.ps1` antes de tomarlo).
pub fn pixpin_es_nuestro_o_libre() -> bool {
    match orden_del_protocolo("pixpin") {
        None => true,
        Some(o) => o.to_lowercase().contains("pixpinmax.exe"),
    }
}

/// La orden con que Windows abre un protocolo (`HKCU`/`HKLM` combinados,
/// `HKEY_CLASSES_ROOT`), si hay.
pub fn orden_del_protocolo(esquema: &str) -> Option<String> {
    use windows::Win32::System::Registry::{HKEY_CLASSES_ROOT, RRF_RT_REG_SZ, RegGetValueW};
    let clave = windows::core::HSTRING::from(format!(r"{esquema}\shell\open\command"));
    let mut bytes = 0u32;
    // SAFETY: primero se pregunta el tamano (sin buffer); la clave es una
    // HSTRING viva durante las llamadas.
    unsafe {
        RegGetValueW(
            HKEY_CLASSES_ROOT,
            &clave,
            None,
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut bytes),
        )
    }
    .ok()
    .ok()?;
    let mut buf = vec![0u16; (bytes as usize).div_ceil(2).max(1)];
    // SAFETY: `buf` mide lo que dijo Windows y vive durante la llamada.
    unsafe {
        RegGetValueW(
            HKEY_CLASSES_ROOT,
            &clave,
            None,
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut _),
            Some(&mut bytes),
        )
    }
    .ok()
    .ok()?;
    let fin = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..fin]))
}

/// Arma un enlace `<esquema de la casa>://<verbo>?campo=valor…` (sin
/// firmar).
pub fn enlace(verbo: &str, campos: &[(&str, &str)]) -> String {
    enlace_con(esquema_de_la_casa(), verbo, campos)
}

/// [`enlace`] con un esquema dado (las pruebas).
pub fn enlace_con(esquema: &str, verbo: &str, campos: &[(&str, &str)]) -> String {
    let mut s = format!("{esquema}://{verbo}");
    for (i, (k, v)) in campos.iter().enumerate() {
        s.push(if i == 0 { '?' } else { '&' });
        s.push_str(&codificar(k));
        s.push('=');
        s.push_str(&codificar(v));
    }
    s
}

// ---------------------------------------------------------------------------
// La seguridad de los enlaces
// ---------------------------------------------------------------------------

/// Cuanto se fia uno de un pedido que llega por enlace.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Seguridad {
    /// Inofensivo: abrir algo, escribir un texto, empezar una captura.
    Directa,
    /// Hace algo que el usuario tiene que ver antes. El motivo es un codigo
    /// (`ficheros`, `asistente`, `microfono`, `otros-aparatos`,
    /// `automatizacion`, `borra`, `cambia`): la app lo traduce como
    /// `orden-motivo-<codigo>`.
    Confirmar(String),
    /// Por enlace, nunca. Codigo: `ruta-no-local` o `crear-automatizacion`.
    Prohibida(String),
}

/// **La lista blanca** de lo que un enlace sin firmar puede pedir sin
/// preguntar. Nada borra, nada corre automatizaciones, nada sincroniza ni
/// manda al movil, nada graba el microfono y nada abre una ruta del disco.
///
/// Es **la unica** lista: la de los botones de los avisos
/// (`notificacion::permitido`, la Jump List) es esta misma. De ellos vienen
/// las tres ultimas: marcar una tarea, posponer un recordatorio y conservar
/// una captura (que la app solo hace con una ruta de su carpeta de capturas,
/// `pedidos::captura_de`; una de red sigue sin pasar).
pub const SEGURAS: [&str; 17] = [
    "abrir",
    "chat",
    "nota_nueva",
    "lienzo_nuevo",
    "lista_nueva",
    "anadir_tarea",
    "ventana_principal",
    "ventana",
    "capturar",
    "lecciones",
    "leccion_nueva",
    "soltar",
    "pinear",
    "pinear_ultima",
    "marcar_tarea",
    "posponer",
    "conservar_captura",
];

/// **Cuanto se fia uno de un pedido llegado por enlace.** Se mira el pedido
/// ya armado, no el verbo, para que `pixpin://pedido?json=…` pase por lo
/// mismo que los verbos.
pub fn seguridad(pedido: &Value) -> Seguridad {
    let accion = pedido.get("accion").and_then(Value::as_str).unwrap_or("");
    // Las rutas primero: una de red (`\\servidor\…`) haria que Windows le
    // diera las credenciales del usuario al servidor solo por mirarla.
    let rutas = rutas_del_pedido(pedido);
    for r in &rutas {
        if !es_ruta_local(r) {
            return Seguridad::Prohibida("ruta-no-local".into());
        }
    }
    if accion == "automatizacion"
        && pedido.get("operacion").and_then(Value::as_str) == Some("crear")
    {
        return Seguridad::Prohibida("crear-automatizacion".into());
    }
    if !rutas.is_empty() && accion != "conservar_captura" {
        return Seguridad::Confirmar("ficheros".into());
    }
    let vacio = |c: &str| {
        pedido.get(c).is_none_or(|v| {
            v.is_null()
                || v.as_str().is_some_and(|s| s.trim().is_empty())
                || v.as_array().is_some_and(|a| a.is_empty())
        })
    };
    match accion {
        "asistente" if vacio("texto") && vacio("imagenes") && vacio("automatizacion") => {
            Seguridad::Directa
        }
        "asistente" => Seguridad::Confirmar("asistente".into()),
        a if SEGURAS.contains(&a) => Seguridad::Directa,
        "grabar" => Seguridad::Confirmar("microfono".into()),
        "sincronizar" | "enviar_al_movil" => Seguridad::Confirmar("otros-aparatos".into()),
        "automatizacion" => Seguridad::Confirmar("automatizacion".into()),
        a if a.starts_with("borrar") || a.starts_with("quitar") => {
            Seguridad::Confirmar("borra".into())
        }
        _ => Seguridad::Confirmar("cambia".into()),
    }
}

/// Todas las rutas de un pedido (`ruta`, `rutas`, `imagenes`, `archivos`,
/// `que.ruta`), tal cual vienen.
pub fn rutas_del_pedido(pedido: &Value) -> Vec<String> {
    let mut v = Vec::new();
    let mut de = |x: Option<&Value>| match x {
        Some(Value::String(s)) => v.push(s.clone()),
        Some(Value::Array(a)) => v.extend(a.iter().map(|e| {
            e.as_str()
                .map(str::to_string)
                .unwrap_or_else(|| e.to_string())
        })),
        Some(Value::Null) | None => {}
        Some(otro) => v.push(otro.to_string()),
    };
    for c in ["ruta", "rutas", "imagenes", "archivos"] {
        de(pedido.get(c));
    }
    de(pedido.get("que").and_then(|q| q.get("ruta")));
    v
}

/// Una ruta absoluta de una unidad de este equipo (`C:\…`). Ni de red
/// (`\\servidor\…`, `//…`), ni de dispositivo (`\\?\`, `\\.\`), ni
/// relativa (por enlace no hay «carpeta actual»), ni una URL.
pub fn es_ruta_local(r: &str) -> bool {
    let b = r.as_bytes();
    b.len() >= 3
        && b[0].is_ascii_alphabetic()
        && b[1] == b':'
        && (b[2] == b'\\' || b[2] == b'/')
        && !r.contains('\0')
}

/// Lo que se le ensena al usuario al confirmar: la accion y sus campos, en
/// texto corto (los textos largos, cortados).
pub fn describir(pedido: &Value) -> String {
    let mut s = String::new();
    if let Some(a) = pedido.get("accion").and_then(Value::as_str) {
        s.push_str(a);
    }
    if let Some(o) = pedido.as_object() {
        for (k, v) in o {
            if k == "pixpin" || k == "accion" {
                continue;
            }
            let texto = match v {
                Value::String(t) => t.clone(),
                otro => otro.to_string(),
            };
            let corto: String = texto.chars().take(160).collect();
            let puntos = if corto.len() < texto.len() { "…" } else { "" };
            s.push_str(&format!("\n  {k}: {corto}{puntos}"));
        }
    }
    s
}

// ---------------------------------------------------------------------------
// La ayuda
// ---------------------------------------------------------------------------

/// La ayuda de la consola, sacada de la tabla.
pub fn ayuda() -> String {
    let mut s = String::from(
        "PixPin Max: pedirle cosas a la PixPin que esta abierta (la abre si no lo esta).\n\n\
         Uso:\n  pixpinmax <verbo> [argumentos] [--sin-arrancar]\n  pixpinmax --pedido '<json>'\n  \
         pixpinmax --pedido-archivo <ruta|->\n  pixpinmax --url \"pixpin://<verbo>?campo=valor\"\n\nVerbos:\n",
    );
    for v in VERBOS {
        s.push_str(&format!("  {:<52} {}\n", v.uso, v.ayuda));
    }
    s.push_str(
        "\n--proyecto acepta el id o el nombre del proyecto. Sin --proyecto, Mensajes guardados.\n\
         Enlaces: pixpin://tarea?texto=comprar%20yeso, pixpin://proyecto/<id>, pixpin://atajo/capturar (Android).\n\n\
         Codigos de salida: 0 aceptado, 1 fallo aqui (fichero, OCR), 2 version, 3 no se entiende,\n\
         4 PixPin no esta abierta, 5 no contesto, 6 rechazado (enlace no permitido o dijo que no).\n\
         Mas: docs/integraciones.md y docs/protocolo-pedidos.md\n",
    );
    s
}

// ---------------------------------------------------------------------------
// Las filas
// ---------------------------------------------------------------------------

/// Un pedido v1.
pub fn pedido(accion: &str, campos: Value) -> Value {
    let mut m = Map::new();
    m.insert("pixpin".into(), json!(1));
    m.insert("accion".into(), json!(accion));
    if let Value::Object(c) = campos {
        for (k, v) in c {
            if !v.is_null() || k == "proyecto" {
                m.insert(k, v);
            }
        }
    }
    Value::Object(m)
}

fn pedidos(v: Vec<Value>) -> Result<Orden, Fallo> {
    Ok(Orden::Pedidos(v))
}

fn ventana(cual: &str) -> Result<Orden, Fallo> {
    pedidos(vec![pedido("ventana", json!({ "cual": cual }))])
}

/// Si una ruta es de imagen, por su extension.
pub fn es_imagen(ruta: &Path) -> bool {
    ruta.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONES_IMAGEN.contains(&e.to_lowercase().as_str()))
}

impl Entrada<'_> {
    fn campo(&self, c: &str) -> Option<String> {
        self.campos
            .get(c)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    fn bandera(&self, c: &str) -> bool {
        self.campos.contains_key(c)
    }

    /// El proyecto pedido, como lo quiere el protocolo (`null` = Mensajes
    /// guardados). El nombre lo cambia por su id quien manda
    /// (`apps/pixpin/src/orden.rs`), que es quien puede leer el disco.
    fn proyecto(&self) -> Value {
        self.campo("proyecto")
            .map(Value::String)
            .unwrap_or(Value::Null)
    }

    /// Los sueltos partidos en ficheros (los que existen) y texto (lo demas,
    /// junto con espacios), mas el campo `texto` de un enlace.
    fn texto_y_ficheros(&self) -> (String, Vec<PathBuf>) {
        let mut texto = Vec::new();
        let mut ficheros = Vec::new();
        for s in &self.sueltos {
            let p = Path::new(s);
            if !self.enlace && (self.es_fichero)(p) {
                ficheros.push(p.to_path_buf());
            } else {
                texto.push(s.as_str());
            }
        }
        let mut t = texto.join(" ");
        if let Some(c) = self.campo("texto") {
            if !t.is_empty() {
                t.push(' ');
            }
            t.push_str(&c);
        }
        (t.trim().to_string(), ficheros)
    }

    /// Solo ficheros: lo suelto que no lo sea es un error que se dice.
    fn solo_ficheros(&self) -> Result<Vec<PathBuf>, Fallo> {
        let mut v = Vec::new();
        for s in &self.sueltos {
            let p = Path::new(s);
            if self.enlace || !(self.es_fichero)(p) {
                return Err(Fallo {
                    codigo: codigo::FALLO,
                    motivo: format!("no existe el fichero «{s}»"),
                });
            }
            v.push(p.to_path_buf());
        }
        if let Some(r) = self.campo("ruta") {
            v.push(PathBuf::from(r));
        }
        Ok(v)
    }
}

fn ruta_json(p: &Path) -> Value {
    Value::String(p.to_string_lossy().into_owned())
}

fn partir_imagenes(ficheros: &[PathBuf]) -> (Vec<Value>, Vec<Value>) {
    let (i, a): (Vec<&PathBuf>, Vec<&PathBuf>) = ficheros.iter().partition(|p| es_imagen(p));
    (
        i.into_iter().map(|p| ruta_json(p)).collect(),
        a.into_iter().map(|p| ruta_json(p)).collect(),
    )
}

fn armar_pinear(e: &Entrada) -> Result<Orden, Fallo> {
    if let Some(c) = e.campo("codigo") {
        return pedidos(vec![pedido(
            "pinear",
            json!({ "codigo": c, "proyecto": e.proyecto() }),
        )]);
    }
    let rutas = e.solo_ficheros()?;
    if rutas.is_empty() {
        return Err(Fallo::no_se_entiende(
            "pinear: falta el fichero (o --codigo)",
        ));
    }
    pedidos(
        rutas
            .iter()
            .map(|r| pedido("pinear", json!({ "ruta": ruta_json(r) })))
            .collect(),
    )
}

fn armar_archivo(e: &Entrada) -> Result<Orden, Fallo> {
    let id = e
        .sueltos
        .first()
        .cloned()
        .or_else(|| e.campo("id"))
        .ok_or_else(|| Fallo::no_se_entiende("archivo: falta el id del mensaje"))?;
    pedidos(vec![pedido(
        "pinear",
        json!({ "codigo": id, "proyecto": e.proyecto() }),
    )])
}

fn armar_chat(e: &Entrada) -> Result<Orden, Fallo> {
    let (texto, ficheros) = e.texto_y_ficheros();
    if texto.is_empty() && ficheros.is_empty() {
        return pedidos(vec![pedido(
            "abrir",
            json!({ "que": { "tipo": "proyecto", "proyecto": e.proyecto() } }),
        )]);
    }
    // Lo que va como foto lo decide la misma regla que `--al-chat` y el
    // panel Compartir (`compartido::es_foto`): un GIF o un WebP van tal cual.
    let (fotos, otros): (Vec<PathBuf>, Vec<PathBuf>) = ficheros
        .into_iter()
        .partition(|p| crate::compartido::es_foto(p));
    let imagenes: Vec<Value> = fotos.iter().map(|p| ruta_json(p)).collect();
    let archivos: Vec<Value> = otros.iter().map(|p| ruta_json(p)).collect();
    let mut campos = json!({ "texto": texto, "proyecto": e.proyecto() });
    if !imagenes.is_empty() {
        campos["imagenes"] = Value::Array(imagenes);
    }
    if !archivos.is_empty() {
        campos["archivos"] = Value::Array(archivos);
    }
    pedidos(vec![pedido("chat", campos)])
}

/// El pedido `leer_en_voz` de la extension del navegador, desde la consola.
fn armar_leer(e: &Entrada) -> Result<Orden, Fallo> {
    let texto = texto_o(e, "texto").ok_or_else(|| Fallo::no_se_entiende("leer: falta el texto"))?;
    pedidos(vec![pedido(
        "leer_en_voz",
        json!({ "texto": texto, "titulo": e.campo("titulo") }),
    )])
}

fn armar_proyecto(e: &Entrada) -> Result<Orden, Fallo> {
    let id = e
        .sueltos
        .first()
        .cloned()
        .or_else(|| e.campo("id"))
        .or_else(|| e.campo("proyecto"));
    let id = id
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Fallo::no_se_entiende("proyecto: falta cual"))?;
    pedidos(vec![pedido(
        "abrir",
        json!({ "que": { "tipo": "proyecto", "proyecto": id } }),
    )])
}

fn armar_tarea(e: &Entrada) -> Result<Orden, Fallo> {
    let (texto, ficheros) = e.texto_y_ficheros();
    if texto.is_empty() && ficheros.is_empty() {
        return ventana("tareas");
    }
    if ficheros.iter().any(|f| !es_imagen(f)) {
        return Err(Fallo::no_se_entiende("una tarea solo lleva imagenes"));
    }
    let mut campos = json!({ "texto": texto, "codigo": e.campo("lista") });
    if e.campo("proyecto").is_some() {
        campos["proyecto"] = e.proyecto();
    }
    if !ficheros.is_empty() {
        campos["imagenes"] = Value::Array(ficheros.iter().map(|p| ruta_json(p)).collect());
    }
    pedidos(vec![pedido("anadir_tarea", campos)])
}

/// Texto e imagenes para `nota_nueva` y `leccion_nueva`.
fn texto_e_imagenes(e: &Entrada, que: &str) -> Result<(Value, Vec<Value>), Fallo> {
    let (texto, ficheros) = e.texto_y_ficheros();
    if let Some(f) = ficheros.iter().find(|f| !es_imagen(f)) {
        return Err(Fallo::no_se_entiende(format!(
            "{que} solo lleva imagenes, y «{}» no lo es",
            f.display()
        )));
    }
    let texto = if texto.is_empty() {
        Value::Null
    } else {
        Value::String(texto)
    };
    Ok((texto, ficheros.iter().map(|p| ruta_json(p)).collect()))
}

fn armar_nota(e: &Entrada) -> Result<Orden, Fallo> {
    let (texto, imagenes) = texto_e_imagenes(e, "una nota")?;
    let mut campos = json!({ "texto": texto, "proyecto": e.proyecto() });
    if !imagenes.is_empty() {
        campos["imagenes"] = Value::Array(imagenes);
    }
    pedidos(vec![pedido("nota_nueva", campos)])
}

fn armar_leccion(e: &Entrada) -> Result<Orden, Fallo> {
    let (texto, imagenes) = texto_e_imagenes(e, "una leccion")?;
    let mut campos = json!({ "texto": texto, "proyecto": e.proyecto() });
    if !imagenes.is_empty() {
        campos["imagenes"] = Value::Array(imagenes);
    }
    pedidos(vec![pedido("leccion_nueva", campos)])
}

/// El texto suelto, o el campo con nombre.
fn texto_o(e: &Entrada, campo: &str) -> Option<String> {
    let t = e.sueltos.join(" ").trim().to_string();
    if t.is_empty() {
        e.campo(campo)
    } else {
        Some(t)
    }
}

fn armar_lienzo(e: &Entrada) -> Result<Orden, Fallo> {
    pedidos(vec![pedido(
        "lienzo_nuevo",
        json!({ "nombre": texto_o(e, "nombre"), "proyecto": e.proyecto() }),
    )])
}

/// `al-lienzo <nombre> [texto] [imagen…]`: el nombre es el primer suelto (o
/// `--lienzo`); lo demas, como en `chat`.
fn armar_al_lienzo(e: &Entrada) -> Result<Orden, Fallo> {
    let (nombre, resto) = match e.campo("lienzo") {
        Some(n) => (n, e.sueltos.clone()),
        None => match e.sueltos.split_first() {
            Some((n, r)) => (n.trim().to_string(), r.to_vec()),
            None => {
                return Err(Fallo::no_se_entiende(
                    "al-lienzo: falta el nombre del lienzo",
                ));
            }
        },
    };
    if nombre.is_empty() {
        return Err(Fallo::no_se_entiende(
            "al-lienzo: falta el nombre del lienzo",
        ));
    }
    let sin_nombre = Entrada {
        sueltos: resto,
        campos: e.campos.clone(),
        es_fichero: e.es_fichero,
        enlace: e.enlace,
    };
    let (texto, ficheros) = sin_nombre.texto_y_ficheros();
    if texto.is_empty() && ficheros.is_empty() {
        return Err(Fallo::no_se_entiende(
            "al-lienzo: falta el texto o alguna imagen",
        ));
    }
    if let Some(f) = ficheros.iter().find(|f| !es_imagen(f)) {
        return Err(Fallo::no_se_entiende(format!(
            "al-lienzo: un lienzo solo recibe imagenes y texto ({})",
            f.display()
        )));
    }
    let imagenes: Vec<Value> = ficheros.iter().map(|p| ruta_json(p)).collect();
    let mut campos = json!({ "lienzo": nombre, "texto": texto, "proyecto": e.proyecto() });
    if !imagenes.is_empty() {
        campos["imagenes"] = Value::Array(imagenes);
    }
    pedidos(vec![pedido("al_lienzo", campos)])
}

fn armar_lista(e: &Entrada) -> Result<Orden, Fallo> {
    // «Tareas», como `Atajos.LISTA` de Android cuando no dice titulo.
    let titulo = texto_o(e, "titulo").unwrap_or_else(|| "Tareas".into());
    pedidos(vec![pedido(
        "lista_nueva",
        json!({ "titulo": titulo, "proyecto": e.proyecto() }),
    )])
}

fn armar_lecciones(e: &Entrada) -> Result<Orden, Fallo> {
    pedidos(vec![pedido(
        "lecciones",
        json!({ "consulta": texto_o(e, "consulta"), "proyecto": e.proyecto() }),
    )])
}

fn armar_grabar(e: &Entrada) -> Result<Orden, Fallo> {
    let nombre = texto_o(e, "nombre").unwrap_or_else(|| "Nota de voz".into());
    pedidos(vec![pedido(
        "grabar",
        json!({ "nombre": nombre, "proyecto": e.proyecto() }),
    )])
}

fn armar_soltar(e: &Entrada) -> Result<Orden, Fallo> {
    pedidos(vec![pedido("soltar", json!({ "proyecto": e.proyecto() }))])
}

fn armar_sincronizar(e: &Entrada) -> Result<Orden, Fallo> {
    // Sin aparato, la pantalla (lo que hace el atajo de Android y la Jump
    // List); con aparato, sincronizar ya.
    match e.sueltos.first().cloned().or_else(|| e.campo("aparato")) {
        Some(a) => pedidos(vec![pedido("sincronizar", json!({ "aparato": a }))]),
        None => ventana("sincronizar"),
    }
}

fn armar_al_movil(e: &Entrada) -> Result<Orden, Fallo> {
    let ficheros = e.solo_ficheros()?;
    if ficheros.is_empty() {
        return Err(Fallo::no_se_entiende("al-movil: faltan los ficheros"));
    }
    let (imagenes, archivos) = partir_imagenes(&ficheros);
    let aparato = e.campo("aparato").unwrap_or_else(|| "todos".into());
    pedidos(vec![pedido(
        "enviar_al_movil",
        json!({ "aparato": aparato, "imagenes": imagenes, "archivos": archivos }),
    )])
}

fn imagenes_solo(e: &Entrada, que: &str) -> Result<Vec<PathBuf>, Fallo> {
    let rutas = e.solo_ficheros()?;
    if rutas.is_empty() {
        return Err(Fallo::no_se_entiende(format!("{que}: faltan las imagenes")));
    }
    if let Some(r) = rutas.iter().find(|r| !es_imagen(r)) {
        return Err(Fallo {
            codigo: codigo::FALLO,
            motivo: format!("«{}» no es una imagen", r.display()),
        });
    }
    Ok(rutas)
}

fn armar_ocr(e: &Entrada) -> Result<Orden, Fallo> {
    Ok(Orden::Ocr {
        rutas: imagenes_solo(e, "ocr")?,
        copiar: e.bandera("copiar"),
    })
}

fn armar_copiar_texto(e: &Entrada) -> Result<Orden, Fallo> {
    // El pedido `ocr` de la extension del navegador: `ruta` y, si hay mas,
    // `rutas` con las demas (un solo texto al portapapeles).
    let rutas = imagenes_solo(e, "copiar-texto")?;
    let mut campos = json!({ "ruta": ruta_json(&rutas[0]) });
    if rutas.len() > 1 {
        campos["rutas"] = Value::Array(rutas[1..].iter().map(|p| ruta_json(p)).collect());
    }
    pedidos(vec![pedido("ocr", campos)])
}

fn armar_asistente(e: &Entrada) -> Result<Orden, Fallo> {
    let (texto, ficheros) = e.texto_y_ficheros();
    if texto.is_empty() && !ficheros.is_empty() {
        return Ok(Orden::Preguntar {
            rutas: ficheros,
            proyecto: e.campo("proyecto"),
        });
    }
    pedidos(vec![pedido_al_asistente(
        &texto,
        &ficheros,
        e.campo("proyecto"),
    )])
}

/// El pedido `asistente` de un texto y unos ficheros: las imagenes las ve
/// el modelo (`imagenes`); de lo demas se le dan las rutas, que sus
/// herramientas de documentos ya saben abrir. Publico porque lo arma tambien
/// quien pregunta en la caja ([`Orden::Preguntar`]).
pub fn pedido_al_asistente(texto: &str, ficheros: &[PathBuf], proyecto: Option<String>) -> Value {
    let (imagenes, otros) = partir_imagenes(ficheros);
    let mut texto = texto.trim().to_string();
    if !otros.is_empty() {
        texto.push_str("\n\nArchivos:");
        for o in &otros {
            texto.push_str(&format!("\n- {}", o.as_str().unwrap_or_default()));
        }
    }
    let mut campos = json!({ "texto": texto, "proyecto": proyecto });
    if !imagenes.is_empty() {
        campos["imagenes"] = Value::Array(imagenes);
    }
    pedido("asistente", campos)
}

fn armar_abrir(e: &Entrada) -> Result<Orden, Fallo> {
    let proyecto = e.proyecto();
    // Lo que no es un fichero: `?tipo=mensaje&codigo=…`, `?tipo=nota…`,
    // `?tipo=hoja&referencia=…`, `?tipo=proyecto&proyecto=…`.
    let tipo = e.campo("tipo").map(|t| t.to_lowercase());
    match tipo.as_deref() {
        Some("mensaje") | Some("nota") => {
            let codigo = e
                .campo("codigo")
                .ok_or_else(|| Fallo::no_se_entiende("abrir: falta codigo"))?;
            return pedidos(vec![pedido(
                "abrir",
                json!({ "que": { "tipo": tipo, "proyecto": proyecto, "codigo": codigo } }),
            )]);
        }
        Some("hoja") => {
            let r = e
                .campo("referencia")
                .ok_or_else(|| Fallo::no_se_entiende("abrir: falta referencia"))?;
            return pedidos(vec![pedido(
                "abrir",
                json!({ "que": { "tipo": "hoja", "proyecto": proyecto, "referencia": r } }),
            )]);
        }
        Some("proyecto") => {
            return pedidos(vec![pedido(
                "abrir",
                json!({ "que": { "tipo": "proyecto", "proyecto": proyecto } }),
            )]);
        }
        Some("fichero") | None => {}
        Some(otro) => {
            return Err(Fallo::no_se_entiende(format!(
                "abrir: no hay tipo «{otro}»"
            )));
        }
    }
    let rutas = e.solo_ficheros()?;
    if rutas.is_empty() {
        return Err(Fallo::no_se_entiende("abrir: falta el fichero"));
    }
    let pagina = match e.campo("pagina") {
        Some(p) => Some(
            p.parse::<u32>()
                .ok()
                .filter(|n| *n >= 1)
                .ok_or_else(|| Fallo::no_se_entiende("--pagina es un numero desde 1"))?,
        ),
        None => None,
    };
    let buscar = e.campo("buscar");
    pedidos(rutas.iter().map(|r| pedido("abrir", json!({ "que": { "tipo": "fichero", "ruta": ruta_json(r), "pagina": pagina, "buscar": buscar } }))).map(sin_nulos_en_que).collect())
}

/// Quita los `null` de dentro de `que` (el protocolo los da por ausentes,
/// pero un pedido limpio se lee mejor en el registro).
fn sin_nulos_en_que(mut p: Value) -> Value {
    if let Some(Value::Object(q)) = p.get_mut("que") {
        q.retain(|_, v| !v.is_null());
    }
    p
}

fn armar_ventana(e: &Entrada) -> Result<Orden, Fallo> {
    let cual = e
        .sueltos
        .first()
        .cloned()
        .or_else(|| e.campo("cual"))
        .unwrap_or_default()
        .to_lowercase();
    match cual.as_str() {
        "tareas" | "galeria" | "lecciones" | "sincronizar" => ventana(&cual),
        _ => Err(Fallo::no_se_entiende(
            "ventana: tareas, galeria, lecciones o sincronizar",
        )),
    }
}

fn armar_pedido(e: &Entrada) -> Result<Orden, Fallo> {
    let json = e
        .sueltos
        .first()
        .cloned()
        .or_else(|| e.campo("json"))
        .ok_or_else(|| Fallo::no_se_entiende("pedido: falta el JSON"))?;
    de_json(&json)
}

#[cfg(test)]
mod pruebas;
