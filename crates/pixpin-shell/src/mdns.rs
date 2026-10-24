//! Anunciar y buscar servicios DNS-SD por mDNS en la red local.
//!
//! El movil usa el `NsdManager` de Android: anuncia `_pixpin._tcp` con el
//! nombre `PixPin <nombre>`, su puerto y un TXT (`g`, `id`, `l`, `n`), y
//! busca ese mismo tipo. Para que el PC y el movil se vean sin que el usuario
//! teclee direcciones, aqui se habla el mismo protocolo con la API nativa de
//! Windows (`DnsServiceRegister`, `DnsServiceBrowse`, `DnsServiceResolve`).
//! Se usa la del sistema y no un respondedor propio porque Windows ya tiene
//! el puerto 5353 abierto en su servicio de DNS: un segundo respondedor en
//! el mismo puerto compite con el y depende del firewall.
//!
//! Las tres operaciones son asincronas: Windows llama a una funcion
//! `extern "system"` con un puntero de contexto desde un hilo suyo. Ese
//! contexto es un `Arc<Buzon<_>>` convertido a puntero crudo; la
//! referencia cruda solo se suelta cuando Windows ya no puede volver a
//! llamar (tras la llamada final, o tras cancelar y verla llegar). Si esa
//! llamada final no llega a tiempo, se deja sin liberar a proposito: un poco
//! de memoria perdida es mejor que una llamada sobre memoria ya liberada.

use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{DNS_REQUEST_PENDING, GetLastError, HANDLE};
use windows::Win32::NetworkManagement::Dns::{
    DNS_QUERY_REQUEST_VERSION1, DNS_RECORDW, DNS_SERVICE_BROWSE_REQUEST,
    DNS_SERVICE_BROWSE_REQUEST_0, DNS_SERVICE_CANCEL, DNS_SERVICE_INSTANCE,
    DNS_SERVICE_REGISTER_REQUEST, DNS_SERVICE_RESOLVE_REQUEST, DNS_TYPE_PTR, DnsFree,
    DnsFreeRecordList, DnsServiceBrowse, DnsServiceBrowseCancel, DnsServiceConstructInstance,
    DnsServiceDeRegister, DnsServiceFreeInstance, DnsServiceRegister, DnsServiceResolve,
    DnsServiceResolveCancel,
};
use windows::Win32::System::SystemInformation::{ComputerNameDnsHostname, GetComputerNameExW};
use windows::core::{PCWSTR, PWSTR};

/// Cuanto se espera a que Windows confirme el anuncio. En la practica tarda
/// lo que dura el sondeo de conflictos de mDNS (unos cientos de ms).
const ESPERA_REGISTRO: Duration = Duration::from_secs(5);
/// Cuanto se espera la llamada final tras cancelar o desregistrar.
const ESPERA_CIERRE: Duration = Duration::from_secs(3);
/// Tiempo extra para terminar de resolver lo que aparecio al final de la
/// busqueda: sin el, un vecino visto en el ultimo instante se perderia.
const GRACIA_RESOLVER: Duration = Duration::from_secs(1);
/// Una etiqueta DNS mide como mucho 63 bytes (RFC 1035).
const MAXIMO_ETIQUETA: usize = 63;
/// Cada cadena del TXT (`clave=valor`) cabe en un byte de largo.
const MAXIMO_ENTRADA_TXT: usize = 255;

/// Un servicio encontrado en la red y ya resuelto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vecino {
    /// El nombre corto de la instancia, sin el tipo ni `.local`
    /// (`"PixPin Max"`), igual que el `serviceName` de Android.
    pub instancia: String,
    pub host: Ipv4Addr,
    pub puerto: u16,
    /// El TXT; las claves en minusculas porque en DNS-SD no distinguen.
    pub datos: HashMap<String, String>,
}

/// Errores de anunciar o buscar.
#[derive(Debug, thiserror::Error)]
pub enum ErrorMdns {
    /// El tipo no tiene la forma `_servicio._tcp` o `_servicio._udp`.
    #[error("tipo de servicio DNS-SD no valido: {0:?}")]
    TipoNoValido(String),
    /// El nombre de la instancia esta vacio.
    #[error("nombre de instancia no valido: {0:?}")]
    NombreNoValido(String),
    /// Una clave del TXT vacia, con `=`, no imprimible, o la entrada es
    /// demasiado larga.
    #[error("dato TXT no valido: {0:?}")]
    DatoNoValido(String),
    /// Una llamada de Windows devolvio un codigo de error.
    #[error("{operacion} fallo con el codigo {codigo}")]
    Windows {
        operacion: &'static str,
        codigo: u32,
    },
    /// Windows no confirmo el anuncio a tiempo.
    #[error("Windows no confirmo el anuncio en {0:?}")]
    SinRespuesta(Duration),
    /// No se pudo leer el nombre del equipo para el host del anuncio.
    #[error("no se pudo leer el nombre del equipo: {0}")]
    NombreDelEquipo(windows::core::Error),
}

// ---------------------------------------------------------------------------
// Nombres y TXT: logica pura, probada sin red.
// ---------------------------------------------------------------------------

/// Deja el tipo como `_servicio._proto` en minusculas. Acepta la forma de
/// Android (`"_pixpin._tcp."`) y la que ya lleva `.local`, porque las dos
/// aparecen en la practica y significan lo mismo.
fn normalizar_tipo(tipo: &str) -> Result<String, ErrorMdns> {
    let rechazo = || ErrorMdns::TipoNoValido(tipo.to_owned());
    let mut t = tipo.trim().trim_end_matches('.').to_ascii_lowercase();
    if let Some(sin) = t.strip_suffix(".local") {
        t = sin.to_owned();
    }
    let (servicio, protocolo) = t.split_once('.').ok_or_else(rechazo)?;
    if protocolo != "_tcp" && protocolo != "_udp" {
        return Err(rechazo());
    }
    // RFC 6335: el nombre del servicio tiene de 1 a 15 caracteres, letras,
    // cifras y guiones.
    let nombre = servicio.strip_prefix('_').ok_or_else(rechazo)?;
    if nombre.is_empty()
        || nombre.len() > 15
        || !nombre
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(rechazo());
    }
    Ok(t)
}

/// El nombre que se pide al buscar: `_pixpin._tcp.local`.
fn nombre_de_consulta(tipo: &str) -> String {
    format!("{tipo}.local")
}

/// El nombre de host del anuncio: el del equipo mas `.local`.
fn nombre_de_host(equipo: &str) -> String {
    format!("{equipo}.local")
}

/// Recorta a 63 bytes sin partir una letra: Android permite nombres largos
/// con acentos y una etiqueta mas larga la rechazaria Windows.
fn recortar_etiqueta(nombre: &str) -> &str {
    if nombre.len() <= MAXIMO_ETIQUETA {
        return nombre;
    }
    let mut corte = MAXIMO_ETIQUETA;
    while !nombre.is_char_boundary(corte) {
        corte -= 1;
    }
    &nombre[..corte]
}

/// `<nombre>._pixpin._tcp.local`, con los puntos y barras del nombre
/// escapados (RFC 6763 §4.3): un punto sin escapar partiria la etiqueta y el
/// nombre dejaria de ser una instancia del tipo.
fn nombre_de_instancia(nombre: &str, tipo: &str) -> Result<String, ErrorMdns> {
    let limpio = recortar_etiqueta(nombre.trim());
    if limpio.is_empty() {
        return Err(ErrorMdns::NombreNoValido(nombre.to_owned()));
    }
    let mut escapado = String::with_capacity(limpio.len() + 4);
    for c in limpio.chars() {
        if c == '.' || c == '\\' {
            escapado.push('\\');
        }
        escapado.push(c);
    }
    Ok(format!("{escapado}.{}", nombre_de_consulta(tipo)))
}

/// De `PixPin Max._pixpin._tcp.local` a `PixPin Max`, deshaciendo los
/// escapes `\.`, `\\` y `\DDD`. `None` si el nombre no es de ese tipo.
fn instancia_corta(completo: &str, tipo: &str) -> Option<String> {
    let completo = completo.trim_end_matches('.');
    let sufijo = format!(".{}", nombre_de_consulta(tipo));
    if completo.len() <= sufijo.len()
        || !completo.is_char_boundary(completo.len() - sufijo.len())
        || !completo[completo.len() - sufijo.len()..].eq_ignore_ascii_case(&sufijo)
    {
        return None;
    }
    let crudo = &completo[..completo.len() - sufijo.len()];
    let mut bytes = Vec::with_capacity(crudo.len());
    let b = crudo.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' && i + 1 < b.len() {
            let tres = b.get(i + 1..i + 4);
            if let Some(d) = tres
                && d.iter().all(u8::is_ascii_digit)
            {
                let valor =
                    (d[0] - b'0') as u32 * 100 + (d[1] - b'0') as u32 * 10 + (d[2] - b'0') as u32;
                if let Ok(byte) = u8::try_from(valor) {
                    bytes.push(byte);
                    i += 4;
                    continue;
                }
            }
            bytes.push(b[i + 1]);
            i += 2;
        } else {
            bytes.push(b[i]);
            i += 1;
        }
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// Comprueba el TXT antes de anunciar: una clave con `=` o vacia no la
/// entiende ningun lector, y una entrada de mas de 255 bytes no cabe.
fn validar_datos(datos: &[(&str, &str)]) -> Result<(), ErrorMdns> {
    for (clave, valor) in datos {
        let clave_valida = !clave.is_empty()
            && clave
                .bytes()
                .all(|b| (0x20..=0x7e).contains(&b) && b != b'=');
        if !clave_valida || clave.len() + 1 + valor.len() > MAXIMO_ENTRADA_TXT {
            return Err(ErrorMdns::DatoNoValido(format!("{clave}={valor}")));
        }
    }
    Ok(())
}

/// Convierte los pares que devuelve Windows en el mapa del TXT.
///
/// Windows suele darlos ya partidos, pero si una entrada llega entera como
/// clave (`"g=0123"` con valor vacio) se parte aqui por el primer `=`. Las
/// claves van a minusculas y, si se repiten, gana la primera (RFC 6763
/// §6.4); las vacias se descartan.
fn datos_txt(pares: impl IntoIterator<Item = (String, String)>) -> HashMap<String, String> {
    let mut datos = HashMap::new();
    for (clave, valor) in pares {
        let (clave, valor) = match (valor.is_empty(), clave.split_once('=')) {
            (true, Some((c, v))) => (c.to_owned(), v.to_owned()),
            _ => (clave, valor),
        };
        let clave = clave.trim().to_ascii_lowercase();
        if clave.is_empty() {
            continue;
        }
        datos.entry(clave).or_insert(valor);
    }
    datos
}

/// Deja un vecino por instancia: con varias tarjetas de red la misma
/// instancia se resuelve una vez por cada una. Se queda la primera.
fn sin_repetidos(vecinos: Vec<Vecino>) -> Vec<Vecino> {
    let mut vistos = HashSet::new();
    vecinos
        .into_iter()
        .filter(|v| vistos.insert(v.instancia.to_lowercase()))
        .collect()
}

/// Una cadena como UTF-16 terminada en cero, para pasarla a Windows.
fn ancho(texto: &str) -> Vec<u16> {
    texto.encode_utf16().chain(std::iter::once(0)).collect()
}

fn nombre_del_equipo() -> Result<String, ErrorMdns> {
    let mut buffer = [0u16; 256];
    let mut largo = buffer.len() as u32;
    // SAFETY: el buffer es nuestro y `largo` dice su capacidad en u16; a la
    // vuelta `largo` son los caracteres escritos sin el cero final.
    unsafe {
        GetComputerNameExW(
            ComputerNameDnsHostname,
            Some(PWSTR(buffer.as_mut_ptr())),
            &mut largo,
        )
    }
    .map_err(ErrorMdns::NombreDelEquipo)?;
    Ok(String::from_utf16_lossy(&buffer[..largo as usize]))
}

// ---------------------------------------------------------------------------
// El buzon: lo que escriben las llamadas de Windows y lee el hilo que espera.
// ---------------------------------------------------------------------------

struct Dentro<T> {
    mensajes: Vec<T>,
    /// Cuantas veces ha llamado Windows.
    llamadas: usize,
    /// Windows ya hizo su llamada final y no volvera a llamar.
    fin: bool,
}

struct Buzon<T> {
    dentro: Mutex<Dentro<T>>,
    cambio: Condvar,
    /// La instancia que se paso a `DnsServiceRegister` (0 si no hay).
    propia: AtomicUsize,
}

impl<T> Buzon<T> {
    fn nuevo() -> Arc<Self> {
        Arc::new(Self {
            propia: AtomicUsize::new(0),
            dentro: Mutex::new(Dentro {
                mensajes: Vec::new(),
                llamadas: 0,
                fin: false,
            }),
            cambio: Condvar::new(),
        })
    }

    /// Un veneno aqui solo puede venir de un panico en otro hilo que ya
    /// aborto el proceso (las llamadas `extern "system"` no desenrollan), asi
    /// que los datos siguen siendo utiles.
    fn cerrojo(&self) -> MutexGuard<'_, Dentro<T>> {
        self.dentro.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn anotar(&self, mensaje: Option<T>, fin: bool) {
        let mut d = self.cerrojo();
        d.llamadas += 1;
        d.fin |= fin;
        d.mensajes.extend(mensaje);
        // Avisar con el cerrojo tomado: quien espera no puede soltar el
        // buzon hasta que este hilo lo libere.
        self.cambio.notify_all();
    }

    /// Espera hasta que se cumpla `listo` o llegue `hasta`; devuelve el
    /// cerrojo para que quien llama lea el estado sin carreras.
    fn esperar(
        &self,
        hasta: Instant,
        listo: impl Fn(&Dentro<T>) -> bool,
    ) -> MutexGuard<'_, Dentro<T>> {
        let mut d = self.cerrojo();
        loop {
            if listo(&d) {
                return d;
            }
            let queda = hasta.saturating_duration_since(Instant::now());
            if queda.is_zero() {
                return d;
            }
            d = self
                .cambio
                .wait_timeout(d, queda)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }
}

/// Entrega una referencia del `Arc` a Windows como puntero de contexto.
fn a_contexto<T>(buzon: &Arc<Buzon<T>>) -> *mut c_void {
    Arc::into_raw(Arc::clone(buzon)) as *mut c_void
}

/// Recupera el buzon dentro de una llamada de Windows sin consumir la
/// referencia cruda: la llamada tiene su propia copia mientras dura.
///
/// # Safety
/// `contexto` salio de `a_contexto::<T>` y su referencia cruda aun no se ha
/// soltado con `soltar_contexto`.
unsafe fn buzon_del_contexto<T>(contexto: *const c_void) -> Arc<Buzon<T>> {
    let p = contexto as *const Buzon<T>;
    // SAFETY: por el contrato de la funcion, `p` es un `Arc` vivo; sumar una
    // referencia y reconstruirla deja el contador igual al soltar la copia.
    unsafe {
        Arc::increment_strong_count(p);
        Arc::from_raw(p)
    }
}

/// Suelta la referencia que tenia Windows.
///
/// # Safety
/// `contexto` salio de `a_contexto::<T>`, se suelta una sola vez, y Windows
/// ya no puede llamar con el.
unsafe fn soltar_contexto<T>(contexto: *mut c_void) {
    // SAFETY: por el contrato, es la referencia de `a_contexto` y es la
    // ultima vez que se usa.
    drop(unsafe { Arc::from_raw(contexto as *const Buzon<T>) });
}

/// Lo que se saca de una `DNS_SERVICE_INSTANCE` resuelta.
struct Resuelto {
    completo: String,
    ip4: Option<Ipv4Addr>,
    puerto: u16,
    pares: Vec<(String, String)>,
}

fn texto_de(p: PWSTR) -> String {
    if p.is_null() {
        return String::new();
    }
    // SAFETY: Windows entrega cadenas terminadas en cero validas mientras
    // viva la instancia, que aun no se ha liberado.
    String::from_utf16_lossy(unsafe { p.as_wide() })
}

/// # Safety
/// `instancia` apunta a una `DNS_SERVICE_INSTANCE` valida.
unsafe fn leer_instancia(instancia: *const DNS_SERVICE_INSTANCE) -> Resuelto {
    // SAFETY: por el contrato de la funcion.
    let i = unsafe { &*instancia };
    let ip4 = if i.ip4Address.is_null() {
        None
    } else {
        // SAFETY: puntero no nulo de la instancia. `IP4_ADDRESS` esta en el
        // orden de la red: sus bytes en memoria son los octetos en orden.
        Some(Ipv4Addr::from(unsafe { *i.ip4Address }.to_ne_bytes()))
    };
    let mut pares = Vec::new();
    if !i.keys.is_null() && !i.values.is_null() {
        for n in 0..i.dwPropertyCount as usize {
            // SAFETY: `keys` y `values` tienen `dwPropertyCount` elementos.
            let (clave, valor) = unsafe { (*i.keys.add(n), *i.values.add(n)) };
            pares.push((texto_de(clave), texto_de(valor)));
        }
    }
    Resuelto {
        completo: texto_de(i.pszInstanceName),
        ip4,
        puerto: i.wPort,
        pares,
    }
}

// ---------------------------------------------------------------------------
// Anunciar.
// ---------------------------------------------------------------------------

/// Mientras viva, el servicio esta anunciado; al soltarlo se retira.
pub struct Anuncio {
    /// La peticion de registro; `DnsServiceDeRegister` pide la misma.
    peticion: Box<DNS_SERVICE_REGISTER_REQUEST>,
    instancia: *mut DNS_SERVICE_INSTANCE,
    contexto: *mut c_void,
    buzon: Arc<Buzon<u32>>,
    /// Llamadas de Windows vistas antes de desregistrar.
    llamadas_al_registrar: usize,
}

// SAFETY: los punteros crudos son de Windows o nuestros y no dependen del
// hilo: desregistrar y liberar desde otro hilo es valido, y `Anuncio` no se
// comparte (no es `Sync`), solo se mueve.
unsafe impl Send for Anuncio {}

impl std::fmt::Debug for Anuncio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Anuncio").finish_non_exhaustive()
    }
}

unsafe extern "system" fn al_registrar(
    estado: u32,
    contexto: *const c_void,
    instancia: *const DNS_SERVICE_INSTANCE,
) {
    // SAFETY: el contexto es el de `anunciar`, vivo hasta ver la llamada
    // que sigue a desregistrar (o nunca soltado si no llega).
    let buzon = unsafe { buzon_del_contexto::<u32>(contexto) };
    // Si Windows devolviera la misma instancia que le dimos, liberarla aqui
    // y otra vez en `Drop` seria doble liberacion: se comprueba.
    let propia = buzon.propia.load(Ordering::SeqCst) == instancia as usize;
    if !instancia.is_null() && !propia {
        // SAFETY: Windows entrega aqui una copia suya que es de quien la
        // recibe (asi lo hace su ejemplo) y no es la nuestra.
        unsafe { DnsServiceFreeInstance(instancia) };
    }
    buzon.anotar(Some(estado), false);
}

/// Anuncia `nombre` como instancia de `tipo` (`"_pixpin._tcp"`) en
/// `puerto`, con `datos` en el TXT. Bloquea hasta que Windows confirma el
/// anuncio (normalmente menos de un segundo, como mucho 5 s).
pub fn anunciar(
    tipo: &str,
    nombre: &str,
    puerto: u16,
    datos: &[(&str, &str)],
) -> Result<Anuncio, ErrorMdns> {
    let tipo = normalizar_tipo(tipo)?;
    validar_datos(datos)?;
    let instancia = ancho(&nombre_de_instancia(nombre, &tipo)?);
    let host = ancho(&nombre_de_host(&nombre_del_equipo()?));
    let claves: Vec<Vec<u16>> = datos.iter().map(|(c, _)| ancho(c)).collect();
    let valores: Vec<Vec<u16>> = datos.iter().map(|(_, v)| ancho(v)).collect();
    let p_claves: Vec<PCWSTR> = claves.iter().map(|c| PCWSTR(c.as_ptr())).collect();
    let p_valores: Vec<PCWSTR> = valores.iter().map(|v| PCWSTR(v.as_ptr())).collect();

    // Sin direcciones: Windows contesta el A/AAAA del host con las de cada
    // tarjeta por la que responde, que es justo lo que hay que anunciar en
    // un equipo con varias redes.
    // SAFETY: todas las cadenas estan terminadas en cero y viven durante la
    // llamada; los dos arrays tienen `datos.len()` elementos. Windows copia
    // lo que necesita en la instancia que devuelve.
    let construida = unsafe {
        DnsServiceConstructInstance(
            PCWSTR(instancia.as_ptr()),
            PCWSTR(host.as_ptr()),
            None,
            None,
            puerto,
            0,
            0,
            datos.len() as u32,
            p_claves.as_ptr(),
            p_valores.as_ptr(),
        )
    };
    if construida.is_null() {
        // SAFETY: consulta del ultimo error del hilo, sin precondiciones.
        let codigo = unsafe { GetLastError() }.0;
        return Err(ErrorMdns::Windows {
            operacion: "DnsServiceConstructInstance",
            codigo,
        });
    }

    let buzon = Buzon::<u32>::nuevo();
    buzon.propia.store(construida as usize, Ordering::SeqCst);
    let contexto = a_contexto(&buzon);
    let peticion = Box::new(DNS_SERVICE_REGISTER_REQUEST {
        Version: DNS_QUERY_REQUEST_VERSION1.0,
        InterfaceIndex: 0,
        pServiceInstance: construida,
        pRegisterCompletionCallback: Some(al_registrar),
        pQueryContext: contexto,
        hCredentials: HANDLE::default(),
        unicastEnabled: false.into(),
    });
    // SAFETY: la peticion, la instancia y el contexto viven en el monton y
    // no se liberan hasta ver la llamada final (ver `Drop`).
    let r = unsafe { DnsServiceRegister(&*peticion, None) };
    if r != DNS_REQUEST_PENDING as u32 {
        // Sin operacion en marcha Windows no llamara: se libera ya.
        // SAFETY: el contexto no llego a usarse y la instancia es nuestra.
        unsafe {
            soltar_contexto::<u32>(contexto);
            DnsServiceFreeInstance(construida);
        }
        return Err(ErrorMdns::Windows {
            operacion: "DnsServiceRegister",
            codigo: r,
        });
    }

    let limite = Instant::now() + ESPERA_REGISTRO;
    let (estado, llamadas) = {
        let d = buzon.esperar(limite, |d| d.llamadas > 0);
        (d.mensajes.first().copied(), d.llamadas)
    };
    if let Some(codigo) = estado
        && codigo != 0
    {
        // El registro fallo y Windows ya dio su llamada final: nada que
        // desregistrar.
        // SAFETY: tras la llamada con error la operacion termino; la
        // peticion se suelta al salir, despues de esto.
        unsafe {
            soltar_contexto::<u32>(contexto);
            DnsServiceFreeInstance(construida);
        }
        return Err(ErrorMdns::Windows {
            operacion: "DnsServiceRegister",
            codigo,
        });
    }
    let anuncio = Anuncio {
        peticion,
        instancia: construida,
        contexto,
        buzon,
        llamadas_al_registrar: llamadas,
    };
    if estado.is_none() {
        // Puede que el registro siga en marcha: `Drop` lo retira.
        drop(anuncio);
        return Err(ErrorMdns::SinRespuesta(ESPERA_REGISTRO));
    }
    Ok(anuncio)
}

impl Drop for Anuncio {
    fn drop(&mut self) {
        let antes = self
            .buzon
            .cerrojo()
            .llamadas
            .max(self.llamadas_al_registrar);
        // SAFETY: la misma peticion del registro, viva en el monton.
        let r = unsafe { DnsServiceDeRegister(&*self.peticion, None) };
        let terminado = r == DNS_REQUEST_PENDING as u32 && {
            let limite = Instant::now() + ESPERA_CIERRE;
            self.buzon.esperar(limite, |d| d.llamadas > antes).llamadas > antes
        };
        if terminado {
            // SAFETY: Windows confirmo la baja con su llamada final; ya no
            // usara ni el contexto ni la instancia.
            unsafe {
                soltar_contexto::<u32>(self.contexto);
                DnsServiceFreeInstance(self.instancia);
            }
        } else {
            // Sin confirmacion no se sabe si Windows aun los usa: se dejan
            // vivos a proposito (peticion incluida).
            tracing::warn!(codigo = r, "mdns: la baja del anuncio no se confirmo");
            let peticion = std::mem::replace(
                &mut self.peticion,
                Box::new(DNS_SERVICE_REGISTER_REQUEST::default()),
            );
            std::mem::forget(peticion);
        }
    }
}

// ---------------------------------------------------------------------------
// Buscar y resolver.
// ---------------------------------------------------------------------------

unsafe extern "system" fn al_navegar(
    estado: u32,
    contexto: *const c_void,
    registros: *const DNS_RECORDW,
) {
    // SAFETY: contexto de `buscar`, vivo hasta ver la llamada final.
    let buzon = unsafe { buzon_del_contexto::<Vec<String>>(contexto) };
    let mut nombres = Vec::new();
    let mut r = registros;
    while !r.is_null() {
        // SAFETY: lista enlazada de Windows, valida hasta liberarla abajo.
        let registro = unsafe { &*r };
        // TTL 0 es una despedida (el servicio se va), no un vecino nuevo.
        if registro.wType == DNS_TYPE_PTR.0 && registro.dwTtl != 0 {
            // SAFETY: para un PTR el dato valido de la union es `PTR`.
            nombres.push(texto_de(unsafe { registro.Data.PTR.pNameHost }));
        }
        r = registro.pNext;
    }
    if !registros.is_null() {
        // SAFETY: la lista es de quien recibe la llamada y ya no se usa.
        unsafe { DnsFree(Some(registros as *const c_void), DnsFreeRecordList) };
    }
    // Cualquier estado distinto de exito termina la busqueda; el normal es
    // ERROR_CANCELLED tras `DnsServiceBrowseCancel`.
    buzon.anotar(Some(nombres), estado != 0);
}

unsafe extern "system" fn al_resolver(
    estado: u32,
    contexto: *const c_void,
    instancia: *const DNS_SERVICE_INSTANCE,
) {
    // SAFETY: contexto de `Resolucion::empezar`, vivo hasta esta llamada,
    // que es la unica de una resolucion.
    let buzon = unsafe { buzon_del_contexto::<Option<Resuelto>>(contexto) };
    let resuelto = if instancia.is_null() {
        None
    } else {
        // SAFETY: instancia entregada por Windows, aun sin liberar.
        let leido = (estado == 0).then(|| unsafe { leer_instancia(instancia) });
        // SAFETY: la instancia es de quien recibe la llamada.
        unsafe { DnsServiceFreeInstance(instancia) };
        leido
    };
    buzon.anotar(Some(resuelto), true);
}

/// Una resolucion en marcha. Lo que Windows puede tocar vive en el monton.
struct Resolucion {
    _nombre: Vec<u16>,
    _peticion: Box<DNS_SERVICE_RESOLVE_REQUEST>,
    cancelar: Box<DNS_SERVICE_CANCEL>,
    contexto: *mut c_void,
    buzon: Arc<Buzon<Option<Resuelto>>>,
}

impl Resolucion {
    fn empezar(completo: &str) -> Result<Self, u32> {
        let mut nombre = ancho(completo);
        let buzon = Buzon::nuevo();
        let contexto = a_contexto(&buzon);
        let peticion = Box::new(DNS_SERVICE_RESOLVE_REQUEST {
            Version: DNS_QUERY_REQUEST_VERSION1.0,
            InterfaceIndex: 0,
            QueryName: PWSTR(nombre.as_mut_ptr()),
            pResolveCompletionCallback: Some(al_resolver),
            pQueryContext: contexto,
        });
        let mut cancelar = Box::new(DNS_SERVICE_CANCEL::default());
        // SAFETY: nombre, peticion y cancelacion viven en el monton dentro de
        // la `Resolucion`, que no los suelta hasta la llamada final.
        let r = unsafe { DnsServiceResolve(&*peticion, &mut *cancelar) };
        if r != DNS_REQUEST_PENDING {
            // SAFETY: sin operacion en marcha no habra llamada.
            unsafe { soltar_contexto::<Option<Resuelto>>(contexto) };
            return Err(r as u32);
        }
        Ok(Self {
            _nombre: nombre,
            _peticion: peticion,
            cancelar,
            contexto,
            buzon,
        })
    }

    /// Espera el resultado hasta `limite`; si no llega, cancela.
    fn terminar(self, limite: Instant) -> Option<Resuelto> {
        let fin = self.buzon.esperar(limite, |d| d.fin).fin;
        if !fin {
            // SAFETY: la cancelacion es la de esta operacion, viva.
            unsafe { DnsServiceResolveCancel(&*self.cancelar) };
            let limite = Instant::now() + ESPERA_CIERRE;
            if !self.buzon.esperar(limite, |d| d.fin).fin {
                tracing::warn!("mdns: una resolucion no confirmo su cancelacion");
                std::mem::forget(self);
                return None;
            }
        }
        // SAFETY: Windows ya hizo la llamada final de esta resolucion.
        unsafe { soltar_contexto::<Option<Resuelto>>(self.contexto) };
        self.buzon.cerrojo().mensajes.pop().flatten()
    }
}

/// Busca durante `espera` y devuelve lo resuelto (sin repetidos por
/// instancia). Bloquea: se llama desde un hilo.
///
/// Tarda `espera` mas, como mucho, un segundo para terminar de resolver lo
/// visto al final. Solo devuelve vecinos con direccion IPv4.
pub fn buscar(tipo: &str, espera: Duration) -> Result<Vec<Vecino>, ErrorMdns> {
    let tipo = normalizar_tipo(tipo)?;
    let consulta = ancho(&nombre_de_consulta(&tipo));
    let buzon = Buzon::<Vec<String>>::nuevo();
    let contexto = a_contexto(&buzon);
    let peticion = Box::new(DNS_SERVICE_BROWSE_REQUEST {
        Version: DNS_QUERY_REQUEST_VERSION1.0,
        InterfaceIndex: 0,
        QueryName: PCWSTR(consulta.as_ptr()),
        Anonymous: DNS_SERVICE_BROWSE_REQUEST_0 {
            pBrowseCallback: Some(al_navegar),
        },
        pQueryContext: contexto,
    });
    let mut cancelar = Box::new(DNS_SERVICE_CANCEL::default());
    // SAFETY: consulta, peticion y cancelacion viven hasta la llamada final
    // (o se dejan sin liberar si no llega).
    let r = unsafe { DnsServiceBrowse(&*peticion, &mut *cancelar) };
    if r != DNS_REQUEST_PENDING {
        // SAFETY: sin operacion en marcha no habra llamada.
        unsafe { soltar_contexto::<Vec<String>>(contexto) };
        return Err(ErrorMdns::Windows {
            operacion: "DnsServiceBrowse",
            codigo: r as u32,
        });
    }

    let limite = Instant::now() + espera;
    let mut vistos = HashSet::new();
    let mut resoluciones = Vec::new();
    let mut terminada;
    loop {
        let nuevos: Vec<String> = {
            let mut d = buzon.esperar(limite, |d| !d.mensajes.is_empty() || d.fin);
            terminada = d.fin;
            d.mensajes.drain(..).flatten().collect()
        };
        for completo in nuevos {
            // Solo instancias de este tipo; y cada una se resuelve una vez
            // aunque llegue por varias tarjetas.
            if instancia_corta(&completo, &tipo).is_none()
                || !vistos.insert(completo.to_lowercase())
            {
                continue;
            }
            match Resolucion::empezar(&completo) {
                Ok(r) => resoluciones.push(r),
                Err(codigo) => tracing::debug!(codigo, %completo, "mdns: no se pudo resolver"),
            }
        }
        if terminada || Instant::now() >= limite {
            break;
        }
    }

    if !terminada {
        // SAFETY: la cancelacion es la de esta busqueda, viva.
        unsafe { DnsServiceBrowseCancel(&*cancelar) };
        let limite = Instant::now() + ESPERA_CIERRE;
        terminada = buzon.esperar(limite, |d| d.fin).fin;
    }
    if terminada {
        // SAFETY: Windows ya hizo la llamada final de la busqueda.
        unsafe { soltar_contexto::<Vec<String>>(contexto) };
    } else {
        tracing::warn!("mdns: la busqueda no confirmo su cancelacion");
        std::mem::forget((consulta, peticion, cancelar));
    }

    let limite = limite + GRACIA_RESOLVER;
    let mut vecinos = Vec::new();
    for r in resoluciones {
        let Some(resuelto) = r.terminar(limite) else {
            continue;
        };
        let (Some(host), Some(instancia)) =
            (resuelto.ip4, instancia_corta(&resuelto.completo, &tipo))
        else {
            tracing::debug!(nombre = %resuelto.completo, "mdns: vecino sin IPv4 o de otro tipo");
            continue;
        };
        vecinos.push(Vecino {
            instancia,
            host,
            puerto: resuelto.puerto,
            datos: datos_txt(resuelto.pares),
        });
    }
    Ok(sin_repetidos(vecinos))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_tipo_de_android_con_punto_final_se_acepta() {
        assert_eq!(normalizar_tipo("_pixpin._tcp.").unwrap(), "_pixpin._tcp");
        assert_eq!(normalizar_tipo("_pixpin._tcp").unwrap(), "_pixpin._tcp");
        assert_eq!(
            normalizar_tipo("_PixPin._TCP.local.").unwrap(),
            "_pixpin._tcp"
        );
        assert_eq!(normalizar_tipo("_ab-1._udp").unwrap(), "_ab-1._udp");
    }

    #[test]
    fn un_tipo_mal_formado_se_rechaza() {
        for malo in [
            "",
            "pixpin._tcp",
            "_pixpin",
            "_pixpin._sctp",
            "_._tcp",
            "_un_nombre_de_mas_de_quince._tcp",
            "_pix pin._tcp",
            "_pixpin._tcp.extra",
        ] {
            assert!(
                matches!(normalizar_tipo(malo), Err(ErrorMdns::TipoNoValido(_))),
                "debio rechazar {malo:?}"
            );
        }
    }

    #[test]
    fn la_consulta_y_el_host_llevan_punto_local() {
        assert_eq!(nombre_de_consulta("_pixpin._tcp"), "_pixpin._tcp.local");
        assert_eq!(nombre_de_host("MAXBOOK"), "MAXBOOK.local");
    }

    #[test]
    fn la_instancia_se_arma_con_el_tipo_y_escapa_puntos_y_barras() {
        assert_eq!(
            nombre_de_instancia("PixPin Max", "_pixpin._tcp").unwrap(),
            "PixPin Max._pixpin._tcp.local"
        );
        assert_eq!(
            nombre_de_instancia("PixPin v1.2\\b", "_pixpin._tcp").unwrap(),
            r"PixPin v1\.2\\b._pixpin._tcp.local"
        );
    }

    #[test]
    fn un_nombre_vacio_no_es_instancia() {
        for malo in ["", "   "] {
            assert!(matches!(
                nombre_de_instancia(malo, "_pixpin._tcp"),
                Err(ErrorMdns::NombreNoValido(_))
            ));
        }
    }

    #[test]
    fn un_nombre_largo_se_recorta_sin_partir_una_letra() {
        let largo = "ñ".repeat(40); // 80 bytes
        let recortado = recortar_etiqueta(&largo);
        assert!(recortado.len() <= MAXIMO_ETIQUETA);
        assert_eq!(recortado, "ñ".repeat(31), "62 bytes: la 32ª letra no cabe");
        assert_eq!(recortar_etiqueta("corto"), "corto");
    }

    #[test]
    fn la_instancia_completa_vuelve_a_su_nombre_corto() {
        let t = "_pixpin._tcp";
        assert_eq!(
            instancia_corta("PixPin Max._pixpin._tcp.local", t).as_deref(),
            Some("PixPin Max")
        );
        assert_eq!(
            instancia_corta("PixPin Max._pixpin._tcp.local.", t).as_deref(),
            Some("PixPin Max"),
            "con el punto final de la raiz"
        );
        assert_eq!(
            instancia_corta(r"PixPin v1\.2\\b._PIXPIN._tcp.local", t).as_deref(),
            Some(r"PixPin v1.2\b")
        );
        assert_eq!(
            instancia_corta(r"PixPin\032Max._pixpin._tcp.local", t).as_deref(),
            Some("PixPin Max"),
            "escape decimal"
        );
        // Ida y vuelta.
        let completo = nombre_de_instancia("Móvil de Ana.2", t).unwrap();
        assert_eq!(
            instancia_corta(&completo, t).as_deref(),
            Some("Móvil de Ana.2")
        );
    }

    #[test]
    fn un_nombre_de_otro_tipo_no_es_instancia_nuestra() {
        let t = "_pixpin._tcp";
        assert_eq!(instancia_corta("Impresora._ipp._tcp.local", t), None);
        assert_eq!(instancia_corta("_pixpin._tcp.local", t), None);
        assert_eq!(instancia_corta("._pixpin._tcp.local", t), None);
        assert_eq!(instancia_corta("", t), None);
    }

    #[test]
    fn el_txt_parte_clave_y_valor_y_gana_la_primera() {
        let datos = datos_txt([
            ("g".into(), "0123456789abcdef".into()),
            ("ID".into(), "abc".into()),
            ("l=P".into(), String::new()),
            ("n".into(), "Ana=Maria".into()),
            ("g".into(), "repetida".into()),
            ("".into(), "sin clave".into()),
            ("=x".into(), String::new()),
            ("vacia".into(), String::new()),
        ]);
        assert_eq!(datos.get("g").map(String::as_str), Some("0123456789abcdef"));
        assert_eq!(datos.get("id").map(String::as_str), Some("abc"));
        assert_eq!(datos.get("l").map(String::as_str), Some("P"));
        assert_eq!(datos.get("n").map(String::as_str), Some("Ana=Maria"));
        assert_eq!(datos.get("vacia").map(String::as_str), Some(""));
        assert_eq!(datos.len(), 5, "la clave vacia se descarta: {datos:?}");
    }

    #[test]
    fn los_datos_validos_pasan_y_los_malos_se_rechazan() {
        assert!(validar_datos(&[("g", "0123456789abcdef"), ("n", "Ana María")]).is_ok());
        assert!(validar_datos(&[]).is_ok());
        let largo = "x".repeat(254);
        for malo in [("", "x"), ("a=b", "x"), ("ñ", "x"), ("k", largo.as_str())] {
            assert!(
                matches!(validar_datos(&[malo]), Err(ErrorMdns::DatoNoValido(_))),
                "debio rechazar {malo:?}"
            );
        }
        let justo = "x".repeat(253);
        assert!(
            validar_datos(&[("k", &justo)]).is_ok(),
            "255 bytes justos caben"
        );
    }

    fn vecino(instancia: &str, ultimo: u8) -> Vecino {
        Vecino {
            instancia: instancia.into(),
            host: Ipv4Addr::new(192, 168, 1, ultimo),
            puerto: 4000,
            datos: HashMap::new(),
        }
    }

    #[test]
    fn la_misma_instancia_por_dos_tarjetas_queda_una_vez() {
        let v = sin_repetidos(vec![
            vecino("PixPin Ana", 10),
            vecino("PixPin Luis", 11),
            vecino("pixpin ana", 12),
            vecino("PixPin Ana", 13),
        ]);
        assert_eq!(v, vec![vecino("PixPin Ana", 10), vecino("PixPin Luis", 11)]);
        assert!(sin_repetidos(Vec::new()).is_empty());
    }

    #[test]
    fn el_nombre_del_equipo_no_esta_vacio() {
        let equipo = nombre_del_equipo().expect("nombre del equipo");
        assert!(!equipo.is_empty() && !equipo.contains('\0'), "{equipo:?}");
    }

    /// Anuncia y se busca a si mismo en esta maquina. Usa la red (mDNS por
    /// el servicio de DNS de Windows); no abre ventanas.
    /// `cargo test -p pixpin-shell -- --ignored mdns`
    #[test]
    #[ignore = "usa la red local"]
    fn un_anuncio_propio_aparece_al_buscar_y_se_retira() {
        let nombre = format!("PixPin prueba.{}", std::process::id());
        let datos = [
            ("g", "0123456789abcdef"),
            ("id", "prueba-id"),
            ("l", "P"),
            ("n", "Prueba"),
        ];
        let inicio = Instant::now();
        let anuncio = anunciar("_pixpin._tcp", &nombre, 45_678, &datos).expect("anunciar");
        eprintln!("anunciado en {:?}", inicio.elapsed());

        let inicio = Instant::now();
        let vecinos = buscar("_pixpin._tcp", Duration::from_secs(4)).expect("buscar");
        eprintln!("busqueda en {:?}: {vecinos:#?}", inicio.elapsed());
        let mio = vecinos
            .iter()
            .find(|v| v.instancia == nombre)
            .expect("el propio anuncio debe aparecer");
        assert_eq!(mio.puerto, 45_678);
        for (clave, valor) in datos {
            assert_eq!(mio.datos.get(clave).map(String::as_str), Some(valor));
        }
        assert!(!mio.host.is_unspecified());

        let inicio = Instant::now();
        drop(anuncio);
        eprintln!("retirado en {:?}", inicio.elapsed());
    }
}
