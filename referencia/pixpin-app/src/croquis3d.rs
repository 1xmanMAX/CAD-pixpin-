//! **Ver un croquis 3D del movil** (tanda 5, 13-oct-2026): lo que PixPin
//! Android dibuja en el espacio (`croquis3d/<id>.croquis.gz`) llega al PC al
//! sincronizar, y hasta ahora no se sabia abrir. Se abre en la ventana del
//! visor 3D (`pixpin_cad::ventana3d`, la de los modelos BIM: girar, mover,
//! acercar, caja de seccion) desde tres sitios:
//!
//! - el chat: tocar la burbuja de un croquis ([`abrir_desde_chat`]);
//! - la hoja de un proyecto pedida por su referencia (`abrir_hoja.rs`);
//! - un `.pixpin` pineado, donde va como `croquis/<id>.json` (`pines.rs`).
//!
//! **Solo se ve**: el fichero se lee una vez y no se escribe nunca (editarlo
//! espera a la fusion trazo a trazo en `pixpin-sincro`). La lectura y la
//! malla van en un hilo aparte: la ventana sale en seguida con «Abriendo…».

use std::path::{Path, PathBuf};
use std::sync::Arc;

use pixpin_cad::croquis::{self, Croquis, malla::Foto};
use pixpin_proyecto::cuaderno::{Clase, Mensaje};
use pixpin_store::Idioma;

/// La clase de un mensaje con un croquis del movil (`Clase.CROQUIS`).
pub const CLASE: &str = "CROQUIS";

/// Si el mensaje es un croquis con su referencia.
pub fn es_croquis(m: &Mensaje) -> bool {
    m.clase == Some(Clase::Otra(CLASE.into()))
        && m.referencia
            .as_deref()
            .is_some_and(|r| !r.trim().is_empty())
}

/// Donde esta en este equipo el croquis de referencia `id` de un proyecto:
/// `croquis3d/<id>.croquis.gz`, donde lo deja la sincronizacion
/// (`android/croquis3d/` de la carpeta del chat). `None` si no esta.
pub fn fichero(raiz: &Path, ficha_id: &str, id: &str) -> Option<PathBuf> {
    let id = id.trim();
    if id.is_empty() || id.contains(['/', '\\']) || id.contains("..") {
        return None;
    }
    pixpin_proyecto::vista::ruta_real(
        raiz,
        ficha_id,
        &format!("pixpin:files/croquis3d/{id}.croquis.gz"),
    )
    .filter(|r| r.is_file())
}

/// El fichero del croquis de un mensaje del chat, si es un croquis y esta.
pub fn fichero_del_mensaje(raiz: &Path, ficha_id: &str, m: &Mensaje) -> Option<PathBuf> {
    if !es_croquis(m) {
        return None;
    }
    fichero(raiz, ficha_id, m.referencia.as_deref()?)
}

/// El idioma de Windows, para cuando quien abre no lo trae.
pub fn idioma_del_sistema() -> Idioma {
    pixpin_store::resolver_idioma(
        &pixpin_shell::entorno::locale_del_sistema(),
        pixpin_store::ajustes::PreferenciaIdioma::Sistema,
    )
}

/// **Toca una burbuja del chat**: si es un croquis que esta en este equipo,
/// lo abre. `false` si no lo es (que siga el camino de siempre).
pub fn abrir_desde_chat(idioma: Idioma, raiz: &Path, ficha_id: &str, m: &Mensaje) -> bool {
    let Some(ruta) = fichero_del_mensaje(raiz, ficha_id, m) else {
        return false;
    };
    let titulo = titulo_de(
        Some(if m.texto.trim().is_empty() {
            m.nombre.as_str()
        } else {
            m.texto.as_str()
        }),
        m.referencia.as_deref().unwrap_or(""),
    );
    abrir_fichero(
        idioma,
        raiz.to_path_buf(),
        ficha_id.to_string(),
        ruta,
        titulo,
    );
    true
}

fn titulo_de(texto: Option<&str>, id: &str) -> String {
    texto
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| t.lines().next().unwrap_or(t).to_string())
        .unwrap_or_else(|| format!("Croquis {id}"))
}

/// Abre el `.croquis.gz` de un proyecto. Sus fotos (`pixpin:files/…`) se
/// buscan como las de cualquier mensaje del proyecto.
pub fn abrir_fichero(
    idioma: Idioma,
    raiz: PathBuf,
    ficha_id: String,
    ruta: PathBuf,
    titulo: String,
) {
    // Su fichero y su chat van al «Exportar…»: Original es el
    // `.croquis.gz` y el `.pixpin`, el proyecto donde vive.
    let origen = Origen {
        ruta: ruta.clone(),
        proyecto: ficha_id.clone(),
    };
    let leer = move || -> Result<(Croquis, Box<dyn Fn(&str) -> Option<Foto> + Send>), String> {
        let bytes = std::fs::read(&ruta).map_err(|e| e.to_string())?;
        let c = croquis::leer(&bytes)?;
        let (raiz, ficha) = (raiz.clone(), ficha_id.clone());
        let fotos = move |r: &str| {
            foto_de_disco(pixpin_proyecto::vista::ruta_real(&raiz, &ficha, r)?.as_path())
        };
        Ok((c, Box::new(fotos)))
    };
    lanzar(idioma, titulo, Some(origen), Box::new(leer));
}

/// De donde se abrio un croquis: su `.croquis.gz` y la ficha de su chat.
/// Uno de dentro de un `.pixpin` no tiene (no esta en el disco).
#[derive(Debug, Clone)]
struct Origen {
    ruta: PathBuf,
    proyecto: String,
}

/// Abre el croquis de dentro de un `.pixpin` (`croquis/<id>.json`). Sus
/// fotos, si van en el paquete, se sacan de el.
pub fn abrir_de_paquete(
    idioma: Idioma,
    paquete: &pixpin_proyecto::Paquete,
    id: &str,
    titulo: &str,
) -> bool {
    let Some(bytes) = paquete
        .entrada(&format!("croquis/{id}.json"))
        .map(<[u8]>::to_vec)
    else {
        return false;
    };
    // Lo que podria ser una foto del croquis, copiado para el hilo.
    let entradas: Arc<Vec<(String, Vec<u8>)>> = Arc::new(
        paquete
            .nombres()
            .filter(|n| {
                n.contains("croquis") || n.starts_with("imagenes/") || n.starts_with("archivos/")
            })
            .filter(|n| !n.ends_with(".json"))
            .filter_map(|n| Some((n.to_string(), paquete.entrada(n)?.to_vec())))
            .collect(),
    );
    let leer = move || -> Result<(Croquis, Box<dyn Fn(&str) -> Option<Foto> + Send>), String> {
        let c = croquis::leer(&bytes)?;
        let e = entradas.clone();
        let fotos = move |r: &str| {
            let rel = r.strip_prefix("pixpin:files/").unwrap_or(r);
            let nombre = rel.rsplit('/').next().unwrap_or(rel);
            let (_, b) = e
                .iter()
                .find(|(n, _)| n == rel || n.rsplit('/').next() == Some(nombre))?;
            foto_de_bytes(b)
        };
        Ok((c, Box::new(fotos)))
    };
    lanzar(idioma, titulo.to_string(), None, Box::new(leer));
    true
}

fn foto_de_disco(ruta: &Path) -> Option<Foto> {
    let im = pixpin_codec::wic::cargar(ruta).ok()?;
    Some(Foto {
        ancho: im.ancho,
        alto: im.alto,
        rgba: im.pixeles,
    })
}

fn foto_de_bytes(b: &[u8]) -> Option<Foto> {
    // WIC lee de un fichero: se pasa por uno temporal.
    let t = std::env::temp_dir().join(format!(
        "pixpin-croquis-foto-{}-{:x}",
        std::process::id(),
        b.len()
    ));
    std::fs::write(&t, b).ok()?;
    let f = foto_de_disco(&t);
    let _ = std::fs::remove_file(&t);
    f
}

type Lector =
    Box<dyn FnOnce() -> Result<(Croquis, Box<dyn Fn(&str) -> Option<Foto> + Send>), String> + Send>;

/// La ventana, en su hilo; lo pesado (leer y hacer la malla), en otro.
fn lanzar(idioma: Idioma, titulo: String, origen: Option<Origen>, leer: Lector) {
    let lanzado = std::thread::Builder::new()
        .name("croquis-3d".into())
        .spawn(move || {
            // Lo que la ventana necesita saber antes que la malla: se lee aqui
            // (es un JSON, rapido) y la malla se hace aparte.
            let (c, fotos) = match leer() {
                Ok(x) => x,
                Err(e) => {
                    tracing::warn!(%e, "no se pudo leer el croquis");
                    let (tx, rx) = std::sync::mpsc::channel();
                    let _ = tx.send(Err(e));
                    let _ = pixpin_cad::ventana3d::ver(&titulo, rx, textos(idioma));
                    return;
                }
            };
            let mut extras = extras_de(&c);
            let (ruta, proyecto) = origen.map(|o| (o.ruta, o.proyecto)).unzip();
            extras.exportar = Some(crate::compartir::exportable::al_exportar_3d(
                idioma,
                titulo.clone(),
                ruta,
                proyecto,
            ));
            let (tx, rx) = std::sync::mpsc::channel();
            let _ = std::thread::Builder::new()
                .name("croquis-3d-malla".into())
                .spawn(move || {
                    let t = std::time::Instant::now();
                    let m = croquis::malla::a_modelo(&c, &*fotos);
                    tracing::info!(
                        ms = t.elapsed().as_millis() as u64,
                        trazos = c.trazos.len(),
                        vertices = m.vertices.len(),
                        "croquis 3D listo"
                    );
                    let _ = tx.send(Ok(m));
                });
            if let Err(e) = pixpin_cad::ventana3d::ver_con(&titulo, rx, textos(idioma), extras) {
                tracing::warn!(%e, "no se pudo abrir el visor del croquis");
            }
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el visor del croquis");
    }
}

/// Lo que el croquis le dice a la ventana: su fondo, el tema, que encuadrar,
/// desde donde mira de fabrica (`Camara3D()`) y sus vistas.
pub fn extras_de(c: &Croquis) -> pixpin_cad::ventana3d::Extras {
    let encuadre = croquis::malla::caja_de_lo_dibujado(c, [0.0; 3]).map(|k| k.map(f64::from));
    let fabrica = croquis::Camara3D::default();
    pixpin_cad::ventana3d::Extras {
        fondo: croquis::malla::fondo_propio(c),
        claro: Some(croquis::malla::abre_claro(c)),
        encuadre,
        orientacion: Some((
            -fabrica.giro - std::f64::consts::FRAC_PI_2,
            fabrica.inclinacion,
        )),
        vistas: c.vistas.clone(),
        exportar: None,
    }
}

fn textos(idioma: Idioma) -> pixpin_cad::ventana3d::TextosUi3d {
    use pixpin_cad::ventana3d::TextosUi3d;
    match idioma {
        Idioma::Ingles => TextosUi3d {
            abriendo: "Opening the sketch…".into(),
            vacio: "The sketch is empty".into(),
            error: "Could not open the sketch".into(),
            todo: "Show whole sketch\tF".into(),
            tema: "Light or dark background\tB".into(),
            corte: "Section box\tC".into(),
            aislar: "Isolate the selected element\tI".into(),
            panel: "Layers\tL".into(),
            niveles: "GROUPS".into(),
            categorias: "KINDS".into(),
            sin_nivel: "Everything".into(),
            mostrar_todo: "Show all".into(),
            aristas: "Edges\tE".into(),
            planta: "Plan view\tP".into(),
            encima: "Always on top\tT".into(),
            cerrar: "Close\tEsc".into(),
            altura_corte: "Section".into(),
            pista: "Drag: orbit · Right drag: pan · Wheel: zoom · 1–9: saved views".into(),
        },
        _ => TextosUi3d {
            abriendo: "Abriendo el croquis…".into(),
            vacio: "El croquis está vacío".into(),
            error: "No se pudo abrir el croquis".into(),
            todo: "Ver todo el croquis\tF".into(),
            panel: "Capas\tL".into(),
            niveles: "GRUPOS".into(),
            categorias: "TIPOS".into(),
            sin_nivel: "Todo".into(),
            pista: "Arrastrar: girar · Derecho: mover · Rueda: acercar · 1–9: vistas guardadas"
                .into(),
            ..TextosUi3d::default()
        },
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn mensaje(clase: &str, referencia: Option<&str>) -> Mensaje {
        Mensaje {
            id: "m1".into(),
            clase: Some(Clase::Otra(clase.into())),
            referencia: referencia.map(String::from),
            ..Default::default()
        }
    }

    #[test]
    fn un_croquis_se_reconoce_por_su_clase_y_su_referencia() {
        assert!(es_croquis(&mensaje("CROQUIS", Some("cr-1"))));
        // Casos negativos: sin referencia, o de otra clase, no es un croquis.
        assert!(!es_croquis(&mensaje("CROQUIS", None)));
        assert!(!es_croquis(&mensaje("CROQUIS", Some("  "))));
        assert!(!es_croquis(&mensaje("TABLA", Some("cr-1"))));
        assert!(!es_croquis(&Mensaje {
            clase: Some(Clase::Dibujo),
            referencia: Some("cr-1".into()),
            ..Default::default()
        }));
    }

    #[test]
    fn el_fichero_se_busca_donde_lo_deja_la_sincronizacion() {
        let raiz = std::env::temp_dir().join(format!("pixpin-croquis3d-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let m = mensaje("CROQUIS", Some("cr-1"));
        // Todavia no ha llegado: no se abre.
        assert!(fichero_del_mensaje(&raiz, "p1", &m).is_none());
        let donde = pixpin_proyecto::vista::ruta_real(
            &raiz,
            "p1",
            "pixpin:files/croquis3d/cr-1.croquis.gz",
        )
        .unwrap();
        std::fs::create_dir_all(donde.parent().unwrap()).unwrap();
        std::fs::write(&donde, b"{}").unwrap();
        assert_eq!(fichero_del_mensaje(&raiz, "p1", &m), Some(donde.clone()));
        // Una referencia que intenta salirse de la carpeta no se sigue.
        assert!(fichero(&raiz, "p1", "../../x").is_none());
        assert!(fichero(&raiz, "p1", "a/b").is_none());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn las_vistas_y_el_fondo_pasan_a_la_ventana() {
        let c = croquis::leer(
            br##"{"colorDelFondo":"#202830","trazos":[{"id":"t","puntos":[{"x":1,"y":2,"z":3},{"x":5,"y":2,"z":3}],"color":"#ffffff","grosor":4}],
                 "vistas":[{"id":"v","nombre":"Frente","camara":{"giro":0.0,"inclinacion":0.0}}]}"##,
        )
        .unwrap();
        let x = extras_de(&c);
        assert_eq!(x.vistas.len(), 1);
        assert_eq!(x.claro, Some(false));
        assert!(x.fondo.is_some());
        assert_eq!(x.encuadre, Some([1.0, 2.0, 3.0, 5.0, 2.0, 3.0]));
        // Sin nada dibujado no hay que encuadrar.
        assert!(extras_de(&croquis::leer(b"{}").unwrap()).encuadre.is_none());
    }

    #[test]
    fn el_titulo_es_el_texto_del_mensaje_o_el_codigo() {
        assert_eq!(titulo_de(Some("Escalera\notra"), "cr"), "Escalera");
        assert_eq!(titulo_de(Some("  "), "cr"), "Croquis cr");
        assert_eq!(titulo_de(None, "cr"), "Croquis cr");
    }
}
