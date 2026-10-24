//! Aligerar un PDF: bajar las fotos de dentro a 200 ppp y reguardarlas en
//! JPEG, **sin tocar el texto ni los vectores** y **nunca a peor**.
//!
//! Es lo que hace el movil en `pdf/ComprimirPdf.kt` (v0.72), resumido en
//! `docs/investigacion/2026-09-20-miniapps-y-grabar.md` §3.2: 200 ppp, si una
//! foto no gana un cuarto se deja como estaba, y si el fichero entero no baja
//! un 15 % o deja de leerse se queda el original. Reescribe el documento
//! entero en vez de rehacerlo pagina a pagina, para no perder marcadores ni
//! formularios.
//!
//! **Que sostiene el «sin tocar el texto ni los vectores».** Tres cosas, y
//! hasta hace poco ninguna: (1) el escaneo cuenta los objetos que no supo
//! leer y el trabajo se para en vez de tirarlos —ver `Escaneo`—; (2) la
//! comprobacion previa a sustituir DIBUJA la primera y la ultima pagina, que
//! es lo que ejercita las fuentes y los recursos, y no solo las cuenta; y
//! (3) antes del renombrado se aparta una copia del original al lado.
//!
//! ## Lo primero: aqui NO hay pdfium
//!
//! Conviene decirlo alto porque el encargo daba por hecho lo contrario. Este
//! crate se apoya en `Windows.Data.Pdf` (ver la cabecera de `lib.rs`), que
//! **solo sabe dibujar paginas**: da mapas de bits y no expone ni un objeto del
//! documento. Ni PDFBox ni pdfium ni ningun otro modelo de PDF entra en el
//! ejecutable, y anadir uno no es una linea de `Cargo.toml`: son varios
//! megabytes de binario nativo que compilar, firmar y actualizar, que es
//! exactamente la decision que este crate ya tomo al reves y por escrito.
//!
//! Asi que el modelo minimo de PDF que hace falta para esto **esta escrito
//! aqui**, y esta escrito para ser **cobarde**: cuando hay algo que no entiende
//! del todo, no lo toca y lo dice ([`Estorbo`]). Vale mas una funcion que
//! conteste «de este PDF no puedo quitar nada» que una que entregue un fichero
//! que el lector de la obra no abre.
//!
//! ## Hasta donde llega, exactamente
//!
//! **Hace**: encuentra las imagenes `/Subtype /Image` guardadas en JPEG
//! (`/Filter /DCTDecode`), calcula a cuantos pixeles de ancho les toca estar
//! para salir a 200 ppp en la pagina donde se usan, las vuelve a codificar mas
//! pequenas, y reescribe el fichero entero con una tabla `xref` nueva.
//!
//! **No hace**, y por que:
//!
//! - **PDF cifrados**: habria que implementar RC4/AES del estandar y la clave
//!   del usuario. Se rechazan.
//! - **Tablas `xref` en flujo y objetos en flujo** (PDF 1.5 en adelante,
//!   `/Type /XRef`, `/Type /ObjStm`): los objetos van comprimidos dentro de
//!   otros objetos, y leerlos pide un descompresor y un modelo de verdad. Se
//!   rechazan enteros. En la practica los escaneres y las camaras de movil
//!   siguen escribiendo la tabla clasica, que es el caso que importa.
//! - **Imagenes que no sean JPEG suelto** (`/FlateDecode`, `/JPXDecode`,
//!   `/CCITTFaxDecode`, filtros en cadena): se dejan como estan. Un fax en
//!   blanco y negro ya pesa menos que el JPEG al que se pasaria.
//! - **Imagenes con mascara, transparencia o cualquier clave rara en su
//!   diccionario** (`/SMask`, `/Mask`, `/Decode`, `/DecodeParms`...): se dejan.
//!   Recodificarlas obligaria a rehacer tambien la mascara.
//! - **Quitar lo que no se usa** (objetos huerfanos, fuentes sin usar): pide
//!   recorrer el arbol de referencias desde el catalogo, que es justo el modelo
//!   que no hay. No se intenta.
//!
//! Cuando nada de eso da fruto, se devuelve [`Resultado::NoSeGanaNada`] con la
//! razon y **no se escribe ningun fichero**.

use crate::{Documento, ErrorPdf};
use std::path::Path;

/// A cuantos puntos por pulgada se bajan las fotos. Es el numero del movil.
///
/// 200 ppp es lo que un ojo distingue en un papel impreso y mas del doble de
/// lo que hace falta en pantalla. Por encima de ahi se guardan pixeles que
/// nadie va a ver nunca.
pub const PPP_OBJETIVO: f64 = 200.0;

/// Cuanto tiene que adelgazar una foto para que valga la pena cambiarla.
///
/// Por debajo de un cuarto no se toca: se estaria recodificando un JPEG —es
/// decir, perdiendo calidad otra vez— a cambio de unos kilobytes.
pub const MEJORA_MINIMA_DE_IMAGEN: f64 = 0.25;

/// Cuanto tiene que adelgazar el fichero entero para quedarse con el nuevo.
///
/// Si un PDF de 4 MB acaba en 3,8 MB, el usuario no nota la diferencia y en
/// cambio ha perdido calidad y ha cambiado un fichero que estaba bien. Por
/// debajo del 15 % se deja el original y se dice que no habia nada que quitar.
pub const MEJORA_MINIMA_DEL_FICHERO: f64 = 0.15;

/// Calidad del JPEG nuevo, de 0 a 1.
///
/// 0,75 es el punto donde el texto escaneado todavia se lee limpio y los
/// artefactos no se ven en una foto de obra. Por debajo empiezan a salir
/// manchas alrededor de las letras, que es justo lo que se va a mirar.
const CALIDAD_JPEG: f32 = 0.75;

/// El ancho de un A4 en puntos PDF (1/72 de pulgada).
///
/// Se usa solo como red: si no se consigue averiguar en que pagina se dibuja
/// una imagen, se supone que ocupa un A4 de ancho, que es el tamano de
/// practicamente todo lo que se escanea. Suponer de mas seria dejar la foto
/// mas grande de lo necesario, que es el fallo bueno de los dos.
const ANCHO_A4_PUNTOS: f64 = 595.28;

/// Algo del fichero que impide reescribirlo con seguridad.
///
/// Que esto sea una lista y no un `bool` es el punto: el usuario merece saber
/// **por que** no se le puede aligerar su PDF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estorbo {
    /// El documento esta cifrado (`/Encrypt` en el trailer).
    Cifrado,
    /// La tabla de referencias va en un flujo comprimido (`/Type /XRef`).
    XrefEnFlujo,
    /// Hay objetos metidos dentro de otros objetos (`/Type /ObjStm`).
    ObjetosEnFlujo,
    /// No hay un `trailer` clasico del que copiar el catalogo.
    SinTrailer,
    /// No se reconocio ni un solo objeto: o no es un PDF, o es de una forma
    /// que este lector minimo no entiende.
    SinObjetos,
    /// Algun flujo declara su `/Length` como referencia a otro objeto. Sin
    /// resolverla no se sabe donde acaba, y adivinarlo buscando `endstream`
    /// dentro de datos binarios es como se corrompe un fichero.
    LargoIndirecto,
    /// Hay objetos que este lector minimo no supo leer. `reescribir` solo
    /// escribe lo que reconocio, asi que seguir adelante los TIRARIA del
    /// fichero —y podrian ser una fuente incrustada, las anotaciones o los
    /// campos de un formulario—. La promesa de la cabecera de este modulo es
    /// «sin tocar el texto ni los vectores»: si no se sabe que hay dentro,
    /// no se toca nada.
    ObjetosSinEntender,
}

impl Estorbo {
    /// Una frase corta para enseñar al usuario. Sin rotulos del catalogo a
    /// proposito: los `.ftl` los toca otra tanda.
    pub fn explicacion(self) -> &'static str {
        match self {
            Estorbo::Cifrado => "el PDF esta cifrado",
            Estorbo::XrefEnFlujo => "el PDF guarda su indice comprimido",
            Estorbo::ObjetosEnFlujo => "el PDF guarda sus objetos comprimidos",
            Estorbo::SinTrailer => "el PDF no tiene un trailer legible",
            Estorbo::SinObjetos => "no se reconocio ningun objeto del PDF",
            Estorbo::LargoIndirecto => "un flujo del PDF no dice cuanto mide",
            Estorbo::ObjetosSinEntender => "hay partes del PDF que no se entienden",
        }
    }
}

/// Una imagen encontrada dentro del PDF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagenHallada {
    /// Numero del objeto que la guarda.
    pub objeto: u32,
    pub ancho: u32,
    pub alto: u32,
    /// El `/Filter` tal cual, para poder decirlo en un informe.
    pub filtro: String,
    /// Lo que ocupan sus datos dentro del fichero.
    pub bytes: usize,
    /// Si esta en la forma que se sabe recomprimir.
    pub se_puede_tocar: bool,
}

/// Lo que se sabe de un PDF sin haberlo tocado.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Informe {
    pub bytes: usize,
    pub imagenes: Vec<ImagenHallada>,
    pub estorbos: Vec<Estorbo>,
}

impl Informe {
    /// Cuanto del fichero se lo llevan las imagenes.
    pub fn bytes_en_imagenes(&self) -> usize {
        self.imagenes.iter().map(|i| i.bytes).sum()
    }

    /// Si merece la pena siquiera intentarlo.
    pub fn se_puede_intentar(&self) -> bool {
        self.estorbos.is_empty() && self.imagenes.iter().any(|i| i.se_puede_tocar)
    }
}

/// Por que no se aligero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Razon {
    /// El fichero tiene algo que este lector no se atreve a reescribir.
    NoSeEntiende(Estorbo),
    /// No hay ni una imagen JPEG que tocar.
    SinFotosQueBajar,
    /// Las hay, pero ya estan a 200 ppp o menos, o recomprimirlas no ganaba
    /// el cuarto de rigor. **Este es el «de este PDF no puedo quitar nada»
    /// honesto.**
    YaEstaAlMinimo,
    /// Salio un fichero mas pequeno, pero no lo bastante (menos del 15 %).
    NoBajaLoBastante { antes: u64, despues: u64 },
    /// Salio un fichero y **no se deja abrir igual que el original**. Es la
    /// red de seguridad: si la reescritura rompio algo, el usuario se queda
    /// con su PDF intacto y no se entera nunca.
    ElNuevoNoSeLee,
}

impl Razon {
    pub fn explicacion(&self) -> String {
        match self {
            Razon::NoSeEntiende(e) => e.explicacion().to_string(),
            Razon::SinFotosQueBajar => "no lleva fotos que se puedan bajar".into(),
            Razon::YaEstaAlMinimo => "las fotos ya estan al minimo".into(),
            Razon::NoBajaLoBastante { antes, despues } => {
                format!("solo bajaria de {antes} a {despues} bytes, no compensa")
            }
            Razon::ElNuevoNoSeLee => "el fichero aligerado no se leia bien".into(),
        }
    }
}

/// Como acabo el intento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resultado {
    /// Se escribio el fichero nuevo.
    Aligerado { antes: u64, despues: u64 },
    /// No se escribio nada, y esta es la razon.
    NoSeGanaNada(Razon),
}

/// Mira un PDF y cuenta lo que hay dentro, **sin tocarlo**.
///
/// No abre el documento con Windows ni reserva memoria por pagina: es un
/// recorrido de los bytes. Sirve para decidir si ofrecerle al usuario la
/// entrada de menu y para explicarle por que no.
pub fn inspeccionar(bytes: &[u8]) -> Informe {
    let mut informe = Informe {
        bytes: bytes.len(),
        ..Default::default()
    };

    let Escaneo {
        objetos,
        sin_entender,
    } = objetos(bytes);
    if objetos.is_empty() {
        informe.estorbos.push(Estorbo::SinObjetos);
        return informe;
    }
    // **Contados, no ignorados.** Ver `Escaneo`.
    if sin_entender > 0 {
        informe.estorbos.push(Estorbo::ObjetosSinEntender);
    }

    let trailer = ultimo_trailer(bytes);
    match trailer {
        None => informe.estorbos.push(Estorbo::SinTrailer),
        Some(d) => {
            if valor(bytes, d, "Encrypt").is_some() {
                informe.estorbos.push(Estorbo::Cifrado);
            }
        }
    }

    for o in &objetos {
        if nombre_es(bytes, o.dic, "Type", "XRef") {
            empujar_una_vez(&mut informe.estorbos, Estorbo::XrefEnFlujo);
        }
        if nombre_es(bytes, o.dic, "Type", "ObjStm") {
            empujar_una_vez(&mut informe.estorbos, Estorbo::ObjetosEnFlujo);
        }
        if o.flujo.is_some() && !largo_directo(bytes, o.dic) {
            empujar_una_vez(&mut informe.estorbos, Estorbo::LargoIndirecto);
        }
        if !nombre_es(bytes, o.dic, "Subtype", "Image") {
            continue;
        }
        let Some((ini, fin)) = o.flujo else { continue };
        let filtro = valor(bytes, o.dic, "Filter")
            .map(|(a, b)| String::from_utf8_lossy(&bytes[a..b]).trim().to_string())
            .unwrap_or_default();
        informe.imagenes.push(ImagenHallada {
            objeto: o.numero,
            ancho: entero_de(bytes, valor(bytes, o.dic, "Width")).unwrap_or(0) as u32,
            alto: entero_de(bytes, valor(bytes, o.dic, "Height")).unwrap_or(0) as u32,
            filtro,
            bytes: fin - ini,
            se_puede_tocar: se_puede_recomprimir(bytes, o),
        });
    }

    informe
}

/// Intenta aligerar `origen` y dejar el resultado en `destino`.
///
/// `destino` puede ser el mismo `origen`: se escribe a un temporal y se
/// renombra al final, asi que un corte de luz a mitad deja el original entero.
/// **Si no se gana lo suficiente no se escribe nada**, ni siquiera un fichero
/// mas grande: el contrato del movil es «nunca a peor».
pub fn aligerar(origen: &Path, destino: &Path) -> Result<Resultado, ErrorPdf> {
    if !origen.is_file() {
        return Err(ErrorPdf::NoExiste(origen.to_path_buf()));
    }
    let bytes = std::fs::read(origen).map_err(|_| ErrorPdf::NoExiste(origen.to_path_buf()))?;
    let cabecera = &bytes[..bytes.len().min(1024)];
    if !cabecera.windows(5).any(|v| v == b"%PDF-") {
        return Err(ErrorPdf::NoEsPdf(origen.to_path_buf()));
    }

    // COM del hilo, puesto una vez para todo el trabajo: WIC lo necesita para
    // recodificar cada foto, y `Documento::abrir` pone y quita el suyo, que
    // esta anidado dentro de este y por tanto no cierra el apartamento a
    // media faena.
    let _com = ComDelHilo::nuevo();

    let informe = inspeccionar(&bytes);
    if let Some(e) = informe.estorbos.first() {
        return Ok(Resultado::NoSeGanaNada(Razon::NoSeEntiende(*e)));
    }
    if !informe.imagenes.iter().any(|i| i.se_puede_tocar) {
        return Ok(Resultado::NoSeGanaNada(Razon::SinFotosQueBajar));
    }

    // Se abre el original ANTES de tocar nada: si no se deja abrir, aligerarlo
    // no es el problema del usuario ahora mismo, y ademas hace falta su numero
    // de paginas para comprobar el resultado.
    let paginas_antes = Documento::abrir(origen)?.paginas();

    // `inspeccionar` ya ha abortado si hubo tramos sin entender, asi que aqui
    // la lista es el fichero entero.
    let objetos = objetos(&bytes).objetos;
    let anchos = anchos_de_pagina(&bytes, &objetos);

    let mut cambios: Vec<(u32, Vec<u8>, u32, u32)> = Vec::new();
    for o in &objetos {
        if !se_puede_recomprimir(&bytes, o) {
            continue;
        }
        let Some((ini, fin)) = o.flujo else { continue };
        let ancho = entero_de(&bytes, valor(&bytes, o.dic, "Width")).unwrap_or(0) as u32;
        if ancho == 0 {
            continue;
        }
        let puntos = anchos.get(&o.numero).copied().unwrap_or(ANCHO_A4_PUNTOS);
        let objetivo = ancho_objetivo(puntos);
        if ancho <= objetivo {
            continue;
        }
        let Ok((nuevo, nw, nh)) = recomprimir_jpeg(&bytes[ini..fin], objetivo) else {
            // Que una foto no se deje recodificar no puede tumbar el trabajo:
            // se deja como estaba y se sigue con las demas.
            continue;
        };
        if !gana_bastante(fin - ini, nuevo.len(), MEJORA_MINIMA_DE_IMAGEN) {
            continue;
        }
        cambios.push((o.numero, nuevo, nw, nh));
    }

    if cambios.is_empty() {
        return Ok(Resultado::NoSeGanaNada(Razon::YaEstaAlMinimo));
    }

    let Some(nuevo) = reescribir(&bytes, &objetos, &cambios) else {
        return Ok(Resultado::NoSeGanaNada(Razon::NoSeEntiende(
            Estorbo::SinTrailer,
        )));
    };

    let antes = bytes.len() as u64;
    let despues = nuevo.len() as u64;
    if !gana_bastante(bytes.len(), nuevo.len(), MEJORA_MINIMA_DEL_FICHERO) {
        return Ok(Resultado::NoSeGanaNada(Razon::NoBajaLoBastante {
            antes,
            despues,
        }));
    }

    // **La red.** Se escribe a un temporal, se abre de verdad y se DIBUJA.
    // Contar paginas no bastaba: un PDF que perdio una fuente incrustada
    // carga igual y declara las mismas paginas, asi que pasaba la prueba y
    // sustituia al original.
    //
    // El temporal lleva la hora en el nombre: con un nombre fijo, dos
    // aligerados a la vez del mismo fichero se pisaban.
    let temporal = destino.with_extension(format!("aligerando-{}.pdf", marca_de_tiempo()));
    std::fs::write(&temporal, &nuevo).map_err(|_| ErrorPdf::NoExiste(temporal.clone()))?;
    if !se_lee_y_se_dibuja(&temporal, paginas_antes) {
        let _ = std::fs::remove_file(&temporal);
        return Ok(Resultado::NoSeGanaNada(Razon::ElNuevoNoSeLee));
    }

    // **Y copia del original antes de sustituirlo.** Aligerar es la unica
    // operacion del programa que reescribe un fichero que el usuario trajo de
    // fuera; si la red de arriba se dejara algo, sin copia no hay vuelta
    // atras. El original se aparta al lado, con la hora en el nombre.
    if destino.is_file() {
        let respaldo = destino.with_extension(format!("original-{}.pdf", marca_de_tiempo()));
        std::fs::copy(destino, &respaldo).map_err(|_| ErrorPdf::NoExiste(respaldo.clone()))?;
    }
    std::fs::rename(&temporal, destino).map_err(|_| ErrorPdf::NoExiste(destino.to_path_buf()))?;

    Ok(Resultado::Aligerado { antes, despues })
}

/// Milisegundos desde 1970, para que dos trabajos a la vez no compartan
/// nombre de fichero.
fn marca_de_tiempo() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// **El PDF nuevo se abre, tiene las mismas paginas y se deja DIBUJAR.**
///
/// Dibujar es lo que de verdad ejercita las fuentes, los recursos y el arbol
/// de paginas; `PageCount()` solo mira el catalogo. Se dibuja la primera y la
/// ultima —las dos puntas del arbol— y no todas, porque en un escaneo de
/// doscientas paginas eso son varios minutos con el usuario esperando.
///
/// Un ancho pequeno basta: lo que se comprueba es que el motor no se atragante
/// con la pagina, no como queda.
fn se_lee_y_se_dibuja(ruta: &Path, paginas_antes: u32) -> bool {
    const ANCHO_DE_PRUEBA: u32 = 64;
    let Ok(doc) = Documento::abrir(ruta) else {
        return false;
    };
    if doc.paginas() != paginas_antes || doc.paginas() == 0 {
        return false;
    }
    let ultima = doc.paginas() - 1;
    for i in if ultima == 0 {
        vec![0]
    } else {
        vec![0, ultima]
    } {
        if doc.renderizar(i, ANCHO_DE_PRUEBA).is_err() {
            return false;
        }
    }
    true
}

/// Cuantos pixeles de ancho le tocan a una imagen que se dibuja sobre
/// `puntos` de ancho de pagina.
///
/// Un punto PDF es 1/72 de pulgada; a 200 ppp, un A4 (595 puntos ≈ 8,27
/// pulgadas) sale a unos 1654 pixeles. Nunca menos de uno.
fn ancho_objetivo(puntos: f64) -> u32 {
    let pulgadas = if puntos.is_finite() && puntos > 0.0 {
        puntos / 72.0
    } else {
        ANCHO_A4_PUNTOS / 72.0
    };
    ((pulgadas * PPP_OBJETIVO).round() as i64).clamp(1, crate::ANCHO_MAXIMO as i64) as u32
}

/// Si pasar de `antes` a `despues` bytes gana al menos la fraccion `minima`.
fn gana_bastante(antes: usize, despues: usize, minima: f64) -> bool {
    if antes == 0 {
        return false;
    }
    let ganado = 1.0 - (despues as f64 / antes as f64);
    ganado >= minima
}

// ---------------------------------------------------------------------------
// El modelo minimo de PDF
// ---------------------------------------------------------------------------

/// Un objeto de primer nivel del fichero: `N G obj … endobj`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Objeto {
    numero: u32,
    generacion: u16,
    /// Desde justo detras de `obj` hasta justo detras de `endobj`. Es lo que
    /// se vuelve a escribir tal cual cuando el objeto no se toca.
    cuerpo: (usize, usize),
    /// El diccionario `<< … >>`, si lo tiene.
    dic: (usize, usize),
    /// Los bytes del flujo, entre `stream` y `endstream`.
    flujo: Option<(usize, usize)>,
}

fn es_blanco(c: u8) -> bool {
    matches!(c, 0 | 9 | 10 | 12 | 13 | 32)
}

fn es_delimitador(c: u8) -> bool {
    matches!(
        c,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

/// Salta blancos y comentarios.
fn saltar_blancos(b: &[u8], mut i: usize) -> usize {
    while i < b.len() {
        if es_blanco(b[i]) {
            i += 1;
        } else if b[i] == b'%' {
            while i < b.len() && b[i] != b'\n' && b[i] != b'\r' {
                i += 1;
            }
        } else {
            return i;
        }
    }
    i
}

/// Donde acaba el valor que empieza en `i` (indice justo detras).
///
/// Entiende lo justo de la sintaxis: diccionarios, vectores, cadenas entre
/// parentesis con sus escapes, cadenas hexadecimales, nombres, numeros y la
/// referencia `N G R`. Con eso basta para recorrer un diccionario sin
/// confundir un `>>` que va dentro de una cadena con el que cierra.
fn fin_del_valor(b: &[u8], i: usize) -> Option<usize> {
    if i >= b.len() {
        return None;
    }
    match b[i] {
        b'<' if b.get(i + 1) == Some(&b'<') => fin_del_diccionario(b, i),
        b'<' => {
            let mut j = i + 1;
            while j < b.len() && b[j] != b'>' {
                j += 1;
            }
            (j < b.len()).then_some(j + 1)
        }
        b'[' => {
            let mut j = i + 1;
            loop {
                j = saltar_blancos(b, j);
                match b.get(j) {
                    None => return None,
                    Some(b']') => return Some(j + 1),
                    _ => j = fin_del_valor(b, j)?,
                }
            }
        }
        b'(' => {
            let mut j = i + 1;
            let mut nivel = 1usize;
            while j < b.len() {
                match b[j] {
                    b'\\' => j += 1,
                    b'(' => nivel += 1,
                    b')' => {
                        nivel -= 1;
                        if nivel == 0 {
                            return Some(j + 1);
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            None
        }
        b'/' => {
            let mut j = i + 1;
            while j < b.len() && !es_blanco(b[j]) && !es_delimitador(b[j]) {
                j += 1;
            }
            Some(j)
        }
        _ => {
            let mut j = i;
            while j < b.len() && !es_blanco(b[j]) && !es_delimitador(b[j]) {
                j += 1;
            }
            if j == i {
                return None;
            }
            // `12 0 R` es UN valor, no tres. Sin esto, la clave siguiente de un
            // diccionario se leeria como si fuera el `0`.
            if let Some(fin) = fin_de_referencia(b, i, j) {
                return Some(fin);
            }
            Some(j)
        }
    }
}

/// Si en `i..j` hay un entero y detras viene ` G R`, donde acaba la referencia.
fn fin_de_referencia(b: &[u8], i: usize, j: usize) -> Option<usize> {
    if !b[i..j].iter().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let k = saltar_blancos(b, j);
    let mut m = k;
    while m < b.len() && b[m].is_ascii_digit() {
        m += 1;
    }
    if m == k {
        return None;
    }
    let n = saltar_blancos(b, m);
    if b.get(n) == Some(&b'R')
        && b.get(n + 1)
            .is_none_or(|c| es_blanco(*c) || es_delimitador(*c))
    {
        Some(n + 1)
    } else {
        None
    }
}

/// Donde acaba el diccionario que empieza en `i` (que tiene que ser un `<<`).
fn fin_del_diccionario(b: &[u8], i: usize) -> Option<usize> {
    if b.get(i) != Some(&b'<') || b.get(i + 1) != Some(&b'<') {
        return None;
    }
    let mut j = i + 2;
    loop {
        j = saltar_blancos(b, j);
        if b.get(j) == Some(&b'>') && b.get(j + 1) == Some(&b'>') {
            return Some(j + 2);
        }
        j = fin_del_valor(b, j)?;
    }
}

/// El valor de `clave` dentro del diccionario `dic`, como rango de bytes.
///
/// `clave` va sin la barra: `valor(b, dic, "Width")`. Solo mira el nivel de
/// arriba, que es lo que hace falta y lo que evita confundir un `/Width` de
/// dentro de un sub-diccionario con el de la imagen.
fn valor(b: &[u8], dic: (usize, usize), clave: &str) -> Option<(usize, usize)> {
    let (ini, _) = dic;
    if b.get(ini) != Some(&b'<') {
        return None;
    }
    let mut j = ini + 2;
    loop {
        j = saltar_blancos(b, j);
        if b.get(j) == Some(&b'>') {
            return None;
        }
        if b.get(j) != Some(&b'/') {
            return None;
        }
        let fin_clave = fin_del_valor(b, j)?;
        let esta = &b[j + 1..fin_clave] == clave.as_bytes();
        let ini_valor = saltar_blancos(b, fin_clave);
        let fin_valor = fin_del_valor(b, ini_valor)?;
        if esta {
            return Some((ini_valor, fin_valor));
        }
        j = fin_valor;
    }
}

/// Si el valor de `clave` es el nombre `esperado`.
fn nombre_es(b: &[u8], dic: (usize, usize), clave: &str, esperado: &str) -> bool {
    match valor(b, dic, clave) {
        Some((i, j)) => b.get(i) == Some(&b'/') && &b[i + 1..j] == esperado.as_bytes(),
        None => false,
    }
}

/// El entero de un valor, si lo es.
fn entero_de(b: &[u8], rango: Option<(usize, usize)>) -> Option<i64> {
    let (i, j) = rango?;
    std::str::from_utf8(&b[i..j]).ok()?.trim().parse().ok()
}

/// El decimal de un valor, si lo es.
fn decimal_de(b: &[u8], rango: Option<(usize, usize)>) -> Option<f64> {
    let (i, j) = rango?;
    std::str::from_utf8(&b[i..j]).ok()?.trim().parse().ok()
}

/// Si el `/Length` del diccionario es un entero escrito ahi mismo.
fn largo_directo(b: &[u8], dic: (usize, usize)) -> bool {
    entero_de(b, valor(b, dic, "Length")).is_some()
}

fn empujar_una_vez(v: &mut Vec<Estorbo>, e: Estorbo) {
    if !v.contains(&e) {
        v.push(e);
    }
}

/// Lo que salio de recorrer los bytes: los objetos **y los que no se
/// supieron leer**.
///
/// Los segundos son la parte importante y por eso viajan pegados a los
/// primeros. `reescribir` solo escribe lo que hay en la lista, asi que un
/// objeto que el analizador no sepa parsear —un `endobj` que falta, un
/// `/Length` que no cuadra, un diccionario con una construccion que
/// `fin_del_valor` no cubre— **desaparecia del fichero de salida sin que
/// nada lo contara**. Fuentes incrustadas, anotaciones, marcadores y campos
/// de formulario son objetos ordinarios: ahi es donde duele.
struct Escaneo {
    objetos: Vec<Objeto>,
    /// Cuantas cabeceras `N G obj` se encontraron y no se pudieron leer.
    sin_entender: usize,
}

/// Todos los objetos de primer nivel, en el orden en que aparecen, y cuantos
/// se quedaron por el camino.
fn objetos(b: &[u8]) -> Escaneo {
    let mut salida = Vec::new();
    let mut sin_entender = 0usize;
    let mut i = 0usize;
    while i + 3 <= b.len() {
        if &b[i..i + 3] != b"obj" {
            i += 1;
            continue;
        }
        if b.get(i + 3)
            .is_some_and(|c| !es_blanco(*c) && !es_delimitador(*c))
        {
            i += 1;
            continue;
        }
        let Some((numero, generacion)) = cabecera_hacia_atras(b, i) else {
            // Sin `N G ` delante esto no era una cabecera de objeto sino la
            // palabra «obj» dentro de otra cosa: no se cuenta.
            i += 1;
            continue;
        };
        let Some(o) = cuerpo_del_objeto(b, numero, generacion, i + 3) else {
            // **Aqui si.** La cabecera es de verdad y el cuerpo no se
            // entendio: ese objeto se perderia al reescribir.
            sin_entender += 1;
            i += 3;
            continue;
        };
        i = o.cuerpo.1;
        salida.push(o);
    }
    Escaneo {
        objetos: salida,
        sin_entender,
    }
}

/// Lee `N G ` hacia atras desde el `obj` que empieza en `i`.
fn cabecera_hacia_atras(b: &[u8], i: usize) -> Option<(u32, u16)> {
    let mut j = i;
    while j > 0 && es_blanco(b[j - 1]) {
        j -= 1;
    }
    let fin_gen = j;
    while j > 0 && b[j - 1].is_ascii_digit() {
        j -= 1;
    }
    if j == fin_gen {
        return None;
    }
    let generacion: u16 = std::str::from_utf8(&b[j..fin_gen]).ok()?.parse().ok()?;
    while j > 0 && es_blanco(b[j - 1]) {
        j -= 1;
    }
    let fin_num = j;
    while j > 0 && b[j - 1].is_ascii_digit() {
        j -= 1;
    }
    if j == fin_num {
        return None;
    }
    let numero: u32 = std::str::from_utf8(&b[j..fin_num]).ok()?.parse().ok()?;
    Some((numero, generacion))
}

/// Del `obj` al `endobj`: diccionario, flujo y donde acaba todo.
fn cuerpo_del_objeto(b: &[u8], numero: u32, generacion: u16, tras_obj: usize) -> Option<Objeto> {
    let ini_dic = saltar_blancos(b, tras_obj);
    let dic = if b.get(ini_dic) == Some(&b'<') && b.get(ini_dic + 1) == Some(&b'<') {
        (ini_dic, fin_del_diccionario(b, ini_dic)?)
    } else {
        (ini_dic, ini_dic)
    };

    let tras_dic = saltar_blancos(b, dic.1);
    let mut flujo = None;
    let mut desde = tras_dic;
    if b[tras_dic..].starts_with(b"stream") {
        let mut d = tras_dic + 6;
        // La norma manda CRLF o LF detras de `stream`, nunca CR suelto; se
        // aceptan los tres porque los hay escritos asi por ahi.
        if b.get(d) == Some(&b'\r') {
            d += 1;
        }
        if b.get(d) == Some(&b'\n') {
            d += 1;
        }
        let fin = match entero_de(b, valor(b, dic, "Length")) {
            // El `/Length` bueno: se cree y se comprueba que detras viene
            // `endstream`, que es la unica forma de saber que no mentia.
            Some(n) if n >= 0 && d + n as usize <= b.len() => {
                let f = d + n as usize;
                let tras = saltar_blancos(b, f);
                if b[tras..].starts_with(b"endstream") {
                    f
                } else {
                    buscar(b, d, b"endstream")?
                }
            }
            _ => buscar(b, d, b"endstream")?,
        };
        flujo = Some((d, fin));
        desde = fin;
    }

    let fin_endobj = buscar(b, desde, b"endobj")? + 6;
    Some(Objeto {
        numero,
        generacion,
        cuerpo: (tras_obj, fin_endobj),
        dic,
        flujo,
    })
}

/// El indice de la proxima aparicion de `aguja` a partir de `desde`.
fn buscar(b: &[u8], desde: usize, aguja: &[u8]) -> Option<usize> {
    if desde >= b.len() {
        return None;
    }
    b[desde..]
        .windows(aguja.len())
        .position(|v| v == aguja)
        .map(|p| desde + p)
}

/// El diccionario del ultimo `trailer` del fichero.
///
/// El ultimo y no el primero porque un PDF con actualizaciones incrementales
/// lleva varios, y el bueno es el de mas abajo.
fn ultimo_trailer(b: &[u8]) -> Option<(usize, usize)> {
    let mut encontrado = None;
    let mut i = 0usize;
    while let Some(p) = buscar(b, i, b"trailer") {
        let d = saltar_blancos(b, p + 7);
        if let Some(fin) = fin_del_diccionario(b, d) {
            encontrado = Some((d, fin));
        }
        i = p + 7;
    }
    encontrado
}

/// Si una imagen esta en la forma exacta que se sabe recomprimir.
///
/// Es una lista blanca a proposito. Todo lo que no esta escrito aqui —una
/// mascara, un espacio de color raro, un filtro en cadena— se deja como esta:
/// recodificarlo a medias saldria peor que no tocarlo.
fn se_puede_recomprimir(b: &[u8], o: &Objeto) -> bool {
    const CLAVES_QUE_SE_ENTIENDEN: [&str; 9] = [
        "Type",
        "Subtype",
        "Width",
        "Height",
        "ColorSpace",
        "BitsPerComponent",
        "Filter",
        "Length",
        "Name",
    ];

    if o.flujo.is_none() || !nombre_es(b, o.dic, "Subtype", "Image") {
        return false;
    }
    if !nombre_es(b, o.dic, "Filter", "DCTDecode") {
        return false;
    }
    if !nombre_es(b, o.dic, "ColorSpace", "DeviceRGB")
        && !nombre_es(b, o.dic, "ColorSpace", "DeviceGray")
    {
        return false;
    }
    if entero_de(b, valor(b, o.dic, "BitsPerComponent")) != Some(8) {
        return false;
    }
    if !largo_directo(b, o.dic) {
        return false;
    }
    let (ancho, alto) = (
        entero_de(b, valor(b, o.dic, "Width")).unwrap_or(0),
        entero_de(b, valor(b, o.dic, "Height")).unwrap_or(0),
    );
    if ancho <= 0 || alto <= 0 {
        return false;
    }
    claves_de(b, o.dic).is_some_and(|claves| {
        claves
            .iter()
            .all(|c| CLAVES_QUE_SE_ENTIENDEN.contains(&c.as_str()))
    })
}

/// Los nombres de las claves de un diccionario, sin la barra.
fn claves_de(b: &[u8], dic: (usize, usize)) -> Option<Vec<String>> {
    let (ini, _) = dic;
    if b.get(ini) != Some(&b'<') {
        return None;
    }
    let mut claves = Vec::new();
    let mut j = ini + 2;
    loop {
        j = saltar_blancos(b, j);
        if b.get(j) == Some(&b'>') {
            return Some(claves);
        }
        if b.get(j) != Some(&b'/') {
            return None;
        }
        let fin_clave = fin_del_valor(b, j)?;
        claves.push(String::from_utf8_lossy(&b[j + 1..fin_clave]).to_string());
        let ini_valor = saltar_blancos(b, fin_clave);
        j = fin_del_valor(b, ini_valor)?;
    }
}

/// Para cada objeto de imagen, el ancho en puntos de la pagina que la usa.
///
/// Se busca por el camino corto: cada `/Type /Page` dice su `/MediaBox` (o la
/// hereda de su `/Parent`) y su `/Resources /XObject` nombra los objetos que
/// dibuja. **No se miran los flujos de contenido**, asi que esto supone que la
/// imagen se dibuja a pagina completa — que es el caso del escaneo y de la foto
/// insertada, y el unico que interesa aqui. Si una imagen se dibujase pequenita
/// en una esquina, saldria con mas pixeles de los necesarios: de mas, nunca de
/// menos.
fn anchos_de_pagina(b: &[u8], objetos: &[Objeto]) -> std::collections::HashMap<u32, f64> {
    use std::collections::HashMap;
    let por_numero: HashMap<u32, &Objeto> = objetos.iter().map(|o| (o.numero, o)).collect();
    let mut salida = HashMap::new();

    for o in objetos {
        if !nombre_es(b, o.dic, "Type", "Page") {
            continue;
        }
        let ancho = ancho_del_mediabox(b, o, &por_numero).unwrap_or(ANCHO_A4_PUNTOS);
        // `/Resources` puede estar en la pagina o ser una referencia.
        let recursos = match valor(b, o.dic, "Resources") {
            Some((i, j)) if b.get(i) == Some(&b'<') => Some((i, j)),
            Some((i, j)) => referencia(b, (i, j))
                .and_then(|n| por_numero.get(&n))
                .map(|r| r.dic),
            None => None,
        };
        let Some(recursos) = recursos else { continue };
        let Some(xobjects) = valor(b, recursos, "XObject") else {
            continue;
        };
        let xobjects = if b.get(xobjects.0) == Some(&b'<') {
            Some(xobjects)
        } else {
            referencia(b, xobjects)
                .and_then(|n| por_numero.get(&n))
                .map(|r| r.dic)
        };
        let Some(xobjects) = xobjects else { continue };
        for n in referencias_de(b, xobjects) {
            // Si dos paginas usan la misma imagen gana la mas estrecha: la
            // imagen tiene que servir para las dos.
            salida
                .entry(n)
                .and_modify(|a: &mut f64| *a = a.min(ancho))
                .or_insert(ancho);
        }
    }
    salida
}

/// El ancho del `/MediaBox`, subiendo por `/Parent` si la pagina no lo trae.
fn ancho_del_mediabox(
    b: &[u8],
    pagina: &Objeto,
    por_numero: &std::collections::HashMap<u32, &Objeto>,
) -> Option<f64> {
    let mut actual = *pagina;
    // Cuatro saltos de sobra: el arbol de paginas de verdad tiene dos o tres
    // niveles, y un tope evita que un `/Parent` que apunta a si mismo cuelgue
    // el programa.
    for _ in 0..4 {
        if let Some(caja) = valor(b, actual.dic, "MediaBox")
            && let Some(ancho) = ancho_de_caja(b, caja)
        {
            return Some(ancho);
        }
        let padre = referencia(b, valor(b, actual.dic, "Parent")?)?;
        actual = **por_numero.get(&padre)?;
    }
    None
}

/// `[x1 y1 x2 y2]` → `x2 - x1`.
fn ancho_de_caja(b: &[u8], caja: (usize, usize)) -> Option<f64> {
    if b.get(caja.0) != Some(&b'[') {
        return None;
    }
    let mut numeros = Vec::new();
    let mut j = caja.0 + 1;
    while numeros.len() < 4 {
        j = saltar_blancos(b, j);
        if b.get(j) == Some(&b']') {
            break;
        }
        let fin = fin_del_valor(b, j)?;
        numeros.push(decimal_de(b, Some((j, fin)))?);
        j = fin;
    }
    if numeros.len() < 4 {
        return None;
    }
    let ancho = (numeros[2] - numeros[0]).abs();
    (ancho > 0.0).then_some(ancho)
}

/// El numero de objeto de un valor `N G R`.
fn referencia(b: &[u8], rango: (usize, usize)) -> Option<u32> {
    let texto = std::str::from_utf8(&b[rango.0..rango.1]).ok()?;
    let mut trozos = texto.split_ascii_whitespace();
    let n: u32 = trozos.next()?.parse().ok()?;
    trozos.next()?;
    (trozos.next()? == "R").then_some(n)
}

/// Todos los `N G R` que hay dentro de un diccionario, en el primer nivel.
fn referencias_de(b: &[u8], dic: (usize, usize)) -> Vec<u32> {
    let mut salida = Vec::new();
    let mut j = dic.0 + 2;
    loop {
        j = saltar_blancos(b, j);
        if b.get(j) == Some(&b'>') || j >= dic.1 {
            return salida;
        }
        let Some(fin_clave) = fin_del_valor(b, j) else {
            return salida;
        };
        let ini_valor = saltar_blancos(b, fin_clave);
        let Some(fin_valor) = fin_del_valor(b, ini_valor) else {
            return salida;
        };
        if let Some(n) = referencia(b, (ini_valor, fin_valor)) {
            salida.push(n);
        }
        j = fin_valor;
    }
}

// ---------------------------------------------------------------------------
// Escribir el fichero nuevo
// ---------------------------------------------------------------------------

/// Reescribe el PDF entero con los objetos de `cambios` sustituidos.
///
/// Se reescribe el fichero **entero**, con todos sus objetos y una tabla
/// `xref` nueva, en vez de anadir una actualizacion incremental al final. Es lo
/// que hace el movil y por lo mismo: una actualizacion incremental solo anade
/// bytes, y aqui de lo que se trata es de que pese menos.
///
/// Devuelve `None` si falta el trailer, que es lo unico del original que no se
/// puede recalcular.
fn reescribir(
    b: &[u8],
    objetos: &[Objeto],
    cambios: &[(u32, Vec<u8>, u32, u32)],
) -> Option<Vec<u8>> {
    let trailer = ultimo_trailer(b)?;

    // Un PDF con actualizaciones incrementales tiene el mismo numero de objeto
    // escrito varias veces; el bueno es el ULTIMO. Se queda uno por numero.
    let mut ultimos: std::collections::BTreeMap<u32, &Objeto> = std::collections::BTreeMap::new();
    for o in objetos {
        ultimos.insert(o.numero, o);
    }

    let mut salida: Vec<u8> = Vec::with_capacity(b.len());
    salida.extend_from_slice(b"%PDF-1.7\n");
    // Los cuatro bytes altos de la segunda linea son la senal convenida de
    // «esto es binario»: sin ellos, un servidor o un cliente de correo puede
    // decidir que es texto y cambiarle los finales de linea.
    salida.extend_from_slice(b"%\xE2\xE3\xCF\xD3\n");

    let mut donde: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
    for (numero, o) in &ultimos {
        donde.insert(*numero, salida.len());
        salida.extend_from_slice(format!("{} {} obj", numero, o.generacion).as_bytes());
        match cambios.iter().find(|(n, _, _, _)| n == numero) {
            None => salida.extend_from_slice(&b[o.cuerpo.0..o.cuerpo.1]),
            Some((_, jpeg, ancho, alto)) => {
                let gris = nombre_es(b, o.dic, "ColorSpace", "DeviceGray");
                // El diccionario se rehace entero y no se parchea: es la unica
                // forma de que `/Length`, `/Width` y `/Height` no puedan quedar
                // diciendo una cosa mientras el flujo dice otra. Por eso
                // `se_puede_recomprimir` exige que no haya ni una clave fuera
                // de estas: lo que no se reescribe aqui se perderia.
                salida.extend_from_slice(
                    format!(
                        "\n<< /Type /XObject /Subtype /Image /Width {ancho} /Height {alto} \
                         /ColorSpace /{} /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\n\
                         stream\n",
                        if gris { "DeviceGray" } else { "DeviceRGB" },
                        jpeg.len()
                    )
                    .as_bytes(),
                );
                salida.extend_from_slice(jpeg);
                salida.extend_from_slice(b"\nendstream\nendobj");
            }
        }
        salida.push(b'\n');
    }

    let ultimo = *ultimos.keys().next_back()?;
    let tamano = ultimo as usize + 1;
    let inicio_xref = salida.len();
    salida.extend_from_slice(format!("xref\n0 {tamano}\n").as_bytes());
    // La entrada 0 es siempre la cabeza de la lista de huecos, con generacion
    // 65535. Las lineas miden exactamente 20 bytes, espacio final incluido:
    // un lector que busque por posicion se pierde si falta uno.
    salida.extend_from_slice(b"0000000000 65535 f \n");
    for n in 1..tamano as u32 {
        match donde.get(&n) {
            Some(pos) => {
                let generacion = ultimos.get(&n).map(|o| o.generacion).unwrap_or(0);
                salida.extend_from_slice(format!("{pos:010} {generacion:05} n \n").as_bytes());
            }
            // Un numero que no existe se declara hueco en vez de omitirse: la
            // tabla tiene que tener tantas lineas como dice su cabecera.
            None => salida.extend_from_slice(b"0000000000 65535 f \n"),
        }
    }

    salida.extend_from_slice(b"trailer\n");
    salida.extend_from_slice(&trailer_nuevo(b, trailer, tamano));
    salida.extend_from_slice(format!("\nstartxref\n{inicio_xref}\n%%EOF\n").as_bytes());
    Some(salida)
}

/// El trailer del original con `/Size` puesto al dia y sin lo que ya no vale.
///
/// `/Prev` y `/XRefStm` apuntaban a tablas del fichero viejo que en el nuevo no
/// existen: dejarlos mandaria al lector a leer basura.
fn trailer_nuevo(b: &[u8], trailer: (usize, usize), tamano: usize) -> Vec<u8> {
    const FUERA: [&str; 3] = ["Prev", "XRefStm", "Size"];
    let mut salida = format!("<< /Size {tamano}").into_bytes();
    let mut j = trailer.0 + 2;
    loop {
        j = saltar_blancos(b, j);
        if b.get(j) != Some(&b'/') {
            break;
        }
        let Some(fin_clave) = fin_del_valor(b, j) else {
            break;
        };
        let clave = String::from_utf8_lossy(&b[j + 1..fin_clave]).to_string();
        let ini_valor = saltar_blancos(b, fin_clave);
        let Some(fin_valor) = fin_del_valor(b, ini_valor) else {
            break;
        };
        if !FUERA.contains(&clave.as_str()) {
            salida.push(b' ');
            salida.extend_from_slice(&b[j..fin_valor]);
        }
        j = fin_valor;
    }
    salida.extend_from_slice(b" >>");
    salida
}

// ---------------------------------------------------------------------------
// El JPEG, con lo que trae Windows
// ---------------------------------------------------------------------------

/// Vuelve a codificar un JPEG con `ancho_objetivo` pixeles de ancho.
///
/// Se usa WIC (`Windows.Graphics.Imaging`), que ya viene puesto y es el mismo
/// que usa `renderizar` para bajar de tamano las paginas. `Fant` es su
/// remuestreo bueno: en texto escaneado la diferencia con el vecino mas proximo
/// es justo el borde dentado de las letras.
fn recomprimir_jpeg(
    jpeg: &[u8],
    ancho_objetivo: u32,
) -> Result<(Vec<u8>, u32, u32), windows::core::Error> {
    use windows::Graphics::Imaging::{
        BitmapAlphaMode, BitmapDecoder, BitmapInterpolationMode, BitmapPixelFormat,
        BitmapTransform, ColorManagementMode, ExifOrientationMode,
    };
    use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};

    let entrada = InMemoryRandomAccessStream::new()?;
    let escritor = DataWriter::CreateDataWriter(&entrada.GetOutputStreamAt(0)?)?;
    escritor.WriteBytes(jpeg)?;
    esperar_operacion(&escritor.StoreAsync()?)?;
    let _ = escritor.DetachStream();
    entrada.Seek(0)?;

    let descodificador = esperar_operacion(&BitmapDecoder::CreateAsync(&entrada)?)?;
    let ancho = descodificador.PixelWidth()?;
    let alto = descodificador.PixelHeight()?;
    if ancho == 0 || alto == 0 {
        return Err(windows::core::Error::from(
            windows::Win32::Foundation::E_FAIL,
        ));
    }
    let nuevo_ancho = ancho_objetivo.min(ancho).max(1);
    let nuevo_alto = ((nuevo_ancho as u64 * alto as u64).div_ceil(ancho as u64) as u32).max(1);

    let transformacion = BitmapTransform::new()?;
    transformacion.SetScaledWidth(nuevo_ancho)?;
    transformacion.SetScaledHeight(nuevo_alto)?;
    transformacion.SetInterpolationMode(BitmapInterpolationMode::Fant)?;

    let datos = esperar_operacion(&descodificador.GetPixelDataTransformedAsync(
        BitmapPixelFormat::Rgba8,
        BitmapAlphaMode::Ignore,
        &transformacion,
        ExifOrientationMode::IgnoreExifOrientation,
        ColorManagementMode::DoNotColorManage,
    )?)?;
    let pixeles = datos.DetachPixelData()?.to_vec();

    let bytes = codificar_jpeg(&pixeles, nuevo_ancho, nuevo_alto)?;
    Ok((bytes, nuevo_ancho, nuevo_alto))
}

/// Pixeles RGBA a JPEG, con la calidad de [`CALIDAD_JPEG`].
fn codificar_jpeg(pixeles: &[u8], ancho: u32, alto: u32) -> Result<Vec<u8>, windows::core::Error> {
    use windows::Foundation::{PropertyType, PropertyValue};
    use windows::Graphics::Imaging::{
        BitmapAlphaMode, BitmapEncoder, BitmapPixelFormat, BitmapPropertySet, BitmapTypedValue,
    };
    use windows::Storage::Streams::{DataReader, InMemoryRandomAccessStream};
    use windows::core::HSTRING;

    let salida = InMemoryRandomAccessStream::new()?;
    let opciones = BitmapPropertySet::new()?;
    opciones.Insert(
        &HSTRING::from("ImageQuality"),
        &BitmapTypedValue::Create(
            &PropertyValue::CreateSingle(CALIDAD_JPEG)?,
            PropertyType::Single,
        )?,
    )?;
    let codificador = esperar_operacion(&BitmapEncoder::CreateWithEncodingOptionsAsync(
        BitmapEncoder::JpegEncoderId()?,
        &salida,
        &opciones,
    )?)?;
    codificador.SetPixelData(
        BitmapPixelFormat::Rgba8,
        BitmapAlphaMode::Ignore,
        ancho,
        alto,
        // 96 ppp es el valor neutro de WIC. La resolucion de verdad la fija el
        // PDF con el tamano al que dibuja la imagen, no la cabecera del JPEG.
        96.0,
        96.0,
        pixeles,
    )?;
    esperar_accion(&codificador.FlushAsync()?)?;

    salida.Seek(0)?;
    let cuantos = salida.Size()? as u32;
    let lector = DataReader::CreateDataReader(&salida.GetInputStreamAt(0)?)?;
    esperar_operacion(&lector.LoadAsync(cuantos)?)?;
    let mut bytes = vec![0u8; cuantos as usize];
    lector.ReadBytes(&mut bytes)?;
    Ok(bytes)
}

/// Igual que la de `lib.rs`, repetida aqui porque aquella es privada y en esta
/// tanda no se toca ese fichero mas que para declarar este modulo.
fn esperar_operacion<T>(
    operacion: &windows_future::IAsyncOperation<T>,
) -> Result<T, windows::core::Error>
where
    T: windows::core::RuntimeType + 'static,
{
    while operacion.Status()? == windows_future::AsyncStatus::Started {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    operacion.GetResults()
}

fn esperar_accion(accion: &windows_future::IAsyncAction) -> Result<(), windows::core::Error> {
    while accion.Status()? == windows_future::AsyncStatus::Started {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    accion.GetResults()
}

/// COM del hilo, soltado al salir solo si fuimos nosotros quien lo abrio.
///
/// Es la misma guardia que la de `lib.rs`, repetida por la misma razon que las
/// dos esperas de arriba: alli es privada y este modulo no puede tocar ese
/// fichero mas que para declararse.
struct ComDelHilo {
    nuestro: bool,
}

impl ComDelHilo {
    fn nuevo() -> ComDelHilo {
        use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
        // SAFETY: `CoInitializeEx` no tiene precondiciones; es la forma normal
        // de entrar en COM. Se guarda si la cuenta subio para deshacerlo con
        // exactitud en `Drop` y no de mas.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        ComDelHilo {
            nuestro: hr != windows::Win32::Foundation::RPC_E_CHANGED_MODE,
        }
    }
}

impl Drop for ComDelHilo {
    fn drop(&mut self) {
        if self.nuestro {
            // SAFETY: empareja exactamente el `CoInitializeEx` de arriba, y
            // para cuando corre ya no queda viva ninguna interfaz creada aqui.
            unsafe { windows::Win32::System::Com::CoUninitialize() };
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Directorio propio de cada prueba, vaciado al ENTRAR: una prueba que
    /// falla se va por el panico sin limpiar, y el fichero viejo haria pasar
    /// la siguiente por la razon equivocada.
    fn temporal(etiqueta: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("pixpin-aligerar-{etiqueta}"));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Monta un PDF con la tabla `xref` bien calculada a partir de los cuerpos
    /// de sus objetos (el 1 es el primero de la lista).
    fn pdf_con(objetos: Vec<Vec<u8>>) -> Vec<u8> {
        let mut salida: Vec<u8> = b"%PDF-1.4\n".to_vec();
        let mut donde = Vec::new();
        for (i, cuerpo) in objetos.iter().enumerate() {
            donde.push(salida.len());
            salida.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
            salida.extend_from_slice(cuerpo);
            salida.extend_from_slice(b"\nendobj\n");
        }
        let inicio = salida.len();
        salida.extend_from_slice(format!("xref\n0 {}\n", objetos.len() + 1).as_bytes());
        salida.extend_from_slice(b"0000000000 65535 f \n");
        for p in &donde {
            salida.extend_from_slice(format!("{p:010} 00000 n \n").as_bytes());
        }
        salida.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{inicio}\n%%EOF\n",
                objetos.len() + 1
            )
            .as_bytes(),
        );
        salida
    }

    /// Un JPEG de verdad, hecho con WIC, con un dibujo que NO se comprime a
    /// nada: un patron que cambia en cada pixel obliga al codificador a
    /// guardar detalle. Un relleno liso saldria en cuatro kilobytes y la
    /// prueba no mediria nada.
    fn jpeg_de_prueba(ancho: u32, alto: u32) -> Vec<u8> {
        let _com = ComDelHilo::nuevo();
        let mut pixeles = vec![0u8; (ancho as usize) * (alto as usize) * 4];
        for y in 0..alto as usize {
            for x in 0..ancho as usize {
                let i = (y * ancho as usize + x) * 4;
                pixeles[i] = ((x * 7 + y * 3) % 256) as u8;
                pixeles[i + 1] = ((x ^ y) % 256) as u8;
                pixeles[i + 2] = ((x * 13 + y * 29) % 256) as u8;
                pixeles[i + 3] = 255;
            }
        }
        codificar_jpeg(&pixeles, ancho, alto).expect("WIC deberia codificar un JPEG")
    }

    /// Un escaneo: un A4 con una sola foto a lo ancho, a los pixeles que se
    /// pidan.
    fn pdf_escaneado(ancho_px: u32, alto_px: u32) -> Vec<u8> {
        let jpeg = jpeg_de_prueba(ancho_px, alto_px);
        let mut imagen: Vec<u8> = format!(
            "<< /Type /XObject /Subtype /Image /Width {ancho_px} /Height {alto_px} \
             /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
            jpeg.len()
        )
        .into_bytes();
        imagen.extend_from_slice(&jpeg);
        imagen.extend_from_slice(b"\nendstream");

        // **Con texto y con una raya, no solo con la foto.** La cabecera de
        // este modulo promete «sin tocar el texto ni los vectores» y hasta
        // ahora ningun fixture tenia ni un `BT … Tj` ni un trazo: se estaba
        // probando que una foto baja de peso, no que lo demas sobrevive, que
        // es justo donde un fallo silencioso de la `xref` borra cosas.
        let dibujo = b"q 595.28 0 0 841.89 0 0 cm /Im0 Do Q\n\
                       BT /F1 12 Tf 50 700 Td (Factura 2026-0042) Tj ET\n\
                       2 w 50 650 m 545 650 l S"
            .to_vec();
        let mut contenido: Vec<u8> =
            format!("<< /Length {} >>\nstream\n", dibujo.len()).into_bytes();
        contenido.extend_from_slice(&dibujo);
        contenido.extend_from_slice(b"\nendstream");

        pdf_con(vec![
            b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595.28 841.89] \
              /Contents 4 0 R /Resources << /XObject << /Im0 5 0 R >> \
              /Font << /F1 6 0 R >> >> >>"
                .to_vec(),
            contenido,
            imagen,
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
        ])
    }

    // --- el modelo minimo de PDF -------------------------------------------

    #[test]
    fn un_mayor_que_dentro_de_una_cadena_no_cierra_el_diccionario() {
        // Es el fallo clasico de quien busca ">>" a pelo: se come medio
        // documento y luego escribe una tabla xref que apunta a cualquier sitio.
        let b = b"<< /A (un >> tramposo) /B 7 >>rastro";
        let fin = fin_del_diccionario(b, 0).unwrap();
        assert_eq!(&b[fin..], b"rastro");
        assert_eq!(entero_de(b, valor(b, (0, fin), "B")), Some(7));
    }

    #[test]
    fn una_referencia_es_un_valor_y_no_tres() {
        let b = b"<< /Resources 12 0 R /Type /Page >>";
        let dic = (0, b.len());
        assert!(
            nombre_es(b, dic, "Type", "Page"),
            "si `12 0 R` se leyera como tres cosas, la clave siguiente saldria mal"
        );
        assert_eq!(referencia(b, valor(b, dic, "Resources").unwrap()), Some(12));
    }

    #[test]
    fn un_width_de_dentro_de_un_subdiccionario_no_se_confunde_con_el_de_fuera() {
        let b = b"<< /Extra << /Width 99 >> /Width 8 >>";
        assert_eq!(entero_de(b, valor(b, (0, b.len()), "Width")), Some(8));
    }

    #[test]
    fn un_endobj_dentro_de_un_flujo_binario_no_corta_el_objeto() {
        // El `/Length` es la verdad; buscar `endobj` a ciegas partiria el
        // objeto por la mitad y el fichero saldria roto.
        let datos = b"basura endobj mas basura";
        let mut cuerpo: Vec<u8> = format!("<< /Length {} >>\nstream\n", datos.len()).into_bytes();
        cuerpo.extend_from_slice(datos);
        cuerpo.extend_from_slice(b"\nendstream");
        let pdf = pdf_con(vec![b"<< /Type /Catalog >>".to_vec(), cuerpo]);

        let objetos = objetos(&pdf).objetos;
        assert_eq!(objetos.len(), 2);
        let (i, f) = objetos[1].flujo.unwrap();
        assert_eq!(&pdf[i..f], datos);
    }

    #[test]
    fn se_reconocen_las_imagenes_y_lo_que_ocupan() {
        let pdf = pdf_escaneado(300, 400);
        let informe = inspeccionar(&pdf);
        assert!(informe.estorbos.is_empty(), "{:?}", informe.estorbos);
        assert_eq!(informe.imagenes.len(), 1);
        let img = &informe.imagenes[0];
        assert_eq!((img.ancho, img.alto), (300, 400));
        assert_eq!(img.filtro, "/DCTDecode");
        assert!(img.se_puede_tocar);
        assert_eq!(informe.bytes_en_imagenes(), img.bytes);
    }

    #[test]
    fn un_pdf_cifrado_se_dice_y_no_se_toca() {
        let mut pdf = pdf_escaneado(300, 400);
        // El trailer del final es el que manda.
        pdf.extend_from_slice(b"\ntrailer\n<< /Size 6 /Root 1 0 R /Encrypt 9 0 R >>\n%%EOF\n");
        let informe = inspeccionar(&pdf);
        assert!(informe.estorbos.contains(&Estorbo::Cifrado));
        assert!(!informe.se_puede_intentar());
    }

    #[test]
    fn un_pdf_con_los_objetos_comprimidos_se_dice_y_no_se_toca() {
        let pdf = pdf_con(vec![
            b"<< /Type /Catalog >>".to_vec(),
            b"<< /Type /ObjStm /N 3 /First 12 /Length 0 >>\nstream\n\nendstream".to_vec(),
        ]);
        let informe = inspeccionar(&pdf);
        assert!(informe.estorbos.contains(&Estorbo::ObjetosEnFlujo));
        assert!(!informe.se_puede_intentar());
    }

    #[test]
    fn una_imagen_con_mascara_o_filtro_raro_se_deja_en_paz() {
        for dic in [
            "<< /Type /XObject /Subtype /Image /Width 9 /Height 9 /ColorSpace /DeviceRGB \
             /BitsPerComponent 8 /Filter /DCTDecode /SMask 9 0 R /Length 3 >>",
            "<< /Type /XObject /Subtype /Image /Width 9 /Height 9 /ColorSpace /DeviceRGB \
             /BitsPerComponent 8 /Filter /FlateDecode /Length 3 >>",
            "<< /Type /XObject /Subtype /Image /Width 9 /Height 9 /ColorSpace [/ICCBased 9 0 R] \
             /BitsPerComponent 8 /Filter /DCTDecode /Length 3 >>",
            "<< /Type /XObject /Subtype /Image /Width 9 /Height 9 /ColorSpace /DeviceRGB \
             /BitsPerComponent 1 /Filter /DCTDecode /Length 3 >>",
        ] {
            let cuerpo = format!("{dic}\nstream\nabc\nendstream").into_bytes();
            let pdf = pdf_con(vec![b"<< /Type /Catalog >>".to_vec(), cuerpo]);
            let informe = inspeccionar(&pdf);
            assert_eq!(informe.imagenes.len(), 1, "la imagen se ve: {dic}");
            assert!(
                !informe.imagenes[0].se_puede_tocar,
                "pero no se debe tocar: {dic}"
            );
            assert!(!informe.se_puede_intentar());
        }
    }

    #[test]
    fn el_ancho_objetivo_sale_de_la_pagina_y_no_del_aire() {
        // Un A4 a 200 ppp son 8,27 pulgadas por 200.
        assert_eq!(ancho_objetivo(595.28), 1654);
        // Media pagina, la mitad de pixeles.
        assert_eq!(ancho_objetivo(297.64), 827);
        // Una caja imposible cae en el A4 en vez de en cero o en infinito.
        assert_eq!(ancho_objetivo(0.0), 1654);
        assert_eq!(ancho_objetivo(f64::NAN), 1654);
    }

    #[test]
    fn la_pagina_de_cada_imagen_se_encuentra_aunque_la_caja_la_herede() {
        let pdf = pdf_con(vec![
            b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
            // La caja esta en el padre, no en la pagina: es como lo escriben
            // casi todos los generadores.
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 1000 500] >>".to_vec(),
            b"<< /Type /Page /Parent 2 0 R /Resources << /XObject << /Im0 4 0 R >> >> >>".to_vec(),
            b"<< /Type /XObject /Subtype /Image /Width 9 /Height 9 >>".to_vec(),
        ]);
        let objetos = objetos(&pdf).objetos;
        let anchos = anchos_de_pagina(&pdf, &objetos);
        assert_eq!(anchos.get(&4), Some(&1000.0));
    }

    #[test]
    fn un_parent_que_apunta_a_si_mismo_no_cuelga_el_programa() {
        let pdf = pdf_con(vec![
            b"<< /Type /Catalog >>".to_vec(),
            b"<< /Type /Page /Parent 2 0 R /Resources << /XObject << /Im0 3 0 R >> >> >>".to_vec(),
            b"<< /Type /XObject /Subtype /Image /Width 9 /Height 9 >>".to_vec(),
        ]);
        let objetos = objetos(&pdf).objetos;
        let anchos = anchos_de_pagina(&pdf, &objetos);
        assert_eq!(
            anchos.get(&3),
            Some(&ANCHO_A4_PUNTOS),
            "sin caja que encontrar, se supone un A4"
        );
    }

    #[test]
    fn el_trailer_nuevo_tira_lo_que_apuntaba_al_fichero_viejo() {
        let b = b"<< /Size 9 /Root 1 0 R /Prev 12345 /Info 4 0 R /XRefStm 99 >>";
        let nuevo = trailer_nuevo(b, (0, b.len()), 6);
        let texto = String::from_utf8(nuevo).unwrap();
        assert!(texto.contains("/Size 6"));
        assert!(texto.contains("/Root 1 0 R"));
        assert!(texto.contains("/Info 4 0 R"));
        assert!(
            !texto.contains("/Prev"),
            "apunta a una tabla que ya no existe"
        );
        assert!(!texto.contains("/XRefStm"));
        assert!(!texto.contains("/Size 9"));
    }

    #[test]
    fn la_cuenta_de_lo_que_se_gana_no_se_cree_una_subida() {
        assert!(gana_bastante(1000, 700, 0.25));
        assert!(!gana_bastante(1000, 800, 0.25));
        assert!(!gana_bastante(1000, 1200, 0.25), "subir no es ganar");
        assert!(!gana_bastante(0, 0, 0.15), "de la nada no se quita nada");
    }

    // --- de punta a punta, con Windows de por medio ------------------------

    #[test]
    fn un_escaneo_grande_baja_de_peso_y_se_sigue_leyendo() {
        let d = temporal("escaneo");
        // 2480 x 3508 es el A4 a 300 ppp, que es lo que saca un escaner de
        // oficina puesto en «calidad»: por encima de los 200 de destino.
        let pdf = pdf_escaneado(2480, 3508);
        let origen = d.join("escaneo.pdf");
        std::fs::write(&origen, &pdf).unwrap();

        let salida = d.join("ligero.pdf");
        let r = aligerar(&origen, &salida).expect("no deberia fallar");
        let Resultado::Aligerado { antes, despues } = r else {
            panic!("tenia que bajar: {r:?}");
        };
        assert!(despues < antes);
        assert!(
            (1.0 - despues as f64 / antes as f64) >= MEJORA_MINIMA_DEL_FICHERO,
            "de {antes} a {despues} no llega al 15 %"
        );

        // Y lo importante: el fichero nuevo se abre y tiene la misma pagina.
        let doc = Documento::abrir(&salida).expect("el aligerado tiene que abrirse");
        assert_eq!(doc.paginas(), 1);
        drop(doc);

        // La foto quedo a los pixeles de 200 ppp, no a los de 300.
        let nuevo = std::fs::read(&salida).unwrap();
        let informe = inspeccionar(&nuevo);
        assert_eq!(informe.imagenes.len(), 1);
        assert_eq!(informe.imagenes[0].ancho, ancho_objetivo(595.28));

        // **Y el texto, la fuente y la raya siguen ahi.** Es la promesa de la
        // cabecera del modulo —«sin tocar el texto ni los vectores»— y es lo
        // que ninguna prueba miraba: `reescribir` reconstruye la `xref`
        // entera, que es exactamente donde un fallo silencioso borra texto.
        let como_texto = String::from_utf8_lossy(&nuevo);
        assert!(
            como_texto.contains("(Factura 2026-0042) Tj"),
            "se perdio el texto de la pagina al aligerar"
        );
        assert!(
            como_texto.contains("/BaseFont /Helvetica"),
            "se perdio la fuente incrustada al aligerar"
        );
        assert!(
            como_texto.contains("50 650 m 545 650 l S"),
            "se perdio el trazo vectorial al aligerar"
        );
        // Caso negativo, para que las tres de arriba no pasen por accidente:
        // la foto SI cambio, que es lo unico que este trabajo puede tocar.
        assert!(
            !como_texto.contains(&String::from_utf8_lossy(&pdf).to_string()),
            "no cambio nada: el fichero de salida es el de entrada"
        );

        eprintln!("escaneo: {antes} -> {despues} bytes");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn un_objeto_que_no_se_entiende_se_cuenta_y_para_el_trabajo() {
        // **El fallo:** `reescribir` solo escribe lo que hay en la lista de
        // objetos, y un objeto que `cuerpo_del_objeto` no supiera parsear no
        // entraba en la lista: desaparecia del fichero de salida **sin que
        // nada lo contara**. Podia ser una fuente, las anotaciones o los
        // campos de un formulario.
        let mut pdf = pdf_escaneado(2480, 3508);
        // Una cabecera de objeto de verdad con un cuerpo que no se cierra.
        pdf.extend_from_slice(b"\n99 0 obj\n<< /Type /Annot /Rect [0 0 1 1]\n");

        let informe = inspeccionar(&pdf);
        assert!(
            informe.estorbos.contains(&Estorbo::ObjetosSinEntender),
            "el tramo sin entender no se conto: {:?}",
            informe.estorbos
        );

        // Y el trabajo entero se para en vez de tirarlo.
        let d = temporal("sin-entender");
        let origen = d.join("raro.pdf");
        std::fs::write(&origen, &pdf).unwrap();
        let salida = d.join("ligero.pdf");
        let r = aligerar(&origen, &salida).expect("no deberia fallar, solo negarse");
        assert_eq!(
            r,
            Resultado::NoSeGanaNada(Razon::NoSeEntiende(Estorbo::ObjetosSinEntender))
        );
        assert!(!salida.exists(), "escribio pese a no entender el fichero");

        // El caso negativo: el mismo PDF sin el trozo raro si se entiende.
        let limpio = pdf_escaneado(2480, 3508);
        assert!(
            !inspeccionar(&limpio)
                .estorbos
                .contains(&Estorbo::ObjetosSinEntender),
            "un PDF bueno no puede dar este estorbo"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn aligerar_encima_del_original_deja_una_copia_al_lado() {
        // **No quedaba copia de nada.** Aligerar es la unica operacion del
        // programa que reescribe un fichero que el usuario trajo de fuera, y
        // la unica red era una comprobacion estructural.
        let d = temporal("copia");
        let sitio = d.join("factura.pdf");
        let pdf = pdf_escaneado(2480, 3508);
        std::fs::write(&sitio, &pdf).unwrap();

        let r = aligerar(&sitio, &sitio).expect("no deberia fallar");
        assert!(
            matches!(r, Resultado::Aligerado { .. }),
            "tenia que bajar: {r:?}"
        );

        let copias: Vec<_> = std::fs::read_dir(&d)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.contains("original-"))
            .collect();
        assert_eq!(copias.len(), 1, "no quedo copia del original: {copias:?}");
        assert_eq!(
            std::fs::read(d.join(&copias[0])).unwrap(),
            pdf,
            "la copia no es el fichero que habia antes"
        );
        // Y no se quedo ningun temporal a medias por el camino.
        let sobras: Vec<_> = std::fs::read_dir(&d)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.contains("aligerando-"))
            .collect();
        assert!(sobras.is_empty(), "quedo un temporal: {sobras:?}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn un_pdf_que_ya_esta_al_minimo_dice_que_no_puede_quitar_nada_y_no_escribe() {
        let d = temporal("alminimo");
        // Un A4 a menos de 200 ppp: no hay nada que bajar.
        let pdf = pdf_escaneado(800, 1131);
        let origen = d.join("ya.pdf");
        std::fs::write(&origen, &pdf).unwrap();
        let salida = d.join("ligero.pdf");

        let r = aligerar(&origen, &salida).unwrap();
        assert_eq!(r, Resultado::NoSeGanaNada(Razon::YaEstaAlMinimo));
        assert!(
            !salida.exists(),
            "no puede dejar un fichero por ahi cuando no ha hecho nada"
        );
        assert_eq!(
            std::fs::read(&origen).unwrap(),
            pdf,
            "y el original tiene que seguir igual, byte a byte"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn un_pdf_sin_fotos_lo_dice_y_no_escribe_nada() {
        let d = temporal("sinfotos");
        let pdf = pdf_con(vec![
            b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Resources << >> >>".to_vec(),
        ]);
        let origen = d.join("solotexto.pdf");
        std::fs::write(&origen, &pdf).unwrap();
        let salida = d.join("ligero.pdf");
        assert_eq!(
            aligerar(&origen, &salida).unwrap(),
            Resultado::NoSeGanaNada(Razon::SinFotosQueBajar)
        );
        assert!(!salida.exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn lo_que_no_es_un_pdf_se_rechaza_por_lo_que_es() {
        let d = temporal("nopdf");
        let ruta = d.join("cosa.pdf");
        std::fs::write(&ruta, b"PK esto es un zip, no un PDF").unwrap();
        assert!(matches!(
            aligerar(&ruta, &d.join("x.pdf")),
            Err(ErrorPdf::NoEsPdf(_))
        ));
        assert!(matches!(
            aligerar(&d.join("no-esta.pdf"), &d.join("x.pdf")),
            Err(ErrorPdf::NoExiste(_))
        ));
        let _ = std::fs::remove_dir_all(&d);
    }
}
