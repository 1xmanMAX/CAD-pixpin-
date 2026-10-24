//! **El plano de un PDF leido como rayas y no como fotografia.** Puerto de
//! `motor/PlanoDePdf.kt` del movil.
//!
//! Un plano de AutoCAD no es una imagen: por dentro son lineas, y por eso se
//! puede mirar de cerca sin que se deshaga. Todo lo que hacia PixPin con un
//! PDF —la hoja de la pagina web, el papel del lienzo— pasaba por
//! **rasterizarlo** con `Windows.Data.Pdf`, y una vez son pixeles hay un
//! techo: ampliar mas de la cuenta ensena el grano.
//!
//! Esto lee las lineas. Entra el archivo tal cual y sale la geometria de una
//! pagina: caminos con su color y su grosor, repartidos por **capas** —las de
//! AutoCAD, que el PDF guarda como grupos de contenido opcional (OCG)— y el
//! texto con su sitio y su tamano. Con eso la pagina web pinta el plano con
//! el navegador y se ve nitido a cualquier aumento, pesando ademas bastante
//! menos que la fotografia que sustituye. Ver [`crate::plano_web`].
//!
//! ## Que entiende y que no
//!
//! Lo que un plano usa: caminos (`m l c v y re h`), pintarlos (`S s f B b n`),
//! la matriz (`cm`), el estado (`q Q w gs d`), los colores de los espacios
//! corrientes (`G g RG rg K k`, `cs/CS` + `sc/scn`, paletas, ICC, tintas), las
//! capas (`BDC /OC … EMC`), los recortes rectangulares (`W n`), los
//! formularios (`Do` de un `/Form`), las fotos JPEG tal cual y el texto
//! (`BT … ET`).
//!
//! No entiende, **a proposito**: sombreados (`sh`), patrones, imagenes que no
//! son JPEG, imagenes incrustadas (`BI`). No revientan: se cuentan en
//! [`Plano::sin_entender`], y con una sola la pagina se manda como
//! fotografia —una lamina borrosa se ve; una a la que le falta algo, no—.
//!
//! ## Por que aqui y no con lopdf
//!
//! El analizador de objetos ya estaba en este crate (`union`, el puerto de
//! `PdfUnion`): indices clasicos y en flujo, flujos de objetos y Flate. Leer
//! un plano es eso mas el interprete del contenido, que ninguna libreria
//! trae hecho para esto. Meter lopdf seria arrastrar su arbol entero a
//! `pixpin` (y la prueba de capas lo prohibe) para usar la mitad que ya hay.
//!
//! ## Los numeros en punto fijo
//!
//! Las coordenadas salen en **pasos de 1/[`FINEZA`] de punto** y como enteros.
//! Es lo que permite (1) empalmar segmentos comparando extremos con `==`
//! —AutoCAD escribe cada tramo de una polilinea por separado— y (2) escribir
//! la web como diferencias pequenas. Un 1/256 de punto es una milesima de
//! milimetro: ni ampliando cuatrocientas veces se ve el escalon.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use crate::union::{Archivo, Dicc, Valor, en, entero};

/// Pasos por punto de las coordenadas. Ver la cabecera.
pub const FINEZA: i32 = 256;

/// **Tope de puntos de una pagina.** Un plano grande de verdad ronda el
/// millon y medio; pasado el tope se deja de leer y se entrega lo que haya
/// con [`Plano::cortado`], que quien llame usa para volver a la imagen. Es lo
/// que evita que un archivo enorme —o roto— se coma la memoria.
pub const TOPE_DE_PUNTOS: usize = 6_000_000;

/// Hasta donde se sigue metiendo un formulario dentro de otro.
const FONDO_MAXIMO: u32 = 12;

/// **Cuantas brochas distintas como mucho.** Un degradado hecho de mil
/// lineas de mil colores haria mil brochas: pasado el tope el color y el
/// grosor se redondean y las parecidas se juntan.
pub const TOPE_DE_BROCHAS: usize = 2048;

/// Cuantos rellenos se guardan en su orden antes de volver a juntarlos por
/// color. Ver `relleno_en_orden`.
const TOPE_DE_RELLENOS_EN_ORDEN: usize = 6000;

/// Tope de fotos, en bytes: pasado eso la pagina web pesaria mas que la
/// fotografia.
const PESO_DE_LAS_FOTOS: usize = 8 * 1024 * 1024;

pub const MOVER: u8 = 1;
pub const LINEA: u8 = 2;
pub const CURVA: u8 = 3;
pub const CERRAR: u8 = 4;

// ---------------------------------------------------------------------------
// Lo que sale

/// Una capa del plano: en AutoCAD, una capa del dibujo.
#[derive(Clone, Debug, PartialEq)]
pub struct Capa {
    pub nombre: String,
    pub encendida: bool,
}

/// **Un monton de caminos que se pintan igual**: misma capa, color, grosor y
/// clase de pintada. El lienzo del navegador cobra por cambio de pincel: mil
/// caminos negros de un pelo son **una** orden de pintar, no mil.
///
/// `ops` son las ordenes ([`MOVER`], [`LINEA`], [`CURVA`], [`CERRAR`]) y
/// `xs`/`ys` los puntos que gastan en orden: `MOVER` y `LINEA` uno, `CURVA`
/// tres, `CERRAR` ninguno.
#[derive(Clone, Debug)]
pub struct Brocha {
    pub capa: i32,
    /// `0xRRGGBB`.
    pub color: u32,
    pub alfa: f64,
    /// En puntos del papel; 0 es un pelo.
    pub grosor: f64,
    pub relleno: bool,
    /// El rayado, en puntos, o vacio si es continua.
    pub raya: Vec<f64>,
    pub ops: Vec<u8>,
    pub xs: Vec<i32>,
    pub ys: Vec<i32>,
    /// **Relleno par-impar** (`f*`): donde los caminos se cruzan dos veces
    /// queda hueco. Sin esto las letras con agujeros salian macizas.
    pub par_impar: bool,
}

impl Brocha {
    pub fn puntos(&self) -> usize {
        self.xs.len()
    }
}

/// Un texto del plano con la matriz que lo coloca: `a b c d x y` llevan «una
/// letra de un punto de alto apoyada en el origen, escrita hacia la derecha»
/// hasta donde va en el papel. `ancho` (en emes) es lo que ocupa segun el
/// PDF: el visor estira la letra del navegador hasta eso, y un rotulo con
/// una tipografia que no esta no se sale de su casilla.
#[derive(Clone, Debug)]
pub struct Texto {
    pub capa: i32,
    pub color: u32,
    pub alfa: f64,
    pub texto: String,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub x: f64,
    pub y: f64,
    pub ancho: f64,
    /// `sans-serif`, `serif`, `monospace` o `sans-serif-condensed`.
    pub familia: &'static str,
    pub negrita: bool,
    pub cursiva: bool,
}

/// **Una foto del PDF que viaja tal cual**, en los bytes JPEG en que ya
/// estaba. La matriz lleva el cuadrado de la imagen —con el (0,0) arriba a la
/// izquierda— hasta el papel. Dos con el mismo `id` son la misma imagen
/// colocada en dos sitios: un plano de Revit coloca 81 que son 21, y
/// escribirlas una vez es la cuarta parte de peso.
#[derive(Clone, Debug)]
pub struct Imagen {
    pub capa: i32,
    pub alfa: f64,
    pub tipo: &'static str,
    pub datos: Arc<Vec<u8>>,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub x: f64,
    pub y: f64,
    /// La transparencia (`/SMask`) en sus bytes de origen: gris, negro
    /// transparente. Se junta con la de color quien pinta.
    pub mascara: Option<Arc<Vec<u8>>>,
    pub tipo_mascara: Option<&'static str>,
    pub id: usize,
}

/// Una pagina leida. Las medidas van en puntos del papel, ya giradas.
#[derive(Clone, Debug)]
pub struct Plano {
    pub ancho: f64,
    pub alto: f64,
    pub capas: Vec<Capa>,
    pub brochas: Vec<Brocha>,
    pub textos: Vec<Texto>,
    pub fotos: Vec<Imagen>,
    /// **Cuantas cosas pinta la pagina que aqui no estan.** Con una, la pagina
    /// se manda como fotografia.
    pub sin_entender: u32,
    /// Si se llego a [`TOPE_DE_PUNTOS`] y la lectura se corto.
    pub cortado: bool,
}

impl Plano {
    pub fn puntos(&self) -> usize {
        self.brochas.iter().map(Brocha::puntos).sum()
    }

    /// Si hay bastante geometria como para mandarla en vez de la foto. Un
    /// escaneo da cero: es una imagen y nada mas.
    pub fn vale_la_pena(&self) -> bool {
        !self.cortado && (self.puntos() > 16 || self.textos.len() > 2)
    }

    /// **Si esta pagina puede ir como lineas**: se entiende entera, no se
    /// corto y tiene geometria. Es la regla de `PlanoWeb.deArchivo`.
    pub fn se_manda_como_lineas(&self) -> bool {
        self.vale_la_pena() && self.sin_entender == 0
    }
}

// ---------------------------------------------------------------------------
// Leer

/// La pagina `pagina` (desde 0) de estos bytes, o `None` si no se entiende
/// (no es un PDF, esta cifrado, la pagina no existe o esta girada en angulo
/// raro).
pub fn de_bytes(bytes: &[u8], pagina: usize) -> Option<Plano> {
    let archivo = Archivo::leer(bytes)?;
    de_archivo_leido(&archivo, pagina)
}

/// Lo mismo desde el disco.
pub fn de_ruta(ruta: &Path, pagina: usize) -> Option<Plano> {
    de_bytes(&std::fs::read(ruta).ok()?, pagina)
}

/// **Varias paginas de una vez**, abriendo el indice una sola: el analisis
/// del indice es lo caro de abrir. Se entregan de una en una a `cada` y no
/// en una lista: tres planos grandes juntos en memoria son cientos de megas.
pub fn con_cada(bytes: &[u8], cuales: &[usize], mut cada: impl FnMut(usize, Option<Plano>)) {
    let archivo = Archivo::leer(bytes);
    for &p in cuales {
        cada(p, archivo.as_ref().and_then(|a| de_archivo_leido(a, p)));
    }
}

fn de_archivo_leido(archivo: &Archivo, pagina: usize) -> Option<Plano> {
    if archivo.cifrado() {
        return None;
    }
    let n = *archivo.paginas().get(pagina)?;
    let hoja = archivo.dicc_de(Some(&Valor::Ref(n, 0)))?;
    let caja = media_box(archivo, &hoja)?;
    if caja.giro % 90 != 0 {
        return None;
    }
    let datos = contenido(archivo, &hoja)?;
    let recursos = match heredado(archivo, &hoja, b"Resources") {
        Some(Valor::Dicc(d)) => Some(Rc::new(d)),
        _ => None,
    };
    let mut i = Interprete::nuevo(archivo, caja);
    i.capas_del_documento();
    i.ejecutar(&datos, recursos.as_ref(), 0);
    Some(i.cosecha())
}

/// El contenido de la pagina ya descomprimido. `/Contents` puede ser una
/// lista de flujos que se leen **como si fueran uno** (el formato deja
/// partirlo incluso a mitad de una orden).
fn contenido(archivo: &Archivo, hoja: &Dicc) -> Option<Vec<u8>> {
    let trozos: Vec<(Dicc, Vec<u8>)> = match archivo.resolver(en(hoja, b"Contents"))? {
        Valor::Flujo(d, datos) => vec![(d, datos)],
        Valor::Lista(l) => l
            .iter()
            .filter_map(|v| match archivo.resolver(Some(v))? {
                Valor::Flujo(d, datos) => Some((d, datos)),
                _ => None,
            })
            .collect(),
        _ => return None,
    };
    let mut salida = Vec::new();
    for (d, datos) in &trozos {
        let Some(t) = descomprimir(archivo, d, datos) else {
            continue;
        };
        salida.extend_from_slice(&t);
        salida.push(b'\n');
    }
    (!salida.is_empty()).then_some(salida)
}

/// Los nombres de los filtros de un flujo, en orden.
fn filtros_de(archivo: &Archivo, d: &Dicc) -> Option<Vec<Vec<u8>>> {
    match archivo.resolver(en(d, b"Filter")) {
        None | Some(Valor::Nulo) => Some(Vec::new()),
        Some(Valor::Nombre(n)) => Some(vec![n]),
        Some(Valor::Lista(l)) => Some(
            l.iter()
                .filter_map(|v| match v {
                    Valor::Nombre(n) => Some(n.clone()),
                    _ => None,
                })
                .collect(),
        ),
        _ => None,
    }
}

/// Los parametros del filtro `i` (`/DecodeParms`, suelto o en lista).
fn parametros_de(archivo: &Archivo, d: &Dicc, i: usize) -> Option<Dicc> {
    match archivo.resolver(en(d, b"DecodeParms"))? {
        Valor::Dicc(p) => (i == 0).then_some(p),
        Valor::Lista(l) => match archivo.resolver(l.get(i))? {
            Valor::Dicc(p) => Some(p),
            _ => None,
        },
        _ => None,
    }
}

/// Aplica un filtro. `None` si no se sabe.
fn un_filtro(
    archivo: &Archivo,
    d: &Dicc,
    i: usize,
    nombre: &[u8],
    datos: &[u8],
) -> Option<Vec<u8>> {
    match nombre {
        b"FlateDecode" | b"Fl" => {
            let mut z = flate2::read::ZlibDecoder::new(datos);
            let mut fuera = Vec::new();
            // Un flujo cortado al final aun da lo que llevaba.
            if z.read_to_end(&mut fuera).is_err() && fuera.is_empty() {
                return None;
            }
            match parametros_de(archivo, d, i) {
                Some(p) => {
                    let predictor = entero(en(&p, b"Predictor")).unwrap_or(1);
                    if predictor >= 10 {
                        let columnas = entero(en(&p, b"Columns")).unwrap_or(1).max(1) as usize;
                        let colores = entero(en(&p, b"Colors")).unwrap_or(1).max(1) as usize;
                        let bits = entero(en(&p, b"BitsPerComponent")).unwrap_or(8).max(1) as usize;
                        crate::union::deshacer_png(&fuera, (columnas * colores * bits).div_ceil(8))
                    } else if predictor <= 1 {
                        Some(fuera)
                    } else {
                        None
                    }
                }
                None => Some(fuera),
            }
        }
        b"ASCIIHexDecode" | b"AHx" => Some(de_hex(datos)),
        b"ASCII85Decode" | b"A85" => de_base85(datos),
        _ => None,
    }
}

/// Los datos de un flujo sin ninguno de sus filtros.
fn descomprimir(archivo: &Archivo, d: &Dicc, datos: &[u8]) -> Option<Vec<u8>> {
    let mut salida = datos.to_vec();
    for (i, f) in filtros_de(archivo, d)?.iter().enumerate() {
        salida = un_filtro(archivo, d, i, f, &salida)?;
    }
    Some(salida)
}

/// **Los datos sin el ultimo filtro**, y cual es: lo que hace falta para
/// pasar una foto tal cual (un JPEG metido ademas en Flate se deshace de
/// Flate y se queda en JPEG).
fn sin_el_ultimo_filtro(archivo: &Archivo, d: &Dicc, datos: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    let filtros = filtros_de(archivo, d)?;
    let (ultimo, antes) = filtros.split_last()?;
    let mut salida = datos.to_vec();
    for (i, f) in antes.iter().enumerate() {
        salida = un_filtro(archivo, d, i, f, &salida)?;
    }
    Some((ultimo.clone(), salida))
}

fn de_hex(datos: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(datos.len() / 2);
    let mut alto: Option<u8> = None;
    for &c in datos {
        if c == b'>' {
            break;
        }
        let Some(v) = (c as char).to_digit(16) else {
            continue;
        };
        match alto.take() {
            None => alto = Some(v as u8),
            Some(a) => out.push(a * 16 + v as u8),
        }
    }
    if let Some(a) = alto {
        out.push(a * 16);
    }
    out
}

fn de_base85(datos: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(datos.len() * 4 / 5);
    let mut grupo = [0u32; 5];
    let mut n = 0;
    let mut i = 0;
    // `<~` delante es opcional.
    if datos.starts_with(b"<~") {
        i = 2;
    }
    while i < datos.len() {
        let c = datos[i];
        i += 1;
        if c == b'~' {
            break;
        }
        if c.is_ascii_whitespace() || c == 0 {
            continue;
        }
        if c == b'z' && n == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
            continue;
        }
        if !(b'!'..=b'u').contains(&c) {
            return None;
        }
        grupo[n] = (c - b'!') as u32;
        n += 1;
        if n == 5 {
            let v = grupo.iter().fold(0u64, |a, &d| a * 85 + d as u64) as u32;
            out.extend_from_slice(&v.to_be_bytes());
            n = 0;
        }
    }
    if n > 1 {
        for g in grupo.iter_mut().skip(n) {
            *g = 84;
        }
        let v = grupo.iter().fold(0u64, |a, &d| a * 85 + d as u64) as u32;
        out.extend_from_slice(&v.to_be_bytes()[..n - 1]);
    }
    Some(out)
}

/// Una entrada de la pagina que puede estar escrita en un padre.
fn heredado(archivo: &Archivo, hoja: &Dicc, clave: &[u8]) -> Option<Valor> {
    let mut d = Some(hoja.clone());
    let mut saltos = 0;
    while let Some(actual) = d {
        saltos += 1;
        if saltos > 32 {
            break;
        }
        if let Some(v) = en(&actual, clave) {
            return archivo.resolver(Some(v));
        }
        d = archivo.dicc_de(en(&actual, b"Parent"));
    }
    None
}

/// Un numero de la sintaxis (`Valor::Numero` lo guarda tal cual venia).
fn numero_de(v: &Valor) -> Option<f64> {
    match v {
        Valor::Numero(t) => std::str::from_utf8(t)
            .ok()?
            .parse::<f64>()
            .ok()
            .filter(|x| x.is_finite()),
        _ => None,
    }
}

fn num(archivo: &Archivo, v: Option<&Valor>) -> Option<f64> {
    numero_de(&archivo.resolver(v)?)
}

/// Los bytes de una cadena del PDF, sin sus delimitadores y sin escapes.
pub(crate) fn bytes_de_cadena(c: &[u8]) -> Vec<u8> {
    if c.first() == Some(&b'<') {
        return de_hex(&c[1..]);
    }
    let dentro = if c.first() == Some(&b'(') && c.last() == Some(&b')') && c.len() >= 2 {
        &c[1..c.len() - 1]
    } else {
        c
    };
    let mut f = Fichas { b: dentro, pos: 0 };
    f.cadena_sin_parentesis()
}

/// Una cadena del PDF a texto: UTF-16 con su marca, o Latin-1.
pub(crate) fn texto_de_cadena(b: &[u8]) -> String {
    if b.len() >= 2 && b[0] == 0xFE && b[1] == 0xFF {
        let u: Vec<u16> = b[2..]
            .chunks_exact(2)
            .map(|p| u16::from_be_bytes([p[0], p[1]]))
            .collect();
        String::from_utf16_lossy(&u)
    } else {
        b.iter().map(|&c| c as char).collect()
    }
}

/// **La caja de la pagina**: donde empieza el papel, cuanto mide y **como se
/// mira**. `/Rotate` no es un adorno: el plano del usuario viene con 270, y
/// `Windows.Data.Pdf` lo aplica al pintar; si aqui no se girara igual las
/// lineas caerian atravesadas sobre lo anotado.
fn media_box(archivo: &Archivo, hoja: &Dicc) -> Option<Caja> {
    let Valor::Lista(l) = heredado(archivo, hoja, b"MediaBox")? else {
        return None;
    };
    let n: Vec<f64> = l.iter().filter_map(|v| num(archivo, Some(v))).collect();
    if n.len() < 4 {
        return None;
    }
    let (x0, y0, x1, y1) = (
        n[0].min(n[2]),
        n[1].min(n[3]),
        n[0].max(n[2]),
        n[1].max(n[3]),
    );
    if x1 - x0 < 1.0 || y1 - y0 < 1.0 {
        return None;
    }
    let giro = heredado(archivo, hoja, b"Rotate")
        .as_ref()
        .and_then(numero_de)
        .map_or(0, |g| g as i32)
        .rem_euclid(360);
    Some(Caja {
        x0,
        y0,
        x1,
        y1,
        giro,
    })
}

/// El papel y su giro. `ancho` y `alto` son los de la pagina **como se mira**.
#[derive(Clone, Copy, Debug)]
struct Caja {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    giro: i32,
}

impl Caja {
    fn de_lado(&self) -> bool {
        self.giro == 90 || self.giro == 270
    }
    fn ancho(&self) -> f64 {
        if self.de_lado() {
            self.y1 - self.y0
        } else {
            self.x1 - self.x0
        }
    }
    fn alto(&self) -> f64 {
        if self.de_lado() {
            self.x1 - self.x0
        } else {
            self.y1 - self.y0
        }
    }
    /// Un punto del PDF donde se ve: origen arriba a la izquierda, y abajo.
    fn en_x(&self, px: f64, py: f64) -> f64 {
        match self.giro {
            90 => py - self.y0,
            180 => self.x1 - px,
            270 => self.y1 - py,
            _ => px - self.x0,
        }
    }
    fn en_y(&self, px: f64, py: f64) -> f64 {
        match self.giro {
            90 => px - self.x0,
            180 => py - self.y0,
            270 => self.x1 - px,
            _ => self.y1 - py,
        }
    }
    /// Una direccion: gira igual pero no se traslada.
    fn vector_x(&self, vx: f64, vy: f64) -> f64 {
        match self.giro {
            90 => vy,
            180 => -vx,
            270 => -vy,
            _ => vx,
        }
    }
    fn vector_y(&self, vx: f64, vy: f64) -> f64 {
        match self.giro {
            90 => vx,
            180 => vy,
            270 => -vx,
            _ => -vy,
        }
    }
}

// ---------------------------------------------------------------------------
// El estado de la maquina de pintar

/// En que espacio de color habla `sc`/`scn`. Con una paleta el numero que
/// viene es un **indice**, no un tono: adivinandolo por cuantos numeros
/// venian, un `scn 5` sobre una paleta salia negro (usuario, 13-sep-2026).
#[derive(Clone, Debug)]
enum Espacio {
    Adivinar,
    Gris,
    Rgb,
    Cmyk,
    Indexado(Rc<Vec<u32>>),
    /// Una tinta plana: el numero es cuanta tinta, de 0 a 1.
    Tinta,
}

#[derive(Clone, Copy, Debug)]
struct Recorte {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

impl Recorte {
    const TODO: Recorte = Recorte {
        x0: i32::MIN / 2,
        y0: i32::MIN / 2,
        x1: i32::MAX / 2,
        y1: i32::MAX / 2,
    };
    fn corta(self, ax: i32, ay: i32, bx: i32, by: i32) -> Recorte {
        Recorte {
            x0: self.x0.max(ax),
            y0: self.y0.max(ay),
            x1: self.x1.min(bx),
            y1: self.y1.min(by),
        }
    }
}

#[derive(Clone)]
struct Estado {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    tx0: f64,
    ty0: f64,
    grosor: f64,
    color_trazo: u32,
    color_relleno: u32,
    alfa_trazo: f64,
    alfa_relleno: f64,
    raya: Rc<Vec<f64>>,
    recorte: Recorte,
    espacio_trazo: Espacio,
    espacio_relleno: Espacio,
    // El texto: la matriz del renglon y el cursor.
    txa: f64,
    txb: f64,
    txc: f64,
    txd: f64,
    txe: f64,
    txf: f64,
    tx: f64,
    ty: f64,
    fuente: Option<Rc<Fuente>>,
    tam: f64,
    tc: f64,
    tw: f64,
    tz: f64,
    ts: f64,
    tl: f64,
    tr: i32,
}

impl Estado {
    fn nuevo() -> Estado {
        Estado {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx0: 0.0,
            ty0: 0.0,
            grosor: 1.0,
            color_trazo: 0,
            color_relleno: 0,
            alfa_trazo: 1.0,
            alfa_relleno: 1.0,
            raya: Rc::new(Vec::new()),
            recorte: Recorte::TODO,
            espacio_trazo: Espacio::Adivinar,
            espacio_relleno: Espacio::Adivinar,
            txa: 1.0,
            txb: 0.0,
            txc: 0.0,
            txd: 1.0,
            txe: 0.0,
            txf: 0.0,
            tx: 0.0,
            ty: 0.0,
            fuente: None,
            tam: 0.0,
            tc: 0.0,
            tw: 0.0,
            tz: 100.0,
            ts: 0.0,
            tl: 0.0,
            tr: 0,
        }
    }

    fn multiplicar(&mut self, na: f64, nb: f64, nc: f64, nd: f64, ne: f64, nf: f64) {
        let ra = na * self.a + nb * self.c;
        let rb = na * self.b + nb * self.d;
        let rc = nc * self.a + nd * self.c;
        let rd = nc * self.b + nd * self.d;
        let re = ne * self.a + nf * self.c + self.tx0;
        let rf = ne * self.b + nf * self.d + self.ty0;
        (self.a, self.b, self.c, self.d, self.tx0, self.ty0) = (ra, rb, rc, rd, re, rf);
    }

    fn poner_texto(&mut self, m: [f64; 6]) {
        (self.txa, self.txb, self.txc, self.txd, self.txe, self.txf) =
            (m[0], m[1], m[2], m[3], m[4], m[5]);
        self.tx = 0.0;
        self.ty = 0.0;
    }

    /// `Td`: el salto va en el espacio del renglon.
    fn salto_de_linea(&mut self, dx: f64, dy: f64) {
        self.txe += dx * self.txa + dy * self.txc;
        self.txf += dx * self.txb + dy * self.txd;
        self.tx = 0.0;
        self.ty = 0.0;
    }

    fn reiniciar_texto(&mut self) {
        self.poner_texto([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    }
}

/// Una brocha mientras se llena.
struct BrochaViva {
    capa: i32,
    color: u32,
    alfa: f64,
    grosor: f64,
    relleno: bool,
    raya: Rc<Vec<f64>>,
    par_impar: bool,
    ops: Vec<u8>,
    xs: Vec<i32>,
    ys: Vec<i32>,
}

impl BrochaViva {
    fn cerrar(self) -> Brocha {
        Brocha {
            capa: self.capa,
            color: self.color,
            alfa: self.alfa,
            grosor: self.grosor,
            relleno: self.relleno,
            raya: (*self.raya).clone(),
            ops: self.ops,
            xs: self.xs,
            ys: self.ys,
            par_impar: self.par_impar,
        }
    }
}

// ---------------------------------------------------------------------------
// El interprete

/// **Lo que hace un PDF cuando se mira: se ejecuta.** El contenido es un
/// programa de una maquina de pila: `100 200 m` deja la pluma en (100,200),
/// `300 400 l S` traza una raya. Esto lo ejecuta y en vez de encender
/// pixeles apunta la geometria.
// La sena de cada foto (numero, datos, mascara) es de un solo uso aqui.
#[allow(clippy::type_complexity)]
struct Interprete<'a, 'b> {
    archivo: &'a Archivo<'b>,
    caja: Caja,
    brochas: Vec<BrochaViva>,
    brocha_de: HashMap<u64, usize>,
    rellenos: Vec<BrochaViva>,
    relleno_de: HashMap<u64, usize>,
    ultima_clave_de_relleno: Option<u64>,
    textos: Vec<Texto>,
    fotos: Vec<Imagen>,
    peso_de_fotos: usize,
    /// El numero de objeto de cada foto ya vista y su sena: la misma imagen
    /// colocada dos veces se escribe una. Ver [`Imagen::id`].
    senas_de_foto: HashMap<u32, (usize, Arc<Vec<u8>>, Option<(Arc<Vec<u8>>, &'static str)>)>,
    capas: Vec<Capa>,
    capa_por_objeto: HashMap<u32, i32>,
    rayados: Vec<Rc<Vec<f64>>>,
    sin_entender: u32,
    puntos: usize,
    cortado: bool,
    // El camino en construccion, en pasos de 1/FINEZA y con la y hacia abajo.
    ops: Vec<u8>,
    px: Vec<i32>,
    py: Vec<i32>,
    cerro_subcamino: bool,
    min_x: i32,
    min_y: i32,
    max_x: i32,
    max_y: i32,
    ultimo_x: f64,
    ultimo_y: f64,
    recorte_pendiente: bool,
    e: Estado,
    pila: Vec<Estado>,
    /// Las capas metidas unas en otras por `BDC`/`EMC`.
    marcas: Vec<i32>,
    fuentes: HashMap<(usize, Vec<u8>), Rc<Fuente>>,
}

/// Lo que se apunta de un token mientras llega su operador.
#[derive(Clone, Debug)]
enum Dato {
    Num(f64),
    Cad(Vec<u8>),
    Nom,
    /// Una lista dentro de otra cosa: los destinos de un tramo del CMap.
    Lis(Vec<Dato>),
}

impl<'a, 'b> Interprete<'a, 'b> {
    fn nuevo(archivo: &'a Archivo<'b>, caja: Caja) -> Self {
        Interprete {
            archivo,
            caja,
            brochas: Vec::new(),
            brocha_de: HashMap::new(),
            rellenos: Vec::new(),
            relleno_de: HashMap::new(),
            ultima_clave_de_relleno: None,
            textos: Vec::new(),
            fotos: Vec::new(),
            peso_de_fotos: 0,
            senas_de_foto: HashMap::new(),
            capas: Vec::new(),
            capa_por_objeto: HashMap::new(),
            rayados: Vec::new(),
            sin_entender: 0,
            puntos: 0,
            cortado: false,
            ops: Vec::with_capacity(256),
            px: Vec::with_capacity(256),
            py: Vec::with_capacity(256),
            cerro_subcamino: false,
            min_x: i32::MAX,
            min_y: i32::MAX,
            max_x: i32::MIN,
            max_y: i32::MIN,
            ultimo_x: 0.0,
            ultimo_y: 0.0,
            recorte_pendiente: false,
            e: Estado::nuevo(),
            pila: Vec::with_capacity(32),
            marcas: Vec::with_capacity(16),
            fuentes: HashMap::new(),
        }
    }

    /// Lee `/OCProperties`: como se llama cada capa y cuales vienen
    /// apagadas, en el orden de `/Order` (como las ensena Acrobat).
    fn capas_del_documento(&mut self) {
        let a = self.archivo;
        let Some(raiz) = a.raiz() else { return };
        let Some(props) = a.dicc_de(en(&raiz, b"OCProperties")) else {
            return;
        };
        let todas = match a.resolver(en(&props, b"OCGs")) {
            Some(Valor::Lista(l)) => l,
            _ => Vec::new(),
        };
        let pred = a.dicc_de(en(&props, b"D"));
        let apagadas: std::collections::HashSet<u32> =
            match pred.as_ref().and_then(|p| a.resolver(en(p, b"OFF"))) {
                Some(Valor::Lista(l)) => l
                    .iter()
                    .filter_map(|v| match v {
                        Valor::Ref(n, _) => Some(*n),
                        _ => None,
                    })
                    .collect(),
                _ => Default::default(),
            };
        let mut ordenadas: Vec<u32> = Vec::new();
        if let Some(orden) = pred.as_ref().and_then(|p| en(p, b"Order")).cloned() {
            self.recoger_orden(&orden, &mut ordenadas, 0);
        }
        for v in &todas {
            if let Valor::Ref(n, _) = v
                && !ordenadas.contains(n)
            {
                ordenadas.push(*n);
            }
        }
        for n in ordenadas {
            if self.capa_por_objeto.contains_key(&n) {
                continue;
            }
            let Some(d) = a.dicc_de(Some(&Valor::Ref(n, 0))) else {
                continue;
            };
            let nombre = self.nombre_de_capa(&d);
            self.capa_por_objeto.insert(n, self.capas.len() as i32);
            self.capas.push(Capa {
                nombre,
                encendida: !apagadas.contains(&n),
            });
        }
    }

    fn nombre_de_capa(&self, d: &Dicc) -> String {
        match self.archivo.resolver(en(d, b"Name")) {
            Some(Valor::Cadena(c)) => texto_de_cadena(&bytes_de_cadena(&c)),
            Some(Valor::Nombre(n)) => String::from_utf8_lossy(&n).into_owned(),
            _ => format!("Capa {}", self.capas.len() + 1),
        }
    }

    fn recoger_orden(&self, v: &Valor, out: &mut Vec<u32>, fondo: u32) {
        if fondo > 8 {
            return;
        }
        match v {
            Valor::Ref(n, _) => {
                // Una lista de `/Order` tambien puede ir por referencia.
                match self.archivo.objeto(*n) {
                    Some(Valor::Lista(l)) => {
                        for x in &l {
                            self.recoger_orden(x, out, fondo + 1);
                        }
                    }
                    _ => {
                        if !out.contains(n) {
                            out.push(*n);
                        }
                    }
                }
            }
            Valor::Lista(l) => {
                for x in l {
                    self.recoger_orden(x, out, fondo + 1);
                }
            }
            _ => {}
        }
    }

    // ---- Ejecutar ----

    fn ejecutar(&mut self, datos: &[u8], recursos: Option<&Rc<Dicc>>, fondo: u32) {
        let mut f = Fichas { b: datos, pos: 0 };
        let mut nums = [0.0f64; 16];
        let mut n_num = 0usize;
        let mut nombre1: Option<String> = None;
        let mut nombre2: Option<String> = None;
        let mut cadena: Option<Vec<u8>> = None;
        let mut lista: Option<Vec<Dato>> = None;
        loop {
            match f.siguiente() {
                Ficha::Fin => break,
                Ficha::Numero(v) => {
                    if n_num < nums.len() {
                        nums[n_num] = v;
                        n_num += 1;
                    }
                }
                Ficha::Nombre(n) => {
                    nombre2 = nombre1.take();
                    nombre1 = Some(n);
                }
                Ficha::Cadena(c) => cadena = Some(c),
                Ficha::Lista(l) => lista = Some(l),
                Ficha::Operador(op) => {
                    if self.puntos > TOPE_DE_PUNTOS {
                        self.cortado = true;
                        return;
                    }
                    let n = &nums[..n_num];
                    match op {
                        b"q" => self.pila.push(self.e.clone()),
                        b"Q" => {
                            if let Some(e) = self.pila.pop() {
                                self.e = e;
                            }
                        }
                        b"cm" if n.len() >= 6 => {
                            self.e.multiplicar(n[0], n[1], n[2], n[3], n[4], n[5])
                        }
                        b"w" if !n.is_empty() => self.e.grosor = n[0],
                        b"gs" => {
                            if let Some(nom) = nombre1.clone() {
                                self.estado_extra(recursos, &nom);
                            }
                        }
                        b"d" => self.e.raya = Rc::new(self.rayado(lista.as_deref())),
                        // ---- Los colores ----
                        b"G" if !n.is_empty() => self.e.color_trazo = gris(n[0]),
                        b"g" if !n.is_empty() => self.e.color_relleno = gris(n[0]),
                        b"RG" if n.len() >= 3 => self.e.color_trazo = rgb(n[0], n[1], n[2]),
                        b"rg" if n.len() >= 3 => self.e.color_relleno = rgb(n[0], n[1], n[2]),
                        b"K" if n.len() >= 4 => self.e.color_trazo = cmyk(n[0], n[1], n[2], n[3]),
                        b"k" if n.len() >= 4 => self.e.color_relleno = cmyk(n[0], n[1], n[2], n[3]),
                        b"CS" => {
                            self.e.espacio_trazo = self.espacio_de(recursos, nombre1.as_deref());
                            self.e.color_trazo = al_empezar(&self.e.espacio_trazo);
                        }
                        b"cs" => {
                            self.e.espacio_relleno = self.espacio_de(recursos, nombre1.as_deref());
                            self.e.color_relleno = al_empezar(&self.e.espacio_relleno);
                        }
                        b"SC" | b"SCN" => {
                            // Con un nombre delante es un patron: tampoco se sabe pintar.
                            if nombre1.is_some() {
                                self.sin_entender += 1;
                            }
                            self.e.color_trazo =
                                en_este_espacio(&self.e.espacio_trazo, n, self.e.color_trazo);
                        }
                        b"sc" | b"scn" => {
                            if nombre1.is_some() {
                                self.sin_entender += 1;
                            }
                            self.e.color_relleno =
                                en_este_espacio(&self.e.espacio_relleno, n, self.e.color_relleno);
                        }
                        // ---- El camino ----
                        b"m" if n.len() >= 2 => self.mover(n[0], n[1]),
                        b"l" if n.len() >= 2 => self.linea(n[0], n[1]),
                        b"c" if n.len() >= 6 => self.curva(n[0], n[1], n[2], n[3], n[4], n[5]),
                        b"v" if n.len() >= 4 => {
                            self.curva(self.ultimo_x, self.ultimo_y, n[0], n[1], n[2], n[3])
                        }
                        b"y" if n.len() >= 4 => self.curva(n[0], n[1], n[2], n[3], n[2], n[3]),
                        b"h" => self.cerrar(),
                        b"re" if n.len() >= 4 => self.rectangulo(n[0], n[1], n[2], n[3]),
                        // ---- Pintarlo ----
                        b"S" => self.pintar(true, false, false, false),
                        b"s" => self.pintar(true, false, true, false),
                        b"f" | b"F" => self.pintar(false, true, false, false),
                        b"f*" => self.pintar(false, true, false, true),
                        b"B" => self.pintar(true, true, false, false),
                        b"B*" => self.pintar(true, true, false, true),
                        b"b" => self.pintar(true, true, true, false),
                        b"b*" => self.pintar(true, true, true, true),
                        b"n" => self.pintar(false, false, false, false),
                        b"W" | b"W*" => self.recorte_pendiente = true,
                        // ---- Las capas ----
                        b"BDC" => {
                            let c = if nombre2.as_deref() == Some("OC") {
                                self.capa_de(recursos, nombre1.as_deref())
                            } else {
                                self.capa_actual()
                            };
                            self.marcas.push(c);
                        }
                        b"BMC" => self.marcas.push(self.capa_actual()),
                        b"EMC" => {
                            self.marcas.pop();
                        }
                        // ---- Lo de fuera ----
                        b"Do" => self.objeto_externo(recursos, nombre1.as_deref(), fondo),
                        b"BI" => {
                            self.sin_entender += 1;
                            f.saltar_imagen_incrustada();
                        }
                        // Un degradado pinta y aqui no se sabe: la pagina ira como foto.
                        b"sh" => self.sin_entender += 1,
                        // ---- El texto ----
                        b"BT" => self.e.reiniciar_texto(),
                        b"Tf" if !n.is_empty() => {
                            self.e.fuente = self.fuente_de(recursos, nombre1.as_deref());
                            self.e.tam = n[0];
                        }
                        b"Td" if n.len() >= 2 => self.e.salto_de_linea(n[0], n[1]),
                        b"TD" if n.len() >= 2 => {
                            self.e.tl = -n[1];
                            self.e.salto_de_linea(n[0], n[1]);
                        }
                        b"Tm" if n.len() >= 6 => {
                            self.e.poner_texto([n[0], n[1], n[2], n[3], n[4], n[5]])
                        }
                        b"T*" => {
                            let tl = self.e.tl;
                            self.e.salto_de_linea(0.0, -tl);
                        }
                        b"TL" if !n.is_empty() => self.e.tl = n[0],
                        b"Tc" if !n.is_empty() => self.e.tc = n[0],
                        b"Tw" if !n.is_empty() => self.e.tw = n[0],
                        b"Tz" if !n.is_empty() => self.e.tz = n[0],
                        b"Ts" if !n.is_empty() => self.e.ts = n[0],
                        b"Tr" if !n.is_empty() => self.e.tr = n[0] as i32,
                        b"Tj" => {
                            if let Some(c) = cadena.take() {
                                self.escribir(&c);
                            }
                        }
                        b"'" => {
                            let tl = self.e.tl;
                            self.e.salto_de_linea(0.0, -tl);
                            if let Some(c) = cadena.take() {
                                self.escribir(&c);
                            }
                        }
                        b"\"" => {
                            if n.len() >= 2 {
                                self.e.tw = n[0];
                                self.e.tc = n[1];
                            }
                            let tl = self.e.tl;
                            self.e.salto_de_linea(0.0, -tl);
                            if let Some(c) = cadena.take() {
                                self.escribir(&c);
                            }
                        }
                        b"TJ" => {
                            if let Some(l) = lista.take() {
                                self.escribir_lista(&l);
                            }
                        }
                        _ => {}
                    }
                    // Una palabra gasta todos sus argumentos, la entienda esto o
                    // no: dejar numeros sueltos haria que el siguiente operador
                    // leyera los del anterior.
                    n_num = 0;
                    nombre1 = None;
                    nombre2 = None;
                    cadena = None;
                    lista = None;
                }
            }
        }
    }

    // ---- El camino ----

    /// Del espacio del PDF al del dibujo, una sola vez: la y del PDF sube y la
    /// del dibujo baja, y la pagina puede venir girada.
    fn sitio(&self, x: f64, y: f64) -> (i32, i32) {
        let e = &self.e;
        let px = e.a * x + e.c * y + e.tx0;
        let py = e.b * x + e.d * y + e.ty0;
        let ux = self.caja.en_x(px, py) * FINEZA as f64;
        let uy = self.caja.en_y(px, py) * FINEZA as f64;
        let q = |v: f64| {
            if v.is_finite() {
                v.round().clamp(i32::MIN as f64, i32::MAX as f64) as i32
            } else {
                0
            }
        };
        (q(ux), q(uy))
    }

    fn apuntar_punto(&mut self, (x, y): (i32, i32)) {
        self.px.push(x);
        self.py.push(y);
        self.min_x = self.min_x.min(x);
        self.max_x = self.max_x.max(x);
        self.min_y = self.min_y.min(y);
        self.max_y = self.max_y.max(y);
    }

    fn mover(&mut self, x: f64, y: f64) {
        let p = self.sitio(x, y);
        self.ops.push(MOVER);
        self.apuntar_punto(p);
        (self.ultimo_x, self.ultimo_y) = (x, y);
    }

    fn linea(&mut self, x: f64, y: f64) {
        if self.ops.is_empty() {
            return self.mover(x, y);
        }
        self.ops.push(LINEA);
        let p = self.sitio(x, y);
        self.apuntar_punto(p);
        (self.ultimo_x, self.ultimo_y) = (x, y);
    }

    fn curva(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64) {
        if self.ops.is_empty() {
            self.mover(x1, y1);
        }
        self.ops.push(CURVA);
        for (x, y) in [(x1, y1), (x2, y2), (x3, y3)] {
            let p = self.sitio(x, y);
            self.apuntar_punto(p);
        }
        (self.ultimo_x, self.ultimo_y) = (x3, y3);
    }

    fn cerrar(&mut self) {
        if self.ops.is_empty() {
            return;
        }
        self.ops.push(CERRAR);
        self.cerro_subcamino = true;
    }

    fn rectangulo(&mut self, x: f64, y: f64, an: f64, al: f64) {
        self.mover(x, y);
        self.linea(x + an, y);
        self.linea(x + an, y + al);
        self.linea(x, y + al);
        self.cerrar();
    }

    /// **Pinta el camino y lo tira.** Tras un `W` el mismo camino sirve de
    /// recorte, que se guarda solo como **caja**: basta para tirar lo que
    /// queda fuera de un cuadro de dibujo, que es para lo que lo usan los
    /// planos. Un recorte con forma deja pasar de mas; pasar de mas se ve, y
    /// pasar de menos borra dibujo.
    fn pintar(&mut self, trazo: bool, relleno: bool, cerrando: bool, par_impar: bool) {
        if !self.ops.is_empty() && (trazo || relleno) {
            if cerrando && !self.cerro_subcamino {
                self.ops.push(CERRAR);
            }
            if self.dentro_del_recorte() && self.dentro_del_papel() {
                if relleno {
                    let i = self.relleno_en_orden(par_impar);
                    meter(&mut self.rellenos[i], &self.ops, &self.px, &self.py);
                }
                if trazo {
                    let i = self.brocha_viva();
                    meter(&mut self.brochas[i], &self.ops, &self.px, &self.py);
                }
                self.puntos += self.px.len();
            }
        }
        if self.recorte_pendiente && !self.px.is_empty() {
            self.e.recorte = self
                .e
                .recorte
                .corta(self.min_x, self.min_y, self.max_x, self.max_y);
        }
        self.recorte_pendiente = false;
        self.ops.clear();
        self.px.clear();
        self.py.clear();
        self.cerro_subcamino = false;
        self.min_x = i32::MAX;
        self.min_y = i32::MAX;
        self.max_x = i32::MIN;
        self.max_y = i32::MIN;
    }

    fn dentro_del_recorte(&self) -> bool {
        let r = self.e.recorte;
        !(self.max_x < r.x0 || self.min_x > r.x1 || self.max_y < r.y0 || self.min_y > r.y1)
    }

    /// Lo que cae del todo fuera del papel no se manda: no se ve y si pesa.
    fn dentro_del_papel(&self) -> bool {
        let margen = FINEZA * 8;
        let ancho = (self.caja.ancho() * FINEZA as f64) as i32;
        let alto = (self.caja.alto() * FINEZA as f64) as i32;
        !(self.max_x < -margen
            || self.min_x > ancho + margen
            || self.max_y < -margen
            || self.min_y > alto + margen)
    }

    fn capa_actual(&self) -> i32 {
        self.marcas.last().copied().unwrap_or(-1)
    }

    /// La clave de una brocha y lo que la describe, con el redondeo de
    /// [`TOPE_DE_BROCHAS`] puesto si ya se paso.
    fn clave(&mut self, relleno: bool, par_impar: bool) -> (u64, u32, f64, f64, i32, i32) {
        let e = &self.e;
        let mut color = if relleno {
            e.color_relleno
        } else {
            e.color_trazo
        };
        let alfa = if relleno {
            e.alfa_relleno
        } else {
            e.alfa_trazo
        };
        let grosor = if relleno { 0.0 } else { self.grosor_en_papel() };
        let capa = self.capa_actual();
        let raya = if relleno || self.e.raya.is_empty() {
            -1
        } else {
            let r = self.e.raya.clone();
            self.indice_de_rayado(&r)
        };
        let mut gq = ((grosor * 16.0).round() as i64).clamp(0, 4095);
        let aq = ((alfa * 15.0).round() as i64).clamp(0, 15);
        if self.brochas.len() + self.relleno_de.len() >= TOPE_DE_BROCHAS {
            color = (color & 0xF0F0F0) | 0x080808;
            gq = (gq / 8) * 8;
        }
        let clave = (((capa + 1) as u64) << 54)
            | ((relleno as u64) << 53)
            | ((((raya + 1) as u64) & 0xF) << 49)
            | ((aq as u64) << 45)
            | ((gq as u64) << 33)
            | ((par_impar as u64) << 24)
            | (color as u64 & 0xFFFFFF);
        (clave, color, aq as f64 / 15.0, gq as f64 / 16.0, capa, raya)
    }

    fn brocha_viva(&mut self) -> usize {
        let (clave, color, alfa, grosor, capa, raya) = self.clave(false, false);
        if let Some(&i) = self.brocha_de.get(&clave) {
            return i;
        }
        let raya = if raya < 0 {
            Rc::new(Vec::new())
        } else {
            self.rayados[raya as usize].clone()
        };
        self.brochas.push(BrochaViva {
            capa,
            color,
            alfa,
            grosor,
            relleno: false,
            raya,
            par_impar: false,
            ops: Vec::new(),
            xs: Vec::new(),
            ys: Vec::new(),
        });
        self.brocha_de.insert(clave, self.brochas.len() - 1);
        self.brochas.len() - 1
    }

    /// **Los rellenos, en el orden en que se pintan.** Una caja blanca que
    /// tapa parte de una formula tiene que ir **despues** de lo que tapa:
    /// juntandolos por color salian manchas negras y trozos tapados (usuario,
    /// 13-sep-2026). Se junta solo lo **seguido** de la misma brocha; pasado
    /// [`TOPE_DE_RELLENOS_EN_ORDEN`] se vuelve a juntar por color.
    fn relleno_en_orden(&mut self, par_impar: bool) -> usize {
        let (clave, color, alfa, grosor, capa, _) = self.clave(true, par_impar);
        if !self.rellenos.is_empty() && self.ultima_clave_de_relleno == Some(clave) {
            return self.rellenos.len() - 1;
        }
        if self.rellenos.len() >= TOPE_DE_RELLENOS_EN_ORDEN
            && let Some(&i) = self.relleno_de.get(&clave)
        {
            self.ultima_clave_de_relleno = Some(clave);
            return i;
        }
        self.rellenos.push(BrochaViva {
            capa,
            color,
            alfa,
            grosor,
            relleno: true,
            raya: Rc::new(Vec::new()),
            par_impar,
            ops: Vec::new(),
            xs: Vec::new(),
            ys: Vec::new(),
        });
        let i = self.rellenos.len() - 1;
        self.relleno_de.entry(clave).or_insert(i);
        self.ultima_clave_de_relleno = Some(clave);
        i
    }

    /// El grosor como se vera en el papel: el `w` va en las unidades de la
    /// matriz del momento, y un plano empieza con un `cm` que lo achica doce
    /// veces.
    fn grosor_en_papel(&self) -> f64 {
        let e = &self.e;
        let det = (e.a * e.d - e.b * e.c).abs();
        let k = if det > 0.0 {
            det.sqrt()
        } else {
            e.a.hypot(e.b)
        };
        e.grosor * k
    }

    fn indice_de_rayado(&mut self, r: &Rc<Vec<f64>>) -> i32 {
        if let Some(i) = self.rayados.iter().position(|x| **x == **r) {
            return i as i32;
        }
        if self.rayados.len() >= 14 {
            return -1;
        }
        self.rayados.push(r.clone());
        self.rayados.len() as i32 - 1
    }

    fn rayado(&self, lista: Option<&[Dato]>) -> Vec<f64> {
        let Some(l) = lista else { return Vec::new() };
        let v: Vec<f64> = l
            .iter()
            .filter_map(|d| match d {
                Dato::Num(x) if x.is_finite() && *x >= 0.0 => Some(*x),
                _ => None,
            })
            .collect();
        if v.is_empty() || v.iter().all(|x| *x == 0.0) {
            return Vec::new();
        }
        let e = &self.e;
        let det = (e.a * e.d - e.b * e.c).abs();
        let k = if det > 0.0 { det.sqrt() } else { 1.0 };
        v.iter().take(6).map(|x| x * k).collect()
    }

    // ---- Recursos ----

    fn recurso(
        &self,
        recursos: Option<&Rc<Dicc>>,
        grupo: &[u8],
        nombre: Option<&str>,
    ) -> Option<(Option<u32>, Valor)> {
        let nombre = nombre?;
        let g = self.archivo.dicc_de(en(recursos?, grupo))?;
        let v = en(&g, nombre.as_bytes())?;
        let num = match v {
            Valor::Ref(n, _) => Some(*n),
            _ => None,
        };
        Some((num, self.archivo.resolver(Some(v))?))
    }

    fn capa_de(&mut self, recursos: Option<&Rc<Dicc>>, nombre: Option<&str>) -> i32 {
        let a = self.archivo;
        let Some(nombre) = nombre else {
            return self.capa_actual();
        };
        let Some(g) = recursos.and_then(|r| a.dicc_de(en(r, b"Properties"))) else {
            return self.capa_actual();
        };
        let Some(Valor::Ref(n, _)) = en(&g, nombre.as_bytes()) else {
            return self.capa_actual();
        };
        let n = *n;
        if let Some(&c) = self.capa_por_objeto.get(&n) {
            return c;
        }
        // Una capa que no estaba en el catalogo: se apunta ahora. `/OCMD`
        // es un envoltorio: la capa de verdad cuelga de su `/OCGs`.
        let d = a.dicc_de(Some(&Valor::Ref(n, 0)));
        let real = match d.as_ref().and_then(|d| en(d, b"OCGs")) {
            Some(Valor::Ref(r, _)) => *r,
            _ => n,
        };
        if let Some(&c) = self.capa_por_objeto.get(&real) {
            self.capa_por_objeto.insert(n, c);
            return c;
        }
        let nombre_capa = a
            .dicc_de(Some(&Valor::Ref(real, 0)))
            .map(|d| self.nombre_de_capa(&d))
            .unwrap_or_else(|| format!("Capa {}", self.capas.len() + 1));
        let i = self.capas.len() as i32;
        self.capas.push(Capa {
            nombre: nombre_capa,
            encendida: true,
        });
        self.capa_por_objeto.insert(real, i);
        self.capa_por_objeto.insert(n, i);
        i
    }

    fn estado_extra(&mut self, recursos: Option<&Rc<Dicc>>, nombre: &str) {
        let Some((_, Valor::Dicc(d))) = self.recurso(recursos, b"ExtGState", Some(nombre)) else {
            return;
        };
        let a = self.archivo;
        if let Some(v) = num(a, en(&d, b"LW")) {
            self.e.grosor = v;
        }
        if let Some(v) = num(a, en(&d, b"CA")) {
            self.e.alfa_trazo = v.clamp(0.0, 1.0);
        }
        if let Some(v) = num(a, en(&d, b"ca")) {
            self.e.alfa_relleno = v.clamp(0.0, 1.0);
        }
        if let Some(Valor::Lista(l)) = a.resolver(en(&d, b"D")) {
            let datos: Vec<Dato> = match l.first() {
                Some(Valor::Lista(x)) => x.iter().filter_map(numero_de).map(Dato::Num).collect(),
                _ => Vec::new(),
            };
            self.e.raya = Rc::new(self.rayado(Some(&datos)));
        }
    }

    /// Un `Do`: un formulario —se ejecuta aqui dentro con su matriz y sus
    /// recursos— o una imagen, que se pasa tal cual si se puede. Lo que no se
    /// sabe seguir **se cuenta**: una lamina a la que le faltan las fotos sin
    /// decir nada es justo el fallo que no se puede tener.
    fn objeto_externo(&mut self, recursos: Option<&Rc<Dicc>>, nombre: Option<&str>, fondo: u32) {
        if fondo >= FONDO_MAXIMO {
            self.sin_entender += 1;
            return;
        }
        let Some((numero, Valor::Flujo(d, crudo))) = self.recurso(recursos, b"XObject", nombre)
        else {
            self.sin_entender += 1;
            return;
        };
        let subtipo = match en(&d, b"Subtype") {
            Some(Valor::Nombre(n)) => n.clone(),
            _ => Vec::new(),
        };
        match subtipo.as_slice() {
            b"Image" => {
                if !self.pasar_la_foto(numero, &d, &crudo) {
                    self.sin_entender += 1;
                }
            }
            b"Form" => {
                let Some(datos) = descomprimir(self.archivo, &d, &crudo) else {
                    self.sin_entender += 1;
                    return;
                };
                let guardado = self.e.clone();
                let marcas_antes = self.marcas.len();
                let pila_antes = self.pila.len();
                // Un formulario puede llevar su capa puesta desde fuera.
                if let Some(Valor::Ref(r, _)) = en(&d, b"OC")
                    && let Some(&c) = self.capa_por_objeto.get(r)
                {
                    self.marcas.push(c);
                }
                if let Some(Valor::Lista(m)) = self.archivo.resolver(en(&d, b"Matrix")) {
                    let m: Vec<f64> = m.iter().filter_map(numero_de).collect();
                    if m.len() >= 6 {
                        self.e.multiplicar(m[0], m[1], m[2], m[3], m[4], m[5]);
                    }
                }
                let suyos = match self.archivo.dicc_de(en(&d, b"Resources")) {
                    Some(r) => Some(Rc::new(r)),
                    None => recursos.cloned(),
                };
                self.ejecutar(&datos, suyos.as_ref(), fondo + 1);
                self.pila.truncate(pila_antes);
                self.marcas.truncate(marcas_antes);
                self.e = guardado;
            }
            _ => self.sin_entender += 1,
        }
    }

    /// **Pasa la imagen tal cual si se puede**: solo las que ya son un JPEG
    /// por dentro, con su mascara JPEG del mismo tamano si la traen (asi
    /// pinta Revit sus zonas de color). Cualquier otra cosa habria que
    /// dibujarla, que es rasterizar, justo de lo que se huye aqui.
    fn pasar_la_foto(&mut self, numero: Option<u32>, d: &Dicc, crudo: &[u8]) -> bool {
        let a = self.archivo;
        if en(d, b"Mask").is_some() {
            return false;
        }
        if matches!(en(d, b"ImageMask"), Some(Valor::Booleano(true))) {
            return false;
        }
        if self.peso_de_fotos > PESO_DE_LAS_FOTOS {
            return false;
        }
        let ya = numero.and_then(|n| self.senas_de_foto.get(&n).cloned());
        let (sena, datos, mascara) = match ya {
            Some(v) => v,
            None => {
                let Some((filtro, datos)) = sin_el_ultimo_filtro(a, d, crudo) else {
                    return false;
                };
                let es_jpeg = filtro == b"DCTDecode" || filtro == b"DCT";
                // Lo que no es JPEG, como PNG sin perder nada si se deja (`png`).
                let (datos, con_su_mascara) = if es_jpeg {
                    (datos, false)
                } else {
                    match self.como_png(d, &filtro, &datos) {
                        Some(p) => (p, true),
                        None => return false,
                    }
                };
                let mut mascara = None;
                if let Some(v) = en(d, b"SMask").filter(|_| !con_su_mascara) {
                    let Some(Valor::Flujo(md, mcrudo)) = a.resolver(Some(v)) else {
                        return false;
                    };
                    if en(&md, b"SMask").is_some() || en(&md, b"Mask").is_some() {
                        return false;
                    }
                    if !self.mismo_tamano(d, &md) {
                        return false;
                    }
                    let Some((fm, dm)) = sin_el_ultimo_filtro(a, &md, &mcrudo) else {
                        return false;
                    };
                    // La mascara de un JPEG de Word suele ir en Flate: va
                    // como PNG en gris, que el visor lee igual.
                    let (dm, tipo) = if fm == b"DCTDecode" || fm == b"DCT" {
                        (dm, "image/jpeg")
                    } else {
                        match self.como_png(&md, &fm, &dm) {
                            Some(p) => (p, "image/png"),
                            None => return false,
                        }
                    };
                    self.peso_de_fotos += dm.len();
                    mascara = Some((Arc::new(dm), tipo));
                }
                self.peso_de_fotos += datos.len();
                let sena = self.senas_de_foto.len();
                let datos = Arc::new(datos);
                let v = (sena, datos, mascara);
                // Sin numero de objeto (una imagen escrita dentro de su
                // diccionario de recursos) no se puede reconocer: sale repetida,
                // que pesa mas pero pinta lo mismo.
                self.senas_de_foto.insert(
                    numero.unwrap_or(u32::MAX - self.senas_de_foto.len() as u32),
                    v.clone(),
                );
                v
            }
        };
        let e = &self.e;
        // El cuadrado de la imagen va del (0,0) al (1,1) y su primera fila
        // cae arriba, en la y = 1: el vector de bajar es el de la y al reves.
        let (ux, uy, vx, vy) = (e.a, e.b, -e.c, -e.d);
        let (ox, oy) = (e.c + e.tx0, e.d + e.ty0);
        let c = self.caja;
        self.fotos.push(Imagen {
            capa: self.capa_actual(),
            alfa: e.alfa_relleno,
            tipo: if datos.starts_with(b"\x89PNG") {
                "image/png"
            } else {
                "image/jpeg"
            },
            datos,
            a: c.vector_x(ux, uy),
            b: c.vector_y(ux, uy),
            c: c.vector_x(vx, vy),
            d: c.vector_y(vx, vy),
            x: c.en_x(ox, oy),
            y: c.en_y(ox, oy),
            tipo_mascara: mascara.as_ref().map(|m| m.1),
            mascara: mascara.map(|m| m.0),
            id: sena,
        });
        true
    }

    /// Si la mascara mide lo mismo que su imagen: juntarlas pixel a pixel lo
    /// exige, y estirar aqui seria rasterizar.
    /// **Una imagen en Flate, como PNG** (ver `crate::png`): 8 bits por
    /// canal, en gris, color, CMYK (que se pasa a color) o paleta, con su
    /// transparencia (`/SMask` en Flate y del mismo tamano) ya dentro.
    /// `filtro` es su ultimo filtro y `datos` lo que queda sin deshacer de
    /// el. `None` con cualquier otra cosa: la hoja ira como foto.
    fn como_png(&self, d: &Dicc, filtro: &[u8], datos: &[u8]) -> Option<Vec<u8>> {
        /// Tope de pixeles de una imagen que se pasa: una de 16 Mpx ya pesa
        /// mas en la pagina que la foto de la hoja entera.
        const TOPE_DE_PIXELES: usize = 16_000_000;
        let a = self.archivo;
        if filtro != b"FlateDecode" && filtro != b"Fl" || en(d, b"Decode").is_some() {
            return None;
        }
        let lado = |d: &Dicc, k: &[u8]| num(a, en(d, k)).map(|v| v as i64).filter(|v| *v > 0);
        let (ancho, alto) = (lado(d, b"Width")? as usize, lado(d, b"Height")? as usize);
        let bits = lado(d, b"BitsPerComponent")? as usize;
        if ancho * alto > TOPE_DE_PIXELES || !matches!(bits, 1 | 2 | 4 | 8) {
            return None;
        }
        // Sin espacio de color solo puede ser una mascara, que es gris.
        let espacio = match a.resolver(en(d, b"ColorSpace")) {
            None => Espacio::Gris,
            Some(Valor::Nombre(n)) => por_nombre(&String::from_utf8_lossy(&n))?,
            Some(v @ Valor::Lista(_)) => self.de_la_lista(&v, 0),
            _ => return None,
        };
        let canales = match espacio {
            Espacio::Gris | Espacio::Indexado(_) => 1,
            Espacio::Rgb => 3,
            Espacio::Cmyk => 4,
            _ => return None,
        };
        // Menos de 8 bits (un escaneo en blanco y negro, la silueta de una
        // figura de Word) solo en gris o con paleta, que es lo que la norma deja.
        if bits < 8 && canales != 1 {
            return None;
        }
        let pixeles = |d: &Dicc, datos: &[u8], canales: usize, bits: usize| -> Option<Vec<u8>> {
            let crudo = crate::png::inflar(datos)?;
            let n = filtros_de(a, d)?.len().saturating_sub(1);
            let predictor = parametros_de(a, d, n)
                .and_then(|p| entero(en(&p, b"Predictor")))
                .unwrap_or(1);
            let fila = (ancho * canales * bits).div_ceil(8);
            let mut px = if predictor >= 10 {
                crate::png::desfiltrar(&crudo, fila, (canales * bits / 8).max(1))?
            } else if predictor <= 1 {
                crudo
            } else {
                return None;
            };
            if px.len() < fila * alto {
                return None;
            }
            px.truncate(fila * alto);
            Some(if bits < 8 {
                crate::png::a_ocho_bits(&px, ancho, bits, matches!(espacio, Espacio::Indexado(_)))
            } else {
                px
            })
        };
        let mut color = pixeles(d, datos, canales, bits)?;
        let mut canales = canales;
        let mut paleta: Option<Vec<u8>> = None;
        match &espacio {
            Espacio::Cmyk => {
                color = color
                    .chunks_exact(4)
                    .flat_map(|p| {
                        let c = |i: usize| p[i] as f64 / 255.0;
                        let v = cmyk(c(0), c(1), c(2), c(3));
                        [(v >> 16) as u8, (v >> 8) as u8, v as u8]
                    })
                    .collect();
                canales = 3;
            }
            Espacio::Indexado(p) => {
                paleta = Some(
                    p.iter()
                        .flat_map(|v| [(*v >> 16) as u8, (*v >> 8) as u8, *v as u8])
                        .collect(),
                );
            }
            _ => {}
        }
        if let Some(m) = en(d, b"SMask") {
            let Some(Valor::Flujo(md, mcrudo)) = a.resolver(Some(m)) else {
                return None;
            };
            if lado(&md, b"Width") != Some(ancho as i64)
                || lado(&md, b"Height") != Some(alto as i64)
            {
                return None;
            }
            if lado(&md, b"BitsPerComponent") != Some(8) || en(&md, b"Decode").is_some() {
                return None;
            }
            let (fm, dm) = sin_el_ultimo_filtro(a, &md, &mcrudo)?;
            if fm != b"FlateDecode" && fm != b"Fl" {
                return None;
            }
            let alfa = pixeles(&md, &dm, 1, 8)?;
            if let Some(p) = paleta.take() {
                color = crate::png::sin_paleta(&color, &p);
                canales = 3;
            }
            color = crate::png::con_alfa(&color, canales, &alfa)?;
            canales += 1;
        }
        crate::png::escribir(
            ancho as u32,
            alto as u32,
            canales as u8,
            &color,
            paleta.as_deref(),
        )
    }

    fn mismo_tamano(&self, imagen: &Dicc, mascara: &Dicc) -> bool {
        let a = self.archivo;
        let lado = |d: &Dicc, k: &[u8]| num(a, en(d, k)).map(|v| v as i64);
        match (lado(imagen, b"Width"), lado(imagen, b"Height")) {
            (Some(w), Some(h)) => {
                lado(mascara, b"Width") == Some(w) && lado(mascara, b"Height") == Some(h)
            }
            _ => false,
        }
    }

    // ---- El texto ----

    fn fuente_de(
        &mut self,
        recursos: Option<&Rc<Dicc>>,
        nombre: Option<&str>,
    ) -> Option<Rc<Fuente>> {
        let nombre = nombre?;
        // Por recursos y nombre: dos formularios pueden llamar /F1 a fuentes
        // distintas.
        let clave = (
            recursos.map_or(0, |r| Rc::as_ptr(r) as usize),
            nombre.as_bytes().to_vec(),
        );
        if let Some(f) = self.fuentes.get(&clave) {
            return Some(f.clone());
        }
        let (_, Valor::Dicc(d)) = self.recurso(recursos, b"Font", Some(nombre))? else {
            return None;
        };
        let f = Rc::new(Fuente::leer(self.archivo, &d));
        self.fuentes.insert(clave, f.clone());
        Some(f)
    }

    fn escribir_lista(&mut self, lista: &[Dato]) {
        for x in lista {
            match x {
                Dato::Cad(c) => self.escribir(c),
                // Un numero dentro de `TJ` es un empujon hacia atras, en
                // milesimas de eme.
                Dato::Num(v) => self.e.tx -= v / 1000.0 * self.e.tam * (self.e.tz / 100.0),
                Dato::Nom | Dato::Lis(_) => {}
            }
        }
    }

    /// Escribe una cadena: apunta el texto donde toca y adelanta el cursor.
    fn escribir(&mut self, bytes: &[u8]) {
        let Some(f) = self.e.fuente.clone() else {
            return;
        };
        let tam = self.e.tam;
        if tam == 0.0 {
            return;
        }
        let e = &self.e;
        let th = e.tz / 100.0;
        let mut avance = 0.0;
        let mut texto = String::new();
        for l in f.letras(bytes) {
            avance += (l.ancho / 1000.0) * tam + e.tc + if l.es_espacio { e.tw } else { 0.0 };
            if let Some(t) = l.texto {
                texto.push_str(&t);
            }
        }
        // Modo 3 es texto invisible: el del escaneo con OCR debajo de su foto.
        let visible = e.tr != 3 && e.tr != 7 && !texto.trim().is_empty();
        if visible {
            let (m00, m01, m10, m11) = (e.txa * th, e.txb * th, e.txc, e.txd);
            let (ox, oy) = (e.txe, e.txf + e.ts);
            let ax = m00 * e.a + m01 * e.c;
            let ay = m00 * e.b + m01 * e.d;
            let bx = m10 * e.a + m11 * e.c;
            let by = m10 * e.b + m11 * e.d;
            let cx = e.tx * e.txa + e.ty * e.txc;
            let cy = e.tx * e.txb + e.ty * e.txd;
            let ex = ox * e.a + oy * e.c + e.tx0 + cx * e.a + cy * e.c;
            let ey = ox * e.b + oy * e.d + e.ty0 + cx * e.b + cy * e.d;
            let c = self.caja;
            self.textos.push(Texto {
                capa: self.capa_actual(),
                color: if e.tr == 1 || e.tr == 5 {
                    e.color_trazo
                } else {
                    e.color_relleno
                },
                alfa: e.alfa_relleno,
                texto,
                a: c.vector_x(ax, ay) * tam,
                b: c.vector_y(ax, ay) * tam,
                c: -c.vector_x(bx, by) * tam,
                d: -c.vector_y(bx, by) * tam,
                x: c.en_x(ex, ey),
                y: c.en_y(ex, ey),
                ancho: avance / tam,
                familia: f.familia,
                negrita: f.negrita,
                cursiva: f.cursiva,
            });
        }
        // El cursor avanza en el espacio del texto, donde el apretado cuenta.
        self.e.tx += avance * th;
    }

    // ---- Colores ----

    fn espacio_de(&self, recursos: Option<&Rc<Dicc>>, nombre: Option<&str>) -> Espacio {
        let Some(nombre) = nombre else {
            return Espacio::Adivinar;
        };
        if let Some(e) = por_nombre(nombre) {
            return e;
        }
        match self.recurso(recursos, b"ColorSpace", Some(nombre)) {
            Some((_, v)) => self.de_la_lista(&v, 0),
            None => Espacio::Adivinar,
        }
    }

    /// El espacio escrito como lista: `[/Indexed base hival tabla]`,
    /// `[/ICCBased ref]`...
    fn de_la_lista(&self, v: &Valor, fondo: u32) -> Espacio {
        let a = self.archivo;
        if fondo > 4 {
            return Espacio::Adivinar;
        }
        let l = match v {
            Valor::Nombre(n) => {
                return por_nombre(&String::from_utf8_lossy(n)).unwrap_or(Espacio::Adivinar);
            }
            Valor::Lista(l) => l,
            _ => return Espacio::Adivinar,
        };
        let clase = match l.first() {
            Some(Valor::Nombre(n)) => n.as_slice(),
            _ => return Espacio::Adivinar,
        };
        match clase {
            b"ICCBased" => match a
                .dicc_de(l.get(1))
                .and_then(|d| entero(a.resolver(en(&d, b"N")).as_ref()))
            {
                Some(1) => Espacio::Gris,
                Some(3) => Espacio::Rgb,
                Some(4) => Espacio::Cmyk,
                _ => Espacio::Adivinar,
            },
            b"CalGray" | b"DeviceGray" => Espacio::Gris,
            b"CalRGB" | b"Lab" | b"DeviceRGB" => Espacio::Rgb,
            b"DeviceCMYK" => Espacio::Cmyk,
            b"Indexed" | b"I" => self.paleta_de(l, fondo),
            // El numero es cuanta tinta se echa; la tabla que la vuelve color
            // es una funcion que aqui no se ejecuta.
            b"Separation" | b"DeviceN" => Espacio::Tinta,
            _ => Espacio::Adivinar,
        }
    }

    fn paleta_de(&self, l: &[Valor], fondo: u32) -> Espacio {
        let a = self.archivo;
        let Some(base) = a
            .resolver(l.get(1))
            .map(|b| self.de_la_lista(&b, fondo + 1))
        else {
            return Espacio::Adivinar;
        };
        let (comp, tinta) = match base {
            Espacio::Gris => (1, false),
            Espacio::Tinta => (1, true),
            Espacio::Rgb => (3, false),
            Espacio::Cmyk => (4, false),
            _ => return Espacio::Adivinar,
        };
        let Some(tope) = num(a, l.get(2)).map(|v| v as i64) else {
            return Espacio::Adivinar;
        };
        if !(0..=255).contains(&tope) {
            return Espacio::Adivinar;
        }
        let tabla = match a.resolver(l.get(3)) {
            Some(Valor::Cadena(c)) => bytes_de_cadena(&c),
            Some(Valor::Flujo(d, datos)) => match descomprimir(a, &d, &datos) {
                Some(t) => t,
                None => return Espacio::Adivinar,
            },
            _ => return Espacio::Adivinar,
        };
        let mut paleta = vec![0u32; tope as usize + 1];
        for (i, p) in paleta.iter_mut().enumerate() {
            let o = i * comp;
            if o + comp > tabla.len() {
                break;
            }
            let c = |k: usize| tabla[o + k] as f64 / 255.0;
            *p = match comp {
                1 if tinta => gris(1.0 - c(0)),
                1 => gris(c(0)),
                3 => rgb(c(0), c(1), c(2)),
                _ => cmyk(c(0), c(1), c(2), c(3)),
            };
        }
        Espacio::Indexado(Rc::new(paleta))
    }

    fn cosecha(self) -> Plano {
        // Los rellenos primero y en su orden; las rayas, que van encima, despues.
        let mut brochas: Vec<Brocha> = Vec::with_capacity(self.rellenos.len() + self.brochas.len());
        brochas.extend(self.rellenos.into_iter().map(BrochaViva::cerrar));
        brochas.extend(self.brochas.into_iter().map(BrochaViva::cerrar));
        Plano {
            ancho: self.caja.ancho(),
            alto: self.caja.alto(),
            capas: self.capas,
            brochas,
            textos: juntar_renglones(self.textos),
            fotos: self.fotos,
            sin_entender: self.sin_entender,
            cortado: self.cortado,
        }
    }
}

fn meter(b: &mut BrochaViva, ops: &[u8], xs: &[i32], ys: &[i32]) {
    b.ops.extend_from_slice(ops);
    b.xs.extend_from_slice(xs);
    b.ys.extend_from_slice(ys);
}

fn a_byte(v: f64) -> u32 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u32
}

fn gris(v: f64) -> u32 {
    let g = a_byte(v);
    (g << 16) | (g << 8) | g
}

fn rgb(r: f64, g: f64, b: f64) -> u32 {
    (a_byte(r) << 16) | (a_byte(g) << 8) | a_byte(b)
}

fn cmyk(c: f64, m: f64, y: f64, k: f64) -> u32 {
    rgb(
        (1.0 - c) * (1.0 - k),
        (1.0 - m) * (1.0 - k),
        (1.0 - y) * (1.0 - k),
    )
}

fn por_nombre(nombre: &str) -> Option<Espacio> {
    match nombre {
        "DeviceGray" | "CalGray" | "G" => Some(Espacio::Gris),
        "DeviceRGB" | "CalRGB" | "RGB" => Some(Espacio::Rgb),
        "DeviceCMYK" | "CMYK" => Some(Espacio::Cmyk),
        // Un patron no se sabe pintar y ya se cuenta en `scn`.
        "Pattern" => Some(Espacio::Adivinar),
        _ => None,
    }
}

/// El color con que se estrena un espacio: negro, o el primero de la paleta.
fn al_empezar(e: &Espacio) -> u32 {
    match e {
        Espacio::Indexado(p) => p.first().copied().unwrap_or(0),
        _ => 0,
    }
}

/// Los numeros de `sc`/`scn` leidos en el espacio que toque.
fn en_este_espacio(e: &Espacio, n: &[f64], antes: u32) -> u32 {
    if n.is_empty() {
        return antes;
    }
    match e {
        Espacio::Gris => gris(n[0]),
        Espacio::Rgb if n.len() >= 3 => rgb(n[0], n[1], n[2]),
        Espacio::Cmyk if n.len() >= 4 => cmyk(n[0], n[1], n[2], n[3]),
        Espacio::Rgb | Espacio::Cmyk => antes,
        Espacio::Tinta => gris(1.0 - n[0].clamp(0.0, 1.0)),
        Espacio::Indexado(p) => {
            if p.is_empty() {
                antes
            } else {
                p[(n[0] as i64).clamp(0, p.len() as i64 - 1) as usize]
            }
        }
        // Sin saber el espacio se adivina por cuantos numeros trae.
        Espacio::Adivinar => match n.len() {
            1 => gris(1.0 - n[0].clamp(0.0, 1.0)),
            3 => rgb(n[0], n[1], n[2]),
            4 => cmyk(n[0], n[1], n[2], n[3]),
            _ => antes,
        },
    }
}

// ---------------------------------------------------------------------------
// Las fichas del contenido

enum Ficha<'x> {
    Fin,
    Numero(f64),
    Nombre(String),
    Cadena(Vec<u8>),
    Lista(Vec<Dato>),
    Operador(&'x [u8]),
}

/// **El troceador del programa de la pagina.** Va en bucle y no llamandose a
/// si mismo: un archivo estropeado con cien mil corchetes sueltos desbordaria
/// la pila, y eso es cerrar la aplicacion de alguien.
struct Fichas<'x> {
    b: &'x [u8],
    pos: usize,
}

fn es_corte(c: u8) -> bool {
    matches!(
        c,
        0 | 9
            | 10
            | 12
            | 13
            | 32
            | b'('
            | b')'
            | b'<'
            | b'>'
            | b'['
            | b']'
            | b'{'
            | b'}'
            | b'/'
            | b'%'
    )
}

impl<'x> Fichas<'x> {
    fn siguiente(&mut self) -> Ficha<'x> {
        loop {
            self.saltar();
            if self.pos >= self.b.len() {
                return Ficha::Fin;
            }
            let c = self.b[self.pos];
            match c {
                b'/' => return Ficha::Nombre(self.nombre()),
                b'(' => {
                    self.pos += 1;
                    return Ficha::Cadena(self.cadena_sin_parentesis());
                }
                b'<' if self.b.get(self.pos + 1) == Some(&b'<') => self.saltar_diccionario(),
                b'<' => return Ficha::Cadena(self.cadena_hex()),
                b'[' => return Ficha::Lista(self.leer_lista()),
                b']' | b'}' | b'{' | b')' | b'>' => self.pos += 1,
                b'+' | b'-' | b'.' | b'0'..=b'9' => return Ficha::Numero(self.leer_numero()),
                _ => return Ficha::Operador(self.palabra()),
            }
        }
    }

    fn saltar(&mut self) {
        while self.pos < self.b.len() {
            match self.b[self.pos] {
                0 | 9 | 10 | 12 | 13 | 32 => self.pos += 1,
                b'%' => {
                    while self.pos < self.b.len()
                        && self.b[self.pos] != 10
                        && self.b[self.pos] != 13
                    {
                        self.pos += 1;
                    }
                }
                _ => return,
            }
        }
    }

    /// Un numero a mano: millones de ellos no pueden ser millones de cadenas.
    fn leer_numero(&mut self) -> f64 {
        let b = self.b;
        let mut signo = 1.0;
        if b[self.pos] == b'+' {
            self.pos += 1;
        } else if b[self.pos] == b'-' {
            signo = -1.0;
            self.pos += 1;
        }
        let mut v = 0.0f64;
        while self.pos < b.len() && b[self.pos].is_ascii_digit() {
            v = v * 10.0 + (b[self.pos] - b'0') as f64;
            self.pos += 1;
        }
        if self.pos < b.len() && b[self.pos] == b'.' {
            self.pos += 1;
            let mut escala = 0.1;
            while self.pos < b.len() && b[self.pos].is_ascii_digit() {
                v += (b[self.pos] - b'0') as f64 * escala;
                escala *= 0.1;
                self.pos += 1;
            }
        }
        // Hay archivos con `--5` o con exponente: se salta el resto.
        while self.pos < b.len() && !es_corte(b[self.pos]) {
            self.pos += 1;
        }
        signo * v
    }

    fn palabra(&mut self) -> &'x [u8] {
        let inicio = self.pos;
        while self.pos < self.b.len() && !es_corte(self.b[self.pos]) {
            self.pos += 1;
        }
        if self.pos == inicio {
            self.pos += 1;
        }
        &self.b[inicio..self.pos]
    }

    fn nombre(&mut self) -> String {
        self.pos += 1;
        let mut s = String::new();
        while self.pos < self.b.len() {
            let c = self.b[self.pos];
            if es_corte(c) {
                break;
            }
            self.pos += 1;
            if c == b'#'
                && self.pos + 1 < self.b.len()
                && let Ok(h) = u8::from_str_radix(
                    std::str::from_utf8(&self.b[self.pos..self.pos + 2]).unwrap_or("zz"),
                    16,
                )
            {
                s.push(h as char);
                self.pos += 2;
                continue;
            }
            s.push(c as char);
        }
        s
    }

    /// Una cadena `( … )`, con el parentesis de apertura ya pasado.
    fn cadena_sin_parentesis(&mut self) -> Vec<u8> {
        let b = self.b;
        let mut out = Vec::with_capacity(32);
        let mut nivel = 1;
        while self.pos < b.len() {
            let c = b[self.pos];
            self.pos += 1;
            match c {
                b'\\' => {
                    if self.pos >= b.len() {
                        break;
                    }
                    let d = b[self.pos];
                    self.pos += 1;
                    match d {
                        b'n' => out.push(10),
                        b'r' => out.push(13),
                        b't' => out.push(9),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'\n' => {}
                        b'\r' => {
                            if b.get(self.pos) == Some(&10) {
                                self.pos += 1;
                            }
                        }
                        b'0'..=b'7' => {
                            let mut v = (d - b'0') as u32;
                            let mut n = 1;
                            while n < 3
                                && self.pos < b.len()
                                && (b'0'..=b'7').contains(&b[self.pos])
                            {
                                v = v * 8 + (b[self.pos] - b'0') as u32;
                                self.pos += 1;
                                n += 1;
                            }
                            out.push((v & 0xFF) as u8);
                        }
                        otro => out.push(otro),
                    }
                }
                b'(' => {
                    nivel += 1;
                    out.push(c);
                }
                b')' => {
                    nivel -= 1;
                    if nivel == 0 {
                        break;
                    }
                    out.push(c);
                }
                _ => out.push(c),
            }
        }
        out
    }

    fn cadena_hex(&mut self) -> Vec<u8> {
        self.pos += 1;
        let inicio = self.pos;
        while self.pos < self.b.len() && self.b[self.pos] != b'>' {
            self.pos += 1;
        }
        let v = de_hex(&self.b[inicio..self.pos]);
        self.pos = (self.pos + 1).min(self.b.len());
        v
    }

    fn leer_lista(&mut self) -> Vec<Dato> {
        self.pos += 1;
        let mut out = Vec::with_capacity(8);
        while self.pos < self.b.len() {
            self.saltar();
            if self.pos >= self.b.len() {
                break;
            }
            match self.b[self.pos] {
                b']' => {
                    self.pos += 1;
                    break;
                }
                b'(' => {
                    self.pos += 1;
                    out.push(Dato::Cad(self.cadena_sin_parentesis()));
                }
                b'<' => out.push(Dato::Cad(self.cadena_hex())),
                b'/' => {
                    self.nombre();
                    out.push(Dato::Nom);
                }
                b'+' | b'-' | b'.' | b'0'..=b'9' => out.push(Dato::Num(self.leer_numero())),
                _ => {
                    self.palabra();
                }
            }
            if out.len() > 4096 {
                break;
            }
        }
        out
    }

    /// Un diccionario suelto en el contenido (el de `BDC`): se lee y se tira.
    fn saltar_diccionario(&mut self) {
        let b = self.b;
        let mut nivel = 0;
        while self.pos < b.len() {
            if b[self.pos..].starts_with(b"<<") {
                nivel += 1;
                self.pos += 2;
                continue;
            }
            if b[self.pos..].starts_with(b">>") {
                nivel -= 1;
                self.pos += 2;
                if nivel <= 0 {
                    return;
                }
                continue;
            }
            if b[self.pos] == b'(' {
                self.pos += 1;
                self.cadena_sin_parentesis();
                continue;
            }
            self.pos += 1;
        }
    }

    /// Una imagen incrustada (`BI … ID …datos… EI`): los datos van en crudo,
    /// asi que se busca un `EI` suelto —con blanco delante y detras—, porque
    /// esos dos bytes pueden salir dentro de la imagen por casualidad.
    fn saltar_imagen_incrustada(&mut self) {
        let b = self.b;
        let mut i = self.pos;
        while i + 1 < b.len() {
            if b[i] == b'I' && b[i + 1] == b'D' {
                i += 2;
                break;
            }
            i += 1;
        }
        if i + 1 < b.len() {
            i += 1;
        }
        while i + 2 < b.len() {
            if matches!(b[i], 32 | 10 | 13 | 9)
                && b[i + 1] == b'E'
                && b[i + 2] == b'I'
                && (i + 3 >= b.len() || es_corte(b[i + 3]))
            {
                self.pos = i + 3;
                return;
            }
            i += 1;
        }
        self.pos = b.len();
    }
}

// ---------------------------------------------------------------------------
// Las fuentes

/// **Lo justo de una fuente para colocar un rotulo**: que dice cada codigo y
/// cuanto ocupa. No se extrae la tipografia: el navegador pone la suya y se
/// estira hasta el ancho que decia el PDF.
struct Fuente {
    dos_bytes: bool,
    anchos: HashMap<u32, f64>,
    ancho_por_defecto: f64,
    a_texto: HashMap<u32, String>,
    familia: &'static str,
    negrita: bool,
    cursiva: bool,
}

struct Letra {
    texto: Option<String>,
    ancho: f64,
    es_espacio: bool,
}

/// De 128 a 160 WinAnsi pone comillas y guiones donde Latin-1 no tiene nada.
const WIN_ANSI: [Option<&str>; 33] = [
    Some("€"),
    None,
    Some("‚"),
    Some("ƒ"),
    Some("„"),
    Some("…"),
    Some("†"),
    Some("‡"),
    Some("ˆ"),
    Some("‰"),
    Some("Š"),
    Some("‹"),
    Some("Œ"),
    None,
    Some("Ž"),
    None,
    None,
    Some("\u{2018}"),
    Some("\u{2019}"),
    Some("“"),
    Some("”"),
    Some("•"),
    Some("–"),
    Some("—"),
    Some("˜"),
    Some("™"),
    Some("š"),
    Some("›"),
    Some("œ"),
    None,
    Some("ž"),
    Some("Ÿ"),
    Some(" "),
];

impl Fuente {
    fn letras(&self, bytes: &[u8]) -> Vec<Letra> {
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            let codigo = if self.dos_bytes {
                if i + 1 >= bytes.len() {
                    break;
                }
                let c = ((bytes[i] as u32) << 8) | bytes[i + 1] as u32;
                i += 2;
                c
            } else {
                let c = bytes[i] as u32;
                i += 1;
                c
            };
            let texto = self
                .a_texto
                .get(&codigo)
                .cloned()
                .or_else(|| if self.dos_bytes { None } else { latin(codigo) });
            out.push(Letra {
                texto,
                ancho: self
                    .anchos
                    .get(&codigo)
                    .copied()
                    .unwrap_or(self.ancho_por_defecto),
                es_espacio: !self.dos_bytes && codigo == 32,
            });
        }
        out
    }

    fn leer(a: &Archivo, d: &Dicc) -> Fuente {
        let nombre = |v: Option<Valor>| match v {
            Some(Valor::Nombre(n)) => Some(String::from_utf8_lossy(&n).into_owned()),
            _ => None,
        };
        let subtipo = nombre(a.resolver(en(d, b"Subtype"))).unwrap_or_default();
        let base = nombre(a.resolver(en(d, b"BaseFont"))).unwrap_or_default();
        let enc = a.resolver(en(d, b"Encoding"));
        let nombre_enc = nombre(enc.clone());
        let tipo0 = subtipo == "Type0";
        let dos_bytes = tipo0
            && nombre_enc.as_deref().is_none_or(|n| {
                n.starts_with("Identity")
                    || n.contains("UCS2")
                    || n.ends_with("-H")
                    || n.ends_with("-V")
            });
        let mut anchos = HashMap::new();
        let mut por_defecto = if tipo0 { 1000.0 } else { 500.0 };
        if tipo0 {
            let hijo = match a.resolver(en(d, b"DescendantFonts")) {
                Some(Valor::Lista(l)) => l.first().and_then(|v| a.dicc_de(Some(v))),
                _ => None,
            };
            if let Some(h) = &hijo {
                if let Some(v) = num(a, en(h, b"DW")) {
                    por_defecto = v;
                }
                if let Some(Valor::Lista(w)) = a.resolver(en(h, b"W")) {
                    leer_w(a, &w, &mut anchos);
                }
            }
        } else {
            let primero = num(a, en(d, b"FirstChar")).unwrap_or(0.0) as u32;
            if let Some(Valor::Lista(w)) = a.resolver(en(d, b"Widths")) {
                for (i, v) in w.iter().enumerate() {
                    if let Some(x) = num(a, Some(v)) {
                        anchos.insert(primero + i as u32, x);
                    }
                }
            }
            if !anchos.is_empty() {
                por_defecto = a
                    .dicc_de(en(d, b"FontDescriptor"))
                    .and_then(|fd| num(a, en(&fd, b"MissingWidth")))
                    .unwrap_or(0.0);
            }
        }
        let mut a_texto = HashMap::new();
        if let Some(Valor::Flujo(fd, datos)) = a.resolver(en(d, b"ToUnicode"))
            && let Some(t) = descomprimir(a, &fd, &datos)
        {
            leer_cmap(&t, &mut a_texto);
        }
        if let Some(Valor::Dicc(e)) = &enc {
            leer_diferencias(a, e, &mut a_texto);
        }
        // **MacRoman** (Android v0.98.0): de 128 en adelante no es Latin-1;
        // sin esto la «fi» de un articulo hecho en Mac salia «Þ» y el Ctrl+F
        // de la pagina web no encontraba «defined». Solo donde no hayan
        // dicho nada el `ToUnicode` ni las `Differences`.
        if !tipo0 && crate::texto::es_mac_roman(a, d) {
            for b in 128u8..=255 {
                if let Some(t) = crate::texto::mac_roman(b) {
                    a_texto.entry(u32::from(b)).or_insert_with(|| t.to_string());
                }
            }
        }
        let minus = base.to_lowercase();
        // **Las estrechas se reconocen y se dicen**: un cajetin las usa a
        // mansalva, y sustituidas por una normal la letra sale aplastada.
        let estrecha = ["narrow", "condensed", "cond", "compressed"]
            .iter()
            .any(|k| minus.contains(k));
        let familia = if minus.contains("courier") || minus.contains("mono") {
            "monospace"
        } else if ["times", "georgia", "garamond", "cambria", "minion"]
            .iter()
            .any(|k| minus.contains(k))
            || (minus.contains("serif") && !minus.contains("sans"))
        {
            "serif"
        } else if estrecha {
            "sans-serif-condensed"
        } else {
            "sans-serif"
        };
        Fuente {
            dos_bytes,
            anchos,
            ancho_por_defecto: por_defecto,
            a_texto,
            familia,
            negrita: ["bold", "black", "heavy"].iter().any(|k| minus.contains(k)),
            cursiva: minus.contains("italic") || minus.contains("oblique"),
        }
    }
}

fn latin(c: u32) -> Option<String> {
    match c {
        0..=31 | 127 => None,
        32..=126 => Some((c as u8 as char).to_string()),
        128..=160 => WIN_ANSI[(c - 128) as usize].map(str::to_string),
        _ => char::from_u32(c).map(|x| x.to_string()),
    }
}

/// `/W` de una fuente CID: `[ 3 [200 300] 10 20 500 ]`.
fn leer_w(a: &Archivo, v: &[Valor], out: &mut HashMap<u32, f64>) {
    let mut i = 0;
    while i < v.len() {
        let Some(primero) = num(a, Some(&v[i])).map(|x| x as i64) else {
            break;
        };
        match a.resolver(v.get(i + 1)) {
            Some(Valor::Lista(l)) => {
                for (k, x) in l.iter().enumerate() {
                    if let Some(w) = num(a, Some(x)) {
                        out.insert((primero + k as i64) as u32, w);
                    }
                }
                i += 2;
            }
            Some(sig) => {
                let (Some(ultimo), Some(ancho)) =
                    (numero_de(&sig).map(|x| x as i64), num(a, v.get(i + 2)))
                else {
                    break;
                };
                if (0..=65535).contains(&(ultimo - primero)) {
                    for c in primero..=ultimo {
                        out.insert(c as u32, ancho);
                    }
                }
                i += 3;
            }
            None => break,
        }
    }
}

/// El destino de un CMap es UTF-16 en hexadecimal: `<0041>` es la «A».
fn texto_utf16(b: &[u8]) -> String {
    if b.len() >= 2 {
        let u: Vec<u16> = b
            .chunks_exact(2)
            .map(|p| u16::from_be_bytes([p[0], p[1]]))
            .collect();
        String::from_utf16_lossy(&u)
    } else {
        b.iter().map(|&c| c as char).collect()
    }
}

fn codigo_de(b: &[u8]) -> u32 {
    b.iter().fold(0u32, |v, &x| (v << 8) | x as u32)
}

/// **El `/ToUnicode`**: un CMap con `beginbfchar` y `beginbfrange`. Es lo
/// que dice que letra es cada codigo cuando la fuente va con su propia
/// numeracion, lo normal en un plano de AutoCAD. Sin esto, jeroglificos.
fn leer_cmap(datos: &[u8], out: &mut HashMap<u32, String>) {
    let mut f = Fichas { b: datos, pos: 0 };
    let mut modo = 0; // 1 = sueltos, 2 = tramos
    let mut pila: Vec<Dato> = Vec::with_capacity(3);
    loop {
        match f.siguiente() {
            Ficha::Fin => return,
            Ficha::Cadena(c) => pila.push(Dato::Cad(c)),
            Ficha::Numero(n) => pila.push(Dato::Num(n)),
            Ficha::Lista(l) => pila.push(Dato::Lis(l)),
            Ficha::Nombre(_) => {}
            Ficha::Operador(op) => {
                match op {
                    b"beginbfchar" => modo = 1,
                    b"beginbfrange" => modo = 2,
                    _ => {
                        modo = if op.starts_with(b"end") || op.starts_with(b"begin") {
                            0
                        } else {
                            modo
                        }
                    }
                }
                pila.clear();
            }
        }
        if modo == 1 && pila.len() >= 2 {
            if let (Dato::Cad(de), Dato::Cad(a)) = (&pila[0], &pila[1]) {
                let t = texto_utf16(a);
                if !t.is_empty() {
                    out.insert(codigo_de(de), t);
                }
            }
            pila.clear();
        } else if modo == 2 && pila.len() >= 3 {
            if let (Dato::Cad(de), Dato::Cad(hasta)) = (&pila[0], &pila[1]) {
                let (d, h) = (codigo_de(de), codigo_de(hasta));
                if h >= d && h - d <= 65535 {
                    match &pila[2] {
                        Dato::Cad(a) => {
                            // Un tramo con un solo destino numera desde ahi.
                            let base = texto_utf16(a);
                            if let Some(ultimo) = base.chars().last() {
                                let sin: String =
                                    base.chars().take(base.chars().count() - 1).collect();
                                for i in 0..=(h - d) {
                                    if let Some(ch) = char::from_u32(ultimo as u32 + i) {
                                        out.insert(d + i, format!("{sin}{ch}"));
                                    }
                                }
                            }
                        }
                        Dato::Lis(l) => {
                            for (i, x) in l.iter().enumerate().take((h - d + 1) as usize) {
                                if let Dato::Cad(a) = x {
                                    let t = texto_utf16(a);
                                    if !t.is_empty() {
                                        out.insert(d + i as u32, t);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            pila.clear();
        }
        if pila.len() > 3 {
            pila.clear();
        }
    }
}

fn leer_diferencias(a: &Archivo, enc: &Dicc, out: &mut HashMap<u32, String>) {
    let Some(Valor::Lista(l)) = a.resolver(en(enc, b"Differences")) else {
        return;
    };
    let mut codigo = 0u32;
    for v in &l {
        match a.resolver(Some(v)) {
            Some(Valor::Numero(_)) => codigo = num(a, Some(v)).unwrap_or(0.0) as u32,
            Some(Valor::Nombre(n)) => {
                let n = String::from_utf8_lossy(&n).into_owned();
                if let Some(t) = de_nombre_de_glifo(&n) {
                    out.entry(codigo).or_insert(t);
                }
                codigo += 1;
            }
            _ => {}
        }
    }
}

/// `/uni0041` y `/A` son la «A».
fn de_nombre_de_glifo(n: &str) -> Option<String> {
    if let Some(h) = n.strip_prefix("uni")
        && h.len() >= 4
    {
        return u32::from_str_radix(&h[..4], 16)
            .ok()
            .and_then(char::from_u32)
            .map(|c| c.to_string());
    }
    (n.chars().count() == 1).then(|| n.to_string())
}

#[cfg(test)]
#[path = "plano/pruebas.rs"]
pub(crate) mod pruebas;

/// Hasta que hueco (en emes) dos trozos seguidos de un renglon son de la
/// misma frase. Un espacio ronda un cuarto de eme, y en un parrafo
/// justificado se estira hasta media; a partir de casi una eme ya es otra
/// cosa (una columna, un tabulador, una cota al lado).
const HUECO_DE_LA_MISMA_FRASE: f64 = 0.8;
/// Desde que hueco se pone un espacio entre los dos trozos: el
/// interletrado de Word (`[(B)20(e)-15(ne)] TJ`) mueve centesimas de eme.
const HUECO_DE_UN_ESPACIO: f64 = 0.12;

/// **Los trozos de un mismo renglon, juntos.** Word y los procesadores de
/// texto escriben cada palabra en trozos para ajustar el interletrado
/// (`[(B)20(e)-15(ne)] TJ`), y cada trozo salia como un rotulo aparte: la
/// pagina web se veia igual, pero el Ctrl+F del navegador no encontraba
/// «Beneficios» porque no estaba escrita entera en ningun sitio. Se juntan
/// los seguidos que van en la misma direccion, con la misma letra y el mismo
/// color, y cuyo hueco es de interletrado o de espacio; el rotulo junto
/// ocupa del principio del primero al final del ultimo, que es lo que el
/// visor estira.
fn juntar_renglones(textos: Vec<Texto>) -> Vec<Texto> {
    let mut salida: Vec<Texto> = Vec::with_capacity(textos.len());
    for t in textos {
        if let Some(p) = salida.last_mut()
            && let Some(hueco) = hueco_entre(p, &t)
        {
            let espacio = hueco > HUECO_DE_UN_ESPACIO
                && !p.texto.ends_with(char::is_whitespace)
                && !t.texto.starts_with(char::is_whitespace);
            if espacio {
                p.texto.push(' ');
            }
            p.texto.push_str(&t.texto);
            p.ancho += hueco + t.ancho;
            continue;
        }
        salida.push(t);
    }
    salida
}

/// El hueco en emes entre el final de `p` y el principio de `t` si `t` sigue
/// a `p` en su renglon; `None` si no es de la misma frase.
fn hueco_entre(p: &Texto, t: &Texto) -> Option<f64> {
    let misma_letra = p.capa == t.capa
        && p.color == t.color
        && (p.alfa - t.alfa).abs() < 1e-3
        && p.familia == t.familia
        && p.negrita == t.negrita
        && p.cursiva == t.cursiva;
    let eme2 = p.a * p.a + p.b * p.b;
    if !misma_letra || eme2 < 1e-6 {
        return None;
    }
    let tol = 1e-3 * eme2.sqrt();
    let igual = |x: f64, y: f64| (x - y).abs() <= tol;
    if !(igual(p.a, t.a) && igual(p.b, t.b) && igual(p.c, t.c) && igual(p.d, t.d)) {
        return None;
    }
    // Del final de `p` al principio de `t`, a lo largo del renglon y de
    // traves, en emes.
    let (fx, fy) = (p.x + p.a * p.ancho, p.y + p.b * p.ancho);
    let (gx, gy) = (t.x - fx, t.y - fy);
    let largo = (gx * p.a + gy * p.b) / eme2;
    let traves = (gy * p.a - gx * p.b) / eme2;
    (traves.abs() < 0.2 && largo > -0.3 && largo < HUECO_DE_LA_MISMA_FRASE).then_some(largo)
}
