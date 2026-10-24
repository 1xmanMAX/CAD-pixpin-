//! Una hoja: sus celdas guardadas en poco sitio y el lector que las va
//! sacando del XML mientras se descomprime.
//!
//! El XML de una hoja grande pesa decenas de megas. No se descomprime entero
//! ni se convierte en arbol: [`LectorHoja`] lo recorre de tiron y entrega
//! filas por tandas ([`Tanda`]), de modo que la primera pantalla se pinta
//! con las cien primeras filas mientras el resto sigue llegando (en este
//! hilo, poco a poco, o en otro: el lector es `Send`).

use std::io::Read;
use std::sync::Arc;

use crate::dibujos::{self, Dibujo};
use crate::paquete::Paquete;
use crate::xml::{self, Evento, Nodo, Paso};

/// El valor guardado de una celda. Las formulas no se evaluan: se muestra lo
/// que Excel dejo calculado.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Valor {
    Vacio,
    Numero(f64),
    /// Indice en los textos compartidos del libro.
    Compartido(u32),
    /// Indice en los textos propios de la hoja (`str`, `inlineStr`).
    Propio(u32),
    Logico(bool),
    /// Indice en [`ERRORES`].
    Error(u8),
}

pub const ERRORES: [&str; 11] = [
    "#NULL!",
    "#DIV/0!",
    "#VALUE!",
    "#REF!",
    "#NAME?",
    "#NUM!",
    "#N/A",
    "#GETTING_DATA",
    "#SPILL!",
    "#CALC!",
    "#FIELD!",
];

/// Una celda: 24 bytes. Dos millones caben en 48 MB.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Celda {
    /// Columna, desde 0.
    pub col: u32,
    /// Estilo (`cellXfs`).
    pub xf: u32,
    pub valor: Valor,
}

/// Una fila con algo (celdas o formato propio).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fila {
    /// Fila, desde 0.
    pub indice: u32,
    /// Alto en puntos; `NaN` si es el de por defecto.
    pub alto: f32,
    pub oculta: bool,
    /// Estilo de la fila entera (`customFormat`).
    pub xf: Option<u32>,
    /// La fila no trae `ht`: Excel la ajusta al abrir a lo que ocupe su
    /// texto (ver `pixpin_render::hoja_office::autoajustar_filas`).
    pub sin_alto: bool,
    inicio: u32,
    fin: u32,
}

/// Una fila para armar una hoja de una vez ([`Hoja::desde_filas`]).
#[derive(Debug, Clone)]
pub struct FilaNueva {
    pub indice: u32,
    pub alto: f32,
    pub oculta: bool,
    pub xf: Option<u32>,
    pub sin_alto: bool,
    pub celdas: Vec<Celda>,
}

/// Un grupo de columnas `<col min max>`, en base 0 e inclusivo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Columnas {
    pub min: u32,
    pub max: u32,
    /// Ancho en caracteres tal cual el XML (`None`: el de por defecto).
    pub ancho: Option<f64>,
    pub oculta: bool,
    pub xf: u32,
}

/// Un rango de celdas, base 0, inclusivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Rango {
    pub fila0: u32,
    pub col0: u32,
    pub fila1: u32,
    pub col1: u32,
}

impl Rango {
    pub fn contiene(&self, fila: u32, col: u32) -> bool {
        fila >= self.fila0 && fila <= self.fila1 && col >= self.col0 && col <= self.col1
    }

    /// `A1:C3` o `B2`.
    pub fn leer(s: &str) -> Option<Rango> {
        let s = s.trim();
        let (a, b) = s.split_once(':').unwrap_or((s, s));
        let (f0, c0) = referencia(a.as_bytes())?;
        let (f1, c1) = referencia(b.as_bytes())?;
        Some(Rango {
            fila0: f0.min(f1),
            col0: c0.min(c1),
            fila1: f0.max(f1),
            col1: c0.max(c1),
        })
    }
}

/// `B12` -> (11, 1). Admite `$`. Topes de Excel: 1 048 576 filas, 16 384
/// columnas.
pub fn referencia(s: &[u8]) -> Option<(u32, u32)> {
    let mut col: u32 = 0;
    let mut i = 0;
    while i < s.len() && s[i] == b'$' {
        i += 1;
    }
    let ini = i;
    while i < s.len() && s[i].is_ascii_alphabetic() {
        col = col
            .checked_mul(26)?
            .checked_add((s[i].to_ascii_uppercase() - b'A' + 1) as u32)?;
        i += 1;
    }
    if i == ini || col > 16384 {
        return None;
    }
    while i < s.len() && s[i] == b'$' {
        i += 1;
    }
    let mut fila: u32 = 0;
    let ini_f = i;
    while i < s.len() && s[i].is_ascii_digit() {
        fila = fila.checked_mul(10)?.checked_add((s[i] - b'0') as u32)?;
        i += 1;
    }
    if i == ini_f || fila == 0 || fila > 1_048_576 || i != s.len() {
        return None;
    }
    Some((fila - 1, col - 1))
}

/// Solo la columna de una referencia (`B12` -> 1), sin mirar la fila.
fn columna_de(s: &[u8]) -> Option<u32> {
    let mut col: u32 = 0;
    let mut vista = false;
    for &b in s {
        if b == b'$' {
            continue;
        }
        if b.is_ascii_alphabetic() {
            col = col
                .checked_mul(26)?
                .checked_add((b.to_ascii_uppercase() - b'A' + 1) as u32)?;
            vista = true;
        } else {
            break;
        }
    }
    if !vista || col > 16384 {
        None
    } else {
        Some(col - 1)
    }
}

/// El nombre de una columna: 0 -> `A`, 27 -> `AB`.
pub fn nombre_columna(mut c: u32) -> String {
    let mut v = Vec::new();
    loop {
        v.push(b'A' + (c % 26) as u8);
        if c < 26 {
            break;
        }
        c = c / 26 - 1;
    }
    v.reverse();
    String::from_utf8(v).unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hipervinculo {
    pub rango: Rango,
    /// Una URL (externo) o un sitio del libro (`Hoja2!A1`).
    pub destino: String,
    pub externo: bool,
    pub ayuda: Option<String>,
}

/// La vista guardada (`sheetView`).
#[derive(Debug, Clone, PartialEq)]
pub struct VistaHoja {
    /// Filas y columnas inmovilizadas (paneles congelados).
    pub filas_fijas: u32,
    pub columnas_fijas: u32,
    /// La primera celda de la zona que se desplaza.
    pub primera_fila: u32,
    pub primera_columna: u32,
    pub cuadricula: bool,
    pub encabezados: bool,
    /// Porcentaje (10-400).
    pub zoom: u32,
    pub derecha_a_izquierda: bool,
    pub celda_activa: Option<(u32, u32)>,
}

impl Default for VistaHoja {
    fn default() -> Self {
        Self {
            filas_fijas: 0,
            columnas_fijas: 0,
            primera_fila: 0,
            primera_columna: 0,
            cuadricula: true,
            encabezados: true,
            zoom: 100,
            derecha_a_izquierda: false,
            celda_activa: None,
        }
    }
}

/// Lo que va antes de `<sheetData>`.
#[derive(Debug, Clone, Default)]
pub struct Cabecera {
    pub vista: VistaHoja,
    pub columnas: Vec<Columnas>,
    pub ancho_defecto: Option<f64>,
    pub ancho_base: Option<f64>,
    pub alto_defecto: Option<f32>,
    pub filas_ocultas_por_defecto: bool,
    pub dimension: Option<Rango>,
    pub color_pestana: Option<crate::estilos::Rgb>,
}

/// Lo que va despues de `</sheetData>`, ya resuelto.
#[derive(Debug, Clone, Default)]
pub struct Cola {
    pub combinadas: Vec<Rango>,
    pub hipervinculos: Vec<Hipervinculo>,
    pub dibujos: Vec<Dibujo>,
    pub tablas: Vec<crate::tablas::Tabla>,
    /// Formato condicional (con sus cuentas hechas al acabar de cargar).
    pub condicionales: Vec<crate::condicional::Condicional>,
    /// Validacion de datos (solo para ver: listas desplegables).
    pub validaciones: Vec<crate::validacion::Validacion>,
    /// Minigraficos: sus grupos y, por celda (ordenados), cada uno.
    pub grupos_mini: Vec<crate::minigraficos::GrupoMini>,
    pub minigraficos: Vec<crate::minigraficos::Minigrafico>,
    /// Notas y comentarios encadenados, ordenados por celda.
    pub comentarios: Vec<crate::comentarios::Comentario>,
    /// El autofiltro de la hoja (los de las tablas estan en cada tabla).
    pub autofiltro: Option<crate::filtros::AutoFiltro>,
}

/// Un trozo de hoja leido: filas y celdas con indices propios de la tanda.
#[derive(Debug, Default)]
pub struct Tanda {
    pub filas: Vec<Fila>,
    pub celdas: Vec<Celda>,
    pub textos: Vec<String>,
    pub cabecera: Option<Cabecera>,
    pub cola: Option<Cola>,
    /// Ya no hay mas: el lector termino (o el XML se acabo de golpe).
    pub fin: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Estado {
    Cabecera,
    Datos,
    Cola,
    Fin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dentro {
    Nada,
    V,
    T,
    Fonetica,
}

/// El lector de una hoja de tiron. Es `Send`: se puede llevar a otro hilo y
/// mandar las tandas al de la interfaz.
pub struct LectorHoja {
    fuente: Box<dyn Read + Send>,
    buf: Vec<u8>,
    pos: usize,
    eof: bool,
    p: Proceso,
    paquete: Paquete,
    ruta: String,
    tema: Arc<crate::estilos::Tema>,
    paleta: Arc<[u32]>,
}

/// Bytes que se piden al descompresor de cada vez.
const PEDAZO: usize = 256 * 1024;
/// Tope de la cabecera y la cola (lo de fuera de `sheetData`).
const TOPE_FUERA: usize = 256 * 1024 * 1024;

struct Proceso {
    estado: Estado,
    tanda: Tanda,
    // Fila en curso.
    fila_abierta: bool,
    fila: Fila,
    ultima_fila: i64,
    max_tam_fila: f32,
    // Celda en curso.
    celda_abierta: bool,
    celda: Celda,
    tipo: [u8; 9],
    tipo_len: usize,
    ultima_col: i64,
    dentro: Dentro,
    texto: Vec<u8>,
    tiene_v: bool,
    // Para la altura automatica de filas sin `ht`.
    tam_por_xf: Arc<[f32]>,
    tam_normal: f32,
    alto_normal: f32,
}

impl LectorHoja {
    pub(crate) fn nuevo(
        paquete: &Paquete,
        ruta: &str,
        tam_por_xf: Arc<[f32]>,
        tam_normal: f32,
        alto_normal: f32,
        tema: Arc<crate::estilos::Tema>,
        paleta: Arc<[u32]>,
    ) -> Option<LectorHoja> {
        // Una hoja grande se descomprime en otro hilo mientras esta lee.
        let fuente = if paquete.comprimido(ruta).unwrap_or(0) > 512 * 1024 {
            paquete.lector_en_hilo(ruta)?
        } else {
            paquete.lector(ruta)?
        };
        Some(LectorHoja {
            fuente,
            buf: Vec::with_capacity(PEDAZO * 2),
            pos: 0,
            eof: false,
            paquete: paquete.clone(),
            ruta: ruta.to_string(),
            tema,
            paleta,
            p: Proceso {
                estado: Estado::Cabecera,
                tanda: Tanda::default(),
                fila_abierta: false,
                fila: Fila {
                    indice: 0,
                    alto: f32::NAN,
                    oculta: false,
                    xf: None,
                    sin_alto: true,
                    inicio: 0,
                    fin: 0,
                },
                ultima_fila: -1,
                max_tam_fila: 0.0,
                celda_abierta: false,
                celda: Celda {
                    col: 0,
                    xf: 0,
                    valor: Valor::Vacio,
                },
                tipo: [0; 9],
                tipo_len: 0,
                ultima_col: -1,
                dentro: Dentro::Nada,
                texto: Vec::new(),
                tiene_v: false,
                tam_por_xf,
                tam_normal,
                alto_normal,
            },
        })
    }

    pub fn terminado(&self) -> bool {
        self.p.estado == Estado::Fin
    }

    /// Lee mas: devuelve la tanda cuando ha cerrado una fila de indice >=
    /// `hasta_fila`, o cuando ya lleva `max_celdas`, o al terminar.
    pub fn leer(&mut self, hasta_fila: u32, max_celdas: usize) -> Tanda {
        loop {
            if self.p.estado == Estado::Fin {
                break;
            }
            if self.p.estado == Estado::Cola {
                // El resto entero, sin mirar dentro: es poco.
                self.leer_hasta_el_final();
                let cola = self.p.leer_cola(
                    &self.buf[self.pos..],
                    &self.paquete,
                    &self.ruta,
                    &self.tema,
                    &self.paleta,
                );
                self.p.tanda.cola = Some(cola);
                self.p.estado = Estado::Fin;
                break;
            }
            // El camino rapido de la celda corriente (`<c ...><v>..</v></c>`):
            // casi todo el XML de una hoja grande es eso.
            if self.p.estado == Estado::Datos
                && self.p.fila_abierta
                && self.buf[self.pos..].starts_with(b"<c ")
            {
                if let Some(n) = self.p.celda_rapida(&self.buf[self.pos..]) {
                    self.pos += n;
                    continue;
                }
            }
            let paso = xml::siguiente(&self.buf, &mut self.pos, self.eof);
            match paso {
                Paso::Evento(e) => {
                    let antes_de_la_etiqueta = self.pos;
                    let parar =
                        self.p
                            .evento(e, &self.buf, antes_de_la_etiqueta, hasta_fila, max_celdas);
                    if parar {
                        break;
                    }
                }
                Paso::Incompleto => {
                    if !self.rellenar() {
                        self.eof = true;
                    }
                }
                Paso::Fin => {
                    // El XML se acabo (o venia cortado).
                    self.p.cerrar_lo_abierto();
                    if self.p.estado == Estado::Cabecera {
                        let cab = cabecera_de(&xml::arbol(&self.buf));
                        self.p.tanda.cabecera = Some(cab);
                    }
                    if self.p.tanda.cola.is_none() {
                        self.p.tanda.cola = Some(Cola::default());
                    }
                    self.p.estado = Estado::Fin;
                    break;
                }
            }
        }
        let mut t = std::mem::take(&mut self.p.tanda);
        t.fin = self.p.estado == Estado::Fin;
        t
    }

    fn rellenar(&mut self) -> bool {
        if self.eof {
            return false;
        }
        // En la cabecera se guarda todo (se lee como arbol al acabar); en los
        // datos se tira lo ya leido.
        if self.p.estado == Estado::Datos && self.pos > 0 {
            self.buf.drain(..self.pos);
            self.pos = 0;
        }
        if self.buf.len() > TOPE_FUERA && self.p.estado == Estado::Cabecera {
            return false;
        }
        let largo = self.buf.len();
        self.buf.resize(largo + PEDAZO, 0);
        let mut n = 0;
        while n == 0 {
            match self.fuente.read(&mut self.buf[largo..]) {
                Ok(0) => {
                    self.buf.truncate(largo);
                    return false;
                }
                Ok(k) => n = k,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => {
                    self.buf.truncate(largo);
                    return false;
                }
            }
        }
        self.buf.truncate(largo + n);
        true
    }

    fn leer_hasta_el_final(&mut self) {
        if self.pos > 0 {
            self.buf.drain(..self.pos);
            self.pos = 0;
        }
        while self.buf.len() < TOPE_FUERA && self.rellenar_cola() {}
        self.eof = true;
    }

    fn rellenar_cola(&mut self) -> bool {
        let largo = self.buf.len();
        self.buf.resize(largo + PEDAZO, 0);
        match self.fuente.read(&mut self.buf[largo..]) {
            Ok(0) | Err(_) => {
                self.buf.truncate(largo);
                false
            }
            Ok(k) => {
                self.buf.truncate(largo + k);
                true
            }
        }
    }
}

fn es_si(v: &[u8]) -> bool {
    v == b"1" || v == b"true"
}

fn num<T: std::str::FromStr>(v: &[u8]) -> Option<T> {
    std::str::from_utf8(v).ok()?.trim().parse().ok()
}

impl Proceso {
    /// Atiende un evento. Devuelve `true` si hay que entregar la tanda.
    fn evento(
        &mut self,
        e: Evento<'_>,
        buf: &[u8],
        pos_tras: usize,
        hasta_fila: u32,
        max_celdas: usize,
    ) -> bool {
        match self.estado {
            Estado::Cabecera => {
                if let Evento::Abre {
                    nombre: b"sheetData",
                    vacia,
                    ..
                } = e
                {
                    // Lo de antes de `<sheetData` se lee como arbol.
                    let ini = buf[..pos_tras]
                        .iter()
                        .rposition(|&b| b == b'<')
                        .unwrap_or(0);
                    let cab = cabecera_de(&xml::arbol(&buf[..ini]));
                    self.tanda.cabecera = Some(cab);
                    self.estado = if vacia { Estado::Cola } else { Estado::Datos };
                    return true;
                }
                false
            }
            Estado::Datos => self.dato(e, hasta_fila, max_celdas),
            Estado::Cola | Estado::Fin => false,
        }
    }

    fn dato(&mut self, e: Evento<'_>, hasta_fila: u32, max_celdas: usize) -> bool {
        match e {
            Evento::Abre {
                nombre,
                attrs,
                vacia,
            } => match nombre {
                b"c" => {
                    if !self.fila_abierta {
                        self.abrir_fila(b"");
                    }
                    self.celda_abierta = true;
                    self.tiene_v = false;
                    self.texto.clear();
                    self.tipo_len = 0;
                    let mut col: Option<u32> = None;
                    let mut xf = 0u32;
                    for (n, v) in xml::atributos(attrs) {
                        match n {
                            b"r" => col = columna_de(v),
                            b"s" => xf = num(v).unwrap_or(0),
                            b"t" => {
                                let k = v.len().min(9);
                                self.tipo[..k].copy_from_slice(&v[..k]);
                                self.tipo_len = k;
                            }
                            _ => {}
                        }
                    }
                    let col = col.unwrap_or((self.ultima_col + 1).clamp(0, 16383) as u32);
                    self.ultima_col = col as i64;
                    self.celda = Celda {
                        col,
                        xf,
                        valor: Valor::Vacio,
                    };
                    if vacia {
                        self.cerrar_celda();
                    }
                }
                b"v" if self.celda_abierta && !vacia => {
                    self.dentro = Dentro::V;
                    self.tiene_v = true;
                }
                b"t" if self.celda_abierta && !vacia && self.dentro != Dentro::Fonetica => {
                    self.dentro = Dentro::T;
                    self.tiene_v = true;
                }
                b"rPh" if !vacia => self.dentro = Dentro::Fonetica,
                b"row" => {
                    if self.fila_abierta {
                        self.cerrar_fila();
                    }
                    self.abrir_fila(attrs);
                    if vacia {
                        return self.cerrar_fila_y_ver(hasta_fila, max_celdas);
                    }
                }
                _ => {}
            },
            Evento::Texto(t) | Evento::Cdata(t) => {
                if matches!(self.dentro, Dentro::V | Dentro::T) {
                    if let Evento::Cdata(_) = e {
                        // CDATA es literal: se escapa el & para que el
                        // desescapado de despues lo deje igual.
                        for &b in t {
                            if b == b'&' {
                                self.texto.extend_from_slice(b"&amp;");
                            } else if b == b'<' {
                                self.texto.extend_from_slice(b"&lt;");
                            } else {
                                self.texto.push(b);
                            }
                        }
                    } else {
                        self.texto.extend_from_slice(t);
                    }
                }
            }
            Evento::Cierra { nombre } => match nombre {
                b"v" | b"t" if self.dentro != Dentro::Fonetica => self.dentro = Dentro::Nada,
                b"rPh" => self.dentro = Dentro::Nada,
                b"c" => {
                    if self.celda_abierta {
                        self.cerrar_celda();
                    }
                }
                b"row" => return self.cerrar_fila_y_ver(hasta_fila, max_celdas),
                b"sheetData" => {
                    self.cerrar_lo_abierto();
                    self.estado = Estado::Cola;
                    return true;
                }
                _ => {}
            },
        }
        false
    }

    fn cerrar_fila_y_ver(&mut self, hasta_fila: u32, max_celdas: usize) -> bool {
        if self.fila_abierta {
            self.cerrar_fila();
        }
        self.ultima_fila >= hasta_fila as i64 || self.tanda.celdas.len() >= max_celdas
    }

    fn abrir_fila(&mut self, attrs: &[u8]) {
        let mut f = Fila {
            indice: 0,
            alto: f32::NAN,
            oculta: false,
            xf: None,
            sin_alto: true,
            inicio: 0,
            fin: 0,
        };
        let mut r: Option<u32> = None;
        let mut s: Option<u32> = None;
        let mut formato_propio = false;
        for (n, v) in xml::atributos(attrs) {
            match n {
                b"r" => {
                    r = num::<u32>(v)
                        .filter(|&x| (1..=1_048_576).contains(&x))
                        .map(|x| x - 1)
                }
                b"ht" => {
                    if let Some(h) = num::<f32>(v) {
                        if h.is_finite() && (0.0..=409.5).contains(&h) {
                            f.alto = h;
                            f.sin_alto = false;
                        }
                    }
                }
                b"hidden" => f.oculta = es_si(v),
                b"s" => s = num(v),
                b"customFormat" => formato_propio = es_si(v),
                _ => {}
            }
        }
        if formato_propio {
            f.xf = s;
        }
        f.indice = r.unwrap_or((self.ultima_fila + 1).clamp(0, 1_048_575) as u32);
        self.ultima_fila = f.indice as i64;
        self.ultima_col = -1;
        f.inicio = self.tanda.celdas.len() as u32;
        self.fila = f;
        self.fila_abierta = true;
        self.max_tam_fila = 0.0;
    }

    fn cerrar_fila(&mut self) {
        if self.celda_abierta {
            self.cerrar_celda();
        }
        self.fila_abierta = false;
        let mut f = self.fila;
        f.fin = self.tanda.celdas.len() as u32;
        // Celdas desordenadas dentro de la fila (raro): se ordenan.
        let tramo = &mut self.tanda.celdas[f.inicio as usize..f.fin as usize];
        if tramo.windows(2).any(|w| w[0].col >= w[1].col) {
            tramo.sort_by_key(|c| c.col);
        }
        // Sin `ht`, Excel ajusta la fila a la letra mas grande que tenga.
        if f.alto.is_nan() && self.max_tam_fila > self.tam_normal + 0.01 {
            f.alto =
                (self.alto_normal * self.max_tam_fila / self.tam_normal * 4.0 / 3.0).round() * 0.75;
        }
        if f.fin > f.inicio || f.xf.is_some() || f.oculta || !f.alto.is_nan() {
            self.tanda.filas.push(f);
        }
    }

    /// Lee de un tiron una celda corriente: `<c ...>` con, como mucho, una
    /// formula y un `<v>`, y su `</c>`. Si la celda trae otra cosa (texto en
    /// linea, prefijos, espacios) o no esta entera en `d`, devuelve `None`
    /// sin tocar nada y la lee el camino general.
    fn celda_rapida(&mut self, d: &[u8]) -> Option<usize> {
        let gt = xml::pos_byte(d, b'>')?;
        let mut attrs = &d[3..gt];
        let vacia = attrs.last() == Some(&b'/');
        if vacia {
            attrs = &attrs[..attrs.len() - 1];
        }
        // Un `>` dentro de comillas romperia la cuenta: con comillas impares
        // no es una celda corriente.
        if attrs.iter().filter(|&&b| b == b'"' || b == b'\'').count() % 2 != 0 {
            return None;
        }
        let mut i = gt + 1;
        let mut valor: Option<&[u8]> = None;
        if !vacia {
            if d[i..].starts_with(b"<f") {
                let siguiente = *d.get(i + 2)?;
                if !(siguiente == b'>' || siguiente == b' ' || siguiente == b'/') {
                    return None;
                }
                let fin = i + xml::pos_byte(&d[i..], b'>')?;
                if d[fin - 1] == b'/' {
                    i = fin + 1;
                } else {
                    i = xml::buscar(d, fin, b"</f>")? + 4;
                }
            }
            if d[i..].starts_with(b"<v>") {
                let lt = i + 3 + xml::pos_byte(&d[i + 3..], b'<')?;
                if !d[lt..].starts_with(b"</v>") {
                    return None;
                }
                valor = Some(&d[i + 3..lt]);
                i = lt + 4;
            } else if d[i..].starts_with(b"<v/>") {
                i += 4;
            }
            if !d[i..].starts_with(b"</c>") {
                return None;
            }
            i += 4;
        }
        // Todo bien: ahora si, como el camino general.
        self.dato(
            Evento::Abre {
                nombre: b"c",
                attrs,
                vacia: false,
            },
            u32::MAX,
            usize::MAX,
        );
        if let Some(v) = valor {
            self.tiene_v = true;
            self.texto.extend_from_slice(v);
        }
        self.cerrar_celda();
        Some(i)
    }

    fn cerrar_celda(&mut self) {
        self.celda_abierta = false;
        self.dentro = Dentro::Nada;
        let tipo = &self.tipo[..self.tipo_len];
        let texto = &self.texto;
        let valor = if !self.tiene_v {
            Valor::Vacio
        } else {
            match tipo {
                b"s" => num::<u32>(texto)
                    .map(Valor::Compartido)
                    .unwrap_or(Valor::Vacio),
                b"str" | b"inlineStr" => {
                    let s = xml::desescapar(texto);
                    let s = xml::sin_escapes_ooxml(&s).into_owned();
                    self.tanda.textos.push(s);
                    Valor::Propio(self.tanda.textos.len() as u32 - 1)
                }
                b"b" => Valor::Logico(texto.trim_ascii() == b"1" || texto.trim_ascii() == b"true"),
                b"e" => {
                    let s = String::from_utf8_lossy(texto.trim_ascii());
                    Valor::Error(ERRORES.iter().position(|e| *e == s).unwrap_or(2) as u8)
                }
                b"d" => serie_de_iso(&String::from_utf8_lossy(texto))
                    .map(Valor::Numero)
                    .unwrap_or(Valor::Vacio),
                _ => num::<f64>(texto).map(Valor::Numero).unwrap_or(Valor::Vacio),
            }
        };
        let c = Celda {
            valor,
            ..self.celda
        };
        if c.valor != Valor::Vacio || c.xf != 0 {
            if let Some(&t) = self.tam_por_xf.get(c.xf as usize) {
                if c.valor != Valor::Vacio && t > self.max_tam_fila {
                    self.max_tam_fila = t;
                }
            }
            self.tanda.celdas.push(c);
        }
    }

    fn cerrar_lo_abierto(&mut self) {
        if self.fila_abierta {
            self.cerrar_fila();
        }
    }

    fn leer_cola(
        &mut self,
        resto: &[u8],
        paquete: &Paquete,
        ruta: &str,
        tema: &crate::estilos::Tema,
        paleta: &[u32],
    ) -> Cola {
        let raiz = xml::arbol(resto);
        let mut cola = Cola::default();
        let mut v = Vec::new();
        raiz.todos("mergeCell", &mut v);
        cola.combinadas = v
            .iter()
            .filter_map(|m| Rango::leer(m.attr("ref")?))
            .filter(|r| r.fila0 != r.fila1 || r.col0 != r.col1)
            .collect();
        cola.combinadas.sort_by_key(|r| (r.fila0, r.col0));
        let rels = paquete.relaciones(ruta);
        let mut v = Vec::new();
        raiz.todos("hyperlink", &mut v);
        for h in v {
            let Some(rango) = h.attr("ref").and_then(Rango::leer) else {
                continue;
            };
            let (destino, externo) =
                match h.attr("id").and_then(|id| rels.iter().find(|r| r.id == id)) {
                    Some(r) => (r.destino.clone(), r.externa),
                    None => (h.attr("location").unwrap_or_default().to_string(), false),
                };
            cola.hipervinculos.push(Hipervinculo {
                rango,
                destino,
                externo,
                ayuda: h.attr("tooltip").map(str::to_string),
            });
        }
        let mut v = Vec::new();
        raiz.todos("drawing", &mut v);
        for d in v {
            if let Some(r) = d.attr("id").and_then(|id| rels.iter().find(|r| r.id == id)) {
                cola.dibujos
                    .extend(dibujos::leer(paquete, &r.destino, tema, paleta));
            }
        }
        let mut v = Vec::new();
        raiz.todos("tablePart", &mut v);
        for d in v {
            if let Some(r) = d.attr("id").and_then(|id| rels.iter().find(|r| r.id == id)) {
                if let Some(t) = paquete
                    .leer(&r.destino)
                    .and_then(|b| crate::tablas::leer(&xml::arbol(&b)))
                {
                    cola.tablas.push(t);
                }
            }
        }
        cola.condicionales = crate::condicional::leer(&raiz, tema, paleta);
        cola.validaciones = crate::validacion::leer(&raiz);
        (cola.grupos_mini, cola.minigraficos) = crate::minigraficos::leer(&raiz, tema, paleta);
        cola.comentarios = crate::comentarios::leer(paquete, &rels);
        cola.autofiltro = raiz
            .hijo("autoFilter")
            .and_then(|a| crate::filtros::AutoFiltro::leer(a, None));
        cola
    }
}

/// `2024-03-15T10:30:00` a numero de serie (sistema 1900).
fn serie_de_iso(s: &str) -> Option<f64> {
    let s = s.trim();
    let (f, h) = s.split_once('T').unwrap_or((s, ""));
    let mut p = f.split('-');
    let y: i64 = p.next()?.parse().ok()?;
    let m: u32 = p.next()?.parse().ok()?;
    let d: u32 = p.next()?.parse().ok()?;
    let mut serie = crate::libro::serie_de_fecha(y, m, d)?;
    if !h.is_empty() {
        let h = h.trim_end_matches('Z');
        let mut q = h.split(':');
        let hh: f64 = q.next().and_then(|x| x.parse().ok()).unwrap_or(0.0);
        let mm: f64 = q.next().and_then(|x| x.parse().ok()).unwrap_or(0.0);
        let ss: f64 = q.next().and_then(|x| x.parse().ok()).unwrap_or(0.0);
        serie += (hh * 3600.0 + mm * 60.0 + ss) / 86400.0;
    }
    Some(serie)
}

fn cabecera_de(raiz: &Nodo) -> Cabecera {
    let mut c = Cabecera::default();
    let ws = raiz.hijo("worksheet").unwrap_or(raiz);
    if let Some(d) = ws.hijo("dimension").and_then(|d| d.attr("ref")) {
        c.dimension = Rango::leer(d);
    }
    if let Some(sp) = ws.hijo("sheetPr") {
        if let Some(col) = sp.hijo("tabColor") {
            c.color_pestana = crate::estilos::color_de(
                col,
                &crate::estilos::Tema::default(),
                &crate::estilos::PALETA,
            );
        }
    }
    if let Some(sv) = ws.hijo("sheetViews").and_then(|s| s.hijo("sheetView")) {
        let v = &mut c.vista;
        let no = |a: &str| sv.attr(a).is_some_and(|x| x == "0" || x == "false");
        let si = |a: &str| sv.attr(a).is_some_and(|x| x == "1" || x == "true");
        v.cuadricula = !no("showGridLines");
        v.encabezados = !no("showRowColHeaders");
        v.derecha_a_izquierda = si("rightToLeft");
        v.zoom = sv
            .attr("zoomScale")
            .and_then(|z| z.parse().ok())
            .filter(|z| (10..=400).contains(z))
            .unwrap_or(100);
        if let Some((f, col)) = sv
            .attr("topLeftCell")
            .and_then(|t| referencia(t.as_bytes()))
        {
            v.primera_fila = f;
            v.primera_columna = col;
        }
        if let Some(p) = sv.hijo("pane") {
            let estado = p.attr("state").unwrap_or("split");
            if estado == "frozen" || estado == "frozenSplit" {
                let n = |a: &str| {
                    p.attr(a)
                        .and_then(|x| x.parse::<f64>().ok())
                        .unwrap_or(0.0)
                        .max(0.0) as u32
                };
                v.columnas_fijas = n("xSplit").min(16384);
                v.filas_fijas = n("ySplit").min(1_048_576);
                if let Some((f, col)) = p.attr("topLeftCell").and_then(|t| referencia(t.as_bytes()))
                {
                    v.primera_fila = f.max(v.filas_fijas);
                    v.primera_columna = col.max(v.columnas_fijas);
                } else {
                    v.primera_fila = v.primera_fila.max(v.filas_fijas);
                    v.primera_columna = v.primera_columna.max(v.columnas_fijas);
                }
            }
        }
        if let Some(sel) = sv
            .hijos_de("selection")
            .last()
            .and_then(|s| s.attr("activeCell"))
        {
            v.celda_activa = referencia(sel.as_bytes());
        }
    }
    if let Some(f) = ws.hijo("sheetFormatPr") {
        c.ancho_defecto = f
            .attr("defaultColWidth")
            .and_then(|x| x.parse().ok())
            .filter(|x: &f64| x.is_finite() && *x >= 0.0);
        c.ancho_base = f
            .attr("baseColWidth")
            .and_then(|x| x.parse().ok())
            .filter(|x: &f64| x.is_finite() && *x >= 0.0);
        c.alto_defecto = f
            .attr("defaultRowHeight")
            .and_then(|x| x.parse().ok())
            .filter(|x: &f32| x.is_finite() && *x > 0.0 && *x < 410.0);
        c.filas_ocultas_por_defecto = f
            .attr("zeroHeight")
            .is_some_and(|x| x == "1" || x == "true");
    }
    if let Some(cols) = ws.hijo("cols") {
        for col in cols.hijos_de("col") {
            let n = |a: &str| col.attr(a).and_then(|x| x.parse::<u32>().ok());
            let (Some(min), Some(max)) = (n("min"), n("max")) else {
                continue;
            };
            if min == 0 || max < min {
                continue;
            }
            let si = |a: &str| col.attr(a).is_some_and(|x| x == "1" || x == "true");
            c.columnas.push(Columnas {
                min: (min - 1).min(16383),
                max: (max - 1).min(16383),
                ancho: col
                    .attr("width")
                    .and_then(|x| x.parse().ok())
                    .filter(|x: &f64| x.is_finite() && *x >= 0.0 && *x <= 255.0),
                oculta: si("hidden"),
                xf: n("style").unwrap_or(0),
            });
        }
        c.columnas.sort_by_key(|x| x.min);
    }
    c
}

/// Una hoja abierta. Las filas llegan poco a poco (ver [`Hoja::cargar_hasta_fila`]).
pub struct Hoja {
    pub nombre: String,
    pub filas: Vec<Fila>,
    pub celdas: Vec<Celda>,
    pub textos: Vec<String>,
    pub cabecera: Cabecera,
    pub cola: Cola,
    /// Ya se leyo entera.
    pub completa: bool,
    /// La columna mas a la derecha con algo.
    pub max_col: u32,
    lector: Option<LectorHoja>,
    desordenada: bool,
    /// Version: sube cada vez que llegan filas (para que quien cachea sepa
    /// que tiene que rehacer).
    pub version: u64,
}

impl Hoja {
    pub(crate) fn nueva(nombre: String, lector: Option<LectorHoja>) -> Hoja {
        let completa = lector.is_none();
        Hoja {
            nombre,
            filas: Vec::new(),
            celdas: Vec::new(),
            textos: Vec::new(),
            cabecera: Cabecera::default(),
            cola: Cola::default(),
            completa,
            max_col: 0,
            lector,
            desordenada: false,
            version: 0,
        }
    }

    /// Una hoja ya entera, armada fila a fila (los `.xls` antiguos, que se
    /// leen de una vez, y las pruebas). Las filas deben venir ordenadas.
    pub fn desde_filas(
        nombre: String,
        cabecera: Cabecera,
        cola: Cola,
        textos: Vec<String>,
        filas: Vec<FilaNueva>,
    ) -> Hoja {
        let mut h = Hoja::nueva(nombre, None);
        h.cabecera = cabecera;
        h.cola = cola;
        h.textos = textos;
        if let Some(d) = h.cabecera.dimension {
            h.max_col = d.col1.min(16383);
        }
        for f in filas {
            let inicio = h.celdas.len() as u32;
            for c in f.celdas {
                if c.valor != Valor::Vacio && c.col > h.max_col {
                    h.max_col = c.col;
                }
                h.celdas.push(c);
            }
            h.filas.push(Fila {
                indice: f.indice,
                alto: f.alto,
                oculta: f.oculta,
                xf: f.xf,
                sin_alto: f.sin_alto,
                inicio,
                fin: h.celdas.len() as u32,
            });
        }
        h.filas.sort_by_key(|f| f.indice);
        let mut cs = std::mem::take(&mut h.cola.condicionales);
        for c in &mut cs {
            c.calcular(&h);
        }
        h.cola.condicionales = cs;
        h.version = 1;
        h
    }

    /// Una copia de una hoja ya entera (sin lector): lo que entrega
    /// `Libro::abrir_hoja` de un `.xls`.
    pub(crate) fn copia_entera(&self) -> Hoja {
        Hoja {
            nombre: self.nombre.clone(),
            filas: self.filas.clone(),
            celdas: self.celdas.clone(),
            textos: self.textos.clone(),
            cabecera: self.cabecera.clone(),
            cola: self.cola.clone(),
            completa: true,
            max_col: self.max_col,
            lector: None,
            desordenada: false,
            version: self.version,
        }
    }
    #[cfg(test)]
    pub(crate) fn vacia_para_pruebas() -> Hoja {
        Hoja::nueva("P".into(), None)
    }

    /// Una columna de numeros en A1.. (para las pruebas).
    #[cfg(test)]
    pub(crate) fn poner_numeros_para_pruebas(&mut self, v: &[f64]) {
        let filas = v
            .iter()
            .enumerate()
            .map(|(i, &x)| FilaNueva {
                indice: i as u32,
                alto: f32::NAN,
                oculta: false,
                xf: None,
                sin_alto: false,
                celdas: vec![Celda {
                    col: 0,
                    xf: 0,
                    valor: Valor::Numero(x),
                }],
            })
            .collect();
        *self = Hoja::desde_filas(
            self.nombre.clone(),
            Cabecera::default(),
            Cola::default(),
            Vec::new(),
            filas,
        );
    }

    /// La vista guardada (paneles, cuadricula, zoom).
    pub fn vista(&self) -> &VistaHoja {
        &self.cabecera.vista
    }

    /// Mete una tanda leida (de este hilo o de otro).
    pub fn recibir(&mut self, t: Tanda) {
        let base_celda = self.celdas.len() as u32;
        let base_texto = self.textos.len() as u32;
        if let Some(c) = t.cabecera {
            if let Some(d) = c.dimension {
                self.max_col = self.max_col.max(d.col1.min(16383));
            }
            self.cabecera = c;
        }
        self.textos.extend(t.textos);
        let ultima = self.filas.last().map(|f| f.indice);
        for (k, mut f) in t.filas.into_iter().enumerate() {
            f.inicio += base_celda;
            f.fin += base_celda;
            let previa = if k == 0 { ultima } else { None };
            if previa.is_some_and(|u| f.indice <= u)
                || self.filas.last().is_some_and(|u| f.indice <= u.indice)
            {
                self.desordenada = true;
            }
            self.filas.push(f);
        }
        for mut c in t.celdas {
            if let Valor::Propio(i) = c.valor {
                c.valor = Valor::Propio(i + base_texto);
            }
            if c.valor != Valor::Vacio && c.col > self.max_col {
                self.max_col = c.col;
            }
            self.celdas.push(c);
        }
        if let Some(c) = t.cola {
            self.cola = c;
        }
        if t.fin {
            self.completa = true;
            self.lector = None;
            if self.desordenada {
                // Filas repetidas o desordenadas: se ordenan y la ultima gana.
                self.filas.sort_by_key(|f| f.indice);
                self.filas.dedup_by(|b, a| a.indice == b.indice);
                self.desordenada = false;
            }
            // Las cuentas del formato condicional, ya con todas las celdas.
            let mut cs = std::mem::take(&mut self.cola.condicionales);
            for c in &mut cs {
                c.calcular(self);
            }
            self.cola.condicionales = cs;
        }
        self.version += 1;
    }

    /// Lee hasta tener la fila `fila` (base 0), o hasta el final. Devuelve si
    /// la hoja ya esta completa.
    pub fn cargar_hasta_fila(&mut self, fila: u32) -> bool {
        while !self.completa && self.filas.last().is_none_or(|f| f.indice < fila) {
            let Some(l) = self.lector.as_mut() else {
                self.completa = true;
                break;
            };
            let t = l.leer(fila, usize::MAX);
            self.recibir(t);
        }
        self.completa
    }

    /// Lee una tanda de unas `celdas` celdas (para repartir la carga entre
    /// fotogramas). Devuelve si ya esta completa.
    pub fn cargar_tanda(&mut self, celdas: usize) -> bool {
        if let Some(l) = self.lector.as_mut() {
            let t = l.leer(u32::MAX, celdas.max(1));
            self.recibir(t);
        } else {
            self.completa = true;
        }
        self.completa
    }

    pub fn cargar_todo(&mut self) {
        while !self.cargar_tanda(1 << 20) {}
    }

    /// Se lleva el lector (para leer el resto en otro hilo y mandar las
    /// tandas con [`Hoja::recibir`]).
    pub fn tomar_lector(&mut self) -> Option<LectorHoja> {
        self.lector.take()
    }

    /// Devuelve un lector tomado con [`Hoja::tomar_lector`] (si no termino).
    pub fn devolver_lector(&mut self, l: LectorHoja) {
        if !l.terminado() && !self.completa {
            self.lector = Some(l);
        }
    }

    fn pos_fila(&self, fila: u32) -> Option<usize> {
        if self.desordenada {
            return self.filas.iter().rposition(|f| f.indice == fila);
        }
        self.filas.binary_search_by_key(&fila, |f| f.indice).ok()
    }

    pub fn fila(&self, fila: u32) -> Option<&Fila> {
        self.pos_fila(fila).map(|i| &self.filas[i])
    }

    /// Las celdas de una fila (ordenadas por columna).
    pub fn celdas_de(&self, f: &Fila) -> &[Celda] {
        self.celdas
            .get(f.inicio as usize..f.fin as usize)
            .unwrap_or(&[])
    }

    /// El comentario de la celda, si lo tiene.
    pub fn comentario_en(&self, fila: u32, col: u32) -> Option<&crate::comentarios::Comentario> {
        let i = self
            .cola
            .comentarios
            .binary_search_by_key(&(fila, col), |c| (c.fila, c.col))
            .ok()?;
        self.cola.comentarios.get(i)
    }

    /// La regla de validacion de lista (con flecha) que afecta a la celda.
    pub fn lista_en(&self, fila: u32, col: u32) -> Option<&crate::validacion::Validacion> {
        self.cola.validaciones.iter().find(|v| {
            v.tipo == crate::validacion::TipoValidacion::Lista && v.flecha && v.afecta(fila, col)
        })
    }

    pub fn celda(&self, fila: u32, col: u32) -> Option<&Celda> {
        let f = self.fila(fila)?;
        let cs = self.celdas_de(f);
        cs.binary_search_by_key(&col, |c| c.col)
            .ok()
            .map(|i| &cs[i])
    }

    /// Las filas con indice en `desde..=hasta`.
    pub fn filas_entre(&self, desde: u32, hasta: u32) -> &[Fila] {
        if self.desordenada {
            return &[];
        }
        let a = self.filas.partition_point(|f| f.indice < desde);
        let b = self.filas.partition_point(|f| f.indice <= hasta);
        &self.filas[a..b.max(a)]
    }

    /// El estilo de una celda aunque no exista: el suyo, el de su fila o el
    /// de su columna.
    pub fn xf_en(&self, fila: u32, col: u32) -> u32 {
        if let Some(f) = self.fila(fila) {
            let cs = self.celdas_de(f);
            if let Ok(i) = cs.binary_search_by_key(&col, |c| c.col) {
                return cs[i].xf;
            }
            if let Some(x) = f.xf {
                return x;
            }
        }
        self.xf_columna(col)
    }

    pub fn xf_columna(&self, col: u32) -> u32 {
        self.cabecera
            .columnas
            .iter()
            .find(|c| col >= c.min && col <= c.max)
            .map_or(0, |c| c.xf)
    }

    /// La combinacion que contiene la celda, si la hay.
    pub fn combinada_en(&self, fila: u32, col: u32) -> Option<Rango> {
        let k = self.cola.combinadas.partition_point(|r| r.fila0 <= fila);
        self.cola.combinadas[..k]
            .iter()
            .rev()
            .find(|r| r.contiene(fila, col))
            .copied()
    }

    /// Las combinaciones que tocan las filas `f0..=f1` y columnas `c0..=c1`.
    pub fn combinadas_en(
        &self,
        f0: u32,
        f1: u32,
        c0: u32,
        c1: u32,
    ) -> impl Iterator<Item = &Rango> {
        let k = self.cola.combinadas.partition_point(|r| r.fila0 <= f1);
        self.cola.combinadas[..k]
            .iter()
            .filter(move |r| r.fila1 >= f0 && r.col1 >= c0 && r.col0 <= c1)
    }

    pub fn hipervinculo_en(&self, fila: u32, col: u32) -> Option<&Hipervinculo> {
        self.cola
            .hipervinculos
            .iter()
            .find(|h| h.rango.contiene(fila, col))
    }

    /// La ultima fila con algo (base 0).
    pub fn max_fila(&self) -> u32 {
        self.filas
            .iter()
            .rev()
            .take(64)
            .map(|f| f.indice)
            .max()
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn referencias_y_nombres() {
        assert_eq!(referencia(b"A1"), Some((0, 0)));
        assert_eq!(referencia(b"$AB$12"), Some((11, 27)));
        assert_eq!(referencia(b"XFD1048576"), Some((1_048_575, 16383)));
        assert_eq!(referencia(b"XFE1"), None);
        assert_eq!(referencia(b"A0"), None);
        assert_eq!(referencia(b"A99999999999"), None);
        assert_eq!(nombre_columna(0), "A");
        assert_eq!(nombre_columna(25), "Z");
        assert_eq!(nombre_columna(26), "AA");
        assert_eq!(nombre_columna(16383), "XFD");
        assert_eq!(
            Rango::leer("C3:A1"),
            Some(Rango {
                fila0: 0,
                col0: 0,
                fila1: 2,
                col1: 2
            })
        );
    }

    #[test]
    fn fechas_iso() {
        assert_eq!(serie_de_iso("2023-03-15"), Some(45000.0));
        assert_eq!(serie_de_iso("2023-03-15T18:00:00Z"), Some(45000.75));
        assert_eq!(serie_de_iso("basura"), None);
    }
}
