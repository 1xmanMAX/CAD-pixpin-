//! Pasarle ficheros a la copia que ya esta corriendo.
//!
//! PixPin Max solo deja una instancia. Sin esto, abrir una imagen con
//! «Abrir con» no haria nada: la segunda copia veria el mutex tomado y se
//! iria en silencio, llevandose la ruta que le habian dado. El usuario
//! veria que no pasa NADA al abrir su fichero, que es lo peor que puede
//! hacer un programa.
//!
//! Asi que la segunda copia, antes de irse, busca la ventana de mensajes de
//! la primera y le manda las rutas con `WM_COPYDATA`.
//!
//! `WM_COPYDATA` y no un socket ni una tuberia: es lo unico que Windows
//! copia entre procesos por su cuenta, sin permisos, sin nombres que
//! chocar y sin nada que limpiar si el otro lado se cae.

use std::path::PathBuf;

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::DataExchange::COPYDATASTRUCT;
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, SendMessageW};
use windows::core::w;

/// El identificador de nuestro mensaje dentro de `WM_COPYDATA`.
///
/// Lo lleva `dwData`, y sirve para que otro programa que le mande
/// `WM_COPYDATA` a nuestra ventana por error no acabe pineando cosas.
pub const ABRIR_FICHEROS: usize = 0x5049_5850;

/// Las rutas, empaquetadas como texto UTF-16 separado por nulos.
///
/// Se separan por nulo y no por salto de linea porque un nombre de fichero
/// en Windows puede contener casi cualquier cosa MENOS un nulo: es el unico
/// separador que no puede aparecer dentro de una ruta y partirla en dos.
fn empaquetar(rutas: &[PathBuf]) -> Vec<u16> {
    let mut fuera = Vec::new();
    for ruta in rutas {
        fuera.extend(ruta.as_os_str().encode_wide());
        fuera.push(0);
    }
    fuera
}

/// Deshace lo que hizo `empaquetar`.
///
/// Aparte y pura para poder probarla: el desempaquetado es donde se cuelan
/// los fallos de uno de mas o de menos, y no necesita dos procesos para
/// comprobarse.
pub fn desempaquetar(unidades: &[u16]) -> Vec<PathBuf> {
    unidades
        .split(|u| *u == 0)
        .filter(|trozo| !trozo.is_empty())
        .map(|trozo| PathBuf::from(String::from_utf16_lossy(trozo)))
        .collect()
}

use std::os::windows::ffi::OsStrExt;

/// Le pide a la copia que ya corre que saque su ventana principal.
///
/// Devuelve si habia alguien escuchando. `id_comando` es el del catalogo
/// (`pixpin_store::comandos`), que este crate no conoce: lo pone quien llama.
///
/// Es lo que hace que volver a abrir PixPin lleve al chat en vez de no hacer
/// nada. Antes, la segunda copia se iba en silencio —correcto para no abrir
/// dos— y el usuario pulsaba el icono y no pasaba nada: «cuando abro la app
/// me lleve ahi directamente», con sus palabras.
pub fn pedir_ventana_principal(id_comando: u32) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_COMMAND};
    // SAFETY: la clase es un literal estatico terminado en cero; devuelve
    // una ventana nula si no hay ninguna, que se descarta abajo.
    let Ok(destino) = (unsafe { FindWindowW(w!("PixPinMaxVentanaMensajes"), None) }) else {
        return false;
    };
    if destino.0.is_null() {
        return false;
    }
    // `Post` y no `Send`: aqui no viaja memoria prestada (a diferencia de
    // `WM_COPYDATA`), y esta copia se va a morir enseguida; esperar a que la
    // otra termine de abrir una ventana entera seria esperar por nada.
    // SAFETY: mensaje sin punteros a una ventana de otro proceso propio.
    unsafe {
        PostMessageW(
            Some(destino),
            WM_COMMAND,
            WPARAM(id_comando as usize),
            LPARAM(0),
        )
    }
    .is_ok()
}

/// Manda las rutas a la instancia que ya corre. Devuelve si llegaron.
///
/// `false` significa que no hay nadie escuchando, y entonces quien llama
/// debe seguir arrancando con normalidad: puede que el mutex estuviera
/// tomado por una copia que se estaba cerrando justo en ese instante.
pub fn enviar_ficheros(rutas: &[PathBuf]) -> bool {
    if rutas.is_empty() {
        return false;
    }
    // SAFETY: la clase es un literal estatico terminado en cero. Devuelve
    // una ventana nula si no hay ninguna, que se descarta abajo.
    let destino = unsafe { FindWindowW(w!("PixPinMaxVentanaMensajes"), None) };
    let Ok(destino) = destino else { return false };
    if destino.0.is_null() {
        return false;
    }
    let datos = empaquetar(rutas);
    let paquete = COPYDATASTRUCT {
        dwData: ABRIR_FICHEROS,
        cbData: std::mem::size_of_val(datos.as_slice()) as u32,
        lpData: datos.as_ptr() as *mut _,
    };
    // SAFETY: SendMessageW es SINCRONO, asi que `datos` y `paquete` siguen
    // vivos durante toda la llamada. Con PostMessage esto seria memoria
    // liberada leida por el otro proceso: es el fallo clasico de
    // WM_COPYDATA y por eso Windows exige mandarlo con Send y no con Post.
    let respuesta = unsafe {
        SendMessageW(
            destino,
            windows::Win32::UI::WindowsAndMessaging::WM_COPYDATA,
            Some(WPARAM(0)),
            Some(LPARAM(&paquete as *const _ as isize)),
        )
    };
    // Y un toque a la cola. `WM_COPYDATA` entra DIRECTO al procedimiento de
    // ventana sin pasar por ella: el evento queda apuntado, pero el bucle de
    // la otra copia esta dormido en `GetMessage` esperando algo que ya paso.
    // Sin esto, lo que se le manda a una copia ya abierta no aparecia hasta
    // que el usuario tocaba cualquier otra cosa —y un proyecto recibido del
    // movil parecia perdido—.
    if respuesta.0 != 0 {
        crate::ventana::despertar(destino);
    }
    respuesta.0 != 0
}

/// El identificador de los **pedidos** (`docs/protocolo-pedidos.md`): el
/// siguiente de [`ABRIR_FICHEROS`]. Lo manda cualquier programa del equipo
/// que quiera que PixPin haga algo (el plugin de Flow Launcher, un script).
pub const PEDIDO_JSON: usize = 0x5049_5851;

/// Lo mas que puede medir un pedido. Un pedido es una orden corta, no un
/// documento: 64 KB caben de sobra en el texto mas largo que alguien escriba
/// en un lanzador, y un tope evita copiar megas que nadie deberia mandar.
pub const TOPE_PEDIDO: usize = 64 * 1024;

/// La version del protocolo que entiende esta copia (`"pixpin": 1`).
pub const VERSION_PEDIDO: u64 = 1;

/// Las respuestas inmediatas a un pedido (el `LRESULT` de `WM_COPYDATA`).
/// El `0` no esta: es lo que contesta Windows cuando no hay nadie o el
/// mensaje no era para nosotros.
pub mod respuesta {
    /// Aceptado: queda en la cola y el bucle lo hara enseguida.
    pub const ACEPTADO: isize = 1;
    /// Se pidio con una version del protocolo que esta copia no conoce.
    pub const VERSION_NO: isize = 2;
    /// JSON roto, demasiado grande o con una accion que no existe.
    pub const NO_SE_ENTIENDE: isize = 3;
}

/// Las acciones que existen en la version 1. Viven aqui, y no en la
/// aplicacion, porque la ventana tiene que contestar `3` en el acto a una
/// accion desconocida sin esperar al bucle. La aplicacion tiene una prueba
/// que comprueba que entiende todas (`pedidos.rs`): si una se anade aqui y
/// no alli, falla.
pub const ACCIONES: [&str; 37] = [
    "abrir",
    "chat",
    "nota_nueva",
    "lienzo_nuevo",
    "grabar",
    "lista_nueva",
    "anadir_tarea",
    "marcar_tarea",
    "ventana_principal",
    "pinear",
    "iconos",
    "soltar",
    "reproducir",
    "leccion_nueva",
    "lecciones",
    "mover_tarea",
    "ventana",
    "capturar",
    "pinear_ultima",
    "conservar_captura",
    "borrar_captura",
    "copiar_imagen",
    "quitar_tarea",
    "corregir_tarea",
    "borrar_lista",
    "enviar_al_movil",
    "sincronizar",
    // 13-oct: el asistente sin abrir el chat (`p ia …`) y sus
    // automatizaciones.
    "asistente",
    "automatizacion",
    // 14-oct: una tabla del chat ya rellena (el asistente, tabla escaneada).
    "tabla_nueva",
    // 15-oct: «+10 min» de la notificacion de un recordatorio
    // (`notificacion`).
    "posponer",
    // 15-oct: el asistente maneja los pines, el timeline y la voz.
    "pines",
    "timeline",
    // 15-oct: la extension del navegador (`apps/pixpin-puente`): leer en voz
    // alta un texto suelto y pasar el texto de una imagen al portapapeles.
    "leer_en_voz",
    "ocr",
    // 15-oct: una herramienta del asistente con sus argumentos y el
    // resultado por un fichero (el servidor MCP, `apps/pixpin-mcp`).
    "herramienta",
    // 17-oct: fotos y texto a un lienzo por su nombre, apilados.
    "al_lienzo",
];

/// Mira un pedido por encima: lo justo para contestar en el acto.
///
/// Solo comprueba que es un objeto JSON con `"pixpin": 1` y una `accion`
/// conocida; que los campos de esa accion esten bien lo mira la aplicacion
/// despues, en su bucle, y si falta algo lo dice con su aviso. Asi el
/// procedimiento de ventana no hace trabajo de verdad, pero un pedido
/// escrito a mano con una errata en la accion no se da por bueno.
///
/// Una version que no es un numero (o falta) es un pedido roto, no una
/// version distinta: sin `pixpin` no es un pedido nuestro.
pub fn validar_pedido(json: &str) -> isize {
    let Ok(serde_json::Value::Object(pedido)) = serde_json::from_str::<serde_json::Value>(json)
    else {
        return respuesta::NO_SE_ENTIENDE;
    };
    match pedido.get("pixpin").and_then(|v| v.as_u64()) {
        Some(VERSION_PEDIDO) => {}
        Some(_) => return respuesta::VERSION_NO,
        None => return respuesta::NO_SE_ENTIENDE,
    }
    match pedido.get("accion").and_then(|v| v.as_str()) {
        Some(a) if ACCIONES.contains(&a) => respuesta::ACEPTADO,
        _ => respuesta::NO_SE_ENTIENDE,
    }
}

/// Los bytes de un pedido, ya mirados: el texto si vale, o la respuesta con
/// la que se rechaza. Pura y aparte para probarla sin ventanas.
pub fn leer_pedido(bytes: &[u8]) -> Result<String, isize> {
    if bytes.len() > TOPE_PEDIDO {
        return Err(respuesta::NO_SE_ENTIENDE);
    }
    let Ok(texto) = std::str::from_utf8(bytes) else {
        return Err(respuesta::NO_SE_ENTIENDE);
    };
    // Un nulo final no lo pide el protocolo, pero un script en C lo pondria
    // sin pensar; quitarlo cuesta nada y evita rechazarle el pedido.
    let texto = texto.trim_end_matches('\0');
    match validar_pedido(texto) {
        respuesta::ACEPTADO => Ok(texto.to_string()),
        otra => Err(otra),
    }
}

/// Lee un pedido de un `WM_COPYDATA` recibido.
///
/// `None` si el mensaje no es un pedido (otro `dwData`): entonces lo mira
/// quien sigue, que puede ser [`ficheros_de_copydata`]. Si lo es, el texto
/// **copiado** o la respuesta con que se rechaza.
///
/// # Safety
///
/// Igual que [`ficheros_de_copydata`]: `lparam` tiene que ser el de un
/// `WM_COPYDATA` recien recibido, con su `COPYDATASTRUCT` vivo. Se copia
/// aqui y no se guarda el puntero.
pub unsafe fn pedido_de_copydata(lparam: LPARAM) -> Option<Result<String, isize>> {
    if lparam.0 == 0 {
        return None;
    }
    // SAFETY: el llamante garantiza que viene de un WM_COPYDATA vivo.
    let paquete = unsafe { &*(lparam.0 as *const COPYDATASTRUCT) };
    if paquete.dwData != PEDIDO_JSON {
        return None;
    }
    let cuantos = paquete.cbData as usize;
    // El tope se mira ANTES de leer: un pedido enorme se rechaza sin
    // copiarlo.
    if cuantos > TOPE_PEDIDO {
        return Some(Err(respuesta::NO_SE_ENTIENDE));
    }
    if cuantos == 0 || paquete.lpData.is_null() {
        return Some(Err(respuesta::NO_SE_ENTIENDE));
    }
    // SAFETY: el otro proceso escribio `cbData` bytes en esa direccion y
    // Windows los ha copiado a nuestro espacio mientras dura el mensaje;
    // `leer_pedido` hace su propia copia antes de volver.
    let bytes = unsafe { std::slice::from_raw_parts(paquete.lpData as *const u8, cuantos) };
    Some(leer_pedido(bytes))
}

/// La ventana de mensajes de la copia que corre, si hay una.
///
/// Primero `FindWindowW`, que es lo que usa todo lo demas y lo que se midio
/// en este equipo; si no da nada, se busca entre las ventanas de solo
/// mensajes (`HWND_MESSAGE`), que es donde Windows dice que viven y donde
/// `FindWindowW` no tiene por que mirar.
fn ventana_de_la_copia() -> Option<windows::Win32::Foundation::HWND> {
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowExW, HWND_MESSAGE};
    // SAFETY: la clase es un literal estatico terminado en cero; una
    // ventana nula significa que no hay ninguna.
    if let Ok(h) = unsafe { FindWindowW(w!("PixPinMaxVentanaMensajes"), None) }
        && !h.0.is_null()
    {
        return Some(h);
    }
    // SAFETY: igual que arriba; HWND_MESSAGE es el padre valido para buscar
    // entre las ventanas de solo mensajes.
    let h = unsafe {
        FindWindowExW(
            Some(HWND_MESSAGE),
            None,
            w!("PixPinMaxVentanaMensajes"),
            None,
        )
    }
    .ok()?;
    (!h.0.is_null()).then_some(h)
}

/// Manda un pedido a la copia que corre. Devuelve la respuesta inmediata
/// ([`respuesta`]), o `0` si no hay nadie escuchando.
///
/// Es lo que usan el plugin de Flow Launcher y quien quiera escribir el
/// suyo en Rust; esta aqui para que el que manda y el que recibe compartan
/// el mismo identificador y el mismo empaquetado.
pub fn enviar_pedido(json: &str) -> isize {
    match ventana_de_la_copia() {
        Some(destino) => enviar_pedido_a(destino, json),
        None => 0,
    }
}

/// Lo de [`enviar_pedido`] a una ventana concreta. Aparte para las pruebas,
/// que mandan a su propia ventana: buscarla por la clase encontraria la de
/// la PixPin del usuario si esta abierta, y le haria cosas de verdad.
///
/// Es el mismo camino que [`enviar_pedido_con_espera`] (un solo
/// empaquetado y un solo envio para todos los que mandan): con su espera de
/// [`ESPERA_RESPUESTA_MS`], para que una copia colgada no deje colgado
/// tambien al que manda. `0` si no contesto.
pub fn enviar_pedido_a(destino: windows::Win32::Foundation::HWND, json: &str) -> isize {
    enviar_pedido_con_espera(destino, json, ESPERA_RESPUESTA_MS).unwrap_or(0)
}

/// Lo que salio de mandar un pedido desde fuera (la linea de ordenes, los
/// enlaces, el menu del Explorador, y quien quiera reutilizarlo: el servidor
/// MCP, el puente del navegador). Los codigos son los de
/// `pixpin-lanzador pedido` y los de `orden::codigo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Envio {
    /// `1`: en la cola de la app.
    Aceptado,
    /// `2`: version del protocolo que esta copia no conoce.
    VersionNo,
    /// `3`: JSON roto o accion desconocida.
    NoSeEntiende,
    /// No hay PixPin abierta y no se pudo (o no se quiso) abrir.
    SinApp(String),
    /// No contesto a tiempo, o contesto algo que no es del protocolo.
    SinRespuesta(isize),
}

impl Envio {
    /// El codigo con que sale la orden.
    pub fn codigo_de_salida(&self) -> i32 {
        match self {
            Envio::Aceptado => 0,
            Envio::VersionNo => 2,
            Envio::NoSeEntiende => 3,
            Envio::SinApp(_) => 4,
            Envio::SinRespuesta(_) => 5,
        }
    }

    /// De la respuesta de la ventana.
    pub fn de_respuesta(r: isize) -> Envio {
        match r {
            respuesta::ACEPTADO => Envio::Aceptado,
            respuesta::VERSION_NO => Envio::VersionNo,
            respuesta::NO_SE_ENTIENDE => Envio::NoSeEntiende,
            otra => Envio::SinRespuesta(otra),
        }
    }
}

/// Cuanto se espera a que aparezca la ventana de una PixPin recien abierta
/// (lo mismo que el plugin de Flow).
pub const ESPERA_ARRANQUE: std::time::Duration = std::time::Duration::from_secs(8);

/// Cuanto se espera, como mucho, a que la app conteste un pedido. La
/// ventana contesta en el acto (solo mira el JSON por encima); si tarda
/// mas, esta colgada y no tiene sentido quedarse esperando con ella.
pub const ESPERA_RESPUESTA_MS: u32 = 5000;

/// **Solo para pruebas**: con esta variable, los pedidos de fuera buscan la
/// ventana de esa clase en vez de la de PixPin, y nunca abren la app. Asi
/// una prueba de punta a punta (`pixpinmax tarea …` contra una ventana de
/// mentira) no puede tocar la PixPin del usuario.
pub const VARIABLE_CLASE_DE_PRUEBA: &str = "PIXPIN_PEDIDOS_VENTANA";

fn clase_de_prueba() -> Option<String> {
    std::env::var(VARIABLE_CLASE_DE_PRUEBA)
        .ok()
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty())
}

fn ventana_para_pedidos() -> Option<windows::Win32::Foundation::HWND> {
    let Some(clase) = clase_de_prueba() else {
        return ventana_de_la_copia();
    };
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowExW, HWND_MESSAGE};
    let clase = windows::core::HSTRING::from(clase);
    // SAFETY: la clase es una HSTRING viva durante las llamadas; una ventana
    // nula es que no hay.
    unsafe {
        if let Ok(h) = FindWindowW(&clase, None)
            && !h.0.is_null()
        {
            return Some(h);
        }
        FindWindowExW(Some(HWND_MESSAGE), None, &clase, None)
            .ok()
            .filter(|h| !h.0.is_null())
    }
}

/// Manda un pedido a una ventana sin quedarse colgado si no contesta
/// (`SendMessageTimeoutW`). `None`: no contesto a tiempo.
pub fn enviar_pedido_con_espera(
    destino: windows::Win32::Foundation::HWND,
    json: &str,
    espera_ms: u32,
) -> Option<isize> {
    use windows::Win32::UI::WindowsAndMessaging::{
        SMTO_ABORTIFHUNG, SMTO_BLOCK, SendMessageTimeoutW, WM_COPYDATA,
    };
    if json.len() > TOPE_PEDIDO {
        return Some(respuesta::NO_SE_ENTIENDE);
    }
    let paquete = COPYDATASTRUCT {
        dwData: PEDIDO_JSON,
        cbData: json.len() as u32,
        lpData: json.as_ptr() as *mut _,
    };
    let mut r: usize = 0;
    // SAFETY: SendMessageTimeoutW es sincrono: `json` y `paquete` viven
    // durante toda la llamada, que es lo que exige WM_COPYDATA.
    let hecho = unsafe {
        SendMessageTimeoutW(
            destino,
            WM_COPYDATA,
            WPARAM(0),
            LPARAM(&paquete as *const _ as isize),
            SMTO_ABORTIFHUNG | SMTO_BLOCK,
            espera_ms,
            Some(&mut r),
        )
    };
    if hecho.0 == 0 {
        return None;
    }
    // El toque a la cola, como en `enviar_pedido_a`.
    if r != 0 {
        crate::ventana::despertar(destino);
    }
    Some(r as isize)
}

/// Abre `exe` sin ventana de consola y fuera del Job de quien nos lanzo (si
/// no, cerrar el lanzador se llevaria PixPin por delante).
fn abrir_la_app(exe: &std::path::Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    if !exe.is_file() {
        return Err(format!("no esta {}", exe.display()));
    }
    let mut ultimo = String::new();
    // Si el Job no deja salirse, CreateProcess falla con la bandera: se
    // reintenta sin ella.
    for banderas in [
        CREATE_BREAKAWAY_FROM_JOB | DETACHED_PROCESS,
        DETACHED_PROCESS,
    ] {
        let mut c = Command::new(exe);
        // Arranca para el pedido: sin el chat delante (el pedido abre lo suyo).
        c.arg("--sin-chat");
        c.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(banderas);
        if let Some(d) = exe.parent() {
            c.current_dir(d);
        }
        match c.spawn() {
            Ok(_) => return Ok(()),
            Err(e) => ultimo = e.to_string(),
        }
    }
    Err(ultimo)
}

/// **Manda pedidos a la PixPin que corre, y si no hay ninguna, la abre**
/// (`exe`) y la espera (hasta [`ESPERA_ARRANQUE`]). Es la funcion para
/// reutilizar desde fuera: la usan la linea de ordenes, los enlaces
/// `pixpin://` y el menu del Explorador, y es la que deberian usar el
/// servidor MCP o el puente del navegador.
///
/// - `exe` `None`: no abrir nada (si no esta, `SinApp`).
/// - Se mandan en orden y se para en el primero que no se acepte.
/// - Antes de mandar se le da permiso a la app para pasar al frente
///   (`AllowSetForegroundWindow`): quien nos lanzo (el Explorador, un
///   Stream Deck) tiene el foco, y sin el permiso la ventana que abra PixPin
///   se quedaria detras parpadeando.
pub fn enviar_pedidos_o_arrancar(pedidos: &[String], exe: Option<&std::path::Path>) -> Envio {
    use windows::Win32::UI::WindowsAndMessaging::{ASFW_ANY, AllowSetForegroundWindow};
    if pedidos.is_empty() || pedidos.iter().any(|p| p.len() > TOPE_PEDIDO) {
        return Envio::NoSeEntiende;
    }
    // SAFETY: sin punteros; falla sin efecto si no se puede dar.
    let _ = unsafe { AllowSetForegroundWindow(ASFW_ANY) };
    let mut espera = ESPERA_RESPUESTA_MS;
    let destino = match ventana_para_pedidos() {
        Some(h) => h,
        None => {
            // Con la ventana de prueba nunca se abre la app de verdad.
            let Some(exe) = exe.filter(|_| clase_de_prueba().is_none()) else {
                return Envio::SinApp("PixPin no esta abierta".into());
            };
            if let Err(e) = abrir_la_app(exe) {
                return Envio::SinApp(e);
            }
            let inicio = std::time::Instant::now();
            loop {
                std::thread::sleep(std::time::Duration::from_millis(50));
                if let Some(h) = ventana_para_pedidos() {
                    // Recien abierta, su bucle aun no corre: la ventana
                    // existe, pero contesta cuando acabe de arrancar.
                    espera = 15_000;
                    break h;
                }
                if inicio.elapsed() > ESPERA_ARRANQUE {
                    return Envio::SinApp("PixPin no aparecio en 8 s".into());
                }
            }
        }
    };
    for p in pedidos {
        match enviar_pedido_con_espera(destino, p, espera) {
            Some(respuesta::ACEPTADO) => {}
            Some(r) => return Envio::de_respuesta(r),
            None => return Envio::SinRespuesta(0),
        }
    }
    Envio::Aceptado
}

/// [`enviar_pedidos_o_arrancar`] de un solo pedido.
pub fn enviar_pedido_o_arrancar(json: &str, exe: Option<&std::path::Path>) -> Envio {
    enviar_pedidos_o_arrancar(&[json.to_string()], exe)
}

/// Lee las rutas de un `WM_COPYDATA` recibido.
///
/// Devuelve vacio si el mensaje no es nuestro. Otro programa puede mandarle
/// `WM_COPYDATA` a cualquier ventana, asi que se comprueba el
/// identificador antes de hacerle caso a nada.
///
/// # Safety
///
/// `lparam` tiene que ser el de un `WM_COPYDATA` recien recibido, con su
/// `COPYDATASTRUCT` todavia vivo. Windows lo garantiza durante el
/// procesamiento del mensaje y NO despues: copiar aqui, no guardar el
/// puntero.
pub unsafe fn ficheros_de_copydata(lparam: LPARAM) -> Vec<PathBuf> {
    if lparam.0 == 0 {
        return Vec::new();
    }
    // SAFETY: el llamante garantiza que viene de un WM_COPYDATA vivo.
    let paquete = unsafe { &*(lparam.0 as *const COPYDATASTRUCT) };
    if paquete.dwData != ABRIR_FICHEROS || paquete.lpData.is_null() {
        return Vec::new();
    }
    let cuantas = paquete.cbData as usize / 2;
    if cuantas == 0 {
        return Vec::new();
    }
    // SAFETY: el otro proceso escribio `cbData` bytes de UTF-16 en esa
    // direccion, y Windows los ha copiado a nuestro espacio para la
    // duracion del mensaje.
    let unidades = unsafe { std::slice::from_raw_parts(paquete.lpData as *const u16, cuantas) };
    desempaquetar(unidades)
}

/// Las rutas que vienen en la linea de mandatos, ya filtradas.
///
/// Solo las que existen: Windows pasa la ruta del propio ejecutable como
/// primer argumento, y ademas un acceso directo roto o un fichero borrado
/// entre el doble clic y el arranque no deben abrir un pin vacio.
pub fn rutas_de_los_argumentos() -> Vec<PathBuf> {
    std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .filter(|r| r.is_file())
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn una_lista_de_rutas_va_y_vuelve() {
        let rutas = vec![
            PathBuf::from(r"C:\Users\alguien\foto.png"),
            PathBuf::from(r"D:\videos\clip largo.mp4"),
        ];
        assert_eq!(desempaquetar(&empaquetar(&rutas)), rutas);
    }

    #[test]
    fn los_espacios_y_los_acentos_sobreviven() {
        // Van en UTF-16 justamente por esto: una ruta con acentos o con
        // caracteres de otro alfabeto tiene que llegar entera.
        let rutas = vec![PathBuf::from(r"C:\Fotos\año 2026\niño ñu.png")];
        assert_eq!(desempaquetar(&empaquetar(&rutas)), rutas);
    }

    #[test]
    fn una_lista_vacia_no_da_rutas() {
        // Caso negativo: sin esto, empaquetar nada y desempaquetarlo podria
        // dar una ruta vacia que luego se intentaria abrir.
        assert!(empaquetar(&[]).is_empty());
        assert!(desempaquetar(&[]).is_empty());
        assert!(desempaquetar(&[0, 0, 0]).is_empty());
    }

    #[test]
    fn un_pedido_bueno_se_acepta_con_campos_de_mas() {
        // Lo que no se conoce se ignora: un campo nuevo no sube la version.
        let json = r#"{"pixpin":1,"accion":"chat","texto":"hola","de_futuro":[1,2]}"#;
        assert_eq!(validar_pedido(json), respuesta::ACEPTADO);
        assert_eq!(leer_pedido(json.as_bytes()), Ok(json.to_string()));
    }

    #[test]
    fn todas_las_acciones_de_la_tabla_se_aceptan() {
        for a in ACCIONES {
            let json = format!(r#"{{"pixpin":1,"accion":"{a}"}}"#);
            assert_eq!(validar_pedido(&json), respuesta::ACEPTADO, "{a}");
        }
    }

    #[test]
    fn otra_version_se_contesta_con_dos() {
        assert_eq!(
            validar_pedido(r#"{"pixpin":2,"accion":"chat"}"#),
            respuesta::VERSION_NO
        );
    }

    #[test]
    fn lo_roto_o_desconocido_se_contesta_con_tres() {
        // Casos negativos: cada uno es una forma distinta de no ser un
        // pedido, y ninguno puede llegar al bucle.
        for json in [
            "",
            "no es json",
            "[1,2]",
            r#"{"accion":"chat"}"#,
            r#"{"pixpin":"1","accion":"chat"}"#,
            r#"{"pixpin":1}"#,
            r#"{"pixpin":1,"accion":"borrar_todo"}"#,
            r#"{"pixpin":1,"accion":7}"#,
        ] {
            assert_eq!(validar_pedido(json), respuesta::NO_SE_ENTIENDE, "{json}");
        }
    }

    #[test]
    fn un_pedido_demasiado_grande_o_sin_utf8_no_se_lee() {
        let mut grande = br#"{"pixpin":1,"accion":"chat","texto":""#.to_vec();
        grande.extend(std::iter::repeat_n(b'a', TOPE_PEDIDO));
        grande.extend(br#""}"#);
        assert_eq!(leer_pedido(&grande), Err(respuesta::NO_SE_ENTIENDE));
        assert_eq!(
            leer_pedido(&[0xFF, 0xFE, b'{']),
            Err(respuesta::NO_SE_ENTIENDE)
        );
    }

    #[test]
    fn el_nulo_final_de_un_script_en_c_no_estorba() {
        let json = "{\"pixpin\":1,\"accion\":\"ventana_principal\"}\0";
        assert_eq!(
            leer_pedido(json.as_bytes()),
            Ok(json.trim_end_matches('\0').to_string())
        );
    }

    #[test]
    fn un_texto_con_acentos_llega_entero() {
        let json = r#"{"pixpin":1,"accion":"anadir_tarea","texto":"comprar pan y ñoquis — 2 kg"}"#;
        assert_eq!(leer_pedido(json.as_bytes()), Ok(json.to_string()));
    }

    #[test]
    fn los_nulos_de_mas_no_inventan_rutas() {
        // Caso negativo del separador: dos nulos seguidos no son una ruta
        // vacia entre medias, son el final de una y el relleno del paquete.
        let unidades: Vec<u16> = "C:\\a.png\0\0\0D:\\b.png\0".encode_utf16().collect();
        assert_eq!(
            desempaquetar(&unidades),
            vec![PathBuf::from(r"C:\a.png"), PathBuf::from(r"D:\b.png")]
        );
    }
}
