//! Que es un elemento del dibujo y como se toca.
//!
//! Un elemento es su geometria mas su estilo mas **su semilla**. La semilla es
//! lo que hace que el dibujo sea el mismo cada vez que se abre (ver `azar`), y
//! por eso viaja en el fichero como cualquier otro dato.
//!
//! `version` sube en cada cambio. No es informacion para el usuario: es lo que
//! le dice al dibujante que su geometria cacheada ya no vale. Sin ella habria
//! que comparar el elemento entero en cada fotograma.

use std::mem::size_of;

use serde::{Deserialize, Serialize};

use crate::relleno::EstiloRelleno;
use crate::vector::Punto2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EstiloTrazo {
    Solido,
    Discontinuo,
    Punteado,
    /// **Raya y punto**, el de los ejes de simetria y las lineas de centro de
    /// los planos. Viaja al movil como `dashed` con `trazoRayaPunto` (ver
    /// `geometria_exacta`), y se pinta partido en tramos lisos.
    #[serde(rename = "rayapunto")]
    RayaPunto,
}

/// Color RGBA, cada canal en `[0,1]`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ColorRgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl ColorRgba {
    pub const fn opaco(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }
}

/// La geometria propia de cada tipo. Lo que no depende del tipo (posicion,
/// color, grosor) vive en `Elemento`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "lowercase")]
pub enum Figura {
    /// Trazo a mano alzada. Las presiones son opcionales: sin tableta se
    /// simulan a partir de la velocidad.
    Lapiz {
        puntos: Vec<Punto2>,
        #[serde(default)]
        presiones: Vec<f32>,
        /// La pluma de Excalidraw (`strokeOptions`). `None` = trazo de antes
        /// de E1, cuyo grosor eran pixeles: ver `tinta::contorno_de_lapiz`.
        #[serde(default)]
        opciones: Option<crate::tinta::OpcionesTinta>,
    },
    /// Como el lapiz pero grueso, translucido y sin rugosidad: resaltar sobre
    /// texto tiene que dejarlo legible (D45).
    Resaltador {
        puntos: Vec<Punto2>,
    },
    Linea {
        puntos: Vec<Punto2>,
    },
    /// Una flecha. **Sus puntas son un tipo y no un `bool`**: las ocho de
    /// Excalidraw viajan en `startArrowhead`/`endArrowhead`, y con «lleva
    /// punta o no» un diagrama entidad-relacion del movil —donde la punta DICE
    /// la cardinalidad— se abria con ocho flechas iguales y dejaba de decir
    /// nada. Ver `formas::TipoPunta`.
    ///
    /// `codos` es el `elbowed` del movil: el conector ortogonal del
    /// organigrama, que no traza la recta entre los dos puntos sino el camino
    /// en angulo recto de `codo::trazado_de_flecha`.
    Flecha {
        puntos: Vec<Punto2>,
        #[serde(default)]
        punta_inicio: crate::formas::TipoPunta,
        #[serde(default = "punta_de_flecha")]
        punta_fin: crate::formas::TipoPunta,
        #[serde(default)]
        codos: bool,
    },
    /// **Un mosaico de censura** (`pixpin-mosaic` del movil): tapa su caja
    /// para que no se lea lo que hay debajo.
    ///
    /// Hasta ahora entraba como elemento ajeno: se conservaba al guardar
    /// pero **no se pintaba**, asi que en una captura anotada en el telefono
    /// —un numero de cuenta, una cara— el PC ENSENABA lo que el usuario
    /// habia tapado. No era una figura que faltaba: era un dato que se
    /// escapaba.
    ///
    /// `desenfoque` es el `mosaicBlur` del movil: con el puesto tapa con
    /// mancha y sin el, con bloques.
    Mosaico {
        #[serde(default)]
        desenfoque: bool,
    },
    Rectangulo,
    /// El rombo de Excalidraw (`diamond`): una de sus diez figuras
    /// principales, y aqui era un elemento ajeno e invisible. Su geometria
    /// sale de la caja, como la del rectangulo; lo unico distinto es por
    /// donde pasan los lados.
    Rombo,
    Elipse,
    /// **El foco del movil** (`pixpin-spotlight`, `Tool.SPOTLIGHT`): dos
    /// figuras, como alli. El **marco** es la caja del elemento —se estira con
    /// sus tiradores— y el **hueco** es la figura que se toco con la varita
    /// (`cristal`: su forma, donde esta y cuanto mide, lo mismo que la lupa).
    /// Se oscurece **solo el anillo entre los dos**, no el lienzo entero: asi
    /// se resalta algo sin esconder lo demas, y caben varios focos sin que se
    /// sumen sus sombras (`drawSpotlights`).
    Foco {
        #[serde(default)]
        cristal: crate::lupa_elemento::Cristal,
    },
    /// **La lupa** (`pixpin-lupa`, `Tool.LUPA` del movil): un trozo del
    /// dibujo ensenado en grande en otro sitio. La caja del elemento es el
    /// cristal; `cristal` dice a donde mira, cuanto agranda, de que forma es
    /// y con que guia se senala (`lupa_elemento`). Lo de dentro no es una
    /// orden: lo vuelve a pintar quien tiene la escena, agrandado.
    Lupa {
        #[serde(default)]
        cristal: crate::lupa_elemento::Cristal,
    },
    Texto {
        texto: String,
        tam: f32,
        #[serde(default = "familia_por_defecto")]
        familia: String,
    },
    /// El bitmap no vive aqui: lo aporta quien dibuja (el pin tiene el suyo,
    /// el PDF el suyo). Aqui solo va la referencia.
    Imagen {
        id_objeto: u64,
    },
    /// Una cota: el segmento acotado y nada mas.
    ///
    /// **No guarda su texto** (D34): ni el numero, ni la unidad, ni el
    /// angulo. El rotulo se deriva al pintar, cada vez. Una cota que
    /// guardara su texto podria decir una cosa y medir otra, y en un plano
    /// eso es peor que no tener cotas.
    ///
    /// Es figura propia y no una `Linea` con una bandera (D38): el movil usa
    /// el tipo `pixpin-measure`, y con una bandera se romperia la
    /// compatibilidad que esta tarea viene a asegurar.
    Cota {
        puntos: Vec<Punto2>,
    },
    /// La reglita a cuadros. Usa `x`/`y`/`ancho`/`alto` como el rectangulo;
    /// cuantos cuadros pone y cuanto mide cada uno sale de la escala de la
    /// escena al pintar.
    ///
    /// Es un **elemento del dibujo y no un adorno del editor** (D37): se
    /// guarda, se mueve, se estira y sale en la exportacion. Es lo que
    /// permite medir sobre la imagen que recibe otro.
    EscalaGrafica,
    /// Un marco: un recuadro con nombre que agrupa POR CONTENCION lo que
    /// cae dentro (`frame` de Excalidraw). Moverlo mueve lo de dentro, que
    /// es para lo que sirve: preparar laminas y moverlas de una pieza.
    ///
    /// Quien esta dentro no se guarda en ninguna lista: se mira la caja cada
    /// vez. Una lista seria una segunda verdad sobre lo mismo, y habria que
    /// mantenerla al mover, al borrar y al deshacer.
    Marco {
        #[serde(default)]
        nombre: String,
    },
    /// Un emoji suelto, del tamano de su caja (universo, §2.2). Se pinta
    /// con la fuente de color de Windows; no hay imagenes empaquetadas.
    Emoji {
        caracter: String,
    },
    /// **Un trozo de ovalo** (`pixpin-arc` del movil): se pone un ovalo guia
    /// y se repasa con el lapiz solo el tramo que interesa.
    ///
    /// No guarda puntos sino el tramo del ovalo, que es lo que permite
    /// estirar la caja despues y que el arco se reajuste en vez de
    /// deformarse. `inicio` y `barrido` van en radianes, medidos sobre el
    /// ovalo de la caja (`arcStart`/`arcSweep` del movil, que alli van en
    /// grados: la traduccion esta en `excalidraw.rs`).
    ///
    /// **`barrido: None` es un estado de verdad**, no un dato que falta:
    /// significa «todavia es solo la guia», el ovalo puesto y aun sin
    /// repasar. Machacarlo con un cero convertiria una guia en un arco de
    /// longitud nula, que no es lo mismo ni se ve igual.
    Arco {
        #[serde(default)]
        inicio: f32,
        #[serde(default)]
        barrido: Option<f32>,
    },
    /// **Un numero de serie** (`pixpin-serial`): el circulito con un 1, un 2,
    /// un 3 con el que se anota una captura paso a paso. El numero va en el
    /// `text` del movil; aqui es un numero y no una cadena porque el movil
    /// calcula el siguiente sumando uno al mayor que haya en la escena.
    Serie {
        #[serde(default)]
        numero: u32,
    },
    /// **Lo que pinto el bote de relleno** (`pixpin-region`): el contorno que
    /// se ENCONTRO entre las figuras que cerraban el hueco, con sus agujeros.
    ///
    /// Guarda lo encontrado y no una referencia a las figuras que lo
    /// encerraban, igual que el movil y a proposito: lo que se relleno se
    /// queda relleno aunque despues se mueva una de las paredes.
    ///
    /// Los puntos son ABSOLUTOS, como en el resto de figuras de aqui, y los
    /// huecos tambien. `huecos` se pinta por la regla par/impar: un anillo es
    /// su contorno menos su agujero.
    Region {
        #[serde(default)]
        contorno: Vec<Punto2>,
        #[serde(default)]
        huecos: Vec<Vec<Punto2>>,
    },
    /// **Un punto con su letra** (`pixpin-point`): la A, la B y la C de un
    /// croquis de geometria.
    ///
    /// Su caja no tiene tamano: `x` e `y` SON el punto. La letra orbita a su
    /// alrededor en polares —`angulo` en radianes y `radio` en pixeles del
    /// documento— para que al mover el punto la letra lo siga sin pisar el
    /// dibujo (`etiquetaAngulo`/`etiquetaRadio` del movil).
    Punto {
        #[serde(default)]
        letra: String,
        #[serde(default)]
        angulo: f32,
        #[serde(default = "radio_de_etiqueta")]
        radio: f32,
    },
    /// **El instrumento de graficar** (`pixpin-number-line`, `pixpin-axes`,
    /// `pixpin-space` del movil): uno solo, con el numero de ejes como dato
    /// (1 recta numerica, 2 plano cartesiano, 3 espacio). Ver `instrumento`.
    Instrumento(crate::instrumento::Instrumento),
    // El cronograma (`pixpin-gantt` del movil) se quito: no estaba bien
    // hecho y no servia. Uno que llegue del movil viaja como ajeno (ver
    // `excalidraw.rs`): no se pinta aqui, pero vuelve intacto al guardar.
}

/// Lo que el movil pone de fabrica cuando planta un punto etiquetado.
fn radio_de_etiqueta() -> f32 {
    14.0
}

/// Como se posa el extremo de una flecha sobre la figura a la que se ata
/// (`BindMode` del movil).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModoEnganche {
    /// Se queda FUERA, sobre el contorno, orbitandolo.
    #[default]
    Orbita,
    /// Se queda DENTRO, en el punto exacto donde se solto.
    Dentro,
}

/// El anclaje de un extremo de flecha a una figura (`Binding` del movil).
///
/// **Guarda un punto y no solo la figura.** Con el id a secas la flecha se
/// ataba «a la caja» y acababa siempre proyectada al borde mas cercano, con
/// lo que daba igual donde se hubiera soltado la punta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Enganche {
    /// El id de TEXTO de la figura, el del fichero (`elementId`). Es texto y
    /// no un numero por lo mismo que `grupos`: lo genera el movil y aqui
    /// viaja de ida y vuelta sin tocarlo.
    pub elemento: String,
    /// Cuanto se desvia el punto de contacto del centro, de -1 a 1.
    #[serde(default)]
    pub foco: f32,
    /// Separacion entre la punta y el borde, en pixeles del documento.
    #[serde(default = "uno")]
    pub hueco: f32,
    /// El punto agarrado, en proporcion de la caja: `(0,0)` es su esquina
    /// superior izquierda y `(1,1)` la inferior derecha.
    #[serde(default)]
    pub punto_fijo: Option<(f32, f32)>,
    #[serde(default)]
    pub modo: ModoEnganche,
}

/// Una referencia a un elemento atado a este: el texto de dentro de una
/// figura, o la flecha que le apunta (`BoundElement` del movil).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Atado {
    pub id: String,
    /// El tipo tal cual viene del fichero (`arrow`, `text`...). Se guarda
    /// como texto y no como enumerado porque aqui no se usa para decidir
    /// nada: se conserva para devolverlo intacto.
    pub tipo: String,
}

/// El tamano de una hoja, POR SU PROPORCION (`TamanoDePapel` del movil).
///
/// Una hoja no se guarda en centimetros porque el lienzo no tiene
/// centimetros: lo que el tamano aporta es la proporcion —que un A4 sea un A4
/// y no un recuadro cualquiera—.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TamanoPapel {
    A4,
    A5,
    Carta,
    Cuadrada,
    Apaisada,
}

impl TamanoPapel {
    /// Alto partido por ancho, con los mismos numeros del movil.
    pub fn proporcion(self) -> f32 {
        match self {
            TamanoPapel::A4 => 297.0 / 210.0,
            TamanoPapel::A5 => 210.0 / 148.0,
            TamanoPapel::Carta => 11.0 / 8.5,
            TamanoPapel::Cuadrada => 1.0,
            TamanoPapel::Apaisada => 9.0 / 16.0,
        }
    }

    pub fn palabra(self) -> &'static str {
        match self {
            TamanoPapel::A4 => "a4",
            TamanoPapel::A5 => "a5",
            TamanoPapel::Carta => "carta",
            TamanoPapel::Cuadrada => "cuadrada",
            TamanoPapel::Apaisada => "apaisada",
        }
    }

    pub fn desde_palabra(p: &str) -> Option<TamanoPapel> {
        Some(match p {
            "a4" => TamanoPapel::A4,
            "a5" => TamanoPapel::A5,
            "carta" => TamanoPapel::Carta,
            "cuadrada" => TamanoPapel::Cuadrada,
            "apaisada" => TamanoPapel::Apaisada,
            _ => return None,
        })
    }
}

/// La pauta impresa de una hoja: lo que trae el papel antes de escribir
/// (`PautaDeHoja` del movil).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PautaHoja {
    #[default]
    Lisa,
    Rayada,
    Cuadros,
    Puntos,
}

impl PautaHoja {
    pub fn palabra(self) -> &'static str {
        match self {
            PautaHoja::Lisa => "lisa",
            PautaHoja::Rayada => "rayada",
            PautaHoja::Cuadros => "cuadros",
            PautaHoja::Puntos => "puntos",
        }
    }

    pub fn desde_palabra(p: &str) -> Option<PautaHoja> {
        Some(match p {
            "lisa" => PautaHoja::Lisa,
            "rayada" => PautaHoja::Rayada,
            "cuadros" => PautaHoja::Cuadros,
            "puntos" => PautaHoja::Puntos,
            _ => return None,
        })
    }
}

/// **Los diez campos del elemento del movil que el PC ya modela pero que
/// todavia no manda del todo**, juntos en un sitio.
///
/// Van agrupados y no sueltos en `Elemento` por una razon muy concreta y
/// medida: en este proyecto hay ocho sitios que construyen un `Elemento`
/// entero a mano, y varios son de otros duenos (`pixpin-ui`,
/// `pixpin-universo`, el universo de la app). **Cada campo suelto que nace en
/// `Elemento` obliga a tocar los ocho**, y esta tanda anade diez de golpe:
/// ochenta lineas en ficheros ajenos, y los mismos ochenta otra vez cuando el
/// grupo que venga anada el suyo. Con un campo solo, el precio se paga una
/// vez y los grupos A, B y D amplian esto sin salir de aqui.
///
/// Lo que NO es: un cajon de sastre para JSON desconocido. Eso ya existe y
/// esta un nivel mas arriba (`excalidraw::Entrada::Nuestro`, que guarda el
/// original). Aqui solo entra lo que el PC entiende con nombre y tipo.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Extras {
    /// Trazo de ancho constante, para escribir a mano (`presionFirme`). Sin
    /// el, lo escrito adelgaza en las curvas y la letra se rompe.
    #[serde(default)]
    pub presion_firme: bool,
    /// **Es una linea de referencia, no dibujo** (`reference` del movil): el
    /// «lapiz azul» de los planos. Se pinta translucida (`pintado::ordenes`).
    /// Lo que se traza con el modo guia puesto nace asi (`Gesto::modo_guia`),
    /// y fuera de el no se elige ni se borra (`impacto::Mundo`).
    #[serde(default)]
    pub referencia: bool,
    #[serde(default)]
    pub negrita: bool,
    #[serde(default)]
    pub cursiva: bool,
    #[serde(default)]
    pub tachado: bool,
    /// La figura que contiene a este texto (`containerId`). Es lo que hace
    /// que una caja lleve su rotulo dentro y lo arrastre consigo.
    #[serde(default)]
    pub contenedor: Option<String>,
    /// A que figura se ata el primer extremo de esta flecha (`startBinding`).
    #[serde(default)]
    pub enganche_inicio: Option<Enganche>,
    #[serde(default)]
    pub enganche_fin: Option<Enganche>,
    /// Quienes estan atados a ESTE elemento (`boundElements`): su rotulo de
    /// dentro, las flechas que le apuntan.
    #[serde(default)]
    pub atados: Vec<Atado>,
    /// De que tamano de papel es esta hoja, si es de alguno (`papel`).
    #[serde(default)]
    pub papel: Option<TamanoPapel>,
    /// La pauta impresa de la hoja (`pauta`).
    #[serde(default)]
    pub pauta: PautaHoja,
    /// **El id de TEXTO con el que este elemento entro o salio del fichero**
    /// (`id` de Excalidraw).
    ///
    /// Aqui el id es un `u64` que se deriva del texto, y esa cuenta no se
    /// puede deshacer. Mientras solo se leyeran dibujos del movil daba igual,
    /// porque su id venia en el fichero; pero un enganche nacido AQUI, entre
    /// dos elementos que creo el PC, apuntaba a un texto inventado para la
    /// sesion, y al guardar el elemento estrenaba otro con la hora dentro
    /// (`w<hora><id>`). Al reabrir, el enganche apuntaba a un id que ya no
    /// existia y la flecha dejaba de seguir a su caja.
    ///
    /// Recordandolo, el id se escribe una vez y no cambia nunca mas: el
    /// enganche que se hizo hoy sigue atado manana.
    #[serde(default)]
    pub id_de_fichero: Option<String>,
    /// **El trozo de la imagen que se ensena** (`crop`), solo en imagenes.
    ///
    /// Se lee para PINTARLO, no para editarlo: ninguna herramienta de aqui ni
    /// del movil lo crea —llega de un `.excalidraw` de la web—, pero el movil
    /// lo pinta (`Renderer.kt`) y lo exporta (`DrawSvg.recortar`). Mientras
    /// vivia solo en el JSON original, la pantalla y todo lo exportado
    /// ensenaban la foto entera, con lo que el recorte habia quitado.
    #[serde(default)]
    pub recorte: Option<RecorteImagen>,
    /// **A que lado van los renglones** (`textAlign`), solo en textos.
    ///
    /// `None` es «el fichero no lo dice», y no es lo mismo que izquierda
    /// aunque se pinte igual: asi un texto que nunca lo trajo no estrena la
    /// clave al moverlo, y uno con una palabra que aqui no se conoce la
    /// conserva intacta hasta que alguien elija otra.
    #[serde(default)]
    pub alineacion: Option<crate::texto::AlineacionTexto>,
    /// **A que altura va el rotulo dentro de su figura** (`verticalAlign`).
    /// `None` por lo mismo que `alineacion`.
    #[serde(default)]
    pub alineacion_vertical: Option<crate::texto::AlineacionVertical>,
    /// **La letra de una figura con rotulos que no es un texto** (de
    /// la cota, el punto...: `fontFamily` del movil, que los pinta con ella).
    /// Sin ella sus nombres salian en la letra del sistema, distinta de la
    /// de los textos de alrededor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub familia: Option<String>,
    /// Y su tamano de letra (`fontSize`): el del pincel al crearla. Sin el,
    /// la letra salia de la mitad del alto de la fila y en una fila alta los
    /// nombres no cabian («Cimien…»).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tam_letra: Option<f32>,
    /// **La dureza de la mina**, solo en lo de grafito (`dureza` del movil,
    /// v0.91). `None` es HB, como el movil lee la clave ausente: asi un trazo
    /// de fabrica no la estrena. Ver `tinta::grafito::Dureza`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dureza: Option<crate::tinta::grafito::Dureza>,
    /// **La fuente rica de un texto** (`textoRico`, 15-oct-2026): tramos de
    /// color, negrita, formulas en linea `$...$`... (`texto_rico`). `None`: un
    /// texto de los de siempre. Cuando la lleva, `Figura::Texto::texto` es su
    /// version plana (`texto_rico::plano`), la que lee quien no la entiende.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rico: Option<String>,
    /// **Como se cierra un arco** (`cierreArco`): sector o segmento. Sin la
    /// clave, abierto. Ver `geometria_exacta`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cierre: Option<crate::geometria_exacta::CierreArco>,
    /// **El arco es la marca de un angulo** (`marcaAngulo`): sus arquitos,
    /// si es recto y su valor. Ver `geometria_exacta`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marca_angulo: Option<crate::geometria_exacta::MarcaAngulo>,
    /// **El relleno degradado** (`degradado`), lineal o radial. Ver
    /// `geometria_exacta`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degradado: Option<crate::geometria_exacta::Degradado>,
}

/// **El recorte de una imagen** (`crop` de Excalidraw).
///
/// Las cuatro primeras medidas van en pixeles del ORIGINAL, y por eso hacen
/// falta las dos ultimas: sin saber contra que tamano se midio, un recorte
/// no se puede llevar a una copia de otro tamano (el bitmap reducido que se
/// sube a la GPU, por ejemplo).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RecorteImagen {
    pub x: f32,
    pub y: f32,
    pub ancho: f32,
    pub alto: f32,
    pub ancho_natural: f32,
    pub alto_natural: f32,
}

impl RecorteImagen {
    /// El trozo `(x0, y0, x1, y1)` en pixeles de una copia de la imagen de
    /// `ancho` x `alto`.
    ///
    /// `None` cuando el recorte no dice nada util —area cero, tamano natural
    /// cero, menos de un pixel— y entonces se pinta la imagen entera: un
    /// recorte roto no puede hacer desaparecer la foto. Lo que se salga del
    /// original se recorta a lo que existe, porque pedir pixeles que no estan
    /// pinta basura en el borde.
    pub fn trozo_en(&self, ancho: f32, alto: f32) -> Option<(f32, f32, f32, f32)> {
        let finitos = [self.x, self.y, self.ancho, self.alto, ancho, alto]
            .iter()
            .all(|v| v.is_finite());
        if !finitos
            || self.ancho <= 0.0
            || self.alto <= 0.0
            || self.ancho_natural.is_nan()
            || self.alto_natural.is_nan()
            || self.ancho_natural <= 0.0
            || self.alto_natural <= 0.0
            || ancho <= 0.0
            || alto <= 0.0
        {
            return None;
        }
        let (kx, ky) = (ancho / self.ancho_natural, alto / self.alto_natural);
        let x0 = (self.x * kx).clamp(0.0, ancho);
        let y0 = (self.y * ky).clamp(0.0, alto);
        let x1 = ((self.x + self.ancho) * kx).clamp(0.0, ancho);
        let y1 = ((self.y + self.alto) * ky).clamp(0.0, alto);
        (x1 - x0 >= 1.0 && y1 - y0 >= 1.0).then_some((x0, y0, x1, y1))
    }
}

impl Extras {
    /// Si no hay nada que escribir. Lo usa el puente para no ensuciar el
    /// JSON de un elemento que nunca tuvo ninguno de estos campos.
    ///
    /// **`id_de_fichero` no cuenta**: no es un campo del movil, es la memoria
    /// de con que id entro este elemento, y se escribe por la clave `id` como
    /// siempre. Contandolo, todo elemento leido de un fichero tendria extras
    /// «no vacios» y estrenaria las diez claves del movil al moverlo, que es
    /// justo el ensuciado que esta funcion viene a evitar.
    pub fn vacios(&self) -> bool {
        Extras {
            // La referencia vuelve con su original (`reference`), no es de
            // las diez claves.
            referencia: false,
            id_de_fichero: None,
            // Tampoco el recorte: no es de las diez claves, vuelve por su
            // propia clave `crop`, y contarlo ensuciaria igual.
            recorte: None,
            // Ni la alineacion: son claves de Excalidraw que se escriben en la
            // rama del texto (`textAlign`/`verticalAlign`), y con ellas aqui
            // un texto alineado estrenaria las diez claves del movil.
            alineacion: None,
            alineacion_vertical: None,
            // Ni la letra de lo que rotula: viaja en su `fontFamily`.
            familia: None,
            tam_letra: None,
            // Ni la dureza: viaja en su propia clave (`dureza`).
            dureza: None,
            // Ni la fuente rica: viaja en su propia clave (`textoRico`).
            rico: None,
            // Ni lo de las laminas de geometria: cada uno en su clave
            // (`cierreArco`, `marcaAngulo`, `degradado`).
            cierre: None,
            marca_angulo: None,
            degradado: None,
            ..self.clone()
        } == Extras::default()
    }

    /// Lo que ocupa de verdad, contando lo que hay al otro lado de los
    /// punteros. Lo usa `Elemento::bytes`.
    pub fn bytes(&self) -> usize {
        self.contenedor.as_ref().map_or(0, String::len)
            + self.rico.as_ref().map_or(0, String::len)
            + self.id_de_fichero.as_ref().map_or(0, String::len)
            + self
                .enganche_inicio
                .iter()
                .chain(self.enganche_fin.iter())
                .map(|b| b.elemento.len())
                .sum::<usize>()
            + self
                .atados
                .iter()
                .map(|a| a.id.len() + a.tipo.len() + size_of::<Atado>())
                .sum::<usize>()
    }
}

/// Lo que lleva una flecha en el extremo final cuando el dato falta: las dos
/// rayas de siempre. Es lo mismo que decia el viejo `punta_fin: bool` con su
/// `default = "verdadero"`, asi que un `.pixpin2d` guardado antes de las ocho
/// puntas se reabre con la flecha que tenia.
fn punta_de_flecha() -> crate::formas::TipoPunta {
    crate::formas::TipoPunta::Flecha
}

fn familia_por_defecto() -> String {
    "Segoe UI".to_string()
}

/// Un elemento del dibujo. `#[serde(default)]` en todo lo que se pueda: un
/// fichero de una version futura tiene que abrir igual (la misma regla que el
/// indice del almacen y los ajustes).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Elemento {
    pub id: u64,
    pub figura: Figura,
    pub x: f32,
    pub y: f32,
    pub ancho: f32,
    pub alto: f32,
    #[serde(default)]
    pub angulo: f32,
    pub trazo: ColorRgba,
    #[serde(default)]
    pub relleno: Option<ColorRgba>,
    /// Como se pinta ese relleno: plano o rayado (ver `relleno.rs`).
    ///
    /// El valor por omision del TIPO es rayado, que es lo que significa un
    /// elemento sin `fillStyle` en Excalidraw y en el movil; asi lo lee
    /// `excalidraw::leer`, explicitamente.
    ///
    /// Pero al leer un documento NUESTRO sin este campo se usa SOLIDO, que es
    /// como se veia cuando se guardo. Son dos origenes distintos y cada uno
    /// merece su respuesta: un `.excalidraw` sin `fillStyle` siempre quiso
    /// decir rayado, y un `.pixpin2d` de antes de que esto existiera se
    /// dibujo solido. Igualarlos cambiaria el aspecto de lo que el usuario ya
    /// tiene guardado, que es lo que no se puede hacer.
    #[serde(default = "relleno_solido")]
    pub estilo_relleno: EstiloRelleno,
    pub grosor: f32,
    #[serde(default = "estilo_por_defecto")]
    pub estilo: EstiloTrazo,
    #[serde(default = "uno")]
    pub rugosidad: f32,
    #[serde(default = "uno")]
    pub opacidad: f32,
    /// Sin ella el dibujo cambiaria de aspecto en cada apertura (D38).
    #[serde(default = "uno_u32")]
    pub semilla: u32,
    #[serde(default)]
    pub version: u32,
    /// Borrado logico: deshacer es volver a ponerlo en `false`, sin recuperar
    /// nada de disco.
    #[serde(default)]
    pub borrado: bool,
    /// Los grupos a los que pertenece, con los identificadores del movil
    /// (`groupIds` de Excalidraw).
    ///
    /// Cadenas y no numeros porque el movil las genera como cadenas y esto
    /// viaja de ida y vuelta sin tocarlas. Inventar aqui un `u64`
    /// obligaria a mantener una tabla de traduccion, que es una segunda
    /// verdad sobre lo mismo.
    #[serde(default)]
    pub grupos: Vec<String>,
    /// Bloqueado: se ve pero no se puede elegir ni mover (`locked` de
    /// Excalidraw). Es lo que se usa para dejar quieto un plano de fondo y
    /// dibujar encima sin arrastrarlo sin querer.
    #[serde(default)]
    pub bloqueado: bool,
    /// A donde lleva este elemento: el `enlace` de PixPin Android, que es el
    /// id del dibujo de otra hoja. Es lo que convierte un recuadro en la
    /// puerta a un sublienzo —la «zona» de una pagina—.
    #[serde(default)]
    pub enlace: Option<String>,
    /// Esquinas redondeadas (`roundness` de Excalidraw). Sin esto, un
    /// recuadro que el movil dibuja redondeado sale aqui en punta, y encima
    /// del suyo parece otro recuadro distinto.
    #[serde(default)]
    pub redondo: bool,
    /// **De que esta hecha su tinta** (`material` del movil, v0.59). No toca
    /// la geometria: cambiar de material no mueve la figura ni un pixel, solo
    /// cambia como se pinta lo que ya hay. Ver `tinta::material`.
    #[serde(default)]
    pub material: crate::tinta::MaterialTinta,
    /// Los diez campos del movil que el PC modela pero todavia no manda del
    /// todo: ver [`Extras`], que explica por que van juntos.
    #[serde(default)]
    pub extras: Extras,
}

impl Default for Elemento {
    /// Un rectangulo vacio en el origen, con el estilo de fabrica.
    ///
    /// Existe para que quien monta un `Elemento` a mano pueda escribir
    /// `..Default::default()` y no tenga que volver a tocarse cada vez que
    /// aqui nace un campo. Es lo que hace que anadir `material` no sea un
    /// remiendo en veinte ficheros ajenos.
    fn default() -> Self {
        Self {
            id: 0,
            figura: Figura::Rectangulo,
            x: 0.0,
            y: 0.0,
            ancho: 0.0,
            alto: 0.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            relleno: None,
            estilo_relleno: EstiloRelleno::default(),
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
            bloqueado: false,
            enlace: None,
            redondo: false,
            material: crate::tinta::MaterialTinta::Lisa,
            extras: Extras::default(),
        }
    }
}

fn relleno_solido() -> EstiloRelleno {
    EstiloRelleno::Solido
}

fn estilo_por_defecto() -> EstiloTrazo {
    EstiloTrazo::Solido
}

fn uno() -> f32 {
    1.0
}

fn uno_u32() -> u32 {
    1
}

impl Elemento {
    /// La caja que ocupa, en coordenadas del documento.
    ///
    /// Para las figuras con puntos se calcula de los puntos, no de los campos
    /// `x/ancho`: al dibujar un trazo, los puntos van cambiando y la caja
    /// tiene que seguirlos.
    pub fn caja(&self) -> (f32, f32, f32, f32) {
        match &self.figura {
            Figura::Lapiz { puntos, .. }
            | Figura::Resaltador { puntos }
            | Figura::Linea { puntos }
            | Figura::Flecha { puntos, .. }
            | Figura::Cota { puntos } => {
                if puntos.is_empty() {
                    return (self.x, self.y, self.x, self.y);
                }
                // Un lapiz de E1 (con `opciones`) guarda el `strokeWidth` de
                // Excalidraw, no pixeles: su tinta llega a `grosor *
                // FACTOR_VARIABLE` de ancho (ver `tinta::contorno_de_lapiz`).
                // Con la mitad del grosor a secas, el recorte de lo que no se
                // ve hacia saltar el trazo y el marco de seleccion lo cortaba.
                let mitad = match &self.figura {
                    Figura::Lapiz {
                        opciones: Some(_), ..
                    } => self.grosor * crate::tinta::FACTOR_VARIABLE / 2.0,
                    _ => self.grosor / 2.0,
                };
                (
                    puntos.iter().map(|p| p.x).fold(f32::MAX, f32::min) - mitad,
                    puntos.iter().map(|p| p.y).fold(f32::MAX, f32::min) - mitad,
                    puntos.iter().map(|p| p.x).fold(f32::MIN, f32::max) + mitad,
                    puntos.iter().map(|p| p.y).fold(f32::MIN, f32::max) + mitad,
                )
            }
            // **La caja del trozo que se ve, no la del ovalo del que salio.**
            // Un arco guarda el ovalo entero y aparte su tramo, asi que con
            // la caja cruda una una de trazo se seleccionaba por el recuadro
            // del circulo completo: encerrarla con el raton no la cogia
            // nunca, porque el recuadro tenia que contener una caja que no se
            // veia por ninguna parte.
            // El sector y la marca de angulo llegan hasta el centro.
            Figura::Arco { inicio, barrido } => {
                let (x0, y0, x1, y1) = crate::arco::caja_del_arco(
                    (self.x, self.y, self.ancho, self.alto),
                    *inicio,
                    *barrido,
                );
                let al_centro = self.extras.marca_angulo.is_some()
                    || self.extras.cierre == Some(crate::geometria_exacta::CierreArco::Sector);
                if al_centro {
                    let (cx, cy) = (self.x + self.ancho / 2.0, self.y + self.alto / 2.0);
                    (x0.min(cx), y0.min(cy), x1.max(cx), y1.max(cy))
                } else {
                    (x0, y0, x1, y1)
                }
            }
            // La region NO tiene caja propia en el fichero: `width` y
            // `height` del movil son los de su contorno encontrado, y si se
            // usaran aqui un anillo recortado se seleccionaria por un
            // rectangulo que no toca. Se mide lo que de verdad hay pintado.
            Figura::Region { contorno, .. } if !contorno.is_empty() => (
                contorno.iter().map(|p| p.x).fold(f32::MAX, f32::min),
                contorno.iter().map(|p| p.y).fold(f32::MAX, f32::min),
                contorno.iter().map(|p| p.x).fold(f32::MIN, f32::max),
                contorno.iter().map(|p| p.y).fold(f32::MIN, f32::max),
            ),
            _ => (self.x, self.y, self.x + self.ancho, self.y + self.alto),
        }
    }

    /// Mueve el elemento. Con puntos propios se mueven los puntos: si solo se
    /// moviera `x/y`, un trazo se quedaria donde estaba.
    pub fn mover(&mut self, dx: f32, dy: f32) {
        self.x += dx;
        self.y += dy;
        match &mut self.figura {
            Figura::Lapiz { puntos, .. }
            | Figura::Resaltador { puntos }
            | Figura::Linea { puntos }
            | Figura::Flecha { puntos, .. }
            | Figura::Cota { puntos } => {
                for p in puntos.iter_mut() {
                    p.x += dx;
                    p.y += dy;
                }
            }
            // **Los huecos se mueven con el contorno o el anillo se abre.**
            // Por eso la region no asoma sus puntos por `puntos()`: quien los
            // moviera por ahi dejaria los agujeros donde estaban y el relleno
            // taparia justo lo que se queria dejar ver.
            Figura::Region { contorno, huecos } => {
                for p in contorno.iter_mut().chain(huecos.iter_mut().flatten()) {
                    p.x += dx;
                    p.y += dy;
                }
            }
            _ => {}
        }
        self.version = self.version.wrapping_add(1);
    }

    /// Anota que el elemento cambio: invalida su geometria cacheada.
    pub fn tocar(&mut self) {
        self.version = self.version.wrapping_add(1);
    }

    /// Los puntos de la figura, si los tiene.
    pub fn puntos(&self) -> Option<&[Punto2]> {
        match &self.figura {
            Figura::Lapiz { puntos, .. }
            | Figura::Resaltador { puntos }
            | Figura::Linea { puntos }
            | Figura::Flecha { puntos, .. }
            | Figura::Cota { puntos } => Some(puntos),
            _ => None,
        }
    }

    /// Si el interior cuenta para el hit-test y para el dibujo.
    pub fn tiene_relleno(&self) -> bool {
        self.relleno.is_some_and(|c| c.a > 0.0)
    }

    /// Lo que ocupa de verdad, contando lo que hay al otro lado de los
    /// punteros. `size_of` solo cuenta la cabecera, y un trazo de 492
    /// puntos son cuatro kilobytes que no apareceran en el techo.
    pub fn bytes(&self) -> usize {
        let dentro = match &self.figura {
            Figura::Lapiz {
                puntos, presiones, ..
            } => puntos.len() * size_of::<Punto2>() + presiones.len() * size_of::<f32>(),
            Figura::Resaltador { puntos }
            | Figura::Linea { puntos }
            | Figura::Flecha { puntos, .. }
            | Figura::Cota { puntos } => puntos.len() * size_of::<Punto2>(),
            Figura::Texto { texto, familia, .. } => texto.len() + familia.len(),
            Figura::Region { contorno, huecos } => {
                (contorno.len() + huecos.iter().map(Vec::len).sum::<usize>()) * size_of::<Punto2>()
            }
            Figura::Punto { letra, .. } => letra.len(),
            _ => 0,
        };
        let grupos: usize = self.grupos.iter().map(String::len).sum();
        size_of::<Elemento>() + dentro + grupos + self.extras.bytes()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn lapiz() -> Elemento {
        Elemento {
            id: 1,
            figura: Figura::Lapiz {
                puntos: vec![
                    Punto2::nuevo(10.0, 10.0),
                    Punto2::nuevo(50.0, 30.0),
                    Punto2::nuevo(20.0, 60.0),
                ],
                presiones: vec![],
                opciones: None,
            },
            x: 0.0,
            y: 0.0,
            ancho: 0.0,
            alto: 0.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            estilo_relleno: Default::default(),
            relleno: None,
            grosor: 4.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 42,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
            bloqueado: false,
            enlace: None,
            redondo: false,
            material: Default::default(),
            extras: Default::default(),
        }
    }

    fn caja_rect() -> Elemento {
        Elemento {
            figura: Figura::Rectangulo,
            x: 100.0,
            y: 50.0,
            ancho: 200.0,
            alto: 80.0,
            ..lapiz()
        }
    }

    #[test]
    fn la_caja_de_un_trazo_sale_de_sus_puntos_y_cuenta_el_grosor() {
        // Si la caja no contara el grosor, la mitad del trazo quedaria fuera
        // y al borrar la zona quedarian restos de tinta.
        let (x0, y0, x1, y1) = lapiz().caja();
        assert_eq!((x0, y0, x1, y1), (8.0, 8.0, 52.0, 62.0));
    }

    #[test]
    fn la_caja_de_un_lapiz_de_e1_cubre_la_tinta_ancha_de_excalidraw() {
        // Con opciones, el grosor es un strokeWidth y la tinta mide grosor *
        // FACTOR_VARIABLE: la caja crece la mitad de eso a cada lado. El
        // trazo viejo (sin opciones) conserva la mitad del grosor.
        let mut e = lapiz();
        e.figura = Figura::Lapiz {
            puntos: vec![Punto2::nuevo(10.0, 10.0), Punto2::nuevo(50.0, 60.0)],
            presiones: vec![],
            opciones: Some(crate::tinta::OpcionesTinta::default()),
        };
        e.grosor = 2.0;
        let margen = 2.0 * crate::tinta::FACTOR_VARIABLE / 2.0;
        assert_eq!(
            e.caja(),
            (10.0 - margen, 10.0 - margen, 50.0 + margen, 60.0 + margen)
        );
    }

    #[test]
    fn mover_un_trazo_mueve_sus_puntos_y_no_solo_su_origen() {
        // El fallo clasico: mover x/y y dejar los puntos donde estaban, con lo
        // que el trazo no se mueve pero su caja si.
        let mut e = lapiz();
        e.mover(100.0, -20.0);
        let p = e.puntos().unwrap();
        assert_eq!(p[0], Punto2::nuevo(110.0, -10.0));
        assert_eq!(p[2], Punto2::nuevo(120.0, 40.0));
        assert_eq!(e.caja().0, 108.0);
    }

    #[test]
    fn cualquier_cambio_sube_la_version() {
        // Es lo que invalida la geometria cacheada; sin esto el elemento se
        // seguiria dibujando en su sitio anterior.
        let mut e = lapiz();
        let antes = e.version;
        e.mover(1.0, 0.0);
        assert_eq!(e.version, antes + 1);
        e.tocar();
        assert_eq!(e.version, antes + 2);
    }

    #[test]
    fn la_caja_de_una_figura_sin_puntos_es_su_rectangulo() {
        assert_eq!(caja_rect().caja(), (100.0, 50.0, 300.0, 130.0));
    }

    #[test]
    fn un_trazo_vacio_no_entra_en_panico_al_medirse() {
        let mut e = lapiz();
        e.figura = Figura::Lapiz {
            puntos: vec![],
            presiones: vec![],
            opciones: None,
        };
        let (x0, y0, x1, y1) = e.caja();
        assert!(x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite());
    }

    #[test]
    fn un_relleno_transparente_no_cuenta_como_relleno() {
        // Caso negativo: `Some(color)` con alfa 0 es "sin relleno", y tratarlo
        // como relleno haria que el interior de la figura capturase clics
        // destinados a lo que hay debajo.
        let mut e = caja_rect();
        assert!(!e.tiene_relleno());
        e.relleno = Some(ColorRgba {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        });
        assert!(!e.tiene_relleno());
        e.relleno = Some(ColorRgba::opaco(1.0, 0.0, 0.0));
        assert!(e.tiene_relleno());
    }

    #[test]
    fn un_elemento_de_una_version_futura_se_lee_con_lo_que_falta_por_defecto() {
        // La misma regla que el indice del almacen: campos nuevos y campos
        // que faltan no pueden impedir abrir el documento.
        let json = r#"{
            "id": 7,
            "figura": { "tipo": "rectangulo" },
            "x": 1.0, "y": 2.0, "ancho": 3.0, "alto": 4.0,
            "trazo": { "r": 0.0, "g": 0.0, "b": 0.0, "a": 1.0 },
            "grosor": 2.0,
            "funcion_del_futuro": 42
        }"#;
        let e: Elemento = serde_json::from_str(json).unwrap();
        assert_eq!(e.id, 7);
        assert_eq!(e.rugosidad, 1.0, "la rugosidad por defecto es 1");
        assert_eq!(e.semilla, 1, "una semilla ausente vale 1, nunca 0");
        assert_eq!(e.estilo, EstiloTrazo::Solido);
        // Esto es un documento NUESTRO, no un `.excalidraw`: uno guardado
        // antes de que existiera este campo se dibujo solido, y reabrirlo
        // rayado cambiaria el aspecto de lo que el usuario ya tiene. El
        // rayado es lo que significa un `.excalidraw` SIN `fillStyle`, y de
        // eso se encarga `excalidraw::leer`, que lo pone explicitamente.
        assert_eq!(
            e.estilo_relleno,
            EstiloRelleno::Solido,
            "un documento nuestro de antes de este campo se dibujo solido"
        );
        assert!(!e.borrado);
    }

    #[test]
    fn la_ida_y_vuelta_por_json_conserva_el_elemento() {
        let e = lapiz();
        let texto = serde_json::to_string(&e).unwrap();
        let vuelta: Elemento = serde_json::from_str(&texto).unwrap();
        assert_eq!(e, vuelta);
    }
}
