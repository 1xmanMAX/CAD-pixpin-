//! Escribir: donde esta el cursor dentro de una cadena y que le hace cada
//! tecla.
//!
//! Aqui no hay ventana, ni escena, ni elemento: solo una cadena y un cursor.
//! Por eso esta separado de `gesto.rs`, que es quien sabe de escenas, y de
//! `ventana_editor.rs`, que es quien sabe de teclas de Windows. Lo que falla
//! de verdad al escribir -que el cursor se salga, que Retroceso parta una
//! letra por la mitad- se prueba entero aqui, sin abrir nada.
//!
//! # El cursor va en CARACTERES, nunca en bytes
//!
//! Es la decision entera de este fichero. El texto es UTF-8: una `ñ` ocupa
//! dos bytes y un emoji cuatro. Un cursor en bytes que avanzara de uno en
//! uno acabaria apuntando a mitad de una letra, y `String::insert` y
//! `String::remove` entran en panico exactamente ahi. Con el cursor en
//! caracteres eso no puede pasar: el indice de byte se calcula cuando hace
//! falta, siempre desde `char_indices`, que solo da comienzos de caracter.
//!
//! Lo que esto NO resuelve, y se sabe: un grafema compuesto (una `e` seguida
//! de una tilde combinante, o una familia de emojis con uniones de ancho
//! cero) son varios caracteres y se borran de uno en uno. Es lo que hace
//! Notepad, y arreglarlo pide una tabla de Unicode que no cabe sin
//! dependencias nuevas.

/// Cuanto separa dos renglones, en multiplos del tamano de letra, cuando la
/// familia no trae el suyo (`interlineado_de`). El 1,25 de Excalidraw
/// (`lineHeight` de Excalifont) y del movil (`DrawFonts.medirTexto`).
///
/// Publica porque quien pinta el cursor necesita el mismo numero: dos
/// interlineados distintos ponen la barra entre dos lineas.
pub const INTERLINEADO: f32 = 1.25;

/// Lo que se supone que ocupa de ancho un caracter, en multiplos del tamano
/// de letra. Es un promedio a ojo: el motor no tiene DirectWrite y no puede
/// medir de verdad (la misma renuncia que ya hacia el anotador de pines).
const ANCHO_POR_CARACTER: f32 = 0.62;

/// El tamano de letra con el que nace un texto nuevo, en unidades del mundo.
pub const TAM_POR_DEFECTO: f32 = 20.0;

/// El `ancho_max` de un texto que no se parte: a partir de aqui quien pinta
/// no reparte renglones (`pixpin_render::letras::SIN_PARTIR`, el mismo
/// numero). Un texto suelto de Excalidraw no se parte nunca.
pub const SIN_PARTIR: f32 = 1.0e6;

/// La letra con la que nace un texto nuevo. La misma que `Figura::Texto` da
/// por defecto al leer un fichero sin `familia`.
pub const FAMILIA_POR_DEFECTO: &str = "Excalifont";

/// La letra de los rotulos que no son texto del usuario (por ejemplo,
/// las piezas de una ecuacion): la de la interfaz de Windows.
pub const FAMILIA_DEL_SISTEMA: &str = "Segoe UI";

/// Cuanto se inclina la cursiva, en unidades de sesgo horizontal por unidad de
/// alto (`SESGO_DE_LA_CURSIVA` del movil).
///
/// Un cuarto es lo que usa el propio Android para su italica falsa: mas se lee
/// como un error de pintado y menos no se distingue de la letra recta. Es
/// italica **falsa** en los dos aparatos y a proposito: las tres fuentes de
/// Excalidraw no traen corte italico, asi que o se sesga o no hay cursiva.
pub const SESGO_DE_LA_CURSIVA: f32 = -0.25;

/// A que altura del renglon va la raya del tachado, en multiplos del tamano de
/// letra medidos desde la linea de base hacia arriba.
///
/// Por la mitad de la equis, que es donde la pone cualquier tipografia: mas
/// arriba parece un subrayado del renglon de encima y mas abajo se confunde
/// con la linea de base.
pub const ALTURA_DEL_TACHADO: f32 = 0.30;

/// Lo gorda que va la raya del tachado, en multiplos del tamano de letra.
pub const GROSOR_DEL_TACHADO: f32 = 0.06;

/// **Como va escrito un texto**: negrita, cursiva y tachado.
///
/// Son los tres campos propios de PixPin (`Element.kt:686-688`) y el propio
/// codigo del movil avisa de que se pierden al exportar a `.excalidraw`
/// —Excalidraw no los tiene—, pero **dentro del puente PixPin tienen que
/// viajar**: sin ellos, el titulo en negrita de un esquema vuelve del PC en
/// redonda y lo tachado deja de estar tachado.
///
/// Va como tipo y no como tres booleanos sueltos porque los tres se piden, se
/// pintan y se guardan juntos: quien pinta un texto necesita los tres o
/// ninguno.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EstiloDeTexto {
    pub negrita: bool,
    pub cursiva: bool,
    pub tachado: bool,
}

impl EstiloDeTexto {
    /// Si no hay nada que aplicar: el texto se pinta tal cual.
    pub fn liso(self) -> bool {
        self == EstiloDeTexto::default()
    }

    /// El sesgo horizontal que hay que aplicar al pintar, o cero.
    pub fn sesgo(self) -> f32 {
        if self.cursiva {
            SESGO_DE_LA_CURSIVA
        } else {
            0.0
        }
    }
}

/// La raya que cruza un renglon tachado: de donde a donde, y lo gorda que va.
pub type RayaDeTachado = ((f32, f32), (f32, f32), f32);

/// La raya del tachado de un renglon.
///
/// `x`/`y` son la esquina de arriba a la izquierda del renglon, como en
/// `Orden::Texto`. `None` cuando no hay nada que tachar —sin la marca, o un
/// renglon vacio—: una raya suelta en el aire se lee como un guion largo.
///
/// Se calcula aqui y no en quien pinta porque es la misma cuenta para el
/// editor, para la exportacion y para la miniatura, y tres copias acabarian
/// poniendo la raya a tres alturas.
pub fn raya_del_tachado(
    texto: &str,
    x: f32,
    y: f32,
    tam: f32,
    familia: &str,
    estilo: EstiloDeTexto,
) -> Option<RayaDeTachado> {
    if !estilo.tachado || texto.trim().is_empty() || tam <= 0.0 {
        return None;
    }
    // Lo que ocupa lo escrito, medido con su letra si hay con que (sin los
    // espacios del final: una raya que sobresale por la derecha se lee como
    // un guion pegado a la palabra).
    let ancho = ancho_real(texto.trim_end(), tam, familia, estilo);
    if ancho <= 0.0 {
        return None;
    }
    // La base del renglon esta a un alto de letra de su borde de arriba; de
    // ahi se sube la altura del tachado.
    let alto_raya = y + tam - tam * ALTURA_DEL_TACHADO;
    Some((
        (x, alto_raya),
        (x + ancho, alto_raya),
        (tam * GROSOR_DEL_TACHADO).max(1.0),
    ))
}

// -------------------------------------------------------------------------
// La alineacion: `textAlign` y `verticalAlign`
// -------------------------------------------------------------------------

/// **A que lado se pegan los renglones** (`textAlign` de Excalidraw y
/// `TextAlign` del movil). Las tres palabras son las del fichero, y tienen que
/// ser exactamente esas: el movil las lee con un `enum` de kotlinx, y una
/// palabra que no conozca le hace rechazar el dibujo entero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AlineacionTexto {
    /// La de siempre, y la que pinta el movil cuando el campo falta.
    #[default]
    Izquierda,
    Centro,
    Derecha,
}

impl AlineacionTexto {
    pub fn palabra(self) -> &'static str {
        match self {
            AlineacionTexto::Izquierda => "left",
            AlineacionTexto::Centro => "center",
            AlineacionTexto::Derecha => "right",
        }
    }

    pub fn desde_palabra(p: &str) -> Option<AlineacionTexto> {
        Some(match p {
            "left" => AlineacionTexto::Izquierda,
            "center" => AlineacionTexto::Centro,
            "right" => AlineacionTexto::Derecha,
            _ => return None,
        })
    }

    /// Que parte del hueco sobrante va a la izquierda del renglon: nada,
    /// la mitad o todo. Es la misma cuenta para el renglon dentro de su caja
    /// y para la caja dentro de su figura.
    pub fn fraccion(self) -> f32 {
        match self {
            AlineacionTexto::Izquierda => 0.0,
            AlineacionTexto::Centro => 0.5,
            AlineacionTexto::Derecha => 1.0,
        }
    }
}

/// **A que altura va el rotulo dentro de su figura** (`verticalAlign`). Solo
/// dice algo en un texto con `containerId`: un texto suelto no tiene hueco
/// en el que subir o bajar, igual que en Excalidraw.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AlineacionVertical {
    /// La de fabrica del movil (`ItemStyle.verticalAlign = TOP`).
    #[default]
    Arriba,
    Medio,
    Abajo,
}

impl AlineacionVertical {
    pub fn palabra(self) -> &'static str {
        match self {
            AlineacionVertical::Arriba => "top",
            AlineacionVertical::Medio => "middle",
            AlineacionVertical::Abajo => "bottom",
        }
    }

    pub fn desde_palabra(p: &str) -> Option<AlineacionVertical> {
        Some(match p {
            "top" => AlineacionVertical::Arriba,
            "middle" => AlineacionVertical::Medio,
            "bottom" => AlineacionVertical::Abajo,
            _ => return None,
        })
    }

    pub fn fraccion(self) -> f32 {
        match self {
            AlineacionVertical::Arriba => 0.0,
            AlineacionVertical::Medio => 0.5,
            AlineacionVertical::Abajo => 1.0,
        }
    }
}

/// Lo que ocupa un renglon, a ojo y **sin** la holgura del cursor. Es la
/// medida con la que se aparta cada renglon al centrarlo o pegarlo a la
/// derecha: con la holgura, un texto a la derecha quedaria despegado del
/// borde un caracter entero.
pub fn ancho_de_renglon(renglon: &str, tam: f32) -> f32 {
    renglon.trim_end().chars().count() as f32 * tam * ANCHO_POR_CARACTER
}

/// **Cuanto se aparta cada renglon de la izquierda de su caja.**
///
/// Como el movil (`Renderer.drawText`): cada renglon se alinea por su cuenta
/// dentro del ancho de la caja, y no el bloque entero. Asi es como se ve un
/// parrafo centrado: el renglon corto en medio del largo, no pegado a el.
///
/// Cada renglon se mide con su letra (`ancho_real`): con el medidor del
/// anfitrion, lo que mide DirectWrite; sin el, a ojo. Nunca saca el texto de
/// su caja: el apartado no baja de cero.
pub fn apartados_de_renglones(
    texto: &str,
    tam: f32,
    familia: &str,
    estilo: EstiloDeTexto,
    ancho_caja: f32,
    a: AlineacionTexto,
) -> Vec<f32> {
    texto
        .split('\n')
        .map(|r| {
            ((ancho_caja - ancho_real(r.trim_end(), tam, familia, estilo)) * a.fraccion()).max(0.0)
        })
        .collect()
}

// -------------------------------------------------------------------------
// Las letras, por su numero: las de Excalidraw y las del lienzo de citas
// -------------------------------------------------------------------------

/// Excalifont, la de a mano. La de por omision.
pub const FUENTE_EXCALIFONT: u8 = 5;
/// Nunito, la «normal», para cuando hace falta que se lea limpio.
pub const FUENTE_NUNITO: u8 = 6;
/// Lilita One, la gorda de rotular (la cuarta del desplegable de Excalidraw).
pub const FUENTE_LILITA_ONE: u8 = 7;
/// Comic Shanns, la monoespaciada.
pub const FUENTE_COMIC_SHANNS: u8 = 8;
/// Work Sans, la «Normal» del lienzo de citas.
pub const FUENTE_WORK_SANS: u8 = 101;
/// Fraunces, la «Serif» del lienzo de citas.
pub const FUENTE_FRAUNCES: u8 = 102;
/// Courier New, la «Maquina» del lienzo de citas (ya viene con Windows).
pub const FUENTE_COURIER_NEW: u8 = 103;
/// Caveat, la «Manuscrita» del lienzo de citas.
pub const FUENTE_CAVEAT: u8 = 104;

/// Una letra que se puede elegir para un texto del lienzo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fuente {
    /// El `fontFamily` que va en el fichero.
    pub id: u8,
    /// Con que nombre se pide a DirectWrite, y con cual la guarda
    /// `Figura::Texto::familia`.
    pub nombre: &'static str,
    /// Como la llama quien la usa: el movil y el lienzo de citas.
    pub etiqueta: &'static str,
    /// La distancia entre renglones en veces el tamano de letra
    /// (`lineHeight` de `font-metadata.ts` de Excalidraw).
    pub interlineado: f32,
    /// `(ascenso, descenso)` de la cara normal, en em, tal como los da
    /// DirectWrite (`DWRITE_FONT_METRICS`, medidos con
    /// `pixpin_render::letras`). Solo hacen falta donde no hay DirectWrite
    /// que coloque la linea base: el SVG.
    pub alturas: (f32, f32),
}

impl Fuente {
    /// Donde cae la linea base del primer renglon, en veces el tamano de
    /// letra desde arriba de la caja: la letra centrada en su renglon,
    /// como la centra CSS y como la pinta `pixpin_render::letras` con el
    /// interlineado fijo (`getVerticalOffset` de Excalidraw).
    pub fn linea_base(&self) -> f32 {
        let (a, d) = self.alturas;
        a + (self.interlineado - (a + d)) / 2.0
    }
}

/// **Las ocho letras del lienzo**, en el orden en que salen en el panel.
///
/// Las cuatro primeras son las que ofrece el selector de Excalidraw (las tres
/// de siempre y Lilita One, del desplegable; las viejas Virgil, Helvetica y
/// Cascadia estan retiradas alli y aqui solo se leen como alias). Las cuatro
/// ultimas son las del lienzo de citas del usuario (`LETRAS` de
/// `tarjetas.js`).
///
/// **Los numeros de las cuatro de citas (101 a 104) no son de Excalidraw**, y
/// a proposito caen fuera de los suyos (1-10, 100 y los de reserva 998, 999 y
/// 1000): el movil resuelve un numero que no conoce a Excalifont sin
/// rechazar el dibujo (`ItemStyle.fontFamilyResuelta`) y guarda el numero tal
/// cual en su `Element`, asi que el texto vuelve al PC con su letra; y
/// Excalidraw lo pinta con su letra de reserva.
pub const FUENTES: [Fuente; 8] = [
    Fuente {
        id: FUENTE_EXCALIFONT,
        nombre: "Excalifont",
        etiqueta: "A mano",
        interlineado: 1.25,
        alturas: (0.886, 0.374),
    },
    Fuente {
        id: FUENTE_NUNITO,
        nombre: "Nunito",
        etiqueta: "Normal",
        interlineado: 1.25,
        alturas: (1.011, 0.353),
    },
    Fuente {
        id: FUENTE_LILITA_ONE,
        nombre: "Lilita One",
        etiqueta: "Lilita One",
        interlineado: 1.15,
        alturas: (0.923, 0.22),
    },
    Fuente {
        id: FUENTE_COMIC_SHANNS,
        nombre: "Comic Shanns",
        etiqueta: "Codigo",
        interlineado: 1.25,
        alturas: (1.167, 0.564),
    },
    Fuente {
        id: FUENTE_WORK_SANS,
        nombre: "Work Sans",
        etiqueta: "Normal (citas)",
        interlineado: 1.25,
        alturas: (0.93, 0.243),
    },
    Fuente {
        id: FUENTE_FRAUNCES,
        nombre: "Fraunces",
        etiqueta: "Serif",
        interlineado: 1.25,
        alturas: (0.978, 0.255),
    },
    Fuente {
        id: FUENTE_COURIER_NEW,
        nombre: "Courier New",
        etiqueta: "Maquina",
        interlineado: 1.25,
        alturas: (0.833, 0.3),
    },
    Fuente {
        id: FUENTE_CAVEAT,
        nombre: "Caveat",
        etiqueta: "Manuscrita",
        interlineado: 1.25,
        alturas: (0.96, 0.3),
    },
];

/// La letra del catalogo con ese numero, sin resolver alias.
pub fn fuente(id: u8) -> Option<&'static Fuente> {
    FUENTES.iter().find(|f| f.id == id)
}

/// La letra del catalogo con ese nombre (sin distinguir mayusculas).
pub fn fuente_por_nombre(nombre: &str) -> Option<&'static Fuente> {
    let n = nombre.trim();
    // «Comic Shanns 2» es como la llamaba el movil en sus primeras versiones.
    let n = if n.eq_ignore_ascii_case("Comic Shanns 2") {
        "Comic Shanns"
    } else {
        n
    };
    FUENTES.iter().find(|f| f.nombre.eq_ignore_ascii_case(n))
}

/// La familia con la que hay que pintar, **resolviendo los alias viejos**.
///
/// Los numeros no son correlativos y **van en el fichero**: 5, 6, 7 y 8, con
/// 1 (Virgil), 2 (Helvetica) y 3 (Cascadia) como alias de los dibujos de
/// antes, y 101 a 104 para las del lienzo de citas. Un dibujo guardado con
/// aquellos numeros tiene que seguir viendose con la letra que le toca, no
/// caer al valor por omision.
pub fn familia_resuelta(id: Option<u8>) -> u8 {
    match id {
        None | Some(1) => FUENTE_EXCALIFONT,
        Some(2) => FUENTE_NUNITO,
        Some(3) => FUENTE_COMIC_SHANNS,
        Some(n) if fuente(n).is_some() => n,
        _ => FUENTE_EXCALIFONT,
    }
}

/// El numero de familia que le corresponde a un nombre de fuente.
///
/// **Esto es lo que impide que el movil reabra el texto con otra letra.** Aqui
/// `Figura::Texto` guarda el nombre de la fuente y el fichero espera un
/// numero: escribir el nombre haria que el movil no reconociera la familia y
/// cayera a la de por omision.
///
/// No adivina: lo que no es del catalogo («Segoe UI») cae en Excalifont, que
/// es lo que hace el movil con un numero que no conoce.
pub fn numero_de_familia(nombre: &str) -> u8 {
    fuente_por_nombre(nombre).map_or(FUENTE_EXCALIFONT, |f| f.id)
}

/// Y al reves: el nombre con el que pedirle esa letra a DirectWrite.
pub fn nombre_de_familia(id: Option<u8>) -> &'static str {
    fuente(familia_resuelta(id)).map_or("Excalifont", |f| f.nombre)
}

/// El interlineado fijo de una familia del catalogo; `None` para las del
/// sistema (Segoe UI de los rotulos), que siguen con el de su fuente.
pub fn interlineado_de(familia: &str) -> Option<f32> {
    fuente_por_nombre(familia).map(|f| f.interlineado)
}

// -------------------------------------------------------------------------
// Medir de verdad: el medidor que pone quien tiene DirectWrite
// -------------------------------------------------------------------------

/// **Lo que mide un texto** con su letra: ancho (con los espacios del final)
/// y alto. Lo pone el anfitrion, que es quien tiene DirectWrite; el motor es
/// puro y sin el mide a ojo (`medida_estimada`).
pub type Medidor =
    fn(texto: &str, tam: f32, familia: &str, estilo: EstiloDeTexto) -> Option<(f32, f32)>;

static MEDIDOR: std::sync::RwLock<Option<Medidor>> = std::sync::RwLock::new(None);

thread_local! {
    /// Un medidor solo para este hilo: el de las pruebas, que no pueden
    /// tocar el global sin pisarse unas a otras.
    static MEDIDOR_DEL_HILO: std::cell::Cell<Option<Medidor>> = const { std::cell::Cell::new(None) };
}

/// Pone el medidor de todo el proceso. Lo llama la aplicacion al arrancar,
/// antes de abrir ningun lienzo.
pub fn instalar_medidor(m: Medidor) {
    if let Ok(mut g) = MEDIDOR.write() {
        *g = Some(m);
    }
}

/// Corre `f` con `m` como medidor de este hilo (para las pruebas).
pub fn con_medidor<R>(m: Medidor, f: impl FnOnce() -> R) -> R {
    let antes = MEDIDOR_DEL_HILO.with(|c| c.replace(Some(m)));
    let r = f();
    MEDIDOR_DEL_HILO.with(|c| c.set(antes));
    r
}

fn medidor() -> Option<Medidor> {
    MEDIDOR_DEL_HILO
        .with(|c| c.get())
        .or_else(|| MEDIDOR.read().ok().and_then(|g| *g))
}

/// Si hay con que medir de verdad.
pub fn hay_medidor() -> bool {
    medidor().is_some()
}

/// El ancho de un renglon con su letra: el de DirectWrite si hay medidor, a
/// ojo si no. Sin holgura: es donde acaba la ultima letra (o el ultimo
/// espacio), que es donde va el cursor.
pub fn ancho_real(renglon: &str, tam: f32, familia: &str, estilo: EstiloDeTexto) -> f32 {
    if renglon.is_empty() {
        return 0.0;
    }
    medidor()
        .and_then(|m| m(renglon, tam, familia, estilo))
        .map_or_else(
            || renglon.chars().count() as f32 * tam * ANCHO_POR_CARACTER,
            |(w, _)| w,
        )
}

/// **La caja de un texto suelto, ajustada a lo escrito**, como la de
/// Excalidraw con `autoResize`: de ancho, el renglon mas largo medido con su
/// letra; de alto, renglones por tamano por el interlineado de su familia
/// (las del sistema, que no tienen uno fijo, lo que diga DirectWrite).
///
/// Un renglon vacio mide un espacio (`measureText` de Excalidraw hace lo
/// mismo): un texto recien abierto tiene que tener donde pulsar y donde
/// pintar el cursor.
pub fn medida(texto: &str, tam: f32, familia: &str, estilo: EstiloDeTexto) -> (f32, f32) {
    let renglones = texto.split('\n').count().max(1);
    let alto_fijo = interlineado_de(familia).map(|k| renglones as f32 * tam * k);
    let relleno = texto
        .split('\n')
        .map(|r| if r.is_empty() { " " } else { r })
        .collect::<Vec<_>>()
        .join("\n");
    let (ancho, alto) = medidor()
        .and_then(|m| m(&relleno, tam, familia, estilo))
        .unwrap_or_else(|| medida_estimada(texto, tam));
    (ancho, alto_fijo.unwrap_or(alto))
}

/// Una tecla de las que mueven o borran mientras se escribe.
///
/// No incluye las letras: esas llegan ya compuestas (con su IME y su
/// acentuacion) como un `char` y van por `EdicionTexto::insertar`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeclaTexto {
    Izquierda,
    Derecha,
    /// Al principio del RENGLON, no del texto entero: es lo que hace
    /// cualquier editor con varias lineas.
    Inicio,
    /// Al final del renglon, por lo mismo.
    Fin,
    /// Borra el caracter de la izquierda del cursor.
    Retroceso,
    /// Borra el caracter de la derecha del cursor.
    Suprimir,
    /// Hace renglon.
    Entrar,
}

/// Una cadena que se esta escribiendo, con su cursor, lo elegido con
/// Mayus y su propio deshacer (15-oct-2026: el texto rico pide elegir un
/// tramo para darle color, y Ctrl+Z dentro del texto deshace lo escrito,
/// no el dibujo).
#[derive(Debug, Clone, Default)]
pub struct EdicionTexto {
    texto: String,
    /// En CARACTERES, nunca en bytes (ve la cabecera del modulo). Invariante:
    /// nunca pasa de `texto.chars().count()`.
    cursor: usize,
    /// El otro extremo de lo elegido (Mayus+flechas), en caracteres.
    ancla: Option<usize>,
    /// Los estados de antes, para Ctrl+Z: `(texto, cursor)`.
    atras: Vec<(String, usize)>,
    /// Lo deshecho, para Ctrl+Y.
    adelante: Vec<(String, usize)>,
    /// Si lo ultimo fue escribir letras: se juntan en un solo paso de
    /// deshacer hasta un blanco (como Word), no uno por letra.
    escribiendo_letras: bool,
}

/// Dos ediciones son iguales si dicen lo mismo y tienen el cursor y lo
/// elegido en el mismo sitio; su historia no cuenta.
impl PartialEq for EdicionTexto {
    fn eq(&self, o: &Self) -> bool {
        self.texto == o.texto && self.cursor == o.cursor && self.seleccion() == o.seleccion()
    }
}

impl Eq for EdicionTexto {}

/// Cuantos pasos de deshacer guarda un texto: de sobra para escribir, y
/// un texto pegado enorme no se queda cien veces en memoria.
const PASOS_DE_DESHACER: usize = 200;

impl EdicionTexto {
    /// Abre un texto que ya existia, con el cursor al final: es donde lo pone
    /// cualquier editor al que le pides editar algo.
    pub fn nueva(texto: impl Into<String>) -> Self {
        let texto = texto.into();
        let cursor = texto.chars().count();
        Self {
            texto,
            cursor,
            ..Default::default()
        }
    }

    pub fn vacia() -> Self {
        Self::default()
    }

    pub fn texto(&self) -> &str {
        &self.texto
    }

    /// El cursor, en caracteres desde el principio.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Cuantos caracteres hay. No son los bytes: `"ñ".len()` es 2 y esto da 1.
    pub fn largo(&self) -> usize {
        self.texto.chars().count()
    }

    pub fn esta_vacio(&self) -> bool {
        self.texto.is_empty()
    }

    /// Lo elegido, de menor a mayor, en caracteres. `None` si no hay nada.
    pub fn seleccion(&self) -> Option<(usize, usize)> {
        let a = self.ancla?;
        (a != self.cursor).then(|| (a.min(self.cursor), a.max(self.cursor)))
    }

    /// El texto elegido ("" si no hay).
    pub fn texto_elegido(&self) -> &str {
        match self.seleccion() {
            Some((a, b)) => &self.texto[self.byte_de(a)..self.byte_de(b)],
            None => "",
        }
    }

    /// Elige todo (Ctrl+A).
    pub fn elegir_todo(&mut self) -> bool {
        self.ancla = Some(0);
        self.cursor = self.largo();
        self.escribiendo_letras = false;
        !self.texto.is_empty()
    }

    /// El indice de BYTE donde empieza el caracter numero `caracteres`.
    ///
    /// Es el unico sitio del fichero que convierte de uno a otro, y sale
    /// siempre de `char_indices`, que solo da comienzos de caracter. Pasarse
    /// del final da el largo en bytes, que es la posicion valida de «detras
    /// de todo».
    fn byte_de(&self, caracteres: usize) -> usize {
        self.texto
            .char_indices()
            .nth(caracteres)
            .map_or(self.texto.len(), |(i, _)| i)
    }

    /// Guarda el estado de ahora para poder volver (y olvida lo rehecho).
    fn apuntar(&mut self) {
        if self.atras.last().is_some_and(|(t, _)| *t == self.texto) {
            return;
        }
        self.atras.push((self.texto.clone(), self.cursor));
        if self.atras.len() > PASOS_DE_DESHACER {
            self.atras.remove(0);
        }
        self.adelante.clear();
    }

    /// Quita lo elegido, si hay algo, y deja el cursor donde estaba.
    fn borrar_eleccion(&mut self) -> bool {
        let Some((a, b)) = self.seleccion() else {
            self.ancla = None;
            return false;
        };
        let (ba, bb) = (self.byte_de(a), self.byte_de(b));
        self.texto.replace_range(ba..bb, "");
        self.cursor = a;
        self.ancla = None;
        true
    }

    /// Mete un caracter donde esta el cursor y lo deja detras de el. Si
    /// habia algo elegido, lo sustituye.
    pub fn insertar(&mut self, c: char) {
        let letra = !c.is_whitespace();
        if !(letra && self.escribiendo_letras) || self.seleccion().is_some() {
            self.apuntar();
        }
        self.escribiendo_letras = letra;
        self.borrar_eleccion();
        let b = self.byte_de(self.cursor);
        self.texto.insert(b, c);
        self.cursor += 1;
    }

    /// Mete una cadena entera donde esta el cursor (pegar) y deja el cursor
    /// detras de ella. Devuelve si cambio algo.
    pub fn insertar_texto(&mut self, s: &str) -> bool {
        if s.is_empty() {
            return false;
        }
        self.apuntar();
        self.escribiendo_letras = false;
        self.borrar_eleccion();
        let b = self.byte_de(self.cursor);
        self.texto.insert_str(b, s);
        self.cursor += s.chars().count();
        true
    }

    /// **Envuelve lo elegido** entre `antes` y `despues` (Ctrl+B pone `**`,
    /// un color pone `\color{rojo}{` y `}`) y lo deja elegido, ya envuelto
    /// por dentro, para encadenar otro. Sin nada elegido, pone los dos y
    /// deja el cursor en medio para escribir. Devuelve si cambio algo.
    pub fn envolver(&mut self, antes: &str, despues: &str) -> bool {
        self.apuntar();
        self.escribiendo_letras = false;
        let (a, b) = self.seleccion().unwrap_or((self.cursor, self.cursor));
        let (ba, bb) = (self.byte_de(a), self.byte_de(b));
        self.texto.insert_str(bb, despues);
        self.texto.insert_str(ba, antes);
        let n = antes.chars().count();
        if a == b {
            self.ancla = None;
            self.cursor = a + n;
        } else {
            self.ancla = Some(a + n);
            self.cursor = b + n;
        }
        true
    }

    /// Ctrl+Z dentro del texto. Devuelve si habia algo que deshacer.
    pub fn deshacer(&mut self) -> bool {
        let Some((t, c)) = self.atras.pop() else {
            return false;
        };
        self.adelante
            .push((std::mem::replace(&mut self.texto, t), self.cursor));
        self.cursor = c.min(self.largo());
        self.ancla = None;
        self.escribiendo_letras = false;
        true
    }

    /// Ctrl+Y (o Ctrl+Mayus+Z) dentro del texto.
    pub fn rehacer(&mut self) -> bool {
        let Some((t, c)) = self.adelante.pop() else {
            return false;
        };
        self.atras
            .push((std::mem::replace(&mut self.texto, t), self.cursor));
        self.cursor = c.min(self.largo());
        self.ancla = None;
        self.escribiendo_letras = false;
        true
    }

    /// Una flecha, Inicio o Fin **con Mayus**: mueve el cursor y deja
    /// elegido lo recorrido. Las demas teclas van como sin Mayus.
    pub fn tecla_con_mayus(&mut self, t: TeclaTexto) -> bool {
        if !matches!(
            t,
            TeclaTexto::Izquierda | TeclaTexto::Derecha | TeclaTexto::Inicio | TeclaTexto::Fin
        ) {
            return self.tecla(t);
        }
        let ancla = self.ancla.unwrap_or(self.cursor);
        let movio = self.mover(t);
        self.ancla = Some(ancla);
        movio
    }

    fn mover(&mut self, t: TeclaTexto) -> bool {
        self.escribiendo_letras = false;
        match t {
            TeclaTexto::Izquierda => {
                if self.cursor == 0 {
                    return false;
                }
                self.cursor -= 1;
                true
            }
            TeclaTexto::Derecha => {
                if self.cursor >= self.largo() {
                    return false;
                }
                self.cursor += 1;
                true
            }
            TeclaTexto::Inicio => {
                let (inicio, _) = self.limites_del_renglon();
                let cambia = self.cursor != inicio;
                self.cursor = inicio;
                cambia
            }
            TeclaTexto::Fin => {
                let (_, fin) = self.limites_del_renglon();
                let cambia = self.cursor != fin;
                self.cursor = fin;
                cambia
            }
            _ => false,
        }
    }

    /// Aplica una tecla. Devuelve si cambio algo: con `false` no hay nada que
    /// repintar, y una tecla en el borde (Retroceso al principio) no puede
    /// hacer creer a la ventana que el texto se movio.
    pub fn tecla(&mut self, t: TeclaTexto) -> bool {
        match t {
            TeclaTexto::Entrar => {
                self.insertar('\n');
                true
            }
            TeclaTexto::Izquierda | TeclaTexto::Derecha => {
                // Con algo elegido, la flecha va al borde de lo elegido.
                if let Some((a, b)) = self.seleccion() {
                    self.cursor = if t == TeclaTexto::Izquierda { a } else { b };
                    self.ancla = None;
                    return true;
                }
                self.ancla = None;
                self.mover(t)
            }
            TeclaTexto::Inicio | TeclaTexto::Fin => {
                let habia = self.ancla.take().is_some_and(|a| a != self.cursor);
                self.mover(t) || habia
            }
            TeclaTexto::Retroceso => {
                if self.seleccion().is_some() {
                    self.apuntar();
                    self.escribiendo_letras = false;
                    return self.borrar_eleccion();
                }
                self.ancla = None;
                if self.cursor == 0 {
                    return false;
                }
                self.apuntar();
                self.escribiendo_letras = false;
                self.cursor -= 1;
                let b = self.byte_de(self.cursor);
                self.texto.remove(b);
                true
            }
            TeclaTexto::Suprimir => {
                if self.seleccion().is_some() {
                    self.apuntar();
                    self.escribiendo_letras = false;
                    return self.borrar_eleccion();
                }
                self.ancla = None;
                let b = self.byte_de(self.cursor);
                if b >= self.texto.len() {
                    return false;
                }
                self.apuntar();
                self.escribiendo_letras = false;
                self.texto.remove(b);
                true
            }
        }
    }

    /// En que renglon esta el cursor, contando desde cero.
    pub fn fila(&self) -> usize {
        self.fila_de(self.cursor)
    }

    /// En que renglon cae la posicion `pos` (en caracteres).
    pub fn fila_de(&self, pos: usize) -> usize {
        self.texto.chars().take(pos).filter(|c| *c == '\n').count()
    }

    /// Lo que va delante de `pos` en su renglon (para medir donde cae).
    pub fn prefijo_de(&self, pos: usize) -> &str {
        let inicio = self
            .texto
            .chars()
            .take(pos)
            .enumerate()
            .filter(|(_, c)| *c == '\n')
            .map(|(i, _)| i + 1)
            .last()
            .unwrap_or(0);
        &self.texto[self.byte_de(inicio)..self.byte_de(pos)]
    }

    /// Lo que va delante del cursor en SU renglon. Quien pinta lo mide con
    /// DirectWrite para saber a que altura de la linea va la barra; medirlo
    /// aqui seria inventarse el ancho de las letras.
    pub fn prefijo(&self) -> &str {
        let (inicio, _) = self.limites_del_renglon();
        &self.texto[self.byte_de(inicio)..self.byte_de(self.cursor)]
    }

    /// El primer y el ultimo caracter del renglon donde esta el cursor, en
    /// indices de caracter.
    ///
    /// Con el cursor justo delante de un salto de linea, el renglon es el de
    /// ARRIBA: esa posicion es el final de la linea que termina ahi, no el
    /// principio de la siguiente.
    fn limites_del_renglon(&self) -> (usize, usize) {
        let mut inicio = 0;
        let mut fin = self.largo();
        for (i, c) in self.texto.chars().enumerate() {
            if c != '\n' {
                continue;
            }
            if i < self.cursor {
                inicio = i + 1;
            } else {
                fin = i;
                break;
            }
        }
        (inicio, fin)
    }
}

/// **Lo que se pide con el teclado mientras se escribe**, mas alla de las
/// letras y las flechas: elegir, deshacer y dar formato al tramo elegido.
/// Lo traducen igual el lienzo (`dibujo/mano.rs`) y el anotador de capturas
/// (`captura2/anotar.rs`) con [`atajo_de_tecla`]: una sola tabla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtajoTexto {
    Deshacer,
    Rehacer,
    ElegirTodo,
    /// Mayus + una flecha, Inicio o Fin.
    Elegir(TeclaTexto),
    Negrita,
    Cursiva,
    Subrayado,
    Tachado,
    /// Ctrl+M: lo elegido como formula (`$...$`).
    Formula,
    /// Ctrl+Mayus+1..6: el tramo de un color; 0 es el del texto.
    Color(&'static str),
}

/// Los colores de Ctrl+Mayus+1..6, en el orden de las laminas (rojo para la
/// incognita, azul y verde para los datos).
pub const COLORES_DE_ATAJO: [&str; 6] = ["rojo", "azul", "verde", "naranja", "morado", "gris"];

/// **La tecla (codigo virtual de Windows) a su atajo de texto**. Los de
/// Word en ingles y en espanol: Ctrl+B o Ctrl+N negrita, Ctrl+I o Ctrl+K
/// cursiva, Ctrl+U subrayado (Ctrl+S es guardar), Ctrl+Mayus+X tachado.
pub fn atajo_de_tecla(vk: u32, ctrl: bool, shift: bool, alt: bool) -> Option<AtajoTexto> {
    if alt {
        return None;
    }
    if !ctrl {
        if !shift {
            return None;
        }
        return Some(AtajoTexto::Elegir(match vk {
            0x25 => TeclaTexto::Izquierda,
            0x27 => TeclaTexto::Derecha,
            0x24 => TeclaTexto::Inicio,
            0x23 => TeclaTexto::Fin,
            _ => return None,
        }));
    }
    let letra = char::from_u32(vk).filter(|c| c.is_ascii_alphanumeric());
    Some(match (letra, shift) {
        (Some('Z'), false) => AtajoTexto::Deshacer,
        (Some('Z'), true) | (Some('Y'), false) => AtajoTexto::Rehacer,
        (Some('A'), false) => AtajoTexto::ElegirTodo,
        (Some('B' | 'N'), false) => AtajoTexto::Negrita,
        (Some('I' | 'K'), false) => AtajoTexto::Cursiva,
        (Some('U'), false) => AtajoTexto::Subrayado,
        (Some('X'), true) => AtajoTexto::Tachado,
        (Some('M'), false) => AtajoTexto::Formula,
        (Some(d @ '1'..='6'), true) => {
            AtajoTexto::Color(COLORES_DE_ATAJO[d as usize - '1' as usize])
        }
        (Some('0'), true) => AtajoTexto::Color("negro"),
        _ => return None,
    })
}

impl EdicionTexto {
    /// Aplica un atajo de texto. Devuelve si cambio algo.
    pub fn atajo(&mut self, a: AtajoTexto) -> bool {
        match a {
            AtajoTexto::Deshacer => self.deshacer(),
            AtajoTexto::Rehacer => self.rehacer(),
            AtajoTexto::ElegirTodo => self.elegir_todo(),
            AtajoTexto::Elegir(t) => self.tecla_con_mayus(t),
            AtajoTexto::Negrita => self.envolver("**", "**"),
            AtajoTexto::Cursiva => self.envolver("*", "*"),
            AtajoTexto::Subrayado => self.envolver("<u>", "</u>"),
            AtajoTexto::Tachado => self.envolver("~~", "~~"),
            AtajoTexto::Formula => self.envolver("$", "$"),
            AtajoTexto::Color(c) => self.envolver(&format!("\\color{{{c}}}{{"), "}"),
        }
    }
}

/// Lo que ocupa un texto, a ojo: el motor no tiene con que medir letras.
///
/// Sirve para el hit-test (`impacto::toca` trata el texto como caja solida) y
/// para el `ancho_max` con el que quien pinta reparte las lineas. Se le suma
/// un caracter de holgura a proposito: deja sitio a la barra del cursor al
/// final del renglon, y evita que DirectWrite -que mide de verdad- parta una
/// linea que aqui se estimo un poco corta.
///
/// Un texto vacio mide un renglon, no cero: si midiera cero no se podria
/// pulsar encima para volver a editarlo.
pub fn medida_estimada(texto: &str, tam: f32) -> (f32, f32) {
    let renglones = texto.split('\n').count().max(1);
    let mas_largo = texto
        .split('\n')
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0);
    (
        (mas_largo + 1) as f32 * tam * ANCHO_POR_CARACTER,
        renglones as f32 * tam * INTERLINEADO,
    )
}

#[cfg(test)]
mod pruebas_de_estilo {
    use super::*;

    #[test]
    fn un_texto_sin_marcas_se_pinta_tal_cual() {
        let liso = EstiloDeTexto::default();
        assert!(liso.liso());
        assert_eq!(liso.sesgo(), 0.0);
        assert!(raya_del_tachado("hola", 0.0, 0.0, 20.0, "Excalifont", liso).is_none());
    }

    #[test]
    fn la_cursiva_inclina_y_la_negrita_no() {
        // Caso negativo: si la negrita tocara el sesgo, un titulo en negrita
        // saldria ademas torcido.
        let cursiva = EstiloDeTexto {
            cursiva: true,
            ..Default::default()
        };
        assert_eq!(cursiva.sesgo(), SESGO_DE_LA_CURSIVA);
        let negrita = EstiloDeTexto {
            negrita: true,
            ..Default::default()
        };
        assert_eq!(negrita.sesgo(), 0.0);
        assert!(!negrita.liso());
    }

    #[test]
    fn la_raya_del_tachado_cruza_la_palabra_por_la_mitad_de_la_equis() {
        let t = EstiloDeTexto {
            tachado: true,
            ..Default::default()
        };
        let ((x0, y0), (x1, y1), grosor) =
            raya_del_tachado("hola", 10.0, 100.0, 20.0, "Excalifont", t).expect("hay que tachar");
        assert_eq!(x0, 10.0, "la raya empieza donde el texto");
        assert!(x1 > x0, "la raya no tiene largo");
        assert_eq!(y0, y1, "la raya sale torcida");
        // Ni pegada al borde de arriba del renglon ni en la linea de base.
        assert!(y0 > 100.0 && y0 < 120.0, "la raya cae en {y0}");
        assert!(grosor >= 1.0, "una raya de menos de un pixel no se ve");
    }

    #[test]
    fn un_renglon_en_blanco_no_lleva_raya_suelta_en_el_aire() {
        // Caso negativo: una raya sin texto debajo se lee como un guion largo.
        let t = EstiloDeTexto {
            tachado: true,
            ..Default::default()
        };
        assert!(raya_del_tachado("", 0.0, 0.0, 20.0, "Excalifont", t).is_none());
        assert!(raya_del_tachado("   ", 0.0, 0.0, 20.0, "Excalifont", t).is_none());
        assert!(raya_del_tachado("hola", 0.0, 0.0, 0.0, "Excalifont", t).is_none());
    }

    #[test]
    fn los_numeros_de_fuente_son_los_del_fichero_y_los_alias_viejos_se_resuelven() {
        // Si esto se pierde, un dibujo guardado con la numeracion vieja se
        // reabre con otra letra.
        assert_eq!(familia_resuelta(None), FUENTE_EXCALIFONT);
        assert_eq!(familia_resuelta(Some(1)), FUENTE_EXCALIFONT, "Virgil");
        assert_eq!(familia_resuelta(Some(2)), FUENTE_NUNITO, "Helvetica");
        assert_eq!(familia_resuelta(Some(3)), FUENTE_COMIC_SHANNS, "Cascadia");
        assert_eq!(familia_resuelta(Some(5)), FUENTE_EXCALIFONT);
        assert_eq!(familia_resuelta(Some(6)), FUENTE_NUNITO);
        assert_eq!(familia_resuelta(Some(8)), FUENTE_COMIC_SHANNS);
        assert_eq!(familia_resuelta(Some(7)), FUENTE_LILITA_ONE);
        // Las del lienzo de citas se quedan con su numero.
        for id in [
            FUENTE_WORK_SANS,
            FUENTE_FRAUNCES,
            FUENTE_COURIER_NEW,
            FUENTE_CAVEAT,
        ] {
            assert_eq!(familia_resuelta(Some(id)), id);
        }
        // Caso negativo: un numero que no es de nadie cae en la de por
        // omision, no en un panico ni en un hueco. Tampoco el 4 (el hueco
        // que Excalidraw deja para letras de terceros) ni el 100 (su reserva
        // china).
        assert_eq!(familia_resuelta(Some(99)), FUENTE_EXCALIFONT);
        assert_eq!(familia_resuelta(Some(4)), FUENTE_EXCALIFONT);
        assert_eq!(familia_resuelta(Some(100)), FUENTE_EXCALIFONT);
    }

    #[test]
    fn una_fuente_de_windows_no_viaja_al_fichero_con_su_nombre() {
        // **Esto es lo que impide que el movil reabra el texto con otra
        // letra**: el fichero espera un numero, no «Segoe UI».
        assert_eq!(numero_de_familia("Segoe UI"), FUENTE_EXCALIFONT);
        assert_eq!(numero_de_familia("Nunito"), FUENTE_NUNITO);
        assert_eq!(numero_de_familia("comic shanns"), FUENTE_COMIC_SHANNS);
        assert_eq!(numero_de_familia("Comic Shanns 2"), FUENTE_COMIC_SHANNS);
        // Y la ida y vuelta es estable para las ocho del catalogo.
        for f in FUENTES {
            assert_eq!(numero_de_familia(nombre_de_familia(Some(f.id))), f.id);
        }
    }

    #[test]
    fn los_numeros_de_las_letras_de_citas_no_pisan_los_de_excalidraw() {
        // Los de Excalidraw (`FONT_FAMILY` y sus reservas): si uno de los
        // nuestros cayera ahi, excalidraw.com pintaria el texto con otra
        // letra que si conoce, en vez de con su reserva.
        let de_excalidraw = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 100];
        for f in &FUENTES[4..] {
            assert!(
                !de_excalidraw.contains(&f.id),
                "{} usa el {}",
                f.nombre,
                f.id
            );
        }
        // Y no hay dos con el mismo numero ni el mismo nombre.
        for (i, a) in FUENTES.iter().enumerate() {
            for b in &FUENTES[i + 1..] {
                assert_ne!(a.id, b.id);
                assert_ne!(a.nombre, b.nombre);
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn abrir_un_texto_que_ya_existia_pone_el_cursor_al_final() {
        let e = EdicionTexto::nueva("hola");
        assert_eq!(e.cursor(), 4);
        assert_eq!(e.prefijo(), "hola");
    }

    #[test]
    fn escribir_mete_la_letra_donde_esta_el_cursor_y_no_al_final() {
        let mut e = EdicionTexto::nueva("ac");
        e.tecla(TeclaTexto::Izquierda);
        e.insertar('b');
        assert_eq!(e.texto(), "abc");
        assert_eq!(e.cursor(), 2, "el cursor queda detras de lo escrito");
    }

    #[test]
    fn retroceso_borra_a_la_izquierda_y_suprimir_a_la_derecha() {
        let mut e = EdicionTexto::nueva("abc");
        e.tecla(TeclaTexto::Izquierda);
        assert!(e.tecla(TeclaTexto::Retroceso));
        assert_eq!(e.texto(), "ac");
        assert!(e.tecla(TeclaTexto::Suprimir));
        assert_eq!(e.texto(), "a");
    }

    #[test]
    fn el_cursor_no_se_sale_de_la_cadena_por_ninguno_de_los_dos_lados() {
        // Caso negativo: la tecla en el borde no mueve nada y lo dice.
        let mut e = EdicionTexto::nueva("ab");
        assert!(!e.tecla(TeclaTexto::Derecha), "ya estaba al final");
        assert_eq!(e.cursor(), 2);
        e.tecla(TeclaTexto::Inicio);
        assert!(!e.tecla(TeclaTexto::Izquierda), "ya estaba al principio");
        assert_eq!(e.cursor(), 0);
    }

    #[test]
    fn borrar_en_una_cadena_vacia_no_revienta_y_no_cambia_nada() {
        // Caso negativo: es la primera tecla que recibe un texto recien
        // abierto si el usuario se arrepiente.
        let mut e = EdicionTexto::vacia();
        assert!(!e.tecla(TeclaTexto::Retroceso));
        assert!(!e.tecla(TeclaTexto::Suprimir));
        assert!(e.esta_vacio());
        assert_eq!(e.cursor(), 0);
    }

    #[test]
    fn un_texto_con_acentos_no_se_parte_por_la_mitad() {
        // El fallo que este fichero existe para evitar: la `ñ` ocupa dos
        // bytes, y un cursor en bytes que retrocediera uno entraria en panico
        // al borrar. Aqui el cursor va en caracteres.
        let mut e = EdicionTexto::nueva("añoño");
        assert_eq!(e.largo(), 5, "cinco letras aunque sean ocho bytes");
        assert!(e.tecla(TeclaTexto::Retroceso));
        assert_eq!(e.texto(), "añoñ");
        e.tecla(TeclaTexto::Izquierda);
        e.insertar('í');
        assert_eq!(e.texto(), "añoíñ");
    }

    #[test]
    fn un_emoji_se_borra_entero_y_no_por_bytes() {
        let mut e = EdicionTexto::nueva("a🐟b");
        e.tecla(TeclaTexto::Izquierda);
        assert!(e.tecla(TeclaTexto::Retroceso));
        assert_eq!(e.texto(), "ab", "el pez se va de una vez, no a cuartos");
    }

    #[test]
    fn el_cursor_recorre_un_emoji_de_un_solo_paso() {
        let mut e = EdicionTexto::nueva("🐟");
        assert_eq!(e.largo(), 1);
        e.tecla(TeclaTexto::Izquierda);
        assert_eq!(e.cursor(), 0);
        e.insertar('x');
        assert_eq!(e.texto(), "x🐟");
    }

    #[test]
    fn entrar_hace_renglon_y_el_cursor_baja_de_fila() {
        let mut e = EdicionTexto::nueva("uno");
        assert_eq!(e.fila(), 0);
        e.tecla(TeclaTexto::Entrar);
        e.insertar('d');
        assert_eq!(e.texto(), "uno\nd");
        assert_eq!(e.fila(), 1);
        assert_eq!(e.prefijo(), "d", "el prefijo es solo lo de su renglon");
    }

    #[test]
    fn inicio_y_fin_van_a_los_extremos_del_renglon_y_no_del_texto() {
        let mut e = EdicionTexto::nueva("uno\ndos");
        e.tecla(TeclaTexto::Inicio);
        assert_eq!(e.cursor(), 4, "al principio de «dos», no del todo");
        assert_eq!(e.fila(), 1);
        assert_eq!(e.prefijo(), "");
        e.tecla(TeclaTexto::Fin);
        assert_eq!(e.cursor(), 7);
        assert_eq!(e.prefijo(), "dos");
    }

    #[test]
    fn el_cursor_delante_de_un_salto_pertenece_al_renglon_de_arriba() {
        // Si perteneciera al de abajo, Inicio saltaria una linea entera y la
        // barra se pintaria un renglon mas abajo de donde se escribe.
        let mut e = EdicionTexto::nueva("uno\ndos");
        for _ in 0..4 {
            e.tecla(TeclaTexto::Izquierda);
        }
        assert_eq!(e.cursor(), 3);
        assert_eq!(e.fila(), 0);
        assert_eq!(e.prefijo(), "uno");
        e.tecla(TeclaTexto::Inicio);
        assert_eq!(e.cursor(), 0);
    }

    #[test]
    fn retroceso_sobre_un_salto_junta_los_dos_renglones() {
        let mut e = EdicionTexto::nueva("uno\ndos");
        e.tecla(TeclaTexto::Inicio);
        assert!(e.tecla(TeclaTexto::Retroceso));
        assert_eq!(e.texto(), "unodos");
        assert_eq!(e.fila(), 0);
    }

    #[test]
    fn la_medida_estimada_crece_con_el_renglon_mas_largo_y_con_cuantos_hay() {
        let (a1, h1) = medida_estimada("ab", 10.0);
        let (a2, h2) = medida_estimada("ab\nabcd", 10.0);
        assert!(a2 > a1, "manda el renglon mas largo");
        assert!(h2 > h1, "dos renglones son mas altos que uno");
        assert!((h1 - 10.0 * INTERLINEADO).abs() < 1e-3);
    }

    #[test]
    fn un_texto_vacio_mide_un_renglon_y_nunca_cero() {
        // Caso negativo: con medida cero no se podria volver a pulsar encima
        // para editarlo, ni se veria la barra del cursor.
        let (ancho, alto) = medida_estimada("", TAM_POR_DEFECTO);
        assert!(ancho > 0.0, "ancho {ancho}");
        assert!(alto > 0.0, "alto {alto}");
    }

    #[test]
    fn las_palabras_de_la_alineacion_son_las_del_fichero_y_vuelven() {
        for a in [
            AlineacionTexto::Izquierda,
            AlineacionTexto::Centro,
            AlineacionTexto::Derecha,
        ] {
            assert_eq!(AlineacionTexto::desde_palabra(a.palabra()), Some(a));
        }
        for v in [
            AlineacionVertical::Arriba,
            AlineacionVertical::Medio,
            AlineacionVertical::Abajo,
        ] {
            assert_eq!(AlineacionVertical::desde_palabra(v.palabra()), Some(v));
        }
        assert_eq!(AlineacionTexto::Centro.palabra(), "center");
        assert_eq!(AlineacionVertical::Medio.palabra(), "middle");
        // Caso negativo: una palabra de otro sitio no se adivina.
        assert_eq!(AlineacionTexto::desde_palabra("justify"), None);
        assert_eq!(AlineacionVertical::desde_palabra("center"), None);
    }

    #[test]
    fn centrado_el_renglon_corto_se_aparta_la_mitad_de_lo_que_le_sobra() {
        // Cada caracter mide 6,2 con letra 10 (a ojo, sin medidor).
        let apartados = |t: &str, caja: f32, a: AlineacionTexto| {
            apartados_de_renglones(t, 10.0, "Excalifont", EstiloDeTexto::default(), caja, a)
        };
        let a = apartados("abcd\nab", 24.8, AlineacionTexto::Centro);
        assert!(a[0].abs() < 1e-3, "el largo llena la caja: {a:?}");
        assert!((a[1] - 6.2).abs() < 1e-3, "{a:?}");
        let d = apartados("abcd\nab", 24.8, AlineacionTexto::Derecha);
        assert!((d[1] - 12.4).abs() < 1e-3, "{d:?}");
        // Caso negativo: a la izquierda nadie se aparta, y un renglon mas
        // ancho que su caja no se sale por la izquierda.
        let i = apartados("abcd\nab", 24.8, AlineacionTexto::Izquierda);
        assert_eq!(i, vec![0.0, 0.0]);
        let ancho = apartados("abcdefgh", 10.0, AlineacionTexto::Derecha);
        assert_eq!(ancho, vec![0.0]);
    }

    /// Un medidor de mentira: 10 por letra (el espacio tambien) y un alto
    /// de sistema de 1,5 por renglon, para que se note si se usa.
    fn medidor_de_prueba(
        t: &str,
        tam: f32,
        _familia: &str,
        estilo: EstiloDeTexto,
    ) -> Option<(f32, f32)> {
        let largo = t.split('\n').map(|r| r.chars().count()).max().unwrap_or(0) as f32;
        let gordo = if estilo.negrita { 2.0 } else { 1.0 };
        Some((
            largo * 10.0 * gordo,
            t.split('\n').count() as f32 * tam * 1.5,
        ))
    }

    #[test]
    fn con_medidor_la_caja_mide_lo_escrito_y_no_un_caracter_de_mas() {
        con_medidor(medidor_de_prueba, || {
            let (ancho, alto) = medida("hola", 20.0, "Excalifont", EstiloDeTexto::default());
            assert_eq!(ancho, 40.0, "ni la holgura del cursor ni el 0,62 a ojo");
            assert_eq!(alto, 25.0, "un renglon de Excalifont: 20 x 1,25");
            // Dos renglones: manda el largo, y el alto es el interlineado fijo.
            let (a2, h2) = medida("hola\nhol", 20.0, "Excalifont", EstiloDeTexto::default());
            assert_eq!((a2, h2), (40.0, 50.0));
            // La negrita pasa hasta el medidor.
            let negrita = EstiloDeTexto {
                negrita: true,
                ..Default::default()
            };
            assert_eq!(medida("hola", 20.0, "Excalifont", negrita).0, 80.0);
        });
    }

    #[test]
    fn un_renglon_vacio_mide_un_espacio_y_el_cursor_empieza_en_cero() {
        con_medidor(medidor_de_prueba, || {
            let (ancho, alto) = medida("", 20.0, "Nunito", EstiloDeTexto::default());
            assert_eq!(ancho, 10.0, "un espacio");
            assert!(alto > 0.0);
            // Caso negativo: el ancho de NADA delante del cursor es cero, no
            // un espacio; si no la barra saldria despegada del borde.
            assert_eq!(
                ancho_real("", 20.0, "Nunito", EstiloDeTexto::default()),
                0.0
            );
        });
    }

    #[test]
    fn una_letra_del_sistema_toma_el_alto_de_directwrite_y_no_el_fijo() {
        con_medidor(medidor_de_prueba, || {
            let (_, alto) = medida("a\nb", 20.0, "Segoe UI", EstiloDeTexto::default());
            assert_eq!(alto, 60.0, "el 1,5 del medidor, no el 1,25");
        });
        // Y Lilita One lleva el suyo, el 1,15 de Excalidraw.
        let (_, lilita) = medida("a", 20.0, "Lilita One", EstiloDeTexto::default());
        assert!((lilita - 23.0).abs() < 1e-3, "{lilita}");
    }

    #[test]
    fn sin_medidor_se_mide_a_ojo_como_siempre() {
        // Caso negativo: el motor puro no tiene DirectWrite y no puede
        // quedarse sin caja.
        assert!(
            !hay_medidor(),
            "una prueba dejo un medidor puesto en este hilo"
        );
        let (ancho, _) = medida("hola", 10.0, "Excalifont", EstiloDeTexto::default());
        assert_eq!(ancho, medida_estimada("hola", 10.0).0);
    }
}
