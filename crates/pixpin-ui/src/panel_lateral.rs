//! El panel de propiedades de Excalidraw, a la izquierda del editor.
//!
//! Geometria pura: que secciones salen, donde cae cada control y que accion
//! corresponde a un clic. Quien lo pinta es el consumidor; quien aplica la
//! accion, el editor. Medidas de `docs/excalidraw/interfaz.md` §2.6: isla de
//! 200 px con 12 de relleno, titulos de 12 px, muestras de color de 22 y
//! botones de opcion de 32 separados 8, y 12 entre secciones.
//!
//! **Que secciones salen** sigue la regla de Excalidraw
//! (`shapeActionPredicates.ts`, `forToolOrSelection`): una seccion sale si
//! la herramienta la admite o si la admite ALGUNO de los elegidos, no solo
//! si la admiten todos. Con un rectangulo y un texto elegidos salen a la vez
//! el relleno y la letra; y al pulsar, cada cambio va solo a los que lo
//! admiten ([`destinatarios`]), asi que ponerle fondo a esa pareja no le
//! pinta un fondo al texto.
//!
//! El orden es el del JSX de `SelectedShapeActions` (`Actions.tsx`): trazo,
//! fondo, relleno, grosor, estilo, presion, trazo a mano, bordes, tipo de
//! flecha, fuente, tamano, puntas, opacidad, capas, alinear y acciones.
//!
//! Dos mandos abren un **desplegable** a la derecha del panel, como el
//! `PropertiesPopover` de Excalidraw: la muestra del color actual (la paleta
//! entera, los tonos y el codigo) y cada punta de flecha (las nueve puntas).
//! Si esta abierto o no lo guarda quien llama y lo pasa en
//! [`ContextoPanel::abierto`]; aqui solo se coloca.

use pixpin_geom::{Punto, Rect};
use pixpin_motor2d::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};
use pixpin_motor2d::estilo::{CambioEstilo, CambioForma, EstiloDibujo, NivelGrosor, TipoFlecha};
use pixpin_motor2d::formas::TipoPunta;
use pixpin_motor2d::gesto::Herramienta;
use pixpin_motor2d::organizar::{Alineacion, Reparto};
use pixpin_motor2d::relleno::EstiloRelleno;
use pixpin_motor2d::texto::{AlineacionTexto, AlineacionVertical};
use pixpin_motor2d::tinta::Variabilidad;

use crate::propiedades::{self, Propiedad};

// **Mas compacto que el de Excalidraw** (24-oct-2026, el usuario: «la barra
// de propiedades ocupa mucha de la visualizacion, redisenala y pon un boton
// para mostrarla y quitarla»): 168 de ancho en vez de 200, menos relleno y
// menos aire entre secciones, botones de 28. Siguen cuatro por fila.
/// Lo que ocupa a lo ancho una columna del panel.
pub const ANCHO_LOGICO: u32 = 168;
const RELLENO_LOGICO: u32 = 8;
const X_LOGICA: u32 = 16;
/// Debajo del boton de mostrar y quitar el panel ([`boton_plegar`]):
/// 16 + 36 + 8.
pub const Y_LOGICA: u32 = 60;
/// El boton de mostrar y quitar, donde Excalidraw pone su menu.
const PLEGAR_LOGICO: u32 = 36;
const TITULO_LOGICO: u32 = 14;
const TRAS_TITULO_LOGICO: u32 = 4;
const ENTRE_SECCIONES_LOGICO: u32 = 8;
const MUESTRA_LOGICA: u32 = 20;
const MUESTRA_ACTIVA_LOGICA: u32 = 24;
const OPCION_LOGICA: u32 = 28;
const HUECO_OPCION_LOGICO: u32 = 6;
const DESLIZADOR_LOGICO: u32 = 22;
/// La paleta del desplegable: casillas de 30 separadas 4
/// (`ColorPicker.scss:178-182`, `:301-306`).
const CASILLA_LOGICA: u32 = 30;
const HUECO_CASILLA_LOGICO: u32 = 4;
/// El campo del codigo hexadecimal, 32 de alto (`ColorPicker.scss:334-357`).
const HEX_LOGICO: u32 = 32;
/// Cada punta en su desplegable: ancha porque el icono es de 40 × 20.
const PUNTA_ANCHO_LOGICO: u32 = 48;
/// A cuanto del panel sale el desplegable (`sideOffset` menos el relleno
/// que ya separa el disparador del borde).
const SEPARACION_DESPLEGABLE_LOGICA: u32 = 8;
/// El desplegable arranca 16 por encima de su disparador (`alignOffset`).
const SUBIDA_DESPLEGABLE_LOGICA: u32 = 16;

/// Los colores rapidos de Excalidraw (`colors.ts`).
pub const COLORES_TRAZO: [ColorRgba; 5] = [
    hex(0x1e1e1e),
    hex(0xe03131),
    hex(0x2f9e44),
    hex(0x1971c2),
    hex(0xf08c00),
];
pub const COLORES_FONDO: [Option<ColorRgba>; 5] = [
    None,
    Some(hex(0xffc9c9)),
    Some(hex(0xb2f2bb)),
    Some(hex(0xa5d8ff)),
    Some(hex(0xffec99)),
];

/// Los colores rapidos del **papel del lienzo**, los de Excalidraw
/// (`DEFAULT_CANVAS_BACKGROUND_PICKS`, `colors.ts:284-294`): blanco y los
/// «2» de radix slate, blue, yellow y bronze. Muy claros a proposito: son
/// papel, y la tinta de fabrica (`#1e1e1e`) tiene que seguir leyendose.
pub const COLORES_LIENZO: [ColorRgba; 5] = [
    hex(0xffffff),
    hex(0xf8f9fa),
    hex(0xf5faff),
    hex(0xfffce8),
    hex(0xfdf8f6),
];

/// **Los papeles del movil**, en su orden (`DrawTheme.PAPELES`): los mismos
/// valores exactos, para que un lienzo que se pone «Crema» aqui llegue al
/// telefono como su «Crema» y no como un color parecido que ninguna de sus
/// muestras marca. Los cinco ultimos son oscuros: alla son el modo noche.
pub const PAPELES_DEL_MOVIL: [u32; 13] = [
    0xffffff, // Blanco
    0xfdf6e3, // Crema
    0xf5f1e8, // Hueso
    0xf1e7d0, // Sepia
    0xfbf6d9, // Amarillo
    0xe9f5ec, // Menta
    0xe8f1fb, // Azul claro
    0xd8dade, // Gris
    0x0b0f24, // Cosmos
    0x121212, // Pizarra
    0x14213d, // Azul noche
    0x1f3b33, // Verde pizarra
    0x000000, // Negro
];

const fn hex(rgb: u32) -> ColorRgba {
    ColorRgba::opaco(
        ((rgb >> 16) & 0xff) as f32 / 255.0,
        ((rgb >> 8) & 0xff) as f32 / 255.0,
        (rgb & 0xff) as f32 / 255.0,
    )
}

/// Una casilla de la paleta del desplegable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Casilla {
    Transparente,
    /// Blanco y negro: un solo tono.
    Fijo(u32),
    /// Los cinco tonos de open-color (pesos 50/200/400/600/800).
    Tonos([u32; 5]),
}

/// La paleta completa, rejilla de 5 × 3 en el orden de
/// `DEFAULT_ELEMENT_STROKE_COLOR_PALETTE` (`colors.ts:299-319`): la misma
/// para trazo y fondo.
pub const PALETA: [Casilla; 15] = [
    Casilla::Transparente,
    Casilla::Fijo(0xffffff),
    Casilla::Tonos([0xf8f9fa, 0xe9ecef, 0xced4da, 0x868e96, 0x343a40]),
    Casilla::Fijo(0x1e1e1e),
    Casilla::Tonos([0xf8f1ee, 0xeaddd7, 0xd2bab0, 0xa18072, 0x846358]),
    Casilla::Tonos([0xe3fafc, 0x99e9f2, 0x3bc9db, 0x15aabf, 0x0c8599]),
    Casilla::Tonos([0xe7f5ff, 0xa5d8ff, 0x4dabf7, 0x228be6, 0x1971c2]),
    Casilla::Tonos([0xf3f0ff, 0xd0bfff, 0x9775fa, 0x7950f2, 0x6741d9]),
    Casilla::Tonos([0xf8f0fc, 0xeebefa, 0xda77f2, 0xbe4bdb, 0x9c36b5]),
    Casilla::Tonos([0xfff0f6, 0xfcc2d7, 0xf783ac, 0xe64980, 0xc2255c]),
    Casilla::Tonos([0xebfbee, 0xb2f2bb, 0x69db7c, 0x40c057, 0x2f9e44]),
    Casilla::Tonos([0xe6fcf5, 0x96f2d7, 0x38d9a9, 0x12b886, 0x099268]),
    Casilla::Tonos([0xfff9db, 0xffec99, 0xffd43b, 0xfab005, 0xf08c00]),
    Casilla::Tonos([0xfff4e6, 0xffd8a8, 0xffa94d, 0xfd7e14, 0xe8590c]),
    Casilla::Tonos([0xfff5f5, 0xffc9c9, 0xff8787, 0xfa5252, 0xe03131]),
];

/// Las nueve puntas del `IconPicker` de Excalidraw, en su orden
/// (`actionProperties.tsx:1829-1930`). Las seis de cardinalidad no estan:
/// el motor no sabe pintarlas y ofrecerlas seria guardar una punta que en
/// pantalla no sale.
pub const PUNTAS_DEL_PANEL: [TipoPunta; 9] = [
    TipoPunta::Ninguna,
    TipoPunta::Flecha,
    TipoPunta::Triangulo,
    TipoPunta::TrianguloHueco,
    TipoPunta::Circulo,
    TipoPunta::CirculoHueco,
    TipoPunta::Rombo,
    TipoPunta::RomboHueco,
    TipoPunta::Barra,
];

/// Los cuatro tamanos de letra de Excalidraw (`FONT_SIZES`, `constants.ts`).
pub const TAMANOS_DE_LETRA: [f32; 4] = [16.0, 20.0, 28.0, 36.0];

/// Las ocho familias del panel, con sus numeros del fichero: las cuatro del
/// selector de Excalidraw (a mano, normal, Lilita One y codigo) y las cuatro
/// del lienzo de citas (normal, serif, maquina y manuscrita). El orden es el
/// del catalogo (`texto::FUENTES`), que es el que se ve.
pub const FAMILIAS: [u8; 8] = {
    let f = pixpin_motor2d::texto::FUENTES;
    [
        f[0].id, f[1].id, f[2].id, f[3].id, f[4].id, f[5].id, f[6].id, f[7].id,
    ]
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capa {
    Fondo,
    Atras,
    Adelante,
    Frente,
}

/// Que desplegable esta abierto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desplegable {
    ColorTrazo,
    ColorFondo,
    PuntaInicio,
    PuntaFin,
    /// La paleta del papel del lienzo.
    ColorLienzo,
}

/// Lo que hace un clic en el panel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AccionPanel {
    Estilo(CambioEstilo),
    /// Bordes, flecha, letra y presion: ver `estilo::CambioForma`.
    Forma(CambioForma),
    /// Abre el desplegable, o lo cierra si ya era ese el abierto.
    Abrir(Desplegable),
    Capa(Capa),
    Alinear(Alineacion),
    Repartir(Reparto),
    Duplicar,
    Borrar,
    Agrupar,
    Desagrupar,
    /// **Guardar lo elegido en Figuras** (22-oct): pide el nombre y lo pone
    /// en la biblioteca del menu de figuras. Lo atiende el editor, que es
    /// quien puede abrir el cajetin del nombre.
    GuardarFigura,
    /// Empieza a escribir el codigo del color en el campo del desplegable.
    /// Que color (trazo o fondo) lo dice el desplegable abierto.
    EditarHex,
    /// El papel del lienzo (`viewBackgroundColor`). No es un estilo: no va
    /// a lo elegido ni a lo proximo que se dibuje, va a la escena.
    FondoLienzo(ColorRgba),
    /// **De que serie salen las letras de los puntos** (`seriePuntos` del
    /// movil): A B C, a b c o 1 2 3. Es del dibujo entero y no de cada punto
    /// —en un croquis los vertices van en mayusculas y los lados en
    /// minusculas—, asi que no va a lo elegido: va al gesto, para los
    /// proximos puntos.
    SeriePunto(pixpin_motor2d::puntos_etiquetados::SerieDePunto),
    /// **Si la cota pide su medida al trazarla** (`pedirLaMedida` del movil):
    /// si o no. Como la serie, va al gesto y no a lo elegido.
    PedirMedida(bool),
    /// **El modo guia** (`modoReferencia` del movil): lo que se trace sera
    /// linea de referencia. Va al gesto (`Gesto::poner_modo_guia`).
    ModoGuia(bool),
    /// **Ver o esconder las lineas de referencia** (`referenciasVisibles`):
    /// va a la escena, que lo guarda en el fichero.
    VerGuias(bool),
    /// **Mostrar o quitar el panel** (24-oct-2026): el boton de arriba a
    /// la izquierda ([`boton_plegar`]). No es un estilo: lo guarda quien
    /// llama, que construye el panel [`PanelLateral::plegado`] o entero.
    Plegar,
}

/// El titulo de cada seccion. Lo traduce quien pinta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seccion {
    Trazo,
    Fondo,
    Relleno,
    Grosor,
    EstiloTrazo,
    Presion,
    TrazoAMano,
    Bordes,
    TipoFlecha,
    Fuente,
    TamanoFuente,
    /// Izquierda, centro y derecha; y debajo, si es el rotulo de una
    /// figura, arriba, en medio y abajo (Excalidraw pone la fila vertical
    /// sin titulo propio, justo debajo).
    AlineacionTexto,
    Puntas,
    Opacidad,
    Capas,
    Alinear,
    Acciones,
    /// Las del desplegable de color.
    Colores,
    Tonos,
    CodigoHex,
    /// «Fondo del lienzo» (`labels.canvasBackground`).
    FondoLienzo,
    /// Los papeles del movil, en la paleta del papel.
    PapelesDelMovil,
    /// Las letras de los puntos: A B C, a b c o 1 2 3.
    SerieDePunto,
    /// Si al trazar una cota se pide su medida.
    PedirMedida,
    /// La dureza del grafito: 4H ... 8B (`Propiedad.DUREZA` del movil).
    Dureza,
    /// El modo guia y el ojo de las lineas de referencia.
    Guias,
    /// Pixelar o desenfocar un mosaico (`Propiedad.MOSAICO` del movil).
    Mosaico,
    /// Cuanto agranda una lupa (`Propiedad.LUPA`).
    AumentoLupa,
    /// Con que senala una lupa de donde sale lo que ensena.
    GuiaLupa,
    /// Cuanto oscurece un foco (`Propiedad.OSCURECER`).
    Oscurecer,
    /// Que parte de su marco ilumina un foco (`Propiedad.ZONA`).
    ZonaFoco,
    /// Relleno liso, degradado lineal o radial (laminas de geometria).
    Degradado,
    /// Arco abierto, sector o segmento.
    CierreArco,
    /// Como traza la herramienta Arco: guia, centro y dos puntos, tres puntos.
    ModoArco,
    /// Cuantos arquitos lleva una marca de angulo.
    ArcosAngulo,
    /// Que pone la herramienta Angulo: letra griega, la marca sola o el
    /// cuadradito de 90.
    ModoAngulo,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Control {
    Titulo {
        seccion: Seccion,
        rect: Rect,
    },
    /// Una muestra de color. `color: None` es transparente. `grande` es la
    /// del color actual, al final de la fila, que abre la paleta.
    Muestra {
        rect: Rect,
        color: Option<ColorRgba>,
        activa: bool,
        accion: Option<AccionPanel>,
        grande: bool,
    },
    /// Un boton con icono: el consumidor elige el icono por la accion.
    Opcion {
        rect: Rect,
        accion: AccionPanel,
        activa: bool,
    },
    /// Un boton con el dibujo de una punta de flecha. `punta: None` es una
    /// seleccion con puntas distintas; `al_inicio` pide pintarla mirando a
    /// la izquierda, como Excalidraw voltea las del principio.
    Punta {
        rect: Rect,
        accion: AccionPanel,
        punta: Option<TipoPunta>,
        al_inicio: bool,
        activa: bool,
    },
    /// La barra de opacidad: `rect` es la pista entera; `valor` de 0 a 1.
    Deslizador {
        rect: Rect,
        valor: f32,
    },
    /// El codigo hexadecimal del color actual. Pulsarlo empieza a escribir
    /// en el ([`AccionPanel::EditarHex`]); mientras se escribe, el teclado es
    /// del campo y no del lienzo, y lo escrito lo guarda quien pinta.
    Hex {
        rect: Rect,
        color: Option<ColorRgba>,
    },
}

impl Control {
    pub fn rect(&self) -> Rect {
        match *self {
            Control::Titulo { rect, .. }
            | Control::Muestra { rect, .. }
            | Control::Opcion { rect, .. }
            | Control::Punta { rect, .. }
            | Control::Deslizador { rect, .. }
            | Control::Hex { rect, .. } => rect,
        }
    }

    /// El mismo control, corrido `dx`, `dy`.
    fn movido(mut self, dx: i32, dy: i32) -> Control {
        match &mut self {
            Control::Titulo { rect, .. }
            | Control::Muestra { rect, .. }
            | Control::Opcion { rect, .. }
            | Control::Punta { rect, .. }
            | Control::Deslizador { rect, .. }
            | Control::Hex { rect, .. } => {
                rect.x += dx;
                rect.y += dy;
            }
        }
        self
    }

    fn accion(&self) -> Option<AccionPanel> {
        match *self {
            Control::Muestra { accion, .. } => accion,
            Control::Opcion { accion, .. } | Control::Punta { accion, .. } => Some(accion),
            Control::Hex { .. } => Some(AccionPanel::EditarHex),
            _ => None,
        }
    }
}

/// Una seccion que solo sale para algunos tipos: si sale, y que boton va
/// marcado.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Mando<T> {
    /// Ni la herramienta ni nada de lo elegido la admite.
    #[default]
    Fuera,
    /// Sale, pero lo elegido tiene valores distintos: no se marca ninguno
    /// (el `null` de `getFormValue` en Excalidraw).
    Mixto,
    Vale(T),
}

impl<T: Copy + PartialEq> Mando<T> {
    pub fn sale(&self) -> bool {
        !matches!(self, Mando::Fuera)
    }

    fn valor(&self) -> Option<T> {
        match *self {
            Mando::Vale(v) => Some(v),
            _ => None,
        }
    }

    /// Anade el valor de un elemento mas.
    fn juntar(self, v: T) -> Self {
        match self {
            Mando::Fuera => Mando::Vale(v),
            Mando::Vale(a) if a == v => Mando::Vale(a),
            _ => Mando::Mixto,
        }
    }
}

/// Las secciones que no son del estilo comun, con su valor.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Mandos {
    /// Si hay un fondo que rayar: sin el, la seccion de relleno sobra.
    pub con_fondo: bool,
    pub presion: Mando<Variabilidad>,
    /// `true` = redondo.
    pub bordes: Mando<bool>,
    /// Afilada, curva o de codos.
    pub tipo_flecha: Mando<TipoFlecha>,
    pub punta_inicio: Mando<TipoPunta>,
    pub punta_fin: Mando<TipoPunta>,
    pub familia: Mando<u8>,
    pub tamano: Mando<f32>,
    /// Los renglones: sale con la herramienta de texto o un texto elegido.
    pub alineacion: Mando<AlineacionTexto>,
    /// Arriba, en medio o abajo: solo con el rotulo de una figura elegido
    /// (`shouldAllowVerticalAlign`), que es el unico que tiene donde subir.
    pub alineacion_vertical: Mando<AlineacionVertical>,
    pub agrupar: bool,
    pub desagrupar: bool,
    /// La serie de las letras de los puntos: solo con la herramienta de
    /// punto en la mano (ver [`Mandos::con_serie_de_punto`]).
    pub serie_punto: Mando<pixpin_motor2d::puntos_etiquetados::SerieDePunto>,
    /// Si la cota pide su medida: solo con la cota en la mano.
    pub pedir_medida: Mando<bool>,
    /// **La dureza del grafito**: con el grafito en la mano o algo de
    /// grafito elegido (`propiedadesPara(GRAFITO)` y `propiedadesDe` del
    /// movil: solo lo que es de `cuadritos`).
    pub dureza: Mando<pixpin_motor2d::tinta::grafito::Dureza>,
    /// **El modo guia** (si esta puesto) y, si hay alguna guia, **si se ven**
    /// (`None`: no hay ninguna, y el ojo no sale, como en el movil). Lo pone
    /// quien sabe del gesto y de la escena: [`Mandos::con_guias`].
    pub guias: Option<(bool, Option<bool>)>,
    /// Pixelar (`false`) o desenfocar (`true`): con el mosaico en la mano o
    /// un mosaico elegido.
    pub mosaico: Mando<bool>,
    /// El aumento y la guia de las lupas elegidas. Con la herramienta no:
    /// la Lupa del anotador de pantalla es la lupa viva, que no es esto.
    pub lupa_aumento: Mando<f32>,
    pub lupa_guia: Mando<pixpin_motor2d::lupa_elemento::GuiaDeLupa>,
    /// Cuanto oscurece y que parte ilumina el foco elegido (en por ciento
    /// y en fraccion de su marco). Como la lupa, solo con uno elegido.
    pub foco_oscurecer: Mando<u8>,
    pub foco_zona: Mando<f32>,
    /// Liso (`None`) o degradado (laminas de geometria).
    pub degradado: Mando<Option<pixpin_motor2d::geometria_exacta::TipoDegradado>>,
    /// Abierto (`None`), sector o segmento.
    pub cierre_arco: Mando<Option<pixpin_motor2d::geometria_exacta::CierreArco>>,
    pub modo_arco: Mando<pixpin_motor2d::geometria_exacta::ModoArco>,
    pub arcos_angulo: Mando<u8>,
    pub modo_angulo: Mando<pixpin_motor2d::geometria_exacta::ModoAngulo>,
}

/// Que modo de la herramienta Angulo dejaria una marca asi: el cuadradito es
/// el recto, con letra la griega, y si no la sola.
pub fn modo_de_marca(
    m: &pixpin_motor2d::geometria_exacta::MarcaAngulo,
) -> pixpin_motor2d::geometria_exacta::ModoAngulo {
    use pixpin_motor2d::geometria_exacta::ModoAngulo as M;
    if m.recto {
        M::Recto
    } else if m.valor.as_deref().is_some_and(|v| !v.trim().is_empty()) {
        M::Griega
    } else {
        M::Simple
    }
}

impl Mandos {
    /// **Numerar los puntos**: con la herramienta de punto en la mano sale la
    /// fila A B C / a b c / 1 2 3, marcada con la que tiene el gesto. Es
    /// donde uno mira cuando decide si lo que va a plantar son vertices,
    /// lados o una nube numerada (`DrawToolbar.kt`, junto a la herramienta).
    /// La serie la guarda el gesto (`Gesto::serie_de_punto`), no el estilo:
    /// por eso se anade aparte y no en [`Mandos::de_herramienta`].
    pub fn con_serie_de_punto(
        mut self,
        h: Herramienta,
        serie: pixpin_motor2d::puntos_etiquetados::SerieDePunto,
    ) -> Mandos {
        self.serie_punto = if h == Herramienta::Punto {
            Mando::Vale(serie)
        } else {
            Mando::Fuera
        };
        self
    }

    /// **El modo guia y el ojo de las guias**: salen con cualquier
    /// herramienta y con lo elegido, porque cualquier herramienta traza guias.
    /// `visibles` es `None` si no hay ninguna guia (el ojo no sale); y todo
    /// `None` si la seccion no toca.
    pub fn con_guias(mut self, guias: Option<(bool, Option<bool>)>) -> Mandos {
        self.guias = guias;
        self
    }

    /// **La dureza de la mina**, solo con el grafito en la mano (la de lo
    /// proximo, `estilo.dureza`); con algo elegido la pone
    /// [`Mandos::de_seleccion`].
    pub fn con_dureza(
        mut self,
        h: Herramienta,
        dureza: pixpin_motor2d::tinta::grafito::Dureza,
    ) -> Mandos {
        if h == Herramienta::Grafito {
            self.dureza = Mando::Vale(dureza);
        }
        self
    }

    /// **Pedir la medida al trazar la cota**: si o no, solo con la cota en la
    /// mano (el interruptor que el movil pone junto a la herramienta,
    /// `DrawToolbar.kt`). Lo guarda el gesto (`Gesto::pedir_la_medida`).
    pub fn con_pedir_la_medida(mut self, h: Herramienta, pedir: bool) -> Mandos {
        self.pedir_medida = if h == Herramienta::Cota {
            Mando::Vale(pedir)
        } else {
            Mando::Fuera
        };
        self
    }

    /// Sin nada elegido: lo que se ajusta antes de dibujar.
    ///
    /// Como en Excalidraw, bordes, flecha y letra salen tambien aqui, con el
    /// valor «actual» marcado: `EstiloDibujo` los guarda
    /// (`currentItemRoundness` y compania) y `gesto::nuevo_elemento` hace
    /// nacer la figura con ellos. Antes no salian, y era lo que el usuario
    /// vio: con la figura elegida el boton de bordes estaba; con la
    /// herramienta puesta, no. La presion la guarda el gesto aparte
    /// (`Gesto::variabilidad`).
    pub fn de_herramienta(
        h: Herramienta,
        estilo: &EstiloDibujo,
        variabilidad: Variabilidad,
    ) -> Mandos {
        let rayable = propiedades::de_herramienta(h).contains(&Propiedad::EstiloRelleno);
        fn si<T>(cond: bool, v: T) -> Mando<T> {
            if cond { Mando::Vale(v) } else { Mando::Fuera }
        }
        let flecha = matches!(
            h,
            Herramienta::Flecha | Herramienta::FlechaLibre | Herramienta::FlechaCodos
        );
        let texto = h == Herramienta::Texto;
        let rotula = matches!(
            h,
            Herramienta::Serie
                | Herramienta::Cota
                | Herramienta::Punto
                | Herramienta::EscalaGrafica
        );
        Mandos {
            // El bote nunca pinta transparente, asi que su relleno siempre
            // tiene algo que rayar (`shapeActionPredicates.ts:130-139`).
            con_fondo: h == Herramienta::Relleno || (rayable && estilo.relleno.is_some()),
            // El grafito es el mismo trazo que el lapiz (un `freedraw`), y
            // la presion le cambia igual el grueso y la carga de los sellos.
            presion: si(
                matches!(h, Herramienta::Lapiz | Herramienta::Grafito),
                variabilidad,
            ),
            bordes: si(
                matches!(h, Herramienta::Rectangulo | Herramienta::Rombo),
                estilo.redondo,
            ),
            // Solo la flecha recta cambia de tipo: la de codos lo es siempre
            // y la libre sigue la mano, asi que el boton no les haria nada.
            tipo_flecha: si(
                h == Herramienta::Flecha,
                if estilo.codos {
                    TipoFlecha::Codos
                } else if estilo.curva {
                    TipoFlecha::Curva
                } else {
                    TipoFlecha::Afilada
                },
            ),
            // La vertical no: sin nada elegido no hay figura en la que
            // subir o bajar el rotulo, igual que en Excalidraw.
            alineacion: si(texto, estilo.alineacion),
            punta_inicio: si(flecha, estilo.punta_inicio),
            punta_fin: si(flecha, estilo.punta_fin),
            // **La letra en todo lo que la lleva** (pedido del usuario): el
            // texto y lo que nace con rotulo —el numero de serie, la cota, el
            // punto—. El tamano, el texto y el numero (su circulo sale de el).
            familia: si(
                texto || rotula,
                estilo
                    .familia
                    .unwrap_or(pixpin_motor2d::texto::FUENTE_EXCALIFONT),
            ),
            tamano: si(texto || h == Herramienta::Serie, estilo.tamano_letra),
            mosaico: si(h == Herramienta::Mosaico, estilo.desenfoque),
            // Solo con algo que rellenar (como el rayado): sin fondo, un
            // mando de degradado no cambiaria nada de lo que se dibuja.
            degradado: si(
                h == Herramienta::Relleno
                    || (matches!(
                        h,
                        Herramienta::Rectangulo
                            | Herramienta::Rombo
                            | Herramienta::Elipse
                            | Herramienta::Arco
                    ) && (estilo.relleno.is_some() || estilo.degradado.is_some())),
                estilo.degradado,
            ),
            cierre_arco: si(h == Herramienta::Arco, estilo.cierre_arco),
            modo_arco: si(h == Herramienta::Arco, estilo.modo_arco),
            arcos_angulo: si(h == Herramienta::Angulo, estilo.arcos_angulo),
            modo_angulo: si(h == Herramienta::Angulo, estilo.modo_angulo),
            ..Mandos::default()
        }
    }

    /// Con algo elegido: cada seccion sale si ALGUNO la admite.
    pub fn de_seleccion(elementos: &[&Elemento]) -> Mandos {
        let mut m = Mandos::default();
        for e in elementos {
            let p = propiedades::de_figura(&e.figura);
            if p.contains(&Propiedad::EstiloRelleno) && e.relleno.is_some() {
                m.con_fondo = true;
            }
            if let Some(v) = pixpin_motor2d::estilo::variabilidad_de(e) {
                m.presion = m.presion.juntar(v);
            }
            if (CambioForma::Degradado(None)).admite(&e.figura)
                && e.extras.marca_angulo.is_none()
                && (e.relleno.is_some() || e.extras.degradado.is_some())
            {
                m.degradado = m.degradado.juntar(e.extras.degradado.map(|d| d.tipo));
            }
            if e.material.es_grafito() {
                m.dureza = m
                    .dureza
                    .juntar(pixpin_motor2d::tinta::grafito::Dureza::de(e));
            }
            if (CambioForma::Bordes { redondo: false }).admite(&e.figura) {
                m.bordes = m.bordes.juntar(e.redondo);
            }
            match &e.figura {
                Figura::Flecha {
                    punta_inicio,
                    punta_fin,
                    ..
                } => {
                    if let Some(t) = TipoFlecha::de(e) {
                        m.tipo_flecha = m.tipo_flecha.juntar(t);
                    }
                    m.punta_inicio = m.punta_inicio.juntar(*punta_inicio);
                    m.punta_fin = m.punta_fin.juntar(*punta_fin);
                }
                Figura::Texto { tam, familia, .. } => {
                    m.familia = m
                        .familia
                        .juntar(pixpin_motor2d::texto::numero_de_familia(familia));
                    m.tamano = m.tamano.juntar(*tam);
                    // Lo que se ve: sin `textAlign` se pinta a la izquierda.
                    m.alineacion = m.alineacion.juntar(e.extras.alineacion.unwrap_or_default());
                    if e.extras.contenedor.is_some() {
                        m.alineacion_vertical = m
                            .alineacion_vertical
                            .juntar(e.extras.alineacion_vertical.unwrap_or_default());
                    }
                }
                // Lo que rotula sin ser un texto (el numero, la cota, el
                // punto, la escala): su letra en los extras.
                // Sin elegir aun (la del sistema) sale la fila sin marcar.
                f if pixpin_motor2d::estilo::lleva_letra(f) => {
                    m.familia = match &e.extras.familia {
                        Some(n) => m
                            .familia
                            .juntar(pixpin_motor2d::texto::numero_de_familia(n)),
                        None => Mando::Mixto,
                    };
                    if let Figura::Serie { .. } = f {
                        m.tamano = m.tamano.juntar(
                            (e.ancho.min(e.alto) / (2.0 * pixpin_motor2d::serie::RADIO_POR_LETRA))
                                .round(),
                        );
                    }
                }
                Figura::Mosaico { desenfoque } => m.mosaico = m.mosaico.juntar(*desenfoque),
                // El arco de lamina: su cierre, o sus arquitos si es la marca
                // de un angulo (el cuadradito no tiene arquitos que contar).
                Figura::Arco {
                    barrido: Some(_), ..
                } => match &e.extras.marca_angulo {
                    Some(marca) => {
                        if !marca.recto {
                            m.arcos_angulo = m.arcos_angulo.juntar(marca.arcos);
                        }
                        m.modo_angulo = m.modo_angulo.juntar(modo_de_marca(marca));
                    }
                    None => m.cierre_arco = m.cierre_arco.juntar(e.extras.cierre),
                },
                Figura::Foco { cristal } => {
                    use pixpin_motor2d::lupa_elemento as l;
                    m.foco_oscurecer = m.foco_oscurecer.juntar(l::oscurecimiento_de(cristal));
                    let z = l::zona_de(cristal, l::caja_de(e));
                    m.foco_zona = m.foco_zona.juntar((z * 100.0).round() / 100.0);
                }
                Figura::Lupa { cristal } => {
                    let caja = pixpin_motor2d::lupa_elemento::caja_de(e);
                    let z = pixpin_motor2d::lupa_elemento::aumento_de(cristal, caja);
                    m.lupa_aumento = m.lupa_aumento.juntar((z * 10.0).round() / 10.0);
                    m.lupa_guia = m.lupa_guia.juntar(cristal.guia);
                }
                _ => {}
            }
        }
        // Agrupar, con dos o mas que no sean ya un mismo grupo; desagrupar,
        // si alguno esta en un grupo (`actionGroup.tsx`).
        let mismo_grupo = elementos
            .first()
            .and_then(|e| e.grupos.last())
            .is_some_and(|g| {
                elementos
                    .iter()
                    .all(|e| e.grupos.last().is_some_and(|h| h == g))
            });
        m.agrupar = elementos.len() >= 2 && !mismo_grupo;
        m.desagrupar = elementos.iter().any(|e| !e.grupos.is_empty());
        m
    }
}

/// Lo que se puede ajustar con esto elegido: lo que admite ALGUNO, en el
/// orden de [`Propiedad`] y no en el de la seleccion, para que los mandos no
/// bailen de sitio al elegir en otro orden.
pub fn propiedades_de_seleccion(elementos: &[&Elemento]) -> Vec<Propiedad> {
    let mut todas: Vec<Propiedad> = elementos
        .iter()
        .flat_map(|e| propiedades::de_figura(&e.figura).iter().copied())
        .collect();
    todas.sort_unstable();
    todas.dedup();
    todas
}

/// El estilo que marca el panel con esto elegido. Cada valor sale del
/// primero que TIENE esa propiedad: con un texto y un rectangulo, el grosor
/// marcado es el del rectangulo, no el de un texto que no tiene grosor.
pub fn estilo_de_seleccion(elementos: &[&Elemento]) -> Option<EstiloDibujo> {
    let primero = elementos.first()?;
    let de = |p: Propiedad| {
        elementos
            .iter()
            .find(|e| propiedades::de_figura(&e.figura).contains(&p))
            .unwrap_or(primero)
    };
    let g = de(Propiedad::Grosor);
    Some(EstiloDibujo {
        trazo: de(Propiedad::ColorTrazo).trazo,
        relleno: de(Propiedad::Relleno).relleno,
        estilo_relleno: de(Propiedad::EstiloRelleno).estilo_relleno,
        grosor: NivelGrosor::de_elemento(&g.figura, g.grosor),
        estilo: de(Propiedad::Estilo).estilo,
        rugosidad: de(Propiedad::Rugosidad).rugosidad,
        opacidad: de(Propiedad::Opacidad).opacidad,
        material: primero.material,
        // Bordes, flecha y letra los marca `Mandos::de_seleccion`, que sabe
        // cuando lo elegido los tiene mezclados; aqui no se usan.
        ..EstiloDibujo::default()
    })
}

/// Si el elemento admite la accion. Lo que no es un cambio de estilo ni de
/// forma (capas, alinear...) lo admite todo.
pub fn admite(accion: &AccionPanel, e: &Elemento) -> bool {
    let tiene = |p| propiedades::de_figura(&e.figura).contains(&p);
    match accion {
        AccionPanel::Estilo(c) => match c {
            CambioEstilo::Trazo(_) => tiene(Propiedad::ColorTrazo),
            CambioEstilo::Relleno(_) => tiene(Propiedad::Relleno),
            CambioEstilo::EstiloRelleno(_) => tiene(Propiedad::EstiloRelleno),
            CambioEstilo::Grosor(_) => tiene(Propiedad::Grosor),
            CambioEstilo::Estilo(_) => tiene(Propiedad::Estilo),
            CambioEstilo::Rugosidad(_) => tiene(Propiedad::Rugosidad),
            CambioEstilo::Opacidad(_) => tiene(Propiedad::Opacidad),
            // El material no es mando del panel de Excalidraw ni de la
            // tabla de la interfaz: lo decide quien lo pide.
            CambioEstilo::Material(_) => true,
            // La dureza, solo a lo que es de grafito.
            CambioEstilo::Dureza(_) => e.material.es_grafito(),
        },
        // La vertical, solo el rotulo de una figura: a un texto suelto no le
        // cambiaria nada de lo que se ve, y un boton que no cambia nada es un
        // boton que miente (la misma regla de `shouldAllowVerticalAlign`).
        AccionPanel::Forma(c @ CambioForma::AlineacionVertical(_)) => {
            c.admite(&e.figura) && e.extras.contenedor.is_some()
        }
        AccionPanel::Forma(c) => c.admite(&e.figura),
        _ => true,
    }
}

/// A quienes de lo elegido les llega la accion.
///
/// Con la regla de «sale si alguno la admite», una seleccion mezclada
/// ensena mandos que no todos tienen; esto es lo que impide que ponerle
/// fondo a un rectangulo y un texto le pinte tambien un fondo al texto.
pub fn destinatarios(accion: &AccionPanel, elementos: &[&Elemento]) -> Vec<u64> {
    elementos
        .iter()
        .filter(|e| admite(accion, e))
        .map(|e| e.id)
        .collect()
}

/// Lo que el panel necesita saber para montarse.
#[derive(Debug, Clone, Copy)]
pub struct ContextoPanel<'a> {
    /// Las propiedades que aplican (de la herramienta o de lo elegido).
    pub propiedades: &'a [Propiedad],
    /// Los valores que se marcan como activos.
    pub estilo: EstiloDibujo,
    pub seleccionados: usize,
    /// Las secciones que dependen del tipo: presion, bordes, flecha, letra.
    pub mandos: Mandos,
    /// El desplegable abierto, si hay alguno.
    pub abierto: Option<Desplegable>,
    /// El papel del lienzo, **si se ha pedido elegirlo** (el «Fondo del
    /// lienzo» del menu). Con `Some` el panel es solo eso: la isla hace de
    /// menu del papel, como la entrada del menu principal de Excalidraw, y
    /// no se mezcla con las propiedades de lo elegido.
    pub lienzo: Option<ColorRgba>,
}

/// A quien le toca un punto.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DestinoPanel {
    Accion(AccionPanel),
    /// Dentro del panel pero en ningun control: no llega al lienzo.
    Panel,
    Fuera,
}

/// El desplegable, ya colocado.
#[derive(Debug, Clone, PartialEq)]
pub struct Emergente {
    pub cual: Desplegable,
    pub marco: Rect,
    pub controles: Vec<Control>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PanelLateral {
    pub marco: Rect,
    pub controles: Vec<Control>,
    /// Encima del lienzo y del panel, a su derecha.
    pub emergente: Option<Emergente>,
    /// El boton de mostrar y quitar el panel ([`AccionPanel::Plegar`]), si
    /// lo lleva. Va fuera del `marco`, encima.
    pub plegar: Option<Rect>,
    /// Quitado: solo queda el boton; `marco` es el suyo y no hay controles.
    pub plegado: bool,
}

/// **Donde va el boton de mostrar y quitar el panel**: arriba a la
/// izquierda del `area`, encima de donde empieza el panel.
pub fn boton_plegar(area: Rect, escala_por_cien: u32) -> Rect {
    let e = |v: u32| (v * escala_por_cien / 100) as i32;
    Rect {
        x: area.x + e(X_LOGICA),
        y: area.y + e(X_LOGICA),
        ancho: e(PLEGAR_LOGICO) as u32,
        alto: e(PLEGAR_LOGICO) as u32,
    }
}

fn casi(a: ColorRgba, b: ColorRgba) -> bool {
    (a.r - b.r).abs() < 0.01 && (a.g - b.g).abs() < 0.01 && (a.b - b.b).abs() < 0.01
}

fn mismo(a: Option<ColorRgba>, b: Option<ColorRgba>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => casi(a, b),
        _ => false,
    }
}

/// Va colocando secciones de arriba abajo.
///
/// **Si no cabe de alto, abre otra columna.** Excalidraw deja que su panel
/// se desplace (`overflow-y: auto`); aqui la rueda del raton es del lienzo, y
/// una seccion cortada por el borde de la pantalla es una seccion que no se
/// puede pulsar. Con varias cosas distintas elegidas (una caja, un texto,
/// una flecha y un trazo) el panel pasa de 1.000 px, asi que en vez de
/// cortarlo se ensancha: la seccion que no cabe entera pasa arriba de la
/// columna siguiente.
struct Colocador {
    e: u32,
    dentro: i32,
    ancho_util: i32,
    y: i32,
    controles: Vec<Control>,
    /// Donde empieza cada columna.
    arriba: i32,
    /// Hasta donde puede bajar una seccion antes de pasar de columna.
    limite: i32,
    /// Lo que se corre cada columna nueva.
    paso_columna: i32,
    columnas: i32,
    /// Donde empezo la seccion en curso: su primer control y su altura.
    inicio: (usize, i32),
    /// Lo mas bajo que ha llegado alguna columna.
    y_max: i32,
}

impl Colocador {
    /// Uno de una sola columna, sin limite de alto (el desplegable).
    fn sin_limite(e: u32, dentro: i32, ancho_util: i32, y: i32) -> Colocador {
        Colocador {
            e,
            dentro,
            ancho_util,
            y,
            controles: Vec::new(),
            arriba: y,
            limite: i32::MAX,
            paso_columna: 0,
            columnas: 1,
            inicio: (0, y),
            y_max: y,
        }
    }

    fn px(&self, v: u32) -> i32 {
        (v * self.e / 100) as i32
    }

    fn titulo(&mut self, seccion: Seccion) {
        self.inicio = (self.controles.len(), self.y);
        let alto = self.px(TITULO_LOGICO);
        self.controles.push(Control::Titulo {
            seccion,
            rect: Rect {
                x: self.dentro,
                y: self.y,
                ancho: self.ancho_util as u32,
                alto: alto as u32,
            },
        });
        self.y += alto + self.px(TRAS_TITULO_LOGICO);
    }

    /// Una rejilla de celdas de `ancho × alto` separadas `hueco`, de
    /// izquierda a derecha y partiendo fila cuando no caben. `hacer` recibe
    /// el indice y el sitio y devuelve el control, o `None` para dejar el
    /// hueco vacio sin mover a los demas (asi un color escondido no corre
    /// de sitio al resto de la paleta, como en `PickerColorList.tsx`).
    fn rejilla(
        &mut self,
        n: usize,
        (ancho, alto, hueco): (i32, i32, i32),
        mut hacer: impl FnMut(usize, Rect) -> Option<Control>,
    ) {
        if n == 0 {
            return;
        }
        let por_fila = ((self.ancho_util + hueco) / (ancho + hueco)).max(1) as usize;
        for k in 0..n {
            let (col, fil) = ((k % por_fila) as i32, (k / por_fila) as i32);
            let rect = Rect {
                x: self.dentro + col * (ancho + hueco),
                y: self.y + fil * (alto + hueco),
                ancho: ancho as u32,
                alto: alto as u32,
            };
            if let Some(c) = hacer(k, rect) {
                self.controles.push(c);
            }
        }
        let filas = n.div_ceil(por_fila) as i32;
        self.y += filas * alto + (filas - 1).max(0) * hueco;
    }

    /// Una fila de botones de opcion; parte en varias si no caben.
    fn fila(&mut self, opciones: &[(AccionPanel, bool)]) {
        let lado = self.px(OPCION_LOGICA);
        let hueco = self.px(HUECO_OPCION_LOGICO);
        self.rejilla(opciones.len(), (lado, lado, hueco), |k, rect| {
            let (accion, activa) = opciones[k];
            Some(Control::Opcion {
                rect,
                accion,
                activa,
            })
        });
    }

    /// Rejilla de Excalidraw `1fr 20px 26px`: los colores rapidos a la
    /// izquierda y el actual, algo mayor, al final. El actual abre la paleta.
    fn colores(
        &mut self,
        lista: &[(Option<ColorRgba>, AccionPanel)],
        actual: Option<ColorRgba>,
        abre: Desplegable,
        abierto: bool,
    ) {
        let activa = self.px(MUESTRA_ACTIVA_LOGICA);
        let muestra = self.px(MUESTRA_LOGICA);
        let zona = self.ancho_util - activa - self.px(20);
        let paso = (zona - muestra) / (lista.len() as i32 - 1).max(1);
        let arriba = self.y + (activa - muestra) / 2;
        for (k, (color, accion)) in lista.iter().enumerate() {
            self.controles.push(Control::Muestra {
                rect: Rect {
                    x: self.dentro + k as i32 * paso,
                    y: arriba,
                    ancho: muestra as u32,
                    alto: muestra as u32,
                },
                color: *color,
                activa: mismo(*color, actual),
                accion: Some(*accion),
                grande: false,
            });
        }
        self.controles.push(Control::Muestra {
            rect: Rect {
                x: self.dentro + self.ancho_util - activa,
                y: self.y,
                ancho: activa as u32,
                alto: activa as u32,
            },
            color: actual,
            activa: abierto,
            accion: Some(AccionPanel::Abrir(abre)),
            grande: true,
        });
        self.y += activa;
    }

    fn deslizador(&mut self, valor: f32) {
        let alto = self.px(DESLIZADOR_LOGICO);
        self.controles.push(Control::Deslizador {
            rect: Rect {
                x: self.dentro,
                y: self.y,
                ancho: self.ancho_util as u32,
                alto: alto as u32,
            },
            valor,
        });
        self.y += alto;
    }

    /// Cierra la seccion en curso. Si se ha salido por abajo, y no era ya la
    /// primera de su columna (que no cabria en ninguna), la pasa entera
    /// arriba de la columna siguiente.
    fn separar(&mut self) {
        let (k, y_inicio) = self.inicio;
        if self.y > self.limite && y_inicio > self.arriba {
            let (dx, dy) = (self.paso_columna, self.arriba - y_inicio);
            for c in &mut self.controles[k..] {
                *c = c.movido(dx, dy);
            }
            self.dentro += dx;
            self.columnas += 1;
            self.y += dy;
        }
        self.y_max = self.y_max.max(self.y);
        self.y += self.px(ENTRE_SECCIONES_LOGICO);
    }

    /// El hueco entre dos filas de una misma seccion.
    fn salto(&mut self) {
        self.y += self.px(HUECO_OPCION_LOGICO);
    }
}

/// El color de una casilla de la paleta, con el tono que toca: el del
/// color actual si es de esa familia, y si no el de por defecto (el 4 para
/// trazo y el 1 para fondo, `Picker.tsx:117-122`).
fn color_de_casilla(c: Casilla, actual: Option<ColorRgba>, tono: usize) -> Option<ColorRgba> {
    match c {
        Casilla::Transparente => None,
        Casilla::Fijo(v) => Some(hex(v)),
        Casilla::Tonos(t) => {
            let propio = actual.and_then(|a| t.iter().find(|v| casi(hex(**v), a)));
            Some(hex(*propio.unwrap_or(&t[tono])))
        }
    }
}

/// Los tonos de la familia del color actual, si es de alguna.
fn tonos_de(actual: Option<ColorRgba>) -> Option<[u32; 5]> {
    let a = actual?;
    PALETA.iter().find_map(|c| match c {
        Casilla::Tonos(t) if t.iter().any(|v| casi(hex(*v), a)) => Some(*t),
        _ => None,
    })
}

fn accion_de_color(cual: Desplegable, color: Option<ColorRgba>) -> Option<AccionPanel> {
    if cual == Desplegable::ColorLienzo {
        // El papel no tiene transparente: sin papel no hay lienzo que ver.
        // La casilla se queda como hueco y no corre de sitio a las demas.
        return color.map(AccionPanel::FondoLienzo);
    }
    if cual == Desplegable::ColorFondo {
        Some(AccionPanel::Estilo(CambioEstilo::Relleno(color)))
    } else {
        // El trazo no tiene transparente: `ColorRgba` no lo guarda como el
        // «transparent» del fichero, y un trazo invisible es un elemento
        // que no se puede volver a encontrar para elegirlo.
        color.map(|c| AccionPanel::Estilo(CambioEstilo::Trazo(c)))
    }
}

/// `rrggbb` en minusculas y sin almohadilla (la pinta aparte el campo, como
/// Excalidraw); `None` es transparente y no tiene codigo.
pub fn codigo_hex(c: Option<ColorRgba>) -> Option<String> {
    let c = c?;
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Some(format!("{:02x}{:02x}{:02x}", b(c.r), b(c.g), b(c.b)))
}

/// **El color de un codigo escrito a mano**: seis digitos hexadecimales, con
/// o sin la almohadilla delante. `None` con cualquier otra cosa.
///
/// Solo seis, y a proposito: los tres de la forma corta (`#f00`) se leen
/// igual de facil como un codigo a medio escribir, y aplicar un color que el
/// usuario no ha terminado de teclear es cambiarle el dibujo sin pedirlo.
pub fn color_de_codigo(escrito: &str) -> Option<ColorRgba> {
    let s = escrito.trim();
    let s = s.strip_prefix('#').unwrap_or(s);
    if s.len() != 6 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(s, 16).ok().map(hex)
}

/// **Lo que se va escribiendo en el campo del codigo.**
///
/// Nace con el codigo actual entero ELEGIDO, como el `input` de Excalidraw al
/// enfocarse: la primera tecla lo sustituye, que es lo que se quiere casi
/// siempre (pegar o teclear otro color), y Retroceso lo vacia de una vez.
/// Guarda solo digitos: la almohadilla se acepta al teclear y no se guarda,
/// asi «con o sin #» da lo mismo y nunca caben siete caracteres.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdicionHex {
    texto: String,
    todo_elegido: bool,
}

impl EdicionHex {
    pub fn nueva(actual: Option<ColorRgba>) -> EdicionHex {
        let texto = codigo_hex(actual).unwrap_or_default();
        EdicionHex {
            todo_elegido: !texto.is_empty(),
            texto,
        }
    }

    pub fn texto(&self) -> &str {
        &self.texto
    }

    /// Si lo escrito esta elegido entero (la proxima tecla lo sustituye).
    pub fn todo_elegido(&self) -> bool {
        self.todo_elegido
    }

    /// Una tecla escrita. Devuelve si cambio algo: una letra que no es un
    /// digito hexadecimal no se escribe —en un campo de seis digitos solo
    /// ensuciaria— y un septimo digito tampoco.
    pub fn escribir(&mut self, c: char) -> bool {
        if c == '#' {
            // Aceptada y no guardada: «#1e1e1e» y «1e1e1e» son lo mismo.
            return false;
        }
        if !c.is_ascii_hexdigit() {
            return false;
        }
        if self.todo_elegido {
            self.texto.clear();
            self.todo_elegido = false;
        }
        if self.texto.len() >= 6 {
            return false;
        }
        self.texto.push(c.to_ascii_lowercase());
        true
    }

    /// Retroceso: el ultimo digito, o todo si estaba elegido.
    pub fn borrar(&mut self) -> bool {
        if self.todo_elegido {
            self.todo_elegido = false;
            self.texto.clear();
            return true;
        }
        self.texto.pop().is_some()
    }

    /// El color que dice, si ya es un codigo entero.
    pub fn color(&self) -> Option<ColorRgba> {
        color_de_codigo(&self.texto)
    }
}

impl Emergente {
    /// La paleta: rejilla de 5 × 3, los tonos del color elegido y su codigo.
    fn de_color(
        cual: Desplegable,
        disparador: Rect,
        panel: Rect,
        area: Rect,
        e: u32,
        actual: Option<ColorRgba>,
    ) -> Emergente {
        // El tono por defecto de cada familia: el 4 para trazo y el 1 para
        // fondo (`Picker.tsx:117-122`); para el papel, el mas claro (el 50
        // de open-color), que es el que se lee como papel y no como pintura.
        let tono = match cual {
            Desplegable::ColorFondo => 1,
            Desplegable::ColorLienzo => 0,
            _ => 4,
        };
        let px = |v: u32| (v * e / 100) as i32;
        let casilla = px(CASILLA_LOGICA);
        let hueco = px(HUECO_CASILLA_LOGICO);
        let ancho_util = 5 * casilla + 4 * hueco;
        let mut c = Colocador::sin_limite(e, 0, ancho_util, 0);
        c.titulo(Seccion::Colores);
        c.rejilla(PALETA.len(), (casilla, casilla, hueco), |k, rect| {
            let color = color_de_casilla(PALETA[k], actual, tono);
            let accion = accion_de_color(cual, color)?;
            Some(Control::Muestra {
                rect,
                color,
                activa: mismo(color, actual),
                accion: Some(accion),
                grande: false,
            })
        });
        if let Some(t) = tonos_de(actual) {
            c.separar();
            c.titulo(Seccion::Tonos);
            c.rejilla(5, (casilla, casilla, hueco), |k, rect| {
                let color = Some(hex(t[k]));
                Some(Control::Muestra {
                    rect,
                    color,
                    activa: mismo(color, actual),
                    accion: accion_de_color(cual, color),
                    grande: false,
                })
            });
        }
        if cual == Desplegable::ColorLienzo {
            // Los del movil, con su valor exacto: es lo que hace que el papel
            // elegido aqui salga marcado en su selector al abrirlo alla.
            c.separar();
            c.titulo(Seccion::PapelesDelMovil);
            c.rejilla(
                PAPELES_DEL_MOVIL.len(),
                (casilla, casilla, hueco),
                |k, rect| {
                    let color = Some(hex(PAPELES_DEL_MOVIL[k]));
                    Some(Control::Muestra {
                        rect,
                        color,
                        activa: mismo(color, actual),
                        accion: accion_de_color(cual, color),
                        grande: false,
                    })
                },
            );
        }
        c.separar();
        c.titulo(Seccion::CodigoHex);
        let alto = px(HEX_LOGICO);
        c.controles.push(Control::Hex {
            rect: Rect {
                x: 0,
                y: c.y,
                ancho: ancho_util as u32,
                alto: alto as u32,
            },
            color: actual,
        });
        c.y += alto;
        Emergente::colocar(cual, c, disparador, panel, area, e)
    }

    /// Las nueve puntas; la del disparador, marcada.
    fn de_puntas(
        cual: Desplegable,
        disparador: Rect,
        panel: Rect,
        area: Rect,
        e: u32,
        actual: Option<TipoPunta>,
    ) -> Emergente {
        let px = |v: u32| (v * e / 100) as i32;
        let (ancho, alto, hueco) = (
            px(PUNTA_ANCHO_LOGICO),
            px(OPCION_LOGICA),
            px(HUECO_CASILLA_LOGICO),
        );
        let al_inicio = cual == Desplegable::PuntaInicio;
        let mut c = Colocador::sin_limite(e, 0, 3 * ancho + 2 * hueco, 0);
        c.rejilla(PUNTAS_DEL_PANEL.len(), (ancho, alto, hueco), |k, rect| {
            let t = PUNTAS_DEL_PANEL[k];
            Some(Control::Punta {
                rect,
                accion: AccionPanel::Forma(if al_inicio {
                    CambioForma::PuntaInicio(t)
                } else {
                    CambioForma::PuntaFin(t)
                }),
                punta: Some(t),
                al_inicio,
                activa: actual == Some(t),
            })
        });
        Emergente::colocar(cual, c, disparador, panel, area, e)
    }

    /// Lleva lo colocado en el origen a su sitio: a la derecha del panel,
    /// un poco por encima del disparador, y metido en la ventana.
    fn colocar(
        cual: Desplegable,
        c: Colocador,
        disparador: Rect,
        panel: Rect,
        area: Rect,
        e: u32,
    ) -> Emergente {
        let px = |v: u32| (v * e / 100) as i32;
        let relleno = px(RELLENO_LOGICO);
        let ancho = c.ancho_util + 2 * relleno;
        let alto = c.y + 2 * relleno;
        let x = panel.derecha() + px(SEPARACION_DESPLEGABLE_LOGICA);
        let bajo = area.abajo() - px(8) - alto;
        let y = (disparador.y - px(SUBIDA_DESPLEGABLE_LOGICA))
            .min(bajo)
            .max(area.y + px(8));
        let (dx, dy) = (x + relleno, y + relleno);
        let controles = c.controles.into_iter().map(|k| k.movido(dx, dy)).collect();
        Emergente {
            cual,
            marco: Rect {
                x,
                y,
                ancho: ancho as u32,
                alto: alto as u32,
            },
            controles,
        }
    }
}

impl PanelLateral {
    /// `None` si no hay nada que ajustar (la mano sin nada elegido, la lupa):
    /// una isla vacia solo tapa lienzo.
    pub fn construir(area: Rect, escala_por_cien: u32, ctx: ContextoPanel) -> Option<PanelLateral> {
        let e = |v: u32| (v * escala_por_cien / 100) as i32;
        let x0 = area.x + e(X_LOGICA);
        let y0 = area.y + e(Y_LOGICA);
        let mut c = Colocador {
            // Hasta 16 del borde de abajo, como el margen de arriba.
            limite: area.abajo() - e(RELLENO_LOGICO) - e(X_LOGICA),
            paso_columna: e(ANCHO_LOGICO) - e(RELLENO_LOGICO),
            ..Colocador::sin_limite(
                escala_por_cien,
                x0 + e(RELLENO_LOGICO),
                e(ANCHO_LOGICO) - 2 * e(RELLENO_LOGICO),
                y0 + e(RELLENO_LOGICO),
            )
        };
        // Con el papel pedido, el panel es solo el papel: lo demas se
        // construye como si no hubiera nada que ajustar.
        let ctx = if ctx.lienzo.is_some() {
            ContextoPanel {
                propiedades: &[],
                seleccionados: 0,
                mandos: Mandos::default(),
                ..ctx
            }
        } else {
            ctx
        };
        let tiene = |p: Propiedad| ctx.propiedades.contains(&p);
        let s = ctx.estilo;
        let m = ctx.mandos;
        let abierto = |d| ctx.abierto == Some(d);

        // El papel del lienzo: la fila de Excalidraw (cinco rapidos y el
        // actual, que abre la paleta). Ver `ContextoPanel::lienzo`.
        if let Some(papel) = ctx.lienzo {
            c.titulo(Seccion::FondoLienzo);
            let lista: Vec<_> = COLORES_LIENZO
                .iter()
                .map(|col| (Some(*col), AccionPanel::FondoLienzo(*col)))
                .collect();
            c.colores(
                &lista,
                Some(papel),
                Desplegable::ColorLienzo,
                abierto(Desplegable::ColorLienzo),
            );
            c.separar();
        }

        if tiene(Propiedad::ColorTrazo) {
            c.titulo(Seccion::Trazo);
            let lista: Vec<_> = COLORES_TRAZO
                .iter()
                .map(|col| (Some(*col), AccionPanel::Estilo(CambioEstilo::Trazo(*col))))
                .collect();
            c.colores(
                &lista,
                Some(s.trazo),
                Desplegable::ColorTrazo,
                abierto(Desplegable::ColorTrazo),
            );
            c.separar();
        }
        if tiene(Propiedad::Relleno) {
            c.titulo(Seccion::Fondo);
            let lista: Vec<_> = COLORES_FONDO
                .iter()
                .map(|col| (*col, AccionPanel::Estilo(CambioEstilo::Relleno(*col))))
                .collect();
            c.colores(
                &lista,
                s.relleno,
                Desplegable::ColorFondo,
                abierto(Desplegable::ColorFondo),
            );
            c.separar();
        }
        // Con el fondo transparente no hay nada que rayar: Excalidraw esconde
        // esta seccion igual (su `hasBackground` mira el color, no la figura).
        if tiene(Propiedad::EstiloRelleno) && m.con_fondo {
            c.titulo(Seccion::Relleno);
            let f = |v| {
                (
                    AccionPanel::Estilo(CambioEstilo::EstiloRelleno(v)),
                    s.estilo_relleno == v,
                )
            };
            c.fila(&[
                f(EstiloRelleno::Rayado),
                f(EstiloRelleno::Cruzado),
                f(EstiloRelleno::Solido),
            ]);
            c.separar();
        }
        if tiene(Propiedad::Grosor) {
            c.titulo(Seccion::Grosor);
            let g = |n| (AccionPanel::Estilo(CambioEstilo::Grosor(n)), s.grosor == n);
            c.fila(&[
                g(NivelGrosor::Fino),
                g(NivelGrosor::Medio),
                g(NivelGrosor::Grueso),
            ]);
            c.separar();
        }
        if tiene(Propiedad::Estilo) {
            c.titulo(Seccion::EstiloTrazo);
            let t = |v| (AccionPanel::Estilo(CambioEstilo::Estilo(v)), s.estilo == v);
            c.fila(&[
                t(EstiloTrazo::Solido),
                t(EstiloTrazo::Discontinuo),
                t(EstiloTrazo::Punteado),
                t(EstiloTrazo::RayaPunto),
            ]);
            c.separar();
        }
        // **Las de las laminas de geometria**: liso o degradado, como se
        // cierra el arco, como se traza y cuantos arquitos lleva un angulo.
        if m.degradado.sale() {
            use pixpin_motor2d::geometria_exacta::TipoDegradado as T;
            c.titulo(Seccion::Degradado);
            let s = |v| {
                (
                    AccionPanel::Forma(CambioForma::Degradado(v)),
                    m.degradado.valor() == Some(v),
                )
            };
            c.fila(&[s(None), s(Some(T::Lineal)), s(Some(T::Radial))]);
            c.separar();
        }
        if m.cierre_arco.sale() {
            use pixpin_motor2d::geometria_exacta::CierreArco as C;
            c.titulo(Seccion::CierreArco);
            let s = |v| {
                (
                    AccionPanel::Forma(CambioForma::CierreArco(v)),
                    m.cierre_arco.valor() == Some(v),
                )
            };
            c.fila(&[s(None), s(Some(C::Sector)), s(Some(C::Segmento))]);
            c.separar();
        }
        if m.modo_arco.sale() {
            use pixpin_motor2d::geometria_exacta::ModoArco as M;
            c.titulo(Seccion::ModoArco);
            let s = |v| {
                (
                    AccionPanel::Forma(CambioForma::ModoArco(v)),
                    m.modo_arco.valor() == Some(v),
                )
            };
            c.fila(&[s(M::Guia), s(M::CentroYPuntos), s(M::TresPuntos)]);
            c.separar();
        }
        if m.modo_angulo.sale() {
            use pixpin_motor2d::geometria_exacta::ModoAngulo as M;
            c.titulo(Seccion::ModoAngulo);
            let s = |v| {
                (
                    AccionPanel::Forma(CambioForma::ModoAngulo(v)),
                    m.modo_angulo.valor() == Some(v),
                )
            };
            c.fila(&[s(M::Griega), s(M::Simple), s(M::Recto)]);
            c.separar();
        }
        if m.arcos_angulo.sale() {
            c.titulo(Seccion::ArcosAngulo);
            let s = |n: u8| {
                (
                    AccionPanel::Forma(CambioForma::ArcosAngulo(n)),
                    m.arcos_angulo.valor() == Some(n),
                )
            };
            c.fila(&[s(1), s(2), s(3)]);
            c.separar();
        }
        if m.presion.sale() {
            c.titulo(Seccion::Presion);
            let v = |x| {
                (
                    AccionPanel::Forma(CambioForma::Presion(x)),
                    m.presion.valor() == Some(x),
                )
            };
            c.fila(&[v(Variabilidad::Constante), v(Variabilidad::Variable)]);
            c.separar();
        }
        // **La dureza del grafito**, de la 4H, fina y clara, a la 8B, gorda y
        // negra (`LaDurezaQueHay` del panel del movil): siete botones con su
        // nombre, como en la caja de lapices.
        if m.dureza.sale() {
            c.titulo(Seccion::Dureza);
            let opciones: Vec<(AccionPanel, bool)> = pixpin_motor2d::tinta::grafito::Dureza::TODAS
                .into_iter()
                .map(|d| {
                    (
                        AccionPanel::Estilo(CambioEstilo::Dureza(d)),
                        m.dureza.valor() == Some(d),
                    )
                })
                .collect();
            c.fila(&opciones);
            c.separar();
        }
        if tiene(Propiedad::Rugosidad) {
            c.titulo(Seccion::TrazoAMano);
            let r = |v: f32| {
                (
                    AccionPanel::Estilo(CambioEstilo::Rugosidad(v)),
                    (s.rugosidad - v).abs() < 0.01,
                )
            };
            c.fila(&[r(0.0), r(1.0), r(2.0)]);
            c.separar();
        }
        if m.bordes.sale() {
            c.titulo(Seccion::Bordes);
            let b = |redondo| {
                (
                    AccionPanel::Forma(CambioForma::Bordes { redondo }),
                    m.bordes.valor() == Some(redondo),
                )
            };
            c.fila(&[b(false), b(true)]);
            c.separar();
        }
        if m.tipo_flecha.sale() {
            // Afilada, curva y de codos, en el orden de `changeArrowType`.
            // La curva pasa por los puntos (`curva.rs`) y viaja como
            // `roundness`; con dos puntos es recta, como en Excalidraw, hasta
            // que se le arrastra el tirador del medio.
            c.titulo(Seccion::TipoFlecha);
            let k = |v| {
                (
                    AccionPanel::Forma(CambioForma::TipoFlecha(v)),
                    m.tipo_flecha.valor() == Some(v),
                )
            };
            c.fila(&[
                k(TipoFlecha::Afilada),
                k(TipoFlecha::Curva),
                k(TipoFlecha::Codos),
            ]);
            c.separar();
        }
        if m.familia.sale() {
            c.titulo(Seccion::Fuente);
            let opciones: Vec<_> = FAMILIAS
                .iter()
                .map(|n| {
                    (
                        AccionPanel::Forma(CambioForma::Familia(*n)),
                        m.familia.valor() == Some(*n),
                    )
                })
                .collect();
            c.fila(&opciones);
            c.separar();
        }
        if m.tamano.sale() {
            c.titulo(Seccion::TamanoFuente);
            let opciones: Vec<_> = TAMANOS_DE_LETRA
                .iter()
                .map(|t| {
                    (
                        AccionPanel::Forma(CambioForma::TamanoLetra(*t)),
                        m.tamano.valor().is_some_and(|v| (v - t).abs() < 0.5),
                    )
                })
                .collect();
            c.fila(&opciones);
            c.separar();
        }
        if m.alineacion.sale() || m.alineacion_vertical.sale() {
            c.titulo(Seccion::AlineacionTexto);
            if m.alineacion.sale() {
                let a = |v| {
                    (
                        AccionPanel::Forma(CambioForma::Alineacion(v)),
                        m.alineacion.valor() == Some(v),
                    )
                };
                c.fila(&[
                    a(AlineacionTexto::Izquierda),
                    a(AlineacionTexto::Centro),
                    a(AlineacionTexto::Derecha),
                ]);
            }
            if m.alineacion_vertical.sale() {
                if m.alineacion.sale() {
                    c.salto();
                }
                let v = |v| {
                    (
                        AccionPanel::Forma(CambioForma::AlineacionVertical(v)),
                        m.alineacion_vertical.valor() == Some(v),
                    )
                };
                c.fila(&[
                    v(AlineacionVertical::Arriba),
                    v(AlineacionVertical::Medio),
                    v(AlineacionVertical::Abajo),
                ]);
            }
            c.separar();
        }
        if m.mosaico.sale() {
            c.titulo(Seccion::Mosaico);
            let s = |v| {
                (
                    AccionPanel::Forma(CambioForma::Desenfoque(v)),
                    m.mosaico.valor() == Some(v),
                )
            };
            c.fila(&[s(false), s(true)]);
            c.separar();
        }
        if m.lupa_aumento.sale() {
            c.titulo(Seccion::AumentoLupa);
            let actual = m.lupa_aumento.valor();
            let s = |z: f32| {
                (
                    AccionPanel::Forma(CambioForma::AumentoLupa(z)),
                    actual.is_some_and(|a| (a - z).abs() < 0.05),
                )
            };
            c.fila(&[s(1.5), s(2.0), s(3.0), s(4.0)]);
            c.separar();
        }
        if m.lupa_guia.sale() {
            use pixpin_motor2d::lupa_elemento::GuiaDeLupa as G;
            c.titulo(Seccion::GuiaLupa);
            let s = |g| {
                (
                    AccionPanel::Forma(CambioForma::GuiaLupa(g)),
                    m.lupa_guia.valor() == Some(g),
                )
            };
            c.fila(&[s(G::Ninguna), s(G::Flecha), s(G::DosLineas), s(G::Punto)]);
            c.separar();
        }
        if m.foco_oscurecer.sale() {
            c.titulo(Seccion::Oscurecer);
            let actual = m.foco_oscurecer.valor();
            let s = |n: u8| {
                (
                    AccionPanel::Forma(CambioForma::Oscurecer(n)),
                    actual == Some(n),
                )
            };
            c.fila(&[s(25), s(45), s(65), s(85)]);
            c.separar();
        }
        if m.foco_zona.sale() {
            c.titulo(Seccion::ZonaFoco);
            let actual = m.foco_zona.valor();
            let s = |z: f32| {
                (
                    AccionPanel::Forma(CambioForma::ZonaFoco(z)),
                    actual.is_some_and(|a| (a - z).abs() < 0.02),
                )
            };
            c.fila(&[s(0.35), s(0.55), s(0.75), s(0.9)]);
            c.separar();
        }
        if m.pedir_medida.sale() {
            c.titulo(Seccion::PedirMedida);
            let s = |v| {
                (
                    AccionPanel::PedirMedida(v),
                    m.pedir_medida.valor() == Some(v),
                )
            };
            c.fila(&[s(true), s(false)]);
            c.separar();
        }
        if m.serie_punto.sale() {
            use pixpin_motor2d::puntos_etiquetados::SerieDePunto as S;
            c.titulo(Seccion::SerieDePunto);
            let s = |v| (AccionPanel::SeriePunto(v), m.serie_punto.valor() == Some(v));
            c.fila(&[s(S::Mayusculas), s(S::Minusculas), s(S::Numeros)]);
            c.separar();
        }
        if m.punta_inicio.sale() || m.punta_fin.sale() {
            c.titulo(Seccion::Puntas);
            let lado = c.px(OPCION_LOGICA);
            let hueco = c.px(HUECO_OPCION_LOGICO);
            let disparadores = [
                (Desplegable::PuntaInicio, m.punta_inicio, true),
                (Desplegable::PuntaFin, m.punta_fin, false),
            ];
            c.rejilla(2, (lado, lado, hueco), |k, rect| {
                let (d, valor, al_inicio) = disparadores[k];
                Some(Control::Punta {
                    rect,
                    accion: AccionPanel::Abrir(d),
                    punta: valor.valor(),
                    al_inicio,
                    activa: abierto(d),
                })
            });
            c.separar();
        }
        if tiene(Propiedad::Opacidad) {
            c.titulo(Seccion::Opacidad);
            c.deslizador(s.opacidad);
            c.separar();
        }
        // **Las lineas de referencia**: el interruptor del modo guia y, si
        // hay alguna, el ojo que las esconde (`DrawToolbar` del movil, que
        // pone los dos junto a las herramientas). Sin borrar todas de golpe:
        // el movil lo quito porque solo se pulsaba por error.
        if let Some((modo, visibles)) = m.guias {
            c.titulo(Seccion::Guias);
            let mut opciones = vec![(AccionPanel::ModoGuia(!modo), modo)];
            if let Some(v) = visibles {
                opciones.push((AccionPanel::VerGuias(!v), !v));
            }
            c.fila(&opciones);
            c.separar();
        }
        if ctx.seleccionados >= 1 {
            c.titulo(Seccion::Capas);
            let k = |capa| (AccionPanel::Capa(capa), false);
            c.fila(&[
                k(Capa::Fondo),
                k(Capa::Atras),
                k(Capa::Adelante),
                k(Capa::Frente),
            ]);
            c.separar();
        }
        if ctx.seleccionados >= 2 {
            // Dos filas, como `AlignFieldset`: en horizontal arriba y en
            // vertical abajo, cada una con su repartir al final.
            c.titulo(Seccion::Alinear);
            let a = |al| (AccionPanel::Alinear(al), false);
            // Repartir pide tres: con dos no hay nada entre medias.
            let tres = ctx.seleccionados >= 3;
            let mut horizontal = vec![
                a(Alineacion::Izquierda),
                a(Alineacion::CentroHorizontal),
                a(Alineacion::Derecha),
            ];
            let mut vertical = vec![
                a(Alineacion::Arriba),
                a(Alineacion::CentroVertical),
                a(Alineacion::Abajo),
            ];
            if tres {
                horizontal.push((AccionPanel::Repartir(Reparto::Horizontal), false));
                vertical.push((AccionPanel::Repartir(Reparto::Vertical), false));
            }
            c.fila(&horizontal);
            c.salto();
            c.fila(&vertical);
            c.separar();
        }
        if ctx.seleccionados >= 1 {
            c.titulo(Seccion::Acciones);
            let mut opciones = vec![(AccionPanel::Duplicar, false), (AccionPanel::Borrar, false)];
            if m.agrupar {
                opciones.push((AccionPanel::Agrupar, false));
            }
            if m.desagrupar {
                opciones.push((AccionPanel::Desagrupar, false));
            }
            opciones.push((AccionPanel::GuardarFigura, false));
            c.fila(&opciones);
            c.separar();
        }

        if c.controles.is_empty() {
            return None;
        }
        // Lo mas bajo de todas las columnas, mas el relleno de abajo. Cada
        // columna de mas anade su ancho util y un relleno entre medias.
        let alto = (c.y_max + e(RELLENO_LOGICO) - y0) as u32;
        let marco = Rect {
            x: x0,
            y: y0,
            ancho: (e(ANCHO_LOGICO) + (c.columnas - 1) * c.paso_columna) as u32,
            alto,
        };
        // El desplegable solo si su disparador salio: si lo elegido ya no
        // tiene esa seccion, no hay nada que desplegar.
        let emergente = ctx.abierto.and_then(|d| {
            let disparador = c.controles.iter().find_map(|k| match *k {
                Control::Muestra {
                    rect,
                    accion: Some(AccionPanel::Abrir(x)),
                    ..
                }
                | Control::Punta {
                    rect,
                    accion: AccionPanel::Abrir(x),
                    ..
                } if x == d => Some(rect),
                _ => None,
            })?;
            Some(match d {
                Desplegable::ColorTrazo => {
                    Emergente::de_color(d, disparador, marco, area, escala_por_cien, Some(s.trazo))
                }
                Desplegable::ColorFondo => {
                    Emergente::de_color(d, disparador, marco, area, escala_por_cien, s.relleno)
                }
                Desplegable::ColorLienzo => {
                    Emergente::de_color(d, disparador, marco, area, escala_por_cien, ctx.lienzo)
                }
                Desplegable::PuntaInicio => Emergente::de_puntas(
                    d,
                    disparador,
                    marco,
                    area,
                    escala_por_cien,
                    m.punta_inicio.valor(),
                ),
                Desplegable::PuntaFin => Emergente::de_puntas(
                    d,
                    disparador,
                    marco,
                    area,
                    escala_por_cien,
                    m.punta_fin.valor(),
                ),
            })
        });
        Some(PanelLateral {
            marco,
            controles: c.controles,
            emergente,
            plegar: None,
            plegado: false,
        })
    }

    /// El mismo panel con su boton de quitarlo ([`boton_plegar`]).
    pub fn con_plegar(self, area: Rect, escala_por_cien: u32) -> PanelLateral {
        PanelLateral {
            plegar: Some(boton_plegar(area, escala_por_cien)),
            ..self
        }
    }

    /// **El panel quitado**: solo su boton, para volver a sacarlo.
    pub fn plegado(area: Rect, escala_por_cien: u32) -> PanelLateral {
        let boton = boton_plegar(area, escala_por_cien);
        PanelLateral {
            marco: boton,
            controles: Vec::new(),
            emergente: None,
            plegar: Some(boton),
            plegado: true,
        }
    }

    pub fn destino(&self, p: Punto) -> DestinoPanel {
        if self.plegar.is_some_and(|b| b.contiene(p)) {
            return DestinoPanel::Accion(AccionPanel::Plegar);
        }
        if self.plegado {
            return DestinoPanel::Fuera;
        }
        // El desplegable va encima de todo: se mira primero.
        if let Some(em) = &self.emergente
            && em.marco.contiene(p)
        {
            return em
                .controles
                .iter()
                .find(|c| c.rect().contiene(p))
                .and_then(Control::accion)
                .map_or(DestinoPanel::Panel, DestinoPanel::Accion);
        }
        if !self.marco.contiene(p) {
            return DestinoPanel::Fuera;
        }
        for c in &self.controles {
            match *c {
                Control::Deslizador { rect, .. } if rect.contiene(p) => {
                    // Pasos de 10 %, como el deslizador de Excalidraw.
                    let t = (p.x - rect.x) as f32 / rect.ancho.max(1) as f32;
                    let valor = ((t * 10.0).round() / 10.0).clamp(0.0, 1.0);
                    return DestinoPanel::Accion(AccionPanel::Estilo(CambioEstilo::Opacidad(
                        valor,
                    )));
                }
                _ if c.rect().contiene(p) => {
                    if let Some(a) = c.accion() {
                        return DestinoPanel::Accion(a);
                    }
                }
                _ => {}
            }
        }
        DestinoPanel::Panel
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::vector::Punto2;

    fn area() -> Rect {
        Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1080,
        }
    }

    fn ctx(propiedades: &[Propiedad], seleccionados: usize) -> ContextoPanel<'_> {
        ContextoPanel {
            propiedades,
            estilo: EstiloDibujo::default(),
            seleccionados,
            mandos: Mandos::default(),
            abierto: None,
            lienzo: None,
        }
    }

    /// Como `ctx` pero con fondo puesto, que es cuando el relleno se ofrece.
    fn ctx_con_fondo(propiedades: &[Propiedad]) -> ContextoPanel<'_> {
        ContextoPanel {
            propiedades,
            estilo: EstiloDibujo {
                relleno: COLORES_FONDO[1],
                ..EstiloDibujo::default()
            },
            seleccionados: 0,
            mandos: Mandos {
                con_fondo: true,
                ..Mandos::default()
            },
            abierto: None,
            lienzo: None,
        }
    }

    fn secciones(p: &PanelLateral) -> Vec<Seccion> {
        p.controles
            .iter()
            .filter_map(|c| match c {
                Control::Titulo { seccion, .. } => Some(*seccion),
                _ => None,
            })
            .collect()
    }

    fn centro(r: Rect) -> Punto {
        Punto {
            x: r.x + r.ancho as i32 / 2,
            y: r.y + r.alto as i32 / 2,
        }
    }

    fn elem(id: u64, figura: Figura) -> Elemento {
        Elemento {
            id,
            figura,
            ancho: 100.0,
            alto: 40.0,
            ..Default::default()
        }
    }

    fn texto() -> Figura {
        Figura::Texto {
            texto: "hola".into(),
            tam: 20.0,
            familia: "Excalifont".into(),
        }
    }

    fn flecha(inicio: TipoPunta, fin: TipoPunta) -> Figura {
        Figura::Flecha {
            puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(10.0, 0.0)],
            punta_inicio: inicio,
            punta_fin: fin,
            codos: false,
        }
    }

    fn lapiz() -> Figura {
        Figura::Lapiz {
            puntos: Vec::new(),
            presiones: Vec::new(),
            opciones: Some(Default::default()),
        }
    }

    /// El panel de lo elegido, montado como lo monta el editor.
    fn de_seleccion(elegidos: &[&Elemento], abierto: Option<Desplegable>) -> PanelLateral {
        let props = propiedades_de_seleccion(elegidos);
        PanelLateral::construir(
            area(),
            100,
            ContextoPanel {
                propiedades: &props,
                estilo: estilo_de_seleccion(elegidos).unwrap(),
                seleccionados: elegidos.len(),
                mandos: Mandos::de_seleccion(elegidos),
                abierto,
                lienzo: None,
            },
        )
        .unwrap()
    }

    fn rect_de(p: &PanelLateral, buscada: AccionPanel) -> Rect {
        p.controles
            .iter()
            .chain(p.emergente.iter().flat_map(|e| e.controles.iter()))
            .find(|c| c.accion() == Some(buscada))
            .map(Control::rect)
            .unwrap_or_else(|| panic!("no hay control para {buscada:?}"))
    }

    fn marcados(p: &PanelLateral) -> Vec<AccionPanel> {
        p.controles
            .iter()
            .filter_map(|c| match *c {
                Control::Opcion {
                    accion,
                    activa: true,
                    ..
                } => Some(accion),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn el_rectangulo_ensena_trazo_fondo_grosor_estilo_trazo_a_mano_y_opacidad() {
        let props =
            crate::propiedades::de_herramienta(pixpin_motor2d::gesto::Herramienta::Rectangulo);
        let p = PanelLateral::construir(area(), 100, ctx(props, 0)).unwrap();
        assert_eq!(
            secciones(&p),
            vec![
                Seccion::Trazo,
                Seccion::Fondo,
                Seccion::Grosor,
                Seccion::EstiloTrazo,
                Seccion::TrazoAMano,
                Seccion::Opacidad
            ]
        );
        assert_eq!(p.marco.ancho, ANCHO_LOGICO);
        assert_eq!((p.marco.x, p.marco.y), (16, Y_LOGICA as i32));
    }

    /// 24-oct-2026: el boton de arriba a la izquierda quita el panel y lo
    /// vuelve a sacar; quitado no se queda ningun clic mas que el suyo.
    #[test]
    fn el_boton_de_plegar_quita_y_saca_el_panel() {
        let props =
            crate::propiedades::de_herramienta(pixpin_motor2d::gesto::Herramienta::Rectangulo);
        let entero = PanelLateral::construir(area(), 100, ctx(props, 0))
            .unwrap()
            .con_plegar(area(), 100);
        let b = entero.plegar.expect("lleva su boton");
        assert!(b.abajo() <= entero.marco.y, "encima del panel, sin pisarlo");
        let centro = |r: Rect| Punto {
            x: r.x + r.ancho as i32 / 2,
            y: r.y + r.alto as i32 / 2,
        };
        assert_eq!(
            entero.destino(centro(b)),
            DestinoPanel::Accion(AccionPanel::Plegar)
        );
        assert_ne!(entero.destino(centro(entero.marco)), DestinoPanel::Fuera);
        let quitado = PanelLateral::plegado(area(), 100);
        assert_eq!(
            quitado.destino(centro(b)),
            DestinoPanel::Accion(AccionPanel::Plegar)
        );
        // Caso negativo: donde estaba el panel, el clic ya es del lienzo.
        assert_eq!(quitado.destino(centro(entero.marco)), DestinoPanel::Fuera);
        assert!(quitado.controles.is_empty());
    }

    #[test]
    fn el_relleno_se_ofrece_con_fondo_puesto_y_se_esconde_sin_el() {
        let props = [Propiedad::Relleno, Propiedad::EstiloRelleno];
        let con = PanelLateral::construir(area(), 100, ctx_con_fondo(&props)).unwrap();
        assert_eq!(secciones(&con), vec![Seccion::Fondo, Seccion::Relleno]);

        // Caso negativo: con el fondo transparente no hay nada que rayar, y
        // ofrecer tres botones que no cambian nada visible es enganar.
        let sin = PanelLateral::construir(area(), 100, ctx(&props, 0)).unwrap();
        assert_eq!(secciones(&sin), vec![Seccion::Fondo]);
    }

    #[test]
    fn el_bote_ofrece_el_relleno_aunque_el_fondo_actual_sea_transparente() {
        // El bote nunca pinta transparente, asi que su rayado siempre se ve.
        let m = Mandos::de_herramienta(
            Herramienta::Relleno,
            &EstiloDibujo::default(),
            Default::default(),
        );
        assert!(m.con_fondo);
        let m = Mandos::de_herramienta(
            Herramienta::Rectangulo,
            &EstiloDibujo::default(),
            Default::default(),
        );
        assert!(!m.con_fondo, "el rectangulo sin fondo no tiene que rayar");
    }

    #[test]
    fn una_linea_elegida_no_ensena_la_seccion_de_relleno() {
        // Caso negativo del otro lado: la propiedad ni siquiera llega, porque
        // una linea no tiene interior.
        let props = crate::propiedades::de_figura(&pixpin_motor2d::elemento::Figura::Linea {
            puntos: Vec::new(),
        });
        let p = PanelLateral::construir(area(), 100, ctx_con_fondo(props)).unwrap();
        assert!(!secciones(&p).contains(&Seccion::Relleno));
    }

    #[test]
    fn pulsar_el_rayado_cruzado_pide_ese_estilo_de_relleno() {
        let props = [Propiedad::Relleno, Propiedad::EstiloRelleno];
        let p = PanelLateral::construir(area(), 100, ctx_con_fondo(&props)).unwrap();
        let cruzado = AccionPanel::Estilo(CambioEstilo::EstiloRelleno(EstiloRelleno::Cruzado));
        assert_eq!(
            p.destino(centro(rect_de(&p, cruzado))),
            DestinoPanel::Accion(cruzado)
        );

        // Y el boton marcado es el del estilo actual, el rayado.
        assert_eq!(
            marcados(&p),
            vec![AccionPanel::Estilo(CambioEstilo::EstiloRelleno(
                EstiloRelleno::Rayado
            ))]
        );
    }

    #[test]
    fn sin_nada_que_ajustar_no_hay_panel() {
        assert!(PanelLateral::construir(area(), 100, ctx(&[], 0)).is_none());
    }

    #[test]
    fn un_rectangulo_y_un_texto_ensenan_lo_de_los_dos_en_el_orden_de_excalidraw() {
        // La regla de Excalidraw es «alguno», no «todos»: con la interseccion
        // de antes, esta pareja se quedaba en trazo y opacidad y no habia
        // forma de cambiarle la letra al texto sin soltar la caja.
        let caja = Elemento {
            relleno: COLORES_FONDO[2],
            ..elem(1, Figura::Rectangulo)
        };
        let rotulo = elem(2, texto());
        let p = de_seleccion(&[&rotulo, &caja], None);
        assert_eq!(
            secciones(&p),
            vec![
                Seccion::Trazo,
                Seccion::Fondo,
                Seccion::Relleno,
                Seccion::Grosor,
                Seccion::EstiloTrazo,
                // La caja va rellena: puede ir degradada.
                Seccion::Degradado,
                Seccion::TrazoAMano,
                Seccion::Bordes,
                Seccion::Fuente,
                Seccion::TamanoFuente,
                Seccion::AlineacionTexto,
                Seccion::Opacidad,
                Seccion::Capas,
                Seccion::Alinear,
                Seccion::Acciones,
            ]
        );
    }

    #[test]
    fn el_fondo_de_una_pareja_mezclada_solo_llega_a_quien_lo_tiene() {
        let caja = elem(1, Figura::Rectangulo);
        let rotulo = elem(2, texto());
        let fondo = AccionPanel::Estilo(CambioEstilo::Relleno(COLORES_FONDO[1]));
        assert_eq!(destinatarios(&fondo, &[&caja, &rotulo]), vec![1]);
        // El trazo lo tienen los dos.
        let trazo = AccionPanel::Estilo(CambioEstilo::Trazo(COLORES_TRAZO[1]));
        assert_eq!(destinatarios(&trazo, &[&caja, &rotulo]), vec![1, 2]);
        // Y la letra solo el texto.
        let letra = AccionPanel::Forma(CambioForma::TamanoLetra(28.0));
        assert_eq!(destinatarios(&letra, &[&caja, &rotulo]), vec![2]);
    }

    #[test]
    fn el_valor_marcado_sale_de_quien_tiene_la_propiedad() {
        // El primero elegido es un texto, que no tiene grosor: el grosor
        // marcado tiene que ser el del rectangulo, no el valor que el texto
        // arrastre sin usarlo.
        let rotulo = Elemento {
            grosor: 4.0,
            ..elem(1, texto())
        };
        let caja = Elemento {
            grosor: 1.0,
            ..elem(2, Figura::Rectangulo)
        };
        let s = estilo_de_seleccion(&[&rotulo, &caja]).unwrap();
        assert_eq!(s.grosor, NivelGrosor::Fino);
    }

    #[test]
    fn un_texto_ofrece_las_ocho_familias_y_cuatro_tamanos_con_el_suyo_marcado() {
        let rotulo = elem(1, texto());
        let p = de_seleccion(&[&rotulo], None);
        let marcados = marcados(&p);
        assert!(marcados.contains(&AccionPanel::Forma(CambioForma::Familia(5))));
        // Las de Excalidraw y las del lienzo de citas, y solo una marcada.
        assert!(FAMILIAS.contains(&pixpin_motor2d::texto::FUENTE_LILITA_ONE));
        assert!(FAMILIAS.contains(&pixpin_motor2d::texto::FUENTE_CAVEAT));
        let familias_marcadas = marcados
            .iter()
            .filter(|a| matches!(a, AccionPanel::Forma(CambioForma::Familia(_))))
            .count();
        assert_eq!(familias_marcadas, 1);
        assert!(marcados.contains(&AccionPanel::Forma(CambioForma::TamanoLetra(20.0))));
        for n in FAMILIAS {
            rect_de(&p, AccionPanel::Forma(CambioForma::Familia(n)));
        }
        let pulsado = AccionPanel::Forma(CambioForma::TamanoLetra(36.0));
        assert_eq!(
            p.destino(centro(rect_de(&p, pulsado))),
            DestinoPanel::Accion(pulsado)
        );
        // Caso negativo: un texto no tiene bordes, ni puntas, ni presion.
        let s = secciones(&p);
        assert!(!s.contains(&Seccion::Bordes));
        assert!(!s.contains(&Seccion::Puntas));
        assert!(!s.contains(&Seccion::Presion));
    }

    #[test]
    fn dos_textos_de_distinto_tamano_no_marcan_ninguno() {
        let a = elem(1, texto());
        let b = Elemento {
            figura: Figura::Texto {
                texto: "b".into(),
                tam: 36.0,
                familia: "Excalifont".into(),
            },
            ..elem(2, texto())
        };
        let m = Mandos::de_seleccion(&[&a, &b]);
        assert_eq!(m.tamano, Mando::Mixto);
        assert_eq!(m.familia, Mando::Vale(5), "la familia si coincide");
        let p = de_seleccion(&[&a, &b], None);
        assert!(
            !marcados(&p)
                .iter()
                .any(|a| matches!(a, AccionPanel::Forma(CambioForma::TamanoLetra(_))))
        );
    }

    #[test]
    fn una_flecha_ensena_tipo_y_puntas_y_su_desplegable_cambia_la_punta_final() {
        let f = elem(1, flecha(TipoPunta::Ninguna, TipoPunta::Flecha));
        let cerrado = de_seleccion(&[&f], None);
        let s = secciones(&cerrado);
        assert!(s.contains(&Seccion::TipoFlecha) && s.contains(&Seccion::Puntas));
        assert!(
            marcados(&cerrado).contains(&AccionPanel::Forma(CambioForma::TipoFlecha(
                TipoFlecha::Afilada
            )))
        );
        let abrir = AccionPanel::Abrir(Desplegable::PuntaFin);
        assert_eq!(
            cerrado.destino(centro(rect_de(&cerrado, abrir))),
            DestinoPanel::Accion(abrir)
        );
        assert!(cerrado.emergente.is_none());

        let abierto = de_seleccion(&[&f], Some(Desplegable::PuntaFin));
        let em = abierto.emergente.as_ref().expect("desplegable abierto");
        assert_eq!(em.controles.len(), PUNTAS_DEL_PANEL.len());
        // Marcada la que tiene: la flecha de siempre.
        let marcada: Vec<_> = em
            .controles
            .iter()
            .filter_map(|c| match *c {
                Control::Punta {
                    punta,
                    activa: true,
                    ..
                } => punta,
                _ => None,
            })
            .collect();
        assert_eq!(marcada, vec![TipoPunta::Flecha]);
        let triangulo = AccionPanel::Forma(CambioForma::PuntaFin(TipoPunta::Triangulo));
        assert_eq!(
            abierto.destino(centro(rect_de(&abierto, triangulo))),
            DestinoPanel::Accion(triangulo)
        );
        // El desplegable sale a la derecha del panel, sin taparlo.
        assert!(em.marco.x > abierto.marco.derecha());
    }

    #[test]
    fn un_rectangulo_ofrece_bordes_y_uno_redondo_marca_redondo() {
        let caja = Elemento {
            redondo: true,
            ..elem(1, Figura::Rectangulo)
        };
        let p = de_seleccion(&[&caja], None);
        assert!(marcados(&p).contains(&AccionPanel::Forma(CambioForma::Bordes { redondo: true })));
        // Caso negativo: la elipse no tiene esquinas.
        let ovalo = elem(2, Figura::Elipse);
        assert!(!secciones(&de_seleccion(&[&ovalo], None)).contains(&Seccion::Bordes));
    }

    #[test]
    fn la_presion_sale_con_el_lapiz_elegido_o_puesto_y_no_con_otra_cosa() {
        let trazo = elem(1, lapiz());
        let p = de_seleccion(&[&trazo], None);
        assert!(secciones(&p).contains(&Seccion::Presion));
        assert!(
            marcados(&p).contains(&AccionPanel::Forma(CambioForma::Presion(
                Variabilidad::Variable
            )))
        );

        let m = Mandos::de_herramienta(
            Herramienta::Lapiz,
            &EstiloDibujo::default(),
            Variabilidad::Constante,
        );
        assert_eq!(m.presion, Mando::Vale(Variabilidad::Constante));
        let m = Mandos::de_herramienta(
            Herramienta::Linea,
            &EstiloDibujo::default(),
            Variabilidad::Constante,
        );
        assert_eq!(m.presion, Mando::Fuera);
    }

    /// El panel de una herramienta sin nada elegido, con este «actual».
    fn de_herramienta(h: Herramienta, estilo: EstiloDibujo) -> PanelLateral {
        let props = crate::propiedades::de_herramienta(h);
        PanelLateral::construir(
            area(),
            100,
            ContextoPanel {
                estilo,
                mandos: Mandos::de_herramienta(h, &estilo, Default::default()),
                ..ctx(props, 0)
            },
        )
        .unwrap()
    }

    #[test]
    fn sin_nada_elegido_la_herramienta_de_texto_ofrece_la_letra_actual() {
        // Como en Excalidraw: la letra elegida antes de escribir es la del
        // texto nuevo (`gesto::pulsar` la lee del «actual»).
        let estilo = EstiloDibujo {
            tamano_letra: 28.0,
            familia: Some(pixpin_motor2d::texto::FUENTE_NUNITO),
            ..EstiloDibujo::default()
        };
        let p = de_herramienta(Herramienta::Texto, estilo);
        let s = secciones(&p);
        assert!(s.contains(&Seccion::Fuente) && s.contains(&Seccion::TamanoFuente));
        let m = marcados(&p);
        assert!(m.contains(&AccionPanel::Forma(CambioForma::TamanoLetra(28.0))));
        assert!(m.contains(&AccionPanel::Forma(CambioForma::Familia(
            pixpin_motor2d::texto::FUENTE_NUNITO
        ))));
    }

    #[test]
    fn con_los_pasos_la_cota_o_el_punto_en_la_mano_sale_la_letra() {
        // «Poder elegir el tipo de letra en todo lo que tenga letra».
        let estilo = EstiloDibujo {
            familia: Some(pixpin_motor2d::texto::FUENTE_CAVEAT),
            ..EstiloDibujo::default()
        };
        for h in [Herramienta::Serie, Herramienta::Cota, Herramienta::Punto] {
            let p = de_herramienta(h, estilo);
            assert!(secciones(&p).contains(&Seccion::Fuente), "{h:?}");
            assert!(
                marcados(&p).contains(&AccionPanel::Forma(CambioForma::Familia(
                    pixpin_motor2d::texto::FUENTE_CAVEAT
                ))),
                "{h:?}"
            );
        }
        // Los pasos, ademas, su tamano (el del circulo).
        assert!(
            secciones(&de_herramienta(Herramienta::Serie, estilo)).contains(&Seccion::TamanoFuente)
        );
        // Caso negativo: el rectangulo no tiene letra.
        assert!(
            !secciones(&de_herramienta(Herramienta::Rectangulo, estilo)).contains(&Seccion::Fuente)
        );
    }

    #[test]
    fn un_foco_elegido_ofrece_oscurecer_y_zona_con_los_suyos_marcados() {
        let e = Elemento {
            figura: Figura::Foco {
                cristal: pixpin_motor2d::lupa_elemento::Cristal {
                    foco_ancho: Some(100.0),
                    foco_alto: Some(50.0),
                    ..Default::default()
                },
            },
            ancho: 180.0,
            alto: 90.0,
            ..Default::default()
        };
        let m = Mandos::de_seleccion(&[&e]);
        // Los de fabrica del movil: 45 % y la figura en 1/1,8 del marco.
        assert_eq!(m.foco_oscurecer, Mando::Vale(45));
        assert_eq!(m.foco_zona, Mando::Vale(0.56));
        // Caso negativo: un rectangulo no las ofrece.
        let r = Elemento {
            figura: Figura::Rectangulo,
            ..Default::default()
        };
        let m = Mandos::de_seleccion(&[&r]);
        assert!(!m.foco_oscurecer.sale() && !m.foco_zona.sale());
    }

    #[test]
    fn un_numero_de_serie_elegido_marca_su_letra_y_sin_elegir_ninguna() {
        let mut e = Elemento {
            figura: Figura::Serie { numero: 2 },
            ancho: 36.0,
            alto: 36.0,
            ..Default::default()
        };
        let m = Mandos::de_seleccion(&[&e]);
        assert_eq!(m.familia, Mando::Mixto, "la del sistema: sale sin marcar");
        assert_eq!(m.tamano, Mando::Vale(20.0), "36 de circulo = letra 20");
        e.extras.familia = Some("Nunito".into());
        let m = Mandos::de_seleccion(&[&e]);
        assert_eq!(m.familia, Mando::Vale(pixpin_motor2d::texto::FUENTE_NUNITO));
    }

    #[test]
    fn con_el_rectangulo_puesto_salen_los_bordes_con_el_actual_marcado() {
        let redondo = EstiloDibujo {
            redondo: true,
            ..EstiloDibujo::default()
        };
        for h in [Herramienta::Rectangulo, Herramienta::Rombo] {
            let p = de_herramienta(h, redondo);
            assert!(
                marcados(&p).contains(&AccionPanel::Forma(CambioForma::Bordes { redondo: true })),
                "{h:?}"
            );
        }
        // Caso negativo: la elipse y la linea no tienen esquinas que ofrecer.
        for h in [Herramienta::Elipse, Herramienta::Linea] {
            assert!(!secciones(&de_herramienta(h, redondo)).contains(&Seccion::Bordes));
        }
    }

    #[test]
    fn con_la_flecha_puesta_salen_tipo_y_puntas_y_la_de_codos_no_ofrece_tipo() {
        let s = secciones(&de_herramienta(
            Herramienta::Flecha,
            EstiloDibujo::default(),
        ));
        assert!(s.contains(&Seccion::TipoFlecha) && s.contains(&Seccion::Puntas));
        let s = secciones(&de_herramienta(
            Herramienta::FlechaCodos,
            EstiloDibujo::default(),
        ));
        assert!(!s.contains(&Seccion::TipoFlecha) && s.contains(&Seccion::Puntas));
    }

    #[test]
    fn agrupar_sale_con_dos_sueltos_y_desagrupar_con_un_grupo() {
        let a = elem(1, Figura::Rectangulo);
        let b = elem(2, Figura::Elipse);
        let m = Mandos::de_seleccion(&[&a, &b]);
        assert!(m.agrupar && !m.desagrupar);

        let en_grupo = |e: &Elemento| Elemento {
            grupos: vec!["g1".into()],
            ..e.clone()
        };
        let (ga, gb) = (en_grupo(&a), en_grupo(&b));
        let m = Mandos::de_seleccion(&[&ga, &gb]);
        // Caso negativo: ya son un grupo, agruparlos otra vez no hace nada.
        assert!(!m.agrupar && m.desagrupar);
        // Uno solo nunca se agrupa.
        assert!(!Mandos::de_seleccion(&[&a]).agrupar);

        let p = de_seleccion(&[&ga, &gb], None);
        rect_de(&p, AccionPanel::Desagrupar);
    }

    #[test]
    fn el_tipo_de_flecha_ofrece_afilada_curva_y_codos_y_marca_la_curva() {
        let curva = Elemento {
            redondo: true,
            ..elem(1, flecha(TipoPunta::Ninguna, TipoPunta::Flecha))
        };
        let p = de_seleccion(&[&curva], None);
        for t in [TipoFlecha::Afilada, TipoFlecha::Curva, TipoFlecha::Codos] {
            let a = AccionPanel::Forma(CambioForma::TipoFlecha(t));
            assert_eq!(p.destino(centro(rect_de(&p, a))), DestinoPanel::Accion(a));
        }
        let m = marcados(&p);
        assert!(m.contains(&AccionPanel::Forma(CambioForma::TipoFlecha(
            TipoFlecha::Curva
        ))));
        // Caso negativo: una curva y una afilada juntas no marcan ninguna.
        let recta = elem(2, flecha(TipoPunta::Ninguna, TipoPunta::Flecha));
        let mezcla = de_seleccion(&[&curva, &recta], None);
        assert!(
            !marcados(&mezcla)
                .iter()
                .any(|a| matches!(a, AccionPanel::Forma(CambioForma::TipoFlecha(_))))
        );
        // Con la herramienta puesta, marca el «actual».
        let estilo = EstiloDibujo {
            curva: true,
            ..EstiloDibujo::default()
        };
        assert!(
            marcados(&de_herramienta(Herramienta::Flecha, estilo)).contains(&AccionPanel::Forma(
                CambioForma::TipoFlecha(TipoFlecha::Curva)
            ))
        );
    }

    #[test]
    fn un_texto_ofrece_la_alineacion_y_solo_el_rotulo_de_una_caja_la_vertical() {
        let suelto = elem(1, texto());
        let p = de_seleccion(&[&suelto], None);
        assert!(secciones(&p).contains(&Seccion::AlineacionTexto));
        // Sin `textAlign` se ve a la izquierda, y es la que va marcada.
        assert!(
            marcados(&p).contains(&AccionPanel::Forma(CambioForma::Alineacion(
                AlineacionTexto::Izquierda
            )))
        );
        let vertical = |p: &PanelLateral| {
            p.controles.iter().any(|c| {
                matches!(
                    c.accion(),
                    Some(AccionPanel::Forma(CambioForma::AlineacionVertical(_)))
                )
            })
        };
        // Caso negativo: un texto suelto no tiene donde subir ni bajar.
        assert!(!vertical(&p));

        let mut rotulo = elem(2, texto());
        rotulo.extras.contenedor = Some("caja".into());
        rotulo.extras.alineacion = Some(AlineacionTexto::Centro);
        rotulo.extras.alineacion_vertical = Some(AlineacionVertical::Medio);
        let p = de_seleccion(&[&rotulo], None);
        assert!(vertical(&p));
        let m = marcados(&p);
        assert!(m.contains(&AccionPanel::Forma(CambioForma::Alineacion(
            AlineacionTexto::Centro
        ))));
        assert!(
            m.contains(&AccionPanel::Forma(CambioForma::AlineacionVertical(
                AlineacionVertical::Medio
            )))
        );
        // Y al pulsar, la vertical solo le llega al rotulo, no al suelto.
        let abajo = AccionPanel::Forma(CambioForma::AlineacionVertical(AlineacionVertical::Abajo));
        assert_eq!(destinatarios(&abajo, &[&suelto, &rotulo]), vec![2]);

        // La herramienta de texto ofrece la de los renglones con el actual,
        // y la vertical no (sin figura elegida no dice nada).
        let estilo = EstiloDibujo {
            alineacion: AlineacionTexto::Derecha,
            ..EstiloDibujo::default()
        };
        let p = de_herramienta(Herramienta::Texto, estilo);
        assert!(
            marcados(&p).contains(&AccionPanel::Forma(CambioForma::Alineacion(
                AlineacionTexto::Derecha
            )))
        );
        assert!(!vertical(&p));
        // Caso negativo: un rectangulo no alinea renglones.
        let caja = elem(3, Figura::Rectangulo);
        assert!(!secciones(&de_seleccion(&[&caja], None)).contains(&Seccion::AlineacionTexto));
    }

    #[test]
    fn el_codigo_se_escribe_con_o_sin_almohadilla_y_solo_con_seis_digitos() {
        assert_eq!(color_de_codigo("#e03131"), Some(hex(0xe03131)));
        assert_eq!(color_de_codigo("E03131"), Some(hex(0xe03131)));
        assert_eq!(color_de_codigo(" 1971c2 "), Some(hex(0x1971c2)));
        // Casos negativos: corto, largo, con letras que no son hex, vacio.
        for malo in ["#f00", "e031311", "e0313g", "", "#", "transparent"] {
            assert_eq!(color_de_codigo(malo), None, "{malo}");
        }
        assert_eq!(codigo_hex(Some(hex(0x1e1e1e))).as_deref(), Some("1e1e1e"));
        assert_eq!(codigo_hex(None), None);
    }

    #[test]
    fn la_primera_tecla_sustituye_el_codigo_y_no_caben_mas_de_seis() {
        let mut e = EdicionHex::nueva(Some(hex(0x1e1e1e)));
        assert_eq!(e.texto(), "1e1e1e");
        assert!(e.todo_elegido());
        // La almohadilla se acepta y no se guarda.
        assert!(!e.escribir('#'));
        assert!(e.escribir('A'));
        assert_eq!(e.texto(), "a", "la primera tecla sustituye lo elegido");
        for c in "bcdef".chars() {
            assert!(e.escribir(c));
        }
        assert_eq!(e.color(), Some(hex(0xabcdef)));
        // Casos negativos: un septimo digito y una letra que no es hex.
        assert!(!e.escribir('0'));
        assert!(!e.escribir('z'));
        assert_eq!(e.texto(), "abcdef");
        assert!(e.borrar());
        assert_eq!(e.color(), None, "cinco digitos no son un color");
        // Retroceso con todo elegido lo vacia de una vez.
        let mut f = EdicionHex::nueva(Some(hex(0x123456)));
        assert!(f.borrar());
        assert_eq!(f.texto(), "");
        assert!(!f.borrar(), "vacio no hay nada que borrar");
    }

    #[test]
    fn pulsar_el_campo_del_codigo_pide_escribir_en_el() {
        let caja = elem(1, Figura::Rectangulo);
        let p = de_seleccion(&[&caja], Some(Desplegable::ColorTrazo));
        let em = p.emergente.as_ref().unwrap();
        let campo = em
            .controles
            .iter()
            .find_map(|c| match *c {
                Control::Hex { rect, .. } => Some(rect),
                _ => None,
            })
            .expect("hay campo");
        assert_eq!(
            p.destino(centro(campo)),
            DestinoPanel::Accion(AccionPanel::EditarHex)
        );
    }

    #[test]
    fn la_muestra_del_color_actual_abre_la_paleta_de_quince_con_tonos_y_codigo() {
        let caja = Elemento {
            trazo: hex(0x1971c2),
            ..elem(1, Figura::Rectangulo)
        };
        let cerrado = de_seleccion(&[&caja], None);
        let abrir = AccionPanel::Abrir(Desplegable::ColorTrazo);
        assert_eq!(
            cerrado.destino(centro(rect_de(&cerrado, abrir))),
            DestinoPanel::Accion(abrir)
        );

        let p = de_seleccion(&[&caja], Some(Desplegable::ColorTrazo));
        let em = p.emergente.as_ref().unwrap();
        let titulos: Vec<_> = em
            .controles
            .iter()
            .filter_map(|c| match c {
                Control::Titulo { seccion, .. } => Some(*seccion),
                _ => None,
            })
            .collect();
        assert_eq!(
            titulos,
            vec![Seccion::Colores, Seccion::Tonos, Seccion::CodigoHex]
        );
        // 14 de la rejilla (el transparente no es un trazo) y 5 tonos.
        let muestras = em
            .controles
            .iter()
            .filter(|c| matches!(c, Control::Muestra { .. }))
            .count();
        assert_eq!(muestras, 14 + 5);
        // Un tono mas claro del mismo azul, pulsado, cambia el trazo.
        let claro = AccionPanel::Estilo(CambioEstilo::Trazo(hex(0x4dabf7)));
        assert_eq!(
            p.destino(centro(rect_de(&p, claro))),
            DestinoPanel::Accion(claro)
        );
        // El hueco del desplegable no es lienzo.
        assert_eq!(
            p.destino(Punto {
                x: em.marco.x + 2,
                y: em.marco.y + 2
            }),
            DestinoPanel::Panel
        );
    }

    #[test]
    fn la_paleta_de_fondo_si_tiene_transparente_y_sin_tonos_para_el_blanco() {
        let caja = Elemento {
            relleno: Some(hex(0xffffff)),
            ..elem(1, Figura::Rectangulo)
        };
        let p = de_seleccion(&[&caja], Some(Desplegable::ColorFondo));
        let em = p.emergente.as_ref().unwrap();
        rect_de(&p, AccionPanel::Estilo(CambioEstilo::Relleno(None)));
        // El blanco no tiene familia de tonos: no sale esa fila.
        assert!(!em.controles.iter().any(|c| matches!(
            c,
            Control::Titulo {
                seccion: Seccion::Tonos,
                ..
            }
        )));
        // Cada casilla con tonos ensena el 1, el de fondo: la roja, #ffc9c9.
        rect_de(
            &p,
            AccionPanel::Estilo(CambioEstilo::Relleno(Some(hex(0xffc9c9)))),
        );
    }

    fn panel_del_papel(papel: ColorRgba, abierto: Option<Desplegable>) -> PanelLateral {
        // Con un rectangulo elegido: lo elegido no tiene que asomar.
        let caja = elem(1, Figura::Rectangulo);
        let elegidos = [&caja];
        let props = propiedades_de_seleccion(&elegidos);
        PanelLateral::construir(
            area(),
            100,
            ContextoPanel {
                propiedades: &props,
                estilo: estilo_de_seleccion(&elegidos).unwrap(),
                seleccionados: 1,
                mandos: Mandos::de_seleccion(&elegidos),
                abierto,
                lienzo: Some(papel),
            },
        )
        .expect("el papel siempre tiene panel")
    }

    #[test]
    fn el_papel_pedido_ensena_los_cinco_de_excalidraw_y_nada_de_lo_elegido() {
        let p = panel_del_papel(hex(0xfffce8), None);
        for c in COLORES_LIENZO {
            let r = rect_de(&p, AccionPanel::FondoLienzo(c));
            assert_eq!(
                p.destino(centro(r)),
                DestinoPanel::Accion(AccionPanel::FondoLienzo(c))
            );
        }
        // El actual va marcado: el amarillo, y no el blanco.
        assert!(p.controles.iter().any(|c| matches!(
            c,
            Control::Muestra { color: Some(k), activa: true, grande: false, .. } if *k == hex(0xfffce8)
        )));
        // Caso negativo: nada del rectangulo elegido (ni trazo ni capas).
        assert!(!p.controles.iter().any(|c| matches!(
            c,
            Control::Titulo {
                seccion: Seccion::Trazo | Seccion::Capas | Seccion::Acciones,
                ..
            }
        )));
    }

    #[test]
    fn la_paleta_del_papel_trae_los_papeles_del_movil_el_codigo_y_ningun_transparente() {
        let p = panel_del_papel(hex(0xffffff), Some(Desplegable::ColorLienzo));
        let em = p.emergente.as_ref().expect("la paleta del papel se abre");
        assert_eq!(em.cual, Desplegable::ColorLienzo);
        // Los trece del movil, con su valor exacto, cada uno pulsable.
        for v in PAPELES_DEL_MOVIL {
            let r = rect_de(&p, AccionPanel::FondoLienzo(hex(v)));
            assert!(em.marco.contiene(centro(r)) || p.marco.contiene(centro(r)));
        }
        // La paleta con el tono claro: el azul es su 50, #e7f5ff.
        rect_de(&p, AccionPanel::FondoLienzo(hex(0xe7f5ff)));
        assert!(
            em.controles
                .iter()
                .any(|c| matches!(c, Control::Hex { .. }))
        );
        // Caso negativo: sin papel no hay lienzo, asi que no hay transparente
        // ni nada que cambie el estilo de lo elegido.
        assert!(!em.controles.iter().any(|c| matches!(
            c,
            Control::Muestra { color: None, .. }
                | Control::Muestra {
                    accion: Some(AccionPanel::Estilo(_)),
                    ..
                }
        )));
    }

    #[test]
    fn sin_pedir_el_papel_el_panel_no_lo_ensena() {
        // Caso negativo: el panel de siempre no gana una seccion de papel.
        let caja = elem(1, Figura::Rectangulo);
        let p = de_seleccion(&[&caja], None);
        assert!(!p.controles.iter().any(|c| matches!(
            c,
            Control::Muestra {
                accion: Some(AccionPanel::FondoLienzo(_)),
                ..
            }
        )));
    }

    #[test]
    fn un_desplegable_de_algo_que_ya_no_esta_elegido_no_sale() {
        // Abierto el de puntas y elegido un rectangulo: no hay disparador,
        // asi que no hay desplegable colgando de nada.
        let caja = elem(1, Figura::Rectangulo);
        assert!(
            de_seleccion(&[&caja], Some(Desplegable::PuntaFin))
                .emergente
                .is_none()
        );
    }

    #[test]
    fn todos_los_controles_caben_dentro_del_panel_y_no_se_pisan() {
        // Lo mas alto que existe: rectangulo con fondo, texto, flecha y
        // lapiz, tres o mas elegidos y en grupo; a 150 % y con la paleta
        // abierta.
        let caja = Elemento {
            relleno: COLORES_FONDO[1],
            grupos: vec!["g".into()],
            ..elem(1, Figura::Rectangulo)
        };
        let (t, f, l) = (
            elem(2, texto()),
            elem(3, flecha(TipoPunta::Barra, TipoPunta::Flecha)),
            elem(4, lapiz()),
        );
        let elegidos = [&caja, &t, &f, &l];
        for abierto in [
            None,
            Some(Desplegable::ColorFondo),
            Some(Desplegable::PuntaInicio),
        ] {
            let props = propiedades_de_seleccion(&elegidos);
            let p = PanelLateral::construir(
                area(),
                150,
                ContextoPanel {
                    propiedades: &props,
                    estilo: estilo_de_seleccion(&elegidos).unwrap(),
                    seleccionados: elegidos.len(),
                    mandos: Mandos::de_seleccion(&elegidos),
                    abierto,
                    lienzo: None,
                },
            )
            .unwrap();
            assert_eq!(
                area().interseccion(p.marco),
                Some(p.marco),
                "el panel se sale"
            );
            let mut grupos = vec![(p.marco, p.controles.clone())];
            if let Some(em) = &p.emergente {
                assert!(em.marco.interseccion(p.marco).is_none(), "tapa el panel");
                assert_eq!(area().interseccion(em.marco), Some(em.marco), "se sale");
                grupos.push((em.marco, em.controles.clone()));
            }
            for (marco, controles) in grupos {
                let rects: Vec<Rect> = controles.iter().map(Control::rect).collect();
                for (i, r) in rects.iter().enumerate() {
                    assert_eq!(marco.interseccion(*r), Some(*r), "control {i} fuera: {r:?}");
                    for (j, s) in rects.iter().enumerate().skip(i + 1) {
                        assert!(r.interseccion(*s).is_none(), "{i} pisa a {j}");
                    }
                }
            }
        }
    }

    #[test]
    fn si_no_cabe_de_alto_abre_otra_columna_en_vez_de_cortarse() {
        let caja = Elemento {
            relleno: COLORES_FONDO[1],
            ..elem(1, Figura::Rectangulo)
        };
        let (t, f) = (
            elem(2, texto()),
            elem(3, flecha(TipoPunta::Ninguna, TipoPunta::Flecha)),
        );
        let elegidos = [&caja, &t, &f];
        let props = propiedades_de_seleccion(&elegidos);
        let monta = |alto: u32| {
            PanelLateral::construir(
                Rect {
                    x: 0,
                    y: 0,
                    ancho: 1600,
                    alto,
                },
                100,
                ContextoPanel {
                    propiedades: &props,
                    estilo: estilo_de_seleccion(&elegidos).unwrap(),
                    seleccionados: elegidos.len(),
                    mandos: Mandos::de_seleccion(&elegidos),
                    abierto: None,
                    lienzo: None,
                },
            )
            .unwrap()
        };
        let alto = monta(2000);
        assert_eq!(
            alto.marco.ancho, ANCHO_LOGICO,
            "con sitio, una sola columna"
        );
        let bajo = monta(560);
        assert!(bajo.marco.ancho > ANCHO_LOGICO, "tiene que ensancharse");
        assert!(bajo.marco.abajo() <= 560 - 16);
        // Caso negativo: ninguna seccion se parte entre dos columnas. Todas
        // las de la segunda empiezan a la altura de la primera.
        let segunda: Vec<Rect> = bajo
            .controles
            .iter()
            .map(Control::rect)
            .filter(|r| r.x >= bajo.marco.x + ANCHO_LOGICO as i32)
            .collect();
        assert!(!segunda.is_empty());
        assert_eq!(
            segunda.iter().map(|r| r.y).min(),
            Some(bajo.marco.y + RELLENO_LOGICO as i32),
            "la segunda columna empieza arriba"
        );
        // Y los mismos controles en los dos casos: solo cambia donde van.
        assert_eq!(alto.controles.len(), bajo.controles.len());
    }

    #[test]
    fn un_clic_en_el_color_rojo_cambia_el_trazo_a_rojo() {
        let p = PanelLateral::construir(area(), 100, ctx(&[Propiedad::ColorTrazo], 0)).unwrap();
        let rojo = AccionPanel::Estilo(CambioEstilo::Trazo(COLORES_TRAZO[1]));
        assert_eq!(
            p.destino(centro(rect_de(&p, rojo))),
            DestinoPanel::Accion(rojo)
        );
    }

    #[test]
    fn la_opacidad_se_elige_por_donde_se_pulsa_en_pasos_de_diez() {
        let p = PanelLateral::construir(area(), 100, ctx(&[Propiedad::Opacidad], 0)).unwrap();
        let pista = p
            .controles
            .iter()
            .find_map(|c| match *c {
                Control::Deslizador { rect, .. } => Some(rect),
                _ => None,
            })
            .unwrap();
        let en = |fraccion: f32| Punto {
            x: pista.x + (pista.ancho as f32 * fraccion) as i32,
            y: pista.y + 2,
        };
        assert_eq!(
            p.destino(en(0.52)),
            DestinoPanel::Accion(AccionPanel::Estilo(CambioEstilo::Opacidad(0.5)))
        );
        assert_eq!(
            p.destino(en(0.0)),
            DestinoPanel::Accion(AccionPanel::Estilo(CambioEstilo::Opacidad(0.0)))
        );
    }

    #[test]
    fn alinear_sale_con_dos_elegidos_y_repartir_solo_con_tres() {
        let cuenta = |n: usize| {
            PanelLateral::construir(area(), 100, ctx(&[], n)).map_or(0, |p| {
                p.controles
                    .iter()
                    .filter(|c| {
                        matches!(
                            c,
                            Control::Opcion {
                                accion: AccionPanel::Alinear(_) | AccionPanel::Repartir(_),
                                ..
                            }
                        )
                    })
                    .count()
            })
        };
        assert_eq!(cuenta(1), 0);
        assert_eq!(cuenta(2), 6);
        assert_eq!(cuenta(3), 8);
    }

    #[test]
    fn alinear_va_en_dos_filas_horizontal_arriba_y_vertical_abajo() {
        let p = PanelLateral::construir(area(), 100, ctx(&[], 3)).unwrap();
        let fila = |a| rect_de(&p, a).y;
        let arriba = fila(AccionPanel::Alinear(Alineacion::Izquierda));
        assert_eq!(fila(AccionPanel::Repartir(Reparto::Horizontal)), arriba);
        let abajo = fila(AccionPanel::Alinear(Alineacion::Arriba));
        assert!(abajo > arriba);
        assert_eq!(fila(AccionPanel::Repartir(Reparto::Vertical)), abajo);
    }

    #[test]
    fn el_hueco_del_panel_no_es_lienzo_y_fuera_si() {
        let p = PanelLateral::construir(area(), 100, ctx(&[Propiedad::Grosor], 0)).unwrap();
        assert_eq!(
            p.destino(Punto {
                x: p.marco.x + 2,
                y: p.marco.y + 2
            }),
            DestinoPanel::Panel
        );
        assert_eq!(
            p.destino(Punto {
                x: p.marco.derecha() + 5,
                y: p.marco.y + 5
            }),
            DestinoPanel::Fuera
        );
    }
}

#[cfg(test)]
mod pruebas_serie_de_punto {
    //! **Numerar los puntos** desde el panel: con el punto en la mano sale la
    //! fila A B C / a b c / 1 2 3 y dice cual esta puesta.
    use super::*;
    use pixpin_motor2d::puntos_etiquetados::SerieDePunto;

    fn panel(h: Herramienta, serie: SerieDePunto) -> Option<PanelLateral> {
        let estilo = EstiloDibujo::default();
        let props = propiedades::de_herramienta(h);
        PanelLateral::construir(
            Rect {
                x: 0,
                y: 0,
                ancho: 1200,
                alto: 900,
            },
            100,
            ContextoPanel {
                propiedades: props,
                estilo,
                seleccionados: 0,
                mandos: Mandos::de_herramienta(h, &estilo, Default::default())
                    .con_serie_de_punto(h, serie),
                abierto: None,
                lienzo: None,
            },
        )
    }

    fn series(p: &PanelLateral) -> Vec<(SerieDePunto, bool)> {
        p.controles
            .iter()
            .filter_map(|c| match c {
                Control::Opcion {
                    accion: AccionPanel::SeriePunto(s),
                    activa,
                    ..
                } => Some((*s, *activa)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn con_el_punto_en_la_mano_sale_la_fila_de_series_con_la_puesta_marcada() {
        let p = panel(Herramienta::Punto, SerieDePunto::Numeros).expect("sin panel");
        assert_eq!(
            series(&p),
            vec![
                (SerieDePunto::Mayusculas, false),
                (SerieDePunto::Minusculas, false),
                (SerieDePunto::Numeros, true),
            ]
        );
        let r = p
            .controles
            .iter()
            .find_map(|c| match c {
                Control::Opcion {
                    rect,
                    accion: AccionPanel::SeriePunto(SerieDePunto::Minusculas),
                    ..
                } => Some(*rect),
                _ => None,
            })
            .unwrap();
        let centro = Punto {
            x: r.x + r.ancho as i32 / 2,
            y: r.y + r.alto as i32 / 2,
        };
        assert_eq!(
            p.destino(centro),
            DestinoPanel::Accion(AccionPanel::SeriePunto(SerieDePunto::Minusculas))
        );
    }

    #[test]
    fn con_otra_herramienta_no_sale_la_fila_de_series() {
        // Caso negativo: la serie no le dice nada a un lapiz.
        let p = panel(Herramienta::Lapiz, SerieDePunto::Numeros).expect("sin panel");
        assert!(series(&p).is_empty());
    }
}

#[cfg(test)]
mod pruebas_pedir_medida {
    //! **La cota: pedir la medida al trazarla, si o no** (`pedirLaMedida`).
    use super::*;

    fn opciones(h: Herramienta, pedir: bool) -> Vec<(bool, bool)> {
        let estilo = EstiloDibujo::default();
        let p = PanelLateral::construir(
            Rect {
                x: 0,
                y: 0,
                ancho: 1200,
                alto: 900,
            },
            100,
            ContextoPanel {
                propiedades: propiedades::de_herramienta(h),
                estilo,
                seleccionados: 0,
                mandos: Mandos::de_herramienta(h, &estilo, Default::default())
                    .con_pedir_la_medida(h, pedir),
                abierto: None,
                lienzo: None,
            },
        )
        .expect("sin panel");
        p.controles
            .iter()
            .filter_map(|c| match c {
                Control::Opcion {
                    accion: AccionPanel::PedirMedida(v),
                    activa,
                    ..
                } => Some((*v, *activa)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn con_la_cota_en_la_mano_sale_el_si_y_el_no_con_el_puesto_marcado() {
        assert_eq!(
            opciones(Herramienta::Cota, true),
            vec![(true, true), (false, false)]
        );
        assert_eq!(
            opciones(Herramienta::Cota, false),
            vec![(true, false), (false, true)]
        );
    }

    #[test]
    fn con_la_linea_no_sale_el_interruptor_de_la_cota() {
        assert!(opciones(Herramienta::Linea, true).is_empty());
    }
}
