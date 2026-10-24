//! Leer y escribir el JSON de Excalidraw, que es el formato en el que el
//! PixPin de Android guarda sus lienzos.
//!
//! Es el puente entre las dos mitades: capturas en el movil y sigues en el
//! escritorio. El `.pixpin` del Android es un ZIP con un `.excalidraw` por
//! hoja, en JSON plano — su propia documentacion dice que lo descomprime a
//! proposito «para que un editor de escritorio los lea sin mas».
//!
//! # Lo que NO se puede perder
//!
//! El Android tiene treinta y una herramientas y nosotros once. Un fichero
//! suyo trae cotas, mosaicos, solidos y cronogramas que aqui no sabemos ni
//! dibujar. **Esos elementos se guardan tal cual y vuelven a salir tal
//! cual**, en su sitio dentro del orden.
//!
//! Sin eso, abrir un plano en el escritorio y volver a guardarlo le borraria
//! las cotas al usuario. Un puente que pierde la mitad de la carga es peor
//! que no tener puente: al menos sin el, nadie confia en el.
//!
//! # Las tres diferencias de fondo
//!
//! - **Los puntos.** Los nuestros son ABSOLUTOS; los de Excalidraw van
//!   relativos al origen del elemento. Se suman al leer y se restan al
//!   escribir.
//! - **Los colores.** Los nuestros son cuatro numeros de cero a uno; los
//!   suyos, texto hexadecimal, con `"transparent"` como valor especial.
//! - **Los identificadores.** Los nuestros son numeros; los suyos, texto. El
//!   texto original se guarda para devolverlo intacto: cambiarselo romperia
//!   las ataduras entre flechas y figuras dentro del propio fichero.

use std::collections::HashMap;

use serde_json::{Map, Value};

use crate::elemento::{
    Atado, ColorRgba, Elemento, Enganche, EstiloTrazo, Extras, Figura, ModoEnganche, PautaHoja,
    TamanoPapel,
};
use crate::formas::TipoPunta;
use crate::medida::Escala;
use crate::relleno::EstiloRelleno;
use crate::tinta::MaterialTinta;
use crate::vector::Punto2;

#[derive(Debug, thiserror::Error)]
pub enum ErrorExcalidraw {
    #[error("el fichero no es JSON valido: {0}")]
    Json(#[from] serde_json::Error),
    #[error("el JSON no es un lienzo de Excalidraw: falta «elements» o no es una lista")]
    SinElementos,
}

/// Una entrada del lienzo, en su sitio dentro del orden de pintado.
///
/// El orden importa: en Excalidraw lo que va despues tapa a lo que va antes.
/// Separar lo conocido de lo ajeno en dos listas perderia ese entrelazado y
/// un mosaico dejaria de tapar lo que tapaba.
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum Entrada {
    /// Un elemento que sabemos representar. Se guarda tambien su JSON
    /// original para devolver intactos los campos que no usamos.
    Nuestro {
        elemento: Elemento,
        original: Box<Value>,
    },
    /// Un elemento que no sabemos representar: viaja tal cual.
    Ajeno(Box<Value>),
}

/// Un lienzo leido.
#[derive(Debug, Clone)]
pub struct Lienzo {
    pub entradas: Vec<Entrada>,
    /// Todo lo demas del fichero — `type`, `version`, `appState`, `files` —
    /// tal cual venia. Se devuelve sin tocar al escribir.
    pub resto: Map<String, Value>,
    /// Que mide un pixel de este lienzo (D31). Vive al nivel del lienzo, no
    /// en `resto`, porque `leer`/`escribir` la traducen a su propio tipo en
    /// vez de dejarla como JSON crudo.
    pub escala: Option<Escala>,
    /// La clave `escala` cruda del JSON, tal como venia (o `None` si no venia).
    ///
    /// Sirve para dos cosas. Si no supimos entenderla, se devuelve tal cual.
    /// Y si la entendimos y nadie la cambio, tambien: `Escala` guarda el
    /// numero en `f32`, y reescribirlo convertia los 0,13647374510765076 del
    /// movil en 0,1364737451076507568...; la ida y vuelta
    /// tiene que devolver lo mismo que entro.
    ///
    ///
    /// `escala` es `None` tanto si no habia ninguna como si la habia y era
    /// invalida — eso esta bien para medir, pero no para escribir: «no la
    /// entiendo» y «no la hay» no pueden representarse igual, o una escala
    /// corrupta de una version futura del movil se destruye en silencio al
    /// pasar por Windows. Privado: solo lo usan `leer` y `escribir`.
    escala_cruda: Option<Value>,
}

impl Lienzo {
    /// Un lienzo recien creado, sin nada dentro.
    ///
    /// Hace falta porque `escala_cruda` es privado y desde fuera de
    /// este modulo no se puede escribir un `Lienzo` entero a mano. Lo demas
    /// —`type`, `version`, `appState`— lo pone `escribir`.
    pub fn vacio() -> Lienzo {
        Lienzo {
            entradas: Vec::new(),
            resto: Map::new(),
            escala: None,
            escala_cruda: None,
        }
    }

    /// Los elementos que sabemos dibujar, en orden.
    pub fn elementos(&self) -> Vec<Elemento> {
        self.entradas
            .iter()
            .filter_map(|e| match e {
                Entrada::Nuestro { elemento, .. } => Some(elemento.clone()),
                Entrada::Ajeno(_) => None,
            })
            .collect()
    }

    /// Cuantos elementos no supimos representar.
    ///
    /// Es el numero que hay que ensenarle al usuario: «este plano trae siete
    /// cosas que aqui no se pueden editar, pero no se van a perder».
    pub fn cuantos_ajenos(&self) -> usize {
        self.entradas
            .iter()
            .filter(|e| matches!(e, Entrada::Ajeno(_)))
            .count()
    }
}

/// **Unos elementos sueltos como elementos de Excalidraw**, escritos por el
/// mismo camino que un lienzo entero (`con_escena` + `elemento_hacia`): la
/// biblioteca de figuras en el formato del movil y lo que dibuja el
/// asistente con piezas del motor (grafica, ecuacion, instrumento) salen
/// asi, sin un segundo escritor.
pub fn elementos_a_json(elementos: &[Elemento]) -> Vec<Value> {
    let mut escena = crate::Escena::nueva();
    for e in elementos.iter().filter(|e| !e.borrado) {
        escena.anadir(e.clone());
    }
    con_escena(&Lienzo::vacio(), &escena)
        .entradas
        .iter()
        .filter_map(|en| match en {
            Entrada::Nuestro { elemento, original } => {
                Some(elemento_hacia(elemento, original, false))
            }
            Entrada::Ajeno(_) => None,
        })
        .collect()
}

/// Y al reves: los que se sepan leer de una lista de elementos de
/// Excalidraw (los demas se saltan).
pub fn elementos_de_json(lista: &[Value]) -> Vec<Elemento> {
    lista.iter().filter_map(elemento_desde).collect()
}

/// Lee un lienzo.
pub fn leer(json: &str) -> Result<Lienzo, ErrorExcalidraw> {
    let raiz: Value = serde_json::from_str(json)?;
    let Value::Object(mut mapa) = raiz else {
        return Err(ErrorExcalidraw::SinElementos);
    };
    let Some(Value::Array(lista)) = mapa.remove("elements") else {
        return Err(ErrorExcalidraw::SinElementos);
    };
    let entradas = lista
        .into_iter()
        .map(|v| match elemento_desde(&v) {
            Some(elemento) => Entrada::Nuestro {
                elemento,
                original: Box::new(v),
            },
            None => Entrada::Ajeno(Box::new(v)),
        })
        .collect();
    let escala_cruda = mapa.remove("escala");
    let escala = escala_cruda.as_ref().and_then(escala_desde);
    // Se guarda cruda para devolverla intacta al escribir (ver `escala_cruda`).
    Ok(Lienzo {
        entradas,
        resto: mapa,
        escala,
        escala_cruda,
    })
}

/// Escribe un lienzo, devolviendo lo ajeno intacto y en su sitio.
pub fn escribir(lienzo: &Lienzo) -> String {
    let objetos = puntos_como_objetos(lienzo);
    let elementos: Vec<Value> = lienzo
        .entradas
        .iter()
        .map(|e| match e {
            Entrada::Nuestro { elemento, original } => {
                // Lo que nadie toco sale TAL CUAL entro. Pasarlo por
                // `elemento_hacia` lo reescribia entero -los puntos en otra
                // forma, los numeros con otros decimales- y un dibujo de mil
                // trazos del movil volvia cambiado por haberlo mirado.
                if elemento_desde(original).as_ref() == Some(elemento) {
                    (**original).clone()
                } else {
                    elemento_hacia(elemento, original, objetos)
                }
            }
            Entrada::Ajeno(v) => (**v).clone(),
        })
        .collect();
    let mut mapa = lienzo.resto.clone();
    mapa.insert("elements".into(), Value::Array(elementos));
    // Un lienzo que salga de aqui tiene que declararse como lo que es,
    // aunque el que entro no lo hiciera.
    mapa.entry("type")
        .or_insert_with(|| Value::String("excalidraw".into()));
    // Se escribe siempre, tambien cuando es None: si solo se escribiera
    // cuando hay escala, la del original sobreviviria y el movil seguiria
    // midiendo con una escala que aqui se borro. Pero None no siempre
    // significa «no hay»: si habia una clave y no se entendio, se devuelve
    // tal cual en vez de null, para no destruir en silencio una escala
    // corrupta o de una version futura del movil.
    let cruda_entendida = lienzo.escala_cruda.as_ref().map(escala_desde);
    let valor = match (&lienzo.escala, &lienzo.escala_cruda, cruda_entendida) {
        // La misma que entro: los mismos bytes.
        (Some(e), Some(cruda), Some(Some(leida))) if leida == *e => cruda.clone(),
        (Some(e), _, _) => escala_hacia(e),
        // Una que no se entendio: intacta.
        (None, Some(cruda), Some(None)) => cruda.clone(),
        // No habia, o se quito aqui: null, que en el movil tambien la quita.
        (None, _, _) => Value::Null,
    };
    mapa.insert("escala".into(), valor);
    serde_json::to_string_pretty(&Value::Object(mapa)).unwrap_or_default()
}

/// Si este lienzo es de los que llevan los puntos como objetos.
///
/// Decide la forma de los trazos que NACEN aqui; los que ya venian mandan
/// ellos mismos. Un lienzo es «del movil» si algun trazo suyo lo dice o si
/// trae la clave `hoja`, que solo escribe el: una hoja recien extraida de un
/// PDF no tiene todavia ningun trazo del que copiar la forma.
fn puntos_como_objetos(lienzo: &Lienzo) -> bool {
    lienzo.resto.contains_key("hoja")
        || lienzo.entradas.iter().any(|e| match e {
            Entrada::Nuestro { original, .. } => puntos_son_objetos(original),
            Entrada::Ajeno(v) => puntos_son_objetos(v),
        })
}

/// Milisegundos desde 1970, que es como Excalidraw fecha sus elementos.
fn ahora_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Lo que dice a los demas aparatos que este elemento cambio: `version` y
/// `updated`. Y a uno recien nacido, lo que ningun original le trae: su `id`
/// y su `type`, sin los que el movil rechaza el fichero entero.
fn sellar(mapa: &mut Map<String, Value>, e: &Elemento, original: &Value) {
    let ahora = ahora_ms();
    let anterior = original.get("version").and_then(Value::as_u64).unwrap_or(0);
    // La escena sube su version en cada paso de un arrastre; la del fichero
    // solo tiene que crecer. La mayor de las dos sirve a ambos.
    let version = (anterior + 1).max(e.version as u64).min(i32::MAX as u64);
    mapa.insert("version".into(), Value::from(version));
    mapa.insert("updated".into(), Value::from(ahora));
    // El desempate de Excalidraw cuando dos aparatos suben a la misma
    // version. No hace falta azar de verdad, solo que no se repita.
    mapa.insert(
        "versionNonce".into(),
        Value::from((ahora as u64 ^ e.id.wrapping_mul(0x9e37_79b9)) & 0x7fff_ffff),
    );
    // **El id de texto tiene que ser SIEMPRE el mismo.** Antes se inventaba
    // uno con la hora dentro (`w<hora><id>`), asi que cambiaba en cada
    // guardado: un enganche nacido aqui entre dos elementos del PC apuntaba
    // al reabrir a un id que ya no existia y la flecha dejaba de seguir a su
    // caja. Ahora sale, por este orden, del id con el que el elemento entro
    // —si vino de un fichero— o de su id interno, con la misma forma `pc<hex>`
    // que usa `enlace` para nombrar lo que nace aqui. Las dos direcciones son
    // la misma cuenta (ver `id_estable`), asi que reabrir devuelve el mismo
    // numero y el enganche sigue atado.
    if !mapa.contains_key("id") {
        let texto = match &e.extras.id_de_fichero {
            Some(suyo) => suyo.clone(),
            None => crate::enlace::id_de_texto(e.id),
        };
        mapa.insert("id".into(), Value::String(texto));
    }
    if !mapa.contains_key("type") {
        let tipo = match &e.figura {
            Figura::Rectangulo => "rectangle",
            // **El foco tiene tipo propio alla y aqui escribia
            // «rectangle».** Un foco hecho en el escritorio llegaba al movil
            // como un rectangulo transparente cualquiera: dejaba de
            // oscurecer, que es lo unico que un foco hace.
            Figura::Foco { .. } => "pixpin-spotlight",
            Figura::Lupa { .. } => "pixpin-lupa",
            Figura::Rombo => "diamond",
            Figura::Mosaico { .. } => "pixpin-mosaic",
            Figura::Elipse => "ellipse",
            // El resaltador no existe alla: viaja como un trazo, con su
            // opacidad, que es lo que lo hace resaltador a la vista.
            Figura::Lapiz { .. } | Figura::Resaltador { .. } => "freedraw",
            Figura::Linea { .. } => "line",
            Figura::Flecha { .. } => "arrow",
            // El emoji viaja como texto: asi excalidraw.com lo ensena.
            Figura::Texto { .. } | Figura::Emoji { .. } => "text",
            Figura::Imagen { .. } => "image",
            Figura::Cota { .. } => "pixpin-measure",
            Figura::EscalaGrafica => "pixpin-scalebar",
            Figura::Marco { .. } => "frame",
            Figura::Arco { .. } => "pixpin-arc",
            Figura::Serie { .. } => "pixpin-serial",
            Figura::Region { .. } => "pixpin-region",
            Figura::Punto { .. } => "pixpin-point",
            Figura::Instrumento(i) => i.tipo(),
        };
        mapa.insert("type".into(), Value::String(tipo.into()));
    }
}

/// La escena con la que se edita este lienzo.
///
/// Va de la mano de `con_escena`: la escena numera sus elementos del 1 en
/// adelante segun entran, asi que el elemento `n` de la escena ES la entrada
/// nuestra numero `n` del lienzo. Por eso se construye aqui y no a mano en
/// cada sitio: quien la llenara en otro orden guardaria cada cambio en el
/// elemento de al lado.
pub fn a_escena(lienzo: &Lienzo) -> crate::Escena {
    let mut escena = crate::Escena::nueva();
    for e in lienzo.elementos() {
        escena.anadir(e);
    }
    // **Lo que nazca aqui, con un id que el fichero no tenga** (13-oct-2026).
    // Lo nuevo sale como `pc<numero>` (`sellar`) y el numero seguia a los
    // elementos leidos: si el movil quito uno (lo borrado no se guarda alla),
    // el siguiente trazo del PC repetia el id del ultimo que nacio aqui. Dos
    // figuras con el mismo id no se juntan una a una al sincronizar
    // (`fusion::con_ids`) y en el movil una tapa a la otra: lo anotado en el
    // PC no llegaba. Asi, lo nuevo cuenta desde el mayor `pc<n>` que haya.
    let mayor = lienzo
        .entradas
        .iter()
        .filter_map(|e| match e {
            Entrada::Nuestro { elemento, original } => elemento
                .extras
                .id_de_fichero
                .as_deref()
                .or_else(|| original.get("id").and_then(Value::as_str))
                .and_then(numero_nacido_aqui),
            Entrada::Ajeno(v) => v
                .get("id")
                .and_then(Value::as_str)
                .and_then(numero_nacido_aqui),
        })
        .max();
    if let Some(n) = mayor {
        escena.siguiente_id = escena.siguiente_id.max(n.saturating_add(1));
    }
    // A mano y no con `poner_fondo`: abrir un lienzo no es un cambio que se
    // pueda deshacer.
    escena.fondo = fondo(lienzo);
    // Lo que mide un pixel. Se leia del fichero y se quedaba en el lienzo:
    // la escena salia sin escala y las cotas del movil decian «37 px» donde
    // alli dicen «5,00 cm» (27-sep-2026).
    escena.escala = lienzo.escala.clone();
    // Los clavos de soldar (`alfileres` del movil): siguen en `resto` y aqui
    // se traducen a los numeros de la escena. Ver `nudos::leer_del_fichero`.
    escena.alfileres = crate::nudos::leer_del_fichero(&lienzo.resto, &escena.elementos);
    // Si las guias estan escondidas: es del dibujo y viaja con el.
    escena.referencias_visibles = referencias_visibles(lienzo);
    // Las tablas de coordenadas y su eje comun: de la escena, no de ningun
    // elemento (`Scene.tablas`, `Scene.origenCoordenadas`).
    escena.tablas = tablas(lienzo);
    escena.origen_coordenadas = origen_coordenadas(lienzo);
    escena
}

/// El numero de un id nacido en el PC (`pc` y hexadecimal, ver
/// `enlace::id_de_texto`); `None` para los demas (los del movil son de 21
/// letras al azar).
fn numero_nacido_aqui(id: &str) -> Option<u64> {
    let h = id.strip_prefix("pc")?;
    if h.is_empty() || h.len() > 16 {
        return None;
    }
    u64::from_str_radix(h, 16).ok()
}

/// **El papel del lienzo.** Primero el `backgroundColor` de ARRIBA, que es
/// donde lo guarda el movil de verdad (`Scene.backgroundColor`, lo que viaja
/// al sincronizar: `ExcalidrawStore.guardar` escribe la escena entera y sin
/// `appState`); si no, el `appState.viewBackgroundColor` de un `.excalidraw`
/// de la web o de `ExcalidrawStore.exportar`; y si no, blanco. Manda el de
/// arriba cuando estan los dos porque es el que el movil sigue cambiando: el
/// `appState` lo ignora (`ignoreUnknownKeys`).
///
/// Sin alfa, como lo lee el movil (`DrawTheme.colorDe` se queda con los seis
/// ultimos digitos): un papel medio transparente se veria distinto en cada
/// lado. `"transparent"` tampoco es un papel: blanco, como el movil.
pub fn fondo(lienzo: &Lienzo) -> ColorRgba {
    papel_leido(lienzo).map_or(crate::escena::FONDO_DE_FABRICA, |c| ColorRgba {
        a: 1.0,
        ..c
    })
}

/// El papel que dice el fichero, si dice alguno que se entienda.
fn papel_leido(lienzo: &Lienzo) -> Option<ColorRgba> {
    color_desde(lienzo.resto.get(PAPEL_DE_LA_ESCENA)).or_else(|| {
        lienzo
            .resto
            .get("appState")
            .and_then(|a| color_desde(a.get("viewBackgroundColor")))
    })
}

/// La clave del papel en la `Scene` del movil.
const PAPEL_DE_LA_ESCENA: &str = "backgroundColor";

/// Pone el papel de la escena en el lienzo, **solo si cambio**: lo que vino
/// (su `gridSize`, su tema, lo que sea) sale intacto, y un lienzo del movil
/// que nadie repinto no cambia ni un byte por haberlo abierto aqui.
///
/// Va SIEMPRE arriba (`backgroundColor`), que es lo unico que lee el movil
/// al abrir lo sincronizado. Y en el `appState` cuando el fichero ya lo
/// traia o no es una escena del movil, para que excalidraw.com lo vea
/// igual; a una escena del movil no se le inventa un `appState`.
fn fondo_hacia(salida: &mut Lienzo, papel: ColorRgba) {
    if fondo(salida) == papel {
        return;
    }
    let es_escena_del_movil = salida.resto.contains_key(PAPEL_DE_LA_ESCENA);
    let texto = Value::String(color_hacia(papel));
    salida
        .resto
        .insert(PAPEL_DE_LA_ESCENA.into(), texto.clone());
    if es_escena_del_movil && !salida.resto.contains_key("appState") {
        return;
    }
    let estado = salida
        .resto
        .entry("appState")
        .or_insert_with(|| Value::Object(Map::new()));
    // Un `appState` que no es un objeto no se puede completar: se sustituye,
    // porque excalidraw.com lo lee como objeto y rechazaria el fichero.
    if !estado.is_object() {
        *estado = Value::Object(Map::new());
    }
    if let Value::Object(mapa) = estado {
        mapa.insert("viewBackgroundColor".into(), texto);
    }
}

/// El lienzo con lo que se hizo en la escena que salio de `a_escena`.
///
/// Lo ajeno se queda donde estaba y como estaba. Lo nuestro que se borro NO
/// se quita: se marca `isDeleted`, como hace Excalidraw, porque es lo que le
/// dice a otro aparato «esto se borro» en vez de «esto no lo he visto
/// nunca». Lo nuevo va al final, que es encima de todo.
pub fn con_escena(lienzo: &Lienzo, escena: &crate::Escena) -> Lienzo {
    let mut salida = lienzo.clone();
    // Y de vuelta: calibrar en Windows se ve en el movil (`escribir` decide
    // si sale la cruda de siempre o la nueva).
    salida.escala = escena.escala.clone();
    fondo_hacia(&mut salida, escena.fondo);
    referencias_visibles_hacia(&mut salida, escena.referencias_visibles);
    tablas_hacia(&mut salida, escena);
    let mut n: u64 = 0;
    for entrada in &mut salida.entradas {
        let Entrada::Nuestro { elemento, .. } = entrada else {
            continue;
        };
        n += 1;
        match escena.buscar(n) {
            Some(editado) => {
                // El id de la escena es de la sesion; el del lienzo sale del
                // texto del fichero y es el que compara `escribir`. Y la
                // version sola no es un cambio: la escena la sube al tocar.
                let (id, version) = (elemento.id, elemento.version);
                let mut editado = editado.clone();
                editado.id = id;
                let subida = editado.version;
                editado.version = version;
                if editado != *elemento {
                    editado.version = subida;
                    *elemento = editado;
                }
            }
            // La escena lo tiro del todo al compactar: borrado igual.
            None => elemento.borrado = true,
        }
    }
    let mut nuevos: Vec<&Elemento> = escena.visibles().filter(|e| e.id > n).collect();
    nuevos.sort_by_key(|e| e.id);
    // Las fotos que nacen aqui necesitan su `fileId`, que es texto y no se
    // puede sacar del numero: se busca la entrada de `files` cuyo id da ese
    // numero (la que quien guarda acaba de poner con [`poner_fichero`], o la
    // de una foto que ya estaba y se copio y pego dentro del lienzo).
    let claves: HashMap<u64, String> = match salida.resto.get("files") {
        Some(Value::Object(files)) => files.keys().map(|k| (id_estable(k), k.clone())).collect(),
        _ => HashMap::new(),
    };
    for e in nuevos {
        let original = match e.figura {
            // Una foto sin fichero en `files` no se escribe: dejaria en el
            // movil un hueco que no abre.
            Figura::Imagen { id_objeto } => match claves.get(&id_objeto) {
                Some(clave) => serde_json::json!({
                    "fileId": clave,
                    "status": "saved",
                    "scale": [1.0, 1.0],
                }),
                None => continue,
            },
            _ => Value::Null,
        };
        salida.entradas.push(Entrada::Nuestro {
            elemento: e.clone(),
            original: Box::new(original),
        });
    }
    // Los clavos, solo si cambiaron (un lienzo que nadie toco sale igual).
    crate::nudos::escribir_al_fichero(&mut salida.resto, escena);
    salida
}

/// La escala del JSON del movil, o `None` si no la hay o no vale.
///
/// Una escala imposible se ignora en vez de adoptarse (D36): mas vale no
/// medir que medir mal.
fn escala_desde(v: &Value) -> Option<Escala> {
    let upp = v.get("unidadesPorPixel")?.as_f64()? as f32;
    if !upp.is_finite() || upp <= 0.0 {
        return None;
    }
    Some(Escala {
        unidades_por_pixel: upp,
        unidad: v
            .get("unidad")
            .and_then(Value::as_str)
            .unwrap_or("m")
            .to_string(),
        // El mismo tope que `Escala::calibrando` (medida.rs): sin el, un
        // "decimales" disparatado del movil entraria tal cual en el tipo, y
        // solo se veria recortado de casualidad porque `formatear_valor`
        // vuelve a recortar a 6 al pintar.
        decimales: (v.get("decimales").and_then(Value::as_u64).unwrap_or(2) as u8).min(6),
    })
}

fn escala_hacia(e: &Escala) -> Value {
    let mut m = Map::new();
    m.insert(
        "unidadesPorPixel".into(),
        Value::from(e.unidades_por_pixel as f64),
    );
    m.insert("unidad".into(), Value::String(e.unidad.clone()));
    m.insert("decimales".into(), Value::from(e.decimales));
    Value::Object(m)
}

// --- Traduccion de un elemento ---

fn num(v: &Value, clave: &str) -> Option<f32> {
    v.get(clave).and_then(|x| x.as_f64()).map(|x| x as f32)
}

fn num_o(v: &Value, clave: &str, si_no: f32) -> f32 {
    num(v, clave).unwrap_or(si_no)
}

/// Los puntos de un trazo, pasados de relativos a ABSOLUTOS.
fn puntos_desde(v: &Value, x: f32, y: f32) -> Vec<Punto2> {
    v.get("points")
        .and_then(|p| p.as_array())
        .map(|lista| {
            lista
                .iter()
                .filter_map(|p| {
                    // Dos formas, y las dos son de verdad: Excalidraw escribe
                    // `[x, y]` y PixPin Android escribe `{"x":…,"y":…}`. Leer
                    // solo la primera dejaba los trazos del movil SIN PUNTOS:
                    // el dibujo se abria con sus mil elementos y no se veia
                    // ninguno, solo las figuras, que no llevan puntos.
                    let (px, py) = match p {
                        Value::Array(par) => (par.first()?.as_f64()?, par.get(1)?.as_f64()?),
                        Value::Object(o) => (o.get("x")?.as_f64()?, o.get("y")?.as_f64()?),
                        _ => return None,
                    };
                    Some(Punto2 {
                        x: x + px as f32,
                        y: y + py as f32,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Una lista de puntos suelta —un anillo de `huecos`—, de relativa a
/// ABSOLUTA. Acepta las dos formas, como `puntos_desde`: el movil escribe
/// `{"x":…,"y":…}` y Excalidraw, `[x, y]`.
fn lista_de_puntos(v: &Value, x: f32, y: f32) -> Vec<Punto2> {
    v.as_array()
        .map(|lista| {
            lista
                .iter()
                .filter_map(|p| {
                    let (px, py) = match p {
                        Value::Array(par) => (par.first()?.as_f64()?, par.get(1)?.as_f64()?),
                        Value::Object(o) => (o.get("x")?.as_f64()?, o.get("y")?.as_f64()?),
                        _ => return None,
                    };
                    Some(Punto2 {
                        x: x + px as f32,
                        y: y + py as f32,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Y al reves, de absolutos a relativos.
fn puntos_hacia(puntos: &[Punto2], x: f32, y: f32, objetos: bool) -> Value {
    Value::Array(
        puntos
            .iter()
            .map(|p| {
                let (px, py) = ((p.x - x) as f64, (p.y - y) as f64);
                if objetos {
                    // La forma del movil: su `Pt(x, y)` es una clase, y una
                    // lista `[x, y]` le hace rechazar el fichero ENTERO.
                    let mut o = Map::new();
                    o.insert("x".into(), Value::from(px));
                    o.insert("y".into(), Value::from(py));
                    Value::Object(o)
                } else {
                    Value::Array(vec![Value::from(px), Value::from(py)])
                }
            })
            .collect(),
    )
}

/// Si los puntos de este JSON vienen como objetos, que es como los escribe
/// el movil.
fn puntos_son_objetos(v: &Value) -> bool {
    v.get("points")
        .and_then(Value::as_array)
        .and_then(|l| l.first())
        .is_some_and(Value::is_object)
}

/// El `fillStyle` de Excalidraw, con sus cinco palabras exactas.
///
/// Cualquier otra cosa —que no venga, que venga vacia, o que sea un estilo de
/// una version futura del movil— cae en el de por omision. No se rechaza el
/// elemento entero por esto: un relleno que no sabemos pintar no es motivo
/// para que un rectangulo deje de ser un rectangulo.
fn estilo_relleno_desde(v: Option<&Value>) -> EstiloRelleno {
    match v.and_then(Value::as_str) {
        Some("solid") => EstiloRelleno::Solido,
        Some("cross-hatch") => EstiloRelleno::Cruzado,
        Some("zigzag") => EstiloRelleno::Zigzag,
        Some("pixpin-lines") => EstiloRelleno::LineasPixpin,
        _ => EstiloRelleno::Rayado,
    }
}

fn estilo_relleno_hacia(e: EstiloRelleno) -> &'static str {
    match e {
        EstiloRelleno::Solido => "solid",
        EstiloRelleno::Rayado => "hachure",
        EstiloRelleno::Cruzado => "cross-hatch",
        EstiloRelleno::Zigzag => "zigzag",
        EstiloRelleno::LineasPixpin => "pixpin-lines",
    }
}

/// Escribe una punta en su clave, **sin pisar la que no entendimos**.
///
/// Misma regla que el `fillStyle` y que el `material`: una punta de una
/// version futura del movil entra aqui como ninguna, asi que escribir `null`
/// encima la borraria para siempre. Mientras siga sin punta —o sea, mientras
/// aqui no se haya elegido otra— se devuelve el original tal cual.
fn punta_hacia_el_mapa(mapa: &mut Map<String, Value>, clave: &str, p: TipoPunta) {
    let ajena = mapa
        .get(clave)
        .and_then(Value::as_str)
        .is_some_and(|s| TipoPunta::desde_palabra(s).is_none());
    if ajena && p == TipoPunta::Ninguna {
        return;
    }
    mapa.insert(
        clave.into(),
        match p.palabra() {
            Some(s) => Value::String(s.into()),
            None => Value::Null,
        },
    );
}

/// Una punta de flecha del fichero. `ausente` es lo que vale cuando el campo
/// no esta —distinto en cada extremo: el principio no lleva punta y el final
/// si—; un `null` explicito es «sin punta» en los dos, y una palabra que no
/// conocemos tambien, porque no hay nada mejor que dibujar.
///
/// Que una punta futura se lea como ninguna NO la borra del fichero: el
/// elemento se escribe partiendo de su JSON original y `startArrowhead` solo
/// se pisa si aqui se entendio (ver `punta_hacia_el_mapa`).
fn punta_desde(v: Option<&Value>, ausente: TipoPunta) -> TipoPunta {
    match v {
        None => ausente,
        Some(Value::String(s)) => TipoPunta::desde_palabra(s).unwrap_or(TipoPunta::Ninguna),
        Some(_) => TipoPunta::Ninguna,
    }
}

/// Las palabras de `fillStyle` que este lado sabe leer. Es la lista que usa
/// la proteccion del estilo ajeno al escribir, y sale de
/// [`estilo_relleno_hacia`] para que no pueda quedarse corta: el dia que
/// entre una trama nueva, el `match` de alla obliga a nombrarla y esta lista
/// la hereda.
fn palabra_de_relleno_conocida(s: &str) -> bool {
    [
        EstiloRelleno::Solido,
        EstiloRelleno::Rayado,
        EstiloRelleno::Cruzado,
        EstiloRelleno::Zigzag,
        EstiloRelleno::LineasPixpin,
    ]
    .into_iter()
    .any(|e| estilo_relleno_hacia(e) == s)
}

/// Traduce un elemento de Excalidraw al nuestro. `None` si no sabemos que es.
fn elemento_desde(v: &Value) -> Option<Elemento> {
    let tipo = v.get("type")?.as_str()?;
    // Lo borrado en Excalidraw sigue en el fichero con `isDeleted`. Se salta:
    // no es un elemento ajeno que haya que conservar, es basura que el propio
    // formato marca como tal.
    if v.get("isDeleted").and_then(|b| b.as_bool()) == Some(true) {
        return None;
    }
    let x = num_o(v, "x", 0.0);
    let y = num_o(v, "y", 0.0);
    let figura = match tipo {
        "rectangle" => Figura::Rectangulo,
        // El marco de Excalidraw. Sus hijos llevan alli un `frameId`; aqui la
        // pertenencia se mira por la caja (ver `marco.rs`), asi que ese campo
        // se queda en `resto` y vuelve tal cual al guardar.
        "frame" => Figura::Marco {
            nombre: v
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or_default()
                .to_string(),
        },
        "diamond" => Figura::Rombo,
        "pixpin-mosaic" => Figura::Mosaico {
            desenfoque: v
                .get("mosaicBlur")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        },
        // El foco del movil, entero: su hueco (`forma`, `foco`, `focoAncho`,
        // `focoAlto`, `lupaRedonda`) y cuanto oscurece (`oscurecer`), los
        // mismos campos que la lupa (`lupa_elemento::leer`). Antes solo se
        // miraba si era redondo y aqui oscurecia el lienzo entero.
        "pixpin-spotlight" => Figura::Foco {
            cristal: crate::lupa_elemento::leer(v),
        },
        // La lupa del movil (F: lienzo-imagen): se lee entera. Antes viajaba
        // como ajena y aqui no se veia.
        "pixpin-lupa" => Figura::Lupa {
            cristal: crate::lupa_elemento::leer(v),
        },
        "ellipse" => Figura::Elipse,
        "line" => Figura::Linea {
            puntos: puntos_desde(v, x, y),
        },
        "arrow" => Figura::Flecha {
            puntos: puntos_desde(v, x, y),
            punta_inicio: punta_desde(v.get("startArrowhead"), TipoPunta::Ninguna),
            // Una flecha sin `endArrowhead` es una flecha con punta: es lo que
            // significa el campo ausente en Excalidraw, y el campo puesto a
            // `null` es lo contrario —una flecha a la que le quitaron la
            // punta—. Los dos casos existen en ficheros reales.
            punta_fin: punta_desde(v.get("endArrowhead"), TipoPunta::Flecha),
            codos: v.get("elbowed").and_then(Value::as_bool) == Some(true),
        },
        "freedraw" => {
            // `simulatePressure: true` manda sobre `pressures`: Excalidraw las
            // ignora al pintar aunque vengan en el fichero.
            let simulada = v.get("simulatePressure").and_then(|b| b.as_bool()) == Some(true);
            Figura::Lapiz {
                puntos: puntos_desde(v, x, y),
                presiones: if simulada {
                    Vec::new()
                } else {
                    v.get("pressures")
                        .and_then(|p| p.as_array())
                        .map(|l| {
                            l.iter()
                                .filter_map(|p| p.as_f64())
                                .map(|p| p as f32)
                                .collect()
                        })
                        .unwrap_or_default()
                },
                // Todo freedraw de Excalidraw lleva el grosor en `strokeWidth`,
                // con o sin `strokeOptions`: nunca es legado.
                opciones: Some(
                    v.get("strokeOptions")
                        .and_then(|s| serde_json::from_value(s.clone()).ok())
                        .unwrap_or_default(),
                ),
            }
        }
        // Una imagen del paquete: sus pixeles NO estan aqui, sino en
        // `imagenes/<id>` (ver `ficheros`). El elemento solo lleva a que
        // fichero apunta, y quien pinta lo resuelve.
        "image" => Figura::Imagen {
            id_objeto: id_estable(v.get("fileId").and_then(Value::as_str).unwrap_or_default()),
        },
        "text" if v["customData"]["pixpin"] == "emoji" => Figura::Emoji {
            caracter: v.get("text").and_then(|t| t.as_str()).unwrap_or("").into(),
        },
        "text" => Figura::Texto {
            texto: v.get("text").and_then(|t| t.as_str()).unwrap_or("").into(),
            tam: num_o(v, "fontSize", 20.0),
            // `fontFamily` ni se leia: todo texto del movil entraba aqui con
            // «Segoe UI» dijera lo que dijera el fichero, y salia con ese
            // nombre escrito donde el movil espera un numero.
            familia: crate::texto::nombre_de_familia(
                v.get("fontFamily")
                    .and_then(Value::as_u64)
                    .map(|n| n.min(u8::MAX as u64) as u8),
            )
            .into(),
        },
        "pixpin-measure" => {
            let puntos = puntos_desde(v, x, y);
            // Sin dos puntos no hay raya que dibujar ni que tocar
            // (pintado.rs y impacto.rs exigen `len() >= 2`): seria una cota
            // invisible e inseleccionable que se guarda para siempre. Mejor
            // que caiga al carril ajeno, que es donde sobrevivia intacta
            // antes de que aprendieramos a leer `pixpin-measure`.
            if puntos.len() < 2 {
                return None;
            }
            Figura::Cota { puntos }
        }
        "pixpin-scalebar" => Figura::EscalaGrafica,
        // Los tres instrumentos de graficar del movil son uno solo aqui, con
        // el numero de ejes sacado del tipo. Sus numeros se leen como `f64`
        // para volver con los mismos bytes; uno que no es un numero se lee
        // como ausente y se deja intacto al escribir (`instrumento_hacia`).
        t if crate::instrumento::Instrumento::ejes_del_tipo(t).is_some() => {
            Figura::Instrumento(instrumento_desde(v, t))
        }
        // El arco: un trozo del ovalo de su caja. `arcStart` y `arcSweep`
        // van en RADIANES en los dos aparatos, asi que el numero pasa tal
        // cual: `Element.kt` los documenta «en radianes» y `Arco.kt` los
        // mete crudos en `cos`/`sin` y los compara contra 2π. Convertir
        // aqui hacia grados encogia un arco de media vuelta del movil hasta
        // un punto, y mandaba los nuestros como veinte vueltas.
        //
        // `arcSweep` ausente o nulo no se rellena con un cero: es el estado
        // «todavia es solo la guia» y tiene que sobrevivir al viaje, o un
        // ovalo guia del movil volveria convertido en un arco vacio.
        "pixpin-arc" => Figura::Arco {
            inicio: num_o(v, "arcStart", 0.0),
            barrido: num(v, "arcSweep"),
        },
        // El numero de serie. El movil lo guarda como TEXTO en `text`
        // porque su elemento es plano y ahi cabe cualquier rotulo; aqui es
        // un numero, que es lo que de verdad es. Si viniera algo que no es
        // un numero —un rotulo a mano en una version futura— el elemento
        // cae al carril ajeno en vez de perder lo que ponia.
        "pixpin-serial" => Figura::Serie {
            numero: v.get("text").and_then(Value::as_str)?.trim().parse().ok()?,
        },
        // Lo que pinto el bote de relleno: el contorno encontrado en
        // `points` y sus agujeros en `huecos`, los dos relativos a (x, y).
        "pixpin-region" => Figura::Region {
            contorno: puntos_desde(v, x, y),
            huecos: v
                .get("huecos")
                .and_then(Value::as_array)
                .map(|anillos| anillos.iter().map(|a| lista_de_puntos(a, x, y)).collect())
                .unwrap_or_default(),
        },
        // El punto etiquetado: su caja no tiene tamano, (x, y) ES el punto.
        //
        // `etiquetaAngulo` va en RADIANES en los dos aparatos: `Puntos.kt`
        // lo mete directo en `cos`/`sin` y lo produce con `atan2`. Los dos
        // campos son nulables, asi que con `explicitNulls = false` el movil
        // NO los escribe cuando no los ha tocado; los valores por omision
        // tienen que ser entonces los suyos —`-PI/4` y `22.0`— o la letra
        // de un punto recien nacido en el movil se coloca aqui en otro
        // sitio y al guardar se le mueve a el.
        "pixpin-point" => Figura::Punto {
            letra: v.get("text").and_then(Value::as_str).unwrap_or("").into(),
            angulo: num_o(v, "etiquetaAngulo", -std::f32::consts::FRAC_PI_4),
            radio: num_o(v, "etiquetaRadio", 22.0),
        },
        // El cronograma (`pixpin-gantt`) se quito del lienzo: ya no se lee y
        // cae con el resto al carril ajeno, que lo devuelve intacto al guardar.
        // El resto son suyos y no sabemos dibujarlos: `pixpin-solid`,
        // `pixpin-nudo`... Se conservan como ajenos.
        _ => return None,
    };
    Some(Elemento {
        // El identificador de texto no cabe en el nuestro. Se guarda uno
        // derivado para que sea estable dentro de la sesion, y el original
        // vuelve del JSON al escribir.
        id: id_estable(v.get("id").and_then(|i| i.as_str()).unwrap_or("")),
        figura,
        x,
        y,
        ancho: num_o(v, "width", 0.0),
        alto: num_o(v, "height", 0.0),
        angulo: num_o(v, "angle", 0.0),
        trazo: color_desde(v.get("strokeColor")).unwrap_or(ColorRgba::opaco(0.1, 0.1, 0.1)),
        relleno: color_desde(v.get("backgroundColor")),
        estilo_relleno: estilo_relleno_desde(v.get("fillStyle")),
        // Un `material` que no conocemos se lee como lisa y **se conserva**:
        // ver `material_hacia`, que es quien no lo pisa al escribir.
        material: v
            .get("material")
            .and_then(Value::as_str)
            .and_then(MaterialTinta::desde_palabra)
            .unwrap_or_default(),
        grosor: num_o(v, "strokeWidth", 2.0),
        estilo: match v.get("strokeStyle").and_then(|s| s.as_str()) {
            Some("dashed") if v.get("trazoRayaPunto").and_then(Value::as_bool) == Some(true) => {
                EstiloTrazo::RayaPunto
            }
            Some("dashed") => EstiloTrazo::Discontinuo,
            Some("dotted") => EstiloTrazo::Punteado,
            _ => EstiloTrazo::Solido,
        },
        rugosidad: num_o(v, "roughness", 1.0),
        // El suyo va de 0 a 100 y el nuestro de 0 a 1.
        opacidad: (num_o(v, "opacity", 100.0) / 100.0).clamp(0.0, 1.0),
        // **La semilla es el campo mas importante del modelo**: sin ella la
        // figura se sortea de nuevo y cambia de forma al reabrirla. Por eso
        // se acepta tambien escrita como decimal (`1001.0`): un JSON que ha
        // pasado por una herramienta que serializa todos los numeros como
        // coma flotante seguia siendo legible en todo menos en esto, y el
        // dibujo volvia con otro garabato sin que nada avisara.
        semilla: v
            .get("seed")
            .and_then(|s| s.as_u64().or_else(|| s.as_f64().map(|f| f as u64)))
            .map(|s| s as u32)
            .unwrap_or(1),
        version: v
            .get("version")
            .and_then(|s| s.as_u64())
            .map(|s| s as u32)
            .unwrap_or(1),
        borrado: false,
        bloqueado: v.get("locked").and_then(Value::as_bool).unwrap_or(false),
        // El `enlace` del movil: el id del dibujo de la hoja a la que lleva.
        // En Excalidraw no existe, asi que viaja tal cual en `resto` y vuelve
        // intacto al guardar.
        enlace: v
            .get("enlace")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        // `roundness` es un objeto con su tipo; aqui solo importa si las
        // esquinas van redondeadas o en punta.
        redondo: v.get("roundness").is_some_and(|r| !r.is_null()),
        grupos: v
            .get("groupIds")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|g| g.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        extras: extras_desde(v),
    })
}

/// Los diez campos del movil que ahora el PC entiende. Ver `Extras`.
///
/// Todo con valor de reserva: un elemento que no traiga ninguno de estos
/// campos —la mayoria— da un `Extras::default()`, y eso es lo que hace que
/// `escribir` siga devolviendo intacto lo que nadie toco.
fn extras_desde(v: &Value) -> Extras {
    let si = |clave: &str| v.get(clave).and_then(Value::as_bool).unwrap_or(false);
    Extras {
        presion_firme: si("presionFirme"),
        referencia: si("reference"),
        negrita: si("negrita"),
        cursiva: si("cursiva"),
        tachado: si("tachado"),
        contenedor: v
            .get("containerId")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        enganche_inicio: v.get("startBinding").and_then(enganche_desde),
        enganche_fin: v.get("endBinding").and_then(enganche_desde),
        atados: v
            .get("boundElements")
            .and_then(Value::as_array)
            .map(|lista| {
                lista
                    .iter()
                    .filter_map(|a| {
                        Some(Atado {
                            id: a.get("id")?.as_str()?.to_string(),
                            tipo: a
                                .get("type")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
        // Un papel que no conocemos se lee como «ninguno» y **se conserva**:
        // ver `extras_hacia`, que no lo pisa mientras aqui no se toque. La
        // misma regla del `fillStyle` y del `material`.
        papel: v
            .get("papel")
            .and_then(Value::as_str)
            .and_then(TamanoPapel::desde_palabra),
        pauta: v
            .get("pauta")
            .and_then(Value::as_str)
            .and_then(PautaHoja::desde_palabra)
            .unwrap_or_default(),
        // **Con que id entro.** Nuestro `id` es un `u64` derivado de este
        // texto, y esa cuenta no se puede deshacer: sin guardarlo, un
        // enganche nacido aqui apuntaria a un id que al reabrir ya no
        // existe. Ver `Extras::id_de_fichero`.
        id_de_fichero: v
            .get("id")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        recorte: v.get("crop").and_then(recorte_desde),
        // **La fuente rica** (`textoRico`), solo si sigue diciendo lo mismo
        // que el texto plano: si alguien que no la entiende (el movil de
        // hoy, Excalidraw) cambio el `text`, la fuente esta vieja y gana lo
        // que se escribio alli.
        rico: v
            .get("textoRico")
            .and_then(Value::as_str)
            .filter(|r| !r.is_empty())
            .filter(|r| {
                let text = v.get("text").and_then(Value::as_str).unwrap_or_default();
                crate::texto_rico::plano(r) == text
            })
            .map(str::to_string),
        // Una palabra que no conocemos se lee como «no lo dice» y **se
        // conserva**: la rama del texto de `elemento_hacia` no la pisa
        // mientras aqui no se elija otra. La regla del `fillStyle`.
        alineacion: v
            .get("textAlign")
            .and_then(Value::as_str)
            .and_then(crate::texto::AlineacionTexto::desde_palabra),
        alineacion_vertical: v
            .get("verticalAlign")
            .and_then(Value::as_str)
            .and_then(crate::texto::AlineacionVertical::desde_palabra),
        // La letra de lo que rotula sin ser un texto (su `fontFamily`:
        // la cota, el numero de serie, el punto, la escala); la
        // de un texto va en su figura y no aqui. Antes no se leia y
        // una cota del movil en Caveat se veia aqui en la letra del sistema.
        familia: (v
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|t| t != "text"))
        .then(|| v.get("fontFamily").and_then(Value::as_u64))
        .flatten()
        .map(|n| crate::texto::nombre_de_familia(Some(n.min(u8::MAX as u64) as u8)).to_string()),
        // El tamano de lo que rotula: el numero de la cota
        // (`e.fontSize ?: MEASURE_TEXT_SIZE` en `drawMeasure`) y las cifras
        // de los instrumentos (`e.fontSize ?: TAM_DE_LA_CIFRA` en `drawPlano`).
        tam_letra: matches!(
            v.get("type").and_then(Value::as_str),
            Some(
                "pixpin-measure"
                    | "pixpin-axes"
                    | "pixpin-number-line"
                    | "pixpin-space"
                    // La letra del punto (`e.fontSize ?: 20` de `Puntos.kt`) y
                    // el valor de una marca de angulo.
                    | "pixpin-point"
                    | "pixpin-arc"
            )
        )
        .then(|| num(v, "fontSize"))
        .flatten()
        .filter(|t| t.is_finite() && *t > 0.0),
        // La dureza del grafito (v0.91 del movil). Una palabra que no
        // conocemos se lee como «ninguna» (HB) y **se conserva**: ver
        // `dureza_hacia`, la misma regla del `material`.
        dureza: v
            .get("dureza")
            .and_then(Value::as_str)
            .and_then(crate::tinta::grafito::Dureza::desde_rotulo),
        cierre: crate::geometria_exacta::cierre_desde(v),
        marca_angulo: crate::geometria_exacta::marca_desde(v),
        degradado: crate::geometria_exacta::degradado_desde(v),
    }
}

/// **La dureza de vuelta al JSON** (`dureza` del movil).
///
/// La que hay se escribe con su nombre («8B»). Sin ninguna (HB) la clave se
/// quita, que es como la escribe el movil (`explicitNulls = false` con el
/// campo a null) y como la lee: ausente es HB. Pero una palabra que aqui no
/// se entiende —una dureza de una version futura— se deja intacta mientras
/// aqui no se elija otra, como el `material` y el `fillStyle`.
fn dureza_hacia(mapa: &mut Map<String, Value>, d: Option<crate::tinta::grafito::Dureza>) {
    match d {
        Some(d) => {
            mapa.insert("dureza".into(), Value::String(d.rotulo().into()));
        }
        None => {
            let conocida = mapa
                .get("dureza")
                .and_then(Value::as_str)
                .is_some_and(|s| crate::tinta::grafito::Dureza::desde_rotulo(s).is_some());
            if conocida || mapa.get("dureza").is_some_and(Value::is_null) {
                mapa.remove("dureza");
            }
        }
    }
}

/// **Si las lineas de referencia estan a la vista** (`referenciasVisibles`
/// de la escena del movil). Sin la clave, a la vista, como el movil.
pub fn referencias_visibles(lienzo: &Lienzo) -> bool {
    lienzo
        .resto
        .get("referenciasVisibles")
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

/// Y de vuelta. Solo si cambio: un lienzo que nadie toco no estrena la clave
/// (el movil la escribe siempre, y la que trae vuelve tal cual).
fn referencias_visibles_hacia(salida: &mut Lienzo, visibles: bool) {
    if referencias_visibles(salida) == visibles {
        return;
    }
    salida
        .resto
        .insert("referenciasVisibles".into(), Value::Bool(visibles));
}

/// **El instrumento de un elemento del movil** (`unidad`, `pasoDeNumeros`,
/// `pasoDeCuadros`, `azimut`, `elevacion` de `Element.kt`), con sus ejes
/// sacados del tipo. Lo que no es un numero se lee como ausente.
fn instrumento_desde(v: &Value, tipo: &str) -> crate::instrumento::Instrumento {
    let n = |clave: &str| {
        v.get(clave)
            .and_then(Value::as_f64)
            .filter(|x| x.is_finite())
    };
    crate::instrumento::Instrumento {
        ejes: crate::instrumento::Instrumento::ejes_del_tipo(tipo).unwrap_or(2),
        unidad: n("unidad"),
        paso_numeros: n("pasoDeNumeros"),
        paso_cuadros: n("pasoDeCuadros"),
        azimut: n("azimut"),
        elevacion: n("elevacion"),
    }
}

/// Y de vuelta. Solo se escribe lo que hay: un campo ausente no se estrena y
/// uno que no se entendio (no era un numero) se queda como vino, la regla del
/// `material` y del `fillStyle`.
fn instrumento_hacia(mapa: &mut Map<String, Value>, i: &crate::instrumento::Instrumento) {
    for (clave, valor) in [
        ("unidad", i.unidad),
        ("pasoDeNumeros", i.paso_numeros),
        ("pasoDeCuadros", i.paso_cuadros),
        ("azimut", i.azimut),
        ("elevacion", i.elevacion),
    ] {
        if let Some(x) = valor {
            mapa.insert(clave.into(), Value::from(x));
        }
    }
}

/// **Las tablas de coordenadas de la escena** (`Scene.tablas` del movil).
/// Una lista rota se lee vacia, pero no se pisa al escribir mientras aqui no
/// se cambie ninguna (`tablas_hacia`).
pub fn tablas(lienzo: &Lienzo) -> Vec<crate::instrumento::tabla::TablaDeCoordenadas> {
    lienzo
        .resto
        .get("tablas")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default()
}

/// **El eje comun de las coordenadas** (`Scene.origenCoordenadas`): `{x, y}`
/// como lo escribe el movil, o `[x, y]` como lo dejaba el PC antes de
/// traducirlo (`para_el_movil::a_punto`).
pub fn origen_coordenadas(lienzo: &Lienzo) -> Option<Punto2> {
    let v = lienzo.resto.get("origenCoordenadas")?;
    let (x, y) = match v {
        Value::Object(m) => (m.get("x")?.as_f64()?, m.get("y")?.as_f64()?),
        Value::Array(a) if a.len() >= 2 => (a[0].as_f64()?, a[1].as_f64()?),
        _ => return None,
    };
    Some(Punto2::nuevo(x as f32, y as f32))
}

/// Las tablas y su eje de vuelta, **solo si cambiaron**: un lienzo del movil
/// que nadie toco no cambia ni un byte. Cada tabla se escribe encima de la
/// suya de antes (por su `id`), asi una clave que aqui no se conoce sigue
/// ahi; y la que no cambio vuelve tal cual.
fn tablas_hacia(salida: &mut Lienzo, escena: &crate::Escena) {
    if tablas(salida) != escena.tablas {
        let antes: Vec<Value> = match salida.resto.get("tablas") {
            Some(Value::Array(l)) => l.clone(),
            _ => Vec::new(),
        };
        let nuevas: Vec<Value> = escena
            .tablas
            .iter()
            .map(|t| {
                let suya = antes
                    .iter()
                    .find(|v| v.get("id").and_then(Value::as_str) == Some(t.id.as_str()));
                if let Some(v) = suya
                    && serde_json::from_value::<crate::instrumento::tabla::TablaDeCoordenadas>(
                        v.clone(),
                    )
                    .ok()
                    .as_ref()
                        == Some(t)
                {
                    return v.clone();
                }
                let mut m = match suya {
                    Some(Value::Object(m)) => m.clone(),
                    _ => Map::new(),
                };
                if let Value::Object(nueva) = serde_json::to_value(t).unwrap_or(Value::Null) {
                    m.extend(nueva);
                }
                Value::Object(m)
            })
            .collect();
        salida.resto.insert("tablas".into(), Value::Array(nuevas));
    }
    if origen_coordenadas(salida) != escena.origen_coordenadas {
        let valor = match escena.origen_coordenadas {
            Some(p) => serde_json::json!({ "x": p.x as f64, "y": p.y as f64 }),
            None => Value::Null,
        };
        salida.resto.insert("origenCoordenadas".into(), valor);
    }
}

/// El `crop` de una imagen. Sin sus cuatro medidas no hay recorte: uno a
/// medias no dice que trozo ensenar, y ensenar la foto entera es mejor que
/// inventarse el trozo. Si falta el tamano natural se lee como cero: el
/// recorte se conserva (vuelve tal cual al guardar) pero `trozo_en` lo
/// rechaza y se ensena la foto entera, porque sin saber contra que tamano se
/// midio no se puede llevar a los pixeles que hay.
fn recorte_desde(v: &Value) -> Option<crate::elemento::RecorteImagen> {
    let (ancho, alto) = (num(v, "width")?, num(v, "height")?);
    Some(crate::elemento::RecorteImagen {
        x: num(v, "x")?,
        y: num(v, "y")?,
        ancho,
        alto,
        ancho_natural: num(v, "naturalWidth").unwrap_or(0.0),
        alto_natural: num(v, "naturalHeight").unwrap_or(0.0),
    })
}

/// Un `Binding` del movil. Sin `elementId` no hay enganche que valga: un
/// anclaje que no dice a quien se ata no ata a nadie.
fn enganche_desde(v: &Value) -> Option<Enganche> {
    Some(Enganche {
        elemento: v.get("elementId")?.as_str()?.to_string(),
        foco: num_o(v, "focus", 0.0),
        hueco: num_o(v, "gap", 1.0),
        punto_fijo: v
            .get("fixedPoint")
            .and_then(Value::as_array)
            .and_then(|a| Some((a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32))),
        modo: match v.get("mode").and_then(Value::as_str) {
            Some("inside") => ModoEnganche::Dentro,
            _ => ModoEnganche::Orbita,
        },
    })
}

fn enganche_hacia(b: &Enganche) -> Value {
    let mut m = Map::new();
    m.insert("elementId".into(), Value::String(b.elemento.clone()));
    m.insert("focus".into(), Value::from(b.foco as f64));
    m.insert("gap".into(), Value::from(b.hueco as f64));
    match b.punto_fijo {
        Some((x, y)) => m.insert(
            "fixedPoint".into(),
            Value::Array(vec![Value::from(x as f64), Value::from(y as f64)]),
        ),
        None => m.insert("fixedPoint".into(), Value::Null),
    };
    m.insert(
        "mode".into(),
        Value::String(
            match b.modo {
                ModoEnganche::Orbita => "orbit",
                ModoEnganche::Dentro => "inside",
            }
            .into(),
        ),
    );
    Value::Object(m)
}

/// Devuelve los diez campos al JSON, **encima del original**.
///
/// Si aqui no se toco nada —`Extras::vacios()`— no se escribe ninguno. No es
/// una optimizacion: es lo que impide que un elemento que nunca tuvo
/// `negrita` vuelva del PC con diez claves nuevas puestas a falso, y lo que
/// deja intacto un `papel` de una version futura del movil que aqui se leyo
/// como «ninguno».
fn extras_hacia(mapa: &mut Map<String, Value>, x: &Extras) {
    // El recorte va por su cuenta y solo si el JSON no lo trae ya: aqui no se
    // edita, asi que el del original es el bueno —reescribirlo cambiaria
    // `10` por `10.0` en un fichero que nadie toco—. Lo que cubre es el
    // elemento que perdio su original (un duplicado, una copia pegada).
    if let Some(r) = &x.recorte
        && !mapa.contains_key("crop")
    {
        mapa.insert(
            "crop".into(),
            serde_json::json!({
                "x": r.x, "y": r.y, "width": r.ancho, "height": r.alto,
                "naturalWidth": r.ancho_natural, "naturalHeight": r.alto_natural,
            }),
        );
    }
    // ...pero si el original SI traia alguno, hay que escribirlos todos
    // aunque aqui esten vacios: si no, quitar aqui el rotulo de dentro de una
    // caja dejaria su `containerId` viejo y el movil lo volveria a ver atado.
    // Es la misma regla que ya obliga a escribir `groupIds` en vacio.
    const CLAVES: [&str; 10] = [
        "presionFirme",
        "negrita",
        "cursiva",
        "tachado",
        "containerId",
        "startBinding",
        "endBinding",
        "boundElements",
        "papel",
        "pauta",
    ];
    if x.vacios() && !CLAVES.iter().any(|c| mapa.contains_key(*c)) {
        return;
    }
    mapa.insert("presionFirme".into(), Value::Bool(x.presion_firme));
    mapa.insert("negrita".into(), Value::Bool(x.negrita));
    mapa.insert("cursiva".into(), Value::Bool(x.cursiva));
    mapa.insert("tachado".into(), Value::Bool(x.tachado));
    mapa.insert(
        "containerId".into(),
        match &x.contenedor {
            Some(c) => Value::String(c.clone()),
            None => Value::Null,
        },
    );
    for (clave, b) in [
        ("startBinding", &x.enganche_inicio),
        ("endBinding", &x.enganche_fin),
    ] {
        mapa.insert(
            clave.into(),
            match b {
                Some(b) => enganche_hacia(b),
                None => Value::Null,
            },
        );
    }
    mapa.insert(
        "boundElements".into(),
        Value::Array(
            x.atados
                .iter()
                .map(|a| {
                    let mut m = Map::new();
                    m.insert("id".into(), Value::String(a.id.clone()));
                    m.insert("type".into(), Value::String(a.tipo.clone()));
                    Value::Object(m)
                })
                .collect(),
        ),
    );
    // La misma regla que el `fillStyle` y el `material`: un tamano de papel
    // de una version futura del movil se lee aqui como «ninguno», asi que
    // reescribirlo lo destruiria. Mientras no se elija uno aqui, vuelve tal
    // cual.
    let papel_ajeno = mapa
        .get("papel")
        .and_then(Value::as_str)
        .is_some_and(|p| TamanoPapel::desde_palabra(p).is_none());
    if !(papel_ajeno && x.papel.is_none()) {
        mapa.insert(
            "papel".into(),
            match x.papel {
                Some(p) => Value::String(p.palabra().into()),
                None => Value::Null,
            },
        );
    }
    let pauta_ajena = mapa
        .get("pauta")
        .and_then(Value::as_str)
        .is_some_and(|p| PautaHoja::desde_palabra(p).is_none());
    if !(pauta_ajena && x.pauta == PautaHoja::default()) {
        mapa.insert("pauta".into(), Value::String(x.pauta.palabra().into()));
    }
}

/// Devuelve el elemento al JSON, encima del original.
///
/// Encima y no de cero: el original trae campos que no usamos —`groupIds`,
/// `boundElements`, `link`, `frameId`— y que atan unos elementos con otros.
/// Escribir solo lo que entendemos desharia esas ataduras en silencio.
/// El `width` y el `height` con que sale un elemento.
///
/// **En un trazo, una linea o una flecha son la caja de sus puntos**, como
/// los deja el movil al trazar (`Element.withPoint`: `boundsOfPoints`). Aqui
/// el modelo no los lleva al dia —`ancho`/`alto` de lo que nace aqui son
/// 0— y salian a 0: el movil pinta todo lo que mide menos de dos pixeles
/// como la raya de su primer punto al ultimo (`Renderer.sePierdeDePequeno`
/// y `pintarComoRaya`), asi que lo anotado en el PC se veia alli como
/// rayas rectas de un pixel (queja del 28-sep-2026). Lo demas lleva su caja.
fn caja_escrita(e: &Elemento) -> (f64, f64) {
    let puntos = match &e.figura {
        Figura::Lapiz { puntos, .. }
        | Figura::Resaltador { puntos }
        | Figura::Linea { puntos }
        | Figura::Flecha { puntos, .. } => puntos,
        _ => return (e.ancho as f64, e.alto as f64),
    };
    // Con los mismos numeros que se escriben en `points` (relativos), para
    // que la caja sea exactamente la de lo escrito.
    let rel = |p: &Punto2| ((p.x - e.x) as f64, (p.y - e.y) as f64);
    let Some(primero) = puntos.first().map(rel) else {
        return (0.0, 0.0);
    };
    let (mut x1, mut y1, mut x2, mut y2) = (primero.0, primero.1, primero.0, primero.1);
    for (x, y) in puntos.iter().map(rel) {
        (x1, y1, x2, y2) = (x1.min(x), y1.min(y), x2.max(x), y2.max(y));
    }
    (x2 - x1, y2 - y1)
}

fn elemento_hacia(e: &Elemento, original: &Value, objetos: bool) -> Value {
    // La forma de los puntos la manda el propio elemento si ya los traia; el
    // aviso del lienzo solo decide para los que nacen aqui.
    let objetos = if original.get("points").is_some() {
        puntos_son_objetos(original)
    } else {
        objetos
    };
    let mut mapa = match original {
        Value::Object(m) => m.clone(),
        _ => Map::new(),
    };
    mapa.insert("x".into(), Value::from(e.x as f64));
    mapa.insert("y".into(), Value::from(e.y as f64));
    // Solo lo que aqui lleva la caja a cero (lo nacido en el PC) la saca de
    // sus puntos; una caja que ya tiene medida (la del movil) no se toca:
    // mover una flecha del movil no puede cambiarle el alto que el escribio.
    let (ancho, alto) = if e.ancho == 0.0 && e.alto == 0.0 {
        caja_escrita(e)
    } else {
        (e.ancho as f64, e.alto as f64)
    };
    mapa.insert("width".into(), Value::from(ancho));
    mapa.insert("height".into(), Value::from(alto));
    mapa.insert("angle".into(), Value::from(e.angulo as f64));
    mapa.insert("strokeColor".into(), Value::String(color_hacia(e.trazo)));
    mapa.insert(
        "backgroundColor".into(),
        Value::String(match e.relleno {
            Some(c) => color_hacia(c),
            None => "transparent".into(),
        }),
    );
    // Un `fillStyle` que no entendemos se lee como el de por omision, asi que
    // reescribirlo lo destruiria: un estilo de una version futura del movil
    // entraria como «hachure» y saldria como «hachure» para siempre. Mientras
    // el estilo siga siendo el de por omision -o sea, mientras aqui no se haya
    // tocado- se devuelve el original tal cual, la misma regla que ya protege
    // a la escala y a los elementos ajenos.
    let sin_entender = mapa
        .get("fillStyle")
        .and_then(Value::as_str)
        .is_some_and(|s| !palabra_de_relleno_conocida(s));
    if !(sin_entender && e.estilo_relleno == EstiloRelleno::default()) {
        mapa.insert(
            "fillStyle".into(),
            Value::String(estilo_relleno_hacia(e.estilo_relleno).into()),
        );
    }
    // La misma regla que el `fillStyle`: un material de una version futura
    // del movil se lee como lisa, asi que reescribirlo lo destruiria —
    // entraria como «acuarela» y saldria como «lisa» para siempre—. Mientras
    // el material siga siendo el de por omision, o sea mientras aqui no se
    // haya tocado, se devuelve el original tal cual.
    let material_ajeno = mapa
        .get("material")
        .and_then(Value::as_str)
        .is_some_and(|s| MaterialTinta::desde_palabra(s).is_none());
    if !(material_ajeno && e.material == MaterialTinta::default()) {
        mapa.insert(
            "material".into(),
            Value::String(e.material.palabra().into()),
        );
    }
    dureza_hacia(&mut mapa, e.extras.dureza);
    crate::geometria_exacta::laminas_hacia(&mut mapa, e);
    // **La guia que nace aqui** (modo guia): sin la clave el movil la leeria
    // como dibujo. Se escribe si lo es o si el original ya la traia (una que
    // deja de serlo tiene que decirlo); en lo demas, nada nuevo.
    if e.extras.referencia || mapa.contains_key("reference") {
        mapa.insert("reference".into(), Value::Bool(e.extras.referencia));
    }
    mapa.insert("strokeWidth".into(), Value::from(e.grosor as f64));
    mapa.insert(
        "strokeStyle".into(),
        Value::String(
            match e.estilo {
                EstiloTrazo::Solido => "solid",
                EstiloTrazo::Discontinuo => "dashed",
                EstiloTrazo::Punteado => "dotted",
                // El movil no lo conoce: lo mas parecido que si entiende, y
                // la verdad en `trazoRayaPunto` (abajo).
                EstiloTrazo::RayaPunto => "dashed",
            }
            .into(),
        ),
    );
    // El movil lo lee como entero, y un `1.0` le hace rechazar el fichero.
    // Solo se escribe con decimales si de verdad los tiene.
    mapa.insert(
        "roughness".into(),
        if e.rugosidad.fract() == 0.0 {
            Value::from(e.rugosidad as i64)
        } else {
            Value::from(e.rugosidad as f64)
        },
    );
    // **El resaltador viaja como el del movil**: un freedraw al 40 %
    // (`HIGHLIGHTER_OPACITY`) con el grosor ya engordado. Iba con la opacidad
    // del elemento (100) y alli se veia un lapiz opaco.
    let opacidad = if matches!(e.figura, Figura::Resaltador { .. }) {
        e.opacidad * crate::tinta::OPACIDAD_DEL_RESALTADOR
    } else {
        e.opacidad
    };
    mapa.insert(
        "opacity".into(),
        Value::from((opacidad * 100.0).round() as i64),
    );
    // Un `Int` del movil: no le cabe el bit de arriba de nuestro u32.
    mapa.insert("seed".into(), Value::from(e.semilla & 0x7fff_ffff));
    sellar(&mut mapa, e, original);
    // **`roundness` se leia y no se escribia nunca**, asi que un recuadro
    // redondeado nacido en el escritorio llegaba al movil con las esquinas en
    // punta. Es un objeto y no un booleano porque Excalidraw guarda ahi QUE
    // algoritmo de redondeo usar: el 3 es el de radio fijo, que es el que
    // pone Excalidraw para las figuras cerradas de hoy. Solo se escribe donde
    // el movil lo escribe —rectangulo y linea—; en lo demas, si el original
    // traia uno, se respeta y si no, no se inventa.
    if matches!(e.figura, Figura::Rectangulo | Figura::Linea { .. }) {
        if e.redondo {
            if mapa.get("roundness").is_none_or(Value::is_null) {
                let mut r = Map::new();
                r.insert("type".into(), Value::from(3));
                mapa.insert("roundness".into(), Value::Object(r));
            }
        } else {
            mapa.insert("roundness".into(), Value::Null);
        }
    }
    // **La flecha curva** (`roundness` en una flecha = tipo «round» de
    // Excalidraw, `FormaDeFlecha.CURVA` del movil). Va con el 2, el radio
    // proporcional, que es el que ponen los dos al elegir «curva»; si el
    // original ya traia otro —el 3 de fabrica del movil— se respeta, porque
    // para una flecha cualquier `roundness` quiere decir lo mismo. La de
    // codos nunca lo lleva: el movil la pasa a `null` al elegirla.
    if let Figura::Flecha { codos, .. } = &e.figura {
        if e.redondo && !*codos {
            if mapa.get("roundness").is_none_or(Value::is_null) {
                let mut r = Map::new();
                r.insert("type".into(), Value::from(2));
                mapa.insert("roundness".into(), Value::Object(r));
            }
        } else {
            mapa.insert("roundness".into(), Value::Null);
        }
    }
    mapa.insert("isDeleted".into(), Value::Bool(e.borrado));
    mapa.insert("locked".into(), Value::Bool(e.bloqueado));
    if let Some(enlace) = &e.enlace {
        mapa.insert("enlace".into(), Value::String(enlace.clone()));
    }
    // Se escribe siempre, tambien vacio: si solo se escribiera cuando hay
    // grupos, desagrupar en Windows dejaria los groupIds viejos del
    // original y el movil los volveria a ver agrupados.
    mapa.insert(
        "groupIds".into(),
        Value::Array(e.grupos.iter().map(|g| Value::String(g.clone())).collect()),
    );
    match &e.figura {
        Figura::Lapiz {
            puntos,
            presiones,
            opciones,
        } => {
            mapa.insert("points".into(), puntos_hacia(puntos, e.x, e.y, objetos));
            mapa.insert(
                "pressures".into(),
                Value::Array(presiones.iter().map(|p| Value::from(*p as f64)).collect()),
            );
            mapa.insert("simulatePressure".into(), Value::Bool(presiones.is_empty()));
            let o = opciones.unwrap_or_default();
            mapa.insert(
                "strokeOptions".into(),
                serde_json::to_value(o).unwrap_or(Value::Null),
            );
            // Un trazo legado se exporta con su grosor ya convertido, para
            // que Excalidraw y el movil lo vean del mismo ancho que aqui.
            if opciones.is_none() {
                mapa.insert(
                    "strokeWidth".into(),
                    Value::from((e.grosor / crate::tinta::FACTOR_VARIABLE) as f64),
                );
            }
        }
        Figura::Resaltador { puntos } => {
            // Como el lapiz del movil con presion simulada: sin esto alli se
            // leia un freedraw sin tinta de Excalidraw.
            mapa.insert("points".into(), puntos_hacia(puntos, e.x, e.y, objetos));
            mapa.insert("pressures".into(), Value::Array(Vec::new()));
            mapa.insert("simulatePressure".into(), Value::Bool(true));
        }
        Figura::Linea { puntos } => {
            mapa.insert("points".into(), puntos_hacia(puntos, e.x, e.y, objetos));
        }
        Figura::Flecha {
            puntos,
            punta_inicio,
            punta_fin,
            codos,
        } => {
            mapa.insert("points".into(), puntos_hacia(puntos, e.x, e.y, objetos));
            // **Las puntas no se escribian.** Una flecha nacida aqui llegaba
            // al movil sin `endArrowhead`, y alli eso significa «con punta»
            // por omision: acertaba de casualidad en el caso normal y mentia
            // en todos los demas —una flecha sin punta salia con punta, y las
            // siete puntas que no son la de siempre se perdian en el viaje—.
            punta_hacia_el_mapa(&mut mapa, "startArrowhead", *punta_inicio);
            punta_hacia_el_mapa(&mut mapa, "endArrowhead", *punta_fin);
            mapa.insert("elbowed".into(), Value::Bool(*codos));
        }
        Figura::Texto {
            texto,
            tam,
            familia,
        } => {
            mapa.insert("text".into(), Value::String(texto.clone()));
            mapa.insert("fontSize".into(), Value::from(*tam as f64));
            // **`fontFamily` es un numero alli y un nombre aqui.** Sin
            // escribirlo, el movil reabria todo texto del PC con la letra que
            // tuviera puesta de fabrica, que no es la que se ve en esta
            // pantalla. Se traduce con la tabla de `texto.rs`, que conoce los
            // tres numeros del fichero y los tres alias viejos.
            mapa.insert(
                "fontFamily".into(),
                Value::from(crate::texto::numero_de_familia(familia)),
            );
            // La alineacion, solo si aqui se sabe cual es: sin ella (`None`)
            // el original se queda como vino, traiga lo que traiga. Las dos
            // palabras son las del `enum` de kotlinx del movil, que rechaza
            // el fichero con cualquier otra.
            if let Some(a) = e.extras.alineacion {
                mapa.insert("textAlign".into(), Value::String(a.palabra().into()));
            }
            if let Some(v) = e.extras.alineacion_vertical {
                mapa.insert("verticalAlign".into(), Value::String(v.palabra().into()));
            }
            // La fuente rica junto al texto plano de reserva
            // (`docs/texto-rico-del-lienzo.md`). Si se quito aqui, se
            // quita tambien del original.
            match &e.extras.rico {
                Some(r) => {
                    mapa.insert("textoRico".into(), Value::String(r.clone()));
                }
                None => {
                    mapa.remove("textoRico");
                }
            }
        }
        Figura::Emoji { caracter } => {
            // Un texto normal para los demas, con una marca para que aqui
            // vuelva a ser emoji y no un texto que se edita letra a letra.
            mapa.insert("text".into(), Value::String(caracter.clone()));
            mapa.insert("fontSize".into(), Value::from((e.alto * 0.8) as f64));
            mapa.insert(
                "customData".into(),
                serde_json::json!({ "pixpin": "emoji" }),
            );
        }
        Figura::Cota { puntos } => {
            mapa.insert("type".into(), Value::String("pixpin-measure".to_string()));
            mapa.insert("points".into(), puntos_hacia(puntos, e.x, e.y, objetos));
            if let Some(t) = e.extras.tam_letra {
                mapa.insert("fontSize".into(), Value::from(t as f64));
            }
        }
        Figura::EscalaGrafica => {
            mapa.insert("type".into(), Value::String("pixpin-scalebar".to_string()));
        }
        Figura::Instrumento(i) => {
            mapa.insert("type".into(), Value::String(i.tipo().to_string()));
            instrumento_hacia(&mut mapa, i);
            if let Some(t) = e.extras.tam_letra {
                mapa.insert("fontSize".into(), Value::from(t as f64));
            }
        }
        Figura::Marco { nombre } => {
            mapa.insert("type".into(), Value::String("frame".to_string()));
            mapa.insert("name".into(), Value::String(nombre.clone()));
        }
        // El arco sale en RADIANES, que es la unidad de `arcStart`/`arcSweep`
        // en los dos aparatos (ver la lectura). Y `barrido: None` vuelve a
        // ser `null` y no un cero: es la guia sin repasar, y un cero la
        // convertiria en un arco de longitud nula.
        Figura::Arco { inicio, barrido } => {
            mapa.insert("type".into(), Value::String("pixpin-arc".to_string()));
            mapa.insert("arcStart".into(), Value::from(*inicio as f64));
            mapa.insert(
                "arcSweep".into(),
                match barrido {
                    Some(b) => Value::from(*b as f64),
                    None => Value::Null,
                },
            );
        }
        // El numero va en `text`, que es donde lo pone el movil y de donde
        // saca el siguiente de la serie.
        Figura::Serie { numero } => {
            mapa.insert("type".into(), Value::String("pixpin-serial".to_string()));
            mapa.insert("text".into(), Value::String(numero.to_string()));
        }
        Figura::Region { contorno, huecos } => {
            mapa.insert("type".into(), Value::String("pixpin-region".to_string()));
            mapa.insert("points".into(), puntos_hacia(contorno, e.x, e.y, objetos));
            mapa.insert(
                "huecos".into(),
                Value::Array(
                    huecos
                        .iter()
                        .map(|a| puntos_hacia(a, e.x, e.y, objetos))
                        .collect(),
                ),
            );
        }
        Figura::Punto {
            letra,
            angulo,
            radio,
        } => {
            mapa.insert("type".into(), Value::String("pixpin-point".to_string()));
            mapa.insert("text".into(), Value::String(letra.clone()));
            // En radianes, como lo lee `Puntos.sitioDeLaEtiqueta`.
            mapa.insert("etiquetaAngulo".into(), Value::from(*angulo as f64));
            mapa.insert("etiquetaRadio".into(), Value::from(*radio as f64));
            if let Some(t) = e.extras.tam_letra {
                mapa.insert("fontSize".into(), Value::from(t as f64));
            }
        }
        // **`mosaicBlur` se leia y no se escribia.** Un mosaico nacido en el
        // escritorio llegaba al movil sin decir con que tapa, y uno suyo al
        // que aqui se le quitara el desenfoque volvia desenfocado.
        Figura::Mosaico { desenfoque } => {
            mapa.insert("type".into(), Value::String("pixpin-mosaic".to_string()));
            mapa.insert("mosaicBlur".into(), Value::Bool(*desenfoque));
        }
        // La lupa con sus nueve campos, como `Element` del movil. Lo que
        // vale `None` se borra en vez de escribirse a cero (ver
        // `lupa_elemento::escribir`).
        Figura::Lupa { cristal } => {
            mapa.insert("type".into(), Value::String("pixpin-lupa".to_string()));
            // Solo si aqui se cambio algo: una lupa del movil que solo se
            // movio vuelve con sus campos tal como vinieron (5.4 del
            // inventario: lo que viene de alla se escribe encima de lo suyo).
            if crate::lupa_elemento::leer(&Value::Object(mapa.clone())) != *cristal {
                crate::lupa_elemento::escribir(&mut mapa, cristal);
            }
        }
        // El foco, con los mismos campos: su hueco y cuanto oscurece.
        Figura::Foco { cristal } => {
            if crate::lupa_elemento::leer(&Value::Object(mapa.clone())) != *cristal {
                crate::lupa_elemento::escribir(&mut mapa, cristal);
            }
        }
        Figura::Rectangulo | Figura::Rombo | Figura::Elipse | Figura::Imagen { .. } => {}
    }
    // La letra elegida de lo que rotula sin ser un texto, en su `fontFamily`
    // como en el movil (`newElement` la pone en la cota; el panel, en todo lo
    // que ofrece FUENTE).
    if !matches!(e.figura, Figura::Texto { .. })
        && let Some(f) = &e.extras.familia
    {
        mapa.insert(
            "fontFamily".into(),
            Value::from(crate::texto::numero_de_familia(f)),
        );
    }
    extras_hacia(&mut mapa, &e.extras);
    Value::Object(mapa)
}

/// Un numero estable a partir del identificador de texto.
///
/// Los suyos son cadenas y los nuestros numeros. No hace falta que sea
/// reversible —el original vuelve del JSON— pero SI que el mismo texto de
/// siempre el mismo numero: si cambiara entre dos lecturas del mismo
/// fichero, deshacer y seleccionar dejarian de encontrar sus elementos.
/// **Una sola cuenta, y vive en `enlace`.** Aqui habia una copia de la FNV-1a
/// sin la puerta del prefijo `pc`, asi que el id de un elemento nacido en el
/// escritorio y el id al que apuntaba su enganche se calculaban de dos
/// maneras distintas: al reabrir, la flecha buscaba una caja que segun esta
/// cuenta no era esa caja. Dos verdades sobre el mismo texto.
fn id_estable(texto: &str) -> u64 {
    crate::enlace::id_del_fichero(texto)
}

// --- Colores ---

/// Lee un color hexadecimal de Excalidraw.
///
/// `None` para `"transparent"`, que es como dice «sin relleno». Devolver
/// negro transparente en su lugar pintaria una caja invisible que si
/// responde al raton.
pub fn color_desde(v: Option<&Value>) -> Option<ColorRgba> {
    let texto = v?.as_str()?.trim();
    if texto.eq_ignore_ascii_case("transparent") || texto.is_empty() {
        return None;
    }
    let h = texto.strip_prefix('#').unwrap_or(texto);
    let canal = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
    let corto = |i: usize| {
        let c = u8::from_str_radix(h.get(i..i + 1)?, 16).ok()?;
        // El corto duplica el digito: #f0a es #ff00aa, no #f00a00.
        Some(c * 17)
    };
    let (r, g, b, a) = match h.len() {
        3 => (corto(0)?, corto(1)?, corto(2)?, 255),
        6 => (canal(0)?, canal(2)?, canal(4)?, 255),
        8 => (canal(0)?, canal(2)?, canal(4)?, canal(6)?),
        _ => return None,
    };
    Some(ColorRgba {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: a as f32 / 255.0,
    })
}

/// Y al reves. Sin alfa si es opaco: es lo que escribe Excalidraw, y un
/// `#ff0000ff` donde se esperaba `#ff0000` ensucia la comparacion de dos
/// ficheros que deberian ser iguales.
pub fn color_hacia(c: ColorRgba) -> String {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    if c.a >= 1.0 {
        format!("#{:02x}{:02x}{:02x}", byte(c.r), byte(c.g), byte(c.b))
    } else {
        format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            byte(c.r),
            byte(c.g),
            byte(c.b),
            byte(c.a)
        )
    }
}

/// Los ficheros que usa un lienzo: de `id_objeto` a su ruta dentro del
/// proyecto (`imagenes/<id>`).
///
/// Excalidraw guarda las imagenes incrustadas en `files` como `dataURL`;
/// PixPin Android, en cambio, deja ahi una RUTA al fichero de dentro del
/// `.pixpin`, que es lo que hace que un plano de quince megas no se
/// convierta en veinte de base64. Quien pinta las lee con esto y las mete en
/// su almacen con el mismo `id_objeto` que lleva la figura.
pub fn ficheros(lienzo: &Lienzo) -> Vec<(u64, String)> {
    let Some(Value::Object(files)) = lienzo.resto.get("files") else {
        return Vec::new();
    };
    files
        .iter()
        .filter_map(|(id, v)| {
            let ruta = v.get("path")?.as_str()?;
            Some((id_estable(id), ruta.to_string()))
        })
        .collect()
}

/// Apunta en `files` la foto `id` (su `fileId`), guardada en `ruta` dentro
/// del proyecto: lo contrario de [`ficheros`], con la forma del `SceneFile`
/// del movil (`id`, `mimeType`, `path`, `created`). Una entrada que ya
/// estaba no se toca: es la misma foto y su fecha es la de cuando nacio.
pub fn poner_fichero(lienzo: &mut Lienzo, id: &str, tipo: &str, ruta: &str, creado_ms: i64) {
    let files = lienzo
        .resto
        .entry("files")
        .or_insert_with(|| Value::Object(Map::new()));
    if !files.is_object() {
        *files = Value::Object(Map::new());
    }
    let Value::Object(files) = files else {
        return;
    };
    files.entry(id.to_string()).or_insert_with(
        || serde_json::json!({"id": id, "mimeType": tipo, "path": ruta, "created": creado_ms}),
    );
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::medida::Escala;

    /// Un lienzo con un rectangulo nuestro y un cronograma suyo, en ese orden.
    ///
    /// El tipo ajeno de muestra tiene que ser uno que no vayamos a
    /// implementar nunca: `pixpin-measure` sirvio para esto hasta que la
    /// tarea 3 le enseno a `elemento_desde` a entenderlo, y estas mismas
    /// pruebas se pusieron rojas por casualidad, no por ningun fallo. El
    /// cronograma se quito del lienzo (no estaba bien hecho), asi que es el
    /// candidato con menos riesgo de que vuelva a pasar.
    fn lienzo_mixto() -> &'static str {
        r##"{
          "type": "excalidraw",
          "version": 2,
          "source": "pixpin-android",
          "elements": [
            {"id":"a1","type":"rectangle","x":10,"y":20,"width":100,"height":50,
             "strokeColor":"#1e1e1e","backgroundColor":"transparent","strokeWidth":2,
             "strokeStyle":"solid","roughness":1,"opacity":100,"seed":12345,
             "groupIds":["g1"],"boundElements":[{"id":"b1","type":"arrow"}]},
            {"id":"c1","type":"pixpin-gantt","x":0,"y":0,"width":80,"height":10,
             "tareas":[],"periodos":[]}
          ],
          "appState": {"viewBackgroundColor": "#ffffff"},
          "files": {}
        }"##
    }

    #[test]
    fn lo_que_entendemos_se_traduce() {
        let l = leer(lienzo_mixto()).unwrap();
        let e = l.elementos();
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].figura, Figura::Rectangulo);
        assert_eq!(
            (e[0].x, e[0].y, e[0].ancho, e[0].alto),
            (10.0, 20.0, 100.0, 50.0)
        );
        assert_eq!(e[0].opacidad, 1.0, "el suyo va de 0 a 100");
        assert_eq!(e[0].relleno, None, "«transparent» es sin relleno");
        assert_eq!(e[0].semilla, 12345);
    }

    #[test]
    fn lo_que_no_entendemos_no_se_pierde() {
        // Es la razon de ser de este modulo. Si esto fallara, abrir un plano
        // en el escritorio y guardarlo le borraria las cotas al usuario.
        let l = leer(lienzo_mixto()).unwrap();
        assert_eq!(l.cuantos_ajenos(), 1);
        let salida = escribir(&l);
        assert!(
            salida.contains("pixpin-gantt"),
            "se perdio el cronograma:\n{salida}"
        );
        assert!(salida.contains("\"tareas\""), "se perdieron sus campos");
        assert!(salida.contains("\"periodos\""));
    }

    #[test]
    fn el_orden_se_respeta() {
        // Lo que va despues tapa a lo que va antes. Separar lo conocido de
        // lo ajeno en dos listas perderia el entrelazado y un mosaico
        // dejaria de tapar lo que tapaba.
        let l = leer(lienzo_mixto()).unwrap();
        assert!(matches!(l.entradas[0], Entrada::Nuestro { .. }));
        assert!(matches!(l.entradas[1], Entrada::Ajeno(_)));
        let salida = escribir(&l);
        let pos_rect = salida.find("rectangle").unwrap();
        let pos_cronograma = salida.find("pixpin-gantt").unwrap();
        assert!(pos_rect < pos_cronograma, "el orden cambio");
    }

    #[test]
    fn los_campos_que_no_usamos_vuelven_intactos() {
        // `groupIds` y `boundElements` atan unos elementos con otros.
        // Escribir solo lo que entendemos las desharia en silencio.
        let salida = escribir(&leer(lienzo_mixto()).unwrap());
        assert!(salida.contains("groupIds"), "se perdieron los grupos");
        assert!(
            salida.contains("boundElements"),
            "se perdieron las ataduras"
        );
        assert!(
            salida.contains("viewBackgroundColor"),
            "se perdio el appState"
        );
    }

    #[test]
    fn los_puntos_pasan_de_relativos_a_absolutos_y_vuelven() {
        // Los suyos van relativos al origen del elemento y los nuestros son
        // absolutos. Sin la suma, un trazo dibujado en el movil aparece
        // pegado a la esquina al abrirlo aqui.
        let json = r#"{"elements":[
          {"id":"t","type":"freedraw","x":100,"y":200,"width":10,"height":10,
           "points":[[0,0],[5,5],[10,10]]}
        ]}"#;
        let l = leer(json).unwrap();
        let Figura::Lapiz { puntos, .. } = &l.elementos()[0].figura else {
            panic!("deberia ser lapiz");
        };
        assert_eq!(puntos[0], Punto2 { x: 100.0, y: 200.0 });
        assert_eq!(puntos[2], Punto2 { x: 110.0, y: 210.0 });
        // Y al escribir vuelven a ser relativos: el primero en el origen.
        let vuelta = leer(&escribir(&l)).unwrap();
        let Figura::Lapiz { puntos: p2, .. } = &vuelta.elementos()[0].figura else {
            panic!("deberia seguir siendo lapiz");
        };
        assert_eq!(p2, puntos, "la ida y vuelta movio el trazo");
    }

    #[test]
    fn un_lienzo_vacio_se_escribe_y_se_vuelve_a_leer() {
        let json = escribir(&Lienzo::vacio());
        let leido = leer(&json).expect("lo que escribimos tiene que poder leerse");
        assert!(leido.entradas.is_empty());
        assert_eq!(leido.cuantos_ajenos(), 0);
        // Y se declara como lo que es, o el movil no lo abrira.
        let mapa: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(mapa["type"], "excalidraw");
        // Caso negativo: sin `elements` no hay lienzo que leer, por bien que
        // este el resto del JSON.
        assert!(leer(r#"{"type":"excalidraw"}"#).is_err());
    }

    #[test]
    fn los_colores_van_y_vuelven() {
        let rojo = color_desde(Some(&Value::String("#ff0000".into()))).unwrap();
        assert_eq!((rojo.r, rojo.g, rojo.b, rojo.a), (1.0, 0.0, 0.0, 1.0));
        assert_eq!(color_hacia(rojo), "#ff0000");
        // El corto duplica el digito: #f0a es #ff00aa.
        let corto = color_desde(Some(&Value::String("#f0a".into()))).unwrap();
        assert_eq!(color_hacia(corto), "#ff00aa");
        // Con alfa, ocho digitos.
        let medio = color_desde(Some(&Value::String("#00ff0080".into()))).unwrap();
        assert_eq!(color_hacia(medio), "#00ff0080");
    }

    #[test]
    fn las_tres_palabras_del_fill_style_se_leen_y_se_escriben() {
        let json = r##"{"type":"excalidraw","elements":[
            {"type":"rectangle","x":0,"y":0,"width":10,"height":10,
             "backgroundColor":"#ffcc00","fillStyle":"cross-hatch","seed":1},
            {"type":"rectangle","x":20,"y":0,"width":10,"height":10,
             "backgroundColor":"#ffcc00","fillStyle":"solid","seed":2},
            {"type":"rectangle","x":40,"y":0,"width":10,"height":10,
             "backgroundColor":"#ffcc00","fillStyle":"hachure","seed":3}
        ]}"##;
        let l = leer(json).unwrap();
        let e = l.elementos();
        assert_eq!(e[0].estilo_relleno, EstiloRelleno::Cruzado);
        assert_eq!(e[1].estilo_relleno, EstiloRelleno::Solido);
        assert_eq!(e[2].estilo_relleno, EstiloRelleno::Rayado);

        let vuelta = escribir(&l);
        assert!(vuelta.contains("\"cross-hatch\""), "{vuelta}");
        assert!(vuelta.contains("\"solid\""));
        assert!(vuelta.contains("\"hachure\""));
        let estilos: Vec<EstiloRelleno> = leer(&vuelta)
            .unwrap()
            .elementos()
            .iter()
            .map(|x| x.estilo_relleno)
            .collect();
        assert_eq!(
            estilos,
            e.iter().map(|x| x.estilo_relleno).collect::<Vec<_>>(),
            "la ida y vuelta cambio algun estilo de relleno"
        );
    }

    #[test]
    fn un_elemento_sin_fill_style_se_lee_rayado_como_en_excalidraw() {
        // Su `currentItemFillStyle` es «hachure»: si aqui cayera en solido, el
        // mismo fichero se veria rayado en el movil y como una mancha aqui.
        let json = r##"{"type":"excalidraw","elements":[
            {"type":"rectangle","x":0,"y":0,"width":10,"height":10,
             "backgroundColor":"#ffcc00","seed":1}
        ]}"##;
        assert_eq!(
            leer(json).unwrap().elementos()[0].estilo_relleno,
            EstiloRelleno::Rayado
        );
    }

    #[test]
    fn un_fill_style_desconocido_no_rompe_la_lectura_ni_se_pierde_al_escribir() {
        // Caso negativo: un estilo de una version futura del movil no puede
        // tumbar el elemento entero -un relleno raro no deja de ser un
        // rectangulo- ni desaparecer al pasar por Windows, que es la misma
        // regla que protege a la escala y a los elementos ajenos.
        let json = r##"{"type":"excalidraw","elements":[
            {"type":"rectangle","x":0,"y":0,"width":10,"height":10,
             "backgroundColor":"#ffcc00","fillStyle":"zigzag-del-futuro","seed":1}
        ]}"##;
        let mut l = leer(json).unwrap();
        assert_eq!(l.cuantos_ajenos(), 0, "sigue siendo un rectangulo nuestro");
        assert_eq!(
            l.elementos()[0].estilo_relleno,
            EstiloRelleno::Rayado,
            "lo que no se entiende cae en el de por omision"
        );
        assert!(
            escribir(&l).contains("zigzag-del-futuro"),
            "el estilo que no entendemos tiene que volver intacto"
        );

        // Pero en cuanto se cambia aqui, manda lo de aqui: si no, el estilo
        // elegido en Windows no llegaria nunca al movil.
        primero_mut(&mut l).estilo_relleno = EstiloRelleno::Solido;
        let vuelta = escribir(&l);
        assert!(!vuelta.contains("zigzag-del-futuro"), "{vuelta}");
        assert_eq!(
            leer(&vuelta).unwrap().elementos()[0].estilo_relleno,
            EstiloRelleno::Solido
        );
    }

    #[test]
    fn un_rombo_del_movil_se_ve_aqui_y_vuelve_siendo_un_rombo() {
        // `diamond` es una de las diez figuras principales de Excalidraw y
        // aqui caia en el carril ajeno: sobrevivia al guardar, pero era
        // invisible en pantalla.
        let json = r##"{"type":"excalidraw","elements":[
            {"id":"d1","type":"diamond","x":10,"y":20,"width":100,"height":60,"seed":7}
        ]}"##;
        let l = leer(json).unwrap();
        assert_eq!(l.cuantos_ajenos(), 0, "ya no es ajeno");
        assert_eq!(l.elementos()[0].figura, Figura::Rombo);
        assert!(
            !crate::pintado::ordenes(&l.elementos()[0]).is_empty(),
            "un rombo que no produce ordenes sigue siendo invisible"
        );
        let vuelta = escribir(&l);
        assert_eq!(leer(&vuelta).unwrap().elementos()[0].figura, Figura::Rombo);
        assert!(vuelta.contains("\"diamond\""), "{vuelta}");
    }

    #[test]
    fn un_mosaico_del_movil_se_pinta_en_vez_de_ensenar_lo_que_tapaba() {
        // El fallo mas feo de todos: el mosaico entraba como ajeno y no se
        // pintaba, asi que el dato que el usuario tapo en el telefono se
        // VEIA en el escritorio.
        let json = r##"{"type":"excalidraw","elements":[
            {"id":"m1","type":"pixpin-mosaic","x":0,"y":0,"width":80,"height":30,
             "mosaicBlur":true,"seed":3}
        ]}"##;
        let l = leer(json).unwrap();
        assert_eq!(l.cuantos_ajenos(), 0);
        let e = &l.elementos()[0];
        assert_eq!(e.figura, Figura::Mosaico { desenfoque: true });
        let ordenes = crate::pintado::ordenes(e);
        // Lo que importa no es que pinte algo, sino que lo que pinte TAPE:
        // una mancha opaca del tamano de la caja.
        let tapa = ordenes.iter().any(|o| match o {
            crate::pintado::Orden::Relleno { puntos, color } => color.a >= 1.0 && puntos.len() == 4,
            _ => false,
        });
        assert!(tapa, "el mosaico no tapo nada: {ordenes:?}");
        // Y vuelve siendo lo que era, con su `mosaicBlur` intacto.
        let vuelta = escribir(&l);
        assert!(vuelta.contains("pixpin-mosaic"), "{vuelta}");
        assert_eq!(
            leer(&vuelta).unwrap().elementos()[0].figura,
            Figura::Mosaico { desenfoque: true }
        );
    }

    #[test]
    fn un_mosaico_medio_transparente_sigue_tapando_del_todo() {
        // Caso negativo: la opacidad del elemento NO se le aplica. Un
        // mosaico al 20 % no es un mosaico discreto, es un dato legible.
        let json = r##"{"type":"excalidraw","elements":[
            {"type":"pixpin-mosaic","x":0,"y":0,"width":80,"height":30,"opacity":20}
        ]}"##;
        let e = leer(json).unwrap().elementos()[0].clone();
        assert_eq!(e.opacidad, 0.2);
        let opaco = crate::pintado::ordenes(&e)
            .iter()
            .any(|o| matches!(o, crate::pintado::Orden::Relleno { color, .. } if color.a >= 1.0));
        assert!(opaco, "el mosaico se pinto translucido");
    }

    #[test]
    fn un_resaltador_llega_al_movil_como_su_marcador_al_cuarenta_por_ciento() {
        let mut l = Lienzo::vacio();
        l.entradas.push(Entrada::Nuestro {
            elemento: Elemento {
                id: 1,
                figura: Figura::Resaltador {
                    puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(50.0, 0.0)],
                },
                grosor: 5.0,
                ..Default::default()
            },
            original: Box::new(Value::Object(Map::new())),
        });
        let v: Value = serde_json::from_str(&escribir(&l)).unwrap();
        let e = &v["elements"][0];
        assert_eq!(e["type"], "freedraw");
        assert_eq!(e["opacity"], 40, "HIGHLIGHTER_OPACITY");
        assert_eq!(e["strokeWidth"].as_f64(), Some(5.0));
        assert_eq!(e["simulatePressure"], true);
        // Caso negativo: un lapiz normal no se apaga al 40.
        let mut l = Lienzo::vacio();
        l.entradas.push(Entrada::Nuestro {
            elemento: Elemento {
                id: 1,
                figura: Figura::Linea {
                    puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(50.0, 0.0)],
                },
                ..Default::default()
            },
            original: Box::new(Value::Object(Map::new())),
        });
        let v: Value = serde_json::from_str(&escribir(&l)).unwrap();
        assert_eq!(v["elements"][0]["opacity"], 100);
    }

    #[test]
    fn la_letra_de_una_cota_y_de_un_numero_viaja_en_su_font_family() {
        // Como en el movil: la cota del telefono en Caveat (104) se lee aqui
        // en Caveat, y un numero de serie en Nunito sale con su 6.
        let json = r##"{"type":"excalidraw","elements":[
            {"id":"c1","type":"pixpin-measure","x":0,"y":0,"width":10,"height":0,
             "points":[[0,0],[10,0]],"fontFamily":104,"seed":1},
            {"id":"s1","type":"pixpin-serial","x":0,"y":0,"width":36,"height":36,
             "text":"3","fontFamily":6,"seed":2}
        ]}"##;
        let l = leer(json).unwrap();
        let familias: Vec<Option<String>> = l
            .elementos()
            .iter()
            .map(|e| e.extras.familia.clone())
            .collect();
        assert_eq!(
            familias,
            [Some("Caveat".to_string()), Some("Nunito".to_string())]
        );
        let v: Value = serde_json::from_str(&escribir(&l)).unwrap();
        assert_eq!(v["elements"][0]["fontFamily"], 104);
        assert_eq!(v["elements"][1]["fontFamily"], 6);
        // Caso negativo: un rectangulo sin letra no estrena `fontFamily`.
        let mut r = Lienzo::vacio();
        r.entradas.push(Entrada::Nuestro {
            elemento: Elemento {
                id: 1,
                figura: Figura::Rectangulo,
                ancho: 5.0,
                alto: 5.0,
                ..Default::default()
            },
            original: Box::new(Value::Object(Map::new())),
        });
        assert!(!escribir(&r).contains("fontFamily"));
    }

    #[test]
    fn un_foco_va_y_vuelve_con_su_tipo_propio_y_no_como_un_rectangulo() {
        // El puente estaba roto en los dos sentidos: lo que salia de aqui
        // llegaba al movil como un rectangulo transparente —dejaba de
        // oscurecer— y lo que venia de alla entraba como ajeno y no se veia.
        let json = r##"{"type":"excalidraw","elements":[
            {"id":"f1","type":"pixpin-spotlight","x":0,"y":0,"width":200,"height":100,
             "oscurecer":60,"lupaRedonda":true,"foco":{"x":100,"y":50},
             "focoAncho":111,"focoAlto":55,"seed":5}
        ]}"##;
        let l = leer(json).unwrap();
        assert_eq!(l.cuantos_ajenos(), 0, "ya no es ajeno");
        // Ahora se lee entero: el hueco, su sitio y cuanto oscurece.
        let Figura::Foco { cristal } = &l.elementos()[0].figura else {
            panic!("{:?}", l.elementos()[0].figura);
        };
        assert!(cristal.redonda);
        assert_eq!(cristal.oscurecer, Some(60));
        assert_eq!(cristal.foco, Some(Punto2::nuevo(100.0, 50.0)));
        assert_eq!(
            (cristal.foco_ancho, cristal.foco_alto),
            (Some(111.0), Some(55.0))
        );
        let vuelta = escribir(&l);
        assert!(vuelta.contains("pixpin-spotlight"), "{vuelta}");
        assert!(vuelta.contains("\"oscurecer\""), "{vuelta}");
        assert!(vuelta.contains("\"focoAncho\""), "{vuelta}");

        // Y uno nacido aqui sale con el tipo bueno, no como «rectangle», y
        // con su hueco escrito para el movil.
        let mut nuevo = Lienzo::vacio();
        nuevo.entradas.push(Entrada::Nuestro {
            elemento: Elemento {
                id: 1,
                figura: Figura::Foco {
                    cristal: crate::lupa_elemento::Cristal {
                        foco_ancho: Some(5.0),
                        foco_alto: Some(5.0),
                        oscurecer: Some(45),
                        ..Default::default()
                    },
                },
                ancho: 10.0,
                alto: 10.0,
                ..Default::default()
            },
            original: Box::new(Value::Object(Map::new())),
        });
        let salida = escribir(&nuevo);
        assert!(salida.contains("pixpin-spotlight"), "{salida}");
        assert!(!salida.contains("\"rectangle\""), "{salida}");
        assert!(salida.contains("\"focoAncho\""), "{salida}");
    }

    #[test]
    fn un_recuadro_redondeado_de_aqui_llega_redondeado_al_movil() {
        // `roundness` se leia y no se escribia nunca: un recuadro redondeado
        // nacido en el escritorio llegaba al telefono con las esquinas en
        // punta.
        let mut l = Lienzo::vacio();
        l.entradas.push(Entrada::Nuestro {
            elemento: Elemento {
                id: 1,
                figura: Figura::Rectangulo,
                ancho: 50.0,
                alto: 50.0,
                redondo: true,
                ..Default::default()
            },
            original: Box::new(Value::Object(Map::new())),
        });
        let salida = escribir(&l);
        assert!(salida.contains("\"roundness\""), "{salida}");
        assert!(leer(&salida).unwrap().elementos()[0].redondo);

        // Caso negativo: quitarle el redondeo aqui tiene que quitarselo alla.
        let json = r##"{"type":"excalidraw","elements":[
            {"id":"r1","type":"rectangle","x":0,"y":0,"width":10,"height":10,
             "roundness":{"type":3},"seed":1}
        ]}"##;
        let mut leido = leer(json).unwrap();
        assert!(leido.elementos()[0].redondo);
        primero_mut(&mut leido).redondo = false;
        let vuelta = escribir(&leido);
        assert!(
            !leer(&vuelta).unwrap().elementos()[0].redondo,
            "sigue redondo: {vuelta}"
        );
    }

    #[test]
    fn un_trazo_de_tiza_del_movil_se_lee_como_tiza_y_vuelve_como_tiza() {
        // La ida y vuelta que decide si el material sirve para algo: lo que
        // el telefono escribio en `material` tiene que sobrevivir a pasar por
        // Windows, palabra por palabra.
        for m in crate::tinta::MATERIALES {
            let json = format!(
                r##"{{"type":"excalidraw","elements":[
                {{"type":"rectangle","x":0,"y":0,"width":10,"height":10,
                 "material":"{}","seed":1}}
            ]}}"##,
                m.palabra()
            );
            let l = leer(&json).unwrap();
            assert_eq!(l.elementos()[0].material, m, "leyendo {m:?}");
            let vuelta = escribir(&l);
            assert_eq!(
                leer(&vuelta).unwrap().elementos()[0].material,
                m,
                "escribiendo {m:?}"
            );
            assert!(
                vuelta.contains(&format!("\"{}\"", m.palabra())),
                "{m:?} no salio con su palabra: {vuelta}"
            );
        }
    }

    #[test]
    fn un_elemento_sin_material_es_de_tinta_lisa() {
        // Todo lo dibujado antes de que el movil inventara el campo: sin
        // `material` la tinta es la de siempre, no un grano sorpresa.
        let json = r##"{"type":"excalidraw","elements":[
            {"type":"rectangle","x":0,"y":0,"width":10,"height":10,"seed":1}
        ]}"##;
        assert_eq!(
            leer(json).unwrap().elementos()[0].material,
            MaterialTinta::Lisa
        );
    }

    #[test]
    fn un_material_desconocido_no_rompe_nada_y_se_conserva_al_guardar() {
        // Caso negativo, y el que mas cuesta cuando se rompe: un material de
        // una version futura del movil no puede tumbar el elemento -un
        // material raro no deja de ser un rectangulo-, se pinta como pluma, y
        // no puede desaparecer al pasar por Windows.
        let json = r##"{"type":"excalidraw","elements":[
            {"type":"rectangle","x":0,"y":0,"width":10,"height":10,
             "material":"acuarela-del-futuro","seed":1}
        ]}"##;
        let mut l = leer(json).unwrap();
        assert_eq!(l.cuantos_ajenos(), 0, "sigue siendo un rectangulo nuestro");
        assert_eq!(
            l.elementos()[0].material,
            MaterialTinta::Lisa,
            "lo que no se entiende se pinta como pluma"
        );
        assert!(
            escribir(&l).contains("acuarela-del-futuro"),
            "el material que no entendemos tiene que volver intacto"
        );

        // Pero en cuanto se elige uno aqui, manda lo de aqui: si no, la tiza
        // elegida en Windows no llegaria nunca al movil.
        primero_mut(&mut l).material = MaterialTinta::Tiza;
        let vuelta = escribir(&l);
        assert!(!vuelta.contains("acuarela-del-futuro"), "{vuelta}");
        assert_eq!(
            leer(&vuelta).unwrap().elementos()[0].material,
            MaterialTinta::Tiza
        );
    }

    #[test]
    fn transparente_es_sin_relleno_y_no_negro_invisible() {
        // Caso negativo: devolver negro con alfa cero pintaria una caja
        // invisible que ademas responde al raton.
        assert_eq!(
            color_desde(Some(&Value::String("transparent".into()))),
            None
        );
        assert_eq!(color_desde(Some(&Value::String("".into()))), None);
        assert_eq!(color_desde(None), None);
        // Y un color mal escrito tampoco inventa nada.
        assert_eq!(color_desde(Some(&Value::String("#zzz".into()))), None);
        assert_eq!(color_desde(Some(&Value::String("#12345".into()))), None);
    }

    #[test]
    fn lo_borrado_no_vuelve_a_la_vida() {
        // Excalidraw deja lo borrado dentro del fichero con `isDeleted`. No
        // es un elemento ajeno que haya que conservar: es basura que el
        // propio formato marca como tal.
        let json = r#"{"elements":[
          {"id":"x","type":"rectangle","x":0,"y":0,"width":1,"height":1,"isDeleted":true}
        ]}"#;
        let l = leer(json).unwrap();
        assert!(l.elementos().is_empty());
    }

    #[test]
    fn el_identificador_de_texto_da_siempre_el_mismo_numero() {
        // Si cambiara entre dos lecturas del mismo fichero, deshacer y
        // seleccionar dejarian de encontrar sus elementos.
        assert_eq!(id_estable("abc123"), id_estable("abc123"));
        assert_ne!(id_estable("abc123"), id_estable("abc124"));
        assert_ne!(id_estable(""), id_estable("a"));
    }

    #[test]
    fn un_json_que_no_es_un_lienzo_se_rechaza() {
        // Caso negativo: sin esto, arrastrar un fichero cualquiera daria un
        // lienzo vacio y pareceria que el dibujo se perdio.
        assert!(matches!(leer("no soy json"), Err(ErrorExcalidraw::Json(_))));
        assert!(matches!(
            leer("[1,2,3]"),
            Err(ErrorExcalidraw::SinElementos)
        ));
        assert!(matches!(
            leer(r#"{"type":"excalidraw"}"#),
            Err(ErrorExcalidraw::SinElementos)
        ));
        // Pero un lienzo vacio SI es valido: es un dibujo sin nada.
        assert_eq!(leer(r#"{"elements":[]}"#).unwrap().entradas.len(), 0);
    }

    #[test]
    fn la_flecha_conserva_sus_puntas() {
        let json = r#"{"elements":[
          {"id":"f","type":"arrow","x":0,"y":0,"width":10,"height":0,
           "points":[[0,0],[10,0]],"startArrowhead":null,"endArrowhead":"arrow"}
        ]}"#;
        let l = leer(json).unwrap();
        let Figura::Flecha {
            punta_inicio,
            punta_fin,
            ..
        } = &l.elementos()[0].figura
        else {
            panic!("deberia ser flecha");
        };
        assert_eq!(
            *punta_inicio,
            TipoPunta::Ninguna,
            "un `startArrowhead` nulo es no llevar punta al principio"
        );
        assert_eq!(*punta_fin, TipoPunta::Flecha);
    }

    /// **Toca el primer elemento para que se vuelva a escribir de verdad.**
    ///
    /// Sin esto, una prueba de ida y vuelta no prueba nada: lo que nadie toco
    /// sale TAL CUAL entro (ver `escribir`), asi que el campo que se mira lo
    /// habria escrito el fichero de entrada y no nosotros. Un pixel a la
    /// derecha es el cambio mas pequeno que obliga a pasar por
    /// `elemento_hacia`.
    fn movido(l: &mut Lienzo) {
        primero_mut(l).x += 1.0;
    }

    /// **Las ocho puntas, de ida y de vuelta, una por una.**
    ///
    /// Es la prueba que faltaba: las puntas se leian a medias —«lleva o no
    /// lleva»— y **no se escribian nunca**, asi que una flecha del PC llegaba
    /// al movil sin `endArrowhead` declarado. Alli eso significa «con punta»,
    /// con lo que una flecha a la que se le habia quitado la punta la
    /// recuperaba sola, y las siete que no son la de siempre se perdian.
    #[test]
    fn las_ocho_puntas_van_y_vuelven_con_su_palabra() {
        for punta in crate::formas::PUNTAS {
            let palabra = punta.palabra().expect("las ocho tienen palabra");
            let json = format!(
                r#"{{"elements":[
                  {{"id":"f","type":"arrow","x":0,"y":0,"width":10,"height":0,
                   "points":[[0,0],[10,0]],
                   "startArrowhead":"{palabra}","endArrowhead":"{palabra}"}}
                ]}}"#
            );
            let l = leer(&json).unwrap();
            let Figura::Flecha {
                punta_inicio,
                punta_fin,
                ..
            } = &l.elementos()[0].figura
            else {
                panic!("deberia ser flecha");
            };
            assert_eq!((*punta_inicio, *punta_fin), (punta, punta), "al leer");

            let mut l = l;
            movido(&mut l);
            let vuelta: Value = serde_json::from_str(&escribir(&l)).unwrap();
            let e = &vuelta["elements"][0];
            assert_eq!(e["startArrowhead"], Value::String(palabra.into()));
            assert_eq!(e["endArrowhead"], Value::String(palabra.into()));
        }
    }

    #[test]
    fn una_flecha_sin_punta_se_escribe_con_la_punta_a_nulo_y_no_ausente() {
        // Caso negativo, y el que mas se nota: dejar el campo fuera NO es
        // decir «sin punta». En Excalidraw el campo ausente significa lo
        // contrario, asi que la punta volveria sola.
        let json = r#"{"elements":[
          {"id":"f","type":"arrow","x":0,"y":0,"width":10,"height":0,
           "points":[[0,0],[10,0]],"endArrowhead":null}
        ]}"#;
        let l = leer(json).unwrap();
        let mut l = l;
        movido(&mut l);
        let vuelta: Value = serde_json::from_str(&escribir(&l)).unwrap();
        assert_eq!(vuelta["elements"][0]["endArrowhead"], Value::Null);
        assert_eq!(vuelta["elements"][0]["startArrowhead"], Value::Null);
    }

    #[test]
    fn una_punta_que_no_conocemos_no_se_borra_al_guardar() {
        // La misma regla del `fillStyle` y del `material`: entra como
        // ninguna, y como aqui nadie la toco, sale como entro.
        let json = r#"{"elements":[
          {"id":"f","type":"arrow","x":0,"y":0,"width":10,"height":0,
           "points":[[0,0],[10,0]],"endArrowhead":"punta-del-futuro"}
        ]}"#;
        let l = leer(json).unwrap();
        let mut l = l;
        movido(&mut l);
        let vuelta: Value = serde_json::from_str(&escribir(&l)).unwrap();
        assert_eq!(
            vuelta["elements"][0]["endArrowhead"], "punta-del-futuro",
            "una punta de una version futura se perdio en el viaje"
        );
    }

    #[test]
    fn la_flecha_de_codos_va_y_vuelve() {
        let json = r#"{"elements":[
          {"id":"f","type":"arrow","x":0,"y":0,"width":10,"height":10,
           "points":[[0,0],[10,10]],"elbowed":true}
        ]}"#;
        let l = leer(json).unwrap();
        let Figura::Flecha { codos, .. } = &l.elementos()[0].figura else {
            panic!("deberia ser flecha");
        };
        assert!(*codos, "el conector ortogonal entro como flecha recta");
        let mut l = l;
        movido(&mut l);
        let vuelta: Value = serde_json::from_str(&escribir(&l)).unwrap();
        assert_eq!(vuelta["elements"][0]["elbowed"], true);
        // Caso negativo: una flecha normal sale diciendo que NO es de codos,
        // y no callandose.
        let mut recta = leer(
            r#"{"elements":[{"id":"g","type":"arrow","x":0,"y":0,"width":10,"height":0,
                "points":[[0,0],[10,0]]}]}"#,
        )
        .unwrap();
        movido(&mut recta);
        let vuelta: Value = serde_json::from_str(&escribir(&recta)).unwrap();
        assert_eq!(vuelta["elements"][0]["elbowed"], false);
    }

    /// **La flecha curva**: una flecha con `roundness` es la «round» de
    /// Excalidraw y la CURVA del movil. Se leia (el campo `redondo`) pero no
    /// se escribia en flechas, asi que curvarla aqui no llegaba alla.
    #[test]
    fn la_flecha_curva_va_y_vuelve_por_su_roundness() {
        let json = r#"{"elements":[
          {"id":"f","type":"arrow","x":0,"y":0,"width":10,"height":10,
           "points":[[0,0],[5,8],[10,10]],"roundness":{"type":3}}
        ]}"#;
        let mut l = leer(json).unwrap();
        assert!(l.elementos()[0].redondo, "la curva del movil entro recta");
        movido(&mut l);
        let vuelta: Value = serde_json::from_str(&escribir(&l)).unwrap();
        // El tipo que traia se respeta: para una flecha, cualquiera es curva.
        assert_eq!(vuelta["elements"][0]["roundness"]["type"], 3);

        // Una recta curvada aqui sale con el tipo 2 de Excalidraw, entero.
        let mut recta = leer(
            r#"{"elements":[{"id":"g","type":"arrow","x":0,"y":0,"width":10,"height":0,
                "points":[[0,0],[10,0]]}]}"#,
        )
        .unwrap();
        primero_mut(&mut recta).redondo = true;
        let vuelta: Value = serde_json::from_str(&escribir(&recta)).unwrap();
        assert_eq!(
            vuelta["elements"][0]["roundness"],
            serde_json::json!({"type": 2})
        );
        assert!(vuelta["elements"][0]["roundness"]["type"].is_i64());

        // Caso negativo: enderezarla la deja sin `roundness`, o el movil la
        // seguiria viendo curva; y una de codos nunca lo lleva.
        primero_mut(&mut recta).redondo = false;
        let vuelta: Value = serde_json::from_str(&escribir(&recta)).unwrap();
        assert!(vuelta["elements"][0]["roundness"].is_null());
        let e = primero_mut(&mut recta);
        e.redondo = true;
        if let Figura::Flecha { codos, .. } = &mut e.figura {
            *codos = true;
        }
        let vuelta: Value = serde_json::from_str(&escribir(&recta)).unwrap();
        assert!(vuelta["elements"][0]["roundness"].is_null());
    }

    #[test]
    fn la_alineacion_de_un_texto_va_y_vuelve_con_sus_palabras() {
        use crate::texto::{AlineacionTexto, AlineacionVertical};
        let json = r#"{"elements":[
          {"id":"t","type":"text","x":0,"y":0,"width":50,"height":20,"text":"a",
           "fontSize":20,"fontFamily":5,"textAlign":"center","verticalAlign":"bottom"}
        ]}"#;
        let mut l = leer(json).unwrap();
        let e = &l.elementos()[0];
        assert_eq!(e.extras.alineacion, Some(AlineacionTexto::Centro));
        assert_eq!(
            e.extras.alineacion_vertical,
            Some(AlineacionVertical::Abajo)
        );
        // Caso negativo: la alineacion no son extras del movil; un texto
        // alineado no estrena `negrita`, `papel` y compania al guardarse.
        assert!(e.extras.vacios());
        let t = primero_mut(&mut l);
        t.extras.alineacion = Some(AlineacionTexto::Derecha);
        t.extras.alineacion_vertical = Some(AlineacionVertical::Medio);
        let vuelta: Value = serde_json::from_str(&escribir(&l)).unwrap();
        let v = &vuelta["elements"][0];
        assert_eq!(v["textAlign"], "right");
        assert_eq!(v["verticalAlign"], "middle");
        assert!(v.get("negrita").is_none(), "{v}");
    }

    #[test]
    fn una_alineacion_que_no_conocemos_no_se_borra_al_guardar() {
        let json = r#"{"elements":[
          {"id":"t","type":"text","x":0,"y":0,"width":50,"height":20,"text":"a",
           "fontSize":20,"textAlign":"justify"}
        ]}"#;
        let mut l = leer(json).unwrap();
        assert_eq!(l.elementos()[0].extras.alineacion, None);
        movido(&mut l);
        let vuelta: Value = serde_json::from_str(&escribir(&l)).unwrap();
        assert_eq!(vuelta["elements"][0]["textAlign"], "justify");
        // Caso negativo: un texto que nunca la trajo no la estrena.
        let mut sin = leer(
            r#"{"elements":[{"id":"u","type":"text","x":0,"y":0,"width":9,"height":9,
                "text":"b","fontSize":20}]}"#,
        )
        .unwrap();
        movido(&mut sin);
        let vuelta: Value = serde_json::from_str(&escribir(&sin)).unwrap();
        assert!(vuelta["elements"][0].get("textAlign").is_none());
        assert!(vuelta["elements"][0].get("verticalAlign").is_none());
    }

    /// **`fontFamily` ni se leia ni se escribia.**
    ///
    /// Todo texto del movil entraba aqui como «Segoe UI» dijera lo que dijera
    /// el fichero, y salia con ese nombre escrito donde el movil espera uno de
    /// sus tres numeros: al reabrirlo alla, la letra habia cambiado.
    #[test]
    fn la_letra_de_un_texto_va_y_vuelve_por_su_numero() {
        for (numero, nombre) in [
            (crate::texto::FUENTE_EXCALIFONT, "Excalifont"),
            (crate::texto::FUENTE_NUNITO, "Nunito"),
            (crate::texto::FUENTE_COMIC_SHANNS, "Comic Shanns"),
        ] {
            let json = format!(
                r#"{{"elements":[
                  {{"id":"t","type":"text","x":0,"y":0,"width":50,"height":20,
                   "text":"hola","fontSize":20,"fontFamily":{numero}}}
                ]}}"#
            );
            let l = leer(&json).unwrap();
            let Figura::Texto { familia, .. } = &l.elementos()[0].figura else {
                panic!("deberia ser texto");
            };
            assert_eq!(familia, nombre, "al leer la familia {numero}");
            let mut l = l;
            movido(&mut l);
            let vuelta: Value = serde_json::from_str(&escribir(&l)).unwrap();
            assert_eq!(vuelta["elements"][0]["fontFamily"], numero);
        }
    }

    #[test]
    fn los_numeros_de_letra_viejos_no_caen_a_la_de_fabrica() {
        // Caso negativo: 1, 2 y 3 son los alias de los dibujos de antes. Un
        // croquis guardado con ellos tiene que seguir viendose con SU letra,
        // no con la de por omision.
        let de = |n: u32| {
            let json = format!(
                r#"{{"elements":[{{"id":"t","type":"text","x":0,"y":0,"width":50,"height":20,
                   "text":"hola","fontFamily":{n}}}]}}"#
            );
            match &leer(&json).unwrap().elementos()[0].figura {
                Figura::Texto { familia, .. } => familia.clone(),
                f => panic!("deberia ser texto: {f:?}"),
            }
        };
        assert_eq!(de(2), "Nunito", "el alias viejo de la normal");
        assert_eq!(de(3), "Comic Shanns", "el alias viejo de la monoespaciada");
    }

    #[test]
    fn las_dos_tramas_nuevas_van_y_vuelven_con_su_palabra() {
        for (palabra, estilo) in [
            ("zigzag", EstiloRelleno::Zigzag),
            ("pixpin-lines", EstiloRelleno::LineasPixpin),
        ] {
            let json = format!(
                r##"{{"elements":[
                  {{"id":"r","type":"rectangle","x":0,"y":0,"width":10,"height":10,
                   "backgroundColor":"#ffcc00","fillStyle":"{palabra}","seed":1}}
                ]}}"##
            );
            let l = leer(&json).unwrap();
            assert_eq!(l.elementos()[0].estilo_relleno, estilo, "al leer {palabra}");
            let mut l = l;
            movido(&mut l);
            let vuelta: Value = serde_json::from_str(&escribir(&l)).unwrap();
            assert_eq!(vuelta["elements"][0]["fillStyle"], palabra);
        }
    }

    /// **Un enganche nacido AQUI sobrevive a guardar y reabrir.**
    ///
    /// Es el fallo que dejo apuntado el grupo B en la cabecera de `enlace.rs`.
    /// El elemento estrenaba un id de texto con la hora dentro en cada
    /// guardado, asi que al reabrir el enganche apuntaba a un id que ya no
    /// existia: la caja se movia y la flecha se quedaba donde estaba, sin que
    /// nada avisara.
    #[test]
    fn un_enganche_hecho_en_el_pc_sigue_atado_despues_de_guardar_y_reabrir() {
        use crate::elemento::Enganche;

        // **Las DOS nacen aqui**, y eso es lo que hace de esto una prueba:
        // una caja que viniera del movil ya trae su id en el fichero y
        // sobrevivia de antes. La que se rompia era esta.
        let mut l = leer(
            r#"{"elements":[{"id":"x","type":"rectangle","x":0,"y":0,
            "width":10,"height":10}]}"#,
        )
        .unwrap();
        let caja = 77;
        l.entradas.push(Entrada::Nuestro {
            elemento: Elemento {
                id: caja,
                figura: Figura::Rectangulo,
                x: 0.0,
                y: 0.0,
                ancho: 80.0,
                alto: 40.0,
                ..Default::default()
            },
            original: Box::new(Value::Object(Map::new())),
        });
        let flecha = Elemento {
            id: 4242,
            figura: Figura::Flecha {
                puntos: vec![Punto2::nuevo(200.0, 20.0), Punto2::nuevo(90.0, 20.0)],
                punta_inicio: TipoPunta::Ninguna,
                punta_fin: TipoPunta::Flecha,
                codos: false,
            },
            extras: crate::elemento::Extras {
                enganche_fin: Some(Enganche {
                    elemento: crate::enlace::id_de_texto(caja),
                    foco: 0.0,
                    hueco: 1.0,
                    punto_fijo: None,
                    modo: Default::default(),
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        l.entradas.push(Entrada::Nuestro {
            elemento: flecha,
            original: Box::new(Value::Object(Map::new())),
        });

        // Dos vueltas, no una: con una sola, un id que cambia en cada
        // guardado podria acertar de casualidad la primera vez.
        let mut texto = escribir(&l);
        for vuelta in 0..2 {
            let releido = leer(&texto).unwrap();
            let elementos = releido.elementos();
            let flecha = elementos
                .iter()
                .find(|e| matches!(e.figura, Figura::Flecha { .. }))
                .expect("la flecha sigue ahi");
            let atada = flecha.extras.enganche_fin.as_ref().expect("y su enganche");
            assert!(
                crate::enlace::resolver(&elementos, atada).is_some(),
                "vuelta {vuelta}: el enganche apunta a un id que ya no existe"
            );
            texto = escribir(&releido);
        }
    }

    #[test]
    fn el_id_de_un_elemento_del_movil_no_se_reescribe_nunca() {
        // Caso negativo, y la regla de oro: lo que viene del movil se edita
        // partiendo de su JSON. Si al mover una caja le cambiara el id, el
        // telefono la veria como una caja NUEVA y se quedaria con las dos.
        let mut l = leer(
            r#"{"elements":[{"id":"aBcDeF123","type":"rectangle","x":0,"y":0,
            "width":80,"height":40}]}"#,
        )
        .unwrap();
        movido(&mut l);
        let vuelta: Value = serde_json::from_str(&escribir(&l)).unwrap();
        assert_eq!(vuelta["elements"][0]["id"], "aBcDeF123");
    }

    /// El primer elemento nuestro del lienzo, para poder tocarlo.
    ///
    /// Va por `entradas` y no por `elementos()`, que devuelve clones y no
    /// serviria para modificar nada.
    fn primero_mut(l: &mut Lienzo) -> &mut Elemento {
        l.entradas
            .iter_mut()
            .find_map(|e| match e {
                Entrada::Nuestro { elemento, .. } => Some(elemento),
                Entrada::Ajeno(_) => None,
            })
            .expect("el lienzo trae al menos un elemento nuestro")
    }

    #[test]
    fn una_imagen_del_movil_apunta_a_su_fichero_del_paquete() {
        // El movil no incrusta la imagen en el JSON: deja una ruta dentro
        // del `.pixpin`. Sin leerla, una hoja que es una foto con trazos
        // encima salia en blanco con los trazos flotando.
        let texto = r##"{"type":"excalidraw","elements":[
            {"id":"i1","type":"image","x":0,"y":0,"width":1600,"height":1108,
             "fileId":"7xJoKC","strokeColor":"#1e1e1e"}
          ],
          "files":{"7xJoKC":{"id":"7xJoKC","mimeType":"image/png","path":"imagenes/7xJoKC"}}}"##;
        let lienzo = leer(texto).unwrap();
        let elementos = lienzo.elementos();
        assert_eq!(elementos.len(), 1, "la imagen no se leyo");
        let Figura::Imagen { id_objeto } = elementos[0].figura else {
            panic!("no es una imagen: {:?}", elementos[0].figura)
        };
        let ficheros = ficheros(&lienzo);
        assert_eq!(ficheros.len(), 1);
        assert_eq!(
            ficheros[0].0, id_objeto,
            "el id del fichero y el de la figura tienen que ser el mismo"
        );
        assert_eq!(ficheros[0].1, "imagenes/7xJoKC");
    }

    /// Una imagen pegada en el escritorio, con el `id_objeto` que le da el
    /// almacen (`pc<hex>` al escribirse).
    fn imagen_pegada(id_objeto: u64) -> Elemento {
        Elemento {
            figura: Figura::Imagen { id_objeto },
            ancho: 320.0,
            alto: 200.0,
            x: 40.0,
            y: 50.0,
            ..Elemento::default()
        }
    }

    #[test]
    fn una_imagen_pegada_con_su_fichero_apuntado_se_guarda_y_vuelve_a_abrir() {
        // El fallo del usuario (2-oct-2026): «si pego una imagen en el canvas
        // no se guarda». `con_escena` se saltaba toda imagen nueva.
        let id = 0x1a2b_3c4d_5e6f_u64;
        let clave = crate::enlace::id_de_texto(id);
        let mut lienzo = Lienzo::vacio();
        poner_fichero(
            &mut lienzo,
            &clave,
            "image/png",
            &format!("imagenes/{clave}"),
            7,
        );
        let mut escena = crate::Escena::nueva();
        escena.anadir(imagen_pegada(id));
        let texto = escribir(&con_escena(&lienzo, &escena));

        let v: Value = serde_json::from_str(&texto).unwrap();
        assert_eq!(v["elements"][0]["type"], "image");
        assert_eq!(v["elements"][0]["fileId"], clave.as_str());
        assert_eq!(v["files"][&clave]["path"], format!("imagenes/{clave}"));
        assert_eq!(v["files"][&clave]["mimeType"], "image/png");

        let reabierto = leer(&texto).unwrap();
        let elementos = a_escena(&reabierto);
        let foto = elementos.visibles().next().expect("la imagen sigue ahi");
        assert_eq!(foto.figura, Figura::Imagen { id_objeto: id });
        assert_eq!(
            (foto.x, foto.y, foto.ancho, foto.alto),
            (40.0, 50.0, 320.0, 200.0)
        );
        assert_eq!(
            ficheros(&reabierto),
            vec![(id, format!("imagenes/{clave}"))]
        );
    }

    #[test]
    fn una_imagen_pegada_sin_fichero_no_deja_un_hueco_en_el_movil() {
        // Caso negativo: sin su entrada en `files`, el movil abriria una
        // imagen que no encuentra. Se sigue sin escribir.
        let mut escena = crate::Escena::nueva();
        escena.anadir(imagen_pegada(99));
        let texto = escribir(&con_escena(&Lienzo::vacio(), &escena));
        let v: Value = serde_json::from_str(&texto).unwrap();
        assert_eq!(v["elements"].as_array().unwrap().len(), 0);
        // Y apuntar dos veces la misma foto no la duplica ni le cambia la fecha.
        let mut l = Lienzo::vacio();
        poner_fichero(&mut l, "pc63", "image/png", "imagenes/pc63", 1);
        poner_fichero(&mut l, "pc63", "image/png", "imagenes/pc63", 2);
        assert_eq!(l.resto["files"].as_object().unwrap().len(), 1);
        assert_eq!(l.resto["files"]["pc63"]["created"], 1);
    }

    #[test]
    fn los_puntos_del_movil_se_leen_aunque_vengan_como_objetos() {
        // El fallo que dejo un dibujo del movil con mil trazos invisibles:
        // Excalidraw escribe los puntos como `[x, y]` y PixPin Android como
        // `{"x":…,"y":…}`. Leyendo solo la primera forma, los trazos llegaban
        // sin un solo punto y no se pintaba ninguno; se veian las figuras,
        // que no llevan puntos, y parecia que faltaba el 95 % del dibujo.
        let texto = r##"{"type":"excalidraw","elements":[
            {"id":"a","type":"freedraw","x":10,"y":20,"width":5,"height":5,
             "strokeColor":"#000000","strokeWidth":1.25,
             "points":[{"x":0,"y":0},{"x":3,"y":4}]},
            {"id":"b","type":"line","x":0,"y":0,"width":9,"height":0,
             "strokeColor":"#000000","points":[[0,0],[9,0]]}
        ]}"##;
        let elementos = leer(texto).unwrap().elementos();
        let puntos_de = |e: &Elemento| match &e.figura {
            Figura::Lapiz { puntos, .. } | Figura::Linea { puntos } => puntos.clone(),
            otra => panic!("no es un trazo: {otra:?}"),
        };
        let a = puntos_de(&elementos[0]);
        assert_eq!(a.len(), 2, "el trazo del movil llego sin puntos");
        // Y siguen siendo relativos al origen del elemento.
        assert_eq!((a[1].x, a[1].y), (13.0, 24.0));
        assert_eq!(puntos_de(&elementos[1]).len(), 2, "y la forma de siempre");
    }

    #[test]
    fn un_marco_va_y_vuelve_con_su_nombre() {
        let texto = r##"{"type":"excalidraw","elements":[
            {"id":"f1","type":"frame","x":0,"y":0,"width":300,"height":200,
             "name":"Lamina 1","strokeColor":"#000000","frameId":null}
        ]}"##;
        let lienzo = leer(texto).unwrap();
        let elementos = lienzo.elementos();
        assert_eq!(elementos.len(), 1);
        assert!(
            matches!(&elementos[0].figura, Figura::Marco { nombre } if nombre == "Lamina 1"),
            "no se leyo como marco: {:?}",
            elementos[0].figura
        );

        let salida = escribir(&lienzo);
        assert!(salida.contains("\"frame\""), "se perdio el tipo");
        assert!(salida.contains("Lamina 1"), "se perdio el nombre");
    }

    #[test]
    fn los_grupos_del_movil_llegan_al_escritorio() {
        let json = r##"{
            "type": "excalidraw",
            "elements": [
                {"type":"rectangle","x":0,"y":0,"width":10,"height":10,
                 "strokeColor":"#000000","seed":1,"groupIds":["g1","g2"]},
                {"type":"rectangle","x":20,"y":0,"width":10,"height":10,
                 "strokeColor":"#000000","seed":2,"groupIds":[]}
            ]
        }"##;
        let elementos = leer(json).unwrap().elementos();
        assert_eq!(elementos[0].grupos, vec!["g1", "g2"]);
        assert!(elementos[1].grupos.is_empty());
    }

    #[test]
    fn un_elemento_sin_grupos_se_lee_igual() {
        // Compatibilidad hacia atras: los ficheros que ya guardamos no llevan
        // el campo y tienen que seguir abriendo.
        let json = r##"{"type":"excalidraw","elements":[
            {"type":"rectangle","x":0,"y":0,"width":10,"height":10,
             "strokeColor":"#000000","seed":1}
        ]}"##;
        assert!(leer(json).unwrap().elementos()[0].grupos.is_empty());
    }

    #[test]
    fn los_grupos_sobreviven_la_ida_y_la_vuelta() {
        let json = r##"{"type":"excalidraw","elements":[
            {"type":"rectangle","x":0,"y":0,"width":10,"height":10,
             "strokeColor":"#000000","seed":1,"groupIds":["g1"]}
        ]}"##;
        let lienzo = leer(json).unwrap();
        let otra_vez = leer(&escribir(&lienzo)).unwrap();
        assert_eq!(otra_vez.elementos()[0].grupos, vec!["g1"]);
    }

    #[test]
    fn agrupar_en_windows_se_ve_en_el_movil() {
        // Lo que hace util esta tarea: no solo conservar los grupos del
        // telefono, sino que los que se hagan aqui vuelvan alla.
        let json = r##"{"type":"excalidraw","elements":[
            {"type":"rectangle","x":0,"y":0,"width":10,"height":10,
             "strokeColor":"#000000","seed":1}
        ]}"##;
        let mut lienzo = leer(json).unwrap();
        primero_mut(&mut lienzo).grupos = vec!["nuevo".to_string()];

        let vuelta = escribir(&lienzo);
        assert!(
            vuelta.contains("\"groupIds\""),
            "el JSON tiene que llevar groupIds: {vuelta}"
        );
        assert_eq!(leer(&vuelta).unwrap().elementos()[0].grupos, vec!["nuevo"]);
    }

    #[test]
    fn desagrupar_en_windows_no_deja_los_grupos_viejos() {
        // Si groupIds solo se escribiera cuando hay grupos, desagrupar aqui
        // dejaria intactos los del JSON original —que `escribir` reutiliza como
        // base— y el movil los seguiria viendo agrupados. Es el motivo de
        // escribirlo siempre, tambien vacio.
        let json = r##"{"type":"excalidraw","elements":[
            {"type":"rectangle","x":0,"y":0,"width":10,"height":10,
             "strokeColor":"#000000","seed":1,"groupIds":["viejo"]}
        ]}"##;
        let mut lienzo = leer(json).unwrap();
        primero_mut(&mut lienzo).grupos.clear();

        let otra_vez = leer(&escribir(&lienzo)).unwrap();
        assert!(
            otra_vez.elementos()[0].grupos.is_empty(),
            "desagrupado aqui, desagrupado alla"
        );
    }

    #[test]
    fn un_elemento_ajeno_sigue_viajando_intacto_con_sus_grupos() {
        // La garantia que no se puede romper: lo que Windows no entiende
        // sobrevive al viaje. Anadir un campo al lado no puede estropearlo.
        //
        // El tipo de muestra tiene que ser uno que no vayamos a implementar:
        // `pixpin-measure` sirvio hasta que la tarea 3 le enseno a
        // `elemento_desde` a entenderlo, y el cronograma hasta F12. El solido
        // 3D del movil no esta en ninguna fase del plan.
        let json = r#"{"type":"excalidraw","elements":[
            {"type":"pixpin-solid","x":0,"y":0,"groupIds":["g9"],"altura":80}
        ]}"#;
        let lienzo = leer(json).unwrap();
        assert_eq!(lienzo.cuantos_ajenos(), 1);

        let vuelta = escribir(&lienzo);
        assert!(vuelta.contains("pixpin-solid"), "el tipo ajeno sigue");
        assert!(vuelta.contains("\"g9\""), "y su grupo tambien");
        assert!(vuelta.contains("\"altura\""), "y sus campos propios");
    }

    #[test]
    fn un_dibujo_con_un_cronograma_del_movil_se_abre_y_lo_demas_sobrevive() {
        // El cronograma se quito del lienzo, pero los dibujos de antes (y los
        // del movil, que lo sigue teniendo) pueden traerlo. Abrirlos no puede
        // fallar ni perder nada: el cronograma cae al carril ajeno —no se
        // pinta aqui— y vuelve intacto al guardar, con sus filas, en su sitio
        // y sin tocar lo de alrededor.
        let json = r##"{"type":"excalidraw","elements":[
            {"id":"a1","type":"rectangle","x":0,"y":0,"width":50,"height":30,
             "strokeColor":"#1e1e1e","seed":7},
            {"id":"c1","type":"pixpin-gantt","x":10,"y":20,"width":400,"height":200,
             "strokeColor":"#1e1e1e","groupIds":["g2"],"periodos":8,"fontFamily":2,
             "fontSize":20,
             "tareas":[{"nombre":"Obra","desde":0,"cuanto":2.5},
                       {"nombre":"Acabados","desde":2.5,"cuanto":1,"color":"#e03131"}]},
            {"id":"t1","type":"text","x":5,"y":300,"width":80,"height":25,
             "text":"Hola","fontSize":20,"seed":3}
        ]}"##;
        let l = leer(json).unwrap();
        assert_eq!(l.cuantos_ajenos(), 1, "el cronograma viaja como ajeno");
        assert!(matches!(l.entradas[1], Entrada::Ajeno(_)), "en su sitio");
        let escena = a_escena(&l);
        let figuras: Vec<_> = escena.visibles().map(|e| e.figura.clone()).collect();
        assert_eq!(figuras.len(), 2, "el rectangulo y el texto: {figuras:?}");
        assert_eq!(figuras[0], Figura::Rectangulo);
        assert!(matches!(figuras[1], Figura::Texto { .. }));

        // Guardar despues de pasar por la escena: el cronograma vuelve
        // exactamente como vino y en el mismo orden.
        let vuelta: Value = serde_json::from_str(&escribir(&con_escena(&l, &escena))).unwrap();
        let original: Value = serde_json::from_str(json).unwrap();
        let tipos: Vec<_> = vuelta["elements"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["type"].as_str().unwrap_or("").to_string())
            .collect();
        assert_eq!(tipos, ["rectangle", "pixpin-gantt", "text"]);
        assert_eq!(vuelta["elements"][1], original["elements"][1], "intacto");
    }

    #[test]
    fn una_cota_del_movil_se_lee_como_cota() {
        let json = r##"{"type":"excalidraw","elements":[
            {"type":"pixpin-measure","x":0,"y":0,"width":100,"height":0,
             "strokeColor":"#000000","seed":1,
             "points":[[0,0],[100,0]]}
        ]}"##;
        let l = leer(json).unwrap();
        assert_eq!(l.cuantos_ajenos(), 0, "ya la entendemos");
        assert!(matches!(l.elementos()[0].figura, Figura::Cota { .. }));
    }

    #[test]
    fn una_barra_de_escala_del_movil_se_lee_como_barra() {
        let json = r##"{"type":"excalidraw","elements":[
            {"type":"pixpin-scalebar","x":10,"y":20,"width":400,"height":24,
             "strokeColor":"#000000","seed":1}
        ]}"##;
        let l = leer(json).unwrap();
        assert_eq!(l.cuantos_ajenos(), 0);
        assert!(matches!(l.elementos()[0].figura, Figura::EscalaGrafica));
    }

    #[test]
    fn la_escala_del_lienzo_se_lee_y_se_devuelve() {
        let json = r##"{"type":"excalidraw","elements":[],
            "escala":{"unidadesPorPixel":0.03,"unidad":"m","decimales":2}}"##;
        let l = leer(json).unwrap();
        let e = l.escala.as_ref().expect("hay escala");
        assert!((e.unidades_por_pixel - 0.03).abs() < 1e-6);
        assert_eq!(e.unidad, "m");
        assert_eq!(e.decimales, 2);

        let vuelta = leer(&escribir(&l)).unwrap();
        assert_eq!(vuelta.escala, l.escala, "sobrevive la ida y la vuelta");
    }

    /// Un lienzo inventado con la forma exacta del movil: la escala arriba
    /// del todo (`Scene.escala`, junto a `backgroundColor`) y dos cotas con
    /// su `fontSize` y sus puntos como objetos. 36,637 px a 0,13647 cm/px
    /// son 5 cm.
    const FOTO_MEDIDA_DEL_MOVIL: &str = r##"{"type":"excalidraw","backgroundColor":"#ffffff",
        "escala":{"decimales":2,"unidad":"cm","unidadesPorPixel":0.13647374510765076},
        "elements":[
          {"type":"pixpin-measure","id":"cota-a","x":100.5,"y":200.25,"width":2.2e-15,"height":36.637,
           "points":[{"x":0.0,"y":0.0},{"x":2.2e-15,"y":36.637}],"strokeColor":"#0edeff",
           "strokeWidth":2.5,"roughness":1,"opacity":100,"seed":42,"fontFamily":5,"fontSize":9.0,
           "angle":0.0,"isDeleted":false,"version":3},
          {"type":"pixpin-measure","id":"cota-b","x":300.0,"y":400.0,"width":0.6955,"height":36.6305,
           "points":[{"x":0.0,"y":0.0},{"x":-0.6955,"y":36.6305}],"strokeColor":"#0edeff",
           "strokeWidth":2.5,"roughness":1,"opacity":100,"seed":7,"fontFamily":5,"fontSize":20.0,
           "angle":0.0,"isDeleted":false,"version":2}
        ]}"##;

    #[test]
    fn la_escala_del_movil_llega_a_la_escena_y_la_cota_dice_centimetros() {
        let escena = a_escena(&leer(FOTO_MEDIDA_DEL_MOVIL).unwrap());
        let e = escena
            .escala
            .as_ref()
            .expect("la escala del fichero llega a la escena");
        assert_eq!(e.unidad, "cm");
        let cotas: Vec<&Elemento> = escena.visibles().collect();
        let textos: Vec<String> = cotas
            .iter()
            .map(|c| crate::medida::texto_de_cota(c, escena.escala.as_ref(), ','))
            .collect();
        // Lo mismo que `textoDeCota` del movil: la medida con sus dos
        // decimales y coma, y el angulo porque la raya esta torcida.
        assert_eq!(
            textos,
            vec!["5,00 cm · -90°".to_string(), "5,00 cm · -91°".to_string()]
        );
        // Sin la escala (lo que pasaba), pixeles: el fallo que vio el usuario.
        assert_eq!(crate::medida::texto_de_medida(36.637, None, ','), "37 px");
    }

    #[test]
    fn el_tamano_del_numero_de_la_cota_viaja_en_su_font_size() {
        let l = leer(FOTO_MEDIDA_DEL_MOVIL).unwrap();
        let tams: Vec<Option<f32>> = l.elementos().iter().map(|e| e.extras.tam_letra).collect();
        assert_eq!(tams, vec![Some(9.0), Some(20.0)]);
        // Y una cota tocada en Windows lo devuelve.
        let mut escena = a_escena(&l);
        let id = escena.visibles().next().unwrap().id;
        escena.buscar_mut(id).unwrap().extras.tam_letra = Some(12.0);
        escena.buscar_mut(id).unwrap().version += 1;
        let v: Value = serde_json::from_str(&escribir(&con_escena(&l, &escena))).unwrap();
        assert_eq!(v["elements"][0]["fontSize"], 12.0);
        assert_eq!(v["elements"][1]["fontSize"], 20.0);
    }

    /// Lo que nace en el PC tras abrir una capa a la que el movil quito un
    /// elemento (13-oct-2026): el id nuevo no repite el de ningun `pc<n>` del
    /// fichero. Caso negativo: sin ids del PC se numera como siempre.
    #[test]
    fn lo_que_nace_aqui_no_repite_el_id_de_otro_nacido_aqui() {
        let raya = |id: &str| {
            format!(
                r#"{{"id":"{id}","type":"line","x":0,"y":0,"width":5,"height":0,"seed":1,"points":[[0,0],[5,0]]}}"#
            )
        };
        // Uno del movil y el `pc3` de otra vez: habia tres y el movil quito
        // uno. Antes, lo nuevo era el tercero: `pc3` otra vez.
        let json = format!(
            r#"{{"type":"excalidraw","elements":[{},{}]}}"#,
            raya("aaaaaaaaaaaaaaaaaaaaa"),
            raya("pc3")
        );
        let l = leer(&json).unwrap();
        let mut escena = a_escena(&l);
        let nuevo = escena.anadir(crate::zona::marca((0.0, 0.0, 10.0, 10.0), "x"));
        let v: Value = serde_json::from_str(&escribir(&con_escena(&l, &escena))).unwrap();
        let ids: Vec<&str> = v["elements"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids.len(), 3);
        assert_eq!(ids.iter().filter(|i| **i == "pc3").count(), 1, "{ids:?}");
        assert_eq!(
            ids[2],
            crate::enlace::id_de_texto(nuevo),
            "lo nuevo, con el id de su numero"
        );
        assert_eq!(nuevo, 4);
        // Caso negativo: solo del movil, lo nuevo va detras como siempre.
        let json = format!(
            r#"{{"type":"excalidraw","elements":[{},{}]}}"#,
            raya("aaaaaaaaaaaaaaaaaaaaa"),
            raya("pcZZ")
        );
        let mut escena = a_escena(&leer(&json).unwrap());
        assert_eq!(
            escena.anadir(crate::zona::marca((0.0, 0.0, 10.0, 10.0), "x")),
            3
        );
    }

    #[test]
    fn la_escala_hace_ida_y_vuelta_por_la_escena_sin_perder_un_decimal() {
        let l = leer(FOTO_MEDIDA_DEL_MOVIL).unwrap();
        let vuelta: Value =
            serde_json::from_str(&escribir(&con_escena(&l, &a_escena(&l)))).unwrap();
        let original: Value = serde_json::from_str(FOTO_MEDIDA_DEL_MOVIL).unwrap();
        assert_eq!(
            vuelta["escala"], original["escala"],
            "tal cual, con sus 17 cifras"
        );
    }

    #[test]
    fn calibrar_o_quitar_la_escala_en_la_escena_llega_al_fichero() {
        let l = leer(FOTO_MEDIDA_DEL_MOVIL).unwrap();
        let mut escena = a_escena(&l);
        escena.escala = Escala::calibrando(100.0, 3.0, "m", 1);
        let v: Value = serde_json::from_str(&escribir(&con_escena(&l, &escena))).unwrap();
        assert_eq!(v["escala"]["unidad"], "m");
        assert_eq!(v["escala"]["decimales"], 1);
        // Quitarla la quita tambien alla (no sobrevive la del original).
        escena.escala = None;
        let v: Value = serde_json::from_str(&escribir(&con_escena(&l, &escena))).unwrap();
        assert!(v["escala"].is_null());
    }

    #[test]
    fn una_escala_que_no_se_entiende_sobrevive_al_pasar_por_la_escena() {
        let json = r##"{"type":"excalidraw","elements":[],"escala":{"unidadesPorPixel":-1,"futuro":true}}"##;
        let l = leer(json).unwrap();
        let escena = a_escena(&l);
        assert!(escena.escala.is_none(), "no se mide con ella");
        let v: Value = serde_json::from_str(&escribir(&con_escena(&l, &escena))).unwrap();
        assert_eq!(v["escala"]["futuro"], true, "pero no se destruye");
    }

    #[test]
    fn un_lienzo_sin_escala_se_lee_igual() {
        // Compatibilidad hacia atras: los ficheros que ya hay no la llevan.
        let json = r##"{"type":"excalidraw","elements":[]}"##;
        assert!(leer(json).unwrap().escala.is_none());
    }

    #[test]
    fn una_escala_imposible_del_movil_se_ignora() {
        // D36: mas vale no medir que medir mal. Si el movil escribiera una
        // escala rota, no se adopta.
        let json = r##"{"type":"excalidraw","elements":[],
            "escala":{"unidadesPorPixel":0,"unidad":"m","decimales":2}}"##;
        assert!(leer(json).unwrap().escala.is_none());
    }

    #[test]
    fn calibrar_en_windows_se_ve_en_el_movil() {
        let json = r##"{"type":"excalidraw","elements":[]}"##;
        let mut l = leer(json).unwrap();
        l.escala = Escala::calibrando(100.0, 3.0, "m", 2);

        let vuelta = escribir(&l);
        assert!(vuelta.contains("\"escala\""), "el JSON la lleva: {vuelta}");
        let otra = leer(&vuelta).unwrap();
        assert!((otra.escala.unwrap().unidades_por_pixel - 0.03).abs() < 1e-6);
    }

    #[test]
    fn quitar_la_escala_en_windows_la_quita_tambien_alla() {
        // Si solo se escribiera cuando existe, la del original sobreviviria y
        // el movil seguiria midiendo con una escala que aqui se borro.
        let json = r##"{"type":"excalidraw","elements":[],
            "escala":{"unidadesPorPixel":0.03,"unidad":"m","decimales":2}}"##;
        let mut l = leer(json).unwrap();
        l.escala = None;
        assert!(leer(&escribir(&l)).unwrap().escala.is_none());
    }

    #[test]
    fn una_cota_hecha_en_windows_vuelve_como_pixpin_measure() {
        let json = r##"{"type":"excalidraw","elements":[]}"##;
        let mut l = leer(json).unwrap();
        l.entradas.push(Entrada::Nuestro {
            elemento: Elemento {
                id: 1,
                figura: Figura::Cota {
                    puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0)],
                },
                x: 0.0,
                y: 0.0,
                ancho: 100.0,
                alto: 0.0,
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
            },
            original: Box::new(Value::Object(Map::new())),
        });

        let vuelta = escribir(&l);
        assert!(vuelta.contains("pixpin-measure"), "con el tipo del movil");
        assert!(matches!(
            leer(&vuelta).unwrap().elementos()[0].figura,
            Figura::Cota { .. }
        ));
    }

    #[test]
    fn una_escala_que_no_entendemos_sobrevive_intacta_y_no_se_pisa_con_null() {
        // Hallazgo 7: `leer` no quitaba «escala» de `resto`, y `escribir` la
        // pisaba siempre. Una escala corrupta -o de una version futura del
        // movil que aqui no se entiende- pasaba por Windows y salia con
        // "escala": null: «no la entiendo» y «no la hay» se representaban
        // igual, exactamente lo que `Entrada::Ajeno` existe para impedir en
        // los elementos.
        let json = r##"{"type":"excalidraw","elements":[],
            "escala":{"unidadesPorPixel":0,"unidad":"m","decimales":2,"deVersionFutura":true}}"##;
        let l = leer(json).unwrap();
        assert!(l.escala.is_none(), "no la entendemos, no mide con ella");

        let vuelta = escribir(&l);
        assert!(
            !vuelta.contains("\"escala\":null") && !vuelta.contains("\"escala\": null"),
            "la escala corrupta se piso con null:\n{vuelta}"
        );
        assert!(
            vuelta.contains("deVersionFutura"),
            "el campo que no entendemos tiene que sobrevivir tal cual:\n{vuelta}"
        );
    }

    #[test]
    fn el_decimales_de_una_escala_del_movil_tambien_se_recorta_a_seis() {
        // El mismo tope que `Escala::calibrando`, para que el invariante del
        // tipo lo sostengan los dos constructores y no solo uno.
        let json = r##"{"type":"excalidraw","elements":[],
            "escala":{"unidadesPorPixel":0.03,"unidad":"m","decimales":44}}"##;
        let l = leer(json).unwrap();
        assert_eq!(l.escala.unwrap().decimales, 6);
    }

    #[test]
    fn una_cota_del_movil_sin_puntos_entra_como_ajena_y_no_como_cota_fantasma() {
        // Hallazgo 8: sin dos puntos no hay raya que dibujar ni que tocar, y
        // antes de que `elemento_desde` aprendiera `pixpin-measure` este
        // mismo JSON sobrevivia intacto como ajeno. Una cota sin puntos no
        // es una cota: mejor que vuelva a caer ahi, donde no se pierde.
        let json = r##"{"type":"excalidraw","elements":[
            {"id":"m1","type":"pixpin-measure","x":0,"y":0,"width":0,"height":0,
             "strokeColor":"#000000","seed":1}
        ]}"##;
        let l = leer(json).unwrap();
        assert_eq!(l.cuantos_ajenos(), 1, "sin puntos, tiene que caer a ajeno");
        assert!(l.elementos().is_empty());

        let vuelta = escribir(&l);
        assert!(vuelta.contains("pixpin-measure"), "sobrevive intacta");
    }

    #[test]
    fn los_elementos_ajenos_siguen_viajando_intactos() {
        // La garantia que no se puede romper: lo que Windows no entiende
        // sobrevive. Anadir dos tipos nuevos no puede estropearlo.
        let json = r##"{"type":"excalidraw","elements":[
            {"type":"pixpin-gantt","x":0,"y":0,"tareas":[],"periodos":[]}
        ]}"##;
        let l = leer(json).unwrap();
        assert_eq!(l.cuantos_ajenos(), 1);
        let vuelta = escribir(&l);
        assert!(vuelta.contains("pixpin-gantt"));
        assert!(vuelta.contains("periodos"));
    }

    #[test]
    fn un_freedraw_de_excalidraw_trae_sus_opciones_de_pluma_y_vuelve_igual() {
        use crate::tinta::{OpcionesTinta, Variabilidad};
        let texto = r##"{"type":"excalidraw","version":2,"elements":[{"id":"a","type":"freedraw",
            "x":0,"y":0,"width":10,"height":10,"angle":0,"strokeColor":"#1e1e1e",
            "backgroundColor":"transparent","strokeWidth":2,"seed":1,
            "points":[[0,0],[5,5],[10,10]],"pressures":[0.2,0.5,0.9],"simulatePressure":false,
            "strokeOptions":{"variability":"constant","streamline":0.2}}]}"##;
        let escena = leer(texto).expect("se lee");
        let Figura::Lapiz {
            presiones,
            opciones,
            ..
        } = &escena.elementos()[0].figura
        else {
            panic!("tendria que ser un lapiz")
        };
        assert_eq!(presiones.len(), 3);
        assert_eq!(
            *opciones,
            Some(OpcionesTinta {
                variabilidad: Variabilidad::Constante,
                streamline: 0.2
            })
        );
        let vuelta = escribir(&escena);
        // `escribir` usa `to_string_pretty`: hay un espacio tras los dos puntos.
        assert!(vuelta.contains("\"variability\": \"constant\""), "{vuelta}");
        assert!(vuelta.contains("\"simulatePressure\": false"), "{vuelta}");
    }

    #[test]
    fn con_simulate_pressure_verdadero_se_ignoran_las_presiones_guardadas() {
        let texto = r##"{"type":"excalidraw","version":2,"elements":[{"id":"a","type":"freedraw",
            "x":0,"y":0,"width":10,"height":10,"strokeWidth":1,"seed":1,
            "points":[[0,0],[5,5]],"pressures":[0.2,0.5],"simulatePressure":true}]}"##;
        let escena = leer(texto).expect("se lee");
        let Figura::Lapiz {
            presiones,
            opciones,
            ..
        } = &escena.elementos()[0].figura
        else {
            panic!()
        };
        assert!(presiones.is_empty());
        assert_eq!(*opciones, Some(crate::tinta::OpcionesTinta::default()));
    }

    // --- Editar y guardar de vuelta ---

    const HOJA_DEL_MOVIL: &str = r##"{"type":"excalidraw","hoja":{"padre":"foto-1"},
      "elements":[
        {"id":"t1","type":"freedraw","x":10,"y":20,"width":5,"height":5,"seed":7,
         "roughness":0,"version":3,"points":[{"x":0,"y":0},{"x":5,"y":5}],
         "campoDelFuturo":"se queda"},
        {"id":"g1","type":"pixpin-solid","x":0,"y":0,"width":1,"height":1},
        {"id":"r1","type":"rectangle","x":0,"y":0,"width":10,"height":10,"seed":9,
         "roughness":0,"version":1}
      ]}"##;

    fn elementos_de(json: &str) -> Vec<Value> {
        let v: Value = serde_json::from_str(json).unwrap();
        v["elements"].as_array().unwrap().clone()
    }

    #[test]
    fn un_emoji_va_y_vuelve_por_excalidraw_como_texto() {
        let lienzo = leer(HOJA_DEL_MOVIL).unwrap();
        let mut escena = a_escena(&lienzo);
        let base = escena.buscar(2).unwrap().clone();
        escena.anadir(Elemento {
            id: 0,
            figura: Figura::Emoji {
                caracter: "😀".into(),
            },
            ancho: 64.0,
            alto: 64.0,
            ..base
        });
        let texto = escribir(&con_escena(&lienzo, &escena));
        // Fuera, en excalidraw.com, es un texto normal que ensena el emoji.
        let ultimo = elementos_de(&texto).pop().unwrap();
        assert_eq!(ultimo["type"], "text");
        assert_eq!(ultimo["text"], "😀");
        assert_eq!(ultimo["customData"]["pixpin"], "emoji");
        let vuelta = leer(&texto).unwrap();
        let elementos = vuelta.elementos();
        assert!(matches!(
            &elementos.last().unwrap().figura,
            Figura::Emoji { caracter } if caracter == "😀"
        ));
        // Caso negativo: lo que no lleva la marca no se vuelve emoji.
        assert!(elementos.len() >= 3);
        assert!(!matches!(elementos[0].figura, Figura::Emoji { .. }));
    }

    #[test]
    fn abrir_y_cerrar_sin_tocar_nada_no_cambia_ni_un_elemento() {
        let lienzo = leer(HOJA_DEL_MOVIL).unwrap();
        let escena = a_escena(&lienzo);
        let salida = escribir(&con_escena(&lienzo, &escena));
        assert_eq!(elementos_de(&salida), elementos_de(HOJA_DEL_MOVIL));
    }

    #[test]
    fn mover_un_trazo_del_movil_lo_guarda_con_los_puntos_como_objetos() {
        let lienzo = leer(HOJA_DEL_MOVIL).unwrap();
        let mut escena = a_escena(&lienzo);
        assert!(escena.mover(1, 100.0, 0.0));
        let salida = elementos_de(&escribir(&con_escena(&lienzo, &escena)));
        let t = &salida[0];
        assert_eq!(t["id"], "t1");
        assert_eq!(t["x"].as_f64(), Some(110.0));
        // Una lista `[0,0]` aqui y el movil no abre la hoja.
        assert!(t["points"][0].is_object());
        assert_eq!(t["campoDelFuturo"], "se queda");
        assert!(t["version"].as_u64().unwrap() > 3);
        // Y el vecino que no se toco, ni se entera.
        assert_eq!(salida[2], elementos_de(HOJA_DEL_MOVIL)[2]);
    }

    #[test]
    fn la_rugosidad_entera_se_escribe_sin_decimales() {
        let lienzo = leer(HOJA_DEL_MOVIL).unwrap();
        let mut escena = a_escena(&lienzo);
        escena.mover(2, 1.0, 1.0);
        let salida = elementos_de(&escribir(&con_escena(&lienzo, &escena)));
        assert!(salida[2]["roughness"].is_i64() || salida[2]["roughness"].is_u64());
    }

    #[test]
    fn lo_borrado_se_marca_y_no_se_quita_y_lo_ajeno_no_se_mueve() {
        let lienzo = leer(HOJA_DEL_MOVIL).unwrap();
        let mut escena = a_escena(&lienzo);
        assert!(escena.borrar_apuntando(2));
        let salida = elementos_de(&escribir(&con_escena(&lienzo, &escena)));
        assert_eq!(salida.len(), 3);
        assert_eq!(salida[1]["type"], "pixpin-solid");
        assert_eq!(salida[2]["id"], "r1");
        assert_eq!(salida[2]["isDeleted"], true);
    }

    #[test]
    fn un_trazo_nuevo_sale_con_id_tipo_y_la_forma_de_puntos_del_movil() {
        let lienzo = leer(HOJA_DEL_MOVIL).unwrap();
        let mut escena = a_escena(&lienzo);
        let mut nuevo = lienzo.elementos()[0].clone();
        nuevo.semilla = u32::MAX;
        escena.anadir(nuevo);
        let salida = elementos_de(&escribir(&con_escena(&lienzo, &escena)));
        assert_eq!(salida.len(), 4);
        let n = &salida[3];
        assert_eq!(n["type"], "freedraw");
        assert!(n["id"].as_str().is_some_and(|s| !s.is_empty()));
        assert!(n["points"][0].is_object());
        // Un `Int` de Kotlin: no le cabe mas.
        assert!(n["seed"].as_u64().unwrap() <= i32::MAX as u64);
    }

    #[test]
    fn en_un_lienzo_de_excalidraw_lo_nuevo_lleva_los_puntos_como_listas() {
        let web = r#"{"type":"excalidraw","elements":[
          {"id":"a","type":"freedraw","x":0,"y":0,"width":1,"height":1,
           "points":[[0,0],[1,1]]}]}"#;
        let lienzo = leer(web).unwrap();
        let mut escena = a_escena(&lienzo);
        escena.anadir(lienzo.elementos()[0].clone());
        let salida = elementos_de(&escribir(&con_escena(&lienzo, &escena)));
        assert!(salida[1]["points"][0].is_array());
    }

    /// Un lienzo como lo escribe el movil (`ExcalidrawStore.exportar`): su
    /// papel «Crema» de `DrawTheme.PAPELES` y un `gridSize` que aqui no se
    /// usa y tiene que volver.
    const LIENZO_CREMA_DEL_MOVIL: &str = r##"{"type":"excalidraw","version":2,
      "source":"https://github.com/1xmanMAX/PIXPIN_PRO_ANDROID",
      "elements":[
        {"id":"r1","type":"rectangle","x":0,"y":0,"width":40,"height":20,
         "strokeColor":"#1e1e1e","backgroundColor":"transparent"}],
      "appState":{"gridSize":20,"viewBackgroundColor":"#fdf6e3"}}"##;

    fn app_state_de(json: &str) -> Value {
        let v: Value = serde_json::from_str(json).unwrap();
        v["appState"].clone()
    }

    #[test]
    fn el_papel_del_movil_se_lee_en_la_escena_y_sale_intacto_si_no_se_toca() {
        let lienzo = leer(LIENZO_CREMA_DEL_MOVIL).unwrap();
        let escena = a_escena(&lienzo);
        assert_eq!(color_hacia(escena.fondo), "#fdf6e3");
        // Abrir no es un cambio: deshacerlo todo no le quita el papel.
        let mut deshecha = escena.clone();
        while deshecha.deshacer() {}
        assert_eq!(deshecha.fondo, escena.fondo, "abrir no es un cambio");
        let salida = escribir(&con_escena(&lienzo, &escena));
        assert_eq!(
            app_state_de(&salida),
            app_state_de(LIENZO_CREMA_DEL_MOVIL),
            "el appState cambio sin haber tocado el papel"
        );
    }

    #[test]
    fn cambiar_el_papel_lo_escribe_donde_lo_lee_el_movil_sin_tocar_el_resto() {
        let lienzo = leer(LIENZO_CREMA_DEL_MOVIL).unwrap();
        let mut escena = a_escena(&lienzo);
        // «Azul noche» del movil.
        assert!(escena.poner_fondo(color_desde(Some(&Value::from("#14213d"))).unwrap()));
        let salida = escribir(&con_escena(&lienzo, &escena));
        let estado = app_state_de(&salida);
        assert_eq!(estado["viewBackgroundColor"], "#14213d");
        assert_eq!(estado["gridSize"], 20, "se perdio lo que no es el papel");
        // Y de vuelta: el mismo papel al reabrir.
        let otra = a_escena(&leer(&salida).unwrap());
        assert_eq!(otra.fondo, escena.fondo);
    }

    #[test]
    fn un_lienzo_sin_app_state_y_con_papel_blanco_no_gana_un_app_state() {
        // Caso negativo: nada que decir, nada que escribir.
        let web = r#"{"type":"excalidraw","elements":[]}"#;
        let lienzo = leer(web).unwrap();
        let escena = a_escena(&lienzo);
        assert_eq!(escena.fondo, crate::escena::FONDO_DE_FABRICA);
        let salida = escribir(&con_escena(&lienzo, &escena));
        assert!(!salida.contains("appState"), "{salida}");
    }

    #[test]
    fn un_lienzo_nuevo_con_papel_elegido_se_crea_su_app_state() {
        let mut escena = crate::Escena::nueva();
        escena.poner_fondo(color_desde(Some(&Value::from("#f5faff"))).unwrap());
        let salida = escribir(&con_escena(&Lienzo::vacio(), &escena));
        assert_eq!(app_state_de(&salida)["viewBackgroundColor"], "#f5faff");
    }

    #[test]
    fn un_papel_que_no_se_entiende_se_ve_blanco_y_no_se_pisa_al_guardar() {
        // Caso negativo: «transparent» o basura de una version futura. Se
        // pinta blanco, como el movil, pero si nadie elige otro papel el
        // texto original vuelve tal cual.
        for raro in [r#""transparent""#, r#""azulito""#, "42"] {
            let json = format!(
                r#"{{"type":"excalidraw","elements":[],"appState":{{"viewBackgroundColor":{raro}}}}}"#
            );
            let lienzo = leer(&json).unwrap();
            let escena = a_escena(&lienzo);
            assert_eq!(escena.fondo, crate::escena::FONDO_DE_FABRICA, "{raro}");
            let salida = escribir(&con_escena(&lienzo, &escena));
            assert_eq!(app_state_de(&salida), app_state_de(&json), "{raro}");
        }
    }

    #[test]
    fn un_papel_con_alfa_se_lee_opaco_como_en_el_movil() {
        let json = r##"{"type":"excalidraw","elements":[],"appState":{"viewBackgroundColor":"#11223380"}}"##;
        let escena = a_escena(&leer(json).unwrap());
        assert_eq!(color_hacia(escena.fondo), "#112233");
    }

    /// Lo que se escribe de un elemento de puntos nacido aqui.
    fn escrito(figura: Figura) -> Value {
        let mut escena = crate::Escena::nueva();
        let mut e = Elemento {
            figura,
            grosor: 1.0,
            ..Elemento::default()
        };
        // Como lo deja el gesto: el origen en el primer punto.
        if let Some(p) = e.puntos().and_then(|l| l.first()).copied() {
            (e.x, e.y) = (p.x, p.y);
        }
        escena.anadir(e);
        let json = escribir(&con_escena(&Lienzo::vacio(), &escena));
        let v: Value = serde_json::from_str(&json).unwrap();
        v["elements"][0].clone()
    }

    #[test]
    fn un_trazo_una_linea_y_una_flecha_del_pc_llevan_la_caja_de_sus_puntos_como_en_el_movil() {
        // Queja del 28-sep: «las anotaciones se pasan solo como rayas
        // rectas, conectando los puntos iniciales y finales». El PC escribia
        // `width` y `height` a 0 y el movil (`Renderer.sePierdeDePequeno`)
        // pinta todo lo que mide menos de dos pixeles como la cuerda de su
        // primer punto al ultimo (`pintarComoRaya`). Alli la caja de un
        // elemento de puntos es la de sus puntos (`Element.withPoint`).
        let puntos: Vec<Punto2> = (0..9)
            .map(|i| Punto2::nuevo(300.0 + i as f32 * 5.0, 100.0 + ((i % 3) as f32) * 10.0))
            .collect();
        for figura in [
            Figura::Lapiz {
                puntos: puntos.clone(),
                presiones: Vec::new(),
                opciones: Some(Default::default()),
            },
            Figura::Linea {
                puntos: puntos.clone(),
            },
            Figura::Flecha {
                puntos: puntos.clone(),
                punta_inicio: TipoPunta::Ninguna,
                punta_fin: TipoPunta::Flecha,
                codos: false,
            },
        ] {
            let e = escrito(figura);
            assert_eq!(
                (e["width"].as_f64(), e["height"].as_f64()),
                (Some(40.0), Some(20.0)),
                "{e}"
            );
            // Los N puntos, relativos a su x/y.
            let p = e["points"].as_array().unwrap();
            assert_eq!(p.len(), 9);
            assert_eq!(
                (e["x"].as_f64(), e["y"].as_f64()),
                (Some(300.0), Some(100.0))
            );
            assert_eq!(p[0], serde_json::json!([0.0, 0.0]));
            assert_eq!(p[8], serde_json::json!([40.0, 20.0]));
        }
    }

    #[test]
    fn la_caja_de_lo_que_no_es_de_puntos_y_la_de_lo_que_nadie_toco_no_cambian() {
        // Caso negativo: un rectangulo sigue con su ancho y su alto.
        let mut escena = crate::Escena::nueva();
        escena.anadir(Elemento {
            figura: Figura::Rectangulo,
            ancho: 70.0,
            alto: 30.0,
            ..Elemento::default()
        });
        let v: Value =
            serde_json::from_str(&escribir(&con_escena(&Lienzo::vacio(), &escena))).unwrap();
        assert_eq!(
            (
                v["elements"][0]["width"].as_f64(),
                v["elements"][0]["height"].as_f64()
            ),
            (Some(70.0), Some(30.0))
        );
        // Y un trazo del movil que aqui no se toco sale como entro, con su
        // caja aunque no sea la exacta (el movil la deja asi al mover un punto).
        let del_movil = r#"{"elements":[{"id":"m","type":"freedraw","x":1,"y":2,"width":99,"height":7,"seed":3,"version":2,"points":[{"x":0.0,"y":0.0},{"x":4.0,"y":4.0}]}]}"#;
        let l = leer(del_movil).unwrap();
        let salida = escribir(&con_escena(&l, &a_escena(&l)));
        let v: Value = serde_json::from_str(&salida).unwrap();
        assert_eq!(v["elements"][0]["width"].as_f64(), Some(99.0));
        // Uno solo o ninguno: una caja de nada, como alli.
        let e = escrito(Figura::Lapiz {
            puntos: vec![Punto2::nuevo(5.0, 5.0)],
            presiones: Vec::new(),
            opciones: None,
        });
        assert_eq!(
            (e["width"].as_f64(), e["height"].as_f64()),
            (Some(0.0), Some(0.0))
        );
    }
}

/// **El papel tal como lo guarda el movil de verdad.** Lo que llega al
/// sincronizar no es el `.excalidraw` de exportar sino la `Scene` entera
/// (`ExcalidrawStore.guardar`): el papel va ARRIBA, en `backgroundColor`, y
/// no hay `appState`. Se leia solo `appState.viewBackgroundColor` y todo
/// lienzo del movil se abria en blanco: un dibujo con tiza blanca sobre
/// «Azul noche» desaparecia.
#[cfg(test)]
mod papel_de_la_escena_del_movil {
    use super::*;

    /// La forma de `Scene` (motor/Scene.kt): `style` lleva su propio
    /// `backgroundColor` (el relleno de la proxima figura), que NO es el
    /// papel.
    const ESCENA_AZUL_NOCHE: &str = r##"{"elements":[
        {"id":"t1","type":"freedraw","x":10,"y":10,"width":20,"height":0,"angle":0,
         "strokeColor":"#ffffff","backgroundColor":"transparent","points":[[0,0],[20,0]],
         "pressures":[],"simulatePressure":true,"seed":7,"version":1,"isDeleted":false}],
      "files":{},"viewport":{"scrollX":0,"scrollY":0,"zoom":1},
      "style":{"strokeColor":"#ffffff","backgroundColor":"transparent"},
      "backgroundColor":"#14213d","luces":{"encendidas":true,"fuerza":1},
      "tablas":[],"referenciasVisibles":true,"alfileres":[],"vista":"cero"}"##;

    fn raiz(json: &str) -> Value {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn el_papel_de_una_escena_del_movil_se_lee_de_su_background_color_de_arriba() {
        let escena = a_escena(&leer(ESCENA_AZUL_NOCHE).unwrap());
        assert_eq!(color_hacia(escena.fondo), "#14213d");
    }

    #[test]
    fn abrir_una_escena_del_movil_y_guardarla_sin_tocar_el_papel_no_cambia_nada_arriba() {
        let lienzo = leer(ESCENA_AZUL_NOCHE).unwrap();
        let salida = raiz(&escribir(&con_escena(&lienzo, &a_escena(&lienzo))));
        assert_eq!(salida["backgroundColor"], "#14213d");
        assert!(
            salida.get("appState").is_none(),
            "no se inventa un appState: {salida}"
        );
        assert_eq!(
            salida["style"]["backgroundColor"], "transparent",
            "el estilo no es el papel"
        );
    }

    #[test]
    fn cambiar_el_papel_de_una_escena_del_movil_lo_escribe_arriba_que_es_donde_lo_lee_el_movil() {
        let lienzo = leer(ESCENA_AZUL_NOCHE).unwrap();
        let mut escena = a_escena(&lienzo);
        assert!(escena.poner_fondo(color_desde(Some(&Value::from("#1f3b33"))).unwrap()));
        let salida = raiz(&escribir(&con_escena(&lienzo, &escena)));
        assert_eq!(salida["backgroundColor"], "#1f3b33");
        assert_eq!(salida["style"]["backgroundColor"], "transparent");
        assert_eq!(salida["vista"], "cero", "lo demas de la escena sigue");
        // Y reabierto aqui, el mismo papel.
        let otra = a_escena(&leer(&salida.to_string()).unwrap());
        assert_eq!(otra.fondo, escena.fondo);
    }

    #[test]
    fn si_las_dos_claves_no_coinciden_manda_la_de_arriba_que_es_la_que_el_movil_mantiene() {
        // Un lienzo del movil al que una version vieja de aqui le puso un
        // `appState`: el movil lo ignora y sigue cambiando la de arriba.
        let json = r##"{"elements":[],"backgroundColor":"#000000",
            "appState":{"viewBackgroundColor":"#ffffff"}}"##;
        let escena = a_escena(&leer(json).unwrap());
        assert_eq!(color_hacia(escena.fondo), "#000000");
        // Y al cambiarlo aqui, las dos dicen lo mismo.
        let lienzo = leer(json).unwrap();
        let mut e = a_escena(&lienzo);
        e.poner_fondo(color_desde(Some(&Value::from("#121212"))).unwrap());
        let salida = raiz(&escribir(&con_escena(&lienzo, &e)));
        assert_eq!(salida["backgroundColor"], "#121212");
        assert_eq!(salida["appState"]["viewBackgroundColor"], "#121212");
    }

    #[test]
    fn un_background_color_de_arriba_que_no_se_entiende_se_ve_blanco_y_no_se_pisa() {
        // Caso negativo: como en el movil (`colorDe`), lo que no es un color
        // es blanco; pero si nadie elige otro papel, vuelve tal cual.
        for raro in [r#""transparent""#, r#""azulito""#, "42", "null"] {
            let json = format!(r#"{{"elements":[],"backgroundColor":{raro}}}"#);
            let lienzo = leer(&json).unwrap();
            let escena = a_escena(&lienzo);
            assert_eq!(escena.fondo, crate::escena::FONDO_DE_FABRICA, "{raro}");
            let salida = raiz(&escribir(&con_escena(&lienzo, &escena)));
            assert_eq!(
                salida["backgroundColor"],
                raiz(&json)["backgroundColor"],
                "{raro}"
            );
        }
    }

    #[test]
    fn un_lienzo_nuevo_con_papel_elegido_lleva_el_papel_arriba_para_el_movil_y_en_app_state_para_la_web()
     {
        let mut escena = crate::Escena::nueva();
        escena.poner_fondo(color_desde(Some(&Value::from("#f5faff"))).unwrap());
        let salida = raiz(&escribir(&con_escena(&Lienzo::vacio(), &escena)));
        assert_eq!(salida["backgroundColor"], "#f5faff");
        assert_eq!(salida["appState"]["viewBackgroundColor"], "#f5faff");
    }
}

/// **La vuelta**: un lienzo del movil con una foto girada, papel oscuro y
/// rayas encima, abierto aqui, tocado y guardado, no pierde nada de lo que
/// el movil puso (el giro, el papel, las rayas, sus campos propios).
#[cfg(test)]
mod vuelta_de_un_lienzo_del_movil {
    use super::*;

    const DEL_MOVIL: &str = r##"{"elements":[
      {"id":"img-1","type":"image","x":100,"y":150,"width":400,"height":300,"angle":1.5813086,
       "strokeColor":"#1e1e1e","backgroundColor":"transparent","fillStyle":"solid","strokeWidth":2,
       "strokeStyle":"solid","roughness":1,"opacity":100,"seed":1,"version":2,"versionNonce":0,
       "isDeleted":false,"groupIds":[],"updated":0,"locked":false,"formaSolida":"caja",
       "lupaRedonda":true,"periodos":6,"fileId":"Qwertyuiopasdfghjklzx","scale":[1,1]},
      {"id":"t-1","type":"freedraw","x":80,"y":80,"width":40,"height":40,"angle":0,
       "strokeColor":"#ffffff","backgroundColor":"transparent","strokeWidth":4,"seed":3,
       "version":1,"isDeleted":false,"points":[{"x":0,"y":0},{"x":40,"y":40}],"pressures":[],
       "simulatePressure":true,"material":"tiza","presionFirme":false}],
      "files":{"Qwertyuiopasdfghjklzx":{"id":"Qwertyuiopasdfghjklzx","mimeType":"image/png",
        "path":"pixpin:files/pins/draw/files/Qwertyuiopasdfghjklzx","created":1}},
      "viewport":{"scrollX":1,"scrollY":2,"zoom":0.5},"style":{"backgroundColor":"transparent"},
      "backgroundColor":"#121212","luces":{"encendidas":true,"fuerza":1},"tablas":[],
      "referenciasVisibles":true,"alfileres":[],"vista":"cero"}"##;

    fn raiz(json: &str) -> Value {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn anadir_una_raya_aqui_no_le_quita_nada_al_lienzo_del_movil() {
        let lienzo = leer(DEL_MOVIL).unwrap();
        assert_eq!(lienzo.cuantos_ajenos(), 0, "la foto y la tiza se entienden");
        let mut escena = a_escena(&lienzo);
        let foto = escena.elementos[0].clone();
        assert!((foto.angulo - 1.5813086).abs() < 1e-6, "el giro se lee");
        let mut raya = escena.elementos[1].clone();
        raya.x += 200.0;
        raya.extras.id_de_fichero = None;
        escena.anadir(raya);
        let antes = raiz(DEL_MOVIL);
        let despues = raiz(&escribir(&con_escena(&lienzo, &escena)));
        // Lo que no se toco sale identico, con sus campos propios.
        assert_eq!(despues["elements"][0], antes["elements"][0]);
        assert_eq!(despues["elements"][1], antes["elements"][1]);
        assert_eq!(despues["elements"].as_array().unwrap().len(), 3);
        for clave in [
            "backgroundColor",
            "files",
            "viewport",
            "luces",
            "vista",
            "style",
            "alfileres",
        ] {
            assert_eq!(despues[clave], antes[clave], "{clave}");
        }
        // Y la raya nueva va con los puntos como objetos, como las del movil.
        assert!(
            despues["elements"][2]["points"][0].is_object(),
            "{}",
            despues["elements"][2]
        );
    }

    #[test]
    fn mover_la_foto_girada_aqui_conserva_su_giro_y_sus_campos_del_movil() {
        let lienzo = leer(DEL_MOVIL).unwrap();
        let mut escena = a_escena(&lienzo);
        escena.elementos[0].x += 10.0;
        escena.elementos[0].version += 1;
        let despues = raiz(&escribir(&con_escena(&lienzo, &escena)));
        let foto = &despues["elements"][0];
        assert_eq!(foto["x"], 110.0);
        assert!(
            (foto["angle"].as_f64().unwrap() - 1.5813086).abs() < 1e-6,
            "{foto}"
        );
        assert_eq!(foto["fileId"], "Qwertyuiopasdfghjklzx");
        assert_eq!(
            foto["formaSolida"], "caja",
            "un campo propio del movil no se pierde"
        );
        assert_eq!(foto["periodos"], 6);
        assert_eq!(despues["backgroundColor"], "#121212");
    }
}
