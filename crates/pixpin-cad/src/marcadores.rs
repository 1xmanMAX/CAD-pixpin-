//! **Los marcadores de los planos y de los modelos** (19-oct-2026). El
//! usuario: «los marcadores deben existir en CAD y en todos los formatos,
//! para ir a una parte concreta (un muro concreto), guardando no solo la
//! posicion sino tambien el zoom. En BIM, al poner un marcador puedo hacer
//! anotaciones, y solo se ven en esa vista, a ese zoom, en esa posicion y en
//! ese estado (si estaba en corte o algo): las anotaciones van con todo el
//! estado; asi se guarda informacion de esos puntos en un modelo 3D».
//!
//! - **En el plano** ([`Marcador2d`]): el centro y lo que se ve de ancho y
//!   de alto (la posicion y el zoom), en coordenadas del dibujo.
//! - **En el modelo** ([`Estado3d`]): la camara (objetivo, rumbo, altura y
//!   distancia: el zoom), la caja de seccion, lo aislado, las plantas y
//!   categorias ocultas y, para volver igual, el tema y las aristas.
//!   Lo anotado sobre un marcador 3D es tinta en la pantalla y solo se ve
//!   mientras la vista **es** la del marcador ([`coincide`]): al girar se
//!   va y al volver vuelve.
//! - **Los marcos para imprimir del modelo** ([`Marco3d`]), como los del
//!   plano (I, numerados 1, 2, 3…, Ctrl+P en grupo): cada uno es un recuadro
//!   de la pantalla con el estado en que se hizo; se ve cuando la vista es
//!   esa, y al imprimir se vuelve a cada uno y se fotografia.
//!
//! La pantalla se cuenta **desde el centro y en altos de ventana**
//! ([`a_normal`]): la camara 3D tiene el campo de vision vertical fijo, asi
//! que un punto del modelo cae en el mismo `(x, y)` normalizado aunque la
//! ventana cambie de tamano, y la tinta y los marcos no se separan de el.
//!
//! Todo va en un fichero JSON que da quien abre la ventana ([`Guardados`]).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::modelo::Constructor;
use crate::texto::Textos;

/// Un marcador del plano: el centro (coordenadas del dibujo, absolutas) y lo
/// que se ve de ancho y de alto alli.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Marcador2d {
    pub id: String,
    #[serde(default)]
    pub nombre: String,
    pub x: f64,
    pub y: f64,
    pub ancho: f64,
    pub alto: f64,
}

impl Marcador2d {
    /// Unidades del dibujo por pixel para ver lo mismo en una ventana de
    /// `w` x `h`: cabe todo lo que se veia (el lado que mas aprieta).
    pub fn px_en(&self, w: u32, h: u32) -> f64 {
        let px = (self.ancho / w.max(1) as f64).max(self.alto / h.max(1) as f64);
        if px.is_finite() && px > 0.0 { px } else { 1.0 }
    }
}

/// **El estado entero de la vista de un modelo**: lo que hace falta para
/// volver justo ahi. Coordenadas del mundo absolutas (con el origen del
/// modelo sumado), asi no dependen de como se leyo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Estado3d {
    pub objetivo: [f64; 3],
    pub rumbo: f64,
    pub altura: f64,
    pub dist: f64,
    /// La caja de seccion `[x0, y0, z0, x1, y1, z1]`, si estaba puesta.
    #[serde(default)]
    pub seccion: Option<[f64; 6]>,
    /// El elemento aislado (su numero en el modelo de este PC: solo una
    /// pista, el que manda es `aislado_clave`).
    #[serde(default)]
    pub aislado: Option<u32>,
    /// **La clave estable del aislado** (20-oct-2026,
    /// `Modelo3d::clave_de_elemento`): su `GlobalId`, o
    /// `~<tipo>|<nombre>|<orden>` sin el. Es la que vale en otro PC y en el
    /// movil; los marcadores de antes no la traen y van por `aislado`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aislado_clave: Option<String>,
    /// Las plantas y categorias ocultas (ver `arbol`), ordenadas: `n:=<clave
    /// del nivel>`, `n:-` y `t:<categoria>` (`n:<numero>`, lo de antes).
    #[serde(default)]
    pub ocultos: Vec<String>,
    #[serde(default)]
    pub claro: bool,
    #[serde(default = "si")]
    pub aristas: bool,
}

fn si() -> bool {
    true
}

/// **Si dos estados son la misma vista**: la misma camara (con una
/// milesima de margen, lo que se pierde al guardar y al animar), la misma
/// caja de seccion, lo mismo aislado y lo mismo oculto. El tema y las
/// aristas no cuentan: cambian como se ve, no lo que se ve.
pub fn coincide(a: &Estado3d, b: &Estado3d) -> bool {
    let d = (0..3)
        .map(|k| (a.objetivo[k] - b.objetivo[k]).powi(2))
        .sum::<f64>()
        .sqrt();
    let escala = b.dist.abs().max(1e-6);
    let mut dr = (a.rumbo - b.rumbo) % std::f64::consts::TAU;
    if dr > std::f64::consts::PI {
        dr -= std::f64::consts::TAU;
    } else if dr < -std::f64::consts::PI {
        dr += std::f64::consts::TAU;
    }
    let camara = d < escala * 2e-3
        && dr.abs() < 2e-3
        && (a.altura - b.altura).abs() < 2e-3
        && (a.dist / escala - 1.0).abs() < 2e-3;
    let seccion = match (a.seccion, b.seccion) {
        (None, None) => true,
        (Some(p), Some(q)) => p
            .iter()
            .zip(q.iter())
            .all(|(x, y)| (x - y).abs() <= escala * 1e-3 + 1e-6),
        _ => false,
    };
    let aislado = match (&a.aislado_clave, &b.aislado_clave) {
        (Some(x), Some(y)) => x == y,
        _ => a.aislado == b.aislado,
    };
    camara && seccion && aislado && a.ocultos == b.ocultos
}

/// Un marcador del modelo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Marcador3d {
    pub id: String,
    #[serde(default)]
    pub nombre: String,
    pub estado: Estado3d,
}

/// Un marco para imprimir del modelo: su recuadro en la pantalla
/// (normalizado, [`a_normal`]: `[x0, y0, x1, y1]`) y el estado de la vista.
/// Con su `id` (desde el 19-oct-2026, al viajar): la sincronizacion junta
/// las listas elemento por elemento por el `id`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Marco3d {
    #[serde(default)]
    pub id: String,
    pub caja: [f64; 4],
    pub estado: Estado3d,
}

/// Del pixel de la ventana al punto normalizado: desde el centro y en altos
/// de ventana (x a la derecha, y hacia abajo).
pub fn a_normal(x: f64, y: f64, w: u32, h: u32) -> [f64; 2] {
    let h = h.max(1) as f64;
    [(x - w as f64 / 2.0) / h, (y - h / 2.0) / h]
}

/// Y de vuelta, al pixel.
pub fn de_normal(p: [f64; 2], w: u32, h: u32) -> [f64; 2] {
    let hf = h.max(1) as f64;
    [p[0] * hf + w as f64 / 2.0, p[1] * hf + hf / 2.0]
}

/// La caja de un marco, en pixeles de la ventana `[x0, y0, x1, y1]`.
pub fn caja_en_pixeles(caja: [f64; 4], w: u32, h: u32) -> [f64; 4] {
    let a = de_normal([caja[0], caja[1]], w, h);
    let b = de_normal([caja[2], caja[3]], w, h);
    [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[0].max(b[0]),
        a[1].max(b[1]),
    ]
}

/// **Un id nuevo** para un marcador: la hora en nanosegundos y un contador
/// (dos en el mismo instante no se pisan).
pub fn nuevo_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static CUENTA: AtomicU64 = AtomicU64::new(0);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    format!("m{:x}{:x}", t, CUENTA.fetch_add(1, Ordering::Relaxed))
}

/// Lo que se guarda de un plano o un modelo, en memoria (en el disco va en
/// dos ficheros: [`FicheroDeVistas`] y [`FicheroDeMarcos`]).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Contenido {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plano: Vec<Marcador2d>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modelo: Vec<Marcador3d>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marcos: Vec<Marco3d>,
}

/// La version del formato de los dos ficheros. Sube solo si cambia el
/// sentido de un campo; un campo nuevo con su valor por defecto no la sube.
pub const VERSION: u32 = 1;

fn version() -> u32 {
    VERSION
}

/// **El fichero de los marcadores** (`anot-<uid>.vistas.json` de un
/// adjunto del chat, que viaja; `vistas.json` en la carpeta local si no):
///
/// ```json
/// { "version": 1,
///   "plano":  [ { "id": "m…", "nombre": "", "x": 0, "y": 0, "ancho": 0, "alto": 0 } ],
///   "modelo": [ { "id": "m…", "nombre": "", "estado": { "objetivo": [0, 0, 0], "rumbo": 0, "altura": 0, "dist": 1 } } ] }
/// ```
///
/// Las dos listas van siempre, aunque esten vacias: «sin marcadores» es un
/// `[]` escrito (la sincronizacion no lleva borrados de ficheros, y quitar
/// la clave no se distingue de un fichero viejo). Lo que no se conoce (un
/// campo que ponga el movil) se deja como estaba.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FicheroDeVistas {
    #[serde(default = "version")]
    pub version: u32,
    #[serde(default)]
    pub plano: Vec<Marcador2d>,
    #[serde(default)]
    pub modelo: Vec<Marcador3d>,
    /// Solo al leer: el `vistas.json` del 19-oct llevaba aqui los marcos.
    #[serde(default, skip_serializing)]
    pub marcos: Vec<Marco3d>,
    #[serde(flatten)]
    pub resto: serde_json::Map<String, serde_json::Value>,
}

/// **El fichero de los marcos para imprimir del modelo**
/// (`anot-<uid>.marcos.json`; `marcos.json` en la carpeta local):
///
/// ```json
/// { "version": 1, "marcos": [ { "id": "m…", "caja": [-0.2, -0.1, 0.3, 0.2], "estado": { "objetivo": [0, 0, 0], "rumbo": 0, "altura": 0, "dist": 1 } } ] }
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FicheroDeMarcos {
    #[serde(default = "version")]
    pub version: u32,
    #[serde(default)]
    pub marcos: Vec<Marco3d>,
    #[serde(flatten)]
    pub resto: serde_json::Map<String, serde_json::Value>,
}

/// **El fichero de los marcos, junto al de los marcadores**:
/// `anot-<uid>.vistas.json` → `anot-<uid>.marcos.json`, `vistas.json` →
/// `marcos.json`; otro nombre, `<nombre>.marcos.json` a su lado.
pub fn marcos_junto_a(vistas: &Path) -> PathBuf {
    let nombre = vistas
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let nuevo = match nombre.strip_suffix("vistas.json") {
        Some(base) => format!("{base}marcos.json"),
        None => format!("{nombre}.marcos.json"),
    };
    vistas.with_file_name(nuevo)
}

/// Lo que tiene un `id` para juntar listas.
trait ConId {
    fn id(&self) -> &str;
}
impl ConId for Marcador2d {
    fn id(&self) -> &str {
        &self.id
    }
}
impl ConId for Marcador3d {
    fn id(&self) -> &str {
        &self.id
    }
}
impl ConId for Marco3d {
    fn id(&self) -> &str {
        &self.id
    }
}

/// **Junta dos versiones de una lista** a partir de la que tuvieron en
/// comun (`base`), por el `id`, con las reglas de la sincronizacion
/// (`pixpin_sincro::fusion`): lo nuevo de cualquiera entra; lo que uno
/// quito se va si el otro no lo cambio; lo cambiado gana a lo quitado; si
/// solo uno cambio algo, manda ese, y si los dos, el mio. El orden es el
/// mio, con lo nuevo del otro detras.
fn juntar_lista<T: ConId + Clone + PartialEq>(base: &[T], mio: &[T], suyo: &[T]) -> Vec<T> {
    let en = |l: &[T], id: &str| l.iter().find(|x| x.id() == id).cloned();
    let mut salida = Vec::with_capacity(mio.len().max(suyo.len()));
    for m in mio {
        let b = en(base, m.id());
        match en(suyo, m.id()) {
            Some(s) => salida.push(if b.as_ref() == Some(m) { s } else { m.clone() }),
            // Lo quito el otro: se va si aqui sigue como estaba.
            None if b.as_ref() == Some(m) => {}
            None => salida.push(m.clone()),
        }
    }
    for s in suyo.iter().filter(|s| en(mio, s.id()).is_none()) {
        match en(base, s.id()) {
            // Lo quite yo y el otro no lo cambio: fuera.
            Some(b) if b == *s => {}
            _ => salida.push(s.clone()),
        }
    }
    salida
}

/// [`juntar_lista`] para todo lo guardado.
fn juntar(base: &Contenido, mio: &Contenido, suyo: &Contenido) -> Contenido {
    Contenido {
        plano: juntar_lista(&base.plano, &mio.plano, &suyo.plano),
        modelo: juntar_lista(&base.modelo, &mio.modelo, &suyo.modelo),
        marcos: juntar_lista(&base.marcos, &mio.marcos, &suyo.marcos),
    }
}

/// Fecha y tamano de un fichero: cambia cuando la sincronizacion trae algo.
type Sello = Option<(std::time::SystemTime, u64)>;

fn sello_de(f: &Path) -> Sello {
    std::fs::metadata(f)
        .ok()
        .and_then(|m| Some((m.modified().ok()?, m.len())))
}

/// Cada cuanto se mira si el disco cambio ([`Guardados::mirar_el_disco`]).
const MIRAR_EL_DISCO: std::time::Duration = std::time::Duration::from_millis(1000);

/// Por un temporal y luego encima: la sincronizacion no lee nunca uno a
/// medias. El temporal (`….tmp`) no es de lo que viaja.
fn escribir_atomico(f: &Path, texto: &str) -> std::io::Result<()> {
    if let Some(d) = f.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = f.with_extension("tmp");
    std::fs::write(&tmp, texto)?;
    std::fs::rename(&tmp, f)
}

/// Lo que no se conoce de un fichero, para dejarlo como estaba.
type Resto = serde_json::Map<String, serde_json::Value>;

/// **Los marcadores (y los marcos del modelo) con sus ficheros**: el de los
/// marcadores que da quien abre la ventana y el de los marcos a su lado
/// ([`marcos_junto_a`]). Sin fichero duran lo que la ventana.
///
/// Guardar **junta con lo que haya en el disco** si alguien lo cambio
/// desde la ultima vez (la sincronizacion, u otra ventana del mismo plano:
/// la de 2D y la de 3D comparten fichero), y no toca el fichero si ya dice
/// lo mismo. Nunca lo borra: sin nada escribe las listas vacias.
#[derive(Debug, Clone, Default)]
pub struct Guardados {
    pub c: Contenido,
    fichero: Option<PathBuf>,
    /// Lo que habia en el disco la ultima vez que se leyo o escribio.
    base: Contenido,
    /// El texto de cada fichero entonces (`None`: no estaba).
    textos: [Option<String>; 2],
    restos: [Resto; 2],
    sellos: [Sello; 2],
    mirado: Option<std::time::Instant>,
}

/// Lo leido de los dos ficheros.
struct Leido {
    c: Contenido,
    textos: [Option<String>; 2],
    restos: [Resto; 2],
    /// Marcos sin `id` (los del 19-oct): hay que escribirlos con el suyo.
    sin_id: bool,
}

fn leer_ficheros(vistas: &Path) -> Leido {
    let marcos = marcos_junto_a(vistas);
    let textos = [
        std::fs::read_to_string(vistas).ok(),
        std::fs::read_to_string(&marcos).ok(),
    ];
    // Uno roto no tumba nada: como si no hubiera.
    let v: FicheroDeVistas = textos[0]
        .as_deref()
        .and_then(|t| serde_json::from_str(t).ok())
        .unwrap_or_default();
    let m: Option<FicheroDeMarcos> = textos[1]
        .as_deref()
        .and_then(|t| serde_json::from_str(t).ok());
    let mut lista = match &m {
        Some(m) => m.marcos.clone(),
        // Aun sin su fichero: los que traia el de los marcadores.
        None => v.marcos.clone(),
    };
    let mut sin_id = false;
    for k in lista.iter_mut().filter(|k| k.id.is_empty()) {
        k.id = nuevo_id();
        sin_id = true;
    }
    Leido {
        c: Contenido {
            plano: v.plano,
            modelo: v.modelo,
            marcos: lista,
        },
        textos,
        restos: [v.resto, m.map(|m| m.resto).unwrap_or_default()],
        sin_id,
    }
}

/// Lo guardado en `vistas` (y en el de sus marcos, al lado), sin escribir
/// nada: lo de antes, para pasarlo a otro sitio.
pub fn leer(vistas: &Path) -> Contenido {
    leer_ficheros(vistas).c
}

impl Guardados {
    pub fn de(fichero: Option<&Path>) -> Guardados {
        let Some(f) = fichero else {
            return Guardados::default();
        };
        let l = leer_ficheros(f);
        let mut g = Guardados {
            c: l.c.clone(),
            fichero: Some(f.to_path_buf()),
            base: l.c,
            textos: l.textos,
            restos: l.restos,
            sellos: [sello_de(f), sello_de(&marcos_junto_a(f))],
            mirado: None,
        };
        // Los marcos de antes, con su id y en su fichero, una vez.
        if l.sin_id {
            g.guardar();
        }
        g
    }

    /// El fichero de los marcadores, si lo hay.
    pub fn fichero(&self) -> Option<&Path> {
        self.fichero.as_deref()
    }

    /// Los dos textos que dicen lo de ahora (`None`: ese fichero no hace
    /// falta: no esta y no hay nada que poner).
    fn textos_de_ahora(&self) -> [Option<String>; 2] {
        let v = FicheroDeVistas {
            version: VERSION,
            plano: self.c.plano.clone(),
            modelo: self.c.modelo.clone(),
            marcos: Vec::new(),
            resto: self.restos[0].clone(),
        };
        let m = FicheroDeMarcos {
            version: VERSION,
            marcos: self.c.marcos.clone(),
            resto: self.restos[1].clone(),
        };
        let vacios = [
            self.c.plano.is_empty() && self.c.modelo.is_empty(),
            self.c.marcos.is_empty(),
        ];
        let t = [
            serde_json::to_string_pretty(&v).ok(),
            serde_json::to_string_pretty(&m).ok(),
        ];
        let mut salida: [Option<String>; 2] = [None, None];
        for k in 0..2 {
            // Vacio y sin fichero: nada que escribir (el plano en 2D no
            // tiene marcos del modelo; un plano sin marcadores, ningun fichero).
            if !(vacios[k] && self.textos[k].is_none()) {
                salida[k] = t[k].clone();
            }
        }
        salida
    }

    /// Lo escribe: si el disco cambio desde la ultima vez, junta antes; y
    /// solo el fichero que cambia. Sin nada, las listas vacias.
    pub fn guardar(&mut self) {
        let Some(f) = self.fichero.clone() else {
            return;
        };
        let disco = leer_ficheros(&f);
        if disco.textos != self.textos {
            self.c = juntar(&self.base, &self.c, &disco.c);
            self.textos = disco.textos;
            self.restos = disco.restos;
        }
        let rutas = [f.clone(), marcos_junto_a(&f)];
        let nuevos = self.textos_de_ahora();
        for k in 0..2 {
            let Some(t) = &nuevos[k] else { continue };
            if self.textos[k].as_deref() == Some(t.as_str()) {
                continue;
            }
            match escribir_atomico(&rutas[k], t) {
                Ok(()) => self.textos[k] = Some(t.clone()),
                Err(e) => {
                    tracing::warn!(?e, fichero = %rutas[k].display(), "marcadores: no se pudieron guardar")
                }
            }
        }
        self.base = self.c.clone();
        self.sellos = [sello_de(&rutas[0]), sello_de(&rutas[1])];
    }

    /// **Lo que trajo la sincronizacion, a la vista**: si un fichero cambio
    /// en el disco, se junta con lo de aqui. Dice si cambio lo que se ve.
    pub fn al_dia(&mut self) -> bool {
        let Some(f) = self.fichero.clone() else {
            return false;
        };
        let sellos = [sello_de(&f), sello_de(&marcos_junto_a(&f))];
        if sellos == self.sellos {
            return false;
        }
        self.sellos = sellos;
        let disco = leer_ficheros(&f);
        if disco.textos == self.textos {
            return false;
        }
        let antes = self.c.clone();
        self.c = juntar(&self.base, &self.c, &disco.c);
        self.base = disco.c;
        self.textos = disco.textos;
        self.restos = disco.restos;
        // Si aqui habia algo que el disco no tiene, se escribe junto.
        if self.c != self.base || disco.sin_id {
            self.guardar();
        }
        self.c != antes
    }

    /// [`Guardados::al_dia`] sin mirar el disco mas de una vez por segundo
    /// (se llama en cada vuelta de la ventana).
    pub fn mirar_el_disco(&mut self) -> bool {
        if self.mirado.is_some_and(|t| t.elapsed() < MIRAR_EL_DISCO) {
            return false;
        }
        self.mirado = Some(std::time::Instant::now());
        self.al_dia()
    }

    /// **Suma lo de `otro` que aqui no esta** (por el `id`) y dice si sumo
    /// algo: la migracion de los marcadores de antes. No guarda.
    pub fn sumar(&mut self, otro: &Contenido) -> bool {
        fn sumar_lista<T: ConId + Clone>(aqui: &mut Vec<T>, de: &[T]) -> bool {
            let nuevos: Vec<T> = de
                .iter()
                .filter(|x| !aqui.iter().any(|y| y.id() == x.id()))
                .cloned()
                .collect();
            let hay = !nuevos.is_empty();
            aqui.extend(nuevos);
            hay
        }
        let a = sumar_lista(&mut self.c.plano, &otro.plano);
        let b = sumar_lista(&mut self.c.modelo, &otro.modelo);
        let c = sumar_lista(&mut self.c.marcos, &otro.marcos);
        a || b || c
    }
}

// ---------------------------------------------------------------------------
// Los puntos bajo el riel

/// Lo que ocupa cada punto a lo alto y su diametro (en reposo y con el
/// raton encima): los del riel de los lectores (`pixpin_ui::riel_marcas`).
const PASO: f64 = 30.0;
const PUNTO: f64 = 20.0;
const PUNTO_CERCA: f64 = 26.0;

/// El centro de cada punto, debajo de la isla del riel (`isla`, en pixeles
/// de la ventana), apretados si no caben en `alto`.
pub fn puntos(isla: [f64; 4], n: usize, alto: u32, e: f64) -> Vec<[f64; 2]> {
    if n == 0 {
        return Vec::new();
    }
    let arriba = isla[3] + 12.0 * e;
    let hueco = (alto as f64 - arriba - 16.0 * e).max(0.0);
    let paso = (PASO * e).min(hueco / n as f64).max(1.0);
    let x = (isla[0] + isla[2]) / 2.0;
    (0..n)
        .map(|i| [x, arriba + paso * i as f64 + paso / 2.0])
        .collect()
}

/// El punto bajo (x, y): dentro de la columna, el mas cercano a lo alto.
pub fn punto_en(isla: [f64; 4], n: usize, alto: u32, e: f64, x: i32, y: i32) -> Option<usize> {
    let p = puntos(isla, n, alto, e);
    let (Some(a), Some(b)) = (p.first(), p.last()) else {
        return None;
    };
    let medio = (p.get(1).map_or(PASO * e, |q| q[1] - a[1])) / 2.0;
    let (x, y) = (x as f64, y as f64);
    if x < isla[0] || x >= isla[2] || y < a[1] - medio || y >= b[1] + medio {
        return None;
    }
    p.iter()
        .enumerate()
        .min_by(|(_, u), (_, v)| (u[1] - y).abs().total_cmp(&(v[1] - y).abs()))
        .map(|(i, _)| i)
}

fn disco_en(c: &mut Constructor, centro: [f64; 2], r: f64, color: u32, capa: f64) {
    let n = 24;
    let mut t = Vec::with_capacity(n * 3);
    for i in 0..n {
        let a0 = i as f64 / n as f64 * std::f64::consts::TAU;
        let a1 = (i + 1) as f64 / n as f64 * std::f64::consts::TAU;
        t.extend_from_slice(&[
            centro,
            [centro[0] + r * a0.cos(), centro[1] + r * a0.sin()],
            [centro[0] + r * a1.cos(), centro[1] + r * a1.sin()],
        ]);
    }
    c.triangulos(&t, color, Some(capa));
}

/// **Pinta los puntos**: cada uno con su numero (1, 2, 3…), en el cristal
/// oscuro del riel; el que tiene el raton encima, mas grande, y el de la
/// vista de ahora en el azul de lo pulsado.
#[allow(clippy::too_many_arguments)]
pub fn pintar_puntos(
    c: &mut Constructor,
    textos: &mut Textos,
    isla: [f64; 4],
    n: usize,
    alto: u32,
    e: f64,
    encima: Option<usize>,
    actual: Option<usize>,
) {
    let cristal = (crate::convertir::rgba(0x26, 0x26, 0x2c) & 0x00ff_ffff) | 0xEB00_0000;
    let azul = crate::convertir::rgba(0x00, 0x60, 0xDF);
    let blanco = crate::convertir::rgba(255, 255, 255);
    let capa = crate::ventana::CAPA_RAYAS * 10.0;
    for (i, p) in puntos(isla, n, alto, e).into_iter().enumerate() {
        let d = if encima == Some(i) {
            PUNTO_CERCA
        } else {
            PUNTO
        } * e;
        disco_en(c, p, d / 2.0 + 1.0 * e, 0x60ff_ffff, capa * 2.0);
        disco_en(
            c,
            p,
            d / 2.0,
            if actual == Some(i) { azul } else { cristal },
            capa,
        );
        let t = (i + 1).to_string();
        let tam = 11.0 * e;
        let w = textos.medir_pantalla(&t, tam);
        textos.en_pantalla(
            c,
            &t,
            p[0] - w / 2.0,
            p[1] + tam * 0.36,
            tam,
            blanco,
            p[0] + d,
        );
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn estado() -> Estado3d {
        Estado3d {
            objetivo: [100.0, -20.0, 3.0],
            rumbo: -0.8,
            altura: 0.5,
            dist: 40.0,
            seccion: Some([0.0, 0.0, 0.0, 10.0, 10.0, 3.5]),
            aislado: None,
            aislado_clave: None,
            ocultos: vec!["t:Muro".into()],
            claro: false,
            aristas: true,
        }
    }

    #[test]
    fn el_estado_se_guarda_y_vuelve_igual() {
        let d = std::env::temp_dir().join(format!("pixpin-marcadores-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let f = d.join("anot-x.vistas.json");
        let fm = marcos_junto_a(&f);
        assert_eq!(fm, d.join("anot-x.marcos.json"));
        let mut g = Guardados::de(Some(&f));
        assert_eq!(g.c, Contenido::default());
        g.c.modelo.push(Marcador3d {
            id: nuevo_id(),
            nombre: "Muro norte".into(),
            estado: estado(),
        });
        g.c.plano.push(Marcador2d {
            id: nuevo_id(),
            nombre: String::new(),
            x: 1.5,
            y: -2.0,
            ancho: 300.0,
            alto: 200.0,
        });
        g.c.marcos.push(Marco3d {
            id: nuevo_id(),
            caja: [-0.2, -0.1, 0.3, 0.2],
            estado: estado(),
        });
        g.guardar();
        let otra = Guardados::de(Some(&f));
        assert_eq!(otra.c, g.c);
        assert!(coincide(&otra.c.modelo[0].estado, &estado()));
        // Los marcos en su fichero, no en el de los marcadores; los dos con su version.
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        let m: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&fm).unwrap()).unwrap();
        assert!(v.get("marcos").is_none() && v["version"] == 1 && m["version"] == 1);
        assert_eq!(m["marcos"][0]["caja"][2], 0.3);
        // Sin nada, **no se borra**: las listas vacias (la sincronizacion no lleva borrados).
        g.c = Contenido::default();
        g.guardar();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        let m: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&fm).unwrap()).unwrap();
        assert_eq!(v["plano"], serde_json::json!([]));
        assert_eq!(v["modelo"], serde_json::json!([]));
        assert_eq!(m["marcos"], serde_json::json!([]));
        assert_eq!(Guardados::de(Some(&f)).c, Contenido::default());
        // Caso negativo: un fichero roto no tumba nada, no hay marcadores.
        std::fs::write(&f, "{roto").unwrap();
        assert_eq!(Guardados::de(Some(&f)).c, Contenido::default());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Un plano sin marcadores no deja ficheros; y lo que ya dice lo mismo
    /// no se reescribe (una fecha nueva seria un envio mas).
    #[test]
    fn sin_cambios_no_se_escribe() {
        let d =
            std::env::temp_dir().join(format!("pixpin-marcadores-igual-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let f = d.join("vistas.json");
        let mut g = Guardados::de(Some(&f));
        g.guardar();
        assert!(!f.exists() && !marcos_junto_a(&f).exists());
        g.c.plano.push(Marcador2d {
            id: "a".into(),
            nombre: String::new(),
            x: 0.0,
            y: 0.0,
            ancho: 1.0,
            alto: 1.0,
        });
        g.guardar();
        // El plano en 2D no tiene marcos del modelo: ese fichero no hace falta.
        assert!(f.exists() && !marcos_junto_a(&f).exists());
        let antes = std::fs::metadata(&f).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(30));
        g.guardar();
        assert_eq!(std::fs::metadata(&f).unwrap().modified().unwrap(), antes);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// **Lo que cambio en el disco se junta**: el movil (o la otra ventana)
    /// puso uno y quito otro mientras aqui se ponia un tercero.
    #[test]
    fn al_guardar_se_junta_con_lo_del_disco() {
        let d =
            std::env::temp_dir().join(format!("pixpin-marcadores-juntar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let f = d.join("anot-x.vistas.json");
        let m2 = |id: &str| Marcador2d {
            id: id.into(),
            nombre: String::new(),
            x: 0.0,
            y: 0.0,
            ancho: 1.0,
            alto: 1.0,
        };
        let mut g = Guardados::de(Some(&f));
        g.c.plano = vec![m2("a"), m2("b")];
        g.guardar();
        // El otro: quita «a», cambia el nombre de «b», pone «c» y un campo que aqui no se conoce.
        let suyo = r#"{"version":1,"plano":[{"id":"b","nombre":"Muro","x":0,"y":0,"ancho":1,"alto":1},{"id":"c","x":5,"y":5,"ancho":2,"alto":2}],"modelo":[],"delMovil":7}"#;
        std::fs::write(&f, suyo).unwrap();
        // Aqui, sin haberlo visto: se pone «d».
        g.c.plano.push(m2("d"));
        g.guardar();
        let ids: Vec<&str> = g.c.plano.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["b", "d", "c"]);
        assert_eq!(g.c.plano[0].nombre, "Muro");
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        assert_eq!(v["delMovil"], 7, "lo que no se conoce se deja");
        // Y con la ventana abierta, lo que llega se ve.
        let suyo = std::fs::read_to_string(&f)
            .unwrap()
            .replace(r#""id": "d""#, r#""id": "e""#);
        std::thread::sleep(std::time::Duration::from_millis(30));
        std::fs::write(&f, suyo).unwrap();
        assert!(g.al_dia());
        let ids: Vec<&str> = g.c.plano.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["b", "c", "e"]);
        // Caso negativo: sin cambios en el disco, nada.
        assert!(!g.al_dia());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// El `vistas.json` del 19-oct (los marcos dentro y sin `id`) se lee, y
    /// los marcos pasan a su fichero con su `id`, una vez.
    #[test]
    fn el_fichero_de_antes_pasa_al_de_ahora() {
        let d =
            std::env::temp_dir().join(format!("pixpin-marcadores-antes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("vistas.json");
        let viejo = r#"{"modelo":[{"id":"a","estado":{"objetivo":[0,0,0],"rumbo":0,"altura":0,"dist":1}}],
            "marcos":[{"caja":[0,0,0.1,0.1],"estado":{"objetivo":[0,0,0],"rumbo":0,"altura":0,"dist":1}}]}"#;
        std::fs::write(&f, viejo).unwrap();
        let g = Guardados::de(Some(&f));
        assert_eq!(g.c.modelo.len(), 1);
        assert_eq!(g.c.marcos.len(), 1);
        assert!(!g.c.marcos[0].id.is_empty());
        let m: FicheroDeMarcos =
            serde_json::from_str(&std::fs::read_to_string(marcos_junto_a(&f)).unwrap()).unwrap();
        assert_eq!(m.marcos, g.c.marcos);
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        assert!(v.get("marcos").is_none());
        // Otra vez: los mismos ids (ya escritos).
        assert_eq!(Guardados::de(Some(&f)).c, g.c);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn sumar_no_repite_lo_que_ya_esta() {
        let mut g = Guardados::default();
        let m2 = |id: &str| Marcador2d {
            id: id.into(),
            nombre: String::new(),
            x: 0.0,
            y: 0.0,
            ancho: 1.0,
            alto: 1.0,
        };
        g.c.plano = vec![m2("a")];
        assert!(g.sumar(&Contenido {
            plano: vec![m2("a"), m2("b")],
            ..Default::default()
        }));
        assert_eq!(g.c.plano.len(), 2);
        assert!(!g.sumar(&Contenido {
            plano: vec![m2("b")],
            ..Default::default()
        }));
    }

    #[test]
    fn un_estado_viejo_sin_campos_nuevos_se_lee() {
        let t = r#"{"modelo":[{"id":"a","estado":{"objetivo":[0,0,0],"rumbo":0,"altura":0,"dist":1}}]}"#;
        let c: Contenido = serde_json::from_str(t).unwrap();
        assert!(c.modelo[0].estado.aristas && c.modelo[0].estado.seccion.is_none());
    }

    #[test]
    fn la_vista_es_la_del_marcador_solo_si_nada_cambio() {
        let a = estado();
        // Lo que se pierde al guardar y al animar: sigue siendo la misma.
        let mut casi = a.clone();
        casi.objetivo[0] += 1e-6;
        casi.rumbo += std::f64::consts::TAU;
        casi.claro = true;
        casi.aristas = false;
        assert!(coincide(&casi, &a), "el tema y las aristas no cuentan");
        // Casos negativos: girar, acercar, mover el corte, aislar u ocultar es otra.
        type Cambio = Box<dyn Fn(&mut Estado3d)>;
        let cambios: Vec<Cambio> = vec![
            Box::new(|e| e.rumbo += 0.05),
            Box::new(|e| e.altura -= 0.05),
            Box::new(|e| e.dist *= 1.1),
            Box::new(|e| e.objetivo[2] += 1.0),
            Box::new(|e| e.seccion = None),
            Box::new(|e| e.seccion.as_mut().unwrap()[5] = 2.0),
            Box::new(|e| e.aislado = Some(7)),
            Box::new(|e| e.ocultos.clear()),
        ];
        for (i, c) in cambios.iter().enumerate() {
            let mut b = a.clone();
            c(&mut b);
            assert!(!coincide(&b, &a), "cambio {i}");
        }
    }

    #[test]
    fn la_pantalla_normalizada_no_depende_del_tamano() {
        let p = a_normal(700.0, 150.0, 1000, 600);
        assert!((p[0] - 200.0 / 600.0).abs() < 1e-12 && (p[1] + 0.25).abs() < 1e-12);
        let q = de_normal(p, 1000, 600);
        assert!((q[0] - 700.0).abs() < 1e-9 && (q[1] - 150.0).abs() < 1e-9);
        // El centro es el centro en cualquier ventana.
        assert_eq!(de_normal([0.0, 0.0], 800, 400), [400.0, 200.0]);
        let c = caja_en_pixeles([0.1, 0.1, -0.1, -0.1], 200, 100);
        assert_eq!(c, [90.0, 40.0, 110.0, 60.0]);
    }

    #[test]
    fn el_marcador_del_plano_ve_lo_mismo_en_otra_ventana() {
        let m = Marcador2d {
            id: "a".into(),
            nombre: String::new(),
            x: 0.0,
            y: 0.0,
            ancho: 1000.0,
            alto: 500.0,
        };
        assert_eq!(m.px_en(1000, 500), 1.0);
        // Mas estrecha: manda el ancho.
        assert_eq!(m.px_en(500, 500), 2.0);
        // Caso negativo: medidas imposibles no dan un zoom imposible.
        let roto = Marcador2d {
            ancho: f64::NAN,
            alto: 0.0,
            ..m
        };
        assert_eq!(roto.px_en(100, 100), 1.0);
    }

    #[test]
    fn los_puntos_van_bajo_la_isla_y_se_aprietan_si_no_caben() {
        let isla = [940.0, 16.0, 984.0, 124.0];
        let p = puntos(isla, 3, 800, 1.0);
        assert_eq!(p.len(), 3);
        assert!(p[0][1] > isla[3] && (p[1][1] - p[0][1] - 30.0).abs() < 1e-9);
        assert_eq!(punto_en(isla, 3, 800, 1.0, 962, p[1][1] as i32), Some(1));
        assert_eq!(
            punto_en(isla, 3, 800, 1.0, 962, p[2][1] as i32 + 4),
            Some(2)
        );
        // Fuera de la columna, ninguno.
        assert_eq!(punto_en(isla, 3, 800, 1.0, 500, p[1][1] as i32), None);
        assert_eq!(punto_en(isla, 3, 800, 1.0, 962, 700), None);
        assert_eq!(punto_en(isla, 0, 800, 1.0, 962, 150), None);
        // 30 en una ventana baja: caben todos.
        let p = puntos(isla, 30, 400, 1.0);
        assert!(p.last().unwrap()[1] < 400.0);
    }
}
