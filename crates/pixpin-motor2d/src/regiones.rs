//! **El bote de pintura**: rellenar el hueco que dejan varias figuras.
//!
//! Port de `Regiones.kt` (763 lineas) del PixPin de Android.
//!
//! Hasta aqui un relleno era siempre **de una figura** —el fondo de un
//! rectangulo, el de un lazo cerrado— y eso deja fuera lo que mas se quiere
//! pintar: el hueco entre tres rayas y media elipse no es de ninguna de las
//! cuatro. Repasarlo a mano con el lapiz para poder rellenar ese repaso es
//! tanto trabajo como no tener la herramienta.
//!
//! Aqui se hace al reves: se toca **dentro** del hueco y se busca hasta donde
//! llega sin salirse. Si esta encerrado, sale su contorno con sus agujeros; si
//! se escapa por una rendija, no sale nada — y eso **es una respuesta, no un
//! fallo**: quiere decir que el recinto esta abierto. Es la prueba que de
//! verdad importa de esta herramienta, y esta abajo con ese nombre.
//!
//! # Por que por rejilla y no por geometria
//!
//! Lo «de libro» seria construir el grafo de todas las intersecciones entre
//! figuras y buscar la cara minima que contiene el punto. Es exacto y es una
//! fuente inagotable de casos degenerados: tres rectas que se cortan en el
//! mismo punto, un trazo tangente a un circulo, un garabato con doscientos
//! cruces consigo mismo. En un motor de dibujo a mano alzada cada uno de esos
//! casos es **lo normal**, no la excepcion.
//!
//! Pintar las paredes en una rejilla y derramar desde el punto tocado no tiene
//! casos degenerados: una tangente es una pared, un cruce triple es una pared,
//! y el resultado depende solo de por donde se puede pasar. A cambio se pierde
//! precision, y esa perdida **es justo la tolerancia que hace falta**: un hueco
//! mas estrecho que una celda se da por cerrado. Con raton, igual que con el
//! dedo, dos trazos que «se tocan» casi nunca se tocan de verdad; sin
//! tolerancia el bote no funcionaria casi nunca y nadie sabria por que.
//!
//! **Pero la rejilla ya no va sola** (F2, v0.76 del movil): delante de ella
//! se intenta la cara exacta de [`crate::cara_exacta`], que cose los cabos a
//! menos de dos unidades y, si el recinto cierra, devuelve un borde que esta
//! sobre los trazos y no a una celda de ellos. Solo cuando no cierra —una
//! rendija de verdad, o demasiados tramos— decide la rejilla.
//!
//! # De donde salen las paredes
//!
//! De `perimetros::segmentos_de`, que es el cimiento que comparte con el iman
//! y con el recorte. Aqui no se sabe como es por dentro una elipse ni un
//! rombo: se piden tramos rectos y se pintan.

use std::collections::HashMap;

use crate::elemento::{ColorRgba, Elemento, Figura};
use crate::perimetros::{PASO_PERIMETRO, proyeccion_en_segmento, segmentos_de};
use crate::vector::{Punto2, distancia_a_segmento};

/// Un espacio cerrado, ya encontrado. Los huecos son sus agujeros.
///
/// Los puntos van en coordenadas del documento, como todo lo de este motor;
/// quien monte el elemento los deja tal cual (ver [`nueva_region`]).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Region {
    pub contorno: Vec<Punto2>,
    pub huecos: Vec<Vec<Punto2>>,
}

/// Como de fino se busca. Los valores de fabrica son los del movil.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AjustesRelleno {
    /// En cuantas celdas se parte el lado mayor de la zona de trabajo.
    ///
    /// Es el mando que reparte precision y tiempo, y **tambien el que decide
    /// cuanto se sale la mancha**: el relleno crece una celda hacia el trazo
    /// para llegar hasta el (ver `dilatacion`), asi que puede asomar hasta una
    /// celda por la cara de fuera. Con celdas gordas eso es un reborde de
    /// color alrededor de la figura.
    ///
    /// El movil lo subio de 320 a 480 por eso mismo: en un dibujo de 2000 px
    /// de ancho la celda pasa de 6 a 4 px, y lo que asoma queda por debajo del
    /// grosor de un trazo normal, tapado por el. El coste sube con el
    /// cuadrado, y 480 son unas 230.000 celdas: milisegundos, y solo al tocar.
    pub celdas: u32,
    /// Cuanto se aparta el borde de la zona de trabajo del dibujo, en celdas.
    ///
    /// Tiene que ser al menos una: el borde **es** lo que detecta que el
    /// recinto no estaba cerrado —el derrame lo alcanza y se sabe que se ha
    /// escapado— y pegado al dibujo se confundiria con una pared.
    pub margen: u32,
    /// Cuantas celdas crece la mancha hacia las paredes al terminar.
    ///
    /// El derrame se para **antes** de la celda de la pared, asi que sin esto
    /// el relleno acabaria una celda antes del trazo y se veria una raya de
    /// fondo entre la mancha y la linea. Mas de una empezaria a colarse al
    /// otro lado de las paredes de un solo trazo.
    pub dilatacion: u32,
}

impl Default for AjustesRelleno {
    fn default() -> Self {
        Self {
            celdas: 480,
            margen: 3,
            dilatacion: 1,
        }
    }
}

const LIBRE: u8 = 0;
const PARED: u8 = 1;
const MANCHA: u8 = 2;

/// Hasta donde se busca sitio libre al tocar justo encima de un trazo.
const BUSQUEDA_LIBRE: i64 = 3;

/// A cuantas celdas de distancia se le permite a un vertice buscar su pared.
///
/// Poco mas de una: el derrame se para a una celda de la pared y la dilatacion
/// lo acerca otra, asi que la distancia real es menos de dos. Mas margen
/// empezaria a pegar a la pared vertices que no eran del borde —los de un
/// recodo hacia dentro de la mancha— y eso deforma el relleno en vez de
/// ajustarlo.
const ARRIME: f32 = 1.6;

/// Dos puntos mas cerca que esto son el mismo punto.
const JUNTOS: f32 = 0.01;

/// **El espacio cerrado que contiene `p`, o `None` si no hay ninguno.**
///
/// `None` significa una de tres, y las tres quieren decir lo mismo para quien
/// dibuja —«esto no esta cerrado»—: que el punto caiga fuera de todo, que el
/// derrame llegue al borde de la zona de trabajo, o que se haya tocado justo
/// encima de un trazo y no quede sitio libre al lado.
pub fn region_en(elementos: &[Elemento], p: Punto2, ajustes: &AjustesRelleno) -> Option<Region> {
    let segmentos: Vec<(Punto2, Punto2)> = elementos
        .iter()
        .filter(|e| !e.borrado && es_pared(e))
        .flat_map(|e| segmentos_de(e, PASO_PERIMETRO))
        .collect();
    if segmentos.is_empty() {
        return None;
    }

    // **Primero, exacto** (F2, `CaraExacta.kt`): la cara del plano que forman
    // los propios tramos, con las esquinas donde se cruzan. La rejilla se
    // abombaba en las curvas y se comia los picos del hueco entre figuras; se
    // queda para lo que no cierra del todo —rendijas de mas de un par de
    // unidades—, que es justo la tolerancia de la que habla la cabecera.
    if let Some(r) =
        crate::cara_exacta::cara_exacta(&segmentos, p, crate::cara_exacta::TOLERANCIA_DE_CARA)
    {
        return Some(r);
    }

    let mut rejilla = Rejilla::para(&segmentos, p, ajustes)?;
    for (a, b) in &segmentos {
        rejilla.pintar_pared(*a, *b);
    }

    if !rejilla.derramar(p) {
        return None;
    }
    for _ in 0..ajustes.dilatacion {
        rejilla.dilatar();
    }

    // **Primero se pegan a las paredes, despues se simplifican.**
    //
    // El contorno que sale del derrame es una escalera: sigue los bordes de
    // las celdas, asi que donde la pared va en diagonal la mancha asoma por un
    // lado y se queda corta por el otro. No se arregla subiendo la resolucion:
    // con celdas mas finas la escalera es mas fina, pero sigue siendo una
    // escalera, y cada duplicar cuesta cuatro veces mas.
    //
    // Pegando cada vertice a la recta que tiene al lado, el contorno deja de
    // aproximar la pared y pasa a **estar sobre ella**. El orden importa: si
    // se simplificara antes, la simplificacion elegiria vertices de la
    // escalera y se pegarian los que quedaran, no los que habia que pegar.
    let celda = rejilla.celda;
    let anillos: Vec<Vec<Punto2>> = rejilla
        .contornos()
        .into_iter()
        .map(|a| simplificar(&pegado_a_las_paredes(&a, &segmentos, celda * ARRIME), celda))
        .filter(|a| a.len() >= 3)
        .collect();
    let fuera = anillos
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| {
            area_de(a)
                .abs()
                .partial_cmp(&area_de(b).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)?;

    // El de fuera es el de mayor superficie: los demas son sus agujeros. Con
    // la mancha ya derramada no puede haber dos «de fuera», asi que no hace
    // falta mirar quien esta dentro de quien.
    let mut huecos = Vec::new();
    for (i, a) in anillos.iter().enumerate() {
        if i != fuera {
            huecos.push(a.clone());
        }
    }
    Some(Region {
        contorno: anillos[fuera].clone(),
        huecos,
    })
}

/// ¿Este elemento hace de pared?
///
/// La regla es «lo que dibuja una linea encierra; lo que es un fondo, no».
///
/// - **La imagen y la hoja** se quedan fuera porque son el soporte: contando
///   la foto sobre la que se anota, *todo* estaria siempre cerrado —la propia
///   foto seria el contorno— y tocar en un sitio vacio teñiria la captura
///   entera.
/// - **El foco y el mosaico** son manchas, no bordes.
/// - **El texto** tampoco: su caja no se dibuja, y chocar contra un rectangulo
///   invisible es de las cosas que mas desconciertan. Lo mismo el emoji.
/// - **El punto etiquetado** es una marca sobre el dibujo, no encierra nada, y
///   un redondel de cinco pixeles no puede detener un derrame.
/// - **La escala grafica** es una lamina con datos: contar su marco como pared
///   dejaria cada cuadro suyo como un hueco que rellenar.
///
/// Y la que mas costo descubrir en el movil: **un relleno no es pared**. Al
/// principio lo era, y el bote funcionaba de maravilla las primeras veces y
/// luego empezaba a rellenar a trozos. El motivo: el contorno de un relleno no
/// cae exactamente sobre los trazos que lo encerraban —sale de una rejilla,
/// con su celda de margen— asi que cada mancha dejaba en la escena una pared
/// nueva ligeramente desplazada de las de verdad. La siguiente chocaba contra
/// ella y se quedaba corta, y la siguiente contra las dos.
pub fn es_pared(e: &Elemento) -> bool {
    match &e.figura {
        Figura::Imagen { .. }
        | Figura::Marco { .. }
        | Figura::Foco { .. }
        | Figura::Lupa { .. }
        | Figura::Mosaico { .. }
        | Figura::Texto { .. }
        | Figura::Emoji { .. }
        | Figura::EscalaGrafica
        | Figura::Instrumento(_)
        | Figura::Punto { .. }
        | Figura::Region { .. } => false,

        Figura::Rectangulo
        | Figura::Rombo
        | Figura::Elipse
        | Figura::Arco { .. }
        | Figura::Linea { .. }
        | Figura::Flecha { .. }
        | Figura::Lapiz { .. }
        | Figura::Resaltador { .. }
        | Figura::Serie { .. }
        | Figura::Cota { .. } => true,
    }
}

/// Un relleno ya colocado, a partir de la region encontrada.
///
/// El color sale del fondo de `plantilla`, y **si no hay fondo se usa el del
/// trazo**: el fondo nace transparente, asi que sin esa salida el primer
/// botellazo de todo el mundo no pintaria nada y pareceria que la herramienta
/// esta rota.
pub fn nueva_region(region: Region, plantilla: &Elemento) -> Elemento {
    let color = match plantilla.relleno {
        Some(c) if c.a > 0.0 => c,
        _ => plantilla.trazo,
    };
    let (x0, y0, x1, y1) = caja_de(&region.contorno);
    Elemento {
        id: 0,
        figura: Figura::Region {
            contorno: region.contorno,
            huecos: region.huecos,
        },
        x: x0,
        y: y0,
        ancho: x1 - x0,
        alto: y1 - y0,
        angulo: 0.0,
        relleno: Some(color),
        ..plantilla.clone()
    }
}

/// Donde meter el relleno en la lista de la escena: **por debajo de lo que lo
/// encierra**.
///
/// Si fuera encima taparia por dentro justo los trazos que forman el hueco, y
/// una linea a la que le comen medio grosor se ve mas fina que sus vecinas: el
/// dibujo queda desigual sin que se sepa por que. Debajo del todo tampoco vale
/// — se esconderia detras de la foto sobre la que se anota, que va al fondo y
/// bloqueada.
///
/// Asi que va justo debajo de **la primera pared que se cruza en su camino**:
/// por encima del soporte y por debajo de todo lo que lo dibuja. Devuelve el
/// indice donde insertarlo (el final de la lista si no topa con ninguna).
pub fn sitio_del_relleno(elementos: &[Elemento], relleno: &Elemento) -> usize {
    let caja = relleno.caja();
    elementos
        .iter()
        .position(|e| !e.borrado && !e.bloqueado && es_pared(e) && se_solapan(caja, e.caja()))
        .unwrap_or(elementos.len())
}

/// **La figura cerrada que se rellena ella sola, exacta** (F3,
/// `figuraQueSeRellenaSola` del movil, v0.67), o `None` si el hueco tocado no
/// es de una sola figura.
///
/// Las figuras son vectoriales y su relleno deberia ser exacto; la rejilla
/// —que esta para los huecos ENTRE varias figuras— no puede serlo. Pero el
/// caso de todos los dias es el facil: se toca dentro de un rectangulo, un
/// rombo o una elipse y dentro no hay nada mas. Ahi el hueco ES la figura, y
/// su propio fondo la rellena con su forma de verdad —curvas, esquinas
/// redondas y giro incluidos—, se mueve y se estira con ella.
///
/// Vale solo si nada mas se mete en la figura: ninguna otra pared la cruza
/// ni tiene un extremo dentro. Si lo hay, se devuelve `None` y decide la
/// rejilla, con sus agujeros. De las que el PC pinta con fondo: rectangulo,
/// rombo y elipse (la mas pequena si hay varias una dentro de otra).
pub fn figura_que_se_rellena_sola(elementos: &[Elemento], p: Punto2) -> Option<u64> {
    let paredes: Vec<&Elemento> = elementos
        .iter()
        .filter(|e| !e.borrado && es_pared(e))
        .collect();
    let (figura, anillo) = paredes
        .iter()
        .filter(|e| {
            !e.bloqueado
                && matches!(
                    e.figura,
                    Figura::Rectangulo | Figura::Rombo | Figura::Elipse
                )
        })
        .filter_map(|e| {
            let anillo = crate::perimetros::contornos_de(e, PASO_PERIMETRO)
                .into_iter()
                .find(|c| c.cerrado && c.puntos.len() >= 3)?
                .puntos;
            cruza_impar(p, &anillo).then_some((*e, anillo))
        })
        .min_by(|a, b| area_de(&a.1).abs().total_cmp(&area_de(&b.1).abs()))?;
    let caja = caja_de(&anillo);
    let n = anillo.len();
    for otra in paredes {
        if otra.id == figura.id || !se_solapan(caja, otra.caja()) {
            continue;
        }
        for (a, b) in segmentos_de(otra, PASO_PERIMETRO) {
            if cruza_impar(a, &anillo) || cruza_impar(b, &anillo) {
                return None;
            }
            let corta = (0..n).any(|i| {
                crate::perimetros::interseccion(a, b, anillo[i], anillo[(i + 1) % n]).is_some()
            });
            if corta {
                return None;
            }
        }
    }
    Some(figura.id)
}

/// Los rellenos que ya habia en ese sitio, para quitarlos antes de poner el
/// nuevo.
///
/// **Un bote no apila capas de pintura**: se vuelve a dar sobre el mismo hueco
/// porque se queria otro color, no dos manchas superpuestas. Sin esto,
/// insistir deja una pila de rellenos identicos que hay que deshacer uno a uno,
/// y el rayado de todos ellos se suma hasta verse como un solido sucio.
pub fn rellenos_en(elementos: &[Elemento], p: Punto2) -> Vec<u64> {
    elementos
        .iter()
        .filter(|e| {
            matches!(e.figura, Figura::Region { .. })
                && !e.borrado
                && !e.bloqueado
                && punto_en_region(e, p)
        })
        .map(|e| e.id)
        .collect()
}

/// ¿El punto cae dentro del relleno, **contando sus agujeros**?
pub fn punto_en_region(e: &Elemento, p: Punto2) -> bool {
    let Figura::Region { contorno, huecos } = &e.figura else {
        return false;
    };
    // Par/impar sobre todos los anillos a la vez: dentro de un agujero se
    // cruzan dos contornos y vuelve a quedar fuera, que es justo lo que se
    // quiere.
    let mut dentro = false;
    for anillo in std::iter::once(contorno).chain(huecos.iter()) {
        if cruza_impar(p, anillo) {
            dentro = !dentro;
        }
    }
    dentro
}

/// Superficie con signo de un anillo. Sirve para saber cual es el de fuera, y
/// es tambien el area que se rotula (en pixeles del documento al cuadrado:
/// pasarla a unidades reales es de `medida`).
pub fn area_de(anillo: &[Punto2]) -> f32 {
    if anillo.len() < 3 {
        return 0.0;
    }
    let mut suma = 0.0;
    let mut j = anillo.len() - 1;
    for i in 0..anillo.len() {
        suma += (anillo[j].x + anillo[i].x) * (anillo[j].y - anillo[i].y);
        j = i;
    }
    suma / 2.0
}

/// El area de un relleno entero: su contorno **menos** sus agujeros.
///
/// Sin restar los agujeros, el area de una arandela seria la del disco, y una
/// cota de superficie que miente es peor que no tenerla.
pub fn area_de_region(e: &Elemento) -> f32 {
    let Figura::Region { contorno, huecos } = &e.figura else {
        return 0.0;
    };
    let fuera = area_de(contorno).abs();
    let dentro: f32 = huecos.iter().map(|h| area_de(h).abs()).sum();
    (fuera - dentro).max(0.0)
}

fn cruza_impar(p: Punto2, poligono: &[Punto2]) -> bool {
    if poligono.len() < 3 {
        return false;
    }
    let mut dentro = false;
    let mut j = poligono.len() - 1;
    for i in 0..poligono.len() {
        let (a, b) = (poligono[i], poligono[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            dentro = !dentro;
        }
        j = i;
    }
    dentro
}

fn caja_de(puntos: &[Punto2]) -> (f32, f32, f32, f32) {
    if puntos.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    (
        puntos.iter().map(|p| p.x).fold(f32::MAX, f32::min),
        puntos.iter().map(|p| p.y).fold(f32::MAX, f32::min),
        puntos.iter().map(|p| p.x).fold(f32::MIN, f32::max),
        puntos.iter().map(|p| p.y).fold(f32::MIN, f32::max),
    )
}

fn se_solapan(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
    a.0 <= b.2 && b.0 <= a.2 && a.1 <= b.3 && b.1 <= a.3
}

// -------------------------------------------------------------------------
// La rejilla
// -------------------------------------------------------------------------

/// La zona de trabajo partida en celdas, con sus paredes y su mancha.
///
/// Las dos van en el mismo array —`PARED`, `LIBRE`, `MANCHA`— y no en dos: se
/// consulta una vez por celda en el bucle mas caliente del algoritmo.
struct Rejilla {
    x1: f32,
    y1: f32,
    celda: f32,
    cols: usize,
    filas: usize,
    estado: Vec<u8>,
}

impl Rejilla {
    /// La rejilla que cubre lo dibujado **y** el punto tocado, o `None` si el
    /// punto se sale de ahi: fuera del dibujo no hay nada que encerrar.
    fn para(
        segmentos: &[(Punto2, Punto2)],
        p: Punto2,
        ajustes: &AjustesRelleno,
    ) -> Option<Rejilla> {
        let (mut min_x, mut min_y) = (f32::MAX, f32::MAX);
        let (mut max_x, mut max_y) = (f32::MIN, f32::MIN);
        for (a, b) in segmentos {
            min_x = min_x.min(a.x).min(b.x);
            max_x = max_x.max(a.x).max(b.x);
            min_y = min_y.min(a.y).min(b.y);
            max_y = max_y.max(a.y).max(b.y);
        }
        if !min_x.is_finite() || !min_y.is_finite() || !max_x.is_finite() || !max_y.is_finite() {
            return None;
        }
        if p.x < min_x || p.x > max_x || p.y < min_y || p.y > max_y {
            return None;
        }

        let (ancho, alto) = (max_x - min_x, max_y - min_y);
        let lado = ancho.max(alto);
        if lado <= 0.0 {
            return None;
        }
        let celdas = ajustes.celdas.clamp(32, 1024) as f32;
        let celda = lado / celdas;
        let margen = ajustes.margen.max(1) as f32;
        let cols = (ancho / celda).ceil() as usize + 1 + (margen as usize) * 2;
        let filas = (alto / celda).ceil() as usize + 1 + (margen as usize) * 2;
        Some(Rejilla {
            x1: min_x - margen * celda,
            y1: min_y - margen * celda,
            celda,
            cols,
            filas,
            estado: vec![LIBRE; cols * filas],
        })
    }

    fn dentro(&self, ix: i64, iy: i64) -> bool {
        ix >= 0 && iy >= 0 && (ix as usize) < self.cols && (iy as usize) < self.filas
    }

    fn indice(&self, ix: i64, iy: i64) -> usize {
        iy as usize * self.cols + ix as usize
    }

    fn columna_de(&self, x: f32) -> i64 {
        ((x - self.x1) / self.celda).floor() as i64
    }

    fn fila_de(&self, y: f32) -> i64 {
        ((y - self.y1) / self.celda).floor() as i64
    }

    fn es_mancha(&self, ix: i64, iy: i64) -> bool {
        self.dentro(ix, iy) && self.estado[self.indice(ix, iy)] == MANCHA
    }

    /// La esquina superior izquierda de la celda, en coordenadas del documento.
    fn esquina(&self, ix: i64, iy: i64) -> Punto2 {
        Punto2::nuevo(
            self.x1 + ix as f32 * self.celda,
            self.y1 + iy as f32 * self.celda,
        )
    }

    /// Pinta el tramo de pared que va de `a` a `b` (Amanatides y Woo).
    ///
    /// **Avanza celda a celda, nunca en diagonal**, y eso no es un detalle de
    /// implementacion: es lo que hace la pared estanca. Marcando solo las
    /// celdas de los extremos —o dando saltos en diagonal— una linea inclinada
    /// deja pasar la mancha por las esquinas, y el relleno se escapa por una
    /// pared que a la vista esta entera.
    fn pintar_pared(&mut self, a: Punto2, b: Punto2) {
        let (mut ix, mut iy) = (self.columna_de(a.x), self.fila_de(a.y));
        let (ix_fin, iy_fin) = (self.columna_de(b.x), self.fila_de(b.y));
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let paso_x: i64 = if dx > 0.0 {
            1
        } else if dx < 0.0 {
            -1
        } else {
            0
        };
        let paso_y: i64 = if dy > 0.0 {
            1
        } else if dy < 0.0 {
            -1
        } else {
            0
        };

        // Cuanto falta —en parametro de la recta— para cruzar el siguiente
        // borde vertical y el siguiente horizontal, y cuanto se tarda de borde
        // a borde.
        let mut t_x = if paso_x == 0 {
            f32::MAX
        } else {
            (self.x1 + (ix + if paso_x > 0 { 1 } else { 0 }) as f32 * self.celda - a.x) / dx
        };
        let mut t_y = if paso_y == 0 {
            f32::MAX
        } else {
            (self.y1 + (iy + if paso_y > 0 { 1 } else { 0 }) as f32 * self.celda - a.y) / dy
        };
        let salto_x = if paso_x == 0 {
            f32::MAX
        } else {
            self.celda / dx.abs()
        };
        let salto_y = if paso_y == 0 {
            f32::MAX
        } else {
            self.celda / dy.abs()
        };

        self.marcar_pared(ix, iy);
        // Cortafuegos: con coordenadas absurdas (infinitos, NaN) el recorrido
        // no termina solo, y esto se llama con lo que haya en la escena.
        let tope = self.cols + self.filas + 4;
        let mut vueltas = 0;
        while (ix != ix_fin || iy != iy_fin) && vueltas < tope {
            vueltas += 1;
            if t_x < t_y {
                ix += paso_x;
                t_x += salto_x;
            } else {
                iy += paso_y;
                t_y += salto_y;
            }
            self.marcar_pared(ix, iy);
        }
    }

    fn marcar_pared(&mut self, ix: i64, iy: i64) {
        if self.dentro(ix, iy) {
            let i = self.indice(ix, iy);
            self.estado[i] = PARED;
        }
    }

    /// Derrama desde `p`. Devuelve `false` si el recinto **no** estaba cerrado.
    ///
    /// Que la mancha llegue al borde de la zona de trabajo *es* la prueba de
    /// que se ha escapado: ese borde esta a varias celdas de lo mas externo que
    /// haya dibujado, asi que solo se alcanza saliendo por algun lado.
    fn derramar(&mut self, p: Punto2) -> bool {
        let inicio = match self.celda_libre_cerca(self.columna_de(p.x), self.fila_de(p.y)) {
            Some(i) => i,
            None => return false,
        };
        let mut pila = vec![inicio];
        self.estado[inicio] = MANCHA;

        while let Some(i) = pila.pop() {
            let ix = (i % self.cols) as i64;
            let iy = (i / self.cols) as i64;
            if ix == 0 || iy == 0 || ix as usize == self.cols - 1 || iy as usize == self.filas - 1 {
                return false;
            }
            for (jx, jy) in [(ix - 1, iy), (ix + 1, iy), (ix, iy - 1), (ix, iy + 1)] {
                if !self.dentro(jx, jy) {
                    continue;
                }
                let j = self.indice(jx, jy);
                if self.estado[j] == LIBRE {
                    self.estado[j] = MANCHA;
                    pila.push(j);
                }
            }
        }
        true
    }

    /// La celda libre desde la que derramar, o `None` si no hay ninguna al
    /// lado.
    ///
    /// Tocar justo encima de un trazo es lo mas normal del mundo —con raton se
    /// apunta al borde para saber que hueco se rellena—, y ahi lo que se quiere
    /// es rellenar a un lado. Se busca en anillos hacia afuera, asi que gana la
    /// mas cercana.
    fn celda_libre_cerca(&self, ix: i64, iy: i64) -> Option<usize> {
        if !self.dentro(ix, iy) {
            return None;
        }
        if self.estado[self.indice(ix, iy)] == LIBRE {
            return Some(self.indice(ix, iy));
        }
        for r in 1..=BUSQUEDA_LIBRE {
            for dx in -r..=r {
                for dy in -r..=r {
                    if dx.abs() != r && dy.abs() != r {
                        continue;
                    }
                    let (jx, jy) = (ix + dx, iy + dy);
                    if self.dentro(jx, jy) && self.estado[self.indice(jx, jy)] == LIBRE {
                        return Some(self.indice(jx, jy));
                    }
                }
            }
        }
        None
    }

    /// Crece la mancha una celda hacia las paredes que la rodean.
    fn dilatar(&mut self) {
        let mut frontera = Vec::new();
        for iy in 0..self.filas as i64 {
            for ix in 0..self.cols as i64 {
                if self.estado[self.indice(ix, iy)] != MANCHA {
                    continue;
                }
                for (jx, jy) in [(ix - 1, iy), (ix + 1, iy), (ix, iy - 1), (ix, iy + 1)] {
                    // El borde de la zona de trabajo no se invade: es lo que
                    // sostiene la comprobacion de «se ha escapado».
                    if jx < 1
                        || jy < 1
                        || jx as usize >= self.cols - 1
                        || jy as usize >= self.filas - 1
                    {
                        continue;
                    }
                    if self.estado[self.indice(jx, jy)] == PARED {
                        frontera.push(self.indice(jx, jy));
                    }
                }
            }
        }
        for i in frontera {
            self.estado[i] = MANCHA;
        }
    }

    /// Los contornos de la mancha, en coordenadas del documento.
    ///
    /// Se recorren los **bordes de celda** que separan mancha de no-mancha,
    /// cada uno orientado para dejar la mancha a un lado, y se encadenan por
    /// sus vertices. Salen tantos anillos como bordes tenga la mancha: uno por
    /// fuera y uno por cada agujero, sin tener que distinguirlos aqui.
    fn contornos(&self) -> Vec<Vec<Punto2>> {
        // Vertice = esquina de celda, numerada en una rejilla de
        // (cols+1)x(filas+1).
        let ancho = self.cols + 1;
        let vertice = |ix: i64, iy: i64| -> usize { iy as usize * ancho + ix as usize };
        let mut salidas: HashMap<usize, Vec<usize>> = HashMap::new();
        let mut arista = |desde: usize, hasta: usize| {
            salidas.entry(desde).or_default().push(hasta);
        };

        for iy in 0..self.filas as i64 {
            for ix in 0..self.cols as i64 {
                if self.estado[self.indice(ix, iy)] != MANCHA {
                    continue;
                }
                if !self.es_mancha(ix, iy - 1) {
                    arista(vertice(ix, iy), vertice(ix + 1, iy));
                }
                if !self.es_mancha(ix + 1, iy) {
                    arista(vertice(ix + 1, iy), vertice(ix + 1, iy + 1));
                }
                if !self.es_mancha(ix, iy + 1) {
                    arista(vertice(ix + 1, iy + 1), vertice(ix, iy + 1));
                }
                if !self.es_mancha(ix - 1, iy) {
                    arista(vertice(ix, iy + 1), vertice(ix, iy));
                }
            }
        }

        let mut anillos = Vec::new();
        while let Some(&arranque) = salidas.keys().next() {
            let mut camino: Vec<usize> = Vec::new();
            let mut actual = arranque;
            let mut cerro = false;
            // Un vertice puede tener dos salidas cuando la mancha se estrangula
            // en una esquina; se coge cualquiera y el otro anillo sale en la
            // vuelta siguiente. Lo importante es que ningun borde se quede sin
            // recorrer, y por eso se consumen de uno en uno.
            while let Some(siguientes) = salidas.get_mut(&actual) {
                let Some(siguiente) = siguientes.pop() else {
                    salidas.remove(&actual);
                    break;
                };
                if siguientes.is_empty() {
                    salidas.remove(&actual);
                }
                camino.push(actual);
                actual = siguiente;
                if actual == arranque {
                    cerro = true;
                    break;
                }
            }
            // **Solo se queda con lo que cierra.** En un estrangulamiento, una
            // vuelta puede dejar el vertice descuadrado y la siguiente acabar
            // en un callejon; ese camino abierto no es el borde de nada, y
            // meterlo como anillo pintaria una cuña que no existe.
            if cerro && camino.len() >= 4 {
                anillos.push(
                    camino
                        .into_iter()
                        .map(|v| self.esquina((v % ancho) as i64, (v / ancho) as i64))
                        .collect(),
                );
            }
        }
        anillos
    }
}

// -------------------------------------------------------------------------
// Del escalon al contorno
// -------------------------------------------------------------------------

/// Lleva cada vertice del contorno **hasta la pared que tiene al lado**.
///
/// El contorno que sale de una rejilla va por los bordes de las celdas, asi que
/// contra una pared en diagonal describe una escalera: asoma por fuera en unos
/// tramos y se queda corto en otros. Aqui cada punto se proyecta sobre el
/// segmento mas cercano que tenga dentro de `tolerancia` y se queda ahi.
///
/// Lo que no esta cerca de ninguna pared se deja intacto: son los tramos que
/// cruzan el aire —la mancha entre dos figuras que no se tocan— y ahi no hay
/// nada a lo que ajustarse.
fn pegado_a_las_paredes(
    contorno: &[Punto2],
    paredes: &[(Punto2, Punto2)],
    tolerancia: f32,
) -> Vec<Punto2> {
    if contorno.is_empty() || paredes.is_empty() || tolerancia <= 0.0 {
        return contorno.to_vec();
    }
    let mut pegados: Vec<Punto2> = Vec::with_capacity(contorno.len());
    for v in contorno {
        let mut mejor: Option<Punto2> = None;
        let mut mejor_d = tolerancia;
        for (a, b) in paredes {
            // La criba barata primero: proyectar contra cada pared de un plano
            // entero, por cada vertice de la escalera, es el gasto que este
            // `if` evita.
            if distancia_a_segmento(*v, *a, *b) > mejor_d {
                continue;
            }
            let sobre = proyeccion_en_segmento(*v, *a, *b);
            let d = sobre.distancia(*v);
            if d <= mejor_d {
                mejor_d = d;
                mejor = Some(sobre);
            }
        }
        pegados.push(mejor.unwrap_or(*v));
    }

    // Fundir los que han quedado encima: dos vertices de la escalera pegados a
    // la misma recta acaban en el mismo sitio, y un camino con puntos repetidos
    // desconcierta a todo lo que venga despues.
    let mut juntos: Vec<Punto2> = Vec::with_capacity(pegados.len());
    for p in pegados {
        if juntos.last().is_none_or(|u| p.distancia(*u) > JUNTOS) {
            juntos.push(p);
        }
    }
    // Y el primero con el ultimo, que tambien se tocan al cerrar el anillo.
    while juntos.len() > 2 && juntos[0].distancia(juntos[juntos.len() - 1]) <= JUNTOS {
        juntos.pop();
    }
    if juntos.len() >= 3 {
        juntos
    } else {
        contorno.to_vec()
    }
}

/// Quita los puntos que no aportan forma.
///
/// El contorno sale de recorrer bordes de celda, asi que es una escalera: una
/// diagonal de cien celdas llega con cuatrocientos puntos, todos a un paso de
/// distancia. Sin esto, cada relleno meteria miles de puntos en la escena, el
/// fichero pesaria lo que no esta escrito y el rayado tardaria una eternidad.
///
/// Dos pasadas: primero se tiran los colineales exactos —los tramos rectos de
/// la escalera, que son la mayoria— y despues Douglas-Peucker con la tolerancia
/// de una celda, que es la precision que la rejilla tenia de todas formas.
pub fn simplificar(anillo: &[Punto2], tolerancia: f32) -> Vec<Punto2> {
    if anillo.len() < 4 {
        return anillo.to_vec();
    }
    let sin_rectas = quitar_colineales(anillo);
    if sin_rectas.len() < 4 {
        return sin_rectas;
    }
    // Douglas-Peucker quiere una polilinea con principio y final, y esto es un
    // anillo. Se parte por el punto mas lejano al primero: los dos extremos de
    // la particion son los dos vertices mas marcados del contorno, asi que
    // ninguno de los dos se puede perder por el camino.
    let b = (0..sin_rectas.len())
        .max_by(|i, j| {
            sin_rectas[*i]
                .distancia(sin_rectas[0])
                .partial_cmp(&sin_rectas[*j].distancia(sin_rectas[0]))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(0);
    if b <= 1 {
        return sin_rectas;
    }
    let primera = douglas_peucker(&sin_rectas[0..=b], tolerancia);
    let mut cola: Vec<Punto2> = sin_rectas[b..].to_vec();
    cola.push(sin_rectas[0]);
    let segunda = douglas_peucker(&cola, tolerancia);
    // Los dos trozos comparten sus extremos: se quitan los repetidos al coser.
    let mut salida = primera;
    salida.extend_from_slice(&segunda[1..segunda.len() - 1]);
    salida
}

fn quitar_colineales(anillo: &[Punto2]) -> Vec<Punto2> {
    let n = anillo.len();
    let mut salida = Vec::with_capacity(n);
    for i in 0..n {
        let anterior = anillo[(i + n - 1) % n];
        let actual = anillo[i];
        let siguiente = anillo[(i + 1) % n];
        let cruz = (actual.x - anterior.x) * (siguiente.y - anterior.y)
            - (actual.y - anterior.y) * (siguiente.x - anterior.x);
        if cruz.abs() > 1e-9 {
            salida.push(actual);
        }
    }
    if salida.len() >= 3 {
        salida
    } else {
        anillo.to_vec()
    }
}

/// Douglas-Peucker sobre una polilinea abierta.
///
/// Es el `simplify` que usa Excalidraw antes de rellenar un lapiz cerrado, y el
/// que aqui desescalona los contornos que salen de la rejilla. Uno solo para
/// los dos: quitar los puntos que no aportan forma es el mismo problema.
pub fn douglas_peucker(puntos: &[Punto2], tolerancia: f32) -> Vec<Punto2> {
    if puntos.len() < 3 {
        return puntos.to_vec();
    }
    let (primero, ultimo) = (puntos[0], puntos[puntos.len() - 1]);
    let mut peor = 0.0;
    let mut indice = 0;
    for (i, p) in puntos.iter().enumerate().take(puntos.len() - 1).skip(1) {
        let d = distancia_a_segmento(*p, primero, ultimo);
        if d > peor {
            peor = d;
            indice = i;
        }
    }
    if peor <= tolerancia {
        return vec![primero, ultimo];
    }
    let izquierda = douglas_peucker(&puntos[0..=indice], tolerancia);
    let derecha = douglas_peucker(&puntos[indice..], tolerancia);
    let mut salida = izquierda;
    salida.pop();
    salida.extend(derecha);
    salida
}

/// El color transparente con el que nace un fondo sin estrenar.
pub const TRANSPARENTE: ColorRgba = ColorRgba {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};

#[cfg(test)]
mod pruebas {
    use super::*;

    fn raya(id: u64, a: (f32, f32), b: (f32, f32)) -> Elemento {
        let puntos = vec![Punto2::nuevo(a.0, a.1), Punto2::nuevo(b.0, b.1)];
        let (x0, y0) = (a.0.min(b.0), a.1.min(b.1));
        Elemento {
            id,
            figura: Figura::Linea { puntos },
            x: x0,
            y: y0,
            ancho: (b.0 - a.0).abs(),
            alto: (b.1 - a.1).abs(),
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            grosor: 1.0,
            ..Default::default()
        }
    }

    /// Cuatro rayas que se tocan en las esquinas: un recinto cerrado de 200x200
    /// hecho con figuras distintas, que es el caso que el bote viene a
    /// resolver.
    fn caja_de_cuatro_rayas() -> Vec<Elemento> {
        vec![
            raya(1, (0.0, 0.0), (200.0, 0.0)),
            raya(2, (200.0, 0.0), (200.0, 200.0)),
            raya(3, (200.0, 200.0), (0.0, 200.0)),
            raya(4, (0.0, 200.0), (0.0, 0.0)),
        ]
    }

    #[test]
    fn el_bote_en_una_caja_partida_por_una_raya_cae_justo_sobre_las_lineas() {
        // F2: con la rejilla el trozo salia con una celda de mas o de menos
        // por cada lado; con la cara exacta el area es la del dibujo.
        let mut e = caja_de_cuatro_rayas();
        e.push(raya(5, (80.0, -30.0), (80.0, 230.0)));
        let r = region_en(&e, Punto2::nuevo(20.0, 100.0), &AjustesRelleno::default())
            .expect("el trozo de la izquierda esta cerrado");
        let area = area_de(&r.contorno).abs();
        assert!((area - 16_000.0).abs() < 1.0, "area del trozo: {area}");
        assert!(
            r.contorno
                .iter()
                .all(|q| q.x >= -1e-3 && q.x <= 80.001 && q.y >= -1e-3 && q.y <= 200.001),
            "ningun vertice fuera del trozo: {:?}",
            r.contorno
        );
    }

    #[test]
    fn una_rendija_gorda_no_la_cierra_la_cara_exacta_sino_la_rejilla() {
        // El reparto de trabajo: la cara exacta solo cose juntas de dos
        // unidades; una de cinco la decide la rejilla, que la da por cerrada
        // con su celda. Si la cara exacta se tragara esto, su tolerancia
        // estaria mal y cerraria rendijas que el usuario dejo a proposito.
        let mut e = caja_de_cuatro_rayas();
        e[0] = raya(1, (0.0, 0.0), (195.0, 0.0));
        let tramos: Vec<(Punto2, Punto2)> = e
            .iter()
            .flat_map(|x| segmentos_de(x, PASO_PERIMETRO))
            .collect();
        assert!(
            crate::cara_exacta::cara_exacta(
                &tramos,
                Punto2::nuevo(100.0, 100.0),
                crate::cara_exacta::TOLERANCIA_DE_CARA
            )
            .is_none()
        );
    }

    #[test]
    fn el_hueco_entre_cuatro_rayas_sueltas_se_rellena_aunque_no_sea_de_ninguna() {
        // La razon de ser de la herramienta: ese cuadrado no es de ninguna de
        // las cuatro rayas, y hasta que existio el bote habia que repasarlo a
        // mano para poder rellenar el repaso.
        let r = region_en(
            &caja_de_cuatro_rayas(),
            Punto2::nuevo(100.0, 100.0),
            &AjustesRelleno::default(),
        )
        .expect("un recinto cerrado tiene que dar region");
        assert!(r.huecos.is_empty(), "no habia agujeros: {:?}", r.huecos);
        let area = area_de(&r.contorno).abs();
        // 200x200 son 40.000; se acepta el 5 % de holgura de la rejilla.
        assert!(
            (area - 40_000.0).abs() < 2_000.0,
            "el area encontrada no es la del recinto: {area}"
        );
    }

    #[test]
    fn un_recinto_abierto_no_se_rellena() {
        // **La prueba que de verdad importa.** Si el derrame se escapase por
        // la rendija y devolviera algo, lo que se pintaria seria una mancha
        // del tamaño del dibujo entero encima de todo lo demas: peor que no
        // rellenar nada, porque hay que deshacerlo a ciegas.
        let mut abierta = caja_de_cuatro_rayas();
        // Se abre un boquete de 60 px en el lado de arriba, muy por encima del
        // tamaño de celda: no es una junta mal cerrada, es una puerta.
        abierta[0] = raya(1, (0.0, 0.0), (70.0, 0.0));
        abierta.push(raya(5, (130.0, 0.0), (200.0, 0.0)));
        assert!(
            region_en(
                &abierta,
                Punto2::nuevo(100.0, 100.0),
                &AjustesRelleno::default()
            )
            .is_none(),
            "el derrame se escapo por la rendija y devolvio una region"
        );
    }

    #[test]
    fn una_junta_mal_cerrada_se_da_por_cerrada_y_esa_es_la_tolerancia_que_hace_falta() {
        // Con raton, como con el dedo, dos trazos que se ven unidos casi nunca
        // comparten el punto exacto. Un hueco mas estrecho que una celda se da
        // por cerrado: sin eso, el bote no funcionaria casi nunca y nadie
        // sabria por que.
        let mut casi = caja_de_cuatro_rayas();
        casi[0] = raya(1, (0.0, 0.0), (199.6, 0.0));
        assert!(
            region_en(
                &casi,
                Punto2::nuevo(100.0, 100.0),
                &AjustesRelleno::default()
            )
            .is_some(),
            "una junta de cuatro decimas tiene que darse por cerrada"
        );
    }

    #[test]
    fn un_circulo_dentro_de_un_cuadrado_sale_como_agujero_del_relleno() {
        // Es lo que distingue un relleno de verdad de una mancha: el hueco de
        // dentro **no** se pinta, y por eso `huecos` existe.
        let mut todos = caja_de_cuatro_rayas();
        todos.push(Elemento {
            id: 9,
            figura: Figura::Elipse,
            x: 70.0,
            y: 70.0,
            ancho: 60.0,
            alto: 60.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            grosor: 1.0,
            ..Default::default()
        });
        let r = region_en(
            &todos,
            Punto2::nuevo(20.0, 20.0),
            &AjustesRelleno::default(),
        )
        .expect("el marco sigue cerrado");
        assert_eq!(r.huecos.len(), 1, "el circulo de dentro es un agujero");
        let hueco = area_de(&r.huecos[0]).abs();
        // Pi por 30 al cuadrado son ~2.827.
        assert!(
            (hueco - 2_827.0).abs() < 400.0,
            "el agujero no mide lo que el circulo: {hueco}"
        );
    }

    #[test]
    fn tocar_fuera_de_todo_lo_dibujado_no_rellena_nada() {
        // Caso negativo: fuera del dibujo no hay nada que encerrar, y sin esta
        // criba el derrame empezaria en el margen y devolveria «abierto» tras
        // recorrer la rejilla entera.
        assert!(
            region_en(
                &caja_de_cuatro_rayas(),
                Punto2::nuevo(900.0, 900.0),
                &AjustesRelleno::default()
            )
            .is_none()
        );
    }

    #[test]
    fn sin_ninguna_pared_en_la_escena_no_hay_nada_que_rellenar() {
        // Caso negativo: un lienzo con solo un texto y una foto no encierra
        // nada, aunque a la vista la foto «rodee» el punto tocado.
        let solo_soporte = vec![
            Elemento {
                id: 1,
                figura: Figura::Imagen { id_objeto: 3 },
                x: 0.0,
                y: 0.0,
                ancho: 400.0,
                alto: 400.0,
                ..Default::default()
            },
            Elemento {
                id: 2,
                figura: Figura::Texto {
                    texto: "hola".into(),
                    tam: 20.0,
                    familia: "Segoe UI".into(),
                },
                x: 10.0,
                y: 10.0,
                ancho: 40.0,
                alto: 20.0,
                ..Default::default()
            },
        ];
        assert!(
            region_en(
                &solo_soporte,
                Punto2::nuevo(200.0, 200.0),
                &AjustesRelleno::default()
            )
            .is_none(),
            "contando la foto como pared, tocar en un sitio vacio teñiria la captura entera"
        );
    }

    #[test]
    fn un_relleno_no_hace_de_pared_para_el_siguiente() {
        // Lo que mas costo descubrir en el movil: el contorno de un relleno no
        // cae exactamente sobre los trazos que lo encerraban, asi que contarlo
        // como pared deja una pared falsa ligeramente desplazada y el bote
        // empieza a rellenar a trozos.
        let mancha = Elemento {
            id: 7,
            figura: Figura::Region {
                contorno: vec![
                    Punto2::nuevo(0.0, 0.0),
                    Punto2::nuevo(10.0, 0.0),
                    Punto2::nuevo(10.0, 10.0),
                ],
                huecos: vec![],
            },
            ..Default::default()
        };
        assert!(!es_pared(&mancha));
    }

    #[test]
    fn el_relleno_se_mete_por_debajo_de_la_primera_pared_que_encuentra() {
        // Encima taparia por dentro los trazos que forman el hueco y las
        // lineas se verian mas finas que sus vecinas; al fondo del todo se
        // escondería detras de la foto sobre la que se anota.
        let mut escena = vec![Elemento {
            id: 1,
            figura: Figura::Imagen { id_objeto: 1 },
            x: 0.0,
            y: 0.0,
            ancho: 400.0,
            alto: 400.0,
            ..Default::default()
        }];
        escena.extend(caja_de_cuatro_rayas());
        let region = region_en(
            &escena,
            Punto2::nuevo(100.0, 100.0),
            &AjustesRelleno::default(),
        )
        .unwrap();
        let relleno = nueva_region(region, &escena[1]);
        // La foto es la posicion 0 y no es pared; la primera raya es la 1.
        assert_eq!(sitio_del_relleno(&escena, &relleno), 1);
    }

    #[test]
    fn el_bote_sin_fondo_elegido_pinta_con_el_color_del_trazo() {
        // El fondo nace transparente: sin esta salida, el primer botellazo de
        // todo el mundo no pintaria nada y pareceria que esta rota.
        let mut plantilla = raya(1, (0.0, 0.0), (1.0, 1.0));
        plantilla.trazo = ColorRgba::opaco(1.0, 0.0, 0.0);
        plantilla.relleno = Some(TRANSPARENTE);
        let e = nueva_region(
            Region {
                contorno: vec![
                    Punto2::nuevo(0.0, 0.0),
                    Punto2::nuevo(10.0, 0.0),
                    Punto2::nuevo(10.0, 10.0),
                ],
                huecos: vec![],
            },
            &plantilla,
        );
        assert_eq!(e.relleno, Some(ColorRgba::opaco(1.0, 0.0, 0.0)));
        assert_eq!((e.x, e.y, e.ancho, e.alto), (0.0, 0.0, 10.0, 10.0));
    }

    #[test]
    fn el_area_de_una_arandela_descuenta_su_agujero() {
        // Sin restar el agujero, la arandela mediria lo que el disco, y una
        // cota de superficie que miente es peor que no tenerla.
        let e = Elemento {
            figura: Figura::Region {
                contorno: vec![
                    Punto2::nuevo(0.0, 0.0),
                    Punto2::nuevo(100.0, 0.0),
                    Punto2::nuevo(100.0, 100.0),
                    Punto2::nuevo(0.0, 100.0),
                ],
                huecos: vec![vec![
                    Punto2::nuevo(40.0, 40.0),
                    Punto2::nuevo(60.0, 40.0),
                    Punto2::nuevo(60.0, 60.0),
                    Punto2::nuevo(40.0, 60.0),
                ]],
            },
            ..Default::default()
        };
        assert_eq!(area_de_region(&e), 10_000.0 - 400.0);
        // Y el punto de dentro del agujero **no** esta en la region: es lo que
        // hace que volver a dar el bote ahi pinte el agujero y no sustituya la
        // arandela.
        assert!(punto_en_region(&e, Punto2::nuevo(10.0, 10.0)));
        assert!(!punto_en_region(&e, Punto2::nuevo(50.0, 50.0)));
    }

    #[test]
    fn volver_a_dar_el_bote_en_el_mismo_hueco_senala_el_relleno_anterior() {
        // Un bote no apila capas de pintura: se vuelve a dar porque se queria
        // otro color. Sin esto queda una pila de manchas identicas que hay que
        // deshacer una a una.
        let anterior = Elemento {
            id: 42,
            figura: Figura::Region {
                contorno: vec![
                    Punto2::nuevo(0.0, 0.0),
                    Punto2::nuevo(100.0, 0.0),
                    Punto2::nuevo(100.0, 100.0),
                    Punto2::nuevo(0.0, 100.0),
                ],
                huecos: vec![],
            },
            ..Default::default()
        };
        assert_eq!(
            rellenos_en(std::slice::from_ref(&anterior), Punto2::nuevo(50.0, 50.0)),
            vec![42]
        );
        // Caso negativo: fuera de la mancha no se sustituye nada.
        assert!(rellenos_en(&[anterior], Punto2::nuevo(500.0, 50.0)).is_empty());
    }

    #[test]
    fn el_contorno_que_sale_no_es_una_escalera_de_mil_peldanos() {
        // Sin simplificar, una diagonal de cien celdas llega con cuatrocientos
        // puntos: el fichero pesaria lo que no esta escrito y el rayado
        // tardaria una eternidad.
        let r = region_en(
            &caja_de_cuatro_rayas(),
            Punto2::nuevo(100.0, 100.0),
            &AjustesRelleno::default(),
        )
        .unwrap();
        assert!(
            r.contorno.len() < 24,
            "un cuadrado no necesita {} puntos",
            r.contorno.len()
        );
    }
}
