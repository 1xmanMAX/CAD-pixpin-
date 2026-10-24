//! Rectangulos, elipses, lineas y flechas dibujados "a mano".
//!
//! Una linea recta perfecta no parece dibujada. El truco que usan todas las
//! herramientas de este estilo es doble:
//!
//! 1. **Desviar los extremos** un poco, con el azar reproducible del
//!    elemento, y curvar el trazo por el medio con una Bezier cubica cuyos
//!    puntos de control tambien estan desviados.
//! 2. **Trazar cada linea DOS veces**, con desvios distintos. Dos pasadas casi
//!    iguales es exactamente lo que hace una mano al repasar un boceto, y es
//!    lo que separa "linea temblorosa" de "linea dibujada".
//!
//! El azar viene del elemento (su semilla), asi que el mismo rectangulo se
//! dibuja igual siempre: es lo que permite guardar un documento y reabrirlo
//! sin que cambie de aspecto.

use serde::{Deserialize, Serialize};

use crate::azar::Azar;
use crate::rough;
use crate::vector::Punto2;

/// Cuantos tramos tiene cada linea "a mano": suficientes para que la curva se
/// vea suave sin llenar la geometria de puntos.
const TRAMOS: usize = 12;

/// Una linea "a mano" de `a` a `b`: dos pasadas, cada una con su desvio.
///
/// `rugosidad` 0 devuelve la linea recta exacta (util para reglas y para el
/// resaltador); 1 es el aspecto normal; 2, muy marcado.
pub fn linea(a: Punto2, b: Punto2, rugosidad: f32, azar: &mut Azar) -> Vec<Vec<Punto2>> {
    if rugosidad <= 0.0 {
        return vec![vec![a, b]];
    }
    // El desvio crece con la longitud pero se estanca: en una linea de mil
    // pixeles, un temblor proporcional se veria como un garabato.
    let largo = a.distancia(b);
    let desvio = (largo / 40.0).clamp(1.0, 4.0) * rugosidad;

    (0..2)
        .map(|pasada| {
            // La segunda pasada desvia algo menos: al repasar, la mano ya
            // sabe por donde va.
            let d = if pasada == 0 { desvio } else { desvio * 0.7 };
            let inicio = Punto2::nuevo(a.x + azar.desvio(d), a.y + azar.desvio(d));
            let fin = Punto2::nuevo(b.x + azar.desvio(d), b.y + azar.desvio(d));
            // Dos puntos de control desviados: la panza de la linea.
            let c1 = a
                .hacia(b, 0.33)
                .sumar(Punto2::nuevo(azar.desvio(d * 2.0), azar.desvio(d * 2.0)));
            let c2 = a
                .hacia(b, 0.66)
                .sumar(Punto2::nuevo(azar.desvio(d * 2.0), azar.desvio(d * 2.0)));
            bezier(inicio, c1, c2, fin)
        })
        .collect()
}

/// **La raya de rough.js como la pinta el movil** (`Rough.doubleLine`, que
/// son dos `lineOps`): la de las lineas, las flechas y la cota.
///
/// `linea` de arriba era una aproximacion propia con un desvio que crecia
/// con el largo hasta 4 px en los extremos y 8 en la panza: una linea larga
/// del movil (que alli sale casi recta) salia aqui visiblemente torcida, y
/// el usuario lo vio («las lineas se representan con error», 27-sep-2026).
/// Lo que hace el movil, y rough.js, es lo contrario: **cuanto mas larga,
/// menos tiembla** (`roughnessGain` baja a 0,4 pasados 500 px), el ruido de
/// los extremos se topa en `maxRandomnessOffset` (2 px) y la panza sale del
/// `bowing`, no de un desvio a ojo.
///
/// `preservar` es `preserveVertices` (rugosidad menor que la de dibujante):
/// los extremos no se mueven, y la raya empieza y acaba donde se solto.
/// Con rugosidad cero, una sola pasada exacta: dos iguales solo engordarian.
pub fn linea_rough(
    a: Punto2,
    b: Punto2,
    rugosidad: f32,
    preservar: bool,
    azar: &mut Azar,
) -> Vec<Vec<Punto2>> {
    if rugosidad <= 0.0 {
        return vec![vec![a, b]];
    }
    vec![
        pasada_rough(a, b, rugosidad, preservar, false, azar),
        pasada_rough(a, b, rugosidad, preservar, true, azar),
    ]
}

/// `maxRandomnessOffset` y `bowing` de rough.js (`RoughOptions` del movil).
const DESVIO_MAXIMO: f32 = 2.0;
const PANZA: f32 = 1.0;

/// Una pasada (`lineOps`). `repaso` es la segunda (`overlay`), con la mitad
/// de ruido para que las dos se crucen en vez de ir paralelas.
fn pasada_rough(
    a: Punto2,
    b: Punto2,
    rug: f32,
    preservar: bool,
    repaso: bool,
    azar: &mut Azar,
) -> Vec<Punto2> {
    let largo2 = (a.x - b.x).powi(2) + (a.y - b.y).powi(2);
    let largo = largo2.sqrt();
    // Las lineas largas se dibujan mas rectas: el temblor constante en un
    // trazo de 800 px se ve como un error, no como un dibujo a mano.
    let ganancia = if largo < 200.0 {
        1.0
    } else if largo > 500.0 {
        0.4
    } else {
        -0.0016668 * largo + 1.233334
    };
    let mut desvio = DESVIO_MAXIMO;
    if desvio * desvio * 100.0 > largo2 {
        desvio = largo / 10.0;
    }
    let medio_desvio = desvio / 2.0;
    let divergencia = 0.2 + azar.siguiente() * 0.2;
    // `offsetOpt(x, gain)` = rugosidad * ganancia * (azar en [-x, x)).
    let mut ruido = |x: f32| rug * ganancia * azar.desvio(x);
    let panza_x = ruido(PANZA * DESVIO_MAXIMO * (b.y - a.y) / 200.0);
    let panza_y = ruido(PANZA * DESVIO_MAXIMO * (a.x - b.x) / 200.0);
    let d = if repaso { medio_desvio } else { desvio };
    let inicio = if preservar {
        a
    } else {
        Punto2::nuevo(a.x + ruido(d), a.y + ruido(d))
    };
    let c1 = Punto2::nuevo(
        panza_x + a.x + (b.x - a.x) * divergencia + ruido(d),
        panza_y + a.y + (b.y - a.y) * divergencia + ruido(d),
    );
    let c2 = Punto2::nuevo(
        panza_x + a.x + 2.0 * (b.x - a.x) * divergencia + ruido(d),
        panza_y + a.y + 2.0 * (b.y - a.y) * divergencia + ruido(d),
    );
    let fin = if preservar {
        b
    } else {
        Punto2::nuevo(b.x + ruido(d), b.y + ruido(d))
    };
    bezier(inicio, c1, c2, fin)
}

/// Bezier cubica evaluada en `TRAMOS` tramos.
fn bezier(p0: Punto2, c1: Punto2, c2: Punto2, p3: Punto2) -> Vec<Punto2> {
    (0..=TRAMOS)
        .map(|i| {
            let t = i as f32 / TRAMOS as f32;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            Punto2::nuevo(
                p0.x * a + c1.x * b + c2.x * c + p3.x * d,
                p0.y * a + c1.y * b + c2.y * c + p3.y * d,
            )
        })
        .collect()
}

/// Los cuatro lados de un rectangulo, cada uno con sus dos pasadas.
pub fn rectangulo(
    x: f32,
    y: f32,
    ancho: f32,
    alto: f32,
    rugosidad: f32,
    azar: &mut Azar,
) -> Vec<Vec<Punto2>> {
    let e = [
        Punto2::nuevo(x, y),
        Punto2::nuevo(x + ancho, y),
        Punto2::nuevo(x + ancho, y + alto),
        Punto2::nuevo(x, y + alto),
    ];
    let mut salida = Vec::with_capacity(8);
    for i in 0..4 {
        salida.extend(linea(e[i], e[(i + 1) % 4], rugosidad, azar));
    }
    salida
}

/// Los cuatro lados de un rombo, cada uno con sus dos pasadas.
///
/// Los vertices son los puntos medios de los lados de su caja, que es como
/// los pone Excalidraw (`diamond`): asi un rombo y un rectangulo del mismo
/// tamano se estiran igual y los tiradores caen en el mismo sitio.
pub fn rombo(
    x: f32,
    y: f32,
    ancho: f32,
    alto: f32,
    rugosidad: f32,
    azar: &mut Azar,
) -> Vec<Vec<Punto2>> {
    let e = vertices_de_rombo(x, y, ancho, alto);
    let mut salida = Vec::with_capacity(8);
    for i in 0..4 {
        salida.extend(linea(e[i], e[(i + 1) % 4], rugosidad, azar));
    }
    salida
}

/// Los cuatro vertices de un rombo, en orden. Aparte de [`rombo`] porque el
/// relleno los necesita sin temblor: rellenar la figura temblorosa deja
/// huecos por donde se escapa el fondo.
pub fn vertices_de_rombo(x: f32, y: f32, ancho: f32, alto: f32) -> [Punto2; 4] {
    [
        Punto2::nuevo(x + ancho / 2.0, y),
        Punto2::nuevo(x + ancho, y + alto / 2.0),
        Punto2::nuevo(x + ancho / 2.0, y + alto),
        Punto2::nuevo(x, y + alto / 2.0),
    ]
}

/// Una elipse "a mano": dos vueltas completas ligeramente distintas, lisas.
///
/// Pasa por los mismos vertices de siempre (`vertices_de_elipse`, con la
/// misma semilla y el mismo azar gastado, asi que la figura guardada no cambia
/// de sitio ni de temblor), pero los une con la spline de rough.js (`_curve`:
/// Catmull-Rom convertida en cubicas) en vez de con rectas. Unidos con rectas,
/// 32 vertices sacudidos cada uno por su lado dejaban un poligono: con el
/// trazo a mano medio o alto se veian las esquinas (lo vio el usuario). En
/// Excalidraw la elipse tambien es una curva por puntos sacudidos, nunca una
/// quebrada.
pub fn elipse(
    x: f32,
    y: f32,
    ancho: f32,
    alto: f32,
    rugosidad: f32,
    azar: &mut Azar,
) -> Vec<Vec<Punto2>> {
    let vueltas = vertices_de_elipse(x, y, ancho, alto, rugosidad, azar);
    // Sin temblor es una vuelta exacta que cierra en su primer punto: se
    // alisa como lazo cerrado para que la costura tampoco haga pico.
    let cerrada = rugosidad <= 0.0;
    vueltas
        .iter()
        .map(|v| spline_por_puntos(v, cerrada))
        .collect()
}

/// Lo que mide, como mucho, cada trocito recto de la spline. Tres unidades
/// es menos de lo que el ojo distingue de una curva al 100 %, y una elipse de
/// 200 de ancho se queda en unos 250 puntos por vuelta.
const TROCITO_DE_SPLINE: f32 = 3.0;

/// La spline de rough.js (`_curve` con `curveTightness` 0) por `puntos`,
/// muestreada en rectas cortas. Cada tramo de `p[i]` a `p[i+1]` es la cubica
/// con controles `p[i] + (p[i+1] - p[i-1]) / 6` y `p[i+1] - (p[i+2] - p[i]) / 6`:
/// pasa por todos los puntos y sale de cada uno con la tangente de sus dos
/// vecinos, que es lo que borra el pico. `cerrada` toma los vecinos dando la
/// vuelta; abierta, repite el extremo.
fn spline_por_puntos(puntos: &[Punto2], cerrada: bool) -> Vec<Punto2> {
    let n = puntos.len();
    if n < 3 {
        return puntos.to_vec();
    }
    // En un lazo cerrado el ultimo punto repite el primero: los vecinos se
    // buscan entre los `n - 1` distintos.
    let distintos = if cerrada { n - 1 } else { n };
    let en = |i: isize| -> Punto2 {
        if cerrada {
            puntos[i.rem_euclid(distintos as isize) as usize]
        } else {
            puntos[i.clamp(0, n as isize - 1) as usize]
        }
    };
    let mut salida = Vec::with_capacity(n * 4);
    salida.push(puntos[0]);
    for i in 0..n - 1 {
        let i = i as isize;
        let (p0, p1, p2, p3) = (en(i - 1), en(i), en(i + 1), en(i + 2));
        let c1 = Punto2::nuevo(p1.x + (p2.x - p0.x) / 6.0, p1.y + (p2.y - p0.y) / 6.0);
        let c2 = Punto2::nuevo(p2.x - (p3.x - p1.x) / 6.0, p2.y - (p3.y - p1.y) / 6.0);
        let trozos = ((p1.distancia(p2) / TROCITO_DE_SPLINE).ceil() as usize).clamp(1, 16);
        for k in 1..=trozos {
            salida.push(cubica(p1, c1, c2, p2, k as f32 / trozos as f32));
        }
    }
    salida
}

/// Los vertices sacudidos de la elipse "a mano", sin alisar: dos vueltas.
///
/// No se cierra en el mismo punto donde empieza a proposito: una elipse
/// dibujada a mano casi nunca cierra exacta, y ese pequeño exceso es lo que
/// la hace creible. Es lo que se pintaba antes, unido con rectas; se deja
/// aparte para que `elipse` los alise sin tocar el azar, y para poder
/// comparar el antes y el despues.
pub fn vertices_de_elipse(
    x: f32,
    y: f32,
    ancho: f32,
    alto: f32,
    rugosidad: f32,
    azar: &mut Azar,
) -> Vec<Vec<Punto2>> {
    let cx = x + ancho / 2.0;
    let cy = y + alto / 2.0;
    let rx = ancho / 2.0;
    let ry = alto / 2.0;

    if rugosidad <= 0.0 {
        let pasos = 48;
        return vec![
            (0..=pasos)
                .map(|i| {
                    let t = std::f32::consts::TAU * i as f32 / pasos as f32;
                    Punto2::nuevo(cx + rx * t.cos(), cy + ry * t.sin())
                })
                .collect(),
        ];
    }

    let desvio = ((rx + ry) / 40.0).clamp(1.0, 4.0) * rugosidad;
    let pasos = 32;
    (0..2)
        .map(|pasada| {
            let d = if pasada == 0 { desvio } else { desvio * 0.7 };
            // La segunda vuelta arranca con un pequeño desfase y se pasa un
            // poco del cierre: asi las dos pasadas no quedan superpuestas.
            let desfase = azar.desvio(0.4) + pasada as f32 * 0.3;
            let vuelta = std::f32::consts::TAU + if pasada == 1 { 0.35 } else { 0.0 };
            (0..=pasos)
                .map(|i| {
                    let t = desfase + vuelta * i as f32 / pasos as f32;
                    Punto2::nuevo(
                        cx + rx * t.cos() + azar.desvio(d),
                        cy + ry * t.sin() + azar.desvio(d),
                    )
                })
                .collect()
        })
        .collect()
}

/// **Las ocho puntas de flecha de Excalidraw**, con sus palabras del fichero.
///
/// Son las mismas ocho del movil (`Arrowhead` en `Element.kt:487-496`) y las
/// mismas de Excalidraw. Aqui solo habia «lleva punta o no»
/// (`Figura::Flecha{punta_inicio, punta_fin}`), asi que un diagrama entidad-
/// relacion del movil —donde la punta DICE la cardinalidad— se abria con ocho
/// flechas iguales y dejaba de decir nada.
///
/// Se serializa **por su nombre en minuscula** y no por la palabra del
/// `.excalidraw`: esto es el formato NUESTRO (`.pixpin2d`), y mezclar los dos
/// vocabularios en un mismo enum obligaria a elegir cual de los dos gana. La
/// traduccion al fichero del movil la hacen [`TipoPunta::palabra`] y
/// [`TipoPunta::desde_palabra`], que es donde tiene que estar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TipoPunta {
    /// Sin punta: es `null` en el fichero, no una palabra.
    #[default]
    Ninguna,
    /// Las dos rayas de siempre, abiertas.
    Flecha,
    /// Un travesano perpendicular.
    Barra,
    Circulo,
    CirculoHueco,
    Triangulo,
    TrianguloHueco,
    Rombo,
    RomboHueco,
}

/// Las ocho de la barra, en el orden del enum del movil. Sin `Ninguna`: no es
/// una punta, es no tener ninguna.
pub const PUNTAS: [TipoPunta; 8] = [
    TipoPunta::Flecha,
    TipoPunta::Barra,
    TipoPunta::Circulo,
    TipoPunta::CirculoHueco,
    TipoPunta::Triangulo,
    TipoPunta::TrianguloHueco,
    TipoPunta::Rombo,
    TipoPunta::RomboHueco,
];

impl TipoPunta {
    /// La palabra del `.excalidraw`. `None` es la ausencia de punta, que en el
    /// fichero se escribe `null` y no una cadena vacia.
    pub fn palabra(self) -> Option<&'static str> {
        Some(match self {
            TipoPunta::Ninguna => return None,
            TipoPunta::Flecha => "arrow",
            TipoPunta::Barra => "bar",
            TipoPunta::Circulo => "circle",
            TipoPunta::CirculoHueco => "circle_outline",
            TipoPunta::Triangulo => "triangle",
            TipoPunta::TrianguloHueco => "triangle_outline",
            TipoPunta::Rombo => "diamond",
            TipoPunta::RomboHueco => "diamond_outline",
        })
    }

    /// La punta de una palabra del fichero.
    ///
    /// Una palabra que no conocemos NO es «sin punta»: es una punta de una
    /// version futura, y leerla como ninguna la borraria al guardar. Devuelve
    /// `None` para que quien lee decida conservarla, la misma regla del
    /// `fillStyle` y del `material`.
    pub fn desde_palabra(s: &str) -> Option<TipoPunta> {
        PUNTAS.into_iter().find(|p| p.palabra() == Some(s))
    }

    /// Si se pinta maciza. Las «huecas» (`*_outline`) llevan solo el borde:
    /// es lo que distingue en un diagrama la herencia de la composicion.
    pub fn es_maciza(self) -> bool {
        matches!(
            self,
            TipoPunta::Circulo | TipoPunta::Triangulo | TipoPunta::Rombo
        )
    }

    /// Lo que mide la punta, en pixeles del documento (`arrowheadSize`).
    pub fn tamano(self) -> f32 {
        match self {
            TipoPunta::Flecha => 25.0,
            TipoPunta::Rombo | TipoPunta::RomboHueco => 12.0,
            _ => 15.0,
        }
    }

    /// Apertura de la punta, en grados (`getArrowheadAngle`). La barra se abre
    /// noventa: por eso sale perpendicular al trazo y no en pico.
    pub fn apertura(self) -> f32 {
        match self {
            TipoPunta::Barra => 90.0,
            TipoPunta::Flecha => 20.0,
            _ => 25.0,
        }
    }
}

/// La geometria de una punta, ya resuelta para dibujar (`ArrowheadShape`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FormaDePunta {
    pub tipo: TipoPunta,
    /// El extremo del trazo, donde se posa la punta.
    pub punta: Punto2,
    /// Los dos puntos de las alas, o los extremos del travesano de la barra.
    pub alas: (Punto2, Punto2),
    /// El cuarto vertice: solo el rombo lo usa.
    pub opuesto: Option<Punto2>,
    /// Solo el circulo: su diametro.
    pub diametro: f32,
}

/// El punto desde el que se mira la direccion de la punta.
///
/// **No es el de al lado.** Con dos puntos da igual, pero una flecha trazada a
/// pulso tiene los puntos a un pixel unos de otros: mirando solo al anterior
/// sale una direccion temblorosa y, peor, una punta del tamano de ese pixel
/// —el tamano se acota a la mitad de esa distancia—, o sea invisible. Se
/// retrocede por el trazo hasta separarse lo que mide la punta, y asi apunta a
/// donde iba la mano y sale del tamano que le toca. Es lo que hace usable la
/// **flecha a mano alzada**, que por dentro es una flecha con todos los puntos
/// del trazo en vez de dos.
fn de_donde_viene(puntos: &[Punto2], al_final: bool, cuanto: f32) -> Punto2 {
    let n = puntos.len();
    let extremo = if al_final { puntos[n - 1] } else { puntos[0] };
    let mut ultimo = if al_final { puntos[n - 2] } else { puntos[1] };
    let recorrido: Box<dyn Iterator<Item = usize>> = if al_final {
        Box::new((0..n).rev())
    } else {
        Box::new(0..n)
    };
    for i in recorrido {
        let p = puntos[i];
        ultimo = p;
        if extremo.distancia(p) >= cuanto {
            return p;
        }
    }
    ultimo
}

/// Los puntos de la punta de `tipo` en un extremo del trazo
/// (`getArrowheadPoints`).
///
/// `al_final` elige el extremo: falso es el principio (`startArrowhead`).
/// `grosor` es el del trazo, y solo lo usa el circulo para su diametro.
///
/// `None` cuando no hay punta que dibujar: sin dos puntos, con los dos
/// extremos encima, o sin tipo.
pub fn forma_de_punta(
    puntos: &[Punto2],
    al_final: bool,
    tipo: TipoPunta,
    grosor: f32,
) -> Option<FormaDePunta> {
    if tipo == TipoPunta::Ninguna || puntos.len() < 2 {
        return None;
    }
    let extremo = if al_final {
        puntos[puntos.len() - 1]
    } else {
        puntos[0]
    };
    let previo = de_donde_viene(puntos, al_final, tipo.tamano());
    let distancia = extremo.distancia(previo);
    if distancia == 0.0 {
        return None;
    }
    let direccion = extremo.restar(previo).escalar(1.0 / distancia);

    // La punta se encoge en flechas cortas: una punta de 25 px en un trazo de
    // 20 es toda la flecha y se ve como un borron.
    let fraccion = if matches!(tipo, TipoPunta::Rombo | TipoPunta::RomboHueco) {
        0.25
    } else {
        0.5
    };
    let medida = tipo.tamano().min(distancia * fraccion);
    let atras = extremo.restar(direccion.escalar(medida));

    if matches!(tipo, TipoPunta::Circulo | TipoPunta::CirculoHueco) {
        // El circulo no tiene alas: se centra en la punta y lo unico que hace
        // falta es lo gordo que es. El `-2` es del original: compensa que el
        // trazo ya ocupa su propio ancho.
        return Some(FormaDePunta {
            tipo,
            punta: extremo,
            alas: (extremo, extremo),
            opuesto: None,
            diametro: atras.distancia(extremo) + grosor - 2.0,
        });
    }

    let apertura = tipo.apertura().to_radians();
    let alas = (
        atras.girar(extremo, -apertura),
        atras.girar(extremo, apertura),
    );
    let opuesto = matches!(tipo, TipoPunta::Rombo | TipoPunta::RomboHueco).then(|| {
        let dir = (extremo.y - previo.y).atan2(extremo.x - previo.x);
        Punto2::nuevo(extremo.x - medida * 2.0, extremo.y).girar(extremo, dir)
    });
    Some(FormaDePunta {
        tipo,
        punta: extremo,
        alas,
        opuesto,
        diametro: 0.0,
    })
}

/// El contorno CERRADO de una punta, para rellenarla o para recorrerlo.
///
/// La flecha y la barra no cierran nada —son dos rayas y un travesano— y
/// devuelven la lista abierta que hay que trazar; el triangulo y el rombo
/// devuelven su poligono, y el circulo, su borde muestreado. Quien pinta mira
/// [`TipoPunta::es_maciza`] para decidir si lo rellena o solo lo traza.
pub fn contorno_de_punta(f: &FormaDePunta) -> Vec<Punto2> {
    match f.tipo {
        TipoPunta::Ninguna => Vec::new(),
        // Las dos rayas de siempre: del ala a la punta y de la punta a la
        // otra ala. Abierta a proposito: cerrarla la convertiria en un
        // triangulo, que es otra punta distinta.
        TipoPunta::Flecha => vec![f.alas.0, f.punta, f.alas.1],
        // El travesano: de ala a ala, pasando de largo de la punta.
        TipoPunta::Barra => vec![f.alas.0, f.alas.1],
        TipoPunta::Triangulo | TipoPunta::TrianguloHueco => {
            vec![f.alas.0, f.punta, f.alas.1, f.alas.0]
        }
        TipoPunta::Rombo | TipoPunta::RomboHueco => {
            let opuesto = f.opuesto.unwrap_or(f.punta);
            vec![f.alas.0, f.punta, f.alas.1, opuesto, f.alas.0]
        }
        TipoPunta::Circulo | TipoPunta::CirculoHueco => {
            const LADOS: usize = 24;
            let r = (f.diametro / 2.0).max(0.5);
            (0..=LADOS)
                .map(|i| {
                    let t = std::f32::consts::TAU * i as f32 / LADOS as f32;
                    Punto2::nuevo(f.punta.x + r * t.cos(), f.punta.y + r * t.sin())
                })
                .collect()
        }
    }
}

/// Un tramo de un contorno redondeado: una recta o una esquina curva.
///
/// Es la misma ruta que escribe Excalidraw en `shape.ts` (`M … L … Q …` para
/// el rectangulo, `L … C …` para el rombo): una sola descripcion de la forma
/// de la que salen **las tres cosas que tienen que coincidir** —el contorno
/// liso, el contorno a mano y el relleno—. Con tres cuentas sueltas, lo que
/// pasaba es lo que vio el usuario: el contorno redondo y el relleno recto,
/// asomando por las cuatro esquinas.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Tramo {
    Recta(Punto2, Punto2),
    /// Bezier cubica: inicio, dos tiradores y fin. La `Q` del rectangulo se
    /// pasa a cubica sin cambiar la curva (tiradores a 2/3 del de control).
    Curva(Punto2, Punto2, Punto2, Punto2),
}

/// Cuantos tramos rectos por esquina al muestrearla: a los zooms de trabajo
/// no se distingue de una curva de verdad.
const TRAMOS_ESQUINA: usize = 8;

/// La esquina cuadratica de `a` a `b` con control `c`, como cubica.
fn esquina_cuadratica(a: Punto2, c: Punto2, b: Punto2) -> Tramo {
    Tramo::Curva(a, a.hacia(c, 2.0 / 3.0), b.hacia(c, 2.0 / 3.0), b)
}

fn cubica(p0: Punto2, c1: Punto2, c2: Punto2, p3: Punto2, t: f32) -> Punto2 {
    let u = 1.0 - t;
    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    Punto2::nuevo(
        p0.x * a + c1.x * b + c2.x * c + p3.x * d,
        p0.y * a + c1.y * b + c2.y * c + p3.y * d,
    )
}

/// El contorno liso y CERRADO de unos tramos: el mismo para el trazo sin
/// temblor y para el relleno, que asi no se sale de su contenedor.
fn contorno_liso(tramos: &[Tramo]) -> Vec<Punto2> {
    let mut salida = Vec::with_capacity(tramos.len() * (TRAMOS_ESQUINA + 1) + 1);
    for t in tramos {
        match *t {
            Tramo::Recta(a, _) => salida.push(a),
            Tramo::Curva(a, c1, c2, b) => {
                for i in 0..TRAMOS_ESQUINA {
                    salida.push(cubica(a, c1, c2, b, i as f32 / TRAMOS_ESQUINA as f32));
                }
            }
        }
    }
    if let Some(primero) = salida.first().copied() {
        salida.push(primero);
    }
    salida
}

/// La ruta del rectangulo redondeado de Excalidraw (`shape.ts`, rama
/// `roundness`): radio `getCornerRadius` del lado menor, rectas entre las
/// esquinas y cada esquina una `Q` con el vertice de control.
fn tramos_de_rectangulo_redondo(x: f32, y: f32, ancho: f32, alto: f32) -> Option<[Tramo; 8]> {
    // Normalizado: una caja estirada hacia arriba o a la izquierda tiene
    // ancho o alto negativo, y el radio va con el tamano, no con el signo.
    let (x0, x1) = (x.min(x + ancho), x.max(x + ancho));
    let (y0, y1) = (y.min(y + alto), y.max(y + alto));
    let r = radio_de_esquina((x1 - x0).min(y1 - y0));
    if r <= 0.5 {
        return None;
    }
    let p = Punto2::nuevo;
    Some([
        Tramo::Recta(p(x0 + r, y0), p(x1 - r, y0)),
        esquina_cuadratica(p(x1 - r, y0), p(x1, y0), p(x1, y0 + r)),
        Tramo::Recta(p(x1, y0 + r), p(x1, y1 - r)),
        esquina_cuadratica(p(x1, y1 - r), p(x1, y1), p(x1 - r, y1)),
        Tramo::Recta(p(x1 - r, y1), p(x0 + r, y1)),
        esquina_cuadratica(p(x0 + r, y1), p(x0, y1), p(x0, y1 - r)),
        Tramo::Recta(p(x0, y1 - r), p(x0, y0 + r)),
        esquina_cuadratica(p(x0, y0 + r), p(x0, y0), p(x0 + r, y0)),
    ])
}

/// La ruta del rombo redondeado de Excalidraw (`shape.ts`, rama `diamond`).
///
/// La clave esta en **como recorta**: no avanza por la arista una distancia,
/// sino que se desplaza `vr` en horizontal y `hr` en vertical, con **un radio
/// distinto por eje** —el de la media anchura y el de la media altura—. Por
/// eso un rombo aplastado no se redondea igual arriba que a los lados, que es
/// como tiene que ser: con un radio unico las puntas laterales quedarian romas
/// y las de arriba en pico. Cada punta es una `C` con los dos tiradores en el
/// vertice, y la costura queda sobre el tramo recto que baja a la derecha,
/// como en el original: partirla en mitad de una curva dejaria el empalme a
/// la vista.
fn tramos_de_rombo_redondo(x: f32, y: f32, ancho: f32, alto: f32) -> Option<[Tramo; 8]> {
    let [arriba, derecha, abajo, izquierda] = vertices_de_rombo(x, y, ancho, alto);
    let vr = radio_de_esquina((ancho / 2.0).abs());
    let hr = radio_de_esquina((alto / 2.0).abs());
    if vr <= 0.0 || hr <= 0.0 {
        return None;
    }
    let p = Punto2::nuevo;
    let punta = |v: Punto2, entrada: Punto2, salida: Punto2| Tramo::Curva(entrada, v, v, salida);
    Some([
        Tramo::Recta(
            p(arriba.x + vr, arriba.y + hr),
            p(derecha.x - vr, derecha.y - hr),
        ),
        punta(
            derecha,
            p(derecha.x - vr, derecha.y - hr),
            p(derecha.x - vr, derecha.y + hr),
        ),
        Tramo::Recta(
            p(derecha.x - vr, derecha.y + hr),
            p(abajo.x + vr, abajo.y - hr),
        ),
        punta(
            abajo,
            p(abajo.x + vr, abajo.y - hr),
            p(abajo.x - vr, abajo.y - hr),
        ),
        Tramo::Recta(
            p(abajo.x - vr, abajo.y - hr),
            p(izquierda.x + vr, izquierda.y + hr),
        ),
        punta(
            izquierda,
            p(izquierda.x + vr, izquierda.y + hr),
            p(izquierda.x + vr, izquierda.y - hr),
        ),
        Tramo::Recta(
            p(izquierda.x + vr, izquierda.y - hr),
            p(arriba.x - vr, arriba.y + hr),
        ),
        punta(
            arriba,
            p(arriba.x - vr, arriba.y + hr),
            p(arriba.x + vr, arriba.y + hr),
        ),
    ])
}

/// Un rombo con las puntas redondeadas, liso y cerrado. Es el contorno del
/// relleno y el trazo sin temblor; sin tamano, sus cuatro vertices.
pub fn rombo_redondo(x: f32, y: f32, ancho: f32, alto: f32) -> Vec<Punto2> {
    match tramos_de_rombo_redondo(x, y, ancho, alto) {
        Some(t) => contorno_liso(&t),
        None => vertices_de_rombo(x, y, ancho, alto).to_vec(),
    }
}

/// El trazo de un rombo redondeado con cualquier rugosidad: con 0, el
/// contorno liso en una pasada (la ruta de Excalidraw, con sus `C`); con
/// mas, el `roughContinuous` del movil: la curva suave de rough.js por el
/// contorno muestreado parejo, con los vertices quietos.
pub fn rombo_redondo_a_mano(
    x: f32,
    y: f32,
    ancho: f32,
    alto: f32,
    o: rough::Opciones,
    azar: &mut Azar,
) -> Vec<Vec<Punto2>> {
    match tramos_de_rombo_redondo(x, y, ancho, alto) {
        Some(t) if o.rugosidad > 0.0 => tramos_a_mano(&t, o, azar),
        Some(t) => vec![contorno_liso(&t)],
        None => rombo_a_mano(x, y, ancho, alto, o, azar),
    }
}

/// **La ruta redondeada a mano, como la traza Excalidraw** (`generator.path`
/// de rough.js con `preserveVertices`, que es lo que pide para las rutas
/// continuas): cada recta con sus dos pasadas de [`rough::Rough::doble_linea`]
/// y cada esquina con dos de [`rough::Rough::cubica_a`], las dos con los
/// extremos quietos, asi que lados y esquinas empalman.
///
/// El movil traza estas rutas de otra manera (`roughContinuous`: una
/// Catmull-Rom por el contorno muestreado y sacudido punto a punto), y se
/// probo aqui primero: en pantalla los lados salian ondulados, como un
/// contorno abollado, peor que lo que habia. La de Excalidraw es la que el
/// usuario tiene por referencia y la que se ve lisa.
fn tramos_a_mano(tramos: &[Tramo], o: rough::Opciones, azar: &mut Azar) -> Vec<Vec<Punto2>> {
    let o = rough::Opciones {
        preservar: true,
        ..o
    };
    let pt = |p: Punto2| rough::Pt::nuevo(p.x as f64, p.y as f64);
    let mut r = rough::Rough::con_azar(o, azar.clone());
    let mut ops = Vec::new();
    for t in tramos {
        match *t {
            Tramo::Recta(a, b) => ops.extend(r.doble_linea(pt(a), pt(b))),
            Tramo::Curva(a, c1, c2, b) => ops.extend(r.cubica_a(pt(a), pt(c1), pt(c2), pt(b))),
        }
    }
    *azar = r.azar();
    rough::a_pasadas(&ops)
}

/// **El rectangulo a mano, el de rough.js** (`rectangle`): cada lado con
/// sus dos pasadas de cubica con panza, y con los vertices quietos por debajo
/// de la rugosidad de dibujante, que es lo que cierra las esquinas. Sin
/// rugosidad, los cuatro lados exactos en una sola pasada cerrada.
pub fn rectangulo_a_mano(
    x: f32,
    y: f32,
    ancho: f32,
    alto: f32,
    o: rough::Opciones,
    azar: &mut Azar,
) -> Vec<Vec<Punto2>> {
    if o.rugosidad <= 0.0 {
        return vec![vec![
            Punto2::nuevo(x, y),
            Punto2::nuevo(x + ancho, y),
            Punto2::nuevo(x + ancho, y + alto),
            Punto2::nuevo(x, y + alto),
            Punto2::nuevo(x, y),
        ]];
    }
    let mut r = rough::Rough::con_azar(o, azar.clone());
    let ops = r.rectangulo(x as f64, y as f64, ancho as f64, alto as f64);
    *azar = r.azar();
    rough::a_pasadas(&ops)
}

/// El rombo a mano: el `polygon` de rough.js por sus cuatro vertices.
pub fn rombo_a_mano(
    x: f32,
    y: f32,
    ancho: f32,
    alto: f32,
    o: rough::Opciones,
    azar: &mut Azar,
) -> Vec<Vec<Punto2>> {
    let v = vertices_de_rombo(x, y, ancho, alto);
    if o.rugosidad <= 0.0 {
        return vec![vec![v[0], v[1], v[2], v[3], v[0]]];
    }
    let pts = v.map(|p| rough::Pt::nuevo(p.x as f64, p.y as f64));
    let mut r = rough::Rough::con_azar(o, azar.clone());
    let ops = r.poligono(&pts);
    *azar = r.azar();
    rough::a_pasadas(&ops)
}

/// **La elipse a mano, la de rough.js** (`ellipse` con `curveFitting` 1,
/// como la pide Excalidraw): una curva lisa por puntos apenas sacudidos (1 y
/// 1,5 px por la rugosidad), dos vueltas, y la primera se pasa un poco del
/// cierre. Devuelve las pasadas y el poligono del nucleo, que es por donde
/// el movil rellena.
pub fn elipse_a_mano(
    x: f32,
    y: f32,
    ancho: f32,
    alto: f32,
    o: rough::Opciones,
    azar: &mut Azar,
) -> (Vec<Vec<Punto2>>, Vec<Punto2>) {
    let o = rough::Opciones {
        ajuste_de_curva: 1.0,
        ..o
    };
    let mut r = rough::Rough::con_azar(o, azar.clone());
    let (ops, nucleo) = r.elipse(
        (x + ancho / 2.0) as f64,
        (y + alto / 2.0) as f64,
        ancho as f64,
        alto as f64,
    );
    *azar = r.azar();
    (
        rough::a_pasadas(&ops),
        nucleo
            .iter()
            .map(|p| Punto2::nuevo(p.x as f32, p.y as f32))
            .collect(),
    )
}

/// El radio de una esquina redondeada para un lado de `corto`: el
/// `getCornerRadius` de Excalidraw con `ADAPTIVE_RADIUS`, que es el que
/// escribe hoy (`{"type":3}`): proporcional (`DEFAULT_PROPORTIONAL_RADIUS`,
/// 0,25) en formas pequenas y fijo (`DEFAULT_ADAPTIVE_RADIUS`, 32) a partir
/// de 128, o un rectangulo enorme quedaria con unas curvas desmedidas.
pub fn radio_de_esquina(corto: f32) -> f32 {
    const PROPORCIONAL: f32 = 0.25;
    const FIJO: f32 = 32.0;
    let corte = FIJO / PROPORCIONAL;
    if corto <= corte {
        (corto * PROPORCIONAL).max(0.0)
    } else {
        FIJO
    }
}

/// Un rectangulo de esquinas redondeadas, liso y cerrado: el contorno del
/// relleno y el trazo sin temblor.
///
/// Los recuadros redondeados sin temblor que manda el movil son las «zonas»
/// que enlazan con otra hoja, y ahi la forma tiene que coincidir exactamente
/// con la suya o parece otro recuadro dibujado encima: por eso es la ruta de
/// Excalidraw tal cual y en una sola pasada.
pub fn rectangulo_redondo(x: f32, y: f32, ancho: f32, alto: f32) -> Vec<Punto2> {
    match tramos_de_rectangulo_redondo(x, y, ancho, alto) {
        Some(t) => contorno_liso(&t),
        None => vec![
            Punto2::nuevo(x, y),
            Punto2::nuevo(x + ancho, y),
            Punto2::nuevo(x + ancho, y + alto),
            Punto2::nuevo(x, y + alto),
            Punto2::nuevo(x, y),
        ],
    }
}

/// El trazo de un rectangulo redondeado con cualquier rugosidad. Antes solo
/// se redondeaba con rugosidad 0, asi que «Bordes: redondo» con el trazo «a
/// mano» de fabrica no cambiaba nada de lo que se ve.
pub fn rectangulo_redondo_a_mano(
    x: f32,
    y: f32,
    ancho: f32,
    alto: f32,
    o: rough::Opciones,
    azar: &mut Azar,
) -> Vec<Vec<Punto2>> {
    match tramos_de_rectangulo_redondo(x, y, ancho, alto) {
        Some(t) if o.rugosidad > 0.0 => tramos_a_mano(&t, o, azar),
        Some(t) => vec![contorno_liso(&t)],
        None => rectangulo_a_mano(x, y, ancho, alto, o, azar),
    }
}

#[cfg(test)]
mod raya_de_rough {
    use super::*;
    use crate::vector::distancia_a_segmento;

    fn peor_desvio(pasadas: &[Vec<Punto2>], a: Punto2, b: Punto2) -> f32 {
        pasadas
            .iter()
            .flatten()
            .map(|p| distancia_a_segmento(*p, a, b))
            .fold(0.0, f32::max)
    }

    #[test]
    fn una_linea_larga_sale_casi_recta_como_en_el_movil() {
        // La del usuario: 1.215 px de alto. Con `ganancia` 0,4 la panza no
        // pasa de 0,4 * 2 * 1215 / 200 = 4,9 px, y los extremos no se mueven.
        let (a, b) = (Punto2::nuevo(760.0, -27.0), Punto2::nuevo(763.0, 1188.0));
        for semilla in [1, 7, 950_731_993, 123_456] {
            let pasadas = linea_rough(a, b, 1.0, true, &mut Azar::nuevo(semilla));
            assert_eq!(pasadas.len(), 2, "dos pasadas, como rough.js");
            assert!(
                peor_desvio(&pasadas, a, b) < 5.0,
                "semilla {semilla}: {}",
                peor_desvio(&pasadas, a, b)
            );
            // Y la de antes, con la misma semilla, se torcia mas.
            let antes = linea(a, b, 1.0, &mut Azar::nuevo(semilla));
            assert!(
                peor_desvio(&antes, a, b) > peor_desvio(&pasadas, a, b),
                "semilla {semilla}"
            );
        }
    }

    #[test]
    fn preservar_los_vertices_deja_los_extremos_donde_se_soltaron() {
        let (a, b) = (Punto2::nuevo(0.0, 0.0), Punto2::nuevo(300.0, 40.0));
        for p in linea_rough(a, b, 1.0, true, &mut Azar::nuevo(5)) {
            assert_eq!((p[0], *p.last().unwrap()), (a, b));
        }
        // Sin preservar (dibujante), los extremos si tiemblan.
        let sueltas = linea_rough(a, b, 2.0, false, &mut Azar::nuevo(5));
        assert!(sueltas.iter().any(|p| p[0] != a));
    }

    #[test]
    fn una_linea_corta_si_tiembla_y_con_rugosidad_cero_es_exacta() {
        let (a, b) = (Punto2::nuevo(0.0, 0.0), Punto2::nuevo(120.0, 0.0));
        let corta = linea_rough(a, b, 1.0, true, &mut Azar::nuevo(3));
        assert!(peor_desvio(&corta, a, b) > 0.05, "a mano, no a regla");
        assert_eq!(
            linea_rough(a, b, 0.0, true, &mut Azar::nuevo(3)),
            vec![vec![a, b]]
        );
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn caja(trazos: &[Vec<Punto2>]) -> (f32, f32, f32, f32) {
        let todos: Vec<Punto2> = trazos.iter().flatten().copied().collect();
        (
            todos.iter().map(|p| p.x).fold(f32::MAX, f32::min),
            todos.iter().map(|p| p.y).fold(f32::MAX, f32::min),
            todos.iter().map(|p| p.x).fold(f32::MIN, f32::max),
            todos.iter().map(|p| p.y).fold(f32::MIN, f32::max),
        )
    }

    #[test]
    fn sin_rugosidad_la_linea_es_exactamente_recta() {
        // El modo regla: dos puntos, los pedidos, sin un pixel de desvio.
        let mut a = Azar::nuevo(1);
        let t = linea(
            Punto2::nuevo(0.0, 0.0),
            Punto2::nuevo(100.0, 0.0),
            0.0,
            &mut a,
        );
        assert_eq!(t.len(), 1, "sin rugosidad basta una pasada");
        assert_eq!(
            t[0],
            vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0)]
        );
    }

    #[test]
    fn con_rugosidad_hay_dos_pasadas_y_ninguna_es_recta() {
        let mut a = Azar::nuevo(42);
        let t = linea(
            Punto2::nuevo(0.0, 50.0),
            Punto2::nuevo(200.0, 50.0),
            1.0,
            &mut a,
        );
        assert_eq!(t.len(), 2, "una mano repasa la linea");
        for pasada in &t {
            let torcida = pasada.iter().any(|p| (p.y - 50.0).abs() > 0.3);
            assert!(torcida, "la pasada salio recta: no parece dibujada");
        }
        // Y las dos pasadas son DISTINTAS: si fueran iguales se veria una
        // sola linea y el efecto se pierde.
        assert_ne!(t[0], t[1]);
    }

    #[test]
    fn la_misma_semilla_dibuja_exactamente_la_misma_forma() {
        // La puerta de la fase: reabrir el documento no puede cambiar nada.
        let hacer = || {
            let mut a = Azar::nuevo(777);
            rectangulo(10.0, 20.0, 100.0, 60.0, 1.0, &mut a)
        };
        assert_eq!(hacer(), hacer());
    }

    #[test]
    fn semillas_distintas_dibujan_formas_distintas() {
        // Caso negativo: si la semilla no influyera, todos los rectangulos
        // del documento saldrian calcados y el efecto "a mano" desapareceria.
        let hacer = |s| {
            let mut a = Azar::nuevo(s);
            rectangulo(10.0, 20.0, 100.0, 60.0, 1.0, &mut a)
        };
        assert_ne!(hacer(777), hacer(778));
    }

    #[test]
    fn el_rectangulo_no_se_aleja_de_su_caja() {
        // Puede temblar, pero no puede irse: si el desvio creciera sin tope,
        // un rectangulo grande se saldria del elemento y del pin.
        let mut a = Azar::nuevo(5);
        let t = rectangulo(100.0, 100.0, 400.0, 300.0, 2.0, &mut a);
        let (x0, y0, x1, y1) = caja(&t);
        assert!(
            x0 > 100.0 - 20.0 && y0 > 100.0 - 20.0,
            "se sale por arriba: {x0},{y0}"
        );
        assert!(
            x1 < 500.0 + 20.0 && y1 < 400.0 + 20.0,
            "se sale por abajo: {x1},{y1}"
        );
    }

    #[test]
    fn la_elipse_cubre_su_caja_por_los_cuatro_lados() {
        let mut a = Azar::nuevo(9);
        let t = elipse(0.0, 0.0, 200.0, 100.0, 1.0, &mut a);
        let (x0, y0, x1, y1) = caja(&t);
        assert!(x0 < 10.0 && x1 > 190.0, "no llega a los lados: {x0}..{x1}");
        assert!(
            y0 < 10.0 && y1 > 90.0,
            "no llega arriba y abajo: {y0}..{y1}"
        );
    }

    /// El giro mas brusco, en grados, entre dos tramos seguidos de un trazo.
    /// Un circulo liso gira poco en cada punto; una quebrada, de golpe.
    fn peor_giro(v: &[Punto2]) -> f32 {
        v.windows(3)
            .filter(|w| w[0].distancia(w[1]) > 1e-3 && w[1].distancia(w[2]) > 1e-3)
            .map(|w| {
                let (ax, ay) = (w[1].x - w[0].x, w[1].y - w[0].y);
                let (bx, by) = (w[2].x - w[1].x, w[2].y - w[1].y);
                let c = (ax * bx + ay * by) / ((ax.hypot(ay)) * (bx.hypot(by)));
                c.clamp(-1.0, 1.0).acos().to_degrees()
            })
            .fold(0.0, f32::max)
    }

    #[test]
    fn un_circulo_con_trazo_a_mano_medio_o_alto_no_tiene_esquinas() {
        // Con vertices cada ~20 unidades unidos con rectas, el giro de un
        // tramo al siguiente era de 35-45 grados en el trazo medio y de 64-76
        // en el alto: esquinas. Alisado, el medio no pasa de 15 y el sin
        // temblor de 2. El alto sigue ondulando (su temblor es de hasta 8
        // unidades por vertice, y eso es su aspecto), pero ya sin picos.
        for (rugosidad, tope) in [(0.0, 2.0), (1.0, 16.0), (2.0, f32::MAX)] {
            for semilla in [1, 4, 9, 1234] {
                let lisa = elipse(0.0, 0.0, 200.0, 200.0, rugosidad, &mut Azar::nuevo(semilla));
                let antes = vertices_de_elipse(
                    0.0,
                    0.0,
                    200.0,
                    200.0,
                    rugosidad,
                    &mut Azar::nuevo(semilla),
                );
                for (liso, quebrado) in lisa.iter().zip(&antes) {
                    let (g, g0) = (peor_giro(liso), peor_giro(quebrado));
                    assert!(
                        g < tope,
                        "r{rugosidad} s{semilla}: gira {g} grados de golpe"
                    );
                    assert!(
                        g < 0.85 * g0 || g < 2.0,
                        "r{rugosidad} s{semilla}: {g} no es mas liso que {g0}"
                    );
                }
            }
        }
        // Caso negativo: la quebrada de antes si hacia pico, o la prueba no
        // distingue nada.
        let antes = vertices_de_elipse(0.0, 0.0, 200.0, 200.0, 1.0, &mut Azar::nuevo(4));
        assert!(peor_giro(&antes[0]) > 30.0, "{}", peor_giro(&antes[0]));
    }

    #[test]
    fn el_circulo_liso_pasa_por_los_mismos_vertices_que_antes() {
        // Misma semilla, mismo temblor: lo que se guardo se sigue viendo en
        // el mismo sitio y con la misma mano; solo cambian las uniones.
        for rugosidad in [0.0, 1.0, 2.0] {
            let v = vertices_de_elipse(10.0, 20.0, 180.0, 90.0, rugosidad, &mut Azar::nuevo(77));
            let lisa = elipse(10.0, 20.0, 180.0, 90.0, rugosidad, &mut Azar::nuevo(77));
            assert_eq!(v.len(), lisa.len());
            for (vuelta, liso) in v.iter().zip(&lisa) {
                for p in vuelta {
                    assert!(
                        liso.iter().any(|q| q.distancia(*p) < 1e-3),
                        "r{rugosidad}: el vertice {p:?} ya no esta en la curva"
                    );
                }
            }
        }
    }

    /// Una flecha recta de cien pixeles hacia la derecha.
    fn recta() -> Vec<Punto2> {
        vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0)]
    }

    #[test]
    fn las_ocho_puntas_llevan_las_palabras_del_fichero_una_por_una() {
        // Si una sola deja de coincidir, un diagrama entidad-relacion del
        // movil se abre aqui con otra cardinalidad: la punta DICE algo.
        let esperadas = [
            "arrow",
            "bar",
            "circle",
            "circle_outline",
            "triangle",
            "triangle_outline",
            "diamond",
            "diamond_outline",
        ];
        let nuestras: Vec<&str> = PUNTAS.iter().filter_map(|p| p.palabra()).collect();
        assert_eq!(nuestras, esperadas);
        for p in PUNTAS {
            assert_eq!(TipoPunta::desde_palabra(p.palabra().unwrap()), Some(p));
        }
    }

    #[test]
    fn sin_punta_no_hay_palabra_y_una_palabra_rara_no_se_lee_como_sin_punta() {
        // Caso negativo, y el que protege el fichero: leer una punta de una
        // version futura como «ninguna» la borraria al guardar.
        assert_eq!(TipoPunta::Ninguna.palabra(), None);
        assert_eq!(TipoPunta::desde_palabra("crowfoot-del-futuro"), None);
        assert_eq!(TipoPunta::desde_palabra(""), None);
    }

    #[test]
    fn solo_las_tres_macizas_se_rellenan() {
        // Es lo que distingue en un diagrama la herencia de la composicion:
        // el mismo triangulo, uno relleno y otro hueco.
        for p in [TipoPunta::Circulo, TipoPunta::Triangulo, TipoPunta::Rombo] {
            assert!(p.es_maciza(), "{p:?}");
        }
        for p in [
            TipoPunta::Flecha,
            TipoPunta::Barra,
            TipoPunta::CirculoHueco,
            TipoPunta::TrianguloHueco,
            TipoPunta::RomboHueco,
        ] {
            assert!(!p.es_maciza(), "{p:?}");
        }
    }

    #[test]
    fn cada_punta_se_posa_en_el_extremo_y_ninguna_se_pasa_de_largo() {
        for tipo in PUNTAS {
            let f = forma_de_punta(&recta(), true, tipo, 2.0).expect("{tipo:?}");
            assert_eq!(f.punta, Punto2::nuevo(100.0, 0.0), "{tipo:?}");
            for p in contorno_de_punta(&f) {
                assert!(p.x <= 100.01 + f.diametro / 2.0, "{tipo:?} se pasa: {p:?}");
            }
        }
    }

    #[test]
    fn la_punta_del_principio_apunta_al_reves_que_la_del_final() {
        // Caso negativo del extremo: si las dos miraran al mismo lado, una
        // flecha de doble punta saldria con las dos en el mismo sentido.
        let fin = forma_de_punta(&recta(), true, TipoPunta::Flecha, 2.0).unwrap();
        let ini = forma_de_punta(&recta(), false, TipoPunta::Flecha, 2.0).unwrap();
        assert!(fin.alas.0.x < fin.punta.x, "la del final no mira atras");
        assert!(ini.alas.0.x > ini.punta.x, "la del principio no mira atras");
    }

    #[test]
    fn la_barra_sale_perpendicular_al_trazo_y_la_flecha_no() {
        // Los noventa grados de apertura: es lo unico que separa una barra de
        // una punta abierta.
        let barra = forma_de_punta(&recta(), true, TipoPunta::Barra, 2.0).unwrap();
        assert!(
            (barra.alas.0.x - barra.punta.x).abs() < 0.01
                && (barra.alas.1.x - barra.punta.x).abs() < 0.01,
            "la barra no es perpendicular: {barra:?}"
        );
        let flecha = forma_de_punta(&recta(), true, TipoPunta::Flecha, 2.0).unwrap();
        assert!((flecha.alas.0.x - flecha.punta.x).abs() > 1.0);
    }

    #[test]
    fn el_rombo_lleva_su_cuarto_vertice_y_las_demas_puntas_no() {
        let rombo = forma_de_punta(&recta(), true, TipoPunta::Rombo, 2.0).unwrap();
        let opuesto = rombo.opuesto.expect("el rombo necesita cuatro vertices");
        assert!(opuesto.x < rombo.punta.x, "el cuarto vertice va detras");
        assert!(
            forma_de_punta(&recta(), true, TipoPunta::Flecha, 2.0)
                .unwrap()
                .opuesto
                .is_none()
        );
        assert_eq!(contorno_de_punta(&rombo).len(), 5, "rombo cerrado");
    }

    #[test]
    fn en_un_trazo_a_pulso_la_punta_mira_hacia_donde_iba_la_mano() {
        // El fallo que esto viene a impedir: con los puntos a un pixel unos de
        // otros, mirando solo al anterior la punta salia del tamano de ese
        // pixel —invisible— y temblorosa. Es lo que hace usable la flecha a
        // mano alzada, que por dentro es una flecha con todos sus puntos.
        let mut pulso: Vec<Punto2> = (0..200)
            .map(|i| Punto2::nuevo(i as f32 * 0.5, (i as f32 * 0.3).sin()))
            .collect();
        pulso.push(Punto2::nuevo(100.0, 0.0));
        let f = forma_de_punta(&pulso, true, TipoPunta::Flecha, 2.0).unwrap();
        let ala = f.punta.distancia(f.alas.0);
        assert!(ala > 5.0, "la punta salio del tamano de un pixel: {ala}");
    }

    #[test]
    fn una_flecha_corta_no_se_queda_en_pura_punta() {
        // El acotado a la mitad del trazo: una punta de 25 px en una flecha de
        // 20 es toda la flecha y se ve como un borron.
        let corta = vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(20.0, 0.0)];
        let f = forma_de_punta(&corta, true, TipoPunta::Flecha, 2.0).unwrap();
        assert!(f.punta.distancia(f.alas.0) <= 12.0, "{f:?}");
    }

    #[test]
    fn sin_punta_o_sin_dos_puntos_no_hay_nada_que_dibujar() {
        assert!(forma_de_punta(&recta(), true, TipoPunta::Ninguna, 2.0).is_none());
        assert!(forma_de_punta(&[Punto2::nuevo(0.0, 0.0)], true, TipoPunta::Flecha, 2.0).is_none());
        // Caso negativo del cero: los dos extremos encima no tienen direccion,
        // y normalizarla daria NaN, que borra la geometria entera.
        let pegados = vec![Punto2::nuevo(5.0, 5.0), Punto2::nuevo(5.0, 5.0)];
        assert!(forma_de_punta(&pegados, true, TipoPunta::Flecha, 2.0).is_none());
    }

    #[test]
    fn el_rombo_redondo_no_se_sale_de_su_caja_y_ya_no_tiene_picos() {
        let r = rombo_redondo(0.0, 0.0, 200.0, 100.0);
        assert!(r.len() > 4, "no se redondeo nada");
        for p in &r {
            assert!((-0.01..=200.01).contains(&p.x), "{p:?}");
            assert!((-0.01..=100.01).contains(&p.y), "{p:?}");
        }
        // Y los cuatro vertices en pico ya no estan: si siguieran, seria el
        // mismo rombo de antes con mas puntos.
        for v in vertices_de_rombo(0.0, 0.0, 200.0, 100.0) {
            assert!(
                !r.iter().any(|p| p.distancia(v) < 1e-4),
                "la punta {v:?} sigue en pico"
            );
        }
    }

    #[test]
    fn un_rombo_aplastado_se_redondea_distinto_por_cada_eje() {
        // Con un radio unico las puntas laterales quedarian romas y las de
        // arriba en pico. Con 400x40 el radio horizontal se topa en el fijo y
        // el vertical se queda proporcional.
        assert_eq!(radio_de_esquina(200.0), 32.0, "el radio se topa");
        assert_eq!(radio_de_esquina(20.0), 5.0, "y por debajo es proporcional");
        let r = rombo_redondo(0.0, 0.0, 400.0, 40.0);
        for p in &r {
            assert!((-0.01..=400.01).contains(&p.x) && (-0.01..=40.01).contains(&p.y));
        }
    }

    #[test]
    fn un_rombo_sin_tamano_devuelve_sus_cuatro_vertices_y_no_entra_en_panico() {
        let r = rombo_redondo(10.0, 10.0, 0.0, 0.0);
        assert_eq!(r.len(), 4);
        for p in &r {
            assert!(p.x.is_finite() && p.y.is_finite());
        }
    }

    #[test]
    fn una_forma_sin_tamano_no_entra_en_panico() {
        let mut a = Azar::nuevo(1);
        let _ = rectangulo(10.0, 10.0, 0.0, 0.0, 1.0, &mut a);
        let e = elipse(10.0, 10.0, 0.0, 0.0, 1.0, &mut a);
        for p in e.iter().flatten() {
            assert!(p.x.is_finite() && p.y.is_finite());
        }
    }
}
