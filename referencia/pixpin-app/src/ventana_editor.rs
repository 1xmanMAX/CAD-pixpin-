//! La ventana del editor avanzado.
//!
//! **Traduce y nada mas.** Convierte `EventoOverlay` en `EventoGesto`, llama
//! a la maquina del motor, y hace lo que devuelve: redibujar y poner el
//! cursor que toque. Si este fichero crece mas alla de eso, es que se ha
//! colado logica que deberia estar en el motor.
//!
//! No hace falta fontaneria nueva: `VentanaOverlay` ya sirve ventanas
//! completas —el editor de grabaciones (`editor.rs`) la usa asi— y ya trae
//! raton, teclas con sus modificadores, caracteres con IME, cambio de DPI,
//! cursores y el bucle por eventos que da el 0 % de CPU en reposo.
//!
//! # Por que aqui no hay ni un `unsafe`
//!
//! Leer si Shift o Alt estan pulsados AHORA (no en el ultimo evento de
//! teclado, que es otra cosa) hace falta para el raton, que no trae
//! modificadores consigo. `overlay.rs` lo resuelve con `GetKeyState`, pero
//! esa llamada es `unsafe` y este crate es `forbid(unsafe_code)`. La
//! envoltura seria ya existe en `pixpin_shell::entrada::modificadores`, que
//! usa el anotador para lo mismo: se reutiliza en vez de duplicar el sondeo.
//!
//! # Como se traduce `Orden` a dibujo
//!
//! El motor entrega `pixpin_motor2d::pintado::Orden`: geometria ya calculada,
//! sin Direct2D de por medio (`pintado.rs` lo explica: "las mismas ordenes
//! valen para Direct2D hoy y para exportar a SVG manana"). `pixpin-render` no
//! conoce ese tipo a proposito —ninguna dependencia nueva entre crates (D28)—
//! asi que el `match` de `Orden` a llamadas de `Pintor` vive aqui, en
//! `dibujar_orden`. No es geometria: es la misma clase de traduccion mecanica
//! que `a_evento`, solo que de salida en vez de entrada.

use crate::fondo_lienzo::{FondoLienzo, encuadre_inicial};
use crate::imagenes_lienzo::ImagenesLienzo;
// Lo que era de este fichero y ahora es del nucleo comun de las
// herramientas (`dibujo`): se importa con el mismo nombre para que el bucle
// y las pruebas de aqui sigan leyendose igual.
use crate::dibujo::construir;
use crate::dibujo::pintar::{
    a_color, dibujar_orden, pintar_copia_predicha, por_cada_orden, punta_de_tinta,
};
#[cfg(test)]
use crate::dibujo::teclas::{
    CambioPluma, aplicar_orden, pulsar_boton, tecla_a_herramienta, tecla_a_pluma, tecla_del_motor,
};
use crate::dibujo::teclas::{a_evento, elegir_herramienta, forma_de, tecla_de};
use crate::navegacion::{self, vista_efectiva};
use anyhow::{Context, Result};
use pixpin_geom::Punto;
use pixpin_motor2d::cache::Cache;
use pixpin_motor2d::camara::Camara;
use pixpin_motor2d::escena::Escena;
#[cfg(test)]
use pixpin_motor2d::gesto::EventoGesto;
#[cfg(test)]
use pixpin_motor2d::gesto::FormaCursor;
use pixpin_motor2d::gesto::{Gesto, Herramienta, Peticion, Region};
use pixpin_motor2d::indice::Rejilla;
use pixpin_motor2d::pintado::Orden;
#[cfg(test)]
use pixpin_motor2d::seleccion::{OrdenEditor, Tecla};
use pixpin_motor2d::vector::Punto2;
use pixpin_motor2d::{Elemento, Figura};
use pixpin_render::{CapaEstatica, Color, Estampa, MotorRender, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, FormaCursorWin, VentanaOverlay};
#[cfg(test)]
use pixpin_ui::BOTONES_EDITOR;
use pixpin_ui::{BotonCaja, CajaHerramientas, DestinoClic};

/// Cuanto se adelanta la punta del trazo en curso. La medida contra la
/// posicion LOGICA del cursor daba 28 ms como adelantado y se bajo a 14, pero
/// lo que se ve es el cursor que pinta Windows, que tambien llega tarde: con
/// 14 el usuario veia la tinta detras, y con 28 «casi cero lag». Manda la vista.
const HORIZONTE_PREDICCION_MS: f32 = 28.0;
/// Hasta donde puede adelantarse, en pixeles de pantalla: con 48 un trazo
/// rapido topaba y la punta volvia a quedarse atras.
const TOPE_PREDICCION_PX: f32 = 80.0;
/// Lo mas que se espera a la senal de fotograma antes de pintar igualmente.
/// A3: pixeles de colchon por lado de la superficie de la escena.
///
/// Es lo que se puede desplazar el lienzo moviendo el visual, sin repintar
/// nada. 256 px es una tesela de las que usan Chromium y MyPaint: a 30 px
/// por fotograma (un arrastre rapido, 1.800 px/s) son nueve fotogramas
/// gratis por cada repintado, y a 3000x2000 cuesta 22 MB mas de memoria de
/// video (35 MB frente a 24 por buffer). Subirlo da mas fotogramas gratis y
/// cuesta mas memoria; bajarlo, al reves.
pub(crate) const MARGEN_ESCENA: u32 = 256;

const ESPERA_MAXIMA_SENAL_MS: u32 = 20;
const ESPERA_MAXIMA_SENAL: std::time::Duration =
    std::time::Duration::from_millis(ESPERA_MAXIMA_SENAL_MS as u64);

/// La seccion `[tinta]` del TOML, fijada al arrancar (ver
/// `fijar_ajustes_tinta`). Sin fijar, lo de siempre.
static AJUSTES_TINTA: std::sync::OnceLock<pixpin_store::Tinta> = std::sync::OnceLock::new();

/// La llama `main` una sola vez, antes de que exista ningun editor.
///
/// Es un `OnceLock` y no un parametro porque el editor se abre desde cinco
/// sitios que no tienen nada que ver entre si y a ninguno le importa como se
/// siente el lapiz. Llamarla dos veces no hace nada: el ajuste no cambia
/// mientras el programa viva, y asi las pruebas pueden abrir un editor sin
/// haberla llamado.
pub fn fijar_ajustes_tinta(t: pixpin_store::Tinta) {
    let _ = AJUSTES_TINTA.set(t);
}

fn ajustes_tinta() -> pixpin_store::Tinta {
    AJUSTES_TINTA.get().copied().unwrap_or_default()
}

/// Lo mismo para los otros anfitriones de las herramientas (`dibujo::mano`):
/// el lapiz se siente igual en el lector o en la pantalla que aqui.
pub(crate) fn ajustes_tinta_de_la_app() -> pixpin_store::Tinta {
    ajustes_tinta()
}

/// La seccion `[rendimiento]` del TOML, leida una vez por sesion.
static AJUSTES_RENDIMIENTO: std::sync::OnceLock<pixpin_store::Rendimiento> =
    std::sync::OnceLock::new();

/// Las dos palancas de esta tarea: `[rendimiento] ritmo` y
/// `[rendimiento] paneo_por_composicion`.
///
/// Se lee el fichero aqui, y no se recibe por parametro como
/// `medir_fotogramas`, porque el editor se abre desde cinco sitios (bandeja,
/// chat, pines, universo, grabacion) y anadirle un parametro a todos por un
/// ajuste de dibujo obligaria a tocar ficheros que en esta tanda son de
/// otros agentes. Se lee UNA vez por sesion: es un `OnceLock`.
fn ajustes_rendimiento() -> pixpin_store::Rendimiento {
    *AJUSTES_RENDIMIENTO.get_or_init(|| {
        let dir_exe = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
            .unwrap_or_default();
        let appdata = std::env::var_os("APPDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_default();
        pixpin_store::cargar(&pixpin_store::resolver(&dir_exe, &appdata))
            .unwrap_or_default()
            .rendimiento
    })
}

/// El `Gesto` con el que arranca el editor, ya con los ajustes de iman
/// guardados puestos.
///
/// Esta aparte de `abrir` porque `abrir` necesita GPU y una ventana, y este
/// cableado -que la pestana de dibujo no sea decorativa- se puede comprobar
/// sin nada de eso. Sin esta funcion la unica prueba posible seria
/// asignarle el campo a mano y releerlo, que no comprueba nada.
fn gesto_inicial(ajustes_iman: pixpin_motor2d::enganche::Ajustes) -> Gesto {
    let mut gesto = Gesto::nuevo();
    gesto.enganche = ajustes_iman;
    // **El iman fino**: los cruces, el canto (puntos sobre la circunferencia)
    // y las tangentes, para las laminas de geometria. El gesto los trae
    // apagados porque cuestan memoria en cada movimiento del raton; el
    // editor, que es donde se dibuja de verdad, los paga a sabiendas. Se
    // apagan con el iman.
    if ajustes_iman.activo {
        gesto.finos = pixpin_motor2d::enganche::AjustesFinos::default();
    }
    gesto
}

/// Lo que NO entra en la capa congelada: lo que cambia en cada fotograma.
/// Una sola funcion para `abrir` y `pintar`: si cada una calculara su lista,
/// la capa se daria por invalida en cada fotograma (la `Estampa` no casaria)
/// o, peor, valdria sin contener lo que hay que pintar encima.
/// Si `id` es de lo excluido de la capa congelada: la misma pregunta que
/// `excluidos_de(gesto).contains(&id)`, pero sin recorrer una lista. El
/// pintado la hace por cada elemento visible en cada fotograma, y con mil
/// elegidos eran millones de comparaciones.
fn es_excluido(gesto: &Gesto, id: u64) -> bool {
    match gesto.elemento_en_curso() {
        Some((en_curso, _)) => en_curso == id,
        // Las flechas atadas a lo que se mueve cambian de forma en cada
        // aviso igual que lo elegido: horneadas en la capa congelada se
        // verian quietas en su sitio viejo debajo de las que se mueven.
        None => se_arrastra(gesto, id) || gesto.sigue_la_flecha(id),
    }
}

/// **Lo que se mueve con el raton**: lo elegido y, si se arrastra un marco,
/// lo que encerraba al cogerlo (`Gesto::lleva_el_marco`). Sin lo segundo,
/// lo de dentro de un marco se quedaba quieto en la capa congelada (o en la
/// escena, con el marco en la capa de la tinta) hasta soltar, y daba un
/// salto: «al poner los frames pasa lo mismo» (28-sep-2026).
fn se_arrastra(gesto: &Gesto, id: u64) -> bool {
    gesto.seleccion.contiene(id) || gesto.lleva_el_marco(id)
}

fn excluidos_de(gesto: &Gesto) -> Vec<u64> {
    // Todo lo que se esta dibujando, no solo el trazo a mano: una figura en
    // curso fuera de la capa congelada obligaba a repintar la escena entera
    // en cada fotograma.
    match gesto.elemento_en_curso() {
        Some((id, _)) => vec![id],
        None => {
            let mut ids = gesto.seleccion.ids().to_vec();
            ids.extend_from_slice(gesto.lo_que_lleva_el_marco());
            ids.extend_from_slice(gesto.flechas_que_siguen());
            ids
        }
    }
}

/// Lo que decide `decidir_zoom` para esta vuelta del bucle.
#[derive(Debug, Clone, Copy, PartialEq)]
struct DecisionZoom {
    /// El zoom lleva `retardo` QUIETO (no solo "ha pasado `retardo` desde el
    /// primer cambio", D122): hay que rehacer la tinta nitida a esta escala.
    rehacer: bool,
    /// El zoom que se vio esta vuelta, para comparar con el de la siguiente.
    visto: f32,
    /// Cuando empezo a estar quieto en el valor `visto` (si sigue pendiente
    /// de rehacer). `None` significa "nada pendiente": ni hay que
    /// programar un tope de espera, ni girar en vacio.
    desde: Option<std::time::Instant>,
    /// Cuanto darle a `esperar_eventos` para no dormir mas de lo que falta
    /// hasta que el zoom cumpla el retardo. `None` si no hay nada pendiente.
    tope_ms: Option<u32>,
}

/// Pura, sin `&mut` ni reloj propio (recibe `ahora`): decide si hay que
/// rehacer la tinta nitida y cuanto dormir como mucho, a partir del zoom de
/// este fotograma y de lo que se sabia hasta ahora.
///
/// Dos fallos que esto corrige de la primera version (revision de la Tarea
/// 10):
///
/// 1. Girar en vacio: si el zoom vuelve a `escala` (la ya realizada) ANTES
///    de que se cumpla el retardo, aqui se limpia `desde` a `None`. Sin
///    esto, `tope_ms` se quedaria fijo en el resultado de restar un
///    instante ya pasado -0 ms para siempre-, y `esperar_eventos(Some(0))`
///    gira un nucleo entero en reposo: justo lo que el bucle dirigido por
///    eventos existe para evitar.
/// 2. "Quieto" no es "desde el primer cambio": si el zoom sigue cambiando
///    (rueda del raton sin soltar), cada cambio REINICIA `desde`. Sin esto,
///    un zoom continuo rehacia la tinta cada `retardo`, en vez de una vez
///    sola cuando por fin se para (D122: el `shouldCacheIgnoreZoom` de
///    Excalidraw mide quietud, no antiguedad del primer cambio).
fn decidir_zoom(
    zoom: f32,
    visto: f32,
    escala: f32,
    desde: Option<std::time::Instant>,
    ahora: std::time::Instant,
    retardo: std::time::Duration,
) -> DecisionZoom {
    let (visto, desde) = if zoom != visto {
        (zoom, Some(ahora))
    } else {
        (visto, desde)
    };
    if zoom == escala {
        // Ya esta a la escala realizada: nada pendiente.
        return DecisionZoom {
            rehacer: false,
            visto,
            desde: None,
            tope_ms: None,
        };
    }
    match desde {
        Some(d) if ahora.duration_since(d) >= retardo => DecisionZoom {
            rehacer: true,
            visto,
            desde: None,
            tope_ms: None,
        },
        Some(d) => DecisionZoom {
            rehacer: false,
            visto,
            desde: Some(d),
            tope_ms: Some(retardo.saturating_sub(ahora.duration_since(d)).as_millis() as u32),
        },
        // No deberia pasar en la practica (si `zoom != escala` y no se
        // acaba de mover, `desde` ya deberia venir puesto), pero si pasa no
        // hay que girar en vacio: se arranca el reloj ahora.
        None => DecisionZoom {
            rehacer: false,
            visto,
            desde: Some(ahora),
            tope_ms: Some(retardo.as_millis() as u32),
        },
    }
}

/// Cuanto tiene que estar quieta la camara para que se pinte el fotograma
/// nitido. Es el mismo reposo que usa QuickView y el que ya tenia el pin
/// para su zoom (D89): mas corto se pinta a mitad de gesto y no se gana
/// nada, mas largo se nota que la imagen esta estirada.
const REPOSO_CAMARA: std::time::Duration = std::time::Duration::from_millis(150);

/// Cuanto se ve el aviso de una zona vinculada que no lleva a nada: lo de un
/// `Toast.LENGTH_SHORT` del movil y un poco mas, que aqui no se tapa solo.
const AVISO_ENLACE_DURA: std::time::Duration = std::time::Duration::from_millis(3000);

/// Lo mas que se deja estirar la escena antes de exigir un repintado. Por
/// encima de esto, la textura ampliada se ve claramente borrosa.
const ESTIRADO_MAXIMO: f32 = 3.0;

/// **B2 encendido otra vez (2026-09-22): el trazo en curso en su propia capa.**
///
/// Se apago porque la prueba
/// `superficie::pruebas::una_ventana_compuesta_ensena_la_interfaz_encima_de_la_escena`
/// «demostraba» que el visual de la tinta no se veia. Era la prueba: su
/// proceso no era consciente de DPI, Windows estiraba la ventana al 150 % y
/// las capas (que miden pixeles FISICOS) solo cubrian los dos tercios de
/// arriba; la banda de la tinta caia debajo. Una banda pintada a la misma
/// altura en la capa de la INTERFAZ tampoco se veia, que es lo que lo
/// delato. La aplicacion es consciente de DPI y no tiene ese problema; la
/// prueba ya lo es y ahora comprueba que la tinta se ve y que
/// `desplazar_tinta` la mueve. `PIXPIN_TINTA_CLASICA` y
/// `[rendimiento] paneo_por_composicion = false` siguen apagandolo.
const TINTA_EN_CAPA: bool = true;

/// Cada cuanto compone DWM cuando no lo quiere decir (con la ventana tapada,
/// por ejemplo): 60 Hz, que es lo que hay en el equipo suelo.
const PERIODO_SUPUESTO_MS: f32 = 16.7;

/// Parte del periodo de composicion que tiene que haber pasado desde el
/// ultimo fotograma de tinta para pintar otro.
///
/// No es 1,0 porque el reloj del bucle y el de DWM no son el mismo: exigir
/// el periodo entero haria perder una composicion de cada dos en cuanto
/// hubiera medio milisegundo de deriva, y el trazo saldria a 30 Hz.
const PARTE_DEL_PERIODO: f32 = 0.9;

/// **B2.** Cuanto falta para que toque el siguiente fotograma de la capa de
/// tinta, en milisegundos enteros. `None` = ya toca.
///
/// Hace falta porque un fotograma de tinta no presenta: la senal de latencia
/// de la cadena de intercambio, que es la que marca el ritmo del resto del
/// bucle, se queda disparada y no frena nada. Sin este freno, un lapiz que
/// manda 240 muestras por segundo haria cuatro `Commit` por cada composicion
/// de DWM, y tres de los cuatro no llegarian a verse.
fn espera_de_tinta(desde_ms: f32, periodo_ms: f32) -> Option<u32> {
    if !periodo_ms.is_finite() || periodo_ms <= 0.0 {
        return None;
    }
    let plazo = periodo_ms * PARTE_DEL_PERIODO;
    if !desde_ms.is_finite() || desde_ms >= plazo {
        return None;
    }
    Some((plazo - desde_ms).ceil().max(1.0) as u32)
}

/// La punta predicha de este fotograma, si hay algo dibujandose y la
/// prediccion no esta apagada. Horizonte medido (~30 ms de la lectura a la
/// pantalla en el equipo del usuario) y tope de 80 px de pantalla.
fn prediccion_de(
    gesto: &Gesto,
    predictor: &mut pixpin_motor2d::tinta::prediccion::Predictor,
    tinta_clasica: bool,
    zoom: f32,
) -> Option<Punto2> {
    gesto
        .elemento_en_curso()
        .filter(|_| !tinta_clasica)
        .and_then(|_| predictor.predecir(HORIZONTE_PREDICCION_MS, TOPE_PREDICCION_PX / zoom, zoom))
}

/// El tamano de la superficie de la escena: la ventana mas el colchon por
/// cada lado. Es la unica formula: la usan la `Estampa` de la capa congelada
/// y el propio `pintar`, y si se separaran, la capa se daria por valida con
/// un tamano que no es el suyo.
fn tamano_escena(ancho_px: f32, alto_px: f32, margen: f32) -> (u32, u32) {
    (
        (ancho_px + margen * 2.0) as u32,
        (alto_px + margen * 2.0) as u32,
    )
}

/// **A3.** La transformada que hay que ponerle al visual de la escena para
/// que lo pintado con la camara `pintada` se vea como si estuviera pintado
/// con la camara `ahora`. `None` si el colchon no da y hay que repintar.
///
/// Esta es toda la idea de A3: un paneo deja de costar «recorrer la escena y
/// emitir sus primitivas» —15 ms con 2.000 elementos, 83 ms con 10.000,
/// medido— y pasa a costar una matriz y un `Commit`. Lo que asoma por los
/// bordes sale del colchon de `margen` pixeles que la superficie tiene de
/// mas por cada lado.
///
/// Devuelve `(escala, dx, dy)` tal como los quiere `Superficie::estirar`.
///
/// Es pura a proposito: la condicion de «¿cabe?» es aritmetica y se prueba
/// sin GPU, que es justo lo que no se puede improvisar mirando la pantalla.
pub(crate) fn transformada_de_camara(
    pintada: &Camara,
    ahora: &Camara,
    ancho_px: f32,
    alto_px: f32,
    margen: f32,
) -> Option<(f32, f32, f32)> {
    if margen <= 0.0 || pintada.zoom <= 0.0 || ahora.zoom <= 0.0 {
        return None;
    }
    let s = ahora.zoom / pintada.zoom;
    if !s.is_finite() || s > ESTIRADO_MAXIMO {
        return None;
    }
    // Un punto del mundo que se pinto en la superficie en `u` tiene que
    // acabar en `(u − margen)·s + (pintada − ahora)·zoom_nuevo`.
    let ex = (pintada.x - ahora.x) * ahora.zoom;
    let ey = (pintada.y - ahora.y) * ahora.zoom;
    if !ex.is_finite() || !ey.is_finite() {
        return None;
    }
    // ¿Sigue cubierta la ventana entera por lo que de verdad se pinto? El
    // borde izquierdo de la ventana cae en la superficie en `margen − e/s`,
    // y el derecho en `(ancho − e)/s + margen`: los dos tienen que quedar
    // dentro de los `ancho + 2·margen` pixeles pintados.
    let cabe = |e: f32, largo: f32| e <= margen * s && e >= largo - s * (largo + margen);
    if cabe(ex, ancho_px) && cabe(ey, alto_px) {
        Some((s, ex, ey))
    } else {
        None
    }
}

/// Abre el editor y no vuelve hasta que se cierra la ventana.
///
/// Devuelve la escena tal como quedo, compactada: los elementos borrados de
/// verdad (los que un `Ctrl+Z` ya no puede traer de vuelta) se sueltan aqui,
/// no en cada paso del historial.
///
/// `ajustes_iman` es solo el trozo de los ajustes de la aplicacion que este
/// editor necesita conocer: el editor no tiene por que saber de atajos ni de
/// la carpeta de capturas, asi que quien llama pasa `config.enganche`, no
/// los `Ajustes` enteros.
///
/// `nivel` decide cuanto tiene que estar quieto el zoom antes de rehacer la
/// tinta nitida (`retardo_nitido`, D122): en `Ligero` el retardo es mayor
/// porque teselar cuesta mas en ese equipo.
///
/// `medir_fotogramas` enciende el registro de D129: una linea de `tracing`
/// cada 60 fotogramas con cuanto se tarda en vaciar la cola, pintar,
/// presentar y esperar.
///
/// `fondo` es la imagen del pin, fija en el mundo en (0,0)-(ancho, alto)
/// (D132); la bandeja pasa `None` (D137).
/// `fotos` son las imagenes que el dibujo ya trae —las del `.pixpin` del
/// movil—, cada una con el `id_objeto` que lleva su figura. Sin ellas, una
/// hoja que es una foto con trazos encima sale en blanco.
pub fn abrir(
    escena: Escena,
    ajustes_iman: pixpin_motor2d::enganche::Ajustes,
    nivel: pixpin_nivel::Nivel,
    medir_fotogramas: bool,
    fondo: Option<pixpin_codec::ImagenRgba>,
    fotos: &[(u64, std::path::PathBuf)],
) -> Result<(Escena, Option<String>)> {
    abrir_con_pegadas(
        escena,
        ajustes_iman,
        nivel,
        medir_fotogramas,
        fondo,
        fotos,
        &mut Vec::new(),
    )
}

/// `abrir`, devolviendo en `pegadas` los pixeles de las imagenes que se
/// metieron en el lienzo durante la sesion (pegadas, desde un fichero, la
/// foto de una zona), para que quien guarda la escena las guarde tambien
/// (`imagenes_lienzo::guardar_pegadas_junto_a`). Sin eso, la figura quedaba
/// guardada sin pixeles y al reabrir la imagen no estaba.
pub fn abrir_con_pegadas(
    escena: Escena,
    ajustes_iman: pixpin_motor2d::enganche::Ajustes,
    nivel: pixpin_nivel::Nivel,
    medir_fotogramas: bool,
    fondo: Option<pixpin_codec::ImagenRgba>,
    fotos: &[(u64, std::path::PathBuf)],
    pegadas: &mut Vec<(u64, pixpin_codec::ImagenRgba)>,
) -> Result<(Escena, Option<String>)> {
    let fondo = fondo.map(crate::fondo_lienzo::Fuente::from);
    abrir_sobre_con_pegadas(
        escena,
        ajustes_iman,
        nivel,
        medir_fotogramas,
        fondo,
        fotos,
        pegadas,
    )
}

/// `abrir` con cualquier fondo: tambien una pagina de PDF, que se pinta en
/// su hilo y se afina al acercarse (`fondo_lienzo`). Es lo que usa el chat
/// al abrir una hoja-pagina de un proyecto. Devuelve tambien las imagenes
/// nacidas en la sesion (ver [`abrir_con_pegadas`]).
///
/// Abre en ventana o a pantalla completa segun se dejo el ultimo lienzo
/// (`modo_ventana`), y F11 o el boton de la barra lo cambian sin cerrar la
/// ventana: ni el dibujo ni la vista se pierden.
pub fn abrir_sobre_con_pegadas(
    escena: Escena,
    ajustes_iman: pixpin_motor2d::enganche::Ajustes,
    nivel: pixpin_nivel::Nivel,
    medir_fotogramas: bool,
    fondo: Option<crate::fondo_lienzo::Fuente>,
    fotos: &[(u64, std::path::PathBuf)],
    pegadas: &mut Vec<(u64, pixpin_codec::ImagenRgba)>,
) -> Result<(Escena, Option<String>)> {
    abrir_en_modo(
        escena,
        ajustes_iman,
        nivel,
        medir_fotogramas,
        fondo,
        fotos,
        None,
        pegadas,
    )
}

/// **El anotador de pantalla** (ver `pantalla.rs`): el editor encima del
/// escritorio virtual entero, viva o congelada. No vuelve hasta que se sale
/// (Escape o el boton de salir); devuelve la foto de lo anotado si se dibujo
/// algo, para que quien llama la ofrezca guardar y la pinee como siempre.
pub fn abrir_pantalla(
    ajustes_iman: pixpin_motor2d::enganche::Ajustes,
    nivel: pixpin_nivel::Nivel,
    pantalla: &mut pantalla::Pantalla<'_>,
) -> Result<Option<pixpin_codec::ImagenRgba>> {
    let fondo = pantalla.foto.take().map(crate::fondo_lienzo::Fuente::from);
    let mut escena = Escena::nueva();
    escena.fondo = pantalla::papel(pantalla.modo);
    let (vuelta, _) = abrir_en_modo(
        escena,
        ajustes_iman,
        nivel,
        false,
        fondo,
        &[],
        Some(&mut *pantalla),
        &mut Vec::new(),
    )?;
    tracing::info!(
        elementos = vuelta.cuantos_visibles(),
        foto = pantalla.resultado.is_some(),
        "anotador de pantalla cerrado"
    );
    Ok(pantalla.resultado.take())
}

const VK_F11: u32 = 0x7A;
const VK_ESCAPE: u32 = 0x1B;

/// Lo que hace Escape en el lienzo cuando nada de la interfaz se lo quedo
/// (el texto a medias y las hermanas de un grupo los cierra antes `mano`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AlEscape {
    /// Nada del lienzo: lo atiende otro (el motor cancela el arrastre,
    /// presentar deja de presentar, el anotador de pantalla tiene su regla).
    Seguir,
    /// A pantalla completa: pasa a ventana, sin cerrar ni perder nada.
    AVentana,
    /// En ventana: cierra el lienzo guardando, como Salir.
    Cerrar,
}

/// **Escape, de dentro afuera** (15-oct-2026, el usuario: «un Esc que salga
/// de pantalla completa y lo ponga en ventana, y un segundo Esc que cierre
/// la ventana»). Lo que esta a medias manda: en el anotador de pantalla no
/// (tiene su regla, `pantalla::tecla`), presentando tampoco (Escape deja de
/// presentar) ni a mitad de un arrastre (lo cancela, como en Excalidraw).
/// Sin nada que soltar, a pantalla completa vuelve a ventana y en ventana
/// cierra: es el `BackHandler` del editor de Android, que sale primero de
/// «solo el dibujo» (su pantalla completa) y solo despues cierra.
fn al_escape(
    es_lienzo: bool,
    sin_presentar: bool,
    gesto_en_reposo: bool,
    pantalla_completa: bool,
) -> AlEscape {
    if !(es_lienzo && sin_presentar && gesto_en_reposo) {
        AlEscape::Seguir
    } else if pantalla_completa {
        AlEscape::AVentana
    } else {
        AlEscape::Cerrar
    }
}

/// **De mirar a editar** (`mirar = false`) **o al reves** (`mirar_editar`):
/// el texto a medias entra, y la herramienta se guarda al mirar (se mira con
/// la mano) y vuelve al editar. `true` si cambio algo.
fn poner_solo_mirar(
    mirar: bool,
    solo_mirar: &mut bool,
    gesto: &mut Gesto,
    escena: &mut Escena,
    herramienta_al_editar: &mut Herramienta,
) -> bool {
    if *solo_mirar == mirar {
        return false;
    }
    if gesto.esta_escribiendo() {
        gesto.cerrar_texto(escena);
    }
    *solo_mirar = mirar;
    if mirar {
        *herramienta_al_editar = gesto.herramienta;
        elegir_herramienta(gesto, Herramienta::Mano);
    } else {
        elegir_herramienta(gesto, *herramienta_al_editar);
    }
    true
}

/// F5: presentar (G5), como en PowerPoint.
const VK_F5: u32 = 0x74;

// Cada modo de abrir el editor pone lo suyo; agruparlo no aclara nada.
#[allow(clippy::too_many_arguments)]
fn abrir_en_modo(
    escena: Escena,
    ajustes_iman: pixpin_motor2d::enganche::Ajustes,
    nivel: pixpin_nivel::Nivel,
    medir_fotogramas: bool,
    fondo: Option<crate::fondo_lienzo::Fuente>,
    fotos: &[(u64, std::path::PathBuf)],
    // El anotador de pantalla (ver `pantalla.rs`): `None` es el lienzo de
    // siempre.
    mut pantalla: Option<&mut pantalla::Pantalla<'_>>,
    // Las imagenes nacidas en la sesion: entran las que ya trajera quien
    // llama y salen todas al cerrar, para que quien guarda las escriba.
    pegadas: &mut Vec<(u64, pixpin_codec::ImagenRgba)>,
) -> Result<(Escena, Option<String>)> {
    let dispositivo =
        pixpin_capture::Dispositivo::nuevo().context("sin dispositivo para el editor")?;
    let mut motor = MotorRender::nuevo(dispositivo.d3d()).context("sin motor de dibujo")?;
    // D139: se reduce aqui, que es donde se sabe cuanto admite la GPU.
    let mut fondo = fondo.map(|img| FondoLienzo::nuevo(img, motor.lado_maximo_bitmap()));
    // Las imagenes que se peguen durante esta sesion. Nace vacia: lo que
    // trae la escena de disco no puede traer pixeles todavia (ver el aviso
    // de alcance en `imagenes_lienzo`).
    let mut imagenes = ImagenesLienzo::nuevo(motor.lado_maximo_bitmap());
    // Las que ya trae el dibujo, con SU identificador: si se les diera uno
    // nuevo, las figuras seguirian apuntando al viejo y no se veria ninguna.
    for (id, ruta) in fotos {
        match pixpin_codec::cargar(ruta) {
            Ok(img) => {
                imagenes.guardar_con_id(*id, img);
            }
            Err(e) => tracing::warn!(?e, ruta = %ruta.display(), "imagen del proyecto ilegible"),
        }
    }
    // Y las que ya trajera quien llama, que aun no estan en disco.
    for (id, img) in std::mem::take(pegadas) {
        imagenes.reponer_nueva(id, img);
    }

    let disposicion =
        pixpin_capture::enumerar_monitores().context("sin monitores para el editor")?;
    let monitor = disposicion
        .principal()
        .context("sin monitor principal para el editor")?;
    // La herramienta y la barra dependen de quien abre: el lienzo o el
    // anotador de pantalla.
    let anfitrion = pantalla
        .as_deref()
        .map_or(crate::dibujo::permitidas::Anfitrion::Lienzo, |p| {
            p.anfitrion()
        });
    // **El lienzo, en ventana o a pantalla completa, como se dejo**
    // (`modo_ventana`). En ventana es un pin de los visores: sin marco, se
    // estira por los bordes, se lleva por el asa y lleva fuera su barra
    // (`lector_pin`), y nace sin fijar encima como ellos. A pantalla completa
    // tapa tambien la barra de tareas, y la misma barra asoma dentro, arriba
    // a la izquierda, para volver. El anotador de pantalla no es un pin.
    let mut pin = pantalla.is_none().then(|| {
        let recordado = modo_ventana::recordado();
        let minimo = modo_ventana::minimo(
            monitor.escala_por_cien,
            crate::dibujo::permitidas::botones(anfitrion),
        );
        let (_, trabajo_raton) =
            pixpin_shell::overlay::monitor_de(pixpin_shell::overlay::raton_en_pantalla());
        let trabajos: Vec<_> = disposicion
            .monitores()
            .iter()
            .map(|m| m.area_trabajo)
            .collect();
        let sitio = modo_ventana::sitio_al_abrir(
            recordado.ventana,
            &trabajos,
            modo_ventana::por_defecto(trabajo_raton),
            minimo,
        );
        let p = crate::lector_pin::PinLector::nuevo(sitio, monitor.escala_por_cien as f32 / 100.0)
            .con_extension("pixpin")
            .sin_oscuro()
            .con_minimo(minimo.0, minimo.1)
            .con_barra_en_pantalla_completa();
        if recordado.pantalla_completa {
            let centro = Punto {
                x: sitio.x + sitio.ancho as i32 / 2,
                y: sitio.y + sitio.alto as i32 / 2,
            };
            p.en_pantalla_completa(pixpin_shell::overlay::monitor_de(centro).0)
        } else {
            p
        }
    });
    // El nombre en la barra: el de la hoja del chat o «Lienzo».
    let nombre_del_pin = crate::zona_al_chat::hoja_abierta()
        .map(|h| crate::lector_pin::nombre_limpio(&h.nombre))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| exportar::textos().t("chat-lienzo"));
    let mut area = match (pantalla.as_deref(), pin.as_ref()) {
        // El anotador de pantalla cubre TODOS los monitores.
        (Some(p), _) => p.escritorio,
        (None, Some(pn)) => pn.area,
        (None, None) => monitor.area,
    };

    // Donde van la barra y el panel. En el lienzo, la ventana; en el
    // anotador de pantalla, el monitor principal (la ventana cubre todos, y
    // una barra centrada en el escritorio virtual quedaria partida entre
    // dos pantallas).
    let mut area_ui = pantalla.as_deref().map_or(area, |p| p.principal.area);
    // Todo lo de la interfaz vive en coordenadas del escritorio (son las de
    // los clics) y se pinta en las de la ventana: lo que hay que correrlo.
    let mut corrimiento_ui = (-(area.x as f32), -(area.y as f32));

    // El lienzo es una ventana de aplicacion corriente: se tapa con otros
    // programas, sale en la barra de tareas y en Alt+Tab. Siempre encima y
    // fuera de la barra dejaba al usuario sin poder ponerle nada delante.
    // El anotador de pantalla si va encima de todo: anota lo que hay debajo.
    let mut ventana = if pantalla.is_some() {
        VentanaOverlay::nueva(area)
    } else {
        VentanaOverlay::nueva_normal(area, "PixPin")
    }
    .context("no se pudo abrir el editor")?;
    // PIXPIN_TINTA_CLASICA vuelve a la presentacion de antes, para medir la
    // diferencia en el mismo equipo con el mismo binario.
    let tinta_clasica = std::env::var_os("PIXPIN_TINTA_CLASICA").is_some();
    let rendimiento = ajustes_rendimiento();
    // A3: con capas, la escena vive en su propio visual con colchon y la
    // barra en otro encima. `[rendimiento] paneo_por_composicion = false`
    // vuelve a la superficie de siempre, con todo en la misma swapchain.
    let por_composicion = rendimiento.paneo_por_composicion && !tinta_clasica;
    let superficie = if por_composicion {
        Superficie::nueva_con_capas(
            &motor,
            dispositivo.d3d(),
            ventana.handle(),
            area.ancho,
            area.alto,
            // El anotador de pantalla no se desplaza nunca: el colchon para
            // correr el visual sin repintar seria memoria para nada (a dos
            // monitores, 4352 x 1592 en vez de 3840 x 1080 por mapa).
            // Un pixel y no cero: sin colchon la superficie no monta sus capas
            // (`tiene_capas`), y la de la tinta es la que da la latencia.
            if pantalla.is_some() { 1 } else { MARGEN_ESCENA },
        )
    } else if tinta_clasica {
        Superficie::nueva(
            &motor,
            dispositivo.d3d(),
            ventana.handle(),
            area.ancho,
            area.alto,
        )
    } else {
        Superficie::nueva_baja_latencia(
            &motor,
            dispositivo.d3d(),
            ventana.handle(),
            area.ancho,
            area.alto,
        )
    }
    .context("sin superficie para el editor")?;
    // Lo que de verdad quedo montado: si la superficie con capas no se
    // pudo crear se cae aqui mismo, asi que esto es `por_composicion`, pero
    // leerlo de la superficie es lo unico que no puede mentir.
    let con_capas = superficie.tiene_capas();
    let margen_escena = superficie.margen();
    ventana.mostrar();
    // D145: el anotador tiene que tapar los pines. El lienzo, ventana normal,
    // queda delante al abrirse (`enfocar`) sin volverse «siempre encima».
    if pantalla.is_some() {
        ventana.traer_encima();
    }
    ventana.enfocar();
    ventana.pedir_entrada_fina();
    // El lienzo como los visores: al frente sin fijar, y su barra fuera.
    if let Some(pn) = pin.as_mut() {
        pn.al_nacer(&ventana);
        pn.crear_barra(&motor, dispositivo.d3d(), &ventana);
    }
    // **El anotador de pantalla: su pastilla y Alt + doble clic central**
    // (`pastilla_pantalla.rs`). La pastilla va en su propia ventana, con el
    // motor y el dispositivo de este editor; el gesto, mientras viva
    // `escucha`, alterna el clic a traves en vez de abrir otro anotador.
    let mut pastilla = pantalla.as_deref().and_then(|p| {
        let t = exportar::textos();
        let rotulos = (
            t.t("anotador-rotulo-dibujando"),
            t.t("anotador-rotulo-atravesando"),
        );
        pastilla_pantalla::Pastilla::nueva(
            &motor,
            dispositivo.d3d(),
            &p.principal,
            rotulos,
            ventana.handle(),
        )
        .inspect_err(|e| tracing::warn!(?e, "el anotador sin su pastilla"))
        .ok()
    });
    if let Some(ps) = pastilla.as_ref() {
        ps.mostrar();
    }
    let _escucha = pantalla.is_some().then(|| {
        pixpin_shell::gestos::EscuchaAnotador::tomar(
            ventana.handle(),
            pixpin_shell::overlay::MSG_DESPIERTA,
        )
    });
    let mut accion_pastilla: Option<pastilla_pantalla::Accion> = None;
    // El guardado de esta sesion en «Mensajes guardados»: el boton lo lanza
    // cuando se quiera y la salida siempre (`anotador_al_chat::EnElChat`).
    let mut chat = crate::anotador_al_chat::EnElChat::default();
    // Rueda del raton = zoom al cursor; panel tactil: dos dedos desplazan y
    // el pellizco acerca (`navegacion::decidir_rueda`, DirectManipulation en
    // `pixpin_shell::gestos_tactiles`). El anotador de pantalla no se mueve
    // y usa la rueda para lo suyo: no lo pide.
    if pantalla.is_none() {
        let dm = ventana.pedir_gestos_tactiles();
        tracing::debug!(directmanipulation = dm, "gestos tactiles del lienzo");
    }

    let mut escena = escena;
    let mut gesto = gesto_inicial(ajustes_iman);
    // La herramienta con la que abre tiene que salir en la barra: si el
    // usuario la apago en los ajustes, se abre con la mano.
    crate::dibujo::permitidas::asegurar(anfitrion, &mut gesto);
    // D135: con fondo, la imagen centrada; sin fondo, el origen como antes.
    let mut camara = match &fondo {
        Some(f) => encuadre_inicial(
            f.ancho(),
            f.alto(),
            area.ancho as f32,
            area.alto as f32,
            monitor.escala_por_cien,
        ),
        // Sin fondo, todo lo dibujado a la vista y centrado; vacio, el origen.
        None => escena
            .visibles()
            .map(|e| e.caja())
            .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
            .filter(|c| [c.0, c.1, c.2, c.3].iter().all(|v| v.is_finite()))
            .map_or_else(Camara::nueva, |caja| {
                crate::fondo_lienzo::encuadre_de_contenido(
                    caja,
                    area.ancho as f32,
                    area.alto as f32,
                    monitor.escala_por_cien,
                )
            }),
    };
    // D127: la camara del usuario va en pixeles logicos; `efectiva` es la
    // que pinta y traduce el raton. Se recalcula si cambia la escala.
    let mut escala_por_cien = monitor.escala_por_cien;
    // El anotador de pantalla no se mueve: 1:1 con los pixeles fisicos del
    // escritorio, con la foto (si la hay) en su sitio.
    if pantalla.is_some() {
        camara = pantalla::camara_quieta(escala_por_cien);
    }
    // D136: rueda, Shift, Ctrl, espacio y boton central, a la Excalidraw.
    let mut navegador = navegacion::Navegador::nuevo();
    let mut efectiva = vista_efectiva(&camara, escala_por_cien);
    let mut cache = Cache::nueva();
    let mut cache_tinta = pixpin_render::CacheTinta::nueva();
    let retardo = pixpin_render::retardo_nitido(nivel);
    // El ultimo zoom visto y desde cuando esta ahi, para `decidir_zoom`
    // (D136: Ctrl+rueda ya cambia el zoom de la camara del usuario).
    let mut zoom_visto = efectiva.zoom;
    let mut zoom_desde: Option<std::time::Instant> = None;
    let mut rejilla = Rejilla::nueva();
    let mut capa = CapaEstatica::nueva();
    let (mut ancho_px, mut alto_px) = (area.ancho as f32, area.alto as f32);
    // "Contenido" y area de trabajo son el mismo rectangulo, como en
    // `CapaViva::nueva` (`capa.rs`): aqui el contenido ES la pantalla
    // entera, no hay un pin ni una ventana mas pequena de referencia.
    let mut caja = CajaHerramientas::barra_superior(
        area_ui,
        escala_por_cien,
        crate::dibujo::permitidas::botones(anfitrion),
    );
    // Donde estaba el raton la ultima vez, para resaltar el boton de debajo.
    let mut raton_barra: Option<Punto> = None;
    // La mano: portapapeles, goma, forma rapida, punta predicha y filtro
    // de 1 euro, comunes a todos los anfitriones (`dibujo::mano`).
    let mut mano = crate::dibujo::mano::Mano::con_tinta(anfitrion, ajustes_tinta());
    // Anotador de pantalla viva: el pasante elegido con Espacio. Ctrl
    // mantenido lo pone solo mientras dura, y al soltarlo se vuelve a este.
    let mut pasante_fijo = false;
    // La lupa viva del anotador de pantalla (`pantalla::LupaViva`): la de
    // la capa vieja, con su sesion de captura solo mientras esta puesta.
    let mut lupa = pantalla
        .as_deref()
        .map(|p| pantalla::LupaViva::nueva(disposicion.monitores().to_vec(), p.escritorio));
    // La imagen de referencia flotando encima (`referencia.rs`): ninguna al
    // abrir, como en el movil.
    let mut la_referencia = referencia::Referencia::default();
    // Si el fotograma anterior pinto una punta predicha: la zona que se
    // presenta tiene que cubrir tambien donde estaba, o quedaria un resto.
    let mut habia_prediccion = false;
    // El panel lateral tal como se pinto por ultima vez.
    let mut ultimo_panel: Option<pixpin_ui::panel_lateral::PanelLateral> = None;

    // Zona sucia acumulada durante la vuelta: se pinta un solo fotograma
    // DESPUES de vaciar la cola de eventos, no uno por evento (pintar a
    // mitad de la cola era lo que hacia perder puntos del lapiz).
    let mut hay_que_pintar = false;
    // Si DXGI ya admite otro fotograma. Se pinta solo entonces, y justo
    // antes se leen los movimientos: los puntos mas nuevos llegan al
    // siguiente refresco en vez de esperar en una cola de tres fotogramas.
    let mut fotograma_listo = superficie.senal_fotograma().is_none();
    // Desde cuando se espera la senal con algo por pintar. Si otra ventana
    // tapa el editor, DWM no consume fotogramas y la senal no llega: sin un
    // tope el trazo se congelaba hasta soltar (medido, con la terminal
    // encima).
    let mut esperando_senal: Option<std::time::Instant> = None;
    let mut sucio: Option<(f32, f32, f32, f32)> = None;
    // Lo que ocupaba lo elegido en el aviso anterior mientras se estira o
    // se gira (`congelar::zona_al_transformar`): se rehace junto al de ahora.
    let mut zona_transformada: Option<(i32, i32, i32, i32)> = None;
    let mut todo_sucio = false;
    // A3: `todo_sucio` dice «hay que rehacer el fotograma»; esto dice «y
    // ademas cambio el DIBUJO, no solo desde donde se mira». Solo cuando es
    // falso se puede mover el visual en vez de repintar.
    let mut contenido_sucio = false;
    // La camara con la que se pinto la superficie de escena que hay ahora
    // mismo en la swapchain. Es contra esta, y no contra la anterior, contra
    // la que se calcula la transformada del visual.
    let mut camara_pintada = efectiva;
    // Desde cuando la camara se mueve sin repintar. Al pasar `REPOSO_CAMARA`
    // se pinta el fotograma nitido, como hace el pin con el zoom (D89).
    let mut camara_movida: Option<std::time::Instant> = None;
    // La capa de la interfaz (barra, panel, ruta) se repinta cuando cambia,
    // no en cada fotograma: mientras se traza, el trazo no la toca.
    let mut interfaz_sucia = con_capas;
    // B1: el planificador del ritmo y su temporizador fino. Sin
    // `[rendimiento] ritmo` no se crea ni el temporizador.
    let mut planificador = pixpin_tinta::Planificador::nuevo();
    let temporizador = rendimiento
        .ritmo
        .then(pixpin_shell::overlay::TemporizadorFino::nuevo)
        .flatten();
    // A que hora se esperaba que DWM compusiera el fotograma que se esta
    // pintando: si se pasa, el margen del planificador sube.
    let mut plazo_previsto: Option<std::time::Instant> = None;
    // La hoja a la que lleva el recuadro que se pulso, si se pulso alguno.
    let mut enlace_pedido: Option<String> = None;
    // Se pulso una zona vinculada cuyo lienzo no existe: se dice un momento
    // arriba, sin cerrar (`salto_por_enlace`).
    let mut aviso_enlace: Option<(String, std::time::Instant)> = None;
    // Se pulso F11 o el boton de la barra: a pantalla completa o de vuelta.
    let mut alternar_modo = false;
    // La zona sucia del fotograma anterior (D148): hace falta para la union
    // de dos, porque la cadena de intercambio tiene dos mapas.
    let mut zona_anterior: Option<(i32, i32, i32, i32)> = None;
    // B2: el trazo en curso se esta pintando en la capa de tinta, y por
    // tanto la escena no se toca ni se presenta hasta que se suelte.
    let mut tinta_viva = false;
    // La zona que se repinto de la capa de tinta en el fotograma anterior.
    // Aqui basta con UNA (no dos como en D148): una superficie de
    // composicion conserva lo que tenia, no rota entre dos mapas.
    let mut zona_tinta_anterior: Option<(i32, i32, i32, i32)> = None;
    // Arrastrar la seleccion o la marquesina en la capa de la tinta (ver
    // `pintar_seleccion_en_capa`). Excluye a `tinta_viva`: la capa es una.
    let mut en_capa: Option<EnCapa> = None;
    // Al terminar, la capa no se vacia hasta que la escena ya pinta lo que
    // estaba en ella: vaciarla antes dejaria un fotograma sin la seleccion.
    let mut vaciar_capa_tras_pintar = false;
    // Cuando se compuso la capa de tinta por ultima vez. Sin `Present` no
    // hay senal de latencia que marque el ritmo, asi que lo marca el reloj.
    let mut ultimo_commit_tinta = std::time::Instant::now();
    let mut medidor = crate::medir_fotogramas::MedidorFotogramas::nuevo(medir_fotogramas);
    // F5: las marcas de este dibujo y su riel.
    let mut marcador = marcas::Marcador::cargar();
    // Ni en el anotador de pantalla: las marcas son de un dibujo guardado.
    let con_marcas = pantalla.is_none();
    // F8: la pastilla de la Zona y, si el chat dijo de que hoja es este
    // lienzo, su interruptor «Al chat» (ver `zona_al_chat`).
    let mut pastilla_zona = pastilla_zona::PastillaZona::default();
    let hoja_del_chat = crate::zona_al_chat::hoja_abierta().filter(|_| pantalla.is_none());
    // Donde acaba la barra, en pixeles de la ventana: la tira de emoticonos
    // y la hojita salen justo debajo.
    let bajo_la_barra =
        |caja: &CajaHerramientas, area: pixpin_geom::Rect| caja.marco.abajo() - area.y;
    // **Soltar el lapiz sin repintar la escena** (ver `hornear_trazo`).
    //
    // `trazo_limpio`: el trazo que se esta dibujando en la capa de la tinta
    // empezo con la escena al dia -lo que hay en su superficie es EXACTAMENTE
    // la escena sin el-, asi que al soltarlo basta con pintarlo a el encima.
    // `trazo_por_hornear`: ya se solto y espera su fotograma. Mientras
    // espera, la capa de la tinta lo sigue ensenando.
    let mut trazo_limpio: Option<u64> = None;
    let mut trazo_por_hornear: Option<u64> = None;
    // **Arrastrar sin repintar la escena entera** (ver `repintar_zona`): el
    // trozo de escena que hay que rehacer al empezar o al acabar de mover lo
    // elegido en su capa, y la caja de lo elegido al pulsar, que es donde
    // sigue pintado cuando llega el primer aviso de moverse.
    let mut zona_por_rehacer: Option<(i32, i32, i32, i32)> = None;
    let mut caja_al_pulsar: Option<(f32, f32, f32, f32)> = None;
    // En que difiere el mapa de atras de la cadena de intercambio del que
    // se ve (pixeles de SUPERFICIE). `None` = no se sabe, se iguala entero.
    // Tras un horneado es la zona de su trazo: soltar trazo tras trazo
    // copia solo la zona del anterior.
    let mut trasero_distinto: Option<(i32, i32, i32, i32)> = None;
    // Si la ventana estaba delante la vuelta anterior: al perderla se
    // devuelve la reserva de memoria de video (`MotorRender::devolver_memoria`).
    let mut en_primer_plano = true;
    // F14: si la estela del laser seguia viva la vuelta anterior.
    let mut laser_vivo = false;
    // La estela del laser, en la capa de la tinta (22-oct).
    let mut laser_en_capa = false;
    // G5: la presentacion en curso (F5) y el modo solo mirar (Alt+R), con
    // su arrastre de mover el dibujo.
    let mut presentacion: Option<presentar::Presentacion> = None;
    // **Abre siempre mirando** (24-oct-2026, `mirar_editar`): el lapiz del
    // riel (o Alt+R) pasa a editar, a pantalla completa y sin la barra del
    // pin. El anotador de pantalla no: alli se viene a pintar.
    let mut solo_mirar = pantalla.is_none();
    let mut herramienta_al_editar = gesto.herramienta;
    // Lo que hay que hacer con la ventana al pasar de mirar a editar y
    // vuelta, y el cambio que aun no se aplico (lo aplica el pin, abajo).
    let mut cambio_de_mirar: Option<bool> = None;
    if solo_mirar {
        elegir_herramienta(&mut gesto, Herramienta::Mano);
    }
    let mut arrastre_mirar: Option<presentar::Arrastre> = None;
    // Lo que dibuje el asistente mientras este lienzo esta abierto cae aqui,
    // centrado en lo que se ve (`asistente::al_lienzo`). El anotador de
    // pantalla no es un lienzo.
    // El panel de propiedades del lienzo lleva su boton de quitarlo (24-oct).
    crate::panel_dibujo::con_boton_plegar(pantalla.is_none());
    let _para_el_asistente = pantalla.is_none().then(|| {
        // Con la hoja que tiene: lo que el asistente pida para otro
        // lienzo del chat lo hace el chat en su fichero (17-oct).
        crate::asistente::al_lienzo::EditorAbierto::nuevo(ventana.handle().0 as isize)
            .con_hoja(crate::zona_al_chat::hoja_abierta().map(|h| h.dibujo))
    });
    // Solo para probar (`PIXPIN_MENU_FIGURAS_AL_ABRIR`): el menu de las
    // figuras se abre solo a los 2 s, sin tocar el raton del usuario.
    let mut figuras_al_abrir = std::env::var_os("PIXPIN_MENU_FIGURAS_AL_ABRIR")
        .map(|_| std::time::Instant::now() + std::time::Duration::from_secs(2));

    'bucle: loop {
        // D129: se mide siempre (unos nanosegundos por `Instant::now`); solo
        // se acumula y registra con la opcion encendida.
        let t_vuelta = std::time::Instant::now();
        let mut puntos = 0u32;
        pixpin_shell::overlay::bombear_pendientes();
        // La tinta sobre papel de noche se adapta al pintar (`dibujo::tema`);
        // se fija por vuelta porque el papel se cambia desde el panel.
        // Sin papel en el anotador de pantalla: debajo hay cualquier cosa.
        crate::dibujo::tema::fijar_papel(pantalla.is_none().then_some(escena.fondo));
        // Una pagina de PDF de fondo trae su resolucion desde otro hilo: al
        // llegar, la capa congelada (que la lleva cocida) se rehace.
        if let Some(f) = fondo.as_mut() {
            f.avisar_a(ventana.handle().0 as isize);
            if f.hay_novedades() {
                capa.soltar();
                todo_sucio = true;
                contenido_sucio = true;
                ventana.invalidar();
            }
        }
        // Al dejar de estar delante, la reserva de memoria de video que
        // Direct2D y el controlador guardan se devuelve: en la HD 4000 es
        // memoria del sistema que le falta al resto mientras el editor siga
        // abierto detras. Perder el primer plano manda mensajes a la
        // ventana, asi que esta vuelta llega aunque no se mueva el raton.
        let delante = pixpin_render::superficie::en_primer_plano(ventana.handle());
        if en_primer_plano && !delante {
            motor.devolver_memoria(dispositivo.d3d());
        }
        en_primer_plano = delante;
        // Lo que llega de otro aparato del grupo al lienzo abierto
        // (`al_frente`): este se apunta como abierto mientras esta delante
        // y recoge las fotos que le dejan. El anotador de pantalla no es un
        // lienzo: no se apunta.
        if pantalla.is_none() {
            let h = ventana.handle().0 as isize;
            if delante {
                crate::al_frente::poner_lienzo(h);
                // Con varios lienzos abiertos, el asistente dibuja en este.
                crate::asistente::al_lienzo::activar(h);
            }
            let (x0, y0, x1, y1) = efectiva.ventana(ancho_px, alto_px);
            // «Guarda esto como figura»: lo elegido, para el asistente.
            crate::asistente::al_lienzo::contestar_lo_elegido(&escena, gesto.seleccion.ids());
            // Lo que dibuja, edita o resume el asistente (`pixpin_ia::edicion`):
            // con lo que se ve, lo elegido y el almacen de imagenes (pegar).
            let elegidos = gesto.seleccion.ids().to_vec();
            if crate::asistente::al_lienzo::atender_en_editor_con(
                &mut escena,
                (x0, y0, x1, y1),
                &elegidos,
                &mut imagenes,
            ) {
                // Las hojas de una plantilla alargada quedan elegidas.
                if let Some(ids) = crate::asistente::al_lienzo::tomar_para_elegir() {
                    gesto.seleccion.poner_todos(ids);
                }
                todo_sucio = true;
                contenido_sucio = true;
                interfaz_sucia = true;
                ventana.invalidar();
            }
            // Lo mandado a este lienzo por su nombre (`pila_lienzo`, 17-oct):
            // apilado en su columna y, si no se ve, la vista va hasta ello.
            let apilados = crate::pila_lienzo::atender(h, &mut escena, &mut imagenes);
            if !apilados.is_empty() {
                if let Some(c) = crate::pila_lienzo::caja_de(&escena, &apilados)
                    && crate::asistente::al_lienzo::desplazar_para_ver((x0, y0, x1, y1), c)
                        != (0.0, 0.0)
                {
                    crate::asistente::al_lienzo::pedir_mostrar(c);
                }
                todo_sucio = true;
                contenido_sucio = true;
                interfaz_sucia = true;
                ventana.invalidar();
            }
            // Lo que dibujo el asistente no se veia: la vista va hasta ello
            // (15-oct, «traelo»).
            if let Some(caja) = crate::asistente::al_lienzo::tomar_mostrar() {
                let (dx, dy) =
                    crate::asistente::al_lienzo::desplazar_para_ver((x0, y0, x1, y1), caja);
                if (dx, dy) != (0.0, 0.0) {
                    camara.x += dx;
                    camara.y += dy;
                    efectiva = vista_efectiva(&camara, escala_por_cien);
                    todo_sucio = true;
                    ventana.invalidar();
                }
            }
            let llegadas = crate::al_frente::tomar_para(h);
            if !llegadas.is_empty() {
                for r in &llegadas {
                    figuras::meter_imagen_de(
                        r,
                        &mut imagenes,
                        &mut escena,
                        &mut gesto,
                        &efectiva,
                        ancho_px,
                        alto_px,
                    );
                    let _ = std::fs::remove_file(r);
                }
                todo_sucio = true;
                contenido_sucio = true;
                interfaz_sucia = true;
                ventana.invalidar();
            }
        }
        for (hwnd, mut ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            // La pastilla del anotador de pantalla: sus clics son suyos.
            if let Some(ps) = pastilla.as_mut()
                && hwnd == ps.handle()
            {
                if let Some(a) = ps.evento(&ev) {
                    accion_pastilla = Some(a);
                }
                continue;
            }
            // La barra del pin (en ventana, fuera; a pantalla completa,
            // asomando dentro): llevar la ventana por el asa y sus botones.
            if let Some(pn) = pin.as_mut()
                && pn.es_de_la_barra(hwnd)
            {
                let (accion, _) = pn.evento_barra(&ev, &mut ventana);
                match accion.map(modo_ventana::de_la_barra) {
                    Some(modo_ventana::DeLaBarra::Alternar) => alternar_modo = true,
                    Some(modo_ventana::DeLaBarra::Fijar) => pn.alternar_fijado(&ventana),
                    Some(modo_ventana::DeLaBarra::Lapiz) => {
                        if poner_solo_mirar(
                            !solo_mirar,
                            &mut solo_mirar,
                            &mut gesto,
                            &mut escena,
                            &mut herramienta_al_editar,
                        ) {
                            cambio_de_mirar = Some(!solo_mirar);
                        }
                        arrastre_mirar = None;
                        todo_sucio = true;
                        interfaz_sucia = true;
                        ventana.invalidar();
                    }
                    // Como Escape o Salir: el texto a medias entra en lo
                    // que se guarda al volver.
                    Some(modo_ventana::DeLaBarra::Cerrar) => {
                        if gesto.esta_escribiendo() {
                            gesto.cerrar_texto(&mut escena);
                        }
                        break 'bucle;
                    }
                    Some(modo_ventana::DeLaBarra::Nada) | None => {}
                }
                continue;
            }
            if hwnd != ventana.handle() {
                continue;
            }
            // Estirar por los bordes es del pin (en ventana).
            if let Some(pn) = pin.as_mut()
                && pn.evento_ventana(&ev, &mut ventana).consumido
            {
                continue;
            }
            // Alt + doble clic central con el anotador abierto: el gancho
            // (`gestos::EscuchaAnotador`) despierta esta ventana.
            if matches!(ev, EventoOverlay::Despierta)
                && pantalla.is_some()
                && pixpin_shell::gestos::tomar_doble_central_del_anotador()
            {
                accion_pastilla = Some(pastilla_pantalla::Accion::Atravesar);
            }
            if matches!(
                ev,
                EventoOverlay::RatonMovido(_) | EventoOverlay::Muestra(_)
            ) {
                puntos += 1;
            }
            // D127: con el editor abierto se puede cambiar la escala de
            // Windows. La tinta tiene que seguir del tamano de Excalidraw, y
            // la caja de herramientas, del de sus botones.
            if matches!(ev, EventoOverlay::CambioDpi) {
                if let Some(m) = pixpin_capture::enumerar_monitores()
                    .ok()
                    .and_then(|d| d.principal().copied())
                {
                    escala_por_cien = m.escala_por_cien;
                    if let Some(pn) = pin.as_mut() {
                        pn.poner_escala(escala_por_cien as f32 / 100.0);
                    }
                    // El anotador de pantalla sigue a 1:1 con cualquier escala.
                    if pantalla.is_some() {
                        camara = pantalla::camara_quieta(escala_por_cien);
                    }
                    efectiva = vista_efectiva(&camara, escala_por_cien);
                    caja = CajaHerramientas::barra_superior(
                        area_ui,
                        escala_por_cien,
                        crate::dibujo::permitidas::botones(anfitrion),
                    )
                    .con_desplegado(mano.desplegado);
                    // La capa congelada se horneo a la escala vieja.
                    capa.soltar();
                    todo_sucio = true;
                    contenido_sucio = true;
                    ventana.invalidar();
                }
                continue;
            }
            // **El panel pide el teclado mientras se escribe el codigo de un
            // color.** Va antes que todo lo que lee teclas —el universo, el
            // navegador, los atajos de herramienta—: si no, la «a» de
            // «a5d8ff» elegiria una herramienta. Solo se queda las teclas;
            // el raton sigue su camino y un clic fuera suelta el campo.
            if let Some(repintar) = crate::panel_dibujo::tecla_del_hex(&ev, &mut gesto, &mut escena)
            {
                if repintar {
                    todo_sucio = true;
                    contenido_sucio = true;
                    ventana.invalidar();
                }
                continue;
            }
            // El resaltado de la barra sigue al raton, como en Excalidraw.
            // Solo se repinta cuando cambia el boton de debajo: con cada
            // movimiento seria un fotograma entero por nada.
            if let EventoOverlay::RatonMovido(p) = ev {
                let antes = raton_barra.and_then(|r| caja.boton_en(r));
                raton_barra = Some(p);
                if caja.boton_en(p) != antes {
                    todo_sucio = true;
                    contenido_sucio = true;
                    ventana.invalidar();
                }
            }
            // La lupa viva sigue al raton y se queda la rueda (el aumento).
            if let Some(l) = lupa.as_mut()
                && l.evento(&ev, gesto.herramienta == Herramienta::Lupa)
            {
                continue;
            }
            // **El anotador de pantalla** (`pantalla.rs`): Escape sale si no
            // hay nada que soltar; Espacio deja pasar los clics a lo de debajo
            // y los vuelve a coger; Ctrl mantenido los deja pasar mientras
            // dure. Va antes que el navegador, que no hay, y que las
            // herramientas, que no tienen que ver ni el espacio ni el Ctrl.
            if let Some(pa) = pantalla.as_deref() {
                use pantalla::TeclaPantalla;
                let sin_nada =
                    gesto.seleccion.esta_vacia() && gesto.en_reposo() && gesto.lazo.is_none();
                match pantalla::tecla(&ev, pa.modo, !gesto.esta_escribiendo(), sin_nada) {
                    Some(TeclaPantalla::Salir) => break 'bucle,
                    Some(TeclaPantalla::AlternarPasante) => {
                        pasante_fijo = !pasante_fijo;
                        ventana.poner_pasante(pasante_fijo);
                        if let Some(ps) = pastilla.as_mut() {
                            ps.poner_pasante(pasante_fijo);
                        }
                        // Al volver a dibujar se recupera el foco: mientras
                        // era pasante se estuvo pulsando en la aplicacion de
                        // abajo, y Escape y las letras seguirian yendo alli.
                        if !pasante_fijo {
                            ventana.enfocar();
                        }
                        tracing::info!(pasante = pasante_fijo, "el anotador cambia de modo");
                        // La barra aparece o desaparece: dice en que estado
                        // se esta.
                        interfaz_sucia = true;
                        todo_sucio = true;
                        ventana.invalidar();
                        continue;
                    }
                    Some(TeclaPantalla::Ctrl(pulsado)) => {
                        // Solo el raton: el teclado sigue llegando, asi que
                        // Ctrl+Z deshace aunque el clic ya pase.
                        ventana.poner_pasante(pulsado || pasante_fijo);
                    }
                    None => {}
                }
            }
            // **G5: presentar (F5) y solo mirar (Alt+R)** (`presentar.rs`).
            // Van antes que el navegador y que la barra: presentando no hay
            // barra, y lo que se pulsa pasa diapositivas o anota.
            if pantalla.is_none() {
                let escribiendo = gesto.esta_escribiendo();
                let mut repintar_todo = false;
                let mut consumido = false;
                if presentacion.is_none()
                    && !escribiendo
                    && matches!(ev, EventoOverlay::Tecla { vk: VK_F5, .. })
                {
                    consumido = true;
                    if let Some(pr) =
                        presentar::Presentacion::empezar(&escena, camara, gesto.herramienta)
                    {
                        elegir_herramienta(&mut gesto, Herramienta::Mano);
                        camara = pr.camara(ancho_px, alto_px, escala_por_cien);
                        efectiva = vista_efectiva(&camara, escala_por_cien);
                        tracing::info!(diapositivas = pr.hojas.len(), "presentar");
                        presentacion = Some(pr);
                        // Al acabar vuelve a como estaba, mirando o editando
                        // (24-oct): editar es otra ventana (`mirar_editar`).
                        repintar_todo = true;
                    }
                } else if presentacion.is_none()
                    && !escribiendo
                    && matches!(ev, EventoOverlay::Tecla { vk, alt: true, ctrl: false, .. } if vk == b'R' as u32)
                {
                    consumido = true;
                    if poner_solo_mirar(
                        !solo_mirar,
                        &mut solo_mirar,
                        &mut gesto,
                        &mut escena,
                        &mut herramienta_al_editar,
                    ) {
                        cambio_de_mirar = Some(!solo_mirar);
                    }
                    arrastre_mirar = None;
                    repintar_todo = true;
                } else if let Some(pr) = presentacion.as_mut() {
                    let (ancho_v, alto_v) = (ancho_px, alto_px);
                    let accion = match ev {
                        // Ctrl+Z deshace lo anotado: esa sigue al gesto.
                        EventoOverlay::Tecla {
                            vk, ctrl: false, ..
                        } => {
                            consumido = true;
                            presentar::Presentacion::tecla(vk)
                        }
                        // Las letras son de la presentacion, no de la barra.
                        EventoOverlay::Caracter(_) => {
                            consumido = true;
                            None
                        }
                        EventoOverlay::BotonPulsado(p) => {
                            let (x, y) = ((p.x - area.x) as f32, (p.y - area.y) as f32);
                            let boton = (pr.pastilla && !pr.negro)
                                .then(|| {
                                    presentar::boton_en(x, y, ancho_v, alto_v, escala_por_cien)
                                })
                                .flatten();
                            match boton {
                                Some(b) => {
                                    consumido = true;
                                    Some(match b {
                                        presentar::Boton::Anterior => presentar::Accion::Pasar(-1),
                                        presentar::Boton::Siguiente => presentar::Accion::Pasar(1),
                                        presentar::Boton::Modo(m) => presentar::Accion::Modo(m),
                                        presentar::Boton::Salir => presentar::Accion::Salir,
                                    })
                                }
                                None if pr.pastilla
                                    && !pr.negro
                                    && presentar::en_la_pastilla(
                                        x,
                                        y,
                                        ancho_v,
                                        alto_v,
                                        escala_por_cien,
                                    ) =>
                                {
                                    consumido = true;
                                    None
                                }
                                // Fundido a negro: un clic vuelve.
                                None if pr.negro => {
                                    consumido = true;
                                    Some(presentar::Accion::Negro)
                                }
                                None if pr.modo == presentar::Modo::Pasar => {
                                    consumido = true;
                                    Some(presentar::Presentacion::clic_para_pasar(x, ancho_v))
                                }
                                None => None,
                            }
                        }
                        _ => None,
                    };
                    // En «Pasar» (o a negro) el raton no dibuja ni mueve nada.
                    if pr.modo == presentar::Modo::Pasar || pr.negro {
                        consumido = true;
                    }
                    if let Some(a) = accion {
                        let (modo, actual) = (pr.modo, pr.actual);
                        if pr.aplicar(a) {
                            let (cam, h) = pr.antes;
                            camara = cam;
                            elegir_herramienta(&mut gesto, h);
                            presentacion = None;
                        } else {
                            if pr.modo != modo {
                                elegir_herramienta(&mut gesto, pr.modo.herramienta());
                            }
                            if pr.actual != actual {
                                camara = pr.camara(ancho_px, alto_px, escala_por_cien);
                                gesto.laser.vaciar();
                            }
                        }
                        efectiva = vista_efectiva(&camara, escala_por_cien);
                        repintar_todo = true;
                    }
                } else if solo_mirar {
                    // Solo mirar: arrastrar mueve el dibujo, como la mano de
                    // Excalidraw en su «view mode»; nada se elige ni se edita.
                    // El riel sigue a la vista mirando (24-oct): un clic en
                    // el es suyo (el lapiz pasa a editar), no un arrastre.
                    let en_el_riel = |p: Punto| {
                        con_marcas
                            && marcador
                                .riel(
                                    area.ancho,
                                    area.alto,
                                    escala_por_cien,
                                    bajo_la_barra(&caja, area),
                                )
                                .contiene(Punto {
                                    x: p.x - area.x,
                                    y: p.y - area.y,
                                })
                    };
                    match ev {
                        EventoOverlay::BotonPulsado(p) if en_el_riel(p) => {}
                        EventoOverlay::BotonPulsado(p) => {
                            consumido = true;
                            arrastre_mirar = Some(presentar::Arrastre {
                                desde: (p.x as f32, p.y as f32),
                                camara,
                            });
                        }
                        EventoOverlay::RatonMovido(p) => {
                            if let Some(a) = arrastre_mirar {
                                consumido = true;
                                camara = a.camara_en(p.x as f32, p.y as f32, efectiva.zoom);
                                efectiva = vista_efectiva(&camara, escala_por_cien);
                                todo_sucio = true;
                                ventana.invalidar();
                            }
                        }
                        EventoOverlay::BotonSoltado(_) => {
                            consumido = arrastre_mirar.take().is_some();
                        }
                        EventoOverlay::Caracter(_) | EventoOverlay::Muestra(_) => consumido = true,
                        // Las letras eligirian herramientas y Supr borraria: fuera.
                        // Escape y F11 siguen siendo del lienzo.
                        EventoOverlay::Tecla {
                            vk, ctrl: false, ..
                        } if (b'A' as u32..=b'Z' as u32).contains(&vk) || vk == 0x2E => {
                            consumido = true
                        }
                        _ => {}
                    }
                }
                if repintar_todo {
                    todo_sucio = true;
                    contenido_sucio = true;
                    interfaz_sucia = true;
                    ventana.invalidar();
                }
                if consumido {
                    continue;
                }
            }
            // D136: moverse por el lienzo va ANTES que la caja y que el
            // gesto. Un clic con el espacio pulsado arrastra el lienzo aunque
            // caiga sobre un boton, como en Excalidraw, y lo que el navegador
            // consume no puede empezar un trazo. Los modificadores solo se
            // leen para la rueda: sondearlos con cada muestra del lapiz seria
            // pagar cinco llamadas al sistema mil veces por segundo.
            let mods = if matches!(
                ev,
                EventoOverlay::Rueda(_) | EventoOverlay::RuedaHorizontal(_)
            ) {
                let m = pixpin_shell::entrada::modificadores_pulsados();
                navegacion::Modificadores {
                    ctrl: m.ctrl,
                    shift: m.shift,
                }
            } else {
                navegacion::Modificadores::default()
            };
            // **Alt+rueda con el angulo en la mano** (17-oct): cambia la letra
            // griega de la marca recien puesta (o de la elegida), hacia
            // arriba la siguiente; la rueda sola sigue acercando.
            if let EventoOverlay::Rueda(delta) = ev
                && gesto.herramienta == Herramienta::Angulo
                && pixpin_shell::entrada::modificadores_pulsados().alt
                && gesto.marca_para_letra(&escena).is_some()
            {
                if gesto.cambiar_letra_de_angulo(&mut escena, if delta > 0 { 1 } else { -1 }) {
                    todo_sucio = true;
                    contenido_sucio = true;
                    ventana.invalidar();
                }
                continue;
            }
            // Revision 1: el evento que cerraria el gesto a veces no llega
            // (Alt+Tab suelta el espacio sin `TeclaSoltada`; Windows le
            // quita la captura al raton a mitad de un arrastre sin mandar
            // el boton-arriba). `Navegador` sigue puro -no llama a
            // Windows-: aqui se sondea el estado en vivo, y solo para los
            // dos eventos donde `Navegador::evento` lo necesita, no en cada
            // muestra del lapiz.
            //
            // Revision 2: `botones_en_arrastre` puede dar DOS codigos para
            // el arrastre principal -`overlay.rs` traduce el clic derecho al
            // mismo `BotonPulsado` que el izquierdo-, y hace falta que
            // CUALQUIERA de los dos siga pulsado (`algun_boton_pulsado`), no
            // solo el primero: mirar solo el izquierdo cortaria un
            // espacio+arrastre sujeto con el derecho en el primer
            // `RatonMovido`.
            let vivo = navegacion::EnVivo {
                espacio: if matches!(ev, EventoOverlay::BotonPulsado(_)) {
                    pixpin_shell::entrada::tecla_pulsada_ahora(navegacion::VK_ESPACIO)
                } else {
                    true
                },
                boton_arrastre: if navegador.arrastrando()
                    && matches!(
                        ev,
                        EventoOverlay::RatonMovido(_) | EventoOverlay::Muestra(_)
                    ) {
                    navegacion::algun_boton_pulsado(
                        navegador.botones_en_arrastre(),
                        pixpin_shell::entrada::tecla_pulsada_ahora,
                    )
                } else {
                    true
                },
            };
            // El anotador de pantalla no se mueve: ni rueda ni espacio.
            let nav = if pantalla.is_some() {
                navegacion::Respuesta::default()
            } else {
                navegador.evento(
                    &ev,
                    Punto {
                        x: area.x,
                        y: area.y,
                    },
                    escala_por_cien,
                    mods,
                    gesto.en_reposo(),
                    vivo,
                )
            };
            // La rueda no se persigue: perseguir el zoom (~170 ms hasta
            // llegar) se notaba como retraso al acercar para dibujar; cada
            // muesca se aplica entera en el acto.
            if let Some(accion) = nav.accion {
                if navegacion::aplicar_con_minimo(&mut camara, accion, navegador.zoom_minimo) {
                    efectiva = vista_efectiva(&camara, escala_por_cien);
                    // La capa congelada se da por invalida sola: su Estampa
                    // lleva la camara. `decidir_zoom` rehace la tinta nitida
                    // cuando el zoom se quede quieto.
                    //
                    // Este es el UNICO sitio que ensucia sin tocar
                    // `contenido_sucio`: aqui no ha cambiado nada del dibujo,
                    // solo desde donde se mira. Es lo que le permite a A3
                    // mover el visual en vez de repintar (mas abajo, en el
                    // bloque de «pintar o componer»). Si algun dia se anade
                    // otro sitio que mueva la camara, o lo marca tambien o
                    // pagara un repintado: fallar hacia repintar es lo
                    // correcto, porque componer una escena que si cambio
                    // ensenaria el dibujo viejo.
                    todo_sucio = true;
                    ventana.invalidar();
                }
            }
            if nav.consumido {
                if navegador.arrastrando() {
                    ventana.poner_cursor(FormaCursorWin::Mover);
                }
                continue;
            }
            // F8: la pastilla de la Zona es un boton, no el sitio donde empieza
            // una zona (ver `pastilla_zona`).
            if let EventoOverlay::BotonPulsado(q) = &ev {
                let visible =
                    gesto.herramienta == Herramienta::Zona && presentacion.is_none() && !solo_mirar;
                match pastilla_zona.pulsar((q.x - area.x) as f32, (q.y - area.y) as f32, visible) {
                    pastilla_zona::Clic::Fuera => {}
                    pastilla_zona::Clic::Tragado => continue,
                    pastilla_zona::Clic::Cambiado => {
                        interfaz_sucia = true;
                        ventana.invalidar();
                        continue;
                    }
                }
            }
            // F5/F6: el riel de marcas, las marcas del lienzo y la hojita. Van
            // antes que la barra y el gesto por lo mismo que el enlace: un
            // clic en una marca o en el riel es un boton, no un trazo. Todo lo
            // de dentro esta en `marcas.rs`; aqui solo se aplica lo que pide.
            // Tambien mirando: el riel esta a la vista, con su lapiz para
            // editar (lo demas del lienzo ya se lo quedo «solo mirar»).
            if con_marcas && presentacion.is_none() {
                let r = marcador.evento(
                    &ev,
                    &marcas::Contexto {
                        area,
                        escala_por_cien,
                        efectiva: &efectiva,
                        camara: &camara,
                        mano: gesto.herramienta == Herramienta::Mano,
                        escribiendo: gesto.esta_escribiendo(),
                        ocupado: !gesto.en_reposo() || gesto.lazo.is_some() || mano.borrando,
                        bajo_la_barra: bajo_la_barra(&caja, area),
                    },
                );
                if let Some(c) = r.cursor {
                    ventana.poner_cursor(c);
                }
                if r.repintar {
                    todo_sucio = true;
                    contenido_sucio = true;
                    interfaz_sucia = true;
                    ventana.invalidar();
                }
                // Una ficha de los marcos: la camara a ese, llenando la ventana.
                if let Some(i) = r.ir_a_marco
                    && let Some(m) = marcador.marcos.get(i).copied()
                {
                    let k = escala_por_cien as f32 / 100.0;
                    let (w, h) = (area.ancho as f32 / k, area.alto as f32 / k);
                    let zoom = ((w * 0.9) / m[2].max(1.0))
                        .min((h * 0.8) / m[3].max(1.0))
                        .clamp(0.05, 30.0);
                    camara.zoom = zoom;
                    camara.x = m[0] + m[2] / 2.0 - w / (2.0 * zoom);
                    camara.y = m[1] + m[3] / 2.0 - h / (2.0 * zoom);
                    efectiva = vista_efectiva(&camara, escala_por_cien);
                    todo_sucio = true;
                    ventana.invalidar();
                }
                if r.imprimir {
                    // El marco, como en el plano: la herramienta Marco (arrastrar
                    // pone uno; su numero es su orden de lectura y Ctrl+P los
                    // imprime todos, uno por hoja). Otra vez, la de antes.
                    if gesto.herramienta == Herramienta::Marco {
                        let antes = if herramienta_al_editar == Herramienta::Marco {
                            Herramienta::Mano
                        } else {
                            herramienta_al_editar
                        };
                        elegir_herramienta(&mut gesto, antes);
                    } else {
                        // Mirando, el marco es editar: a pantalla completa.
                        if poner_solo_mirar(
                            false,
                            &mut solo_mirar,
                            &mut gesto,
                            &mut escena,
                            &mut herramienta_al_editar,
                        ) {
                            cambio_de_mirar = Some(true);
                            arrastre_mirar = None;
                        }
                        herramienta_al_editar = gesto.herramienta;
                        elegir_herramienta(&mut gesto, Herramienta::Marco);
                    }
                    todo_sucio = true;
                    ventana.invalidar();
                }
                // El lapiz del riel: de mirar a editar y vuelta (24-oct).
                if r.editar {
                    if poner_solo_mirar(
                        !solo_mirar,
                        &mut solo_mirar,
                        &mut gesto,
                        &mut escena,
                        &mut herramienta_al_editar,
                    ) {
                        cambio_de_mirar = Some(!solo_mirar);
                    }
                    arrastre_mirar = None;
                    todo_sucio = true;
                    contenido_sucio = true;
                    interfaz_sucia = true;
                    ventana.invalidar();
                }
                if r.hojita {
                    rejilla.sincronizar(&escena);
                    // La hojita lleva su panel en su sitio, sin el boton.
                    crate::panel_dibujo::con_boton_plegar(false);
                    let pedido = hojita::abrir(hojita::Editor {
                        ventana: &ventana,
                        dispositivo: &dispositivo,
                        motor: &mut motor,
                        superficie: &superficie,
                        escena: &escena,
                        camara: &efectiva,
                        gesto: &gesto,
                        cache: &mut cache,
                        cache_tinta: &mut cache_tinta,
                        rejilla: &rejilla,
                        capa: &capa,
                        fondo: &mut fondo,
                        imagenes: &mut imagenes,
                        caja: &caja,
                        marcador: &marcador,
                        area,
                        escala_por_cien,
                        ancho_px,
                        alto_px,
                        bajo_la_barra: bajo_la_barra(&caja, area),
                    });
                    crate::panel_dibujo::con_boton_plegar(true);
                    if let Some(ins) = pedido {
                        hojita::insertar_en_el_lienzo(
                            &mut escena,
                            &mut gesto,
                            &efectiva,
                            ancho_px,
                            alto_px,
                            ins,
                        );
                    }
                    todo_sucio = true;
                    contenido_sucio = true;
                    interfaz_sucia = true;
                    ventana.invalidar();
                }
                if r.consumido {
                    continue;
                }
            }
            // 0. La caja de herramientas es un dialogo en pantalla: un clic
            // ahi ELIGE o dispara una accion, y no puede llegar ademas al
            // gesto como si fuera un trazo en el lienzo -mismo cuidado que
            // `CapaViva::raton` ya toma en `capa.rs`-. Se resuelve con las
            // coordenadas de pantalla tal cual llegan, ANTES de que
            // `a_evento` las convierta a mundo con la camara.
            //
            // La pregunta "de la caja o del lienzo" es `CajaHerramientas::
            // destino`, pura: aqui solo queda el `match` sobre su resultado,
            // asi que la decision en si esta bajo prueba sin ventana.
            // El panel lateral, igual que la barra: un clic ahi es suyo.
            // Un recuadro con enlace es la puerta a otra hoja (las «zonas» de
            // una pagina del movil): pulsarlo cierra este lienzo y quien
            // llama abre el de al lado. Va antes que nada, porque si no el
            // gesto se lo lleva como una seleccion cualquiera.
            // Con CUALQUIER herramienta: una zona enlazada es un boton, no una
            // figura. Pedir la mano lo hacia inalcanzable, porque el editor
            // abre con el lapiz.
            // El aviso de un enlace roto se va con el primer aviso del raton
            // pasado su tiempo: sin repintar, se quedaria puesto.
            if aviso_enlace
                .as_ref()
                .is_some_and(|(_, d)| d.elapsed() >= AVISO_ENLACE_DURA)
            {
                aviso_enlace = None;
                interfaz_sucia = true;
                ventana.invalidar();
            }
            if let EventoOverlay::BotonPulsado(p) = ev
                // Las hermanas de un grupo de la barra caen sobre el dibujo:
                // un clic en ellas no es en el enlace de debajo.
                && matches!(caja.destino(p), DestinoClic::Lienzo)
                && let Some(q) = a_evento(
                    &ev,
                    &efectiva,
                    Punto {
                        x: area.x,
                        y: area.y,
                    },
                )
                .and_then(|g| match g {
                    pixpin_motor2d::gesto::EventoGesto::Pulsar { p, .. } => Some(p),
                    _ => None,
                })
            {
                let _ = p;
                // Su icono (`enlaceBajoElDedo` del movil), que cae fuera del
                // recuadro, o el propio recuadro.
                let destino = pixpin_motor2d::zona::enlace_bajo_el_puntero(
                    &escena.elementos,
                    q,
                    efectiva.zoom,
                )
                .map(str::to_string)
                .or_else(|| {
                    pixpin_motor2d::impacto::elemento_en(&escena.elementos, q)
                        .and_then(|id| escena.buscar(id))
                        .and_then(|e| e.enlace.clone())
                });
                if let Some(destino) = destino {
                    // Se cierra SOLO si hay adonde ir: antes se cerraba igual
                    // y, si el enlace no llevaba a nada, el usuario se quedaba
                    // fuera del lienzo sin el otro.
                    match hoja_del_chat.as_ref().and_then(|h| {
                        crate::salto_por_enlace::dibujo_del_enlace(&h.raiz, &h.proyecto, &destino)
                    }) {
                        Some(dibujo) => {
                            tracing::info!(%destino, %dibujo, "enlace a otra hoja");
                            enlace_pedido = Some(dibujo);
                            break 'bucle;
                        }
                        None => {
                            tracing::warn!(%destino, "el enlace no lleva a ningun lienzo; el lienzo sigue abierto");
                            aviso_enlace = Some((
                                marcas::textos().t("zona-enlace-roto"),
                                std::time::Instant::now(),
                            ));
                            interfaz_sucia = true;
                            ventana.invalidar();
                            continue;
                        }
                    }
                }
            }
            // El panel lateral, la caja y el texto que se esta escribiendo
            // (`dibujo::mano`): un clic en la caja ELIGE y no puede llegar
            // ademas al gesto como si fuera un trazo, y escribiendo, una «r»
            // es una erre y no la herramienta.
            let hecho = mano.interfaz(
                &ev,
                &mut gesto,
                &mut escena,
                // Presentando o solo mirando no hay barra ni panel (G5).
                (presentacion.is_none() && !solo_mirar).then_some(&caja),
                presentacion.is_none() && !solo_mirar,
                crate::dibujo::mano::Vista {
                    camara: &efectiva,
                    area,
                    interfaz: modo_ventana::area_del_panel(area_ui, caja.marco, escala_por_cien),
                    escala_por_cien,
                },
            );
            // Las hermanas del grupo que la mano tenga abierto: la caja las
            // pinta y las cuenta como suyas al decir que hay bajo el raton.
            caja = caja.con_desplegado(mano.desplegado);
            if hecho.salir {
                break 'bucle;
            }
            // El clic a traves de la barra del anotador: lo mismo que el boton
            // de la pastilla (y que Espacio), que es desde donde se vuelve.
            if matches!(hecho.pedido, Some(crate::dibujo::mano::Pedido::Atravesar)) {
                accion_pastilla = Some(pastilla_pantalla::Accion::Atravesar);
                continue;
            }
            // F11: imprimir de la barra va por donde Ctrl+P (`pedida`, abajo),
            // y compartir por donde Ctrl+Mayus+S.
            let de_la_barra = match hecho.pedido {
                Some(crate::dibujo::mano::Pedido::Imprimir) => Some(exportar::Peticion::Imprimir),
                Some(crate::dibujo::mano::Pedido::Compartir) => Some(exportar::Peticion::Compartir),
                _ => None,
            };
            let imprimir_de_la_barra = de_la_barra.is_some();
            // F14: la imagen y las figuras de la barra (`figuras.rs`).
            let de_prueba = figuras_al_abrir
                .take_if(|t| std::time::Instant::now() >= *t)
                .map(|_| {
                    crate::dibujo::mano::Pedido::Figuras(pixpin_geom::Punto { x: 200, y: 160 })
                });
            if let Some(pedido) = hecho.pedido.filter(|_| !imprimir_de_la_barra).or(de_prueba) {
                let ed = figuras::Editor {
                    ventana: &ventana,
                    dispositivo: &dispositivo,
                    motor: &mut motor,
                    superficie: &superficie,
                    escena: &mut escena,
                    camara: &efectiva,
                    gesto: &mut gesto,
                    cache: &mut cache,
                    cache_tinta: &mut cache_tinta,
                    rejilla: &mut rejilla,
                    capa: &capa,
                    fondo: &mut fondo,
                    imagenes: &mut imagenes,
                    caja: &caja,
                    corrimiento_ui,
                    escala_por_cien,
                    ancho_px,
                    alto_px,
                };
                match pedido {
                    crate::dibujo::mano::Pedido::Imagen => figuras::meter_imagen(ed),
                    crate::dibujo::mano::Pedido::Figuras(p) => {
                        figuras::atender_figuras(ed, p, exportar::textos())
                    }
                    crate::dibujo::mano::Pedido::GuardarFigura => {
                        figuras::guardar_seleccion(ed, exportar::textos())
                    }
                    crate::dibujo::mano::Pedido::Imprimir
                    | crate::dibujo::mano::Pedido::Compartir
                    | crate::dibujo::mano::Pedido::Atravesar => false,
                };
                todo_sucio = true;
                contenido_sucio = true;
                interfaz_sucia = true;
                ventana.invalidar();
                continue;
            }
            if hecho.repinte.algo() {
                todo_sucio |= hecho.repinte.todo;
                contenido_sucio |= hecho.repinte.contenido;
                ventana.invalidar();
            }
            if hecho.consumido && !imprimir_de_la_barra {
                continue;
            }
            // Escape (`al_escape`): a pantalla completa vuelve a ventana, y
            // en ventana cierra el lienzo (guardando, como Salir), que es lo
            // que espera el usuario de una ventana sin marco. Lo que esta a
            // medias manda antes: el texto lo cerro ya `mano` (y se consumio)
            // y un arrastre lo cancela el motor, abajo. Volver a ventana se
            // recuerda como con F11: va por el mismo `alternar_modo`.
            // Editando, Escape vuelve a mirar (y a la ventana si de alli vino).
            if let EventoOverlay::Tecla { vk: VK_ESCAPE, .. } = ev
                && mirar_editar::escape_vuelve_a_mirar(
                    !solo_mirar,
                    pantalla.is_none(),
                    presentacion.is_none(),
                    gesto.en_reposo(),
                )
            {
                if poner_solo_mirar(
                    true,
                    &mut solo_mirar,
                    &mut gesto,
                    &mut escena,
                    &mut herramienta_al_editar,
                ) {
                    cambio_de_mirar = Some(false);
                }
                arrastre_mirar = None;
                todo_sucio = true;
                contenido_sucio = true;
                interfaz_sucia = true;
                ventana.invalidar();
                continue;
            }
            if let EventoOverlay::Tecla { vk: VK_ESCAPE, .. } = ev {
                match al_escape(
                    pantalla.is_none(),
                    presentacion.is_none(),
                    gesto.en_reposo(),
                    pin.as_ref().is_some_and(|p| p.pantalla_completa),
                ) {
                    AlEscape::Cerrar => break 'bucle,
                    AlEscape::AVentana => {
                        alternar_modo = true;
                        continue;
                    }
                    AlEscape::Seguir => {}
                }
            }

            if let EventoOverlay::Tecla { vk: VK_F11, .. } = ev
                && pantalla.is_none()
                && presentacion.is_none()
            {
                // Sin cerrar la ventana: se hace despues de la cola
                // (`alternar_modo`), con los demas cambios de tamano.
                alternar_modo = true;
                continue;
            }
            // **Exportar, copiar como imagen e imprimir** (G2, F11): atajos
            // del editor y el menu del clic derecho, que el lienzo no usaba.
            // Antes de la tabla del motor y del portapapeles: el `Ctrl+C` de
            // copiar elementos no mira Mayus y se comeria `Ctrl+Mayus+C`.
            // En el anotador de pantalla no: lo que se guarda alli es la foto de
            // la pantalla anotada, y exportar el dibujo suelto confundiria.
            let pedida = match ev {
                _ if pantalla.is_some() || presentacion.is_some() => None,
                _ if imprimir_de_la_barra => de_la_barra,
                EventoOverlay::Tecla {
                    vk,
                    ctrl,
                    shift,
                    alt,
                    ..
                } => exportar::atajo(vk, ctrl, shift, alt),
                EventoOverlay::BotonDerechoPulsado(p) => exportar::menu(ventana.handle(), p.x, p.y),
                _ => None,
            };
            if let Some(peticion) = pedida {
                let fotos = |id: u64| imagenes.rgba(id);
                let lienzo = exportar::Lienzo {
                    escena: &escena,
                    seleccion: gesto.seleccion.ids(),
                    papel: fondo
                        .as_ref()
                        .and_then(|f| f.imagen().map(|i| (i, f.ancho(), f.alto()))),
                    fotos: &fotos,
                    nombre: "lienzo".into(),
                };
                exportar::atender_en_el_editor(peticion, ventana.handle(), &lienzo);
                todo_sucio = true;
                ventana.invalidar();
                continue;
            }
            if matches!(ev, EventoOverlay::BotonDerechoPulsado(_)) {
                // El menu pudo abrir el selector del papel (`exportar::menu`).
                // O pedir la imagen de referencia, que se pone o se quita aqui.
                if referencia::tomar_pedido() {
                    referencia::alternar(
                        &mut la_referencia,
                        ventana.handle(),
                        dispositivo.d3d(),
                        ventana.area(),
                        escala_por_cien,
                    );
                }
                todo_sucio = true;
                ventana.invalidar();
                // «Pegar» del menu: es Ctrl+V, por el mismo camino (tabla de
                // Excel, lo copiado dentro, imagen o texto), y el raton sigue
                // donde se abrio el menu, que es donde nace lo pegado.
                if !exportar::tomar_pegar() {
                    continue;
                }
                ev = EventoOverlay::Tecla {
                    vk: b'V' as u32,
                    ctrl: true,
                    shift: false,
                    alt: false,
                };
            }
            // **Las herramientas** (`dibujo::mano`): la tabla de atajos del
            // motor, las letras, la pluma, el portapapeles, el lazo, el
            // cuentagotas, la goma y el gesto con las cuatro de construir.
            // Son las mismas en todos los anfitriones; aqui solo queda lo que
            // es del lienzo: como se repinta lo que cambio.
            //
            // Para `hornear_trazo`: si la escena estaba al dia ANTES de este
            // aviso (lo unico que la ensucia es el propio aviso).
            let escena_al_dia_antes = !todo_sucio && !contenido_sucio;
            // F12: Ctrl+V con unas celdas de Excel copiadas pega la TABLA
            // dibujada (el portapapeles de Windows trae `CF_UNICODETEXT` con
            // tabuladores). Solo si no hay nada copiado dentro, que manda; y
            // si el texto no es una tabla, Ctrl+V sigue su camino.
            if let EventoOverlay::Tecla {
                vk,
                ctrl: true,
                shift: false,
                ..
            } = ev
                && vk == b'V' as u32
                && pantalla.is_none()
                && !mano.copiado_vigente()
                && !gesto.esta_escribiendo()
                && let Some(pixpin_codec::portapapeles::ContenidoPortapapeles::Texto(t)) =
                    pixpin_codec::portapapeles::leer()
                && figuras::pegar_tabla(
                    // El HTML trae las celdas combinadas y la negrita.
                    pixpin_codec::portapapeles::tabla::leer_tabla()
                        .and_then(|(html, _)| html)
                        .as_deref(),
                    &t,
                    &mut escena,
                    &mut gesto,
                    figuras::centro_de_la_vista(&efectiva, ancho_px, alto_px),
                    &figuras::medidor(&motor, pixpin_motor2d::texto::FAMILIA_DEL_SISTEMA),
                )
            {
                todo_sucio = true;
                contenido_sucio = true;
                ventana.invalidar();
                continue;
            }
            // F12: Intro con una tabla elegida abre su cajetin (celdas, filas,
            // columnas y cabecera).
            if let EventoOverlay::Tecla {
                vk: 0x0D,
                ctrl: false,
                shift: false,
                alt: false,
            } = ev
                && pantalla.is_none()
                && !gesto.esta_escribiendo()
                && gesto.en_reposo()
                && gesto.seleccion.cuantos() > 1
                && figuras::TablaElegida::de(&escena, &gesto) != figuras::TablaElegida::Ninguna
            {
                let ed = figuras::Editor {
                    ventana: &ventana,
                    dispositivo: &dispositivo,
                    motor: &mut motor,
                    superficie: &superficie,
                    escena: &mut escena,
                    camara: &efectiva,
                    gesto: &mut gesto,
                    cache: &mut cache,
                    cache_tinta: &mut cache_tinta,
                    rejilla: &mut rejilla,
                    capa: &capa,
                    fondo: &mut fondo,
                    imagenes: &mut imagenes,
                    caja: &caja,
                    corrimiento_ui,
                    escala_por_cien,
                    ancho_px,
                    alto_px,
                };
                figuras::editar_tabla(ed, exportar::textos());
                todo_sucio = true;
                contenido_sucio = true;
                interfaz_sucia = true;
                ventana.invalidar();
                continue;
            }
            // Apuntando una lupa a otro sitio (`dibujo::lupa`) solo cambia
            // ella: donde estaba antes de este aviso, para rehacer ese trozo.
            let apuntaba = mano.lupa.ocupada();
            let lupa_antes = lupas::huella_elegida(&escena, gesto.seleccion.ids(), &efectiva);
            let hecho = mano.herramienta(
                &ev,
                &mut gesto,
                &mut escena,
                Some(&mut imagenes),
                crate::dibujo::mano::Vista {
                    camara: &efectiva,
                    area,
                    interfaz: modo_ventana::area_del_panel(area_ui, caja.marco, escala_por_cien),
                    escala_por_cien,
                },
                ancho_px,
                alto_px,
            );
            let apunta = mano.lupa.ocupada();
            // **La lupa, sobre la capa congelada** (queja: «la lupa se mueve
            // lento»): al cogerla, lo demas se hornea una vez sin ella; en
            // cada aviso se rehace solo su trozo, el de antes y el de ahora.
            // Antes cada aviso pintaba la escena entera.
            let lupa_en_zona = match (apuntaba, apunta) {
                (false, true) if con_capas && !tinta_viva => {
                    congelar::hornear(
                        &mut motor,
                        &mut capa,
                        &escena,
                        &gesto,
                        &mut rejilla,
                        &mut cache,
                        &mut cache_tinta,
                        &mut fondo,
                        &mut imagenes,
                        &efectiva,
                        ancho_px,
                        alto_px,
                        margen_escena,
                        &excluidos_de(&gesto),
                    );
                    false
                }
                (true, true) if capa.lista() => {
                    let ahora = lupas::huella_elegida(&escena, gesto.seleccion.ids(), &efectiva);
                    for caja in [lupa_antes, ahora].into_iter().flatten() {
                        sucio = Some(match sucio {
                            None => caja,
                            Some(s) => (
                                s.0.min(caja.0),
                                s.1.min(caja.1),
                                s.2.max(caja.2),
                                s.3.max(caja.3),
                            ),
                        });
                    }
                    ventana.invalidar();
                    true
                }
                // Al soltarla, la capa fuera y un fotograma entero limpio.
                (true, false) => {
                    capa.soltar();
                    false
                }
                _ => false,
            };
            // El laser con capas no repinta la escena: lo pinta el fotograma
            // en la capa de la tinta (`pintar_laser_en_capa`).
            let laser_en_su_capa = con_capas && !tinta_viva && al_vuelo(&gesto);
            if hecho.repinte.algo() && laser_en_su_capa {
                ventana.invalidar();
            } else if hecho.repinte.algo() && !lupa_en_zona {
                todo_sucio |= hecho.repinte.todo;
                contenido_sucio |= hecho.repinte.contenido;
                ventana.invalidar();
            }
            // F8: se solto la Zona. La foto sale del papel y lo dibujado
            // dentro, recortada en redondo, y queda encima elegida.
            if let Some(caja_zona) = mano.zona_pedida.take() {
                // Con «Al chat», la foto sin redondear va al chat del
                // proyecto y aqui queda la marca con enlace (`mandarLaZona`).
                let al_chat = hoja_del_chat
                    .as_ref()
                    .filter(|_| pastilla_zona.manda_al_chat(true));
                let foto = {
                    let fotos = |id: u64| imagenes.rgba(id);
                    let lienzo = exportar::Lienzo {
                        escena: &escena,
                        seleccion: &[],
                        papel: fondo
                            .as_ref()
                            .and_then(|f| f.imagen().map(|i| (i, f.ancho(), f.alto()))),
                        fotos: &fotos,
                        nombre: "zona".into(),
                    };
                    if al_chat.is_some() {
                        figuras::foto_cruda_de_la_zona(&lienzo, caja_zona)
                    } else {
                        figuras::foto_de_la_zona(&lienzo, caja_zona)
                    }
                };
                if let (Some(foto), Some(h)) = (foto.as_ref(), al_chat) {
                    let dicho = crate::zona_al_chat::mandar_y_marcar(
                        &mut escena,
                        foto,
                        caja_zona,
                        h,
                        marcas::textos(),
                    );
                    pastilla_zona.dicho = Some(dicho.unwrap_or_else(|e| e));
                    interfaz_sucia = true;
                } else if let Some(foto) = foto {
                    pastilla_zona.dicho = None;
                    figuras::poner_copia_de_zona(
                        &mut escena,
                        &mut gesto,
                        &mut imagenes,
                        foto,
                        caja_zona,
                        efectiva.zoom,
                    );
                }
                todo_sucio = true;
                contenido_sucio = true;
                ventana.invalidar();
            }
            if hecho.consumido && hecho.paso.is_none() {
                continue;
            }
            if let Some(paso) = hecho.paso {
                let crate::dibujo::mano::Paso {
                    r,
                    pulsado,
                    construido,
                    en_reposo_antes,
                    habia_eleccion,
                } = paso;
                ventana.poner_cursor(forma_de(r.cursor));
                if pulsado.is_some() {
                    caja_al_pulsar = gesto.seleccion.caja(&escena);
                }
                // **Lo que se arrastra va en su capa, y la escena no se
                // toca** (ver `pintar_seleccion_en_capa`). `compuesto` dice
                // que este aviso ya esta atendido y que la escena no se
                // repinta por el.
                let compuesto = atender_en_capa(
                    &mut en_capa,
                    &mut vaciar_capa_tras_pintar,
                    &motor,
                    &superficie,
                    &escena,
                    &efectiva,
                    &gesto,
                    &mut cache,
                    &imagenes,
                    ancho_px,
                    alto_px,
                    con_capas && !tinta_viva,
                    camara_pintada == efectiva && !superficie.esta_estirada(),
                    caja_al_pulsar,
                );
                match compuesto {
                    Compuesto::No => {}
                    Compuesto::Si => {}
                    Compuesto::EscenaEntera => {
                        todo_sucio = true;
                        contenido_sucio = true;
                        ventana.invalidar();
                    }
                    Compuesto::Zona(z) => {
                        zona_por_rehacer = Some(match zona_por_rehacer {
                            None => z,
                            Some(a) => (a.0.min(z.0), a.1.min(z.1), a.2.max(z.2), a.3.max(z.3)),
                        });
                        ventana.invalidar();
                    }
                }
                // `VentanaOverlay::invalidar` no toma una region: invalida
                // la ventana entera para que Windows mande `WM_PAINT`, pero
                // la zona que de verdad hay que pintar la lleva `sucio`
                // (D125): es lo que reduce a `Superficie::
                // presentar_sincronizado` cuanto tiene que recomponer DWM.
                if compuesto == Compuesto::No {
                    match r.region {
                        Region::Nada => {}
                        Region::Caja(x0, y0, x1, y1) => {
                            let a = efectiva.a_pantalla(Punto2::nuevo(x0, y0));
                            let b = efectiva.a_pantalla(Punto2::nuevo(x1, y1));
                            let caja = (a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y));
                            sucio = Some(match sucio {
                                None => caja,
                                Some(s) => (
                                    s.0.min(caja.0),
                                    s.1.min(caja.1),
                                    s.2.max(caja.2),
                                    s.3.max(caja.3),
                                ),
                            });
                            ventana.invalidar();
                        }
                        Region::Todo => {
                            // Estirando o girando: solo lo elegido, su trozo de
                            // antes y el de ahora, sobre la capa congelada.
                            let ahora = congelar::zona_al_transformar(
                                &escena, &gesto, &efectiva, &mut cache,
                            );
                            match (ahora, zona_transformada) {
                                (Some(z), Some(a)) if capa.lista() => {
                                    let caja = (
                                        a.0.min(z.0) as f32,
                                        a.1.min(z.1) as f32,
                                        a.2.max(z.2) as f32,
                                        a.3.max(z.3) as f32,
                                    );
                                    sucio = Some(match sucio {
                                        None => caja,
                                        Some(s) => (
                                            s.0.min(caja.0),
                                            s.1.min(caja.1),
                                            s.2.max(caja.2),
                                            s.3.max(caja.3),
                                        ),
                                    });
                                }
                                _ => {
                                    todo_sucio = true;
                                    contenido_sucio = true;
                                }
                            }
                            zona_transformada = ahora;
                            ventana.invalidar();
                        }
                    }
                }
                if !gesto.transformando() {
                    zona_transformada = None;
                }
                // La capa estatica vive mientras algo cambia en cada
                // fotograma: mover, escalar o girar la seleccion (como
                // antes) y, desde E1, dibujar a mano (D120). Lo excluido se
                // repinta encima; lo demas se copia de la capa.
                //
                // Con la seleccion arrastrandose en su capa no hace falta:
                // hornearla costaba de 20 a 194 ms justo al empezar a mover,
                // el tiron que se notaba al coger una figura.
                let excluidos = excluidos_de(&gesto);
                let activo_ahora = !gesto.en_reposo() && !excluidos.is_empty() && en_capa.is_none();
                // **B2: el trazo vivo en su propia capa.** Mientras dura, la
                // escena no se toca: ni se hornea al apoyar el lapiz
                // (`capa.preparar`: 20 ms con 200 elementos y 194 ms con
                // 10.000, medidos a 3000 x 2000, mas 33,7 MB de video), ni
                // se copia por fotograma (D148), ni se presenta.
                //
                // Va con las capas de A3 porque necesita su arbol de
                // visuales: `[rendimiento] paneo_por_composicion = false` y
                // PIXPIN_TINTA_CLASICA apagan las dos cosas a la vez, que es
                // lo que hace falta para comparar en el mismo equipo.
                //
                // Y solo entra si lo que hay en la superficie de la escena
                // es ESTA camara: recien desplazada por composicion, lo que
                // se ve es la textura corrida y el trazo saldria donde no
                // es. En ese caso se fuerza el fotograma nitido y la capa
                // entra en la vuelta siguiente.
                let quiere_tinta = TINTA_EN_CAPA && con_capas && gesto.trazo_en_curso().is_some();
                //
                // Tampoco entra mientras la capa guarde algo que la escena
                // todavia no pinta: el trazo anterior esperando su horneado,
                // o lo que quedo de un arrastre. `encender_tinta` no la
                // vaciaria (ya esta encendida) y al soltar este trazo se
                // borraria tambien aquello. Se pide la escena entera, que lo
                // recoge y vacia la capa, y el trazo entra en el aviso
                // siguiente.
                if quiere_tinta && !tinta_viva && trazo_por_hornear.take().is_some() {
                    vaciar_capa_tras_pintar = true;
                }
                let capa_ocupada = vaciar_capa_tras_pintar && en_capa.is_none();
                let tinta_ahora = quiere_tinta
                    && camara_pintada == efectiva
                    && !superficie.esta_estirada()
                    && !capa_ocupada;
                if quiere_tinta && !tinta_ahora && !tinta_viva {
                    todo_sucio = true;
                    contenido_sucio = true;
                    ventana.invalidar();
                }
                if tinta_ahora && !tinta_viva {
                    tinta_viva = superficie.encender_tinta(&motor).is_ok();
                    // Si el trazo empezo con una capa congelada ya horneada
                    // (por el fotograma nitido de arriba), sus megas no
                    // pintan nada mientras la tinta vive en su capa.
                    if tinta_viva {
                        capa.soltar();
                        zona_tinta_anterior = None;
                        // ¿Nace sobre una escena al dia? Solo si este aviso
                        // es el que lo hizo nacer y no hizo nada mas: la
                        // escena estaba pintada, no habia marco de eleccion
                        // que quitar, y construir no toco nada. Entonces el
                        // `Region::Todo` del pulsar es SOLO «hay un elemento
                        // nuevo», y ese elemento lo pinta la capa de la
                        // tinta: la escena no tiene nada que rehacer.
                        let solo_nacio = pulsado.is_some()
                            && escena_al_dia_antes
                            && !habia_eleccion
                            && !construido
                            && compuesto == Compuesto::No;
                        trazo_limpio = if solo_nacio {
                            gesto.trazo_en_curso()
                        } else {
                            None
                        };
                        if trazo_limpio.is_some() {
                            todo_sucio = false;
                            contenido_sucio = false;
                        }
                    }
                }
                if !tinta_ahora && tinta_viva {
                    tinta_viva = false;
                    // Al soltar -o al convertirse en forma rapida-: el trazo
                    // ya esta en la escena. Si nacio sobre la escena al dia y
                    // nada la ha tocado desde entonces, se hornea SOLO el
                    // (`hornear_trazo`): recorrer la escena entera para
                    // anadir un trazo encima era lo que hacia que soltar el
                    // lapiz costara mas cuanto mas habia dibujado.
                    //
                    // Tiene que ser el ultimo elemento: lo de encima de el
                    // habria que repintarlo tambien.
                    let soltado = trazo_limpio.take().filter(|id| {
                        gesto.en_reposo()
                            && escena_al_dia_antes
                            && !construido
                            && compuesto == Compuesto::No
                            && escena
                                .elementos
                                .last()
                                .is_some_and(|e| e.id == *id && !e.borrado)
                    });
                    match soltado {
                        Some(id) => {
                            trazo_por_hornear = Some(id);
                            todo_sucio = false;
                            contenido_sucio = false;
                            sucio = None;
                        }
                        // Lo de siempre: el fotograma de la escena entera.
                        // La capa se vacia DESPUES de presentarlo: vaciarla
                        // ya dejaba una composicion sin el trazo en ningun
                        // sitio, un parpadeo justo al soltar.
                        None => {
                            vaciar_capa_tras_pintar = true;
                            todo_sucio = true;
                            contenido_sucio = true;
                        }
                    }
                    ventana.invalidar();
                }
                if activo_ahora && en_reposo_antes && !tinta_ahora {
                    congelar::hornear(
                        &mut motor,
                        &mut capa,
                        &escena,
                        &gesto,
                        &mut rejilla,
                        &mut cache,
                        &mut cache_tinta,
                        &mut fondo,
                        &mut imagenes,
                        &efectiva,
                        ancho_px,
                        alto_px,
                        margen_escena,
                        &excluidos,
                    );
                } else if !activo_ahora && capa.lista() {
                    capa.soltar();
                }
                // El cajetin de calibrar: abre su propio bucle de eventos y
                // no vuelve hasta que se acepta con Enter o se cancela con
                // Escape. Si se cancela, `pedir_medida` devuelve `None` y no
                // se llama a `calibrar`: el lienzo queda exactamente como
                // estaba (cancelar no puede cambiar la escala).
                if let Some(Peticion::Calibrar { largo_px }) = r.pide {
                    rejilla.sincronizar(&escena);
                    if let Some((valor, unidad)) = pedir_medida(
                        &ventana,
                        &mut motor,
                        &superficie,
                        &escena,
                        &efectiva,
                        &gesto,
                        &mut cache,
                        &mut cache_tinta,
                        &rejilla,
                        &capa,
                        &mut fondo,
                        &mut imagenes,
                        &caja,
                        corrimiento_ui,
                        escala_por_cien,
                        ancho_px,
                        alto_px,
                        largo_px,
                    ) {
                        gesto.calibrar(&mut escena, largo_px, valor, &unidad);
                    }
                    ventana.invalidar();
                    todo_sucio = true;
                    contenido_sucio = true;
                }
                // El cajetin de la cota (`cota.rs`): largo y angulo de la
                // raya recien trazada. Cancelar la deja como se trazo.
                if let Some(Peticion::DictarCota { id }) = r.pide {
                    rejilla.sincronizar(&escena);
                    if let Some((largo, grados)) = cota::dictar(
                        &ventana,
                        &mut motor,
                        &superficie,
                        &escena,
                        &efectiva,
                        &gesto,
                        &mut cache,
                        &mut cache_tinta,
                        &rejilla,
                        &capa,
                        &mut fondo,
                        &mut imagenes,
                        &caja,
                        corrimiento_ui,
                        escala_por_cien,
                        ancho_px,
                        alto_px,
                        id,
                    ) {
                        Gesto::dictar_cota(&mut escena, id, largo, grados);
                    }
                    ventana.invalidar();
                    todo_sucio = true;
                    contenido_sucio = true;
                }
            }
            // 2. Pintar solo cuando lo pide la ventana: marca que hace falta
            // un fotograma, no pinta aqui mismo. Pintar a mitad de la cola
            // era lo que hacia perder puntos de un trazo rapido.
            if matches!(ev, EventoOverlay::Pintar) {
                hay_que_pintar = true;
            }
            if matches!(ev, EventoOverlay::Cerrar) {
                break 'bucle;
            }
        }
        // **El lienzo en ventana** (`modo_ventana`): F11 o el boton de la
        // barra lo pasan a pantalla completa o de vuelta, y la ventana se
        // pudo estirar, llevar o cambiar desde fuera (AltSnap, Win+flecha).
        // Mientras la tinta o lo elegido viven en su capa no se toca nada:
        // se hace en cuanto se suelten.
        if let Some(pn) = pin.as_mut() {
            // Editar se hace donde se este (25-oct-2026, el usuario: «si esta
            // en ventana, la edicion se hace en ventana; si esta en pantalla
            // completa, en pantalla completa»): el lapiz ya no cambia el modo,
            // para eso esta su boton. A pantalla completa editando no hay
            // barra del pin; en ventana si, que sin ella no hay por donde
            // llevarla.
            let _ = cambio_de_mirar.take();
            pn.quitar_barra(!solo_mirar && pn.pantalla_completa);
            if std::mem::take(&mut alternar_modo) {
                pn.alternar_pantalla_completa(&mut ventana);
                modo_ventana::recordar(modo_ventana::Recordado {
                    pantalla_completa: pn.pantalla_completa,
                    ventana: Some(pn.area_pin()),
                });
            }
            pn.seguir_ventana(&mut ventana);
            if pn.area != area && !tinta_viva && en_capa.is_none() {
                let antes = area;
                area = pn.area;
                // Todo lo de la interfaz va en coordenadas del escritorio:
                // la barra de herramientas, a su sitio nuevo.
                area_ui = area;
                corrimiento_ui = (-(area.x as f32), -(area.y as f32));
                caja = CajaHerramientas::barra_superior(
                    area_ui,
                    escala_por_cien,
                    crate::dibujo::permitidas::botones(anfitrion),
                )
                .con_desplegado(mano.desplegado);
                // Llevada sin cambiar de tamano, lo pintado se va con la
                // ventana: no hay nada que repintar.
                if (antes.ancho, antes.alto) != (area.ancho, area.alto) {
                    camara = modo_ventana::camara_al_cambiar(camara, efectiva.zoom, antes, area);
                    efectiva = vista_efectiva(&camara, escala_por_cien);
                    (ancho_px, alto_px) = (area.ancho as f32, area.alto as f32);
                    superficie.apagar_tinta(&motor);
                    if let Err(e) = superficie.cambiar_tamano(area.ancho, area.alto) {
                        tracing::warn!(?e, "la superficie del lienzo no cambio de tamano");
                    }
                    capa.soltar();
                    todo_sucio = true;
                    contenido_sucio = true;
                    trasero_distinto = None;
                    zona_anterior = None;
                    hay_que_pintar = true;
                    ventana.invalidar();
                }
                interfaz_sucia = con_capas;
            }
            pn.actualizar(&motor, &nombre_del_pin, &[]);
        }
        let vaciar = t_vuelta.elapsed();
        // **Las acciones de la pastilla del anotador** (`pastilla_pantalla`),
        // tambien Alt + doble clic central, que alterna el clic a traves.
        if let Some(accion) = accion_pastilla.take()
            && let Some(pa) = pantalla.as_deref_mut()
        {
            use pastilla_pantalla::Accion;
            if gesto.esta_escribiendo() {
                gesto.cerrar_texto(&mut escena);
            }
            match accion {
                Accion::Salir => break 'bucle,
                Accion::Atravesar => {
                    pasante_fijo = !pasante_fijo;
                    ventana.poner_pasante(pasante_fijo);
                    // Al volver a dibujar se recupera el foco (como Espacio).
                    if !pasante_fijo {
                        ventana.enfocar();
                    }
                    if let Some(ps) = pastilla.as_mut() {
                        ps.poner_pasante(pasante_fijo);
                    }
                    tracing::info!(pasante = pasante_fijo, "el anotador cambia de modo");
                }
                Accion::Limpiar => {
                    gesto.seleccion.limpiar();
                    gesto.lazo = None;
                    let borrados = pastilla_pantalla::limpiar(&mut escena);
                    tracing::info!(borrados, "anotador limpio (Ctrl+Z lo devuelve)");
                    capa.soltar();
                    contenido_sucio = true;
                }
                // «Guardar ahora»: lo mismo que al salir, sin salir. Lo que se
                // dibuje despues pone al dia ESE lienzo (al salir o al volver
                // a pulsarlo), no manda otro mensaje.
                Accion::Guardar => {
                    let tinta = crate::anotador_al_chat::tinta_de(&escena);
                    let fondo_ref = fondo.as_ref();
                    let ps = pastilla.as_ref();
                    pantalla::guardar_en_el_chat(
                        &mut chat,
                        pa,
                        tinta,
                        crate::anotador_al_chat::pegadas_de(&escena, |id| {
                            imagenes.rgba(id).cloned()
                        }),
                        pantalla::globo(exportar::textos()),
                        |pa| pantalla::foto_de_debajo(pa, fondo_ref, &ventana, ps, !pasante_fijo),
                    );
                }
                Accion::Copiar => {
                    // Como la foto de al salir: la pantalla con lo anotado y
                    // SIN barra, panel, marco de lo elegido ni pastilla.
                    gesto.seleccion.limpiar();
                    gesto.lazo = None;
                    if let Some(ps) = pastilla.as_ref() {
                        ps.ocultar();
                    }
                    superficie.apagar_tinta(&motor);
                    rejilla.sincronizar(&escena);
                    capa.soltar();
                    let _ = pintar(
                        &mut motor,
                        &superficie,
                        &escena,
                        &efectiva,
                        &gesto,
                        &mut cache,
                        &mut cache_tinta,
                        &rejilla,
                        &capa,
                        &mut fondo,
                        &mut imagenes,
                        &caja,
                        None,
                        corrimiento_ui,
                        false,
                        escala_por_cien,
                        ancho_px,
                        alto_px,
                        None,
                        None,
                        None,
                        true,
                        FueraDeLaEscena::default(),
                        None,
                        |_, _| {},
                    );
                    superficie.dejar_de_estirar();
                    pixpin_shell::esperar_composicion();
                    let foto = (pa.capturar)();
                    if let Some(ps) = pastilla.as_ref() {
                        ps.mostrar();
                    }
                    // Lo que esperaba en la capa de la tinta ya esta en la
                    // escena, y el fotograma de abajo lo rehace entero.
                    trazo_por_hornear = None;
                    trazo_limpio = None;
                    contenido_sucio = true;
                    let copiado = foto.map(|f| pixpin_codec::copiar_imagen(&f));
                    tracing::info!(
                        ok = matches!(copiado, Some(Ok(()))),
                        "anotador: copiado al portapapeles"
                    );
                    if matches!(copiado, Some(Ok(()))) && pa.avisos != 0 {
                        let t = exportar::textos();
                        let _ = pixpin_shell::aviso::Aviso::sobre_la_bandeja(
                            windows::Win32::Foundation::HWND(pa.avisos as *mut _),
                        )
                        .mostrar(
                            &t.t("anotador-globo-titulo"),
                            &t.t("anotador-globo-copiado"),
                        );
                    }
                }
            }
            interfaz_sucia = true;
            todo_sucio = true;
            ventana.invalidar();
        }
        if let Some(ps) = pastilla.as_mut() {
            ps.al_dia(&motor);
        }
        // F5: el viaje hasta una marca, con el reloj. Solo se mueve la
        // camara: sin `contenido_sucio`, A3 lo resuelve corriendo el visual
        // mientras el colchon de.
        if marcador.avanzar(&mut camara) {
            efectiva = vista_efectiva(&camara, escala_por_cien);
            todo_sucio = true;
            ventana.invalidar();
        }
        let mut pintado_medido = None;
        // Un solo fotograma por vuelta, despues de haber pasado TODOS los
        // puntos al gesto: pintar a mitad de la cola era lo que hacia perder
        // puntos. Presentar con vsync bloquea hasta el refresco; mientras,
        // Windows guarda los movimientos y la Tarea 7 los recupera.
        // El gesto de pararse: el trazo a mano quieto con el boton pulsado
        // se convierte en compas (clavado sin moverse), rectangulo (una «L»),
        // linea, rectangulo o elipse, y lo que se arrastre despues la ajusta.
        if mano.forma_rapida(&mut gesto, &mut escena, efectiva.zoom) {
            todo_sucio = true;
            contenido_sucio = true;
            hay_que_pintar = true;
            ventana.invalidar();
        }
        // La lupa viva: su sesion de captura vive solo con la lupa puesta y
        // la ventana cogiendo el raton. Lo que cambia es donde estaba y
        // donde va (en la escena, sin capas) y la capa de la interfaz (con
        // capas, que es donde se pinta): nunca el fotograma entero.
        if let Some(l) = lupa.as_mut() {
            let puesta = gesto.herramienta == Herramienta::Lupa && !ventana.es_pasante();
            l.al_dia(puesta, &dispositivo, nivel, ventana.handle().0 as isize);
            if let Some(caja) = l.tomar_zona() {
                sucio = Some(match sucio {
                    None => caja,
                    Some(s) => (
                        s.0.min(caja.0),
                        s.1.min(caja.1),
                        s.2.max(caja.2),
                        s.3.max(caja.3),
                    ),
                });
                interfaz_sucia = true;
                hay_que_pintar = true;
                ventana.invalidar();
            }
        }
        // F14: la estela del laser se apaga sola, con el raton quieto: un
        // fotograma por vuelta mientras quede algo, y uno mas al acabar para
        // borrar el ultimo rastro.
        // La referencia se cerro desde su menu, o pidio copiarse.
        la_referencia.al_dia();
        let laser_ahora = gesto.laser.vivo();
        // Con capas, la estela va en la de la tinta y la escena no se toca
        // (`pintar_laser_en_capa`); al apagarse, se vacia la capa y la escena
        // se repinta una vez, por si se pinto mientras con la estela dentro.
        let laser_por_capa = con_capas && !tinta_viva;
        if laser_por_capa && al_vuelo(&gesto) {
            if !laser_en_capa {
                laser_en_capa = superficie.encender_tinta(&motor).is_ok();
            }
            if laser_en_capa {
                pintar_laser_en_capa(
                    &motor,
                    &superficie,
                    &efectiva,
                    &gesto,
                    &imagenes,
                    ancho_px,
                    alto_px,
                );
            }
        } else if laser_en_capa {
            superficie.apagar_tinta(&motor);
            laser_en_capa = false;
            todo_sucio = true;
            contenido_sucio = true;
            hay_que_pintar = true;
            ventana.invalidar();
        }
        if !laser_en_capa && (laser_ahora || laser_vivo) {
            todo_sucio = true;
            contenido_sucio = true;
            hay_que_pintar = true;
            ventana.invalidar();
        }
        laser_vivo = laser_ahora;
        // Un trazo soltado espera su horneado, pero despues algo mas ensucio
        // la escena (deshacer, una tecla, la camara...): ya no basta con
        // pintarlo a el encima. Se pinta la escena entera como siempre y la
        // capa de la tinta, que lo sigue ensenando, se vacia al presentarla.
        // Va ANTES de componer: con `contenido_sucio` puesto, A3 no corre el
        // visual de la escena dejando atras el trazo de la capa.
        if trazo_por_hornear.is_some() && (todo_sucio || contenido_sucio) {
            trazo_por_hornear = None;
            vaciar_capa_tras_pintar = true;
            todo_sucio = true;
            contenido_sucio = true;
            hay_que_pintar = true;
        }
        // Lo mismo con el trozo del arrastre: si algo mas ensucio la escena
        // (o hay otra zona pendiente de un gesto, o un trazo por hornear),
        // rehacer solo el trozo no bastaria. La escena entera lo recoge todo.
        if zona_por_rehacer.is_some()
            && (todo_sucio || contenido_sucio || sucio.is_some() || trazo_por_hornear.is_some())
        {
            zona_por_rehacer = None;
            todo_sucio = true;
            contenido_sucio = true;
            hay_que_pintar = true;
        }
        // **A3: componer en vez de pintar.** Si lo unico que cambio es desde
        // donde se mira y el colchon de la superficie da de si, el fotograma
        // se resuelve con una matriz en el visual de la escena: ni se
        // recorre la escena, ni se emite una primitiva, ni se presenta.
        //
        // `todo_sucio && sucio.is_none() && !contenido_sucio` es la forma
        // exacta de decir «la unica razon por la que hay que rehacer el
        // fotograma es que la camara se movio»: `sucio` lo pone un gesto
        // (un trazo en curso, por ejemplo) y `contenido_sucio` todo lo
        // demas.
        if con_capas && hay_que_pintar && todo_sucio && !contenido_sucio && sucio.is_none() {
            match transformada_de_camara(
                &camara_pintada,
                &efectiva,
                ancho_px,
                alto_px,
                margen_escena,
            ) {
                Some((s, dx, dy)) => {
                    superficie.estirar(s, s, dx, dy);
                    hay_que_pintar = false;
                    // `todo_sucio` se queda puesto: el fotograma nitido que
                    // llegue al reposo tiene que ser completo.
                    //
                    // El reloj se pone a cero en CADA composicion, no solo
                    // en la primera: lo que se espera son 150 ms sin
                    // MOVERSE, no 150 ms desde que empezo el arrastre.
                    camara_movida = Some(std::time::Instant::now());
                }
                // No cabe: se pinta
                // nitido ya, sin esperar al reposo.
                _ => camara_movida = None,
            }
        }
        // Quieta `REPOSO_CAMARA`: el fotograma nitido.
        if let Some(desde) = camara_movida
            && desde.elapsed() >= REPOSO_CAMARA
        {
            camara_movida = None;
            todo_sucio = true;
            hay_que_pintar = true;
        }
        if hay_que_pintar && !fotograma_listo {
            if let Some(s) = superficie.senal_fotograma() {
                fotograma_listo = pixpin_shell::overlay::senal_disparada(s);
            }
            let desde = *esperando_senal.get_or_insert_with(std::time::Instant::now);
            if desde.elapsed() >= ESPERA_MAXIMA_SENAL {
                fotograma_listo = true;
            }
        }
        if fotograma_listo || !hay_que_pintar {
            esperando_senal = None;
        }
        // **B2: el fotograma del trazo.** No pasa por `pintar`: no sincroniza
        // la rejilla, no recorre la escena, no copia la capa congelada y no
        // presenta. Es limpiar un rectangulo de la capa de tinta, pintar el
        // trazo dentro y un `Commit`.
        //
        // Sin `Present` no hay senal de latencia que marque el ritmo, asi que
        // lo marca el reloj: como mucho un fotograma por composicion de DWM.
        let mut espera_tinta = None;
        if tinta_viva && hay_que_pintar {
            let periodo = superficie
                .ritmo()
                .map_or(PERIODO_SUPUESTO_MS, |r| r.periodo_ms);
            espera_tinta = espera_de_tinta(
                ultimo_commit_tinta.elapsed().as_secs_f32() * 1000.0,
                periodo,
            );
        }
        if tinta_viva && hay_que_pintar && espera_tinta.is_none() {
            let t_pintar = std::time::Instant::now();
            let prediccion =
                prediccion_de(&gesto, &mut mano.predictor, tinta_clasica, efectiva.zoom);
            let holgura = if prediccion.is_some() || habia_prediccion {
                TOPE_PREDICCION_PX as i32 + 8
            } else {
                2
            };
            habia_prediccion = prediccion.is_some();
            // `todo_sucio` no entra aqui: dice que hay que rehacer la ESCENA,
            // y en la capa de tinta no hay mas que el trazo en curso. Lo que
            // manda es la zona que dio el gesto.
            let zona = sucio.map(|(x0, y0, x1, y1)| {
                (
                    x0.floor() as i32 - holgura,
                    y0.floor() as i32 - holgura,
                    x1.ceil() as i32 + holgura,
                    y1.ceil() as i32 + holgura,
                )
            });
            // La union con la del fotograma anterior: si la punta predicha se
            // acorta, lo que se pinto de mas hay que borrarlo, y eso cae
            // fuera de la zona de ahora.
            let zona_pintada = match (zona, zona_tinta_anterior) {
                (Some(a), Some(b)) => {
                    Some((a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
                }
                _ => None,
            };
            let hecho = pintar_tinta_viva(
                &motor,
                &superficie,
                &escena,
                &efectiva,
                &gesto,
                &mut cache,
                &imagenes,
                zona_pintada,
                prediccion,
                ancho_px,
                alto_px,
            );
            ultimo_commit_tinta = std::time::Instant::now();
            zona_tinta_anterior = zona;
            if hecho {
                let puntos_trazo = if medidor.activo() {
                    gesto
                        .elemento_en_curso()
                        .and_then(|(id, _)| escena.buscar(id))
                        .map_or(0, puntos_de_trazo)
                } else {
                    0
                };
                pintado_medido = Some(crate::medir_fotogramas::Pintado {
                    pintar: t_pintar.elapsed(),
                    // No hay present: el fotograma lo publica el `Commit` de
                    // composicion, que ya va dentro de `pintar_tinta`.
                    presentar: std::time::Duration::ZERO,
                    clase: crate::medir_fotogramas::Clase::Tinta,
                    visibles: 1,
                    puntos_trazo,
                    teseladas: 0,
                });
                hay_que_pintar = false;
                sucio = None;
            }
        }
        // **Soltar el lapiz: solo el trazo nuevo** (ver `hornear_trazo`). Con
        // la misma senal de fotograma que la escena, porque tambien presenta.
        if hay_que_pintar
            && fotograma_listo
            && !tinta_viva
            && let Some(id) = trazo_por_hornear.take()
        {
            let t_pintar = std::time::Instant::now();
            let teseladas_antes = cache_tinta.realizadas();
            // Lo que se pinta ENCIMA de los elementos (el marco, la pista del
            // iman, los puntos de una flecha...) y el panel: si algo de eso
            // tiene que cambiar, hace falta la escena entera. Tras soltar un
            // trazo a mano no deberia haber nada, pero no se da por supuesto.
            let panel = crate::panel_dibujo::panel_para(
                &gesto,
                &escena,
                modo_ventana::area_del_panel(area_ui, caja.marco, escala_por_cien),
                escala_por_cien,
            );
            let apto = camara_pintada == efectiva
                && !superficie.esta_estirada()
                && !interfaz_sucia
                && en_capa.is_none()
                && panel == ultimo_panel
                && nada_encima_de_la_escena(&gesto, &escena, efectiva.zoom);
            let horneado = if apto {
                hornear_trazo(
                    &mut motor,
                    &superficie,
                    &escena,
                    &efectiva,
                    id,
                    &mut cache,
                    &mut cache_tinta,
                    &imagenes,
                    ancho_px,
                    alto_px,
                    trasero_distinto,
                    con_marcas.then_some((&marcador, escala_por_cien)),
                )
            } else {
                None
            };
            match horneado {
                Some(h) => {
                    // El trazo ya esta en la escena: fuera de su capa.
                    superficie.apagar_tinta(&motor);
                    pintado_medido = Some(crate::medir_fotogramas::Pintado {
                        pintar: t_pintar.elapsed().saturating_sub(h.presentar),
                        presentar: h.presentar,
                        clase: crate::medir_fotogramas::Clase::Horneado,
                        visibles: 1,
                        puntos_trazo: escena.buscar(id).map_or(0, puntos_de_trazo),
                        teseladas: (cache_tinta.realizadas() - teseladas_antes) as u32,
                    });
                    hay_que_pintar = false;
                    habia_prediccion = false;
                    sucio = None;
                    fotograma_listo = superficie.senal_fotograma().is_none();
                    // D148: el mapa que ahora queda atras no tiene el trazo.
                    // Para el camino de siempre es lo que cambio en este
                    // fotograma, y para el proximo horneado, lo unico que
                    // hay que copiarle del de delante.
                    zona_anterior = Some(h.ventana);
                    trasero_distinto = Some(h.superficie);
                }
                None => {
                    // No se pudo (o no se debia): la escena entera, y la
                    // capa se vacia cuando la presente.
                    vaciar_capa_tras_pintar = true;
                    todo_sucio = true;
                    contenido_sucio = true;
                }
            }
        }
        // **Empezar o acabar de arrastrar: solo el trozo de lo elegido** (ver
        // `repintar_zona`). Mismas condiciones que el horneado del trazo: la
        // superficie tiene que ser de esta camara y la interfaz no puede
        // haber cambiado, porque esto no la pinta.
        if hay_que_pintar
            && fotograma_listo
            && !tinta_viva
            && let Some(z) = zona_por_rehacer.take()
        {
            let t_pintar = std::time::Instant::now();
            let teseladas_antes = cache_tinta.realizadas();
            let panel = crate::panel_dibujo::panel_para(
                &gesto,
                &escena,
                modo_ventana::area_del_panel(area_ui, caja.marco, escala_por_cien),
                escala_por_cien,
            );
            let apto = camara_pintada == efectiva
                && !superficie.esta_estirada()
                && !interfaz_sucia
                && panel == ultimo_panel
                && nada_mas_que_el_marco(&gesto, &escena, efectiva.zoom);
            let hecho = if apto {
                rejilla.sincronizar(&escena);
                // Lo mismo que `pintar` antes de abrir el fotograma: crear
                // bitmaps con el `BeginDraw` abierto no es lo que espera
                // Direct2D, y lo que no este subido no saldria en el trozo.
                if let Some(f) = fondo.as_mut() {
                    f.asegurar(&motor);
                }
                imagenes.asegurar(&motor);
                repintar_zona(
                    &mut motor,
                    &superficie,
                    &escena,
                    &efectiva,
                    &gesto,
                    &rejilla,
                    &mut cache,
                    &mut cache_tinta,
                    &imagenes,
                    fondo.as_ref(),
                    (con_marcas && presentacion.is_none()).then_some(&marcador),
                    escala_por_cien,
                    ancho_px,
                    alto_px,
                    z,
                    matches!(en_capa, Some(EnCapa::Arrastre { .. })),
                    trasero_distinto,
                )
            } else {
                None
            };
            match hecho {
                Some(h) => {
                    // Al acabar, la escena ya pinta lo que llevaba la capa.
                    if vaciar_capa_tras_pintar && en_capa.is_none() {
                        superficie.apagar_tinta(&motor);
                        vaciar_capa_tras_pintar = false;
                    }
                    pintado_medido = Some(crate::medir_fotogramas::Pintado {
                        pintar: t_pintar.elapsed().saturating_sub(h.presentar),
                        presentar: h.presentar,
                        clase: crate::medir_fotogramas::Clase::Zona,
                        visibles: h.visibles,
                        puntos_trazo: 0,
                        teseladas: (cache_tinta.realizadas() - teseladas_antes) as u32,
                    });
                    hay_que_pintar = false;
                    sucio = None;
                    fotograma_listo = superficie.senal_fotograma().is_none();
                    // D148, como en el horneado.
                    zona_anterior = Some(h.ventana);
                    trasero_distinto = Some(h.superficie);
                }
                None => {
                    todo_sucio = true;
                    contenido_sucio = true;
                }
            }
        }
        if hay_que_pintar && fotograma_listo && !tinta_viva {
            // D129: `t_pintar` empieza AQUI, antes de `rejilla.sincronizar`,
            // no despues: sincronizar la rejilla es coste de este fotograma
            // (recorre la escena, O(su tamano)) y si quedara fuera de toda
            // fase, los numeros de D129 estarian mintiendo justo en lo que
            // se quiere diagnosticar.
            let t_pintar = std::time::Instant::now();
            rejilla.sincronizar(&escena);
            // La decision de si la capa congelada vale para ESTE fotograma
            // (`capa_vale`, dentro de `pintar`) es la que manda sobre si la
            // zona parcial es segura: `capa.lista()` solo dice que hay una
            // capa horneada, no que `volcar` la vaya a usar ahora mismo.
            // La punta predicha: horizonte medido (~30 ms de la lectura a la
            // pantalla en el equipo del usuario) y tope de 48 px de pantalla.
            let prediccion =
                prediccion_de(&gesto, &mut mano.predictor, tinta_clasica, efectiva.zoom);
            // Lo que se presenta crece por el tope: la punta nueva y la del
            // fotograma anterior caen, como mucho, a esa distancia.
            let holgura = if prediccion.is_some() || habia_prediccion {
                TOPE_PREDICCION_PX as i32 + 8
            } else {
                2
            };
            // El panel cambia con la seleccion y la herramienta: si no es el
            // que se pinto la ultima vez, se presenta entero.
            let panel = crate::panel_dibujo::panel_para(
                &gesto,
                &escena,
                modo_ventana::area_del_panel(area_ui, caja.marco, escala_por_cien),
                escala_por_cien,
            );
            if panel != ultimo_panel {
                // Sin `contenido_sucio`: aqui ya se esta pintando, y el
                // propio pintado lo pone a falso. Lo que hace falta es que
                // el fotograma salga entero, y eso lo dice `todo_sucio`
                // (que ademas es lo que rehace la capa de la interfaz).
                todo_sucio = true;
            }
            let zona = if todo_sucio {
                None
            } else {
                sucio.map(|(x0, y0, x1, y1)| {
                    (
                        x0.floor() as i32 - holgura,
                        y0.floor() as i32 - holgura,
                        x1.ceil() as i32 + holgura,
                        y1.ceil() as i32 + holgura,
                    )
                })
            };
            // D148: al mapa que se pinta ahora le toca su turno cada DOS
            // fotogramas (la cadena tiene dos), asi que hay que rehacer
            // tambien lo que cambio en el anterior. Sin esta union quedaria
            // pegada la punta del trazo de hace dos fotogramas.
            let zona_pintada = match (zona, zona_anterior) {
                (Some(a), Some(b)) => {
                    Some((a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
                }
                _ => None,
            };
            habia_prediccion = prediccion.is_some();
            let teseladas_antes = cache_tinta.realizadas();
            // El fotograma de la lupa, antes: dentro de `pintar` el motor
            // esta prestado.
            let cristal = lupa.as_ref().and_then(|l| l.preparar(&motor));
            // El boton del marco del riel, pulsado con la herramienta Marco.
            marcador.enmarcando = gesto.herramienta == Herramienta::Marco;
            marcador.editando = !solo_mirar;
            let hojas = pixpin_motor2d::marco::hojas_en_orden(&escena.elementos);
            marcador.marcos = hojas.iter().map(|h| [h.x, h.y, h.ancho, h.alto]).collect();
            marcador.nombres = hojas
                .iter()
                .map(|h| match &h.figura {
                    pixpin_motor2d::Figura::Marco { nombre } => nombre.clone(),
                    _ => String::new(),
                })
                .collect();
            let pintado = pintar(
                &mut motor,
                &superficie,
                &escena,
                &efectiva,
                &gesto,
                &mut cache,
                &mut cache_tinta,
                &rejilla,
                &capa,
                &mut fondo,
                &mut imagenes,
                &caja,
                raton_barra,
                corrimiento_ui,
                // G5: presentando o solo mirando, sin barra ni panel.
                !pasante_fijo && presentacion.is_none() && !solo_mirar,
                escala_por_cien,
                ancho_px,
                alto_px,
                zona_pintada,
                prediccion,
                panel.as_ref(),
                // La capa de interfaz se repinta cuando cambia algo suyo o
                // cuando se rehace el fotograma entero: mientras se traza
                // (zona parcial) no se toca, que es donde cada `Commit` de
                // mas costaria latencia.
                interfaz_sucia || todo_sucio,
                FueraDeLaEscena {
                    seleccion: matches!(en_capa, Some(EnCapa::Arrastre { .. })),
                    marquesina: matches!(en_capa, Some(EnCapa::Marquesina { .. })),
                },
                (con_marcas && presentacion.is_none()).then_some(&marcador),
                |p, base| {
                    if let Some(pr) = presentacion.as_ref() {
                        presentar::pintar(
                            p,
                            base,
                            pr,
                            ancho_px,
                            alto_px,
                            escala_por_cien,
                            &exportar::textos().t("presentar-ayuda"),
                        );
                    } else if solo_mirar {
                        presentar::pintar_solo_mirar(
                            p,
                            base,
                            ancho_px,
                            escala_por_cien,
                            &exportar::textos().t("vista-mirando"),
                        );
                        // Y el riel, con el lapiz para editar (24-oct).
                        if con_marcas {
                            marcador.pintar_interfaz(
                                p,
                                area.ancho,
                                area.alto,
                                escala_por_cien,
                                bajo_la_barra(&caja, area),
                            );
                        }
                    } else if con_marcas {
                        marcador.pintar_interfaz(
                            p,
                            area.ancho,
                            area.alto,
                            escala_por_cien,
                            bajo_la_barra(&caja, area),
                        );
                    }
                    // El numero de cada marco va con la escena (22-oct,
                    // `marcas::pintar_marcos`): aqui, en la interfaz, se
                    // quedaba atras mientras se movia el lienzo.
                    // F8: con la Zona en la mano, su pastilla bajo la barra.
                    if gesto.herramienta == Herramienta::Zona
                        && presentacion.is_none()
                        && !solo_mirar
                    {
                        pastilla_zona.pintar(
                            p,
                            base,
                            area.ancho as f32,
                            bajo_la_barra(&caja, area) as f32,
                            escala_por_cien,
                            hoja_del_chat.is_some(),
                            marcas::textos(),
                        );
                    } else {
                        pastilla_zona.esconder();
                    }
                    // El cristal de la lupa viva, encima de la barra.
                    if let (Some(l), Some(b)) = (lupa.as_ref(), cristal.as_ref()) {
                        l.pintar(p, b);
                    }
                    // La zona vinculada que no lleva a nada, dicho arriba
                    // tres segundos (el `Toast` del movil).
                    if let Some((texto, desde)) = aviso_enlace.as_ref()
                        && desde.elapsed() < AVISO_ENLACE_DURA
                    {
                        presentar::pintar_solo_mirar(p, base, ancho_px, escala_por_cien, texto);
                    }
                },
            );
            match pintado {
                Some(hecho) => {
                    pintado_medido = Some(crate::medir_fotogramas::Pintado {
                        pintar: t_pintar.elapsed().saturating_sub(hecho.presentar),
                        presentar: hecho.presentar,
                        clase: if hecho.parcial {
                            crate::medir_fotogramas::Clase::Zona
                        } else {
                            crate::medir_fotogramas::Clase::Escena
                        },
                        visibles: hecho.visibles,
                        puntos_trazo: 0,
                        teseladas: (cache_tinta.realizadas() - teseladas_antes) as u32,
                    });
                    // Lo que el mapa de atras tiene distinto del de delante
                    // ya no se sabe: el proximo horneado lo iguala entero.
                    trasero_distinto = None;
                    // `olvidar` no lo llamaba nadie: la geometria de cada
                    // trazo borrado, deshecho o convertido se quedaba en la
                    // cache toda la sesion. Solo de vez en cuando (recorre
                    // la escena), tras un fotograma entero.
                    if !hecho.parcial
                        && cache.sobran(hecho.visibles as usize)
                        && cache.sobran(escena.cuantos_visibles())
                    {
                        cache.podar(&escena);
                    }
                    hay_que_pintar = false;
                    // La escena ya pinta lo que estaba en la capa: ahora si
                    // se puede vaciar sin que falte un fotograma.
                    if vaciar_capa_tras_pintar && en_capa.is_none() {
                        superficie.apagar_tinta(&motor);
                        vaciar_capa_tras_pintar = false;
                    }
                    ultimo_panel = panel;
                    zona_anterior = zona;
                    fotograma_listo = superficie.senal_fotograma().is_none();
                    sucio = None;
                    todo_sucio = false;
                    contenido_sucio = false;
                    interfaz_sucia = false;
                    // A3: lo que hay en la superficie es esta camara, y ya
                    // no hay nada que estirar. `dejar_de_estirar` no hace
                    // nada si no habia transformada puesta.
                    camara_pintada = efectiva;
                    camara_movida = None;
                    superficie.dejar_de_estirar();
                    // B1: lo que costo este fotograma alimenta el p99, y si
                    // se paso del plazo que se habia previsto, el margen
                    // sube. Sin `[rendimiento] ritmo` no hay temporizador y
                    // no se anota nada.
                    if temporizador.is_some() {
                        planificador.anotar(t_pintar.elapsed().as_secs_f32() * 1000.0);
                        match plazo_previsto {
                            Some(p) if std::time::Instant::now() > p => planificador.perdido(),
                            Some(_) => planificador.a_tiempo(),
                            None => {}
                        }
                    }
                }
                None => {
                    // `superficie.empezar` fallo: el fotograma se salto. La
                    // zona sucia acumulada NO se puede dar por pintada -si se
                    // limpiara aqui, el proximo present parcial se quedaria sin
                    // el trozo que este fotograma no llego a cubrir-, asi que
                    // se fuerza el fotograma que si llegue a ser completo.
                    todo_sucio = true;
                    contenido_sucio = true;
                }
            }
        }
        // Zoom quieto durante `retardo`: se rehace la tinta nitida a esa
        // escala y se suelta la capa (sus copias venian de la escala vieja).
        // La decision (y el tope de espera de mas abajo) sale de
        // `decidir_zoom`, pura: aqui solo se aplican sus efectos. Ctrl+rueda
        // (D136) es lo que hace que `efectiva.zoom` cambie de una vuelta a
        // la siguiente.
        let decision = decidir_zoom(
            efectiva.zoom,
            zoom_visto,
            cache_tinta.escala(),
            zoom_desde,
            std::time::Instant::now(),
            retardo,
        );
        zoom_visto = decision.visto;
        zoom_desde = decision.desde;
        if decision.rehacer {
            cache_tinta.fijar_escala(efectiva.zoom);
            capa.soltar();
            ventana.invalidar();
        }
        // Dormir hasta que llegue algo, pero no mas de lo que falta para que
        // el zoom cumpla el retardo: si no, `esperar_eventos(None)` dormiria
        // hasta el siguiente evento y la tinta nitida nunca llegaria a
        // rehacerse en reposo. Sin `sleep` fijo aqui tampoco: con el `sleep`
        // de 5 ms (15,6 ms reales sin `timeBeginPeriod`) el bucle perdia la
        // mitad de los puntos de un trazo rapido, y en reposo no gana nada.
        // `decision.tope_ms` es `None` en cuanto no hay nada pendiente -y
        // solo entonces-, asi que nunca gira en vacio con un tope de 0 ms.
        let t_esperar = std::time::Instant::now();
        let senal = if hay_que_pintar && !fotograma_listo {
            superficie.senal_fotograma()
        } else {
            None
        };
        // Con una pausa de forma rapida en marcha no llegan eventos (el raton
        // esta quieto): el bucle tiene que despertarse cuando se cumpla.
        let tope_forma = mano.tope_forma_ms();
        // Y el laser, que se apaga con el reloj: despertar cada fotograma.
        let tope_forma = match (tope_forma, laser_vivo.then_some(16u32)) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        // A3: con la escena compuesta y la mano quieta no llega ningun
        // evento; sin este tope, el fotograma nitido no llegaria nunca y el
        // lienzo se quedaria estirado hasta el siguiente movimiento.
        let tope_reposo = camara_movida
            .map(|desde| REPOSO_CAMARA.saturating_sub(desde.elapsed()).as_millis() as u32 + 1);
        let minimo = |a: Option<u32>, b: Option<u32>| match (a, b) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        let decision = DecisionZoom {
            tope_ms: minimo(
                minimo(minimo(decision.tope_ms, tope_forma), tope_reposo),
                minimo(
                    // B2: la capa de tinta tiene algo que pintar pero todavia
                    // no toca; sin este tope, con la mano quieta no llegaria
                    // ningun evento y el ultimo tramo del trazo se quedaria
                    // sin salir.
                    espera_tinta,
                    minimo(
                        // Volando hacia una marca (F5): despertar cada fotograma.
                        marcador.tope_ms(),
                        // La barra del pin se va sola un poco despues de
                        // salir el raton: hay que despertar para verlo.
                        pin.as_ref().filter(|p| p.quiere_reloj()).map(|_| 50),
                    ),
                ),
            ),
            ..decision
        };
        let tope = if senal.is_some() {
            Some(
                decision
                    .tope_ms
                    .map_or(ESPERA_MAXIMA_SENAL_MS, |t| t.min(ESPERA_MAXIMA_SENAL_MS)),
            )
        } else {
            decision.tope_ms
        };
        if pixpin_shell::overlay::esperar_eventos_o_senal(senal, tope) {
            fotograma_listo = true;
        }
        // **B1: empezar el fotograma tarde.** Con `[rendimiento] ritmo`, en
        // cuanto se puede pintar NO se pinta: se duerme hasta
        // `plazo de DWM − p99(pintar) − margen` y se vuelve arriba, donde la
        // cola se vacia con los puntos que han llegado mientras tanto. Lo
        // que se gana no es pintar antes, es que lo pintado sea mas nuevo:
        // hoy el fotograma se queda esperando quieto los 10-13 ms que faltan
        // para la composicion, con los puntos envejeciendo dentro (Raph
        // Levien, «Swapchains and frame pacing»).
        //
        // El planificador decide, y decide 0 en cuanto pintar ya no cabe en
        // lo que queda: en un equipo lento esto no cambia nada.
        plazo_previsto = None;
        if let Some(t) = temporizador.as_ref()
            && hay_que_pintar
            && fotograma_listo
            && let Some(r) = superficie.ritmo()
        {
            plazo_previsto = Some(
                std::time::Instant::now()
                    + std::time::Duration::from_secs_f32(r.hasta_el_plazo_ms / 1000.0),
            );
            let espera = planificador.esperar_ms(r.hasta_el_plazo_ms);
            // Por debajo de un cuarto de milisegundo, armar el temporizador
            // cuesta mas que lo que se duerme.
            if espera > 0.25 {
                t.dormir(espera);
            }
        }
        if let Some(linea) = medidor.anotar(crate::medir_fotogramas::Vuelta {
            puntos,
            vaciar,
            pintado: pintado_medido,
            esperar: t_esperar.elapsed(),
        }) {
            linea.registrar();
        }
    }

    // La lupa no sale en la foto ni deja su captura viva detras.
    if let Some(l) = lupa.as_mut() {
        l.cerrar();
    }
    // Ni la pastilla ni el gesto sobreviven al anotador.
    drop(pastilla.take());
    drop(_escucha);
    // Un texto a medias es tinta: entra en lo que se guarda.
    if pantalla.is_some() && gesto.esta_escribiendo() {
        gesto.cerrar_texto(&mut escena);
    }
    // **El anotador de pantalla guarda solo al salir** (2026-09-29: «tiene
    // que guardarse automaticamente en el canvas dentro de Mensajes
    // guardados»): Escape, Salir o cerrar, con algo nuevo dibujado, lleva la
    // captura de debajo y la tinta editable al chat, sin preguntar ni crear
    // un pin; si ya se guardo en esta sesion, pone al dia ese lienzo. Asi
    // Escape nunca tira lo dibujado, que era lo que protegia D54.
    let con_almacen = pantalla.as_deref().is_some_and(|pa| pa.raiz.is_some());
    if let Some(pa) = pantalla.as_deref_mut()
        && con_almacen
    {
        let tinta = crate::anotador_al_chat::tinta_de(&escena);
        let fondo_ref = fondo.as_ref();
        pantalla::guardar_en_el_chat(
            &mut chat,
            pa,
            tinta,
            crate::anotador_al_chat::pegadas_de(&escena, |id| imagenes.rgba(id).cloned()),
            pantalla::globo(exportar::textos()),
            |pa| pantalla::foto_al_salir(pa, fondo_ref, &ventana),
        );
    }
    // **Sin almacen** (no pasa desde `main`, que siempre lo da) queda la
    // salida de antes: una foto de la pantalla con lo anotado, SIN barra ni
    // panel ni marco de lo elegido (D59), que `main` ofrece pinear (D54).
    if let Some(pa) = pantalla
        && !con_almacen
        && escena.cuantos_visibles() > 0
    {
        gesto.seleccion.limpiar();
        gesto.lazo = None;
        ventana.poner_pasante(false);
        // Lo que estuviera en la capa de la tinta ya esta en la escena.
        superficie.apagar_tinta(&motor);
        rejilla.sincronizar(&escena);
        capa.soltar();
        let _ = pintar(
            &mut motor,
            &superficie,
            &escena,
            &efectiva,
            &gesto,
            &mut cache,
            &mut cache_tinta,
            &rejilla,
            &capa,
            &mut fondo,
            &mut imagenes,
            &caja,
            None,
            corrimiento_ui,
            false,
            escala_por_cien,
            ancho_px,
            alto_px,
            None,
            None,
            None,
            true,
            FueraDeLaEscena::default(),
            None,
            |_, _| {},
        );
        superficie.dejar_de_estirar();
        pixpin_shell::esperar_composicion();
        pa.resultado = (pa.capturar)();
    }
    // D143: la copia en GPU (y la de CPU) se suelta al cerrar, no al volver
    // al gestor de pines.
    drop(fondo);
    // Lo de dentro de las lupas son mapas de este motor: fuera con el.
    lupas::olvidar();
    // Como queda (modo y sitio), para el siguiente lienzo; la barra se va
    // con el pin.
    if let Some(pn) = pin.take() {
        modo_ventana::recordar(modo_ventana::Recordado {
            pantalla_completa: pn.pantalla_completa,
            ventana: Some(pn.area_pin()),
        });
    }
    ventana.ocultar();
    crate::dibujo::tema::fijar_papel(None);
    escena.compactar();
    // Lo pegado sale con la escena: quien la guarda escribe tambien sus
    // pixeles (`imagenes_lienzo`), o al reabrir la imagen no estaria.
    pegadas.extend(imagenes.tomar_nuevas());
    // Y a que hoja queria ir, si pulso un recuadro con enlace: quien llama
    // es el unico que sabe donde estan las hojas.
    Ok((escena, enlace_pedido))
}

/// Pinta un fotograma entero: lo que hay en pantalla, y encima el marco de
/// la seleccion, sus tiradores y la marquesina si la hay.
///
/// `encima` se llama al final, con la transformacion todavia puesta a la
/// del mundo: quien la pasa es quien decide si dibuja en coordenadas del
/// mundo o si la deshace con `Pintor::desplazar(0.0, 0.0)` primero (el
/// cajetin de calibrar hace esto ultimo: es un dialogo en pantalla, no algo
/// del lienzo).
///
/// La caja de herramientas es del mismo tipo de dialogo, y se pinta aqui
/// mismo -no via `encima`- para que salga en los dos caminos que llaman a
/// `pintar` (el bucle principal y `pedir_medida`) sin que ninguno tenga que
/// acordarse de repetirla.
///
/// Devuelve `None` si `superficie.empezar` fallo (el backbuffer no estaba
/// listo): quien llama no puede dar la zona sucia por cubierta ni la
/// pantalla por al dia en ese caso, o el proximo present parcial se dejaria
/// un trozo sin pintar creyendo que ya se habia pintado en este fotograma
/// que se saltó. Si pinto, dice cuanto tardo presentar (D129: con vsync,
/// ahi se nota la espera al refresco), cuantos elementos recorrio y si fue
/// solo una zona.
#[allow(clippy::too_many_arguments)]
fn pintar(
    motor: &mut MotorRender,
    superficie: &Superficie,
    escena: &Escena,
    camara: &Camara,
    gesto: &Gesto,
    cache: &mut Cache,
    cache_tinta: &mut pixpin_render::CacheTinta,
    rejilla: &Rejilla,
    capa: &CapaEstatica,
    fondo: &mut Option<FondoLienzo>,
    imagenes: &mut ImagenesLienzo,
    caja_herramientas: &CajaHerramientas,
    raton_barra: Option<Punto>,
    // Cuanto correr la barra y el panel del escritorio a la ventana, y si
    // se ensenan (el anotador pasante los esconde).
    corrimiento_ui: (f32, f32),
    con_interfaz: bool,
    escala_por_cien: u32,
    ancho_px: f32,
    alto_px: f32,
    zona: Option<(i32, i32, i32, i32)>,
    prediccion: Option<Punto2>,
    panel: Option<&pixpin_ui::panel_lateral::PanelLateral>,
    interfaz_sucia: bool,
    fuera: FueraDeLaEscena,
    // F5: las marcas van dentro de la escena (ver `marcas.rs`, «por que la
    // marca se pinta con la escena»).
    marcas: Option<&marcas::Marcador>,
    encima: impl FnOnce(&pixpin_render::Pintor<'_>, (f32, f32)),
) -> Option<Pintada> {
    if let Some(f) = fondo.as_mut() {
        f.asegurar(motor);
    }
    // Antes de `dibujar`: crear un bitmap con el `BeginDraw` abierto no es
    // lo que espera Direct2D.
    imagenes.asegurar(motor);
    let Ok(destino) = superficie.empezar(motor) else {
        return None;
    };
    // A3: la superficie de la escena es `margen` pixeles mas grande por
    // lado. Todo lo del MUNDO se pinta corrido ese margen; lo de la
    // interfaz, que va en su propia capa, no.
    let margen = superficie.margen();
    let con_capas = superficie.tiene_capas();
    // La rejilla dice que PUEDE verse; la camara filtra lo que de verdad se
    // ve. Sin la rejilla, esto recorreria los ocho mil elementos.
    //
    // Con colchon se ve mas mundo del que cabe por la ventana: si se
    // recortara a la ventana, el colchon se pintaria vacio y al desplazar
    // asomaria en blanco, que es justo lo que viene a evitar.
    let holgura_mundo = margen / camara.zoom;
    let v = camara.ventana(ancho_px, alto_px);
    let vista = (
        v.0 - holgura_mundo,
        v.1 - holgura_mundo,
        v.2 + holgura_mundo,
        v.3 + holgura_mundo,
    );
    let candidatos = rejilla.candidatos(vista);
    let visibles = candidatos.len() as u32;

    // Si hay una capa estatica valida para este fotograma, `volcar` ya
    // copio en `destino` todo lo que no se mueve: aqui solo hace falta
    // pintar encima lo excluido (lo seleccionado, que es lo que se arrastra)
    // y el marco. Si no vale, se pinta todo como siempre, y `volcar` no
    // habra tocado `destino`.
    let ahora_excluidos = excluidos_de(gesto);
    let ahora = Estampa {
        camara: (camara.x, camara.y, camara.zoom),
        tamano: tamano_escena(ancho_px, alto_px, margen),
        excluidos: ahora_excluidos.clone(),
    };
    // La zona sucia llega en pixeles de VENTANA y la superficie de escena
    // esta corrida el colchon: sin esto, el recorte, la copia de la capa y
    // el rectangulo del present apuntarian `margen` pixeles mas arriba y a
    // la izquierda de donde de verdad esta el trazo.
    let m = margen as i32;
    let zona = zona.map(|(a, b, c, d)| (a + m, b + m, c + m, d + m));
    // Solo el trozo que cambia (D148): copiar la pantalla entera cada
    // fotograma costaba 6-8 ms medidos en el equipo del usuario, y eso era
    // la mitad del presupuesto de 60 Hz gastada antes de dibujar la tinta.
    let capa_vale = capa.volcar_zona(motor, &destino, &ahora, zona);
    // Lo que de verdad importa para presentar solo un trozo es si la capa
    // congelada SE USO en este fotograma (`capa_vale`), no si `capa.lista()`
    // dice que hay una capa horneada: si no vale, `destino` se limpia y se
    // pinta entero mas abajo, y presentar con una zona pequena dejaria el
    // resto de la pantalla con lo que hubiera antes.
    let zona = if capa_vale { zona } else { None };

    let fondo_ref = fondo.as_ref();
    // Todo lo que NO es lienzo: la barra, el panel y
    // lo que pinte el llamante encima (el cajetin de calibrar).
    //
    // `base` es el desplazamiento que hay que sumarle a todo: 0 cuando se
    // pinta en la misma superficie que la escena, y el que devuelva
    // DirectComposition cuando va en su propia capa.
    let pintar_ui = |p: &pixpin_render::Pintor<'_>, base: (f32, f32)| {
        // Pasante (el anotador de pantalla dejando pasar los clics): la
        // barra y el panel no responderian, asi que no se ensenan. Es
        // tambien lo que dice en que estado se esta.
        if !con_interfaz {
            return;
        }
        // La caja y el panel viven en coordenadas del escritorio, que son
        // las de los clics; aqui se pinta en las de la ventana. Con el
        // lienzo a pantalla completa en el monitor principal no cambia nada;
        // en ventana, o con el anotador que cubre todos los monitores, si.
        p.desplazar(corrimiento_ui.0, corrimiento_ui.1);
        crate::caja_dibujo::pintar_barra(
            p,
            caja_herramientas,
            gesto.herramienta,
            escala_por_cien,
            raton_barra,
            |b| match b {
                BotonCaja::Elegir(h) => tecla_de(h),
                _ => None,
            },
        );
        if let Some(panel) = panel {
            crate::panel_dibujo::pintar(p, panel, escala_por_cien);
        }
        // F11: el rotulo de imprimir, encima de todo lo de la interfaz.
        // Y el nombre de cada herramienta, para encontrarla sin saber su icono.
        crate::caja_dibujo::pintar_pista(p, caja_herramientas, escala_por_cien, raton_barra, |b| {
            crate::dibujo::permitidas::rotulo_de_boton(b, exportar::textos())
        });
        p.desplazar(0.0, 0.0);
        let _ = base;
    };
    // `encima` solo se puede llamar una vez y se llama en una de las dos
    // ramas: dentro de la escena (sin capas) o dentro de la interfaz (con
    // capas). El `Option` es lo que le dice eso al compilador.
    let mut encima = Some(encima);
    // El prestamo compartido del almacen dura solo este bloque: al salir,
    // `imagenes` vuelve a ser mutable, que es lo que necesita `soltar` si el
    // dispositivo se ha perdido.
    let error = {
        let imagenes: &ImagenesLienzo = imagenes;
        motor.dibujar(&destino, |p| {
            // El mundo se dibuja en sus propias coordenadas; la matriz activa es
            // lo unico que cambia al encuadrar o acercar (camara.rs lo explica:
            // "la geometria se calcula UNA VEZ en coordenadas del mundo").
            let origen = camara.a_pantalla(Punto2::nuevo(0.0, 0.0));
            let origen = Punto2::nuevo(origen.x + margen, origen.y + margen);
            // D148: con la capa copiada a trozos, fuera de la zona el mapa
            // lleva lo de hace dos fotogramas, que ahi es lo correcto. Pintar
            // encima volveria a mezclar la tinta consigo misma y el trazo se
            // iria oscureciendo por los bordes. El recorte va ANTES de poner
            // la vista: se toma en pixeles de pantalla.
            if let Some((x0, y0, x1, y1)) = zona {
                p.empujar_recorte(RectF {
                    x: x0 as f32,
                    y: y0 as f32,
                    ancho: (x1 - x0).max(0) as f32,
                    alto: (y1 - y0).max(0) as f32,
                });
            }
            if !capa_vale {
                p.limpiar(a_color(escena.fondo));
            }
            p.poner_vista((0.0, 0.0), camara.zoom, (origen.x, origen.y));
            // D140/D142: con la capa valida la imagen ya esta copiada; si no, va
            // la primera, debajo de todo, y solo si se ve.
            if !capa_vale {
                if let Some(f) = fondo_ref {
                    f.pintar(p, vista, camara.zoom);
                }
            }

            for id in candidatos {
                // Con la capa valida, lo que no esta excluido ya esta copiado
                // en `destino`: repintarlo aqui seria pagar dos veces.
                let excluido = es_excluido(gesto, id);
                if capa_vale && !excluido {
                    continue;
                }
                // Lo que se arrastra en la capa de la tinta no va en la
                // escena: se veria dos veces, una quieta y otra moviendose.
                if fuera.seleccion && se_arrastra(gesto, id) {
                    continue;
                }
                // Derecho a su sitio con lo que sabe la rejilla (se acaba de
                // sincronizar): `buscar` recorre la escena, y hacerlo por
                // cada candidato es cuadratico en lo que se ve.
                let Some(e) = elemento_por_id(escena, rejilla, id) else {
                    continue;
                };
                // Las guias escondidas no se pintan (`Escena::a_la_vista`).
                if e.borrado || guia_escondida(escena, e) {
                    continue;
                }
                // El mosaico lo pinta `tapar::pasar`, con los pixeles de
                // debajo, cuando este fotograma ya este cerrado. Ver
                // `tapar.rs`.
                if tapar::es_mosaico(e) {
                    continue;
                }
                // Lo que se esta dibujando (trazo, linea, flecha, rectangulo,
                // elipse) con su punta en el punto predicho. La escena no se
                // toca, asi que al soltar queda lo real.
                let en_curso = gesto.elemento_en_curso().filter(|(id, _)| *id == e.id);
                let mut punta = None;
                if let (Some(q), Some((_, origen))) = (prediccion, en_curso) {
                    match punta_de_tinta(e, q) {
                        // Lapiz y resaltador: la punta se pinta APARTE, encima
                        // del trazo real, a partir de su cola. Ver
                        // `punta_de_tinta`.
                        Some(orden) => punta = Some(orden),
                        // Las demas figuras tienen punta barata (una linea son
                        // dos puntos, una caja cuatro numeros): la copia
                        // entera sigue siendo lo mas simple y no cuesta nada.
                        None => {
                            if let Some(copia) =
                                pixpin_motor2d::tinta::prediccion::con_punta(e, origen, q)
                            {
                                pintar_copia_predicha(
                                    p,
                                    &copia,
                                    vista,
                                    imagenes,
                                    camara.zoom,
                                    escena.escala.as_ref(),
                                );
                                continue;
                            }
                        }
                    }
                }
                // Lo excluido es lo que se arrastra o el trazo en curso: su
                // version sube en CADA fotograma, asi que cachear su tinta
                // seria teselar de nuevo cada vez sin acertar nunca -pagar la
                // realizacion sin cobrar el ahorro-. Se pinta sin cache, como
                // antes de esta tarea.
                let mut indice = 0u32;
                let clave_tinta = !excluido;
                let mut hubo_tinta = false;
                let grano = pixpin_motor2d::pintado::grano_de(e);
                por_cada_orden(cache, e, camara.zoom, escena.escala.as_ref(), |orden| {
                    hubo_tinta |= matches!(orden, Orden::Tinta { .. });
                    let tinta =
                        clave_tinta.then_some((&mut *cache_tinta, (e.id, e.version, indice)));
                    dibujar_orden(p, orden, vista, tinta, imagenes, camara.zoom, grano);
                    indice += 1;
                });
                // La punta va encima, y SOLO si el trazo se esta pintando como
                // tinta: a zoom muy bajo `ordenes_a_distancia` lo degrada a una
                // polilinea fina (`TINTA_MINIMA_PX`), y una mancha de tinta
                // pegada a una raya se veria como un borron. A ese aumento la
                // punta predicha mide menos de un pixel de todos modos.
                if let (Some(orden), true) = (punta, hubo_tinta) {
                    dibujar_orden(p, &orden, vista, None, imagenes, camara.zoom, None);
                }
            }

            // Encima de todo: el marco de la seleccion, sus tiradores, la
            // marquesina, el lazo, el iman, las uniones y los angulos. Es
            // comun a todos los anfitriones (`dibujo::pintar::pintar_encima`):
            // el lector y la pantalla lo pintan con las mismas llamadas.
            crate::dibujo::pintar::pintar_encima(
                p,
                gesto,
                escena,
                vista,
                camara.zoom,
                imagenes,
                fuera.seleccion,
                fuera.marquesina,
            );
            // Las marcas, encima de todo lo dibujado y con la escena: asi se
            // van con el lienzo cuando A3 corre el visual en vez de repintar.
            if let Some(m) = marcas {
                m.pintar_en_la_escena(p, camara, margen, ancho_px, alto_px, escala_por_cien);
            }
            // La caja es un dialogo en pantalla, no algo del lienzo: no se mueve
            // ni se escala con la camara. `desplazar(0.0, 0.0)` deshace la vista
            // del mundo que `poner_vista` dejo puesta arriba, igual que hace
            // `dibujar_cajetin` mas abajo.
            //
            // Con capas (A3) esto NO se pinta aqui: va a la superficie de la
            // interfaz, que no se mueve cuando la escena se desplaza. Es la
            // razon de ser de las dos capas.
            if !con_capas {
                p.desplazar(0.0, 0.0);
                pintar_ui(p, (0.0, 0.0));
                if let Some(e) = encima.take() {
                    e(p, (0.0, 0.0));
                }
            }
            if zona.is_some() {
                p.soltar_recorte();
            }
        })
    };
    // A3: la capa de la interfaz, aparte y solo cuando cambia. Va DESPUES
    // del fotograma de la escena y antes de presentarlo: su `Commit` y el
    // `Present` de la escena caen en el mismo intervalo de composicion, asi
    // que DWM las ensena juntas.
    if con_capas && interfaz_sucia {
        let _ = superficie.pintar_interfaz(motor, |p, d| {
            pintar_ui(p, d);
            if let Some(e) = encima.take() {
                e(p, d);
            }
        });
    }
    // **La pasada de tapado**, con el fotograma de la escena ya cerrado
    // (`motor.dibujar` acaba de hacer su `EndDraw`): leer lo pintado exige
    // que el destino este escrito de verdad. Va antes de presentar, y solo
    // si el fotograma salio bien: sobre un destino que no se llego a pintar
    // no hay nada que tapar.
    //
    // La camara se corre el colchon porque `plan_en_pantalla` da pixeles de
    // VENTANA y esto se pinta en la superficie de la escena, que va `margen`
    // pixeles mas grande por lado. Sin el corrimiento cada mosaico taparia
    // arriba y a la izquierda de donde esta -y dejaria al descubierto justo
    // el dato que venia a tapar-.
    if error.is_ok() {
        let corrida = Camara {
            x: camara.x - margen / camara.zoom,
            y: camara.y - margen / camara.zoom,
            zoom: camara.zoom,
        };
        let (ancho_sup, alto_sup) = tamano_escena(ancho_px, alto_px, margen);
        tapar::pasar(
            motor,
            &destino,
            &escena.elementos,
            &corrida,
            ancho_sup,
            alto_sup,
        );
        // Y dentro de cada lupa, lo que mira, agrandado (`lupas.rs`): despues
        // del tapado, para que lo tapado salga tapado tambien en grande.
        lupas::pasar(
            motor,
            &destino,
            escena,
            &corrida,
            fondo.as_ref(),
            imagenes,
            ancho_sup,
            alto_sup,
            // La lupa que se arrastra en la capa de la tinta lleva lo de
            // dentro con ella (`lupas::pintar_en_capa`): aqui se quedaria
            // detras, donde estaba al cogerla.
            &|id| fuera.seleccion && se_arrastra(gesto, id),
        );
    }
    if error.is_err() {
        // Dispositivo perdido: las realizaciones de tinta son del dispositivo
        // viejo y ya no valen (D2D las rechazaria en el siguiente fotograma).
        cache_tinta.vaciar();
        // Y los pinceles y brillos que guarda el motor, por lo mismo.
        motor.olvidar_recursos_de_dispositivo();
        // Dispositivo perdido: el bitmap del fondo tambien era del viejo.
        if let Some(f) = fondo.as_mut() {
            f.soltar();
        }
        // Y los de las imagenes pegadas, por lo mismo.
        imagenes.soltar();
        // Y lo de dentro de las lupas, pintado en mapas del viejo.
        lupas::olvidar();
    }
    let t_presentar = std::time::Instant::now();
    let _ = superficie.presentar_sincronizado(zona);
    Some(Pintada {
        presentar: t_presentar.elapsed(),
        visibles,
        parcial: zona.is_some(),
    })
}

/// Lo que devuelve `pintar` cuando llega a pintar.
#[derive(Debug, Clone, Copy)]
struct Pintada {
    presentar: std::time::Duration,
    /// Cuantos candidatos de la rejilla se recorrieron.
    visibles: u32,
    /// Si fue solo una zona sobre la capa congelada.
    parcial: bool,
}

/// El elemento `id`, mirando primero donde la rejilla dice que estaba y,
/// si ahi ya no esta (la escena cambio de orden sin sincronizar), por el
/// camino largo. Nunca devuelve otro elemento: se comprueba el id.
fn elemento_por_id<'a>(escena: &'a Escena, rejilla: &Rejilla, id: u64) -> Option<&'a Elemento> {
    rejilla
        .posicion(id)
        .and_then(|i| escena.elementos.get(i))
        .filter(|e| e.id == id)
        .or_else(|| escena.buscar(id))
}

/// Cuantos puntos tiene un trazo a mano (0 si no lo es). Para el medidor.
fn puntos_de_trazo(e: &Elemento) -> u32 {
    match &e.figura {
        Figura::Lapiz { puntos, .. } | Figura::Resaltador { puntos } => puntos.len() as u32,
        _ => 0,
    }
}

/// Si encima de los elementos no hay nada que pintar: ni marco de eleccion,
/// ni marquesina, ni lazo, ni pista del iman, ni puntos de flecha, ni
/// angulos. Es la condicion para hornear un trazo sin la escena entera: todo
/// eso se pinta DESPUES de los elementos, y si hubiera que quitarlo o
/// ponerlo, pintar solo el trazo no lo haria.
fn nada_encima_de_la_escena(gesto: &Gesto, escena: &Escena, zoom: f32) -> bool {
    gesto.seleccion.ids().is_empty() && nada_mas_que_el_marco(gesto, escena, zoom)
}

/// Lo mismo sin mirar la seleccion: lo que `pintar_zona` no pinta. El marco
/// y sus tiradores si los pinta, asi que con ellos basta con esto.
fn nada_mas_que_el_marco(gesto: &Gesto, escena: &Escena, zoom: f32) -> bool {
    let escala = 1.0 / zoom;
    gesto.marquesina().is_none()
        && gesto.lazo.is_none()
        && gesto.bolita.centro.is_none()
        && gesto.anclaje_activo.is_none()
        && gesto.tiradores_de_punta(escena, escala).is_none()
        && !construir::hay_angulos_que_ensenar(gesto)
        && gesto.resaltado_de_union(escena, zoom).is_empty()
        // Las cabezas de los clavos van encima de todo (`nudos`).
        && escena.alfileres.is_empty()
}

/// **Soltar el lapiz sin repintar la escena.**
///
/// Al soltar, la escena de antes esta intacta en la superficie (el trazo se
/// dibujo en su propia capa, B2) y lo unico nuevo es el trazo, que es el
/// ultimo elemento: se pinta ENCIMA de lo que ya hay, en su zona, y se
/// presenta solo esa zona. Antes se pintaba la escena entera -a ~23 µs por
/// elemento visible, de 20 a 70 ms por suelta en la HD 4000 con 150-300
/// trazos a la vista-, y cada trazo nuevo hacia mas caro soltar el
/// siguiente: «al principio va rapido y al cabo de un minuto va lento».
///
/// Pintar el ultimo elemento encima de lo ya pintado da los mismos pixeles
/// que la escena entera: es lo que haria el fotograma completo al llegar a
/// el. Las condiciones que lo garantizan las pone quien llama (la escena al
/// dia al apoyar el lapiz, nada encima, la misma camara), y dos mas se miran
/// aqui porque son del trazo: que no este girado y que no toque un mosaico
/// (la pasada de tapado lee lo pintado debajo, y un trazo nuevo encima
/// cambiaria lo que lee).
///
/// La cadena de intercambio tiene dos mapas (D148): antes de pintar se
/// iguala el de atras con el que se ve (`igualar_trasero`), solo en
/// `trasero_distinto` si se sabe. Devuelve `None` si no se hizo; entonces
/// quien llama pinta la escena entera como siempre.
#[allow(clippy::too_many_arguments)]
fn hornear_trazo(
    motor: &mut MotorRender,
    superficie: &Superficie,
    escena: &Escena,
    camara: &Camara,
    id: u64,
    cache: &mut Cache,
    cache_tinta: &mut pixpin_render::CacheTinta,
    imagenes: &ImagenesLienzo,
    ancho_px: f32,
    alto_px: f32,
    trasero_distinto: Option<(i32, i32, i32, i32)>,
    marcas: Option<(&marcas::Marcador, u32)>,
) -> Option<Horneado> {
    let e = escena
        .elementos
        .last()
        .filter(|e| e.id == id && !e.borrado)?;
    let margen = superficie.margen();
    let (ancho_sup, alto_sup) = tamano_escena(ancho_px, alto_px, margen);
    let zona = zona_del_horneado(e, camara, margen, ancho_sup, alto_sup)?;
    // **Una marca (F5) debajo**: las marcas van encima de todo lo dibujado,
    // y el trazo pintado encima de lo ya pintado taparia su redondel hasta
    // el siguiente fotograma entero. Repintarla encima tampoco vale: va al
    // 82 % y saldria mas opaca. Lo exacto es el fotograma entero, que la
    // pinta en su sitio; solo cuesta eso cuando el trazo roza una marca.
    if let Some((m, escala_por_cien)) = marcas {
        let ventana = (
            (zona.0 as f32) - margen,
            (zona.1 as f32) - margen,
            (zona.2 as f32) - margen,
            (zona.3 as f32) - margen,
        );
        if m.alguna_toca(camara, ventana, escala_por_cien) {
            return None;
        }
    }
    // Un mosaico debajo: su pasada leeria el trazo. Se mira en el mundo.
    let caja = caja_de_tinta(e);
    let pisa_mosaico = escena.elementos.iter().any(|m| {
        !m.borrado && tapar::es_mosaico(m) && {
            let c = pixpin_motor2d::mosaico::caja_girada(m);
            c.0 <= caja.2 && c.2 >= caja.0 && c.1 <= caja.3 && c.3 >= caja.1
        }
    });
    if pisa_mosaico || lupas::pisa_una_lupa(&escena.elementos, caja) {
        return None;
    }
    superficie.igualar_trasero(trasero_distinto).ok()?;
    let destino = superficie.empezar(motor).ok()?;
    let error = motor.dibujar(&destino, |p| {
        pintar_horneado(
            p,
            e,
            camara,
            margen,
            zona,
            escena,
            cache,
            cache_tinta,
            imagenes,
            ancho_px,
            alto_px,
        );
    });
    if error.is_err() {
        // Dispositivo perdido: lo arregla el fotograma entero, que ya sabe
        // soltar todo lo del dispositivo viejo.
        return None;
    }
    let t_presentar = std::time::Instant::now();
    superficie.presentar_sincronizado(Some(zona)).ok()?;
    let m = margen as i32;
    Some(Horneado {
        ventana: (zona.0 - m, zona.1 - m, zona.2 - m, zona.3 - m),
        superficie: zona,
        presentar: t_presentar.elapsed(),
        visibles: 1,
    })
}

/// Lo que devuelve `hornear_trazo` (y `repintar_zona`).
#[derive(Debug, Clone, Copy)]
struct Horneado {
    /// La zona pintada en pixeles de VENTANA (la de D148).
    ventana: (i32, i32, i32, i32),
    /// Y en pixeles de la superficie de escena (con el colchon).
    superficie: (i32, i32, i32, i32),
    presentar: std::time::Duration,
    /// Cuantos elementos se recorrieron: 1 al hornear un trazo, los que
    /// caen en la zona al rehacer un trozo.
    visibles: u32,
}

/// La caja del MUNDO que puede manchar un trazo a mano. `Elemento::caja`
/// suma medio `size` de perfect-freehand, pero el radio llega casi al
/// `size` entero con presion (`radio` de `freehand.rs`: `size·sen(0,4π)`),
/// y el grafito se cuece con su propio mapa: se suma un `size` mas y se une
/// la caja del mapa. Quedarse corto dejaria el trazo cortado en seco en el
/// borde de la zona; pasarse solo recorta un poco mas de lo necesario.
fn caja_de_tinta(e: &Elemento) -> (f32, f32, f32, f32) {
    let (x0, y0, x1, y1) = e.caja();
    let r = e.grosor * pixpin_motor2d::tinta::FACTOR_VARIABLE + 2.0;
    let mut c = (x0 - r, y0 - r, x1 + r, y1 + r);
    if let Some(g) = pixpin_motor2d::tinta::grafito::cocer(e) {
        let (gx, gy, gw, gh) = g.caja();
        c = (c.0.min(gx), c.1.min(gy), c.2.max(gx + gw), c.3.max(gy + gh));
    }
    c
}

/// La zona del horneado en pixeles de la SUPERFICIE de escena (con el
/// colchon), recortada a ella. `None` si el trazo esta girado (su caja no
/// es la del mundo) o si no cae dentro de la superficie.
fn zona_del_horneado(
    e: &Elemento,
    camara: &Camara,
    margen: f32,
    ancho_sup: u32,
    alto_sup: u32,
) -> Option<(i32, i32, i32, i32)> {
    if e.angulo != 0.0 {
        return None;
    }
    let (x0, y0, x1, y1) = caja_de_tinta(e);
    let a = camara.a_pantalla(Punto2::nuevo(x0, y0));
    let b = camara.a_pantalla(Punto2::nuevo(x1, y1));
    // Dos pixeles de mas por el suavizado de bordes.
    let zona = (
        ((a.x.min(b.x) + margen).floor() as i32 - 2).max(0),
        ((a.y.min(b.y) + margen).floor() as i32 - 2).max(0),
        ((a.x.max(b.x) + margen).ceil() as i32 + 2).min(ancho_sup as i32),
        ((a.y.max(b.y) + margen).ceil() as i32 + 2).min(alto_sup as i32),
    );
    (zona.2 > zona.0 && zona.3 > zona.1).then_some(zona)
}

/// El dibujo del horneado: el trazo, recortado a `zona` (pixeles de la
/// superficie), con la misma vista y las mismas ordenes que le daria el
/// fotograma entero (`pintar`), cache de realizaciones incluida: el
/// siguiente fotograma entero ya lo encuentra teselado.
///
/// Aparte de `hornear_trazo` para que el banco de `medir.rs` mida EXACTAMENTE
/// esto sin ventana.
#[allow(clippy::too_many_arguments)]
fn pintar_horneado(
    p: &pixpin_render::Pintor<'_>,
    e: &Elemento,
    camara: &Camara,
    margen: f32,
    zona: (i32, i32, i32, i32),
    escena: &Escena,
    cache: &mut Cache,
    cache_tinta: &mut pixpin_render::CacheTinta,
    imagenes: &ImagenesLienzo,
    ancho_px: f32,
    alto_px: f32,
) {
    p.empujar_recorte(RectF {
        x: zona.0 as f32,
        y: zona.1 as f32,
        ancho: (zona.2 - zona.0) as f32,
        alto: (zona.3 - zona.1) as f32,
    });
    // La misma vista que `pintar`: el mundo corrido el colchon, y lo que se
    // ve incluye el colchon (si no, `dibujar_orden` recortaria el trazo
    // alli donde asoma por el borde de la ventana).
    let holgura = margen / camara.zoom;
    let v = camara.ventana(ancho_px, alto_px);
    let vista = (v.0 - holgura, v.1 - holgura, v.2 + holgura, v.3 + holgura);
    let origen = camara.a_pantalla(Punto2::nuevo(0.0, 0.0));
    p.poner_vista(
        (0.0, 0.0),
        camara.zoom,
        (origen.x + margen, origen.y + margen),
    );
    let mut indice = 0u32;
    let grano = pixpin_motor2d::pintado::grano_de(e);
    por_cada_orden(cache, e, camara.zoom, escena.escala.as_ref(), |orden| {
        dibujar_orden(
            p,
            orden,
            vista,
            Some((&mut *cache_tinta, (e.id, e.version, indice))),
            imagenes,
            camara.zoom,
            grano,
        );
        indice += 1;
    });
    p.desplazar(0.0, 0.0);
    p.soltar_recorte();
}

/// **B2: un fotograma del trazo en curso, y nada mas.**
///
/// Limpia `zona` de la capa de tinta y pinta ahi el elemento que se esta
/// dibujando, con su punta predicha. La escena no se mira, no se copia y no
/// se presenta: lo que se ve debajo es el mismo fotograma de escena que ya
/// estaba en su visual desde antes de apoyar el lapiz.
///
/// Lo que se pinta es EXACTAMENTE lo mismo que pintaba `pintar` para el
/// elemento en curso (las mismas ordenes, la misma punta aparte): lo que
/// cambia es sobre que. Asi, apagar B2 desde el TOML no cambia la forma de
/// un trazo, solo donde se dibuja.
///
/// Devuelve si llego a pintar: si el gesto dejo de tener elemento en curso
/// entre la cola de eventos y aqui, no hay fotograma y quien llama tiene que
/// seguir pidiendolo.
#[allow(clippy::too_many_arguments)]
fn pintar_tinta_viva(
    motor: &MotorRender,
    superficie: &Superficie,
    escena: &Escena,
    camara: &Camara,
    gesto: &Gesto,
    cache: &mut Cache,
    imagenes: &ImagenesLienzo,
    zona: Option<(i32, i32, i32, i32)>,
    prediccion: Option<Punto2>,
    ancho_px: f32,
    alto_px: f32,
) -> bool {
    let Some((id, origen_trazo)) = gesto.elemento_en_curso() else {
        return false;
    };
    let Some(e) = escena.buscar(id) else {
        return false;
    };
    if e.borrado {
        return false;
    }
    let vista = camara.ventana(ancho_px, alto_px);
    superficie
        .pintar_tinta(motor, zona, |p, base| {
            // La capa de tinta va en pixeles de VENTANA (no lleva el colchon
            // de la escena); `base` es lo que DirectComposition pide sumar.
            let origen = camara.a_pantalla(Punto2::nuevo(0.0, 0.0));
            p.poner_vista(
                (0.0, 0.0),
                camara.zoom,
                (origen.x + base.0, origen.y + base.1),
            );
            // Lo mismo que hace `pintar` con el elemento en curso: la punta
            // de tinta va aparte (sale de las ultimas muestras, no de clonar
            // el elemento entero), y lo que no es tinta lleva la punta dentro
            // de una copia, que ahi son cuatro numeros.
            let mut punta = None;
            if let Some(q) = prediccion {
                match punta_de_tinta(e, q) {
                    Some(orden) => punta = Some(orden),
                    None => {
                        if let Some(copia) =
                            pixpin_motor2d::tinta::prediccion::con_punta(e, origen_trazo, q)
                        {
                            pintar_copia_predicha(
                                p,
                                &copia,
                                vista,
                                imagenes,
                                camara.zoom,
                                escena.escala.as_ref(),
                            );
                            return;
                        }
                    }
                }
            }
            let mut hubo_tinta = false;
            // Sin cache de realizaciones: la version del trazo en curso sube
            // en cada fotograma, asi que teselar para la cache seria pagar y
            // no cobrar nunca. Es lo mismo que hace `pintar`.
            por_cada_orden(cache, e, camara.zoom, escena.escala.as_ref(), |orden| {
                hubo_tinta |= matches!(orden, Orden::Tinta { .. });
                dibujar_orden(p, orden, vista, None, imagenes, camara.zoom, None);
            });
            if let (Some(orden), true) = (punta, hubo_tinta) {
                dibujar_orden(p, &orden, vista, None, imagenes, camara.zoom, None);
            }
        })
        .is_ok()
}

/// **Arrastrar como Excalidraw, o mejor: lo elegido en su propia capa.**
///
/// Excalidraw guarda un bitmap por elemento y al mover solo lo copia en el
/// sitio nuevo, sin volver a generar la figura (`elementWithCanvasCache`,
/// que se rehace por zoom o tema, nunca por posicion). Aqui se va un paso
/// mas alla: lo elegido y su marco se pintan UNA vez en la capa de la tinta
/// al empezar el arrastre, y cada aviso del raton despues es
/// `Superficie::desplazar_tinta` —una matriz y un `Commit`—: ni se copia ni
/// se repinta nada, lo mueve la composicion. Antes, cada aviso repintaba lo
/// elegido sin cache encima de la capa congelada.
///
/// Devuelve si llego a pintar.
#[allow(clippy::too_many_arguments)]
fn pintar_seleccion_en_capa(
    motor: &MotorRender,
    superficie: &Superficie,
    escena: &Escena,
    camara: &Camara,
    gesto: &Gesto,
    cache: &mut Cache,
    imagenes: &ImagenesLienzo,
    ancho_px: f32,
    alto_px: f32,
) -> bool {
    let vista = camara.ventana(ancho_px, alto_px);
    let escala = 1.0 / camara.zoom;
    superficie
        .pintar_tinta(motor, None, |p, base| {
            let origen = camara.a_pantalla(Punto2::nuevo(0.0, 0.0));
            p.poner_vista(
                (0.0, 0.0),
                camara.zoom,
                (origen.x + base.0, origen.y + base.1),
            );
            // En el orden de la escena, para que lo de encima siga encima.
            for e in &escena.elementos {
                if e.borrado
                    || guia_escondida(escena, e)
                    || !se_arrastra(gesto, e.id)
                    || tapar::es_mosaico(e)
                {
                    continue;
                }
                let grano = pixpin_motor2d::pintado::grano_de(e);
                por_cada_orden(cache, e, camara.zoom, escena.escala.as_ref(), |orden| {
                    dibujar_orden(p, orden, vista, None, imagenes, camara.zoom, grano);
                });
                // Una lupa viaja con lo de dentro (ya pintado: moverla no
                // lo cambia), no con el cristal vacio.
                lupas::pintar_en_capa(
                    p,
                    e,
                    camara,
                    base,
                    imagenes,
                    ancho_px as u32,
                    alto_px as u32,
                );
            }
            // El marco y los tiradores viajan con lo elegido: son la misma
            // llamada que usa `pintar`, para que no se desincronicen.
            if let Some(caja) = gesto
                .seleccion
                .caja(escena)
                .filter(|_| gesto.marco_visible())
            {
                let tiradores = gesto.tiradores(escena, escala);
                let angulo = tiradores.as_ref().map_or(0.0, |t| t.angulo);
                p.marco(caja, angulo, escala);
                if let Some(tiradores) = tiradores {
                    for orden in tiradores.ordenes(escala) {
                        dibujar_orden(p, &orden, vista, None, imagenes, camara.zoom, None);
                    }
                }
            }
        })
        .is_ok()
}

/// **La estela del laser en la capa de la tinta** (22-oct). El usuario: «el
/// puntero laser es lento a mas no poder». Iba con la escena: cada aviso
/// del raton, y cada fotograma mientras se apagaba, repintaba el lienzo
/// entero (con unas plantillas, miles de piezas) para dibujar cuatro rayas.
/// En la capa, como la marquesina, cuesta borrar la capa y esas rayas.
fn pintar_laser_en_capa(
    motor: &MotorRender,
    superficie: &Superficie,
    camara: &Camara,
    gesto: &Gesto,
    imagenes: &ImagenesLienzo,
    ancho_px: f32,
    alto_px: f32,
) -> bool {
    let ordenes = gesto.laser.ordenes(camara.zoom);
    let vista = camara.ventana(ancho_px, alto_px);
    superficie
        .pintar_tinta(motor, None, |p, base| {
            let origen = camara.a_pantalla(Punto2::nuevo(0.0, 0.0));
            p.poner_vista(
                (0.0, 0.0),
                camara.zoom,
                (origen.x + base.0, origen.y + base.1),
            );
            for o in &ordenes {
                dibujar_orden(p, o, vista, None, imagenes, camara.zoom, None);
            }
            if let Some((a, b)) = gesto.zona {
                p.marquesina(pixpin_motor2d::zona::caja(a, b), 1.0 / camara.zoom);
            }
        })
        .is_ok()
}

/// **Lo que se pinta al vuelo en la capa de la tinta** y no con la escena:
/// la estela del laser mientras vive y el recuadro de la Zona mientras se
/// arrastra (22-oct: «la herramienta de recorte es super lenta, no se mueve
/// con el mouse»; repintaba la escena entera en cada aviso del raton).
fn al_vuelo(gesto: &Gesto) -> bool {
    (gesto.herramienta == Herramienta::Laser && gesto.laser.vivo())
        || (gesto.herramienta == Herramienta::Zona && gesto.zona.is_some())
}

/// La marquesina en la capa de la tinta: solo el rectangulo que cambia, y
/// la escena sin tocar. Es el «lienzo interactivo» de Excalidraw: la
/// marquesina, el marco y lo que hay bajo el raton van en un lienzo aparte,
/// y arrastrarlos no repinta los elementos. `zona` va en pixeles de
/// ventana: la de ahora unida a la anterior, para borrar lo que ya no es.
fn pintar_marquesina_en_capa(
    motor: &MotorRender,
    superficie: &Superficie,
    camara: &Camara,
    m: (f32, f32, f32, f32),
    zona: (i32, i32, i32, i32),
) -> bool {
    let escala = 1.0 / camara.zoom;
    superficie
        .pintar_tinta(motor, Some(zona), |p, base| {
            let origen = camara.a_pantalla(Punto2::nuevo(0.0, 0.0));
            p.poner_vista(
                (0.0, 0.0),
                camara.zoom,
                (origen.x + base.0, origen.y + base.1),
            );
            p.marquesina(m, escala);
        })
        .is_ok()
}

/// El rectangulo de pantalla (pixeles de ventana) que ocupa una caja del
/// mundo, con un margen para el trazo del marco y los tiradores.
fn caja_en_pantalla(
    camara: &Camara,
    caja: (f32, f32, f32, f32),
    margen: f32,
) -> (i32, i32, i32, i32) {
    let a = camara.a_pantalla(Punto2::nuevo(caja.0, caja.1));
    let b = camara.a_pantalla(Punto2::nuevo(caja.2, caja.3));
    (
        (a.x.min(b.x) - margen).floor() as i32,
        (a.y.min(b.y) - margen).floor() as i32,
        (a.x.max(b.x) + margen).ceil() as i32,
        (a.y.max(b.y) + margen).ceil() as i32,
    )
}

/// **Los pixeles de VENTANA que puede manchar lo elegido**, con su marco y
/// sus tiradores, y tambien corrido `atras` (mundo): donde estaba antes de
/// moverse. `None` si no se puede acotar con seguridad; entonces quien llama
/// rehace la escena entera, que es lo de siempre.
///
/// Se acota con las mismas ordenes que se pintan (`por_cada_orden`, de la
/// cache: ya estan hechas, las pinto la capa), no con `Elemento::caja`: la
/// caja de un trazo a mano no cuenta el grosor de su tinta, y la de rough.js
/// no cuenta el temblor. Lo que no se sabe acotar -un velo, un texto que no
/// es de un elemento de texto (la cota de una medida, la etiqueta de una
/// flecha: su alto depende de como se parta en lineas) o algo girado que no
/// es geometria- dice `None`: quedarse corto dejaria un resto pegado en la
/// pantalla, y eso es peor que un fotograma lento.
fn zona_de_seleccion(
    escena: &Escena,
    gesto: &Gesto,
    camara: &Camara,
    cache: &mut Cache,
    atras: (f32, f32),
) -> Option<(i32, i32, i32, i32)> {
    let escala = 1.0 / camara.zoom;
    let mut u = gesto.seleccion.caja(escena)?;
    let unir = |u: &mut (f32, f32, f32, f32), c: (f32, f32, f32, f32)| {
        *u = (u.0.min(c.0), u.1.min(c.1), u.2.max(c.2), u.3.max(c.3));
    };
    let caja_de = |puntos: &[Punto2], holgura: f32| {
        let mut c = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in puntos {
            c = (c.0.min(p.x), c.1.min(p.y), c.2.max(p.x), c.3.max(p.y));
        }
        (c.0 - holgura, c.1 - holgura, c.2 + holgura, c.3 + holgura)
    };
    // Dos pixeles de pantalla por el suavizado de bordes.
    let suavizado = 2.0 * escala;
    for e in escena.elementos.iter() {
        if e.borrado || !se_arrastra(gesto, e.id) {
            continue;
        }
        let mut se_sabe = true;
        let es_texto = matches!(e.figura, Figura::Texto { .. });
        let es_marco = matches!(e.figura, Figura::Marco { .. });
        let girado = e.angulo != 0.0;
        por_cada_orden(
            cache,
            e,
            camara.zoom,
            escena.escala.as_ref(),
            |orden| match orden {
                Orden::Poligono { puntos, .. } | Orden::Relleno { puntos, .. } => {
                    if !puntos.is_empty() {
                        unir(&mut u, caja_de(puntos, suavizado));
                    }
                }
                // El aviso del grafito: su mapa, que `caja_de_tinta` ya cuenta.
                Orden::Tinta { contorno, .. } if contorno.is_empty() => {
                    if girado {
                        se_sabe = false;
                    } else {
                        unir(&mut u, caja_de_tinta(e));
                    }
                }
                Orden::Tinta { contorno, .. } => unir(&mut u, caja_de(contorno, suavizado)),
                // Las esquinas de una linea gruesa asoman de su caja; tres
                // grosores cubren una union en punta sin llegar a la de inglete
                // mas larga que Direct2D llega a pintar con su limite de 10.
                Orden::Polilinea { puntos, grosor, .. } => {
                    if !puntos.is_empty() {
                        unir(&mut u, caja_de(puntos, grosor * 5.0 + suavizado));
                    }
                }
                Orden::Imagen {
                    x, y, ancho, alto, ..
                } => {
                    if girado {
                        se_sabe = false;
                    } else {
                        unir(&mut u, (*x, *y, x + ancho, y + alto));
                    }
                }
                Orden::Texto {
                    texto,
                    x,
                    y,
                    tam,
                    ancho_max,
                    familia,
                    ..
                } => {
                    if es_texto && !girado {
                        let (c0, c1, c2, c3) = e.caja();
                        unir(
                            &mut u,
                            (
                                x.min(c0) - tam,
                                y.min(c1) - tam,
                                // Un texto suelto no se parte (`SIN_PARTIR`): lo
                                // pintado acaba en su caja, no en el ancho sin fin.
                                if *ancho_max >= pixpin_motor2d::texto::SIN_PARTIR {
                                    c2 + tam
                                } else {
                                    (x + ancho_max).max(c2) + tam
                                },
                                c3.max(*y) + tam,
                            ),
                        );
                    } else if es_marco && !girado {
                        // El nombre de un marco: encima de su raya, partido al
                        // ancho del marco. Baja hacia dentro del marco, cuya caja
                        // ya esta en la cuenta; si sus renglones no caben ni en
                        // el alto del marco, no se sabe. Sin esto, estirar un
                        // marco con nombre rehacia la escena entera en cada aviso.
                        let (c0, c1, c2, c3) = e.caja();
                        let (ancho, _) = pixpin_motor2d::texto::medida(
                            texto,
                            *tam,
                            familia,
                            pixpin_motor2d::texto::EstiloDeTexto::default(),
                        );
                        let renglones =
                            texto.split('\n').count() as f32 + (ancho / ancho_max.max(1.0)).floor();
                        let abajo = y + renglones * tam * 1.5;
                        if abajo <= c3 {
                            unir(
                                &mut u,
                                (
                                    x.min(c0) - tam,
                                    y - tam,
                                    (x + ancho_max).max(c2) + tam,
                                    c3.max(c1),
                                ),
                            );
                        } else {
                            se_sabe = false;
                        }
                    } else {
                        se_sabe = false;
                    }
                }
                // El numero de una cota va girado con su raya: el circulo que
                // barre su caja alrededor del centro, con el halo.
                Orden::Rotulo {
                    x,
                    y,
                    centro,
                    grosor_halo,
                    ..
                } => {
                    let r = (centro.x - x).hypot(centro.y - y) + grosor_halo + suavizado;
                    unir(
                        &mut u,
                        (centro.x - r, centro.y - r, centro.x + r, centro.y + r),
                    );
                }
                Orden::Velo { .. } => se_sabe = false,
            },
        );
        if !se_sabe {
            return None;
        }
    }
    // El marco va 4 px por fuera y los tiradores son cuadros de `LADO`
    // centrados en su sitio; el de girar va `SEPARACION_GIRO` por encima,
    // y esta entre los puntos de `tiradores`.
    if let Some(t) = gesto.tiradores(escena, escala) {
        for (_, q) in t.tamano {
            unir(&mut u, (q.x, q.y, q.x, q.y));
        }
        unir(&mut u, (t.giro.x, t.giro.y, t.giro.x, t.giro.y));
    }
    let h = (pixpin_motor2d::tiradores::LADO + 8.0) * escala;
    let u = (u.0 - h, u.1 - h, u.2 + h, u.3 + h);
    let antes = (u.0 + atras.0, u.1 + atras.1, u.2 + atras.0, u.3 + atras.1);
    let todo = (
        u.0.min(antes.0),
        u.1.min(antes.1),
        u.2.max(antes.2),
        u.3.max(antes.3),
    );
    Some(caja_en_pantalla(camara, todo, 2.0))
}

/// **Rehacer solo un trozo de la escena** y presentarlo: al empezar a
/// arrastrar (quitar lo elegido de donde estaba, que ahora va en la capa de
/// la tinta) y al soltar (pintarlo donde quedo).
///
/// Es `pintar` recortado a `zona_ventana` y sin la interfaz: limpiar el
/// trozo con el papel, la imagen de fondo, los elementos que la rejilla
/// dice que caen ahi -en el orden de la escena, que es el de pintado-, el
/// marco si lo elegido no esta en la capa y las marcas. Como se limpia y se
/// rehace todo lo que cae en el trozo, los pixeles son los mismos que daria
/// la escena entera; lo que cuesta es lo que hay EN la zona, no lo que hay
/// a la vista.
///
/// La cadena tiene dos mapas (D148): antes se iguala el de atras con el que
/// se ve (`igualar_trasero`), como en `hornear_trazo`. `None` si no se hizo
/// (un mosaico en la zona, cuya pasada lee lo de debajo; el dispositivo
/// perdido...): quien llama rehace la escena entera.
#[allow(clippy::too_many_arguments)]
fn repintar_zona(
    motor: &mut MotorRender,
    superficie: &Superficie,
    escena: &Escena,
    camara: &Camara,
    gesto: &Gesto,
    rejilla: &Rejilla,
    cache: &mut Cache,
    cache_tinta: &mut pixpin_render::CacheTinta,
    imagenes: &ImagenesLienzo,
    fondo: Option<&FondoLienzo>,
    marcas: Option<&marcas::Marcador>,
    escala_por_cien: u32,
    ancho_px: f32,
    alto_px: f32,
    zona_ventana: (i32, i32, i32, i32),
    fuera_seleccion: bool,
    trasero_distinto: Option<(i32, i32, i32, i32)>,
) -> Option<Horneado> {
    let margen = superficie.margen();
    let (ancho_sup, alto_sup) = tamano_escena(ancho_px, alto_px, margen);
    let zona = zona_en_superficie(zona_ventana, margen, ancho_sup, alto_sup)?;
    let mundo = mundo_de_zona(camara, zona, margen);
    let pisa_mosaico = escena.elementos.iter().any(|m| {
        !m.borrado && tapar::es_mosaico(m) && {
            let c = pixpin_motor2d::mosaico::caja_girada(m);
            c.0 <= mundo.2 && c.2 >= mundo.0 && c.1 <= mundo.3 && c.3 >= mundo.1
        }
    });
    if pisa_mosaico || lupas::pisa_una_lupa(&escena.elementos, mundo) {
        return None;
    }
    superficie.igualar_trasero(trasero_distinto).ok()?;
    let destino = superficie.empezar(motor).ok()?;
    let mut visibles = 0;
    let error = motor.dibujar(&destino, |p| {
        visibles = pintar_zona(
            p,
            escena,
            camara,
            gesto,
            rejilla,
            cache,
            cache_tinta,
            imagenes,
            fondo,
            marcas,
            escala_por_cien,
            margen,
            zona,
            fuera_seleccion,
            ancho_px,
            alto_px,
        );
    });
    if error.is_err() {
        return None;
    }
    let t_presentar = std::time::Instant::now();
    superficie.presentar_sincronizado(Some(zona)).ok()?;
    let m = margen as i32;
    Some(Horneado {
        ventana: (zona.0 - m, zona.1 - m, zona.2 - m, zona.3 - m),
        superficie: zona,
        presentar: t_presentar.elapsed(),
        visibles,
    })
}

/// Una zona de VENTANA pasada a pixeles de la superficie de escena (con el
/// colchon) y recortada a ella. `None` si queda vacia.
fn zona_en_superficie(
    z: (i32, i32, i32, i32),
    margen: f32,
    ancho_sup: u32,
    alto_sup: u32,
) -> Option<(i32, i32, i32, i32)> {
    let m = margen as i32;
    let zona = (
        (z.0 + m).max(0),
        (z.1 + m).max(0),
        (z.2 + m).min(ancho_sup as i32),
        (z.3 + m).min(alto_sup as i32),
    );
    (zona.2 > zona.0 && zona.3 > zona.1).then_some(zona)
}

/// La caja del mundo que cae en una zona de la superficie de escena.
fn mundo_de_zona(camara: &Camara, zona: (i32, i32, i32, i32), margen: f32) -> (f32, f32, f32, f32) {
    let a = camara.a_mundo(Punto2::nuevo(
        zona.0 as f32 - margen,
        zona.1 as f32 - margen,
    ));
    let b = camara.a_mundo(Punto2::nuevo(
        zona.2 as f32 - margen,
        zona.3 as f32 - margen,
    ));
    (a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y))
}

/// El dibujo de `repintar_zona`, aparte para que el banco de `medir.rs` lo
/// mida sin ventana. `zona` va en pixeles de la superficie. Devuelve cuantos
/// elementos recorrio.
#[allow(clippy::too_many_arguments)]
fn pintar_zona(
    p: &pixpin_render::Pintor<'_>,
    escena: &Escena,
    camara: &Camara,
    gesto: &Gesto,
    rejilla: &Rejilla,
    cache: &mut Cache,
    cache_tinta: &mut pixpin_render::CacheTinta,
    imagenes: &ImagenesLienzo,
    fondo: Option<&FondoLienzo>,
    marcas: Option<&marcas::Marcador>,
    escala_por_cien: u32,
    margen: f32,
    zona: (i32, i32, i32, i32),
    fuera_seleccion: bool,
    ancho_px: f32,
    alto_px: f32,
) -> u32 {
    let escala = 1.0 / camara.zoom;
    let mundo = mundo_de_zona(camara, zona, margen);
    let candidatos = rejilla.candidatos(mundo);
    p.empujar_recorte(RectF {
        x: zona.0 as f32,
        y: zona.1 as f32,
        ancho: (zona.2 - zona.0) as f32,
        alto: (zona.3 - zona.1) as f32,
    });
    // Lo mismo que `pintar` sin la capa congelada: el papel, la vista del
    // mundo corrida el colchon y la imagen de fondo debajo de todo.
    p.limpiar(a_color(escena.fondo));
    let holgura = margen / camara.zoom;
    let v = camara.ventana(ancho_px, alto_px);
    let vista = (v.0 - holgura, v.1 - holgura, v.2 + holgura, v.3 + holgura);
    let origen = camara.a_pantalla(Punto2::nuevo(0.0, 0.0));
    p.poner_vista(
        (0.0, 0.0),
        camara.zoom,
        (origen.x + margen, origen.y + margen),
    );
    if let Some(f) = fondo {
        f.pintar(p, vista, camara.zoom);
    }
    let visibles = candidatos.len() as u32;
    for id in candidatos {
        if fuera_seleccion && se_arrastra(gesto, id) {
            continue;
        }
        let Some(e) = elemento_por_id(escena, rejilla, id) else {
            continue;
        };
        if e.borrado || guia_escondida(escena, e) || tapar::es_mosaico(e) {
            continue;
        }
        let mut indice = 0u32;
        let grano = pixpin_motor2d::pintado::grano_de(e);
        por_cada_orden(cache, e, camara.zoom, escena.escala.as_ref(), |orden| {
            dibujar_orden(
                p,
                orden,
                vista,
                Some((&mut *cache_tinta, (e.id, e.version, indice))),
                imagenes,
                camara.zoom,
                grano,
            );
            indice += 1;
        });
    }
    // El marco y los tiradores, con la misma llamada que `pintar`.
    // Mientras se escribe, sin marco y con el cursor (`Gesto::marco_visible`).
    if !fuera_seleccion
        && gesto.marco_visible()
        && let Some(caja) = gesto.seleccion.caja(escena)
    {
        let tiradores = gesto.tiradores(escena, escala);
        let angulo = tiradores.as_ref().map_or(0.0, |t| t.angulo);
        p.marco(caja, angulo, escala);
        if let Some(tiradores) = tiradores {
            for orden in tiradores.ordenes(escala) {
                dibujar_orden(p, &orden, vista, None, imagenes, camara.zoom, None);
            }
        }
    }
    for orden in gesto.seleccion_de_texto(escena) {
        dibujar_orden(p, &orden, vista, None, imagenes, camara.zoom, None);
    }
    if let Some(cursor) = gesto.cursor_de_texto(escena, camara.zoom) {
        dibujar_orden(p, &cursor, vista, None, imagenes, camara.zoom, None);
    }
    if let Some(m) = marcas {
        m.pintar_en_la_escena(p, camara, margen, ancho_px, alto_px, escala_por_cien);
    }
    p.desplazar(0.0, 0.0);
    p.soltar_recorte();
    visibles
}

/// Lo que hizo `atender_en_capa` con un aviso del raton.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Compuesto {
    /// Nada: el aviso sigue su camino de siempre (`Region` del gesto).
    No,
    /// Atendido en la capa; la escena no se repinta por el.
    Si,
    /// Atendido, pero la escena se rehace una vez: al empezar (para quitar
    /// de ella lo que ahora esta en la capa) o al terminar (para devolverlo).
    EscenaEntera,
    /// Lo mismo, pero basta con rehacer este trozo (pixeles de VENTANA): el
    /// sitio de lo elegido. Ver `repintar_zona`.
    Zona((i32, i32, i32, i32)),
}

/// **Decide si este aviso del raton se atiende en la capa de la tinta.**
///
/// Arrastrar lo elegido y la marquesina son las dos cosas que la seleccion
/// hace en cada aviso, y las dos se pueden hacer sin tocar la escena. Si no
/// se puede (sin capas, con el universo, con la camara recien movida por
/// composicion, o con lo elegido asomando fuera de la ventana, donde la capa
/// no llega), se devuelve `No` y todo sigue como antes.
#[allow(clippy::too_many_arguments)]
fn atender_en_capa(
    en_capa: &mut Option<EnCapa>,
    vaciar_tras_pintar: &mut bool,
    motor: &MotorRender,
    superficie: &Superficie,
    escena: &Escena,
    camara: &Camara,
    gesto: &Gesto,
    cache: &mut Cache,
    imagenes: &ImagenesLienzo,
    ancho_px: f32,
    alto_px: f32,
    se_puede: bool,
    camara_al_dia: bool,
    caja_al_pulsar: Option<(f32, f32, f32, f32)>,
) -> Compuesto {
    // Lo que ya esta en marcha: seguirlo o terminarlo.
    match *en_capa {
        Some(EnCapa::Arrastre { ancla, camara: c }) => {
            if gesto.moviendo() && c == *camara {
                if let Some((x0, y0, _, _)) = gesto.seleccion.caja(escena) {
                    superficie.desplazar_tinta(
                        (x0 - ancla.0) * camara.zoom,
                        (y0 - ancla.1) * camara.zoom,
                    );
                }
                return Compuesto::Si;
            }
            *en_capa = None;
            *vaciar_tras_pintar = true;
            // Al soltar, la escena ya no tiene lo elegido en su sitio viejo
            // (se quito al empezar): solo falta pintarlo en el nuevo. Con la
            // camara cambiada, la escena entera.
            if c == *camara
                && let Some(z) = zona_de_seleccion(escena, gesto, camara, cache, (0.0, 0.0))
            {
                return Compuesto::Zona(z);
            }
            return Compuesto::EscenaEntera;
        }
        Some(EnCapa::Marquesina { zona, camara: c }) => match gesto.marquesina() {
            Some(m) if c == *camara => {
                let ahora = caja_en_pantalla(camara, caja_ordenada(m), 4.0);
                let union = (
                    ahora.0.min(zona.0),
                    ahora.1.min(zona.1),
                    ahora.2.max(zona.2),
                    ahora.3.max(zona.3),
                );
                pintar_marquesina_en_capa(motor, superficie, camara, m, union);
                *en_capa = Some(EnCapa::Marquesina {
                    zona: ahora,
                    camara: c,
                });
                return Compuesto::Si;
            }
            _ => {
                *en_capa = None;
                *vaciar_tras_pintar = true;
                return Compuesto::EscenaEntera;
            }
        },
        None => {}
    }
    if !se_puede || !camara_al_dia {
        return Compuesto::No;
    }
    let arrastre = gesto.moviendo();
    let marquesina = gesto.marquesina();
    if !arrastre && marquesina.is_none() {
        return Compuesto::No;
    }
    // Lo que quedo del arrastre anterior, si la escena todavia no lo ha
    // recogido: fuera, que la capa se va a usar para otra cosa.
    if *vaciar_tras_pintar {
        superficie.apagar_tinta(motor);
        *vaciar_tras_pintar = false;
    }
    if arrastre {
        // **Con flechas atadas que no van dentro de lo elegido, la capa no
        // vale.** La capa se pinta UNA vez y la composicion la desplaza;
        // esas flechas, en cambio, cambian de forma en cada aviso (una punta
        // va con la caja y la otra se queda). Se podria dejar lo elegido en
        // la capa y repintar solo las flechas, pero sin la capa congelada
        // —que no se hornea mientras hay capa— el repintado parcial de
        // `pintar` se convierte en la escena entera en cada aviso, que es
        // peor que el camino de siempre: capa congelada horneada una vez y
        // encima lo elegido mas esas flechas (`es_excluido`).
        if !gesto.flechas_que_siguen().is_empty() {
            return Compuesto::No;
        }
        let Some(caja) = gesto.seleccion.caja(escena) else {
            return Compuesto::No;
        };
        // La capa mide lo que la ventana: lo que asome por fuera al empezar
        // no estaria en ella y no apareceria al arrastrarlo hacia dentro.
        let r = caja_en_pantalla(camara, caja, 16.0);
        if r.0 < 0 || r.1 < 0 || r.2 > ancho_px as i32 || r.3 > alto_px as i32 {
            return Compuesto::No;
        }
        if superficie.encender_tinta(motor).is_err() {
            return Compuesto::No;
        }
        if !pintar_seleccion_en_capa(
            motor, superficie, escena, camara, gesto, cache, imagenes, ancho_px, alto_px,
        ) {
            superficie.apagar_tinta(motor);
            return Compuesto::No;
        }
        *en_capa = Some(EnCapa::Arrastre {
            ancla: (caja.0, caja.1),
            camara: *camara,
        });
        // **Quitar lo elegido de la escena, solo alli donde estaba.** Antes
        // era la escena entera: un fotograma de O(visibles) justo al coger
        // la figura -de 20 a 70 ms en la HD 4000 con unos cientos de trazos-,
        // el tiron que se notaba al empezar a arrastrar. Este aviso ya movio
        // lo elegido, y lo pintado sigue donde estaba al pulsar: la zona es
        // la de ahora corrida lo que se movio desde entonces. Sin la caja de
        // cuando se pulso no se sabe donde estaba: la escena entera.
        if let Some(antes) = caja_al_pulsar {
            let atras = (antes.0 - caja.0, antes.1 - caja.1);
            if let Some(z) = zona_de_seleccion(escena, gesto, camara, cache, atras) {
                return Compuesto::Zona(z);
            }
        }
        return Compuesto::EscenaEntera;
    }
    let Some(m) = marquesina else {
        return Compuesto::No;
    };
    if superficie.encender_tinta(motor).is_err() {
        return Compuesto::No;
    }
    let ahora = caja_en_pantalla(camara, caja_ordenada(m), 4.0);
    pintar_marquesina_en_capa(motor, superficie, camara, m, ahora);
    *en_capa = Some(EnCapa::Marquesina {
        zona: ahora,
        camara: *camara,
    });
    // Al empezar la marquesina se suelta la seleccion de antes: la escena
    // tiene que quitar su marco una vez.
    Compuesto::EscenaEntera
}

/// Una caja con las esquinas en cualquier orden, ordenada.
fn caja_ordenada(m: (f32, f32, f32, f32)) -> (f32, f32, f32, f32) {
    (m.0.min(m.2), m.1.min(m.3), m.0.max(m.2), m.1.max(m.3))
}

/// Lo que esta pasando en la capa de la tinta cuando no hay trazo en curso.
#[derive(Debug, Clone, Copy, PartialEq)]
enum EnCapa {
    /// Lo elegido se arrastra por composicion. `ancla` es la esquina de la
    /// caja de la seleccion cuando se pinto en la capa, y `camara`, la
    /// camara con la que se pinto: si cambia, la capa ya no vale.
    Arrastre { ancla: (f32, f32), camara: Camara },
    /// La marquesina, con la zona de pantalla que se pinto la ultima vez.
    Marquesina {
        zona: (i32, i32, i32, i32),
        camara: Camara,
    },
}

/// Lo que se le deja de pedir a `pintar` porque ya esta en la capa.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct FueraDeLaEscena {
    /// Lo elegido (y su marco) se esta arrastrando en la capa.
    seleccion: bool,
    /// La marquesina se pinta en la capa.
    marquesina: bool,
}

/// Lo que el usuario teclea al calibrar, convertido a numero.
///
/// Acepta coma y punto porque se escribe una cosa o la otra segun la
/// costumbre y el teclado, y obligar a una sola es hacerle repetir el gesto
/// a la mitad de la gente.
fn leer_medida(texto: &str) -> Option<f32> {
    let v: f32 = texto.trim().replace(',', ".").parse().ok()?;
    if !v.is_finite() || v <= 0.0 {
        return None;
    }
    Some(v)
}

/// El cajetin de calibrar: se escribe el numero y se elige la unidad de
/// `pixpin_motor2d::medida::UNIDADES` con Tab, siguiendo el mismo estilo de
/// tarjeta oscura que `apps/pixpin/src/overlay.rs` usa para el panel
/// «Seleccionar todo» y la barra de resultado.
///
/// Abre su propio bucle de eventos porque necesita seguir pintando el
/// fotograma (con el dialogo encima) mientras espera lo que se teclea; por
/// eso su prueba necesita sesion de escritorio y va marcada `#[ignore]`.
///
/// Devuelve `None` si se cancela con Escape o si `leer_medida` rechaza lo
/// tecleado al pulsar Enter. **Si al cancelar la escala cambiara, seria un
/// fallo**: quien llama no tiene que tocar la escena cuando esto devuelve
/// `None`, y por eso aqui no se toca `escena` en ningun caso: solo se lee.
#[allow(clippy::too_many_arguments)]
fn pedir_medida(
    ventana: &VentanaOverlay,
    motor: &mut MotorRender,
    superficie: &Superficie,
    escena: &Escena,
    camara: &Camara,
    gesto: &Gesto,
    cache: &mut Cache,
    cache_tinta: &mut pixpin_render::CacheTinta,
    rejilla: &Rejilla,
    capa: &CapaEstatica,
    fondo: &mut Option<FondoLienzo>,
    imagenes: &mut ImagenesLienzo,
    caja: &CajaHerramientas,
    corrimiento_ui: (f32, f32),
    escala_por_cien: u32,
    ancho_px: f32,
    alto_px: f32,
    largo_px: f32,
) -> Option<(f32, String)> {
    let mut texto = String::new();
    let mut indice_unidad = 0usize;
    ventana.invalidar();

    loop {
        pixpin_shell::overlay::bombear_pendientes();
        for (hwnd, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            match ev {
                EventoOverlay::Caracter(c) => match c {
                    '\r' | '\n' => {
                        let unidad = pixpin_motor2d::medida::UNIDADES[indice_unidad];
                        return leer_medida(&texto).map(|v| (v, unidad.to_string()));
                    }
                    '\u{1b}' => return None,
                    '\u{8}' => {
                        texto.pop();
                        ventana.invalidar();
                    }
                    '\t' => {
                        indice_unidad =
                            (indice_unidad + 1) % pixpin_motor2d::medida::UNIDADES.len();
                        ventana.invalidar();
                    }
                    c if c.is_ascii_digit() || c == ',' || c == '.' => {
                        texto.push(c);
                        ventana.invalidar();
                    }
                    _ => {}
                },
                EventoOverlay::Cerrar => return None,
                EventoOverlay::Pintar => {
                    let unidad = pixpin_motor2d::medida::UNIDADES[indice_unidad];
                    pintar(
                        motor,
                        superficie,
                        escena,
                        camara,
                        gesto,
                        cache,
                        cache_tinta,
                        rejilla,
                        capa,
                        fondo,
                        imagenes,
                        caja,
                        None,
                        corrimiento_ui,
                        true,
                        escala_por_cien,
                        ancho_px,
                        alto_px,
                        None,
                        None,
                        None,
                        // El cajetin cambia con cada tecla: la capa de la
                        // interfaz se rehace en cada fotograma de este
                        // bucle, que dura lo que tarde en escribirse un
                        // numero.
                        true,
                        FueraDeLaEscena::default(),
                        // El cajetin es un momento: sin marcas.
                        None,
                        |p, base| {
                            dibujar_cajetin(p, base, ancho_px, alto_px, largo_px, &texto, unidad)
                        },
                    );
                }
                _ => {}
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// La tarjeta del cajetin de calibrar, centrada en la ventana y en
/// coordenadas de pantalla (no del mundo: un dialogo no se mueve con la
/// camara). `Pintor::desplazar(0.0, 0.0)` deshace la vista del mundo que
/// `pintar` dejo puesta antes de llamar a `encima`.
fn dibujar_cajetin(
    p: &pixpin_render::Pintor<'_>,
    base: (f32, f32),
    ancho_px: f32,
    alto_px: f32,
    largo_px: f32,
    texto: &str,
    unidad: &str,
) {
    // `base` es 0 cuando el cajetin se pinta en la misma superficie que la
    // escena, y el desplazamiento que dio DirectComposition cuando va en la
    // capa de la interfaz (A3).
    p.desplazar(base.0, base.1);

    let ancho = 260.0;
    let alto = 90.0;
    let caja = RectF {
        x: (ancho_px - ancho) / 2.0,
        y: (alto_px - alto) / 2.0,
        ancho,
        alto,
    };
    p.rellenar_redondeado(
        caja,
        8.0,
        Color {
            r: 0.12,
            g: 0.12,
            b: 0.14,
            a: 0.92,
        },
    );

    let pregunta = format!("Cuanto miden {largo_px:.0} px?");
    p.texto(&pregunta, caja.x + 16.0, caja.y + 12.0, 13.0, Color::BLANCO);

    let entrada = format!("{texto} {unidad}");
    let (tw, th) = p.medir_texto(&entrada, 20.0);
    p.texto(
        &entrada,
        caja.x + (caja.ancho - tw) / 2.0,
        caja.y + 32.0 + (alto - 32.0 - th) / 2.0,
        20.0,
        Color::BLANCO,
    );
}

/// Hornear la capa congelada (lo que no se mueve), para dos gestos.
mod congelar;
// El lienzo en ventana (con la barra de los pines) o a pantalla completa.
mod modo_ventana;
// Abrir mirando y el lapiz del riel para editar (24-oct).
/// La pasada de tapado del mosaico. Ver su cabecera: se engancha en dos
/// sitios del fotograma y los dos estan en `pintar`.
/// El cajetin de la cota: cuanto mide y hacia donde va (lienzo-geometria).
mod cota;
/// Exportar (PNG, SVG, PDF, web) e imprimir; lo usa tambien el chat.
pub(crate) mod exportar;
/// F8, F12, F14: la zona, la grafica, las tablas, la biblioteca y la imagen.
mod figuras;
mod mirar_editar;
/// Donde viven las figuras guardadas: el asistente guarda y estampa en la
/// misma biblioteca que el panel del lienzo.
pub(crate) use figuras::ruta_biblioteca;
/// La hojita: el recado que se pega como pin (F6).
mod hojita;
/// La pasada de las lupas: lo que mira cada una, agrandado (lienzo-imagen).
mod lupas;
/// Los marcadores con emoticono y su riel (F5).
pub(crate) mod marcas;
/// El anotador de pantalla: este mismo editor encima del escritorio.
pub(crate) mod pantalla;
/// La pastilla del anotador de pantalla: atravesar, limpiar, copiar, guardar
/// y salir, en su propia ventana (`CapaPantalla.kt`).
pub(crate) mod pastilla_pantalla;
/// F8: la pastilla de la Zona y su interruptor «Al chat».
mod pastilla_zona;
/// G5: presentar por marcos (F5) y solo mirar (Alt+R).
mod presentar;
/// La imagen de referencia flotando encima del lienzo (`VentanaDeReferencia`).
mod referencia;
mod tapar;

#[cfg(test)]
pub(crate) mod medir;
/// Lo que cuesta cada aviso al arrastrar lupa, grafica, tabla y grupos.
#[cfg(test)]
mod medir_arrastre;

/// **Una linea de referencia escondida** (`referenciasVisibles` a falso): no
/// se pinta, como en el movil. Ver `Escena::a_la_vista`.
fn guia_escondida(escena: &Escena, e: &pixpin_motor2d::Elemento) -> bool {
    !escena.referencias_visibles && e.extras.referencia
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::TipoPunta;

    /// Un trazo a mano de `n` puntos en linea recta.
    fn trazo_de(n: usize) -> Elemento {
        let mut e = elemento_de_prueba();
        e.figura = Figura::Lapiz {
            puntos: (0..n).map(|i| Punto2::nuevo(i as f32, 0.0)).collect(),
            presiones: (0..n).map(|_| 0.5).collect(),
            opciones: Some(Default::default()),
        };
        e
    }

    fn elemento_de_prueba() -> Elemento {
        Elemento {
            id: 1,
            figura: Figura::Rectangulo,
            x: 0.0,
            y: 0.0,
            ancho: 10.0,
            alto: 10.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            estilo_relleno: Default::default(),
            relleno: None,
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
            bloqueado: false,
            enlace: None,
            redondo: false,
            material: Default::default(),
            extras: Default::default(),
        }
    }

    #[test]
    fn la_punta_predicha_cuesta_lo_mismo_con_un_trazo_corto_que_con_uno_larguisimo() {
        // B3: antes, la punta clonaba el elemento y recalculaba su contorno
        // entero, asi que al final de un trazo largo costaba una segunda
        // pasada por los 5.000 puntos. Ahora sale de la cola y su contorno
        // tiene el mismo tamano venga de donde venga.
        let q = Punto2::nuevo(9_000.0, 0.0);
        let corto = punta_de_tinta(&trazo_de(40), q).expect("un trazo de 40 tiene punta");
        let largo = punta_de_tinta(&trazo_de(5_000), q).expect("y uno de 5.000 tambien");
        let cuantos = |o: &Orden| match o {
            Orden::Tinta { contorno, .. } => contorno.len(),
            _ => unreachable!("un lapiz da tinta"),
        };
        assert_eq!(cuantos(&corto), cuantos(&largo));
    }

    #[test]
    fn la_punta_acaba_en_el_punto_predicho_y_no_en_el_ultimo_real() {
        let q = Punto2::nuevo(500.0, 0.0);
        let Some(Orden::Tinta { contorno, .. }) = punta_de_tinta(&trazo_de(100), q) else {
            unreachable!("un lapiz da tinta")
        };
        let mas_lejos = contorno.iter().fold(0.0f32, |m, p| m.max(p.x));
        assert!(mas_lejos > 490.0, "la punta llega hasta {mas_lejos}");
    }

    #[test]
    fn lo_que_no_es_tinta_no_tiene_punta_aparte() {
        // Caso negativo: una caja o una flecha siguen usando la copia
        // entera, que en su caso son cuatro numeros y no cuesta nada.
        let q = Punto2::nuevo(5.0, 5.0);
        assert!(punta_de_tinta(&elemento_de_prueba(), q).is_none());
        let mut flecha = elemento_de_prueba();
        flecha.figura = Figura::Flecha {
            puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(1.0, 1.0)],
            punta_inicio: TipoPunta::Ninguna,
            punta_fin: TipoPunta::Flecha,
            codos: false,
        };
        assert!(punta_de_tinta(&flecha, q).is_none());
        // Y un trazo de dos puntos tampoco: no hay cola de la que tirar.
        assert!(punta_de_tinta(&trazo_de(2), q).is_none());
    }

    #[test]
    fn una_figura_de_grafito_en_curso_va_por_la_copia_predicha_y_esa_copia_se_cuece_de_grafito() {
        use pixpin_motor2d::tinta::{MaterialTinta, grafito};
        let q = Punto2::nuevo(160.0, 110.0);
        let mut caja = elemento_de_prueba();
        caja.figura = Figura::Rectangulo;
        caja.material = MaterialTinta::Cuadritos;
        (caja.x, caja.y, caja.ancho, caja.alto) = (10.0, 10.0, 120.0, 80.0);
        // Sin punta aparte: va por la copia entera...
        assert!(punta_de_tinta(&caja, q).is_none());
        // ...y la copia sigue siendo de grafito, con la punta dentro: es lo
        // que `pintar_copia_predicha` cuece en vez de pintarla lisa.
        let copia =
            pixpin_motor2d::tinta::prediccion::con_punta(&caja, Some(Punto2::nuevo(10.0, 10.0)), q)
                .expect("una caja en curso tiene copia predicha");
        let cocido = grafito::cocer_sin_horno(&copia).expect("la copia se cuece de grafito");
        let (x0, y0, w, h) = cocido.caja();
        assert!(
            x0 + w >= 160.0 && y0 + h >= 110.0,
            "el mapa no llega a la punta"
        );
        // Caso negativo: el trazo a mano de grafito si lleva su aviso sin
        // marca, que no pinta nada: una punta lisa se veria como una gota.
        let mut trazo = trazo_de(40);
        trazo.material = MaterialTinta::Cuadritos;
        assert!(matches!(
            punta_de_tinta(&trazo, q),
            Some(Orden::Tinta { contorno, .. }) if contorno.is_empty()
        ));
    }

    #[test]
    fn sin_fijar_nada_el_lapiz_se_comporta_como_siempre() {
        // El ajuste de C1 va apagado de fabrica: abrir un editor sin haber
        // llamado a `fijar_ajustes_tinta` no puede cambiar el trazo.
        assert_eq!(
            ajustes_tinta().suavizado,
            pixpin_store::Suavizado::Excalidraw
        );
    }

    #[test]
    fn la_tecla_pintada_en_cada_boton_elige_esa_herramienta() {
        for b in pixpin_ui::BOTONES_EDITOR {
            if let BotonCaja::Elegir(h) = b {
                if let Some(c) = tecla_de(h) {
                    assert_eq!(tecla_a_herramienta(c), Some(h), "{c} no elige {h:?}");
                    assert_eq!(tecla_a_herramienta(c.to_ascii_lowercase()), Some(h));
                }
            }
        }
        // Caso negativo: el rectangulo no tiene tecla y no se pinta ninguna.
        assert_eq!(tecla_de(Herramienta::Rectangulo), None);
        assert_eq!(tecla_de(Herramienta::Lapiz), Some('L'));
    }
    use pixpin_geom::Tirador;
    use pixpin_motor2d::camara::Camara;
    use pixpin_motor2d::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};
    use pixpin_motor2d::escena::Escena;
    use pixpin_motor2d::gesto::{EventoGesto, Gesto};
    use std::f32::consts::{FRAC_PI_2, PI};

    fn camara_de_prueba(x: f32, y: f32, zoom: f32) -> Camara {
        Camara { x, y, zoom }
    }

    #[test]
    fn un_paneo_que_cabe_en_el_colchon_se_resuelve_moviendo_el_visual() {
        // 100 px de mundo a zoom 1 son 100 px de pantalla, y el colchon es
        // de 256: cabe de sobra, y la escala no cambia.
        let c0 = camara_de_prueba(0.0, 0.0, 1.0);
        let c1 = camara_de_prueba(100.0, 40.0, 1.0);
        let (s, dx, dy) =
            transformada_de_camara(&c0, &c1, 1920.0, 1080.0, 256.0).expect("cabe en el colchon");
        assert_eq!(s, 1.0);
        // La camara se fue a la derecha, asi que el dibujo se mueve a la
        // izquierda: el signo importa y equivocarlo mueve el lienzo al reves.
        assert_eq!((dx, dy), (-100.0, -40.0));
    }

    #[test]
    fn un_paneo_que_se_pasa_del_colchon_pide_repintar() {
        // Caso negativo: 300 px de pantalla con 256 de colchon dejaria una
        // franja vacia por el borde. Mas vale pintar que ensenar el hueco.
        let c0 = camara_de_prueba(0.0, 0.0, 1.0);
        let c1 = camara_de_prueba(-300.0, 0.0, 1.0);
        assert!(transformada_de_camara(&c0, &c1, 1920.0, 1080.0, 256.0).is_none());
    }

    #[test]
    fn acercarse_un_poco_se_compone_y_alejarse_mucho_no() {
        let c0 = camara_de_prueba(0.0, 0.0, 1.0);
        // Acercar siempre cabe: se ve un trozo de lo ya pintado.
        let cerca = camara_de_prueba(0.0, 0.0, 1.1);
        assert_eq!(
            transformada_de_camara(&c0, &cerca, 1920.0, 1080.0, 256.0).map(|t| t.0),
            Some(1.1)
        );
        // Alejar tiene tope: con la mitad de aumento harian falta 1920 px
        // de superficie de mas y solo hay 512.
        let lejos = camara_de_prueba(0.0, 0.0, 0.5);
        assert!(transformada_de_camara(&c0, &lejos, 1920.0, 1080.0, 256.0).is_none());
    }

    #[test]
    fn recien_compuesto_la_tinta_toca_ya_y_a_mitad_de_periodo_espera() {
        // Ya paso el periodo: toca pintar sin esperar.
        assert_eq!(espera_de_tinta(17.0, 16.7), None);
        // Y justo en el 90 % tambien: el plazo es ese, no el periodo entero
        // (si no, la deriva entre el reloj del bucle y el de DWM perderia
        // una composicion de cada dos y el trazo saldria a 30 Hz).
        assert_eq!(espera_de_tinta(16.7 * 0.9, 16.7), None);
        // A mitad de periodo se espera lo que falta, redondeado hacia
        // arriba: dormir de menos es volver a la misma pregunta.
        let e = espera_de_tinta(8.0, 16.7).expect("todavia no toca");
        assert!((7..=8).contains(&e), "{e} ms");
        // Nunca 0: un tope de 0 ms es girar en vacio.
        assert_eq!(espera_de_tinta(16.7 * 0.9 - 0.01, 16.7), Some(1));
        // Caso negativo: sin periodo que valga no se frena nada.
        assert_eq!(espera_de_tinta(0.0, 0.0), None);
        assert_eq!(espera_de_tinta(f32::NAN, 16.7), None);
    }

    #[test]
    fn sin_colchon_nunca_se_compone() {
        // Es la palanca del TOML: con `paneo_por_composicion = false` la
        // superficie nace sin margen y este camino no se toma nunca.
        let c0 = camara_de_prueba(0.0, 0.0, 1.0);
        let c1 = camara_de_prueba(1.0, 0.0, 1.0);
        assert!(transformada_de_camara(&c0, &c1, 1920.0, 1080.0, 0.0).is_none());
    }

    #[test]
    fn un_estirado_enorme_pide_repintar_aunque_quepa() {
        // Acercar x10 «cabe» en el sentido de que se ve, pero seria una
        // textura ampliada diez veces: se ve borrosa y hay que pintar.
        let c0 = camara_de_prueba(0.0, 0.0, 1.0);
        let c1 = camara_de_prueba(0.0, 0.0, 10.0);
        assert!(transformada_de_camara(&c0, &c1, 1920.0, 1080.0, 256.0).is_none());
    }

    #[test]
    fn la_superficie_de_escena_es_la_ventana_mas_el_colchon_por_cada_lado() {
        assert_eq!(tamano_escena(1920.0, 1080.0, 256.0), (2432, 1592));
        // Sin colchon, exactamente la ventana: es lo que espera la capa
        // congelada de siempre.
        assert_eq!(tamano_escena(1920.0, 1080.0, 0.0), (1920, 1080));
    }

    /// Revision de la Tarea 10, hallazgo 1a: sin zoom pendiente
    /// (`zoom == escala`), no hay que dormir con un tope -y mucho menos
    /// `Some(0)`, que giraria un nucleo entero en reposo-.
    #[test]
    fn sin_zoom_pendiente_no_hay_tope_ni_hay_que_rehacer() {
        let ahora = std::time::Instant::now();
        let retardo = std::time::Duration::from_millis(300);
        let d = decidir_zoom(1.0, 1.0, 1.0, None, ahora, retardo);
        assert!(!d.rehacer);
        assert_eq!(d.tope_ms, None);
        assert_eq!(d.desde, None);
    }

    #[test]
    fn un_cambio_de_zoom_pone_el_tope_al_retardo_completo() {
        let ahora = std::time::Instant::now();
        let retardo = std::time::Duration::from_millis(300);
        let d = decidir_zoom(2.0, 1.0, 1.0, None, ahora, retardo);
        assert!(!d.rehacer);
        assert_eq!(d.visto, 2.0);
        assert_eq!(d.tope_ms, Some(300));
    }

    /// Hallazgo 1a: si el zoom vuelve a la escala YA realizada antes de que
    /// se cumpla el retardo, no puede quedar un tope fijo -eso era el giro
    /// en vacio que violaba el 0% de CPU en reposo-.
    #[test]
    fn volver_a_la_escala_realizada_antes_del_retardo_no_deja_tope_pendiente() {
        let t0 = std::time::Instant::now();
        let retardo = std::time::Duration::from_millis(300);
        let d1 = decidir_zoom(2.0, 1.0, 1.0, None, t0, retardo);
        assert_eq!(d1.tope_ms, Some(300));
        let t1 = t0 + std::time::Duration::from_millis(50);
        let d2 = decidir_zoom(1.0, d1.visto, 1.0, d1.desde, t1, retardo);
        assert!(!d2.rehacer);
        assert_eq!(
            d2.tope_ms, None,
            "no puede quedar un tope fijo (giraria un nucleo en reposo)"
        );
        assert_eq!(d2.desde, None);
    }

    /// Hallazgo 1b: "quieto" cuenta desde el ULTIMO cambio, no desde el
    /// primero. Un zoom que sigue cambiando cada 100 ms durante 1 s no
    /// tiene que rehacer nunca; solo cuando por fin se para, 300 ms
    /// despues de su ultimo cambio.
    #[test]
    fn el_zoom_continuo_no_rehace_hasta_que_se_queda_quieto_300_ms() {
        let t0 = std::time::Instant::now();
        let retardo = std::time::Duration::from_millis(300);
        let mut visto = 1.0f32;
        let mut desde = None;
        let mut zoom = 1.0f32;
        for i in 1..=10u64 {
            zoom += 0.1;
            let ahora = t0 + std::time::Duration::from_millis(i * 100);
            let d = decidir_zoom(zoom, visto, 1.0, desde, ahora, retardo);
            assert!(
                !d.rehacer,
                "no debe rehacer mientras el zoom sigue cambiando (vuelta {i})"
            );
            visto = d.visto;
            desde = d.desde;
        }
        // El ultimo cambio fue en t = 1000 ms; a los 300 ms de quietud
        // (t = 1300 ms) toca rehacer, con el tope ya a None.
        let quieto_pero_no_del_todo = t0 + std::time::Duration::from_millis(1299);
        let d = decidir_zoom(zoom, visto, 1.0, desde, quieto_pero_no_del_todo, retardo);
        assert!(!d.rehacer, "todavia no han pasado los 300 ms completos");
        let d = decidir_zoom(
            zoom,
            d.visto,
            1.0,
            d.desde,
            t0 + std::time::Duration::from_millis(1300),
            retardo,
        );
        assert!(d.rehacer);
        assert_eq!(d.tope_ms, None);
    }

    /// El hallazgo 3 de la revision final: las tres herramientas de medir
    /// estaban implementadas y probadas, pero no habia forma de elegirlas
    /// en la unica superficie que las implementa. Estas tres pruebas fijan
    /// las decisiones puras de las que depende el arreglo -que boton cae
    /// bajo un punto, que herramienta elige una tecla, y que hace cada
    /// boton- para no depender de una ventana de verdad.
    #[test]
    fn las_tres_herramientas_de_medir_tienen_boton_en_la_caja_del_editor() {
        let c = CajaHerramientas::colocar(rect_de_prueba(), rect_de_prueba(), 100, &BOTONES_EDITOR);
        for h in [
            Herramienta::Cota,
            Herramienta::Escalar,
            Herramienta::EscalaGrafica,
        ] {
            let indice = BOTONES_EDITOR
                .iter()
                .position(|b| *b == BotonCaja::Elegir(h))
                .unwrap_or_else(|| panic!("{h:?} no esta en la caja del editor"));
            let r = c.rect_de(indice);
            let centro = Punto {
                x: r.x + r.ancho as i32 / 2,
                y: r.y + r.alto as i32 / 2,
            };
            assert_eq!(c.boton_en(centro), Some(BotonCaja::Elegir(h)));
        }
    }

    #[test]
    fn las_letras_de_medir_eligen_la_herramienta_de_medir() {
        assert_eq!(tecla_a_herramienta('A'), Some(Herramienta::Cota));
        assert_eq!(tecla_a_herramienta('E'), Some(Herramienta::Escalar));
        assert_eq!(tecla_a_herramienta('G'), Some(Herramienta::EscalaGrafica));
        // Sin distincion de mayusculas: es lo que espera cualquiera que
        // teclee sin fijarse en Bloq Mayus.
        assert_eq!(tecla_a_herramienta('a'), Some(Herramienta::Cota));
    }

    #[test]
    fn una_letra_sin_herramienta_asignada_no_elige_nada() {
        // Caso negativo: si esto devolviera Some para cualquier caracter,
        // escribir en el cajetin de calibrar (que usa su PROPIO bucle de
        // eventos, no este) tampoco estaria a salvo si algun dia compartiera
        // codigo con esta funcion.
        assert_eq!(tecla_a_herramienta('9'), None);
        assert_eq!(tecla_a_herramienta(' '), None);
        // La «Z» es de la zona desde F8; la «X» sigue libre.
        assert_eq!(tecla_a_herramienta('X'), None);
    }

    #[test]
    fn pulsar_un_boton_de_elegir_cambia_la_herramienta_del_gesto() {
        let mut gesto = Gesto::nuevo();
        let mut escena = Escena::nueva();
        assert_eq!(gesto.herramienta, Herramienta::Lapiz);
        let sigue = pulsar_boton(
            BotonCaja::Elegir(Herramienta::Cota),
            &mut gesto,
            &mut escena,
        );
        assert!(sigue, "elegir una herramienta no pide salir");
        assert_eq!(gesto.herramienta, Herramienta::Cota);
    }

    #[test]
    fn pulsar_deshacer_y_rehacer_llama_a_la_escena() {
        // Rotura a proposito: si `BotonCaja::Deshacer` no llamara a
        // `escena.deshacer()`, el trazo seguiria vivo tras pulsarlo. Esta
        // prueba lo caza comprobando la escena de verdad, no solo el valor
        // devuelto.
        let mut gesto = Gesto::nuevo();
        let mut escena = Escena::nueva();
        gesto.herramienta = Herramienta::Lapiz;
        gesto.evento(
            EventoGesto::Pulsar {
                p: Punto2::nuevo(0.0, 0.0),
                shift: false,
                alt: false,
                presion: None,
            },
            &mut escena,
            1.0,
        );
        gesto.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(10.0, 0.0),
            },
            &mut escena,
            1.0,
        );
        assert_eq!(escena.cuantos_visibles(), 1);

        assert!(pulsar_boton(BotonCaja::Deshacer, &mut gesto, &mut escena));
        assert_eq!(escena.cuantos_visibles(), 0, "Deshacer tiene que deshacer");

        assert!(pulsar_boton(BotonCaja::Rehacer, &mut gesto, &mut escena));
        assert_eq!(escena.cuantos_visibles(), 1, "Rehacer tiene que rehacer");
    }

    #[test]
    fn escape_cierra_el_lienzo_en_reposo_y_no_a_medias_ni_fuera_del_lienzo() {
        // En ventana, Escape cierra.
        assert_eq!(al_escape(true, true, true, false), AlEscape::Cerrar);
        // A mitad de un arrastre, Escape lo cancela y el lienzo sigue.
        assert_eq!(al_escape(true, true, false, false), AlEscape::Seguir);
        // Presentando, Escape solo deja de presentar.
        assert_eq!(al_escape(true, false, true, false), AlEscape::Seguir);
        // El anotador de pantalla tiene su propia regla.
        assert_eq!(al_escape(false, true, true, false), AlEscape::Seguir);
        // Recien abierto en ventana, el gesto esta en reposo: una basta.
        assert_eq!(
            al_escape(
                true,
                true,
                gesto_inicial(Default::default()).en_reposo(),
                false
            ),
            AlEscape::Cerrar
        );
    }

    #[test]
    fn escape_a_pantalla_completa_pasa_a_ventana_y_el_segundo_cierra() {
        // La secuencia del usuario: a pantalla completa, el primer Escape
        // pasa a ventana (lo hace `alternar_modo`, que la deja en ventana)
        // y el segundo cierra.
        let mut pantalla_completa = true;
        let mut pasos = Vec::new();
        for _ in 0..2 {
            let a = al_escape(true, true, true, pantalla_completa);
            if a == AlEscape::AVentana {
                pantalla_completa = false;
            }
            pasos.push(a);
        }
        assert_eq!(pasos, [AlEscape::AVentana, AlEscape::Cerrar]);
        // Caso negativo: con la herramienta a medias (un rectangulo
        // arrastrandose) Escape no cierra ni cambia de modo, a pantalla
        // completa ni en ventana: lo cancela el motor.
        let mut gesto = gesto_inicial(Default::default());
        let mut escena = Escena::nueva();
        crate::dibujo::teclas::elegir_herramienta(&mut gesto, Herramienta::Rectangulo);
        let p = |x, y| pixpin_motor2d::vector::Punto2::nuevo(x, y);
        gesto.evento(
            EventoGesto::Pulsar {
                p: p(10.0, 10.0),
                shift: false,
                alt: false,
                presion: None,
            },
            &mut escena,
            1.0,
        );
        gesto.evento(
            EventoGesto::Mover {
                p: p(80.0, 60.0),
                shift: false,
                alt: false,
                presion: None,
            },
            &mut escena,
            1.0,
        );
        assert!(!gesto.en_reposo(), "arrastrando un rectangulo");
        assert_eq!(
            al_escape(true, true, gesto.en_reposo(), true),
            AlEscape::Seguir
        );
        assert_eq!(
            al_escape(true, true, gesto.en_reposo(), false),
            AlEscape::Seguir
        );
        // Cancelado por el motor, ya en reposo: el siguiente Escape vale.
        gesto.evento(EventoGesto::Escape, &mut escena, 1.0);
        assert!(gesto.en_reposo());
        assert_eq!(
            al_escape(true, true, gesto.en_reposo(), true),
            AlEscape::AVentana
        );
        // Presentando a pantalla completa, Escape deja de presentar y nada mas.
        assert_eq!(al_escape(true, false, true, true), AlEscape::Seguir);
        // El anotador de pantalla ocupa la pantalla, pero no es un lienzo en
        // ventana: Escape sigue su regla.
        assert_eq!(al_escape(false, true, true, true), AlEscape::Seguir);
    }

    #[test]
    fn pulsar_salir_pide_salir_y_los_demas_no() {
        let mut gesto = Gesto::nuevo();
        let mut escena = Escena::nueva();
        assert!(!pulsar_boton(BotonCaja::Salir, &mut gesto, &mut escena));
        assert!(pulsar_boton(BotonCaja::Color, &mut gesto, &mut escena));
        assert!(pulsar_boton(BotonCaja::Deshacer, &mut gesto, &mut escena));
    }

    fn rect_de_prueba() -> pixpin_geom::Rect {
        pixpin_geom::Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1080,
        }
    }

    /// El fallo real: los tiradores se pintaban con el angulo fijo en cero
    /// (`Tiradores::de_caja(caja, 0.0, escala)`) mientras `gesto.rs` los
    /// agarraba con el angulo de verdad del elemento. El usuario veia el
    /// marco recto y pinchaba donde veia el tirador, pero la zona que
    /// respondia estaba girada: no agarraba nada.
    ///
    /// El arreglo hace que pintar llame a `Gesto::tiradores` -la MISMA
    /// funcion que ya usaba `pulsar` para decidir que agarra el clic- en
    /// vez de recalcular el angulo por su cuenta. Esta prueba comprueba
    /// justo eso: el sitio que `Gesto::tiradores` dice que hay que PINTAR
    /// es el mismo que agarra un pulsar ahi.
    #[test]
    fn el_tirador_pintado_de_un_elemento_girado_es_el_que_agarra_el_clic() {
        let mut escena = Escena::nueva();
        let id = escena.anadir(Elemento {
            id: 0,
            figura: Figura::Rectangulo,
            estilo_relleno: Default::default(),
            x: 0.0,
            y: 0.0,
            ancho: 100.0,
            alto: 50.0,
            angulo: FRAC_PI_2,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            relleno: None,
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
            bloqueado: false,
            enlace: None,
            redondo: false,
            material: Default::default(),
            extras: Default::default(),
        });

        let mut gesto = Gesto::nuevo();
        gesto.seleccion.poner(id);

        let escala = 1.0;
        let pintados = gesto
            .tiradores(&escena, escala)
            .expect("hay un elemento elegido: tiene que haber tiradores");
        assert_ne!(
            pintados.angulo, 0.0,
            "con un solo elemento el marco pintado lleva su angulo real"
        );

        // El sitio exacto de un tirador, tal y como se pinta.
        let (cual, punto) = pintados.tamano[0];

        let respuesta = gesto.evento(
            EventoGesto::Pulsar {
                p: punto,
                shift: false,
                alt: false,
                presion: None,
            },
            &mut escena,
            escala,
        );

        assert_eq!(
            respuesta.cursor,
            FormaCursor::Escalar {
                tirador: cual,
                angulo: pintados.angulo,
            },
            "pulsar justo donde se pinto el tirador tiene que agarrarlo"
        );
    }

    #[test]
    fn el_cursor_de_escalar_elige_la_flecha_por_su_direccion() {
        // Windows solo tiene cuatro flechas de redimension. La direccion
        // del tirador, ya girada con el elemento, se reparte entre ellas.
        let se_recto = FormaCursor::Escalar {
            tirador: Tirador::SuresteEsquina,
            angulo: 0.0,
        };
        assert_eq!(forma_de(se_recto), FormaCursorWin::RedimNoSe);

        // El mismo tirador con el elemento girado un cuarto de vuelta
        // apunta a la otra diagonal.
        let se_girado = FormaCursor::Escalar {
            tirador: Tirador::SuresteEsquina,
            angulo: FRAC_PI_2,
        };
        assert_eq!(forma_de(se_girado), FormaCursorWin::RedimNeSo);
    }

    #[test]
    fn el_tirador_del_norte_es_la_flecha_vertical() {
        let n = FormaCursor::Escalar {
            tirador: Tirador::NorteBorde,
            angulo: 0.0,
        };
        assert_eq!(forma_de(n), FormaCursorWin::RedimNS);

        // Girado noventa grados, el borde norte apunta al este.
        let n = FormaCursor::Escalar {
            tirador: Tirador::NorteBorde,
            angulo: FRAC_PI_2,
        };
        assert_eq!(forma_de(n), FormaCursorWin::RedimEO);
    }

    #[test]
    fn media_vuelta_da_la_misma_flecha() {
        // Una flecha de redimension no tiene punta: norte y sur son la
        // misma. Sin esto, girar 180 grados cambiaria el cursor sin motivo.
        let n = FormaCursor::Escalar {
            tirador: Tirador::NorteBorde,
            angulo: 0.0,
        };
        let s = FormaCursor::Escalar {
            tirador: Tirador::SurBorde,
            angulo: 0.0,
        };
        assert_eq!(forma_de(n), forma_de(s));

        let girado = FormaCursor::Escalar {
            tirador: Tirador::NorteBorde,
            angulo: PI,
        };
        assert_eq!(forma_de(n), forma_de(girado));
    }

    #[test]
    fn los_demas_cursores_se_traducen_uno_a_uno() {
        assert_eq!(forma_de(FormaCursor::Flecha), FormaCursorWin::Flecha);
        assert_eq!(forma_de(FormaCursor::Cruz), FormaCursorWin::Cruz);
        assert_eq!(forma_de(FormaCursor::Mover), FormaCursorWin::Mover);
        assert_eq!(forma_de(FormaCursor::Texto), FormaCursorWin::Texto);
        assert_eq!(forma_de(FormaCursor::Giro), FormaCursorWin::Giro);
    }

    #[test]
    fn el_raton_llega_al_motor_en_coordenadas_del_mundo() {
        // El motor trabaja en el mundo; la ventana recibe pixeles. Si esta
        // traduccion se olvidara, dibujar con el lienzo desplazado pintaria
        // en otro sitio.
        //
        // El metodo es `a_mundo`, que convierte un PUNTO. Cuidado con
        // `en_mundo`, que existe y hace otra cosa: convierte una LONGITUD
        // en pixeles a longitud del mundo. Confundirlos compila y da un
        // resultado silenciosamente equivocado.
        let camara = Camara {
            x: 100.0,
            y: 50.0,
            zoom: 2.0,
        };
        let ev = EventoOverlay::BotonPulsado(Punto { x: 20, y: 10 });

        let Some(EventoGesto::Pulsar { p, .. }) = a_evento(&ev, &camara, Punto { x: 0, y: 0 })
        else {
            panic!("un boton pulsado es un Pulsar");
        };
        assert_eq!(p, camara.a_mundo(Punto2::nuevo(20.0, 10.0)));
    }

    #[test]
    fn a_escala_150_el_raton_cae_en_el_mundo_logico() {
        // D127: un clic 150 px fisicos a la derecha del origen es 100 px
        // logicos, que es donde Excalidraw pondria el punto.
        let efectiva = crate::navegacion::vista_efectiva(&Camara::nueva(), 150);
        let ev = EventoOverlay::BotonPulsado(Punto { x: 150, y: 75 });
        let Some(EventoGesto::Pulsar { p, .. }) = a_evento(&ev, &efectiva, Punto { x: 0, y: 0 })
        else {
            panic!("un boton pulsado es un Pulsar");
        };
        assert_eq!((p.x, p.y), (100.0, 50.0));
    }

    #[test]
    fn el_origen_de_la_ventana_se_resta_antes_de_pasar_al_mundo() {
        // Con la barra de tareas arriba o a la izquierda, el area de trabajo no
        // empieza en (0,0) y la tinta salia desplazada del cursor.
        let camara = Camara::nueva();
        let ev = EventoOverlay::BotonPulsado(Punto { x: 110, y: 60 });
        let Some(EventoGesto::Pulsar { p, presion, .. }) =
            a_evento(&ev, &camara, Punto { x: 100, y: 50 })
        else {
            panic!("tendria que ser Pulsar")
        };
        assert_eq!((p.x, p.y), (10.0, 10.0));
        assert_eq!(presion, None);
    }

    #[test]
    fn una_muestra_del_lapiz_llega_al_gesto_con_su_presion_y_su_subpixel() {
        let camara = Camara::nueva();
        let m = pixpin_shell::puntero::Muestra::nueva(12.5, 7.25, Some(0.75));
        let Some(EventoGesto::Mover { p, presion, .. }) =
            a_evento(&EventoOverlay::Muestra(m), &camara, Punto { x: 0, y: 0 })
        else {
            panic!("tendria que ser Mover")
        };
        assert_eq!((p.x, p.y), (12.5, 7.25));
        assert_eq!(presion, Some(0.75));
    }

    #[test]
    fn las_teclas_uno_dos_tres_eligen_grosor_y_v_alterna_la_pluma() {
        assert_eq!(
            tecla_a_pluma('1'),
            Some(CambioPluma::Grosor(pixpin_motor2d::tinta::GROSOR_FINO))
        );
        assert_eq!(
            tecla_a_pluma('2'),
            Some(CambioPluma::Grosor(pixpin_motor2d::tinta::GROSOR_MEDIO))
        );
        assert_eq!(
            tecla_a_pluma('3'),
            Some(CambioPluma::Grosor(pixpin_motor2d::tinta::GROSOR_GRUESO))
        );
        assert_eq!(tecla_a_pluma('v'), Some(CambioPluma::AlternarVariabilidad));
        assert_eq!(tecla_a_pluma('L'), None, "L sigue siendo el lapiz");
        // Ninguna de las nuevas pisa una herramienta.
        for c in ['1', '2', '3', 'V'] {
            assert_eq!(tecla_a_herramienta(c), None, "{c}");
        }
    }

    #[test]
    fn las_tres_tablas_de_teclas_no_se_pisan_en_ninguna_letra_pelada() {
        // **La prueba de la unica verdad.** Habia dos tablas de atajos que no
        // se conocian: `atajo_de` daba `Q` al lazo y `tecla_a_herramienta` se
        // la da a la lupa. Ahora el lazo es `S`, y esto es lo que impide que
        // la contradiccion vuelva por otra letra.
        //
        // Solo las peladas: las de `atajo_de` con Ctrl, Mayus o Alt no pueden
        // chocar con las de herramienta ni con las de pluma, que llegan por
        // `Caracter` sin modificadores.
        for c in 'a'..='z' {
            let del_motor =
                pixpin_motor2d::seleccion::atajo_de(Tecla::Letra(c), false, false, false);
            let Some(orden) = del_motor else { continue };
            let mayus = c.to_ascii_uppercase();
            assert_eq!(
                tecla_a_herramienta(mayus),
                None,
                "«{mayus}» es a la vez {orden:?} y una herramienta de la caja"
            );
            assert_eq!(
                tecla_a_pluma(mayus),
                None,
                "«{mayus}» es a la vez {orden:?} y un mando de la pluma"
            );
        }
        // Y el caso concreto que el revisor encontro, por su nombre.
        assert_eq!(tecla_a_herramienta('Q'), Some(Herramienta::Lupa));
        assert_eq!(
            pixpin_motor2d::seleccion::atajo_de(Tecla::Letra('q'), false, false, false),
            None,
            "`Q` es la lupa: el lazo no puede reclamarla"
        );
        assert_eq!(
            pixpin_motor2d::seleccion::atajo_de(Tecla::Letra('s'), false, false, false),
            Some(OrdenEditor::Lazo)
        );
    }

    #[test]
    fn la_tecla_de_windows_se_traduce_a_la_del_motor_y_lo_demas_no_estorba() {
        assert_eq!(tecla_del_motor(b'S' as u32), Some(Tecla::Letra('s')));
        assert_eq!(tecla_del_motor(0xDB), Some(Tecla::CorcheteAbre));
        assert_eq!(tecla_del_motor(0xDD), Some(Tecla::CorcheteCierra));
        // Caso negativo: lo que no esta en la tabla no puede colarse como
        // una letra cualquiera, o F11 seria un atajo del motor.
        assert_eq!(tecla_del_motor(0x7A), None, "F11 no es una letra");
        assert_eq!(tecla_del_motor(0x1B), None, "Escape no es una letra");
    }

    #[test]
    fn voltear_por_atajo_mueve_lo_elegido_y_se_deshace_de_una_vez() {
        // `transformar::voltear` estaba escrito, probado y sin un solo
        // llamante: el commit que lo trajo prometia «voltear es una orden» y
        // la orden no la ejecutaba nadie.
        let mut escena = Escena::nueva();
        let mut a = elemento_de_prueba();
        a.id = 0;
        a.x = 0.0;
        let a = escena.anadir(a);
        let mut b = elemento_de_prueba();
        b.id = 0;
        b.x = 100.0;
        let b = escena.anadir(b);

        let mut gesto = Gesto::nuevo();
        gesto.seleccion.poner_todos([a, b]);
        assert!(aplicar_orden(
            OrdenEditor::VoltearHorizontal,
            &mut gesto,
            &mut escena
        ));
        // Se intercambian alrededor de la caja comun.
        assert_eq!(escena.buscar(a).unwrap().x, 100.0);
        assert_eq!(escena.buscar(b).unwrap().x, 0.0);
        // Y es UN paso de deshacer, no dos.
        assert!(escena.deshacer());
        assert_eq!(escena.buscar(a).unwrap().x, 0.0);
        assert_eq!(escena.buscar(b).unwrap().x, 100.0);

        // Caso negativo: sin nada elegido no se toca la escena ni se ensucia
        // el historial.
        gesto.seleccion.limpiar();
        assert!(!aplicar_orden(
            OrdenEditor::VoltearHorizontal,
            &mut gesto,
            &mut escena
        ));
        assert_eq!(escena.buscar(a).unwrap().x, 0.0);
    }

    #[test]
    fn el_estilo_se_toma_de_uno_y_se_pega_en_otro_por_atajo() {
        // `estilo::{copiar, pegar_a}` tenian cero referencias fuera de su
        // fichero, y el boton del cuentagotas ya estaba en la caja.
        let mut escena = Escena::nueva();
        let mut modelo = elemento_de_prueba();
        modelo.id = 0;
        modelo.trazo = ColorRgba::opaco(0.9, 0.1, 0.1);
        let modelo = escena.anadir(modelo);
        let mut otro = elemento_de_prueba();
        otro.id = 0;
        let otro = escena.anadir(otro);

        let mut gesto = Gesto::nuevo();
        gesto.seleccion.poner(modelo);
        assert!(aplicar_orden(
            OrdenEditor::TomarEstilo,
            &mut gesto,
            &mut escena
        ));
        assert!(gesto.estilo_tomado.is_some());

        gesto.seleccion.poner(otro);
        assert!(aplicar_orden(
            OrdenEditor::SoltarEstilo,
            &mut gesto,
            &mut escena
        ));
        assert_eq!(
            escena.buscar(otro).unwrap().trazo,
            ColorRgba::opaco(0.9, 0.1, 0.1)
        );

        // Caso negativo: soltar sin haber tomado nada no hace nada.
        let mut limpio = Gesto::nuevo();
        limpio.seleccion.poner(otro);
        assert!(!aplicar_orden(
            OrdenEditor::SoltarEstilo,
            &mut limpio,
            &mut escena
        ));
    }

    #[test]
    fn los_atajos_que_no_piden_seleccion_funcionan_con_el_lienzo_vacio() {
        let mut escena = Escena::nueva();
        let mut gesto = Gesto::nuevo();
        assert!(gesto.seleccion.esta_vacia());

        assert!(aplicar_orden(OrdenEditor::Lazo, &mut gesto, &mut escena));
        assert_eq!(gesto.herramienta, Herramienta::Lazo);
        assert!(aplicar_orden(
            OrdenEditor::CopiarEstilo,
            &mut gesto,
            &mut escena
        ));
        assert_eq!(gesto.herramienta, Herramienta::CopiarEstilo);

        let antes = gesto.enganche.activo;
        assert!(aplicar_orden(
            OrdenEditor::AlternarIman,
            &mut gesto,
            &mut escena
        ));
        assert_ne!(gesto.enganche.activo, antes, "el iman no se apago");

        // Caso negativo: las que si piden seleccion se quedan quietas.
        for orden in [
            OrdenEditor::Agrupar,
            OrdenEditor::AlFrente,
            OrdenEditor::TomarEstilo,
        ] {
            assert!(
                !aplicar_orden(orden, &mut gesto, &mut escena),
                "{orden:?} hizo algo sin nada elegido"
            );
        }
    }

    #[test]
    fn cambiar_de_herramienta_tira_el_lazo_a_medio_trazar() {
        // Un rastro que deja de crecer y se sigue pintando es un rastro
        // mintiendo, y lo que se seleccionara al soltar ya no seria lo que
        // se ve.
        let mut gesto = Gesto::nuevo();
        gesto.herramienta = Herramienta::Lazo;
        gesto.lazo = Some(pixpin_motor2d::lazo::Lazo::empezar(Punto2::nuevo(0.0, 0.0)));
        elegir_herramienta(&mut gesto, Herramienta::Lapiz);
        assert!(gesto.lazo.is_none());
    }

    #[test]
    fn arrastrando_un_marco_lo_de_dentro_sale_de_la_capa_congelada_y_lo_de_fuera_no() {
        let mut escena = Escena::nueva();
        let caja = |figura: Figura, x: f32, y: f32, ancho: f32, alto: f32| Elemento {
            figura,
            x,
            y,
            ancho,
            alto,
            grosor: 2.0,
            ..Default::default()
        };
        let marco = escena.anadir(caja(
            Figura::Marco {
                nombre: String::new(),
            },
            0.0,
            0.0,
            400.0,
            300.0,
        ));
        let dentro = escena.anadir(caja(Figura::Rectangulo, 100.0, 100.0, 50.0, 50.0));
        let fuera = escena.anadir(caja(Figura::Rectangulo, 600.0, 100.0, 50.0, 50.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;
        g.seleccion.poner(marco);
        assert!(!se_arrastra(&g, dentro), "quieto no se arrastra nada");
        for (evento, p) in [
            (true, Punto2::nuevo(100.0, 0.0)),
            (false, Punto2::nuevo(130.0, 10.0)),
        ] {
            let e = if evento {
                EventoGesto::Pulsar {
                    p,
                    shift: false,
                    alt: false,
                    presion: None,
                }
            } else {
                EventoGesto::Mover {
                    p,
                    shift: false,
                    alt: false,
                    presion: None,
                }
            };
            g.evento(e, &mut escena, 1.0);
        }
        assert!(g.moviendo());
        assert!(es_excluido(&g, marco) && es_excluido(&g, dentro));
        assert!(
            !es_excluido(&g, fuera),
            "lo de fuera sigue en la capa congelada"
        );
        let mut ex = excluidos_de(&g);
        ex.sort_unstable();
        assert_eq!(ex, vec![marco, dentro]);
    }

    #[test]
    fn dibujando_se_excluye_el_trazo_y_moviendo_la_seleccion() {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Lapiz;
        assert!(excluidos_de(&g).is_empty());
        g.evento(
            EventoGesto::Pulsar {
                p: Punto2::nuevo(1.0, 1.0),
                shift: false,
                alt: false,
                presion: None,
            },
            &mut escena,
            1.0,
        );
        assert_eq!(excluidos_de(&g), vec![g.trazo_en_curso().unwrap()]);
    }

    #[test]
    fn control_zeta_es_deshacer_y_control_i_griega_rehacer() {
        let camara = Camara::nueva();
        let ctrl = |vk: u32| EventoOverlay::Tecla {
            vk,
            shift: false,
            ctrl: true,
            alt: false,
        };

        assert_eq!(
            a_evento(&ctrl(b'Z' as u32), &camara, Punto { x: 0, y: 0 }),
            Some(EventoGesto::Deshacer)
        );
        assert_eq!(
            a_evento(&ctrl(b'Y' as u32), &camara, Punto { x: 0, y: 0 }),
            Some(EventoGesto::Rehacer)
        );
        assert_eq!(
            a_evento(&ctrl(b'A' as u32), &camara, Punto { x: 0, y: 0 }),
            Some(EventoGesto::SeleccionarTodo)
        );
    }

    #[test]
    fn la_zeta_sin_control_no_deshace() {
        // Escribir una zeta en un texto no puede deshacer el dibujo.
        let camara = Camara::nueva();
        let sola = EventoOverlay::Tecla {
            vk: b'Z' as u32,
            shift: false,
            ctrl: false,
            alt: false,
        };
        assert_ne!(
            a_evento(&sola, &camara, Punto { x: 0, y: 0 }),
            Some(EventoGesto::Deshacer)
        );
    }

    #[test]
    fn lo_que_no_le_toca_al_motor_no_llega_al_motor() {
        let camara = Camara::nueva();
        assert_eq!(
            a_evento(&EventoOverlay::Pintar, &camara, Punto { x: 0, y: 0 }),
            None
        );
        assert_eq!(
            a_evento(&EventoOverlay::CambioDpi, &camara, Punto { x: 0, y: 0 }),
            None
        );
    }

    #[test]
    #[ignore = "necesita sesion de escritorio con GPU; ejecutar con --ignored"]
    fn abrir_y_cerrar_el_editor_no_revienta() {
        // El bucle de la ventana y el pintado necesitan una sesion
        // interactiva real (crear la ventana, el dispositivo D3D11, la
        // superficie de composicion). No se puede probar en CI sin
        // escritorio; se deja marcada para ejecutarla a mano.
        let _: Result<(Escena, Option<String>)> = abrir(
            Escena::nueva(),
            pixpin_motor2d::enganche::Ajustes::default(),
            pixpin_nivel::Nivel::Completo,
            false,
            None,
            &[],
        );
    }

    #[test]
    fn el_fondo_no_es_un_elemento_que_se_pueda_seleccionar() {
        // D133: la imagen vive fuera de la escena. Con un fondo abierto y
        // nada dibujado, Ctrl+A no elige nada y el borrador no tiene que
        // borrar.
        let mut escena = Escena::nueva();
        let mut gesto = Gesto::nuevo();
        let _fondo = crate::fondo_lienzo::FondoLienzo::nuevo(
            crate::fondo_lienzo::recuadro_gris(800, 600),
            4096,
        );
        gesto.evento(EventoGesto::SeleccionarTodo, &mut escena, 1.0);
        assert!(gesto.seleccion.ids().is_empty());
        assert_eq!(escena.cuantos_visibles(), 0);
    }

    #[test]
    fn una_medida_tecleada_se_lee_con_coma_o_con_punto() {
        // El usuario escribe «3,5» o «3.5» segun la costumbre y el teclado.
        assert!((leer_medida("3,5").unwrap() - 3.5).abs() < 1e-5);
        assert!((leer_medida("3.5").unwrap() - 3.5).abs() < 1e-5);
        assert!((leer_medida(" 12 ").unwrap() - 12.0).abs() < 1e-5);
    }

    #[test]
    fn una_medida_que_no_es_un_numero_se_rechaza() {
        assert!(leer_medida("").is_none());
        assert!(leer_medida("tres").is_none());
        assert!(leer_medida("-3").is_none(), "una medida negativa no existe");
        assert!(leer_medida("0").is_none());
    }

    #[test]
    #[ignore = "necesita sesion de escritorio con GPU; ejecutar con --ignored"]
    fn pedir_medida_devuelve_none_al_cancelar_con_escape() {
        // El bucle de `pedir_medida` abre su propia ventana de overlay y
        // necesita un motor D3D11 de verdad: no se puede probar en CI sin
        // escritorio, se deja marcada para ejecutarla a mano.
        //
        // Si al cancelar la escala cambiara, seria un fallo (D39): cancelar
        // tiene que dejar el lienzo exactamente como estaba. A mano: abrir
        // el editor, calibrar con la herramienta Escalar, pulsar Escape en
        // el cajetin y comprobar que `escena.escala` sigue siendo la de
        // antes de arrastrar.
        let dispositivo =
            pixpin_capture::Dispositivo::nuevo().expect("sin dispositivo para la prueba manual");
        let motor = MotorRender::nuevo(dispositivo.d3d()).expect("sin motor para la prueba manual");
        let _ = motor;
    }

    #[test]
    fn por_cada_orden_incluye_el_rotulo_de_una_cota() {
        // El fallo real (ronda 1 de la tarea 6): el cierre de
        // `capa.preparar` horneaba solo `cache.ordenes(e, ...)`, sin las
        // `ordenes_medibles`. Como `pintar` salta con `continue` todo lo que
        // la capa ya volco, el numero de CUALQUIER cota visible en pantalla
        // desaparecia mientras se arrastraba OTRO elemento cualquiera -no
        // hacia falta tocar la cota, bastaba con que la capa se horneara-.
        //
        // El arreglo es que los dos caminos llamen a `por_cada_orden` en vez
        // de a `cache.ordenes` a secas. Esta prueba fija lo que esa funcion
        // compartida tiene que entregar para una cota: no puede probar la
        // ventana de verdad (necesita GPU y sesion interactiva), pero si
        // puede probar que la funcion de la que dependen los dos caminos no
        // vuelve a "olvidarse" del rotulo. Por eso el sumidero es un cierre
        // y no el `Pintor`: pasarle el pintor habria matado esta prueba.
        let cota = Elemento {
            estilo_relleno: Default::default(),
            figura: Figura::Cota {
                puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0)],
            },
            id: 1,
            x: 0.0,
            y: 0.0,
            ancho: 100.0,
            alto: 0.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            relleno: None,
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
            bloqueado: false,
            enlace: None,
            redondo: false,
            material: Default::default(),
            extras: Default::default(),
        };
        let mut cache = Cache::nueva();

        let mut hay_rotulo = false;
        let mut cuantas = 0usize;
        por_cada_orden(&mut cache, &cota, 1.0, None, |o| {
            cuantas += 1;
            // Desde que la cota se pinta como en el movil, su numero va girado
            // con la raya y con halo: un `Rotulo`, no un `Texto`.
            if matches!(o, Orden::Texto { .. } | Orden::Rotulo { .. }) {
                hay_rotulo = true;
            }
        });

        assert!(
            hay_rotulo,
            "una cota tiene que traer su rotulo, venga o no de la cache"
        );
        // Que ademas entregue la forma cacheada: si algun dia el primer bucle
        // desapareciera, el rotulo solo seguiria estando y la asercion de
        // arriba no se enteraria de que la cota se pinta sin su raya.
        assert!(
            cuantas > 1,
            "faltan las ordenes cacheadas de la propia cota, no solo el rotulo"
        );
    }

    #[test]
    fn la_pista_del_iman_es_una_orden_que_el_editor_sabe_pintar() {
        // No se puede probar la ventana de verdad -pide GPU y sesion
        // interactiva-, pero si que lo que el motor produce para el ancla
        // activa es de un tipo que `dibujar_orden` cubre. Si algun dia la
        // pista pasara a ser una variante que el `match` de `dibujar_orden`
        // no contemple, la marca desapareceria en silencio.
        use pixpin_motor2d::enganche::{Anclaje, TipoAnclaje, pista};

        for tipo in [
            TipoAnclaje::Esquina,
            TipoAnclaje::Extremo,
            TipoAnclaje::Medio,
            TipoAnclaje::Centro,
        ] {
            let a = Anclaje {
                punto: Punto2::nuevo(10.0, 20.0),
                tipo,
                id: 1,
            };
            assert!(
                matches!(pista(&a, 1.0), Orden::Polilinea { .. }),
                "la pista de {tipo:?} no es pintable por el editor"
            );
        }
    }

    /// El cableado nuevo: `abrir` tiene que trasladar los ajustes de iman
    /// guardados al `Gesto` que crea, no dejarlos siempre por defecto. Sin
    /// esto la pestana de ajustes seria decorativa -el usuario apaga el
    /// iman y el editor lo ignora-.
    ///
    /// Se ejercita `gesto_inicial`, que es lo que `abrir` llama de verdad:
    /// construir un `Gesto` aqui y releerle el campo solo comprobaria que
    /// asignar campos en Rust funciona.
    #[test]
    fn los_ajustes_de_iman_llegan_al_gesto_recien_creado() {
        let apagado = pixpin_motor2d::enganche::Ajustes {
            activo: false,
            medios: false,
            radio_px: 3.0,
            ..Default::default()
        };

        let gesto = gesto_inicial(apagado);

        assert_eq!(
            gesto.enganche, apagado,
            "el gesto tiene que llevar los ajustes que se le pasan, no los por defecto"
        );
        assert_ne!(
            gesto.enganche,
            pixpin_motor2d::enganche::Ajustes::default(),
            "los ajustes de prueba tienen que diferir de los de fabrica o esto no mide nada"
        );
    }
}
