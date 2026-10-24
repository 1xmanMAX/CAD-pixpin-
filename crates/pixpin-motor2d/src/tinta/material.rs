//! **De que esta hecha la tinta**: lisa, encendida o con grano.
//!
//! Puerto del `MaterialDeTinta` del movil (`motor/Element.kt:416` y el grano
//! de `motor/Renderer.kt:2330-2493`, v0.59.0 `bd1a88c`). Lo que se guarda en
//! el `.excalidraw` es una palabra —`material`— y son EXACTAMENTE las suyas:
//! un trazo de tiza hecho aqui tiene que abrirse como tiza en el telefono y
//! al reves.
//!
//! ## Por que el grano es una tela y no rayas
//!
//! El movil lo intento primero rayando a mano: recortar el lienzo al contorno
//! y pintar una raya detras de otra por toda su caja. Son decenas de lineas
//! por figura **y por fotograma**, y con media docena de trazos rayados el
//! dibujo se arrastraba. Lo que se pinta no es un dibujo vectorial, es **una
//! textura**: un cuadradito que se repite, puesto como brocha, o sea un solo
//! relleno —lo mismo que pintar de un color liso—.
//!
//! Aqui ese cuadradito se teje en `tejido`, que es **puro**: no sabe de GPU
//! ni de Direct2D, devuelve 16x16 bytes de cobertura y se prueba sin
//! escritorio. Quien lo sube a una brocha es `pixpin-render`.
//!
//! ## Por que la cobertura y no el color
//!
//! El movil teje un mapa de bits por material **y color**, porque su brocha
//! pinta pixeles ya tenidos. El tejido de aqui guarda solo **cuanto tapa**
//! cada pixel, que es lo unico que depende del material; el color lo pone
//! quien pinta. Asi una hoja rayada de seis colores teje **un** tejido en vez
//! de seis.

use serde::{Deserialize, Serialize};

/// Cuanto tapa el cuerpo del trazo en las tintas porosas, de 0 a 1.
///
/// «Las porosas pintan el cuerpo mas flojo» (movil, 16-sep-2026): en la tiza,
/// el lapiz blando y el rotulador seco lo que se tiene que leer es el grano;
/// con el cuerpo a plena tinta el grano se pierde encima de un trazo macizo y
/// las tres se ven iguales que la lisa.
pub const CUERPO_DE_LAS_POROSAS: f32 = 0.45;

/// Cada cuantas veces el ancho del trazo se repite el cuadro de la tela.
pub const PASO_DEL_GRANO: f32 = 0.8;

/// El paso, acotado. **No depende del aumento, solo del ancho del trazo.**
///
/// En el movil dependio del aumento por las dos puntas y eso rompia lo unico
/// que una textura tiene que cumplir: ser la misma siempre. Acercandose, el
/// techo apretaba el mosaico y aparecian mas cuadros dentro del mismo trazo;
/// y a mucho aumento el maximo bajaba por debajo del minimo y el acotado
/// reventaba.
pub const PASO_MINIMO_DEL_GRANO: f32 = 3.5;
pub const PASO_MAXIMO_DEL_GRANO: f32 = 24.0;

/// El lado del cuadro que se teje, en pixeles. Siempre el mismo: lo grande o
/// pequeno que sale lo dice la matriz de la brocha, que no cuesta memoria.
pub const LADO_DEL_MOSAICO: u32 = 16;

/// Lo gordas que van las marcas, en veces el lado del cuadro.
pub const GORDO_DEL_GRANO: f32 = 0.13;

/// Cuanto se oscurece la tinta para el grano, de 0 a 1.
///
/// La trama es la **misma** tinta mas apretada, no otro color encima: un
/// rayado negro sobre una tinta azul son dos tintas y se lee como una mancha
/// sucia. Oscureciendo la suya, lo que se ve es relieve.
pub const CUANTO_OSCURECE_EL_GRANO: f32 = 0.42;

/// Lo que se gira la brocha del rayado, en grados.
///
/// Se gira **la brocha y no el dibujo**: con las rayas dibujadas en diagonal
/// dentro del cuadro, el corte se nota en cada junta. Se dibujan rectas —que
/// casan solas— y se inclina el mosaico entero.
pub const GRADOS_DEL_GRANO: f32 = 45.0;

/// **De que esta hecha la tinta.**
///
/// Las palabras del fichero son las del movil (`@SerialName` de
/// `MaterialDeTinta`), una por una y sin excepcion. Portar solo las cuatro
/// nuevas dejaria que un trazo rayado del telefono volviera liso: en cuanto
/// el PC modela un campo, manda lo del PC y lo que se conservaba sin
/// entenderlo se pierde.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum MaterialTinta {
    /// La de siempre.
    #[default]
    #[serde(rename = "lisa")]
    Lisa,
    /// Encendida: el trazo suma luz en vez de tapar.
    #[serde(rename = "luz")]
    Luz,
    /// La luz que se sale del blanco, en pantallas con margen.
    #[serde(rename = "hdr")]
    Hdr,
    /// Rayado de traves: la seccion cortada.
    #[serde(rename = "rayado")]
    Rayado,
    /// Rayado en reticula: la sombra.
    #[serde(rename = "cruzado")]
    Cruzado,
    /// Punteado: la tierra, la arena, el relleno.
    #[serde(rename = "puntos")]
    Puntos,
    /// **Tiza**: grano grueso y desigual, con motas. Se ve el papel dentro
    /// del trazo.
    #[serde(rename = "tiza")]
    Tiza,
    /// **Lapiz 2B**: blando y suave, grano fino y muy tupido, cuerpo a media
    /// tinta. El trazo de apunte.
    #[serde(rename = "lapiz2b")]
    Lapiz2b,
    /// **Rotulador seco**: el marcador que se acaba. Rayas a lo largo con
    /// huecos, asi que la raya sale cortada a lo largo en vez de maciza.
    #[serde(rename = "seco")]
    Seco,
    /// **Trama gruesa**: como el rayado pero mas gorda y mas separada, para
    /// rellenos de plano que se leen de lejos.
    #[serde(rename = "trama")]
    Trama,
    /// **Grafito** (`CUADRITOS` del movil, v0.73-v0.80.2): un lapiz de
    /// verdad, hecho de sellos rascados estampados a lo largo del trazo sobre
    /// una rejilla fija de cuadritos del dibujo. Ver `tinta::grafito`.
    ///
    /// La palabra del fichero es «cuadritos» y no «grafito» porque es la que
    /// escribe el movil: fue el nombre de su primera version, y cambiarla
    /// aqui haria que un trazo de grafito del telefono se abriera liso.
    ///
    /// **No esta en [`MATERIALES`]** a proposito: en el movil dejo de ser un
    /// material del panel y paso a ser una herramienta (v0.75, «Grafito»),
    /// asi que el panel no la ofrece como tinta. Se lee y se escribe igual.
    #[serde(rename = "cuadritos")]
    Cuadritos,
}

/// Todas, en el orden del enum del movil. Es el orden en que las ofrece el
/// panel y el que usan las pruebas para recorrerlas sin olvidarse ninguna.
pub const MATERIALES: [MaterialTinta; 10] = [
    MaterialTinta::Lisa,
    MaterialTinta::Luz,
    MaterialTinta::Hdr,
    MaterialTinta::Rayado,
    MaterialTinta::Cruzado,
    MaterialTinta::Puntos,
    MaterialTinta::Tiza,
    MaterialTinta::Lapiz2b,
    MaterialTinta::Seco,
    MaterialTinta::Trama,
];

impl MaterialTinta {
    /// La palabra que va al `.excalidraw`. La misma que el movil.
    pub fn palabra(self) -> &'static str {
        match self {
            MaterialTinta::Lisa => "lisa",
            MaterialTinta::Luz => "luz",
            MaterialTinta::Hdr => "hdr",
            MaterialTinta::Rayado => "rayado",
            MaterialTinta::Cruzado => "cruzado",
            MaterialTinta::Puntos => "puntos",
            MaterialTinta::Tiza => "tiza",
            MaterialTinta::Lapiz2b => "lapiz2b",
            MaterialTinta::Seco => "seco",
            MaterialTinta::Trama => "trama",
            MaterialTinta::Cuadritos => "cuadritos",
        }
    }

    /// El material de una palabra del fichero. `None` si no la conocemos:
    /// quien lee decide que hacer, y lo que hace es pintarla como pluma y
    /// devolverla intacta al guardar.
    pub fn desde_palabra(s: &str) -> Option<MaterialTinta> {
        // El grafito va aparte porque no esta en el panel, pero se lee igual:
        // sin esto, lo que llega del movil con `material:"cuadritos"` se
        // tomaba por desconocido y se pintaba liso.
        MATERIALES
            .into_iter()
            .chain([MaterialTinta::Cuadritos])
            .find(|m| m.palabra() == s)
    }

    /// Si se pinta como grafito: sellos sobre la rejilla fija, no un
    /// contorno relleno. Ver `tinta::grafito`.
    pub fn es_grafito(self) -> bool {
        self == MaterialTinta::Cuadritos
    }

    /// Si el cuerpo del trazo va mas flojo de lo normal porque lo que se ve
    /// es el grano.
    pub fn es_porosa(self) -> bool {
        matches!(
            self,
            MaterialTinta::Tiza | MaterialTinta::Lapiz2b | MaterialTinta::Seco
        )
    }

    /// Si suma luz: entonces no lleva grano, porque lo que se ve es el
    /// resplandor.
    pub fn alumbra(self) -> bool {
        matches!(self, MaterialTinta::Luz | MaterialTinta::Hdr)
    }

    /// Si hay tela que estampar dentro del trazo.
    ///
    /// El grafito no: su grano no es una tela que se repite dentro de una
    /// silueta, son los propios sellos y el diente del papel casilla a
    /// casilla. Con tela encima saldria dos veces granulado.
    pub fn hay_grano(self) -> bool {
        self != MaterialTinta::Lisa && !self.alumbra() && !self.es_grafito()
    }

    /// Si su tela va inclinada. El punteado y las motas no tienen direccion;
    /// el rotulador seco va **a lo largo del trazo**, que es como se descarga
    /// de verdad, y no de traves.
    pub fn se_inclina(self) -> bool {
        matches!(
            self,
            MaterialTinta::Rayado | MaterialTinta::Cruzado | MaterialTinta::Trama
        )
    }

    /// Cuanto tapa el cuerpo del trazo con este material, de 0 a 1.
    pub fn cuerpo(self) -> f32 {
        if self.es_porosa() {
            CUERPO_DE_LAS_POROSAS
        } else {
            1.0
        }
    }
}

// -------------------------------------------------------------------------
// Las dos encendidas
// -------------------------------------------------------------------------

/// **Hasta donde llega la llave de las luces** (`LO_MAS_QUE_ALUMBRAN`).
///
/// Hasta uno, la luz **se enciende**: el color se va hacia su tono vivo y el
/// centro hacia el blanco. De uno para arriba ya no queda color al que ir, y
/// lo que sube es cuanto se sale del blanco de la pantalla.
pub const LO_MAS_QUE_ALUMBRAN: f32 = 2.0;

/// Cuanto se va hacia el blanco una tinta encendida a tope.
///
/// No hasta el blanco del todo: una luz que llega a blanco puro pierde su
/// color, y lo que distingue una luz azul de una roja es justo eso. Tres
/// cuartos de camino dejan el tono a la vista y el trazo deslumbrando.
pub const HACIA_EL_BLANCO: f32 = 0.75;

/// Cuantas veces el blanco de la interfaz llega a valer la HDR a tope.
pub const VECES_EL_BLANCO: f32 = 2.0;

/// **La llave de paso de las luces del dibujo** (`LucesDelDibujo` del movil,
/// la clave `luces` de la escena).
///
/// Es una sola para todo el dibujo porque es lo que uno quiere tocar: lo que
/// se hace con las luces de un plano es subirlas todas o apagarlas todas —se
/// ensena el dibujo, se apagan; se mira de noche, se suben— y con un mando por
/// trazo eso son veinte gestos para una decision.
///
/// Hoy el PC **conserva** esta clave sin entenderla, dentro de `Lienzo.resto`.
/// Este tipo es lo que hace falta para leerla; el brazo del puente es del
/// grupo que herede `excalidraw.rs`. Ver el informe del grupo A.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LucesDelDibujo {
    pub encendidas: bool,
    /// Cuanto alumbran, de cero a dos. Uno es como se dibujo cada trazo.
    pub fuerza: f32,
}

impl Default for LucesDelDibujo {
    fn default() -> Self {
        Self {
            encendidas: true,
            fuerza: 1.0,
        }
    }
}

impl LucesDelDibujo {
    /// Lo que multiplica a la luz de cada trazo. Cero es apagado, y **apagado
    /// es tinta lisa**: no es un caso raro, es lo que promete el mando. En
    /// cero sale el color que se eligio, ni mas ni menos. Sin esto, una tinta
    /// de luz apagada seguia siendo una raya lavada que no era el color de
    /// nadie.
    pub fn cuanto(self) -> f32 {
        if self.encendidas {
            self.fuerza.clamp(0.0, LO_MAS_QUE_ALUMBRAN)
        } else {
            0.0
        }
    }
}

/// Un color llevado a su tono vivo: el mismo tinte, subido hasta que el canal
/// mas alto toca el tope (`aTodoBrillo`).
fn a_todo_brillo(c: crate::elemento::ColorRgba) -> crate::elemento::ColorRgba {
    let techo = c.r.max(c.g).max(c.b);
    // El negro no tiene tono al que ir: su version encendida es el blanco.
    if techo <= 0.0 {
        return crate::elemento::ColorRgba {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: c.a,
        };
    }
    let k = 1.0 / techo;
    crate::elemento::ColorRgba {
        r: (c.r * k).clamp(0.0, 1.0),
        g: (c.g * k).clamp(0.0, 1.0),
        b: (c.b * k).clamp(0.0, 1.0),
        a: c.a,
    }
}

/// **El color con el que se pinta una tinta encendida.**
///
/// Porte de `comoUnTubo` (`Renderer.kt:2159-2187`), y de aqui sale lo que mas
/// se acerca `luz`/`hdr` a lo del movil **sin inventar nada**:
///
/// El movil **ya no pinta un tubo de neon**. Lo pintaba —una escalera de
/// pasadas cada vez mas anchas alrededor del trazo— y lo quitó: la raya
/// cambiaba de grosor al subir la luz, se veian las capas alrededor, y lo
/// dibujado dejaba de medir lo que decia medir («una cota de un milimetro con
/// la luz alta ocupaba tres»). Hoy la tinta encendida **ocupa exactamente lo
/// mismo que apagada**: la misma ruta, el mismo grueso, el mismo contorno.
/// **Lo unico que cambia es el color.**
///
/// O sea que lo que aqui faltaba no era un resplandor: era esta cuenta. Con
/// ella, `luz` deja de pintarse «como pluma» y pasa a verse como en el
/// telefono, sin una sola pasada de mas.
///
/// `cuanta_luz` es [`LucesDelDibujo::cuanto`]. `margen_hdr` dice si la ventana
/// puede escribir por encima del blanco; sin el, la HDR se pinta como la luz
/// normal, acotada a lo que la pantalla sabe dar, que es lo que hace el movil.
///
/// Lo que **no** se puede portar tal cual es el rango extendido: el movil
/// empaqueta el color en `EXTENDED_SRGB`, donde uno es el blanco de la
/// interfaz y de ahi para arriba se sigue. Aqui el color sale con los canales
/// ya multiplicados y **sin acotar** cuando hay margen, para que quien pinte
/// decida si su destino los admite; con `margen_hdr` en falso sale acotado.
pub fn color_encendido(
    tinta: crate::elemento::ColorRgba,
    material: MaterialTinta,
    cuanta_luz: f32,
    margen_hdr: bool,
) -> crate::elemento::ColorRgba {
    if !material.alumbra() {
        return tinta;
    }
    let mando = cuanta_luz.max(0.0);
    // Apagada, la luz es tinta lisa. Ni una capa encima: el color que se
    // eligio.
    if mando <= 0.005 {
        return tinta;
    }
    let cuanta = mando.min(1.0);
    // Hacia su tono vivo...
    let techo = a_todo_brillo(tinta);
    let hacia = |a: f32, b: f32| a + (b - a) * cuanta;
    let vivo = crate::elemento::ColorRgba {
        r: hacia(tinta.r, techo.r),
        g: hacia(tinta.g, techo.g),
        b: hacia(tinta.b, techo.b),
        a: tinta.a,
    };
    // ...y de ahi hacia el blanco por dentro.
    let k = cuanta * HACIA_EL_BLANCO;
    let aclarar = |v: f32| (v + (1.0 - v) * k).clamp(0.0, 1.0);
    let encendida = crate::elemento::ColorRgba {
        r: aclarar(vivo.r),
        g: aclarar(vivo.g),
        b: aclarar(vivo.b),
        a: tinta.a,
    };
    // **Solo la HDR se sale del blanco**, y solo si hay donde.
    let de_mas = if material == MaterialTinta::Hdr {
        mando
    } else {
        0.0
    };
    if de_mas <= 0.005 || !margen_hdr {
        return encendida;
    }
    let sube = 1.0 + (VECES_EL_BLANCO - 1.0) * de_mas.clamp(0.0, 1.0);
    crate::elemento::ColorRgba {
        r: encendida.r * sube,
        g: encendida.g * sube,
        b: encendida.b * sube,
        a: tinta.a,
    }
}

/// El paso de la tela para un trazo de `gordo` de ancho.
///
/// Del ancho y no en pixeles: una trama es **una proporcion** —tantas rayas
/// por ancho— y no una medida. Con un paso fijo, una raya fina sale de puntos
/// sueltos y una gorda, maciza.
pub fn paso_del_grano(gordo: f32) -> f32 {
    (gordo * PASO_DEL_GRANO).clamp(PASO_MINIMO_DEL_GRANO, PASO_MAXIMO_DEL_GRANO)
}

/// **El azar de Java, clavado.**
///
/// Las motas y los huecos del rotulador seco salen de un `java.util.Random`
/// con semilla fija: una textura tiene que salir igual en cada fotograma, o
/// el trazo herviria al repintarse. Y tiene que salir igual **en los dos
/// aparatos**, o la misma tiza se veria de dos maneras. Asi que no vale
/// cualquier generador: hace falta ESTE, que es un congruencial lineal de 48
/// bits con las constantes de la biblioteca de Java.
#[derive(Clone)]
pub(crate) struct AzarJava {
    semilla: u64,
}

const MULTIPLICADOR: u64 = 0x0005_DEEC_E66D;
const SUMANDO: u64 = 0xB;
const MASCARA: u64 = (1 << 48) - 1;

impl AzarJava {
    pub(crate) fn nuevo(semilla: u64) -> Self {
        Self {
            semilla: (semilla ^ MULTIPLICADOR) & MASCARA,
        }
    }

    fn siguiente(&mut self, bits: u32) -> i32 {
        self.semilla = self
            .semilla
            .wrapping_mul(MULTIPLICADOR)
            .wrapping_add(SUMANDO)
            & MASCARA;
        (self.semilla >> (48 - bits)) as i32
    }

    pub(crate) fn flotante(&mut self) -> f32 {
        self.siguiente(24) as f32 / (1 << 24) as f32
    }

    /// `nextInt(tope)` de Java, con su rechazo y todo: sin el, los valores
    /// altos saldrian un poco mas veces y la secuencia se desviaria de la del
    /// movil en cuanto hubiera un rechazo.
    pub(crate) fn entero(&mut self, tope: i32) -> i32 {
        if tope & -tope == tope {
            return ((tope as i64).wrapping_mul(self.siguiente(31) as i64) >> 31) as i32;
        }
        loop {
            let bits = self.siguiente(31);
            let valor = bits % tope;
            if bits.wrapping_sub(valor).wrapping_add(tope - 1) >= 0 {
                return valor;
            }
        }
    }
}

/// Cuantas muestras por lado lleva cada pixel al tejer.
///
/// El movil teje con el suavizado de Skia; aqui se teje a mano, asi que el
/// suavizado hay que ponerlo. Cuatro por lado son dieciseis muestras: el
/// tejido son 256 pixeles y se hace **una vez por material**, o sea que esto
/// cuesta cuatro mil comprobaciones en toda la vida del programa.
const MUESTRAS: u32 = 4;

/// La tela a medio tejer: cuanto tapa cada **muestra**, de 0 a 1.
struct Telar {
    /// `LADO * MUESTRAS` de lado.
    tapa: Vec<f32>,
    lado: u32,
}

impl Telar {
    fn nuevo() -> Self {
        let lado = LADO_DEL_MOSAICO * MUESTRAS;
        Self {
            tapa: vec![0.0; (lado * lado) as usize],
            lado,
        }
    }

    /// El centro de la muestra `(i, j)`, en coordenadas del cuadro.
    fn centro(&self, i: u32, j: u32) -> (f32, f32) {
        let paso = 1.0 / MUESTRAS as f32;
        ((i as f32 + 0.5) * paso, (j as f32 + 0.5) * paso)
    }

    /// Estampa una figura encima de lo que ya hay, como el `SRC_OVER` de
    /// Android: lo de debajo se ve por lo que la de encima deja pasar.
    fn encima(&mut self, alfa: f32, dentro: impl Fn(f32, f32) -> bool) {
        for j in 0..self.lado {
            for i in 0..self.lado {
                let (x, y) = self.centro(i, j);
                if dentro(x, y) {
                    let bajo = &mut self.tapa[(j * self.lado + i) as usize];
                    *bajo = alfa + *bajo * (1.0 - alfa);
                }
            }
        }
    }

    fn circulo(&mut self, cx: f32, cy: f32, r: f32, alfa: f32) {
        let r2 = r * r;
        self.encima(alfa, |x, y| {
            let (dx, dy) = (x - cx, y - cy);
            dx * dx + dy * dy <= r2
        });
    }

    /// Una raya de `gordo` de ancho con las puntas a escuadra, que es el
    /// `Cap.BUTT` de Android: la raya es un rectangulo, no una capsula.
    fn raya(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, gordo: f32, alfa: f32) {
        let (vx, vy) = (x1 - x0, y1 - y0);
        let largo2 = vx * vx + vy * vy;
        if largo2 <= f32::EPSILON {
            return;
        }
        let mitad = gordo / 2.0;
        self.encima(alfa, |x, y| {
            let (px, py) = (x - x0, y - y0);
            let t = (px * vx + py * vy) / largo2;
            if !(0.0..=1.0).contains(&t) {
                return false;
            }
            let (qx, qy) = (px - t * vx, py - t * vy);
            (qx * qx + qy * qy).sqrt() <= mitad
        });
    }

    /// Promedia las muestras de cada pixel. Es el suavizado.
    fn rematar(self) -> Vec<u8> {
        let lado = LADO_DEL_MOSAICO;
        let por_pixel = (MUESTRAS * MUESTRAS) as f32;
        let mut salida = vec![0u8; (lado * lado) as usize];
        for py in 0..lado {
            for px in 0..lado {
                let mut suma = 0.0;
                for sy in 0..MUESTRAS {
                    for sx in 0..MUESTRAS {
                        let i = px * MUESTRAS + sx;
                        let j = py * MUESTRAS + sy;
                        suma += self.tapa[(j * self.lado + i) as usize];
                    }
                }
                salida[(py * lado + px) as usize] = (suma / por_pixel * 255.0).round() as u8;
            }
        }
        salida
    }
}

/// **El cuadro del grano, tejido.** `LADO_DEL_MOSAICO` al cuadrado bytes de
/// cobertura, de 0 (nada) a 255 (macizo), fila tras fila.
///
/// Devuelve el cuadro vacio para las que no llevan grano —la lisa y las dos
/// encendidas—, que es lo que dicen que son: sin tela que estampar.
pub fn tejido(material: MaterialTinta) -> Vec<u8> {
    let lado = LADO_DEL_MOSAICO as f32;
    let mut telar = Telar::nuevo();
    match material {
        MaterialTinta::Lisa | MaterialTinta::Luz | MaterialTinta::Hdr => {}
        // El grafito no se teje: se estampa (`tinta::grafito`).
        MaterialTinta::Cuadritos => {}

        MaterialTinta::Puntos => {
            telar.circulo(lado / 2.0, lado / 2.0, lado * GORDO_DEL_GRANO * 2.4, 1.0);
        }

        // Motas, siempre las mismas. La tiza las lleva gordas y sueltas; el
        // lapiz blando, finas y muy juntas.
        MaterialTinta::Tiza | MaterialTinta::Lapiz2b => {
            let tiza = material == MaterialTinta::Tiza;
            let cuantas = if tiza { 26 } else { 90 };
            let gordura = if tiza { 0.16 } else { 0.075 };
            let base = if tiza { 150 } else { 190 };
            let mut azar = AzarJava::nuevo(if tiza { 20260916 } else { 20260917 });
            for _ in 0..cuantas {
                let x = azar.flotante() * lado;
                let y = azar.flotante() * lado;
                let r = lado * gordura * (0.5 + azar.flotante());
                let alfa = (base + azar.entero(65)) as f32 / 255.0;
                telar.circulo(x, y, r, alfa);
            }
        }

        // Rayas a lo largo con huecos: el marcador que ya no moja del todo.
        MaterialTinta::Seco => {
            let gordo = lado * GORDO_DEL_GRANO;
            let mut azar = AzarJava::nuevo(20260918);
            for i in 0..5 {
                let y = (i as f32 + 0.5) * lado / 5.0;
                let mut x = -azar.flotante() * lado * 0.4;
                while x < lado {
                    let largo = lado * (0.25 + azar.flotante() * 0.5);
                    telar.raya(x, y, x + largo, y, gordo, 1.0);
                    x += largo + lado * (0.15 + azar.flotante() * 0.35);
                }
            }
        }

        // Como el rayado, pero gorda y con el doble de hueco: una sola linea
        // por cuadro.
        MaterialTinta::Trama => {
            let gordo = lado * GORDO_DEL_GRANO * 4.2;
            telar.raya(0.0, lado / 2.0, lado, lado / 2.0, gordo, 1.0);
        }

        // Por el medio: asi la raya cae entera en la junta en vez de
        // partirse por la mitad.
        MaterialTinta::Rayado | MaterialTinta::Cruzado => {
            let gordo = lado * GORDO_DEL_GRANO * 2.0;
            telar.raya(0.0, lado / 2.0, lado, lado / 2.0, gordo, 1.0);
            if material == MaterialTinta::Cruzado {
                telar.raya(lado / 2.0, 0.0, lado / 2.0, lado, gordo, 1.0);
            }
        }
    }
    telar.rematar()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// El maximo de una tela, para saber si hay algo tejido.
    fn lo_mas_tapado(t: &[u8]) -> u8 {
        t.iter().copied().max().unwrap_or(0)
    }

    #[test]
    fn las_palabras_del_fichero_son_las_del_movil_una_por_una() {
        // Si una sola deja de coincidir, un trazo hecho en el telefono se
        // abre aqui liso y vuelve liso: el campo lo manda ya el PC.
        let esperadas = [
            "lisa", "luz", "hdr", "rayado", "cruzado", "puntos", "tiza", "lapiz2b", "seco", "trama",
        ];
        let nuestras: Vec<&str> = MATERIALES.iter().map(|m| m.palabra()).collect();
        assert_eq!(nuestras, esperadas);
    }

    #[test]
    fn cada_palabra_vuelve_a_su_material_y_una_desconocida_no_se_inventa() {
        for m in MATERIALES {
            assert_eq!(MaterialTinta::desde_palabra(m.palabra()), Some(m));
        }
        // Caso negativo: un material de una version futura del movil no es
        // ninguno de los nuestros, y decirlo es lo que permite conservarlo.
        assert_eq!(MaterialTinta::desde_palabra("acuarela-del-futuro"), None);
        assert_eq!(MaterialTinta::desde_palabra(""), None);
        assert_eq!(MaterialTinta::desde_palabra("LISA"), None);
    }

    #[test]
    fn el_grafito_se_lee_y_se_escribe_como_cuadritos_aunque_no_este_en_el_panel() {
        // Es lo que llega del movil: sin esto un trazo de grafito se tomaba
        // por material desconocido y se pintaba liso.
        assert_eq!(
            MaterialTinta::desde_palabra("cuadritos"),
            Some(MaterialTinta::Cuadritos)
        );
        assert_eq!(MaterialTinta::Cuadritos.palabra(), "cuadritos");
        let texto = serde_json::to_string(&MaterialTinta::Cuadritos).unwrap();
        assert_eq!(texto, "\"cuadritos\"");
        // Caso negativo: el panel no lo ofrece como tinta (en el movil es una
        // herramienta) y «grafito» no es una palabra del fichero.
        assert!(!MATERIALES.contains(&MaterialTinta::Cuadritos));
        assert_eq!(MaterialTinta::desde_palabra("grafito"), None);
        // Ni lleva tela: su grano son los sellos.
        assert!(!MaterialTinta::Cuadritos.hay_grano());
        assert_eq!(lo_mas_tapado(&tejido(MaterialTinta::Cuadritos)), 0);
    }

    #[test]
    fn el_material_viaja_por_json_con_la_palabra_del_movil() {
        let texto = serde_json::to_string(&MaterialTinta::Lapiz2b).unwrap();
        assert_eq!(texto, "\"lapiz2b\"");
        let vuelta: MaterialTinta = serde_json::from_str("\"tiza\"").unwrap();
        assert_eq!(vuelta, MaterialTinta::Tiza);
    }

    #[test]
    fn la_lisa_es_la_de_por_omision_y_no_teje_nada() {
        assert_eq!(MaterialTinta::default(), MaterialTinta::Lisa);
        assert!(!MaterialTinta::Lisa.hay_grano());
        assert_eq!(lo_mas_tapado(&tejido(MaterialTinta::Lisa)), 0);
    }

    #[test]
    fn las_encendidas_no_llevan_grano_porque_lo_que_se_ve_es_el_resplandor() {
        // Caso negativo del grano: una luz con trama no es nada, un rayado
        // dentro de un resplandor no llega a la pantalla.
        for m in [MaterialTinta::Luz, MaterialTinta::Hdr] {
            assert!(m.alumbra());
            assert!(!m.hay_grano());
            assert_eq!(lo_mas_tapado(&tejido(m)), 0, "{m:?} tejio algo");
        }
    }

    #[test]
    fn las_porosas_pintan_el_cuerpo_a_menos_de_la_mitad() {
        // Con el cuerpo a plena tinta el grano se pierde encima de un trazo
        // macizo y las tres se ven iguales que la lisa.
        for m in [
            MaterialTinta::Tiza,
            MaterialTinta::Lapiz2b,
            MaterialTinta::Seco,
        ] {
            assert!(m.es_porosa(), "{m:?}");
            assert_eq!(m.cuerpo(), CUERPO_DE_LAS_POROSAS);
        }
        // Y las que no son porosas tapan del todo.
        for m in [
            MaterialTinta::Lisa,
            MaterialTinta::Rayado,
            MaterialTinta::Cruzado,
            MaterialTinta::Puntos,
            MaterialTinta::Trama,
        ] {
            assert!(!m.es_porosa(), "{m:?}");
            assert_eq!(m.cuerpo(), 1.0);
        }
    }

    #[test]
    fn todas_las_de_grano_tejen_algo_y_del_tamano_que_toca() {
        let lado = (LADO_DEL_MOSAICO * LADO_DEL_MOSAICO) as usize;
        for m in MATERIALES.into_iter().filter(|m| m.hay_grano()) {
            let t = tejido(m);
            assert_eq!(t.len(), lado, "{m:?}");
            assert!(lo_mas_tapado(&t) > 0, "{m:?} no tejio nada");
        }
    }

    #[test]
    fn tejer_dos_veces_da_exactamente_la_misma_tela() {
        // Es lo que impide que el trazo hierva al repintarse: el azar va con
        // semilla fija y la tela sale byte a byte igual siempre.
        for m in MATERIALES {
            assert_eq!(tejido(m), tejido(m), "{m:?} no es estable");
        }
    }

    #[test]
    fn el_azar_de_java_da_la_misma_secuencia_que_la_biblioteca_de_java() {
        // Los tres primeros `nextInt(100)` de `new Random(42)` en Java son
        // 30, 63 y 48, y los tres primeros `nextFloat()` de `new Random(0)`
        // son 0.73096776, 0.83144099 y 0.24053639. Sacados de la definicion
        // del propio `java.util.Random` (el congruencial de 48 bits que su
        // documentacion escribe entera) y cuadrados con el valor publicado de
        // `new Random(42).nextInt()`, que es -1170105035. Si esto cambia, las
        // motas de la tiza dejan de caer donde caen en el telefono.
        // El ancla publicada: `new Random(42).nextInt()`.
        assert_eq!(AzarJava::nuevo(42).siguiente(32), -1_170_105_035);
        let mut a = AzarJava::nuevo(42);
        assert_eq!([a.entero(100), a.entero(100), a.entero(100)], [30, 63, 48]);
        let mut b = AzarJava::nuevo(0);
        for esperado in [0.730_967_76_f32, 0.831_441, 0.240_536_39] {
            assert!((b.flotante() - esperado).abs() < 1e-6);
        }
    }

    /// Lo fino que es un grano: cuantas veces se pasa de tapado a destapado
    /// recorriendo la tela en linea. Muchas marcas pequenas dan muchos
    /// cambios; pocas y gordas, pocos.
    fn lo_fino_que_es(t: &[u8]) -> usize {
        let lado = LADO_DEL_MOSAICO as usize;
        let mut cambios = 0;
        for fila in 0..lado {
            for col in 1..lado {
                let a = t[fila * lado + col - 1] > 128;
                let b = t[fila * lado + col] > 128;
                if a != b {
                    cambios += 1;
                }
            }
        }
        cambios
    }

    #[test]
    fn el_lapiz_blando_tiene_el_grano_mas_fino_que_la_tiza() {
        // Es lo que los distingue, y no cuanto tapan —las dos tapan casi
        // igual—: la tiza lleva veintiseis motas gordas, asi que su tela va a
        // manchones; el lapiz blando lleva noventa finas y muy juntas, asi
        // que la suya cambia de tapado a destapado muchas mas veces. Eso es
        // «grano grueso y desigual» frente a «grano fino y muy tupido».
        let tiza = lo_fino_que_es(&tejido(MaterialTinta::Tiza));
        let lapiz = lo_fino_que_es(&tejido(MaterialTinta::Lapiz2b));
        assert!(lapiz > tiza, "tiza {tiza}, lapiz {lapiz}");
        // Y no son la misma tela con otro nombre: cada una lleva su semilla.
        assert_ne!(tejido(MaterialTinta::Tiza), tejido(MaterialTinta::Lapiz2b));
    }

    #[test]
    fn la_trama_gruesa_tapa_mas_que_el_rayado_fino() {
        // Para rellenos de plano que se leen de lejos, donde el rayado fino
        // se empasta.
        let rayado: u32 = tejido(MaterialTinta::Rayado)
            .iter()
            .map(|b| *b as u32)
            .sum();
        let trama: u32 = tejido(MaterialTinta::Trama).iter().map(|b| *b as u32).sum();
        assert!(trama > rayado, "rayado {rayado}, trama {trama}");
    }

    #[test]
    fn el_cruzado_tapa_mas_que_el_rayado_porque_lleva_las_dos_rayas() {
        let rayado: u32 = tejido(MaterialTinta::Rayado)
            .iter()
            .map(|b| *b as u32)
            .sum();
        let cruzado: u32 = tejido(MaterialTinta::Cruzado)
            .iter()
            .map(|b| *b as u32)
            .sum();
        assert!(cruzado > rayado);
    }

    #[test]
    fn el_rotulador_seco_deja_huecos_a_lo_largo() {
        // Lo que lo hace «seco» es que la raya sale cortada: si cada fila
        // estuviera entera, seria un rayado a lo largo y no un marcador
        // gastado.
        let t = tejido(MaterialTinta::Seco);
        let lado = LADO_DEL_MOSAICO as usize;
        let hay_fila_con_hueco = (0..lado).any(|fila| {
            let f = &t[fila * lado..(fila + 1) * lado];
            f.iter().any(|b| *b > 128) && f.contains(&0)
        });
        assert!(hay_fila_con_hueco, "ninguna fila tiene hueco");
    }

    #[test]
    fn solo_las_de_raya_se_inclinan() {
        for m in [
            MaterialTinta::Rayado,
            MaterialTinta::Cruzado,
            MaterialTinta::Trama,
        ] {
            assert!(m.se_inclina(), "{m:?}");
        }
        // El punteado y las motas no tienen direccion, y el rotulador seco
        // va a lo largo del trazo, que es como se descarga de verdad.
        for m in [
            MaterialTinta::Puntos,
            MaterialTinta::Tiza,
            MaterialTinta::Lapiz2b,
            MaterialTinta::Seco,
        ] {
            assert!(!m.se_inclina(), "{m:?}");
        }
    }

    use crate::elemento::ColorRgba;

    const AZUL: ColorRgba = ColorRgba::opaco(0.1, 0.2, 0.5);

    #[test]
    fn una_tinta_que_no_alumbra_no_cambia_de_color_por_mucha_luz_que_haya() {
        // Caso negativo, y el mas importante: la llave de las luces no puede
        // aclarar un plano entero. Solo mira a las dos encendidas.
        for m in MATERIALES.into_iter().filter(|m| !m.alumbra()) {
            assert_eq!(color_encendido(AZUL, m, 2.0, true), AZUL, "{m:?}");
        }
    }

    #[test]
    fn con_las_luces_apagadas_la_tinta_de_luz_es_tinta_lisa() {
        // No es un caso raro: es lo que promete el mando. Sin esto, una tinta
        // de luz apagada seguia siendo una raya lavada que no era el color de
        // nadie.
        let apagadas = LucesDelDibujo {
            encendidas: false,
            fuerza: 2.0,
        };
        assert_eq!(apagadas.cuanto(), 0.0);
        assert_eq!(
            color_encendido(AZUL, MaterialTinta::Luz, apagadas.cuanto(), true),
            AZUL
        );
    }

    #[test]
    fn encendida_la_tinta_se_va_hacia_su_tono_vivo_y_hacia_el_blanco_sin_perderlo() {
        // Las dos mitades: sube de brillo Y conserva que es azul. Una luz que
        // llegara a blanco puro dejaria de distinguirse de una roja.
        let c = color_encendido(AZUL, MaterialTinta::Luz, 1.0, false);
        assert!(c.r > AZUL.r && c.g > AZUL.g && c.b > AZUL.b, "{c:?}");
        assert!(c.b > c.g && c.g > c.r, "perdio el tono: {c:?}");
        assert!(c.r < 1.0 && c.g < 1.0, "llego al blanco puro: {c:?}");
        assert_eq!(c.a, AZUL.a, "la luz no toca la opacidad");
    }

    #[test]
    fn solo_la_hdr_se_sale_del_blanco_y_solo_si_hay_margen() {
        // La luz normal es la de siempre, acotada a lo que la pantalla sabe
        // dar; la HDR escribe por encima del uno.
        let luz = color_encendido(AZUL, MaterialTinta::Luz, 2.0, true);
        assert!(luz.r <= 1.0 && luz.g <= 1.0 && luz.b <= 1.0, "{luz:?}");
        let hdr = color_encendido(AZUL, MaterialTinta::Hdr, 2.0, true);
        assert!(hdr.b > 1.0, "la hdr no deslumbra: {hdr:?}");
        // Caso negativo: sin margen, la HDR se pinta como la luz normal. Ni se
        // intenta, que es lo que hace el movil.
        assert_eq!(color_encendido(AZUL, MaterialTinta::Hdr, 2.0, false), luz);
    }

    #[test]
    fn la_llave_se_topa_en_el_doble_y_no_baja_de_cero() {
        assert_eq!(
            LucesDelDibujo {
                encendidas: true,
                fuerza: 9.0
            }
            .cuanto(),
            LO_MAS_QUE_ALUMBRAN
        );
        assert_eq!(
            LucesDelDibujo {
                encendidas: true,
                fuerza: -3.0
            }
            .cuanto(),
            0.0
        );
        assert_eq!(LucesDelDibujo::default().cuanto(), 1.0);
    }

    #[test]
    fn una_tinta_negra_encendida_se_va_al_blanco_y_no_se_queda_negra() {
        // El caso que rompe la cuenta del tono vivo: el negro no tiene canal
        // al que subir, asi que dividir por su maximo seria dividir por cero.
        let negro = ColorRgba::opaco(0.0, 0.0, 0.0);
        let c = color_encendido(negro, MaterialTinta::Luz, 1.0, false);
        assert!(c.r > 0.9 && c.g > 0.9 && c.b > 0.9, "{c:?}");
        assert!(c.r.is_finite(), "{c:?}");
    }

    #[test]
    fn el_paso_sale_del_ancho_del_trazo_y_se_queda_dentro_de_sus_topes() {
        assert_eq!(paso_del_grano(10.0), 8.0);
        // Un trazo finisimo no puede apretar el mosaico hasta que no se vea.
        assert_eq!(paso_del_grano(0.0), PASO_MINIMO_DEL_GRANO);
        assert_eq!(paso_del_grano(-5.0), PASO_MINIMO_DEL_GRANO);
        // Ni uno enorme estirarlo hasta que el trazo sea un cuadro y medio.
        assert_eq!(paso_del_grano(1000.0), PASO_MAXIMO_DEL_GRANO);
        // Y no es finito solo por casualidad.
        assert!(paso_del_grano(f32::NAN).is_finite() || paso_del_grano(f32::NAN).is_nan());
    }
}
