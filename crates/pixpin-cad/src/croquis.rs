//! **El croquis 3D del movil, para verlo aqui** (tanda 5, 13-oct-2026).
//!
//! PixPin Android dibuja en el espacio (`croquis3d/`): trazos con su punta y
//! su pulso, hojas que sirven de mesa, cuerpos (bola, cilindro, cono, anillo),
//! imagenes puestas en el espacio, el sol y la luna, y vistas guardadas. Lo
//! guarda en `croquis3d/<id>.croquis.gz` (JSON en gzip, `Croquis3DAlmacen.kt`)
//! y dentro de un `.pixpin` como `croquis/<id>.json`. La sincronizacion ya lo
//! traia al PC, pero aqui no se sabia abrir y pinearlo daba un aviso.
//!
//! Este modulo **solo lee**: el formato tal cual lo escribe Kotlin
//! (`Croquis3D.kt`, con los mismos valores por defecto para que un croquis
//! viejo abra igual que alli), la camara del movil (`Camara3D.kt`, para las
//! vistas guardadas y sus pruebas) y el pulso del trazo (`PulsoDelMundo.kt`).
//! De ahi a triangulos lo hace [`malla`], y lo ensena la ventana del visor 3D
//! ([`crate::ventana3d`]), que ya sabe girar, mover, acercar y cortar. Nada
//! de lo que hay aqui escribe en el fichero: editar el croquis (C2) necesita
//! antes la fusion trazo a trazo en `pixpin-sincro`.

pub mod malla;
#[cfg(test)]
mod pruebas;

use std::f64::consts::PI;
use std::io::Read;

use serde::Deserialize;

// ---- El formato (Croquis3D.kt) ----

/// Un punto del mundo. Z es arriba, como en el movil.
#[derive(Debug, Clone, Copy, PartialEq, Default, Deserialize)]
pub struct Pt3 {
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default)]
    pub z: f64,
}

/// Un punto de la pantalla, en `dp` del movil.
#[derive(Debug, Clone, Copy, PartialEq, Default, Deserialize)]
pub struct Pt {
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
}

pub(crate) fn p3(x: f64, y: f64, z: f64) -> Pt3 {
    Pt3 { x, y, z }
}
pub(crate) fn mas(a: Pt3, b: Pt3) -> Pt3 {
    p3(a.x + b.x, a.y + b.y, a.z + b.z)
}
pub(crate) fn menos(a: Pt3, b: Pt3) -> Pt3 {
    p3(a.x - b.x, a.y - b.y, a.z - b.z)
}
pub(crate) fn por(a: Pt3, k: f64) -> Pt3 {
    p3(a.x * k, a.y * k, a.z * k)
}
pub(crate) fn escalar(a: Pt3, b: Pt3) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z
}
pub(crate) fn producto(a: Pt3, b: Pt3) -> Pt3 {
    p3(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}
pub(crate) fn largo(a: Pt3) -> f64 {
    escalar(a, a).sqrt()
}
/// Unitario; el cero se queda en cero (como `normalizado` de Kotlin).
pub(crate) fn normalizado(a: Pt3) -> Pt3 {
    let l = largo(a);
    if l < 1e-12 { a } else { por(a, 1.0 / l) }
}

fn verdad() -> bool {
    true
}
fn uno() -> f64 {
    1.0
}

/// **El croquis entero**, con los campos que hacen falta para verlo. Lo que
/// no se usa aqui (las tintas guardadas, los favoritos, el espejo, el
/// enfoque...) se ignora al leer, igual que `ignoreUnknownKeys` de Kotlin.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Croquis {
    #[serde(default)]
    pub trazos: Vec<Trazo3D>,
    #[serde(default)]
    pub laminas: Vec<Lamina3D>,
    /// El color del espacio; nulo es el del tema.
    #[serde(default)]
    pub color_del_fondo: Option<String>,
    #[serde(default)]
    pub grupos: Vec<Grupo3D>,
    #[serde(default)]
    pub imagenes: Vec<Imagen3D>,
    #[serde(default)]
    pub vistas: Vec<Vista3D>,
    #[serde(default)]
    pub sol: Option<Sol3D>,
    #[serde(default)]
    pub luna: Option<Sol3D>,
    #[serde(default)]
    pub luces: Luces3D,
    /// La reticula de las hojas y del suelo (de fabrica, puesta).
    #[serde(default = "verdad")]
    pub pauta_de_las_hojas: bool,
    /// Los modelos IFC/OBJ metidos en el croquis: sus triangulos van en otro
    /// fichero del movil. Aqui solo se cuentan (ver el informe de la tanda).
    #[serde(default)]
    pub modelos: Vec<serde_json::Value>,
}

impl Default for Croquis {
    fn default() -> Self {
        serde_json::from_str("{}").expect("un croquis vacio se lee")
    }
}

impl Croquis {
    /// `Croquis.vacio` de Kotlin.
    pub fn vacio(&self) -> bool {
        self.trazos.is_empty()
            && self.laminas.is_empty()
            && self.imagenes.is_empty()
            && self.modelos.is_empty()
    }

    /// `Croquis.puntos()`: lo que se ve, para encuadrar. Sin las hojas
    /// mientras haya algo dibujado (una hoja es un plano, no tiene tamano).
    pub fn puntos(&self) -> Vec<Pt3> {
        let mut v: Vec<Pt3> = self
            .trazos
            .iter()
            .filter(|t| !t.oculto)
            .flat_map(|t| t.puntos.iter().copied())
            .collect();
        v.extend(
            self.imagenes
                .iter()
                .filter(|i| !i.oculto)
                .flat_map(|i| i.esquinas.iter().copied()),
        );
        if v.is_empty() {
            v = self.laminas.iter().flat_map(|l| l.esquinas()).collect();
        }
        v
    }

    /// El nombre del grupo de algo, si lo tiene.
    pub fn nombre_de_grupo(&self, grupo: Option<&str>) -> Option<&str> {
        let g = grupo?;
        self.grupos
            .iter()
            .find(|x| x.id == g)
            .map(|x| x.nombre.as_str())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Grupo3D {
    pub id: String,
    pub nombre: String,
}

/// Con que seccion se pinto un trazo. `PLANO` y `LAPIZ` son puntas
/// retiradas: abren como la redonda (`Pincel.vigente`). Una que no se
/// conozca tambien.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Pincel {
    #[default]
    Redondo,
    Cuadrado,
    Plano,
    Lapiz,
    Resaltador,
    Luz,
    Cuchilla,
    #[serde(other)]
    Otro,
}

impl Pincel {
    /// `Pincel.vigente`: las retiradas son la redonda.
    pub fn vigente(self) -> Pincel {
        match self {
            Pincel::Plano | Pincel::Lapiz | Pincel::Otro => Pincel::Redondo,
            p => p,
        }
    }

    /// `Pincel.comoSeLlama`.
    pub fn como_se_llama(self) -> &'static str {
        match self.vigente() {
            Pincel::Cuadrado => "listón",
            Pincel::Resaltador => "rodillo",
            Pincel::Luz => "luz",
            Pincel::Cuchilla => "cuchilla",
            _ => "rotulador",
        }
    }
}

/// `PuntaDelPincel`: solo lo que cambia la forma al verlo.
#[derive(Debug, Clone, Deserialize)]
pub struct PuntaDelPincel {
    #[serde(default = "uno")]
    pub ancho: f64,
    #[serde(default = "uno")]
    pub largo: f64,
    #[serde(default = "verdad")]
    pub pulso: bool,
    #[serde(default)]
    pub alumbra: bool,
    /// El lapiz: una raya lisa, sin bulto ni pulso.
    #[serde(default)]
    pub plana: bool,
}

impl Default for PuntaDelPincel {
    fn default() -> Self {
        PuntaDelPincel {
            ancho: 1.0,
            largo: 1.0,
            pulso: true,
            alumbra: false,
            plana: false,
        }
    }
}

/// Un trazo del lapiz (`Trazo3D`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trazo3D {
    pub id: String,
    pub puntos: Vec<Pt3>,
    pub color: String,
    /// El numero del selector, en `dp`.
    pub grosor: f64,
    #[serde(default)]
    pub pincel: Pincel,
    /// Lo que mide de gordo en el mundo (nulo en los croquis viejos: entonces
    /// se dibujaron a lo ancho de la pantalla, que a aumento uno es lo mismo).
    #[serde(default)]
    pub calibre: Option<f64>,
    #[serde(default)]
    pub presiones: Option<Vec<f64>>,
    #[serde(default)]
    pub tiempos: Option<Vec<f64>>,
    #[serde(default = "uno")]
    pub opacidad: f64,
    /// La normal de la hoja sobre la que se trazo (la necesita el rodillo).
    #[serde(default)]
    pub normal: Option<Pt3>,
    #[serde(default)]
    pub normales: Option<Vec<Pt3>>,
    #[serde(default)]
    pub oculto: bool,
    #[serde(default = "uno")]
    pub luz: f64,
    #[serde(default)]
    pub color_apagada: Option<String>,
    #[serde(default)]
    pub grupo: Option<String>,
    #[serde(default)]
    pub punta: PuntaDelPincel,
    #[serde(default)]
    pub eje_de_la_punta: Option<Pt3>,
}

impl Trazo3D {
    /// Lo gordo en el mundo: el calibre, o el grosor en los croquis viejos.
    pub fn ancho_del_mundo(&self) -> f64 {
        self.calibre.unwrap_or(self.grosor).max(1e-6)
    }

    /// Si se pinta encendido (la tinta de luz o una punta que alumbra).
    pub fn alumbra(&self) -> bool {
        self.pincel == Pincel::Luz || self.punta.alumbra
    }
}

/// `Luces3D`: la llave de paso de lo que alumbra.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Luces3D {
    #[serde(default = "verdad")]
    pub encendidas: bool,
    #[serde(default = "uno")]
    pub fuerza: f64,
}

impl Default for Luces3D {
    fn default() -> Self {
        Luces3D {
            encendidas: true,
            fuerza: 1.0,
        }
    }
}

impl Luces3D {
    /// Lo que multiplica a la luz de cada trazo: cero es apagado.
    pub fn cuanto(&self) -> f64 {
        if self.encendidas {
            self.fuerza.clamp(0.0, 2.0)
        } else {
            0.0
        }
    }
}

fn azimut_de_fabrica() -> f64 {
    -PI / 4.0
}
fn altura_de_fabrica() -> f64 {
    PI / 4.0
}

/// `Sol3D`: de donde viene la luz, para que lo dibujado tire sombra.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Sol3D {
    #[serde(default = "azimut_de_fabrica")]
    pub azimut: f64,
    #[serde(default = "altura_de_fabrica")]
    pub altura: f64,
}

impl Default for Sol3D {
    fn default() -> Self {
        Sol3D {
            azimut: azimut_de_fabrica(),
            altura: altura_de_fabrica(),
        }
    }
}

/// Lo mas rasante que se deja el sol: por debajo, la sombra se va al infinito.
const ALTURA_MINIMA: f64 = 0.08;

impl Sol3D {
    /// Hacia donde viajan los rayos: del sol al suelo, bajando.
    pub fn rayo(&self) -> Pt3 {
        let alta = self.altura.clamp(ALTURA_MINIMA, PI / 2.0);
        normalizado(p3(
            self.azimut.sin() * alta.cos(),
            self.azimut.cos() * alta.cos(),
            -alta.sin(),
        ))
    }

    /// Donde cae la sombra de un punto en el suelo (`z = 0`), o nada si el
    /// punto esta por debajo (eso seria un reflejo).
    pub fn sombra_de(&self, p: Pt3) -> Option<Pt3> {
        let d = self.rayo();
        if d.z >= -1e-6 || p.z < 0.0 {
            return None;
        }
        let cuanto = -p.z / d.z;
        Some(p3(p.x + d.x * cuanto, p.y + d.y * cuanto, 0.0))
    }
}

/// Una vista congelada: la camara entera y, si se marco, el trozo de pantalla.
#[derive(Debug, Clone, Deserialize)]
pub struct Vista3D {
    pub id: String,
    pub nombre: String,
    pub camara: Camara3D,
    #[serde(default)]
    pub recorte: Option<Recorte>,
}

/// Un rectangulo de la pantalla, en tanto por uno.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Recorte {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Recorte {
    pub fn ancho(&self) -> f64 {
        (self.x1 - self.x0).abs()
    }
    pub fn alto(&self) -> f64 {
        (self.y1 - self.y0).abs()
    }
    /// Un recorte de dos pixeles es un toque, no un encuadre.
    pub fn sirve(&self) -> bool {
        self.ancho() > 0.05 && self.alto() > 0.05
    }
}

/// Una imagen puesta en el espacio, por sus cuatro esquinas (arriba a la
/// izquierda, arriba a la derecha, abajo a la derecha, abajo a la izquierda).
#[derive(Debug, Clone, Deserialize)]
pub struct Imagen3D {
    pub id: String,
    /// `pixpin:files/…` una vez sincronizada; la resuelve quien abre.
    pub ruta: String,
    pub esquinas: Vec<Pt3>,
    #[serde(default = "uno")]
    pub opacidad: f64,
    #[serde(default)]
    pub oculto: bool,
    #[serde(default)]
    pub grupo: Option<String>,
}

/// Una hoja (`Lamina3D`): un perfil barrido hacia lo hondo, una hoja reglada
/// entre dos perfiles o un cuerpo ([`Esfera3D`]).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lamina3D {
    pub id: String,
    pub perfil: Vec<Pt3>,
    pub direccion: Pt3,
    /// Cuanto barre hacia cada lado.
    pub fondo: f64,
    pub color: String,
    pub grosor: f64,
    #[serde(default)]
    pub direcciones: Option<Vec<Pt3>>,
    #[serde(default)]
    pub infinita: bool,
    #[serde(default = "uno")]
    pub opacidad: f64,
    #[serde(default)]
    pub otro_perfil: Option<Vec<Pt3>>,
    #[serde(default)]
    pub esfera: Option<Esfera3D>,
}

impl Lamina3D {
    fn desplazamiento(&self, i: usize, fondo: f64) -> Pt3 {
        let d = self
            .direcciones
            .as_ref()
            .and_then(|v| v.get(i).copied())
            .unwrap_or(self.direccion);
        por(normalizado(d), fondo)
    }

    /// Un lado y el otro del punto `i` del perfil (`unLado`, `otroLado`).
    /// `fondo` deja estirar las hojas que no se acaban a lo hondo.
    pub fn lados(&self, i: usize, fondo: f64) -> (Pt3, Pt3) {
        if let Some(otro) = &self.otro_perfil {
            return (
                self.perfil[i],
                otro.get(i).copied().unwrap_or(self.perfil[i]),
            );
        }
        let d = self.desplazamiento(i, fondo);
        (menos(self.perfil[i], d), mas(self.perfil[i], d))
    }

    /// Las tiras (cuadrilateros) de la hoja: una por tramo del perfil, o la
    /// malla del cuerpo.
    pub fn tiras(&self) -> Vec<[Pt3; 4]> {
        self.tiras_con(self.fondo)
    }

    pub fn tiras_con(&self, fondo: f64) -> Vec<[Pt3; 4]> {
        if let Some(e) = &self.esfera {
            return e.malla();
        }
        if self.perfil.len() < 2 {
            return Vec::new();
        }
        (0..self.perfil.len() - 1)
            .map(|i| {
                let (a0, b0) = self.lados(i, fondo);
                let (a1, b1) = self.lados(i + 1, fondo);
                [a0, a1, b1, b0]
            })
            .collect()
    }

    pub fn esquinas(&self) -> Vec<Pt3> {
        if let Some(e) = &self.esfera {
            return e.caja();
        }
        (0..self.perfil.len())
            .flat_map(|i| {
                let (a, b) = self.lados(i, self.fondo);
                [a, b]
            })
            .collect()
    }

    /// El contorno cerrado de la hoja (vacio en un cuerpo).
    pub fn contorno(&self, fondo: f64) -> Vec<Pt3> {
        if self.esfera.is_some() || self.perfil.len() < 2 {
            return Vec::new();
        }
        let n = self.perfil.len();
        let mut v: Vec<Pt3> = (0..n).map(|i| self.lados(i, fondo).0).collect();
        v.extend((0..n).rev().map(|i| self.lados(i, fondo).1));
        v
    }
}

/// `FormaDeBola`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FormaDeBola {
    #[default]
    Bola,
    Cilindro,
    Cono,
    Anillo,
    #[serde(other)]
    Otra,
}

fn escala_de_fabrica() -> Pt3 {
    p3(1.0, 1.0, 1.0)
}

/// Un cuerpo: bola, cilindro, cono o anillo, con su escala y sus ejes.
#[derive(Debug, Clone, Deserialize)]
pub struct Esfera3D {
    pub centro: Pt3,
    pub radio: f64,
    #[serde(default = "escala_de_fabrica")]
    pub escala: Pt3,
    #[serde(default)]
    pub ejes: Option<Vec<Pt3>>,
    #[serde(default)]
    pub forma: FormaDeBola,
}

pub const MERIDIANOS: usize = 24;
pub const PARALELOS: usize = 12;
const RADIO_DEL_ANILLO: f64 = 0.72;
const GROSOR_DEL_ANILLO: f64 = 0.28;

impl Esfera3D {
    fn eje(&self, k: usize, defecto: Pt3) -> Pt3 {
        self.ejes
            .as_ref()
            .and_then(|e| e.get(k).copied())
            .unwrap_or(defecto)
    }

    fn en_el_mundo(&self, a: f64, b: f64, c: f64) -> Pt3 {
        let (u, v, w) = (
            self.eje(0, p3(1.0, 0.0, 0.0)),
            self.eje(1, p3(0.0, 1.0, 0.0)),
            self.eje(2, p3(0.0, 0.0, 1.0)),
        );
        p3(
            self.centro.x + u.x * a + v.x * b + w.x * c,
            self.centro.y + u.y * a + v.y * b + w.y * c,
            self.centro.z + u.z * a + v.z * b + w.z * c,
        )
    }

    /// `Esfera3D.punto`: `vuelta` alrededor del eje y `subida` de −π/2 a π/2.
    pub fn punto(&self, vuelta: f64, subida: f64) -> Pt3 {
        let (cv, sv) = (vuelta.cos(), vuelta.sin());
        let altura = (subida / (PI / 2.0)).clamp(-1.0, 1.0);
        let (pu, po, pt) = (
            self.radio * self.escala.x,
            self.radio * self.escala.y,
            self.radio * self.escala.z,
        );
        let (a, b, c) = match self.forma {
            FormaDeBola::Cilindro => (pu * cv, po * sv, pt * altura),
            FormaDeBola::Cono => {
                let k = (1.0 - altura) / 2.0;
                (pu * k * cv, po * k * sv, pt * altura)
            }
            FormaDeBola::Anillo => {
                let theta = (subida + PI / 2.0) * 2.0;
                let r = RADIO_DEL_ANILLO + GROSOR_DEL_ANILLO * theta.cos();
                (
                    pu * r * cv,
                    po * r * sv,
                    pt * GROSOR_DEL_ANILLO * theta.sin(),
                )
            }
            FormaDeBola::Bola | FormaDeBola::Otra => {
                let cs = subida.cos();
                (pu * cs * cv, po * cs * sv, pt * subida.sin())
            }
        };
        self.en_el_mundo(a, b, c)
    }

    /// La malla: `MERIDIANOS × PARALELOS` cuadrilateros.
    pub fn malla(&self) -> Vec<[Pt3; 4]> {
        let mut v = Vec::with_capacity(MERIDIANOS * PARALELOS);
        for j in 0..PARALELOS {
            let s0 = -PI / 2.0 + PI * j as f64 / PARALELOS as f64;
            let s1 = -PI / 2.0 + PI * (j + 1) as f64 / PARALELOS as f64;
            for i in 0..MERIDIANOS {
                let v0 = 2.0 * PI * i as f64 / MERIDIANOS as f64;
                let v1 = 2.0 * PI * (i + 1) as f64 / MERIDIANOS as f64;
                v.push([
                    self.punto(v0, s0),
                    self.punto(v1, s0),
                    self.punto(v1, s1),
                    self.punto(v0, s1),
                ]);
            }
        }
        v
    }

    pub fn caja(&self) -> Vec<Pt3> {
        let (pu, po, pt) = (
            self.radio * self.escala.x,
            self.radio * self.escala.y,
            self.radio * self.escala.z,
        );
        let mut v = Vec::with_capacity(8);
        for x in [-1.0, 1.0] {
            for y in [-1.0, 1.0] {
                for z in [-1.0, 1.0] {
                    v.push(self.en_el_mundo(pu * x, po * y, pt * z));
                }
            }
        }
        v
    }
}

// ---- Leer ----

/// **Lee un croquis**: el `.croquis.gz` del movil o el JSON de dentro de un
/// `.pixpin`. Solo lee: no toca el fichero de donde salen los bytes.
pub fn leer(bytes: &[u8]) -> Result<Croquis, String> {
    let texto = if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut s = String::new();
        flate2::read::GzDecoder::new(bytes)
            .read_to_string(&mut s)
            .map_err(|e| format!("gzip: {e}"))?;
        s
    } else {
        String::from_utf8(bytes.to_vec()).map_err(|_| "no es texto".to_string())?
    };
    serde_json::from_str(&texto).map_err(|e| format!("JSON: {e}"))
}

/// `parseColor` de Kotlin: `#RRGGBB` o `#AARRGGBB` a RGBA en 0..1.
pub fn color_de(texto: &str) -> Option<[f32; 4]> {
    let h = texto.trim().strip_prefix('#')?;
    let v = u32::from_str_radix(h, 16).ok()?;
    let c = |s: u32| ((v >> s) & 0xff) as f32 / 255.0;
    match h.len() {
        6 => Some([c(16), c(8), c(0), 1.0]),
        8 => Some([c(16), c(8), c(0), c(24)]),
        _ => None,
    }
}

/// La luminancia (la de Compose: lineal, con los pesos de Rec. 709).
pub fn luminancia(c: [f32; 4]) -> f32 {
    let lin = |v: f32| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

// ---- La camara del movil (Camara3D.kt) ----

fn giro_de_fabrica() -> f64 {
    -PI / 5.0
}
fn inclinacion_de_fabrica() -> f64 {
    PI / 7.0
}

/// La camara del movil: ortografica de fabrica, con lente si se abre.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Camara3D {
    #[serde(default = "giro_de_fabrica")]
    pub giro: f64,
    #[serde(default = "inclinacion_de_fabrica")]
    pub inclinacion: f64,
    #[serde(default = "uno")]
    pub zoom: f64,
    #[serde(default)]
    pub centro: Pt3,
    #[serde(default)]
    pub balanceo: f64,
    #[serde(default)]
    pub lente: f64,
    #[serde(default)]
    pub rectilinea: bool,
}

impl Default for Camara3D {
    fn default() -> Self {
        Camara3D {
            giro: giro_de_fabrica(),
            inclinacion: inclinacion_de_fabrica(),
            zoom: 1.0,
            centro: Pt3::default(),
            balanceo: 0.0,
            lente: 0.0,
            rectilinea: false,
        }
    }
}

pub const TOPE_DE_INCLINACION: f64 = PI / 2.0 - 0.02;
pub const ZOOM_MINIMO: f64 = 0.05;
pub const ZOOM_MAXIMO: f64 = 40.0;
const CAMPO_MAXIMO: f64 = 160.0 * PI / 180.0;
const TOPE_RECTILINEO: f64 = 80.0 * PI / 180.0;
const RETIRADA: f64 = 100000.0;

/// Un rayo: origen y direccion unitaria.
#[derive(Debug, Clone, Copy)]
pub struct Rayo {
    pub origen: Pt3,
    pub direccion: Pt3,
}

/// Un plano por un punto con su normal.
#[derive(Debug, Clone, Copy)]
pub struct Plano3D {
    pub punto: Pt3,
    pub normal: Pt3,
}

impl Plano3D {
    /// Donde lo corta un rayo y a que distancia por el rayo.
    pub fn corte(&self, r: &Rayo) -> Option<(Pt3, f64)> {
        let den = escalar(r.direccion, self.normal);
        if den.abs() < 1e-12 {
            return None;
        }
        let t = escalar(menos(self.punto, r.origen), self.normal) / den;
        Some((mas(r.origen, por(r.direccion, t)), t))
    }
}

impl Camara3D {
    pub fn campo(&self) -> f64 {
        self.lente.clamp(0.0, 1.0) * CAMPO_MAXIMO
    }

    fn focal(&self, alto: f64) -> f64 {
        if self.rectilinea {
            (alto / 2.0) / (self.campo() / 2.0).tan()
        } else {
            alto / self.campo()
        }
    }

    fn distancia_del_ojo(&self, alto: f64) -> f64 {
        self.focal(alto) / self.zoom
    }

    pub fn ojo(&self, alto: f64) -> Option<Pt3> {
        if self.campo() <= 0.0 {
            None
        } else {
            Some(menos(
                self.centro,
                por(self.adelante(), self.distancia_del_ojo(alto)),
            ))
        }
    }

    pub fn adelante(&self) -> Pt3 {
        let ci = self.inclinacion.cos();
        normalizado(p3(
            self.giro.sin() * ci,
            self.giro.cos() * ci,
            -self.inclinacion.sin(),
        ))
    }

    pub fn derecha(&self) -> Pt3 {
        let horizontal = normalizado(p3(self.giro.cos(), -self.giro.sin(), 0.0));
        if self.balanceo == 0.0 {
            return horizontal;
        }
        let vertical = normalizado(producto(horizontal, self.adelante()));
        normalizado(mas(
            por(horizontal, self.balanceo.cos()),
            por(vertical, self.balanceo.sin()),
        ))
    }

    pub fn arriba(&self) -> Pt3 {
        normalizado(producto(self.derecha(), self.adelante()))
    }

    fn angulo_proyectado(angulo: f64, rectilinea: bool) -> f64 {
        if rectilinea {
            angulo.min(TOPE_RECTILINEO).tan()
        } else {
            angulo
        }
    }

    /// `aPantalla`: del mundo a la pantalla (en `dp`), con o sin lente.
    pub fn a_pantalla(&self, p: Pt3, ancho: f64, alto: f64) -> Pt {
        let d = menos(p, self.centro);
        let (der, arr, ade) = (self.derecha(), self.arriba(), self.adelante());
        let u = escalar(d, der);
        let v = escalar(d, arr);
        if self.campo() <= 0.0 {
            return Pt {
                x: ancho / 2.0 + u * self.zoom,
                y: alto / 2.0 - v * self.zoom,
            };
        }
        let radio = u.hypot(v);
        let hondo = escalar(d, ade) + self.distancia_del_ojo(alto);
        if radio < 1e-9 {
            if hondo >= 0.0 {
                return Pt {
                    x: ancho / 2.0,
                    y: alto / 2.0,
                };
            }
            return Pt {
                x: ancho / 2.0,
                y: alto / 2.0 + self.focal(alto) * PI,
            };
        }
        let en_pantalla =
            self.focal(alto) * Self::angulo_proyectado(radio.atan2(hondo), self.rectilinea);
        Pt {
            x: ancho / 2.0 + en_pantalla * u / radio,
            y: alto / 2.0 - en_pantalla * v / radio,
        }
    }

    /// `rayo`: el que sale por un punto de la pantalla.
    pub fn rayo(&self, p: Pt, ancho: f64, alto: f64) -> Rayo {
        let (der, arr, ade) = (self.derecha(), self.arriba(), self.adelante());
        if self.campo() > 0.0 {
            let f = self.focal(alto);
            let dx = p.x - ancho / 2.0;
            let dy = alto / 2.0 - p.y;
            let radio = dx.hypot(dy);
            let ojo = self.ojo(alto).unwrap_or(self.centro);
            if radio < 1e-9 {
                return Rayo {
                    origen: ojo,
                    direccion: ade,
                };
            }
            let angulo = if self.rectilinea {
                (radio / f).atan()
            } else {
                radio / f
            };
            let lado = mas(por(der, dx / radio), por(arr, dy / radio));
            let direccion = normalizado(mas(por(ade, angulo.cos()), por(lado, angulo.sin())));
            return Rayo {
                origen: ojo,
                direccion,
            };
        }
        let dx = (p.x - ancho / 2.0) / self.zoom;
        let dy = (alto / 2.0 - p.y) / self.zoom;
        let origen = mas(self.centro, mas(por(der, dx), por(arr, dy)));
        Rayo {
            origen: mas(origen, por(ade, -RETIRADA)),
            direccion: ade,
        }
    }

    pub fn girada(&self, d_giro: f64, d_inclinacion: f64) -> Camara3D {
        Camara3D {
            giro: self.giro + d_giro,
            inclinacion: (self.inclinacion + d_inclinacion)
                .clamp(-TOPE_DE_INCLINACION, TOPE_DE_INCLINACION),
            ..*self
        }
    }

    pub fn con_zoom(&self, factor: f64) -> Camara3D {
        Camara3D {
            zoom: (self.zoom * factor).clamp(ZOOM_MINIMO, ZOOM_MAXIMO),
            ..*self
        }
    }

    /// `camaraEncuadrada`: con recorte, se mira al centro del rectangulo y
    /// se acerca lo que haga falta para que llene el ancho.
    pub fn encuadrada(&self, recorte: Option<&Recorte>, ancho: f64, alto: f64) -> Camara3D {
        let Some(r) = recorte.filter(|r| r.sirve()) else {
            return *self;
        };
        let plano = Plano3D {
            punto: self.centro,
            normal: self.adelante(),
        };
        let medio = Pt {
            x: (r.x0 + r.x1) / 2.0 * ancho,
            y: (r.y0 + r.y1) / 2.0 * alto,
        };
        let centro = plano
            .corte(&self.rayo(medio, ancho, alto))
            .map_or(self.centro, |c| c.0);
        Camara3D {
            centro,
            zoom: self.zoom / r.ancho(),
            ..*self
        }
    }
}

/// **La vista del movil puesta en la orbita del visor**: misma direccion,
/// mismo centro y el mismo tamano en el centro de la pantalla. El visor del
/// PC mira en perspectiva (45 grados) y sin balanceo; una vista con lente se
/// ve con la del PC. `origen` es el del modelo (los vertices van relativos),
/// y `escala` los pixeles por `dp` de la ventana.
pub fn orbita_de(
    vista: &Vista3D,
    origen: [f64; 3],
    ancho_px: u32,
    alto_px: u32,
    escala: f64,
) -> crate::ventana3d::Orbita {
    let (ancho, alto) = (
        ancho_px.max(1) as f64 / escala,
        alto_px.max(1) as f64 / escala,
    );
    let c = vista.camara.encuadrada(vista.recorte.as_ref(), ancho, alto);
    // Lo que se ve de alto en el centro: `alto / zoom` del mundo.
    let dist = alto / c.zoom.max(1e-9) / (2.0 * (crate::ventana3d::FOV / 2.0).tan());
    crate::ventana3d::Orbita {
        objetivo: [
            c.centro.x - origen[0],
            c.centro.y - origen[1],
            c.centro.z - origen[2],
        ],
        rumbo: -c.giro - PI / 2.0,
        altura: c
            .inclinacion
            .clamp(-crate::ventana3d::ALTURA_MAX, crate::ventana3d::ALTURA_MAX),
        dist: dist.max(1e-4),
    }
}

// ---- El pulso (PulsoDelMundo.kt) ----

const MINIMO_PARA_EL_PULSO: usize = 6;
const MINIMO_DEL_PULSO: f64 = 0.7;
const RANGO_DEL_PULSO: f64 = 2.5;
const VENTANA_DEL_RELOJ: f64 = 0.06;
const VARIACION_QUE_CUENTA: f64 = 0.02;
const PUNTAS_QUE_SE_AFILAN: usize = 3;
const AFILADO: f64 = 0.45;
const SUELO_DEL_FACTOR: f64 = AFILADO * MINIMO_DEL_PULSO * MINIMO_DEL_PULSO;

fn presion_media(presiones: Option<&[f64]>, n: usize) -> f64 {
    let Some(p) = presiones.filter(|p| !p.is_empty()) else {
        return 1.0;
    };
    let buenas: Vec<f64> = p
        .iter()
        .take(n)
        .filter(|x| !x.is_nan())
        .map(|x| x.clamp(0.0, 1.0))
        .collect();
    if buenas.is_empty() {
        1.0
    } else {
        buenas.iter().sum::<f64>() / buenas.len() as f64
    }
}

/// **`anchosDelMundo`**: el ancho de cada muestra de un trazo con pulso.
/// Lo rapido sale fino, apretar engorda, las puntas se afilan, y el mismo
/// gesto mil veces mas grande da los mismos anchos.
pub fn anchos_del_mundo(
    calibre: f64,
    presiones: Option<&[f64]>,
    recorrido: &[f64],
    tiempos: Option<&[f64]>,
    afila_el_principio: bool,
    afila_el_final: bool,
) -> Vec<f64> {
    let n = recorrido.len();
    if n == 0 {
        return Vec::new();
    }
    if n < MINIMO_PARA_EL_PULSO {
        return vec![calibre * presion_media(presiones, n); n];
    }
    let mut senal = vec![0.0; n];
    match tiempos.filter(|t| t.len() >= n) {
        Some(t) => {
            let mut anterior = 0.0;
            for i in 0..n {
                let (mut j, mut k) = (i, i);
                while j > 0 && t[j - 1] <= t[i] && t[i] - t[j - 1] < VENTANA_DEL_RELOJ / 2.0 {
                    j -= 1;
                }
                while k < n - 1 && t[k + 1] >= t[i] && t[k + 1] - t[i] < VENTANA_DEL_RELOJ / 2.0 {
                    k += 1;
                }
                if j == i && i > 0 {
                    j = i - 1;
                }
                if k == i && i < n - 1 {
                    k = i + 1;
                }
                let dt = t[k] - t[j];
                senal[i] = if dt > 0.0 {
                    (recorrido[k] - recorrido[j]) / dt
                } else {
                    anterior
                };
                anterior = senal[i];
            }
        }
        None => {
            for i in 1..n {
                senal[i] = recorrido[i] - recorrido[i - 1];
            }
            senal[0] = senal[1];
        }
    }
    let mut copia = senal.clone();
    copia.sort_by(f64::total_cmp);
    let mediana = copia[n / 2];
    let presiones = presiones.filter(|p| !p.is_empty());
    let hay_presion = presiones.is_some_and(|p| {
        let tomadas = &p[..p.len().min(n)];
        let floja = tomadas.iter().copied().fold(f64::MAX, f64::min);
        let fuerte = tomadas.iter().copied().fold(-f64::MAX, f64::max);
        fuerte - floja > VARIACION_QUE_CUENTA
    });
    let mut salida = vec![0.0; n];
    for i in 0..n {
        let deprisa = if mediana > 0.0 {
            ((senal[i] / mediana - 1.0) / RANGO_DEL_PULSO).clamp(0.0, 1.0)
        } else if senal[i] > 0.0 {
            1.0
        } else {
            0.0
        };
        let mut suyo = 1.0 - deprisa * (1.0 - MINIMO_DEL_PULSO);
        if hay_presion && let Some(p) = presiones {
            let v = p[i.min(p.len() - 1)].clamp(0.0, 1.0);
            suyo *= MINIMO_DEL_PULSO + (1.0 - MINIMO_DEL_PULSO) * v;
        }
        let del_principio = if afila_el_principio { i } else { usize::MAX };
        let del_final = if afila_el_final {
            n - 1 - i
        } else {
            usize::MAX
        };
        let del_borde = del_principio.min(del_final);
        if del_borde < PUNTAS_QUE_SE_AFILAN {
            let cuanto = (del_borde as f64 + 1.0) / (PUNTAS_QUE_SE_AFILAN as f64 + 1.0);
            suyo *= AFILADO + (1.0 - AFILADO) * cuanto;
        }
        salida[i] = if suyo.is_nan() {
            1.0
        } else {
            suyo.clamp(SUELO_DEL_FACTOR, 1.0)
        };
    }
    let alisar = |v: &[f64]| -> Vec<f64> {
        (0..n)
            .map(|i| {
                let a = v[if i == 0 { 0 } else { i - 1 }];
                let b = v[if i == n - 1 { n - 1 } else { i + 1 }];
                (a + v[i] * 2.0 + b) / 4.0
            })
            .collect()
    };
    alisar(&alisar(&salida))
        .into_iter()
        .map(|f| calibre * f)
        .collect()
}

/// La primera letra en mayuscula (`liston` -> `Liston`), para nombrar.
pub(crate) fn mayuscula(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(p) => p.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}
