//! **Recortar y extender: las dos operaciones de un plano hecho a mano.**
//!
//! Port de `Recorte.kt` (475 lineas) del PixPin de Android.
//!
//! Dibujando a escuadra y cartabon nadie traza las rayas a medida. Se trazan de
//! largo, se cruzan con lo que tengan que cruzar, y despues se quita lo que
//! sobra y se estira lo que falta hasta que todo se toca donde debe. Eso es
//! exactamente lo que aqui no se podia hacer: habia que acertar el largo a la
//! primera, o borrar y volver a trazar.
//!
//! - **Recortar** quita el trozo de raya que se toca, hasta donde la cruzan las
//!   demas figuras. Si el trozo esta en medio, la raya se parte en dos.
//! - **Extender** estira la punta que se toca hasta la primera figura que
//!   encuentre en su camino.
//!
//! Las dos trabajan sobre el recorrido de la raya **en coordenadas del
//! documento** y devuelven elementos nuevos, sin tocar la escena: lo que se
//! puede equivocar aqui —que trozo se va, cual se queda, donde queda el origen
//! del elemento despues de cortarlo— se comprueba sin pantalla.
//!
//! # Por que con el raton sale mejor que con el dedo
//!
//! Porque el gesto es un clic y no hay que acertar una coordenada: se toca
//! *dentro* del trozo que sobra, en cualquier punto, que es como uno piensa
//! esto —«este cacho fuera»— y no «cortame en tal sitio».

use crate::elemento::{Elemento, Figura};
use crate::perimetros::{PASO_PERIMETRO, contornos_de, interseccion, segmentos_de};
use crate::vector::{Punto2, distancia_a_segmento};

/// Dos cortes mas juntos que esto son el mismo corte.
const MISMO_CORTE: f32 = 0.01;

/// Por debajo de este largo, un resto no es una raya: es basura que estorba al
/// picar.
const MINIMO_TROZO: f32 = 1.0;

/// Y por debajo de este barrido, un resto de arco no es un arco.
const MINIMO_BARRIDO: f32 = 0.02;

/// Hasta donde se busca al extender, en pixeles del documento.
///
/// Tiene tope a proposito: sin el, una raya casi paralela a otra se estiraria
/// kilometros para alcanzarla, y lo que apareceria en pantalla seria una raya
/// que se va y no vuelve.
pub const ALCANCE_EXTENDER: f32 = 4000.0;

/// Lo que queda de una figura despues de recortarle un trozo.
///
/// Se devuelve asi, y no como una lista, porque los dos huecos **no significan
/// lo mismo** y quien lo consume tiene que tratarlos distinto: `queda` sigue
/// siendo el mismo elemento de la escena y se edita en su sitio; `nace` es un
/// elemento que antes no existia —la segunda mitad de una raya partida— y llega
/// con el id a cero para que la escena le ponga el suyo.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Recorte {
    /// Lo que queda del trazo original. `None` = la raya entera se va, porque
    /// no la cruzaba nada.
    pub queda: Option<Elemento>,
    /// La segunda mitad, cuando el trozo tocado estaba en medio. Nace **sin
    /// id**.
    pub nace: Option<Elemento>,
}

impl Recorte {
    /// Si no queda nada: el elemento se borra entero.
    pub fn se_va_entero(&self) -> bool {
        self.queda.is_none() && self.nace.is_none()
    }
}

/// Las figuras que se recortan como una raya: las que llevan su recorrido en
/// puntos.
pub fn es_lineal(e: &Elemento) -> bool {
    matches!(
        e.figura,
        Figura::Linea { .. }
            | Figura::Flecha { .. }
            | Figura::Lapiz { .. }
            | Figura::Resaltador { .. }
            | Figura::Cota { .. }
    )
}

/// Las figuras a las que se les puede quitar un trozo.
///
/// Las demas —texto, imagen, mosaico, marco, region, punto— no tienen
/// recorrido que cortar, y devolver «se va entera» las borraria de un clic con
/// la herramienta puesta.
pub fn se_recorta(e: &Elemento) -> bool {
    es_lineal(e)
        || matches!(
            e.figura,
            Figura::Elipse | Figura::Arco { .. } | Figura::Rectangulo | Figura::Rombo
        )
}

/// El recorrido de `e` **en coordenadas del documento**, con su inclinacion
/// puesta.
///
/// Los puntos de una figura se guardan sin girar y el angulo va aparte. Con eso
/// se recortaba en un sistema y las paredes se cruzaban en otro: el trozo salia
/// por donde no era y, sobre todo, lo que quedaba volvia a girarse al pintarlo
/// —el angulo seguia puesto en el elemento nuevo—, asi que recortar una figura
/// girada la giraba otra vez.
///
/// Aqui se trabaja en el mundo de principio a fin, que es donde esta el cursor
/// y donde estan las paredes. Ver [`con_puntos`], que por eso deja el angulo a
/// cero.
fn trazado_en_el_mundo(e: &Elemento) -> Vec<Punto2> {
    let Some(puntos) = e.puntos() else {
        return Vec::new();
    };
    if e.angulo == 0.0 {
        return puntos.to_vec();
    }
    let (x0, y0, x1, y1) = e.caja();
    let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    puntos.iter().map(|p| p.girar(centro, e.angulo)).collect()
}

/// Las paredes contra las que se corta: los tramos de todo lo demas.
fn paredes(otros: &[Elemento]) -> Vec<(Punto2, Punto2)> {
    otros
        .iter()
        .filter(|o| !o.borrado)
        .flat_map(|o| segmentos_de(o, PASO_PERIMETRO))
        .collect()
}

/// Los cortes de un camino ya muestreado, como distancias recorridas desde su
/// principio.
///
/// Ordenados y sin repetidos: dos figuras que se cruzan en el mismo punto dan
/// un solo corte, y el vertice donde se juntan dos lados de un rectangulo corta
/// la raya dos veces en el mismo sitio. Tratarlos como dos dejaria entre ellos
/// un trozo de largo cero.
fn cortes_en_camino(camino: &[Punto2], paredes: &[(Punto2, Punto2)]) -> Vec<f32> {
    let mut cortes = Vec::new();
    let mut recorrido = 0.0;
    for par in camino.windows(2) {
        let (a, b) = (par[0], par[1]);
        for (c, d) in paredes {
            if let Some(x) = interseccion(a, b, *c, *d) {
                cortes.push(recorrido + x.distancia(a));
            }
        }
        recorrido += b.distancia(a);
    }
    cortes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut salida: Vec<f32> = Vec::with_capacity(cortes.len());
    for t in cortes {
        if salida.last().is_none_or(|u| t - u > MISMO_CORTE) {
            salida.push(t);
        }
    }
    salida
}

/// Los cortes de `e` con `otros`, como distancias recorridas desde su
/// principio.
pub fn cortes_de(e: &Elemento, otros: &[Elemento]) -> Vec<f32> {
    let camino = trazado_en_el_mundo(e);
    if camino.len() < 2 {
        return Vec::new();
    }
    cortes_en_camino(&camino, &paredes(otros))
}

/// **Recorta el trozo de `e` que contiene `p`.**
///
/// `None` significa «no hay nada que recortar ahi» y la escena no se toca: no
/// es un fallo, es que esa figura no se recorta o el cursor no dio con su
/// recorrido.
pub fn recortar_en(e: &Elemento, otros: &[Elemento], p: Punto2) -> Option<Recorte> {
    if es_lineal(e) {
        return recortar_lineal(e, otros, p);
    }
    match e.figura {
        // **El ovalo y el arco se recortan en arco**, no en polilinea: un trozo
        // de circunferencia sigue siendo una circunferencia, y convertirlo en
        // cien segmentos rectos perderia para siempre la posibilidad de seguir
        // estirandolo con el compas. Un ovalo al que se le quita un trozo es,
        // literalmente, un arco.
        Figura::Elipse | Figura::Arco { .. } => recortar_arco(e, otros, p),
        // El rectangulo y el rombo si: quitarles un lado los deja siendo una
        // polilinea abierta, y eso ya no es un rectangulo por mucho que se
        // llame asi. En un plano es lo normal — se traza la caja y se van
        // quitando los trozos que sobran hasta que queda la pieza.
        Figura::Rectangulo | Figura::Rombo => recortar_anillo(e, otros, p),
        _ => None,
    }
}

fn recortar_lineal(e: &Elemento, otros: &[Elemento], p: Punto2) -> Option<Recorte> {
    let camino = trazado_en_el_mundo(e);
    if camino.len() < 2 {
        return None;
    }
    let total = largo_de(&camino);
    if total <= 0.0 {
        return None;
    }
    let donde = recorrido_hasta(&camino, p)?;
    let cortes = cortes_en_camino(&camino, &paredes(otros));

    let desde = ultimo_menor(&cortes, donde - MISMO_CORTE).unwrap_or(0.0);
    let hasta = primero_mayor(&cortes, donde + MISMO_CORTE).unwrap_or(total);

    let principio = trozo(&camino, 0.0, desde).map(|t| con_puntos(e, &t));
    let final_ = trozo(&camino, hasta, total).map(|t| con_puntos(e, &t));

    Some(match (principio, final_) {
        // Partir en dos hace nacer un elemento: la segunda mitad no es la de
        // antes, y compartir su id la convertiria en la misma raya dos veces.
        (Some(a), Some(b)) => Recorte {
            queda: Some(a),
            nace: Some(sin_id(b)),
        },
        (Some(a), None) | (None, Some(a)) => Recorte {
            queda: Some(a),
            nace: None,
        },
        (None, None) => Recorte::default(),
    })
}

/// Recorta el trozo tocado de una figura **cerrada** de lados rectos.
///
/// Lo que queda deja de ser un rectangulo o un rombo y pasa a ser una raya
/// abierta: es lo que se ve y lo que se espera. La raya empieza donde acababa
/// el trozo quitado y da la vuelta entera hasta donde empezaba, pasando por la
/// costura del contorno como si no existiera — que es justo por lo que hay que
/// recorrerlo en circulo y no de principio a fin.
fn recortar_anillo(e: &Elemento, otros: &[Elemento], p: Punto2) -> Option<Recorte> {
    let contorno = contornos_de(e, PASO_PERIMETRO).into_iter().next()?;
    if contorno.puntos.len() < 3 {
        return None;
    }
    let mut anillo = contorno.puntos;
    anillo.push(anillo[0]);
    let total = largo_de(&anillo);
    if total <= 0.0 {
        return None;
    }
    let donde = recorrido_hasta(&anillo, p)?;
    let cortes: Vec<f32> = cortes_en_camino(&anillo, &paredes(otros))
        .into_iter()
        .filter(|t| *t > MISMO_CORTE && *t < total - MISMO_CORTE)
        .collect();

    // Con menos de dos cortes no hay trozo que quitar: la figura entera se va.
    if cortes.len() < 2 {
        return Some(Recorte::default());
    }
    let (desde, hasta) = trozo_ciclico(&cortes, donde);
    let Some(puntos) = recorrido_ciclico(&anillo, hasta, desde, total) else {
        return Some(Recorte::default());
    };
    let mut resto = con_puntos(e, &puntos);
    resto.figura = Figura::Linea { puntos };
    // Sin redondeo: el contorno ya viene muestreado con sus esquinas
    // redondeadas si las tenia, y pasarle ademas la curva de la linea lo
    // redondearia dos veces.
    resto.redondo = false;
    // Y sin fondo: media caja abierta no encierra nada que pintar.
    resto.relleno = None;
    Some(Recorte {
        queda: Some(resto),
        nace: None,
    })
}

/// Recorta el trozo tocado de un ovalo o de un arco, **y devuelve arcos**.
///
/// Aqui no se trabaja con distancias recorridas sino con **angulos del ovalo**:
/// un arco no guarda por donde pasa, guarda donde empieza y cuanto barre, asi
/// que lo que hay que cortar es el barrido. La conversion sale gratis porque
/// cada cruce es un punto y todo punto tiene su angulo en el ovalo.
fn recortar_arco(e: &Elemento, otros: &[Elemento], p: Punto2) -> Option<Recorte> {
    let (rx, ry) = (e.ancho / 2.0, e.alto / 2.0);
    if rx <= 0.0 || ry <= 0.0 {
        return None;
    }
    let cerrado = matches!(e.figura, Figura::Elipse);
    let (inicio, barrido) = match e.figura {
        Figura::Arco { inicio, barrido } => (inicio, barrido.unwrap_or(std::f32::consts::TAU)),
        _ => (0.0, std::f32::consts::TAU),
    };
    if barrido.abs() < 1e-6 {
        return None;
    }
    let signo = if barrido < 0.0 { -1.0 } else { 1.0 };
    let largo = barrido.abs();

    // Cuanto se lleva barrido al llegar a un punto, de 0 a `largo`.
    let avance = |punto: Punto2| -> f32 {
        let bruto = (angulo_en_el_ovalo(e, punto) - inicio) * signo;
        let mut t = bruto % std::f32::consts::TAU;
        if t < 0.0 {
            t += std::f32::consts::TAU;
        }
        t
    };

    let donde = avance(p);
    if !cerrado && donde > largo {
        return None;
    }

    // El camino contra el que cruzar: los contornos ya vienen muestreados **y
    // girados** (ver `perimetros`), que es donde estan las paredes.
    // Del sector o la marca solo cuenta su arco: sus radios no son camino.
    let mut solo_arco = e.clone();
    solo_arco.extras.cierre = None;
    solo_arco.extras.marca_angulo = None;
    let camino = contornos_de(&solo_arco, PASO_PERIMETRO)
        .into_iter()
        .next()?
        .puntos;
    if camino.len() < 2 {
        return None;
    }
    let paredes = paredes(otros);
    let mut cruces: Vec<f32> = Vec::new();
    for par in camino.windows(2) {
        for (c, d) in &paredes {
            if let Some(x) = interseccion(par[0], par[1], *c, *d) {
                cruces.push(avance(x));
            }
        }
    }
    if cerrado {
        // El ovalo se cierra sobre si mismo: el tramo que une el ultimo punto
        // con el primero tambien puede cruzar algo.
        if let (Some(u), Some(pr)) = (camino.last(), camino.first()) {
            for (c, d) in &paredes {
                if let Some(x) = interseccion(*u, *pr, *c, *d) {
                    cruces.push(avance(x));
                }
            }
        }
    }
    cruces.retain(|t| cerrado || (*t > MISMO_CORTE && *t < largo - MISMO_CORTE));
    cruces.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut cortes: Vec<f32> = Vec::with_capacity(cruces.len());
    for t in cruces {
        if cortes.last().is_none_or(|u| t - u > MISMO_CORTE) {
            cortes.push(t);
        }
    }

    let arco = |desde_t: f32, hasta_t: f32| -> Option<Elemento> {
        let barre = hasta_t - desde_t;
        if barre < MINIMO_BARRIDO {
            return None;
        }
        let mut nuevo = e.clone();
        nuevo.figura = Figura::Arco {
            inicio: inicio + signo * desde_t,
            barrido: Some(signo * barre),
        };
        // Un trozo de ovalo ya no encierra nada: el fondo se va con el resto
        // (y con el, el cierre de sector y el degradado).
        nuevo.relleno = None;
        nuevo.extras.cierre = None;
        nuevo.extras.degradado = None;
        nuevo.tocar();
        Some(nuevo)
    };

    if cerrado {
        if cortes.len() < 2 {
            return Some(Recorte::default());
        }
        let (desde, hasta) = trozo_ciclico(&cortes, donde);
        let mut barre = desde - hasta;
        if barre < 0.0 {
            barre += std::f32::consts::TAU;
        }
        return Some(Recorte {
            queda: arco(hasta, hasta + barre),
            nace: None,
        });
    }

    let desde = ultimo_menor(&cortes, donde - MISMO_CORTE).unwrap_or(0.0);
    let hasta = primero_mayor(&cortes, donde + MISMO_CORTE).unwrap_or(largo);
    Some(match (arco(0.0, desde), arco(hasta, largo)) {
        (Some(a), Some(b)) => Recorte {
            queda: Some(a),
            nace: Some(sin_id(b)),
        },
        (Some(a), None) | (None, Some(a)) => Recorte {
            queda: Some(a),
            nace: None,
        },
        (None, None) => Recorte::default(),
    })
}

/// El angulo **parametrico** de un punto respecto al centro del ovalo.
///
/// Parametrico y no geometrico: se divide por cada semieje antes del `atan2`,
/// de modo que el angulo es el de la circunferencia de la que sale el ovalo al
/// estirarlo. Es la unica forma de que recorrer el borde de una elipse achatada
/// avance a ritmo constante; con el angulo geometrico, el recorrido corre en
/// los extremos y se arrastra en los lados.
fn angulo_en_el_ovalo(e: &Elemento, p: Punto2) -> f32 {
    let (rx, ry) = (e.ancho / 2.0, e.alto / 2.0);
    if rx <= 0.0 || ry <= 0.0 {
        return 0.0;
    }
    let centro = Punto2::nuevo(e.x + rx, e.y + ry);
    // La inclinacion se deshace antes, asi que un ovalo girado se recorre igual
    // que uno recto.
    let local = if e.angulo == 0.0 {
        p
    } else {
        p.girar(centro, -e.angulo)
    };
    ((local.y - centro.y) / ry).atan2((local.x - centro.x) / rx)
}

/// **Estira la punta de `e` mas cercana a `p` hasta la primera figura de
/// `otros`.**
///
/// `None` si no hay nada en su camino: estirar hasta el infinito no es
/// extender, es tirar una raya al vacio, y eso ya se hace dibujando.
///
/// Se prolonga **en la direccion del ultimo tramo**, no en la de toda la raya:
/// en una polilinea con dobleces lo que se estira es el tramo del final, que es
/// lo que uno ve como «la punta».
pub fn extender_en(e: &Elemento, otros: &[Elemento], p: Punto2, alcance: f32) -> Option<Elemento> {
    // **El arco se extiende por su circunferencia**, no en recta: alargar un
    // arco es seguir girando el compas (`geometria_exacta::extender_arco`).
    if matches!(
        e.figura,
        Figura::Arco {
            barrido: Some(_),
            ..
        }
    ) {
        return crate::geometria_exacta::extender_arco(e, otros, p);
    }
    let camino = trazado_en_el_mundo(e);
    if camino.len() < 2 {
        return None;
    }
    let por_el_principio = p.distancia(camino[0]) < p.distancia(camino[camino.len() - 1]);
    let (punta, anterior) = if por_el_principio {
        (camino[0], camino[1])
    } else {
        (camino[camino.len() - 1], camino[camino.len() - 2])
    };
    let direccion = punta.restar(anterior);
    let largo = direccion.longitud();
    if largo <= 0.0 {
        return None;
    }
    let lejos = punta.sumar(direccion.escalar(alcance / largo));

    // La primera que se encuentre, no la mas lejana: extender es «hasta que
    // topes», y pasarse de largo hasta la segunda dejaria la raya cruzando por
    // encima de la figura que tenia delante.
    let mut destino: Option<Punto2> = None;
    let mut mejor = f32::MAX;
    for (c, d) in paredes(otros) {
        let Some(x) = interseccion(punta, lejos, c, d) else {
            continue;
        };
        let dist = x.distancia(punta);
        // El cero es la propia punta apoyada en algo: no es un sitio al que ir.
        if dist > MISMO_CORTE && dist < mejor {
            mejor = dist;
            destino = Some(x);
        }
    }
    let fin = destino?;

    let mut nuevos = camino;
    if por_el_principio {
        nuevos[0] = fin;
    } else {
        let ultimo = nuevos.len() - 1;
        nuevos[ultimo] = fin;
    }
    Some(con_puntos(e, &nuevos))
}

// -------------------------------------------------------------------------
// Sobre el recorrido
// -------------------------------------------------------------------------

fn largo_de(camino: &[Punto2]) -> f32 {
    camino.windows(2).map(|p| p[1].distancia(p[0])).sum()
}

fn ultimo_menor(cortes: &[f32], tope: f32) -> Option<f32> {
    cortes.iter().rev().find(|t| **t < tope).copied()
}

fn primero_mayor(cortes: &[f32], suelo: f32) -> Option<f32> {
    cortes.iter().find(|t| **t > suelo).copied()
}

/// Cuanto se lleva recorrido en el punto del camino mas cercano a `p`.
fn recorrido_hasta(camino: &[Punto2], p: Punto2) -> Option<f32> {
    let mut recorrido = 0.0;
    let mut mejor = f32::MAX;
    let mut respuesta = None;
    for par in camino.windows(2) {
        let (a, b) = (par[0], par[1]);
        let largo = b.distancia(a);
        let d = distancia_a_segmento(p, a, b);
        if d < mejor {
            mejor = d;
            let t = if largo <= 0.0 {
                0.0
            } else {
                (((p.x - a.x) * (b.x - a.x) + (p.y - a.y) * (b.y - a.y)) / (largo * largo))
                    .clamp(0.0, 1.0)
            };
            respuesta = Some(recorrido + t * largo);
        }
        recorrido += largo;
    }
    respuesta
}

/// El trozo de `camino` entre las distancias `desde` y `hasta`.
///
/// `None` si el trozo no da ni para una raya: un cacho de dos pixeles no es un
/// resto, es basura que se queda estorbando al picar.
fn trozo(camino: &[Punto2], desde: f32, hasta: f32) -> Option<Vec<Punto2>> {
    if hasta - desde < MINIMO_TROZO {
        return None;
    }
    let mut salida: Vec<Punto2> = Vec::new();
    let mut recorrido = 0.0;
    for par in camino.windows(2) {
        let (a, b) = (par[0], par[1]);
        let largo = b.distancia(a);
        if largo > 0.0 {
            let fin = recorrido + largo;
            // El principio del trozo cae dentro de este tramo.
            if desde >= recorrido && desde <= fin && salida.is_empty() {
                salida.push(a.hacia(b, (desde - recorrido) / largo));
            }
            // Los vertices que quedan dentro se conservan: son los dobleces.
            if !salida.is_empty() && fin > desde && fin < hasta {
                salida.push(b);
            }
            if hasta >= recorrido && hasta <= fin {
                salida.push(a.hacia(b, (hasta - recorrido) / largo));
                break;
            }
        }
        recorrido += largo;
    }
    if salida.len() >= 2 {
        Some(salida)
    } else {
        None
    }
}

/// Entre que dos cortes cae `donde`, **dando la vuelta si hace falta**.
///
/// En un contorno cerrado no hay principio ni final, asi que el trozo que
/// contiene al cursor puede ser el que cruza la costura: el que va del ultimo
/// corte al primero pasando por el cero. Devolverlo como un par ordenado sin
/// mas lo dejaria del reves y se quitaria justo todo lo contrario.
fn trozo_ciclico(cortes: &[f32], donde: f32) -> (f32, f32) {
    let anterior = ultimo_menor(cortes, donde).unwrap_or_else(|| cortes[cortes.len() - 1]);
    let siguiente = primero_mayor(cortes, donde).unwrap_or(cortes[0]);
    (anterior, siguiente)
}

/// El recorrido de `anillo` de `desde` a `hasta` hacia delante, dando la
/// vuelta.
fn recorrido_ciclico(anillo: &[Punto2], desde: f32, hasta: f32, total: f32) -> Option<Vec<Punto2>> {
    let puntos = if desde <= hasta {
        trozo(anillo, desde, hasta)
    } else {
        let cola = trozo(anillo, desde, total);
        let cabeza = trozo(anillo, 0.0, hasta);
        match (cola, cabeza) {
            (None, c) => c,
            (c, None) => c,
            // La costura: el ultimo punto de la cola y el primero de la cabeza
            // son el mismo sitio, y repetirlo dejaria un tramo de largo cero
            // que luego confunde a todo el que recorra la raya.
            (Some(mut cola), Some(cabeza)) => {
                cola.extend_from_slice(&cabeza[1..]);
                Some(cola)
            }
        }
    };
    puntos.filter(|p| p.len() >= 2)
}

/// El elemento con otro recorrido, **recolocando su origen**.
///
/// `x`/`y` son la referencia con la que el puente escribe los puntos al
/// guardar, asi que cambiar el recorrido sin moverlos mandaria la raya a otro
/// sitio en cuanto el fichero fuera y volviera del movil.
///
/// **Y el angulo se queda a cero.** Los puntos llegan ya en coordenadas del
/// documento, con la inclinacion de la figura vieja aplicada; dejarle ademas el
/// angulo puesto seria girar dos veces lo mismo. Ver [`trazado_en_el_mundo`].
fn con_puntos(e: &Elemento, absolutos: &[Punto2]) -> Elemento {
    let mut nuevo = e.clone();
    let origen = absolutos[0];
    let puntos = absolutos.to_vec();
    match &mut nuevo.figura {
        Figura::Linea { puntos: p }
        | Figura::Flecha { puntos: p, .. }
        | Figura::Cota { puntos: p }
        | Figura::Resaltador { puntos: p }
        | Figura::Lapiz { puntos: p, .. } => *p = puntos.clone(),
        _ => {}
    }
    // El lapiz guarda una presion por punto: si se quedaran las de antes, el
    // trozo que sobrevive se dibujaria con el grosor de otro tramo.
    if let Figura::Lapiz { presiones, .. } = &mut nuevo.figura {
        presiones.clear();
    }
    let x0 = puntos.iter().map(|p| p.x).fold(f32::MAX, f32::min);
    let y0 = puntos.iter().map(|p| p.y).fold(f32::MAX, f32::min);
    let x1 = puntos.iter().map(|p| p.x).fold(f32::MIN, f32::max);
    let y1 = puntos.iter().map(|p| p.y).fold(f32::MIN, f32::max);
    nuevo.x = origen.x;
    nuevo.y = origen.y;
    nuevo.ancho = x1 - x0;
    nuevo.alto = y1 - y0;
    nuevo.angulo = 0.0;
    // Recortar rompe cualquier anclaje: la punta ya no esta donde estaba, y una
    // flecha atada a una caja por un extremo que acaba de desaparecer seguiria
    // tirando de ella.
    nuevo.extras.enganche_inicio = None;
    nuevo.extras.enganche_fin = None;
    nuevo.tocar();
    nuevo
}

/// La mitad que nace: sin id y con semilla propia.
///
/// Compartir la semilla con la mitad de la que salio haria que las dos
/// temblaran igual, y dos rayas con el mismo pulso al lado se ven como una
/// copia y no como dos trozos.
fn sin_id(mut e: Elemento) -> Elemento {
    e.id = 0;
    e.semilla = e.semilla.wrapping_mul(2_654_435_761).wrapping_add(1).max(1);
    e
}

// -------------------------------------------------------------------------
// Sobre la escena: lo que hace la herramienta del editor (y el asistente)
// -------------------------------------------------------------------------

/// El largo del recorrido de una raya, en el mundo (0 si no lo tiene).
pub fn largo_del_recorrido(e: &Elemento) -> f32 {
    largo_de(&trazado_en_el_mundo(e))
}

/// El punto del recorrido de una raya a `d` de su principio (`None` si no
/// tiene recorrido). Es como se nombra «el trozo del principio» o «el del
/// final» sin picar: se pica en medio de ese trozo.
pub fn punto_del_recorrido(e: &Elemento, d: f32) -> Option<Punto2> {
    let camino = trazado_en_el_mundo(e);
    if camino.len() < 2 {
        return None;
    }
    let mut recorrido = 0.0;
    for par in camino.windows(2) {
        let largo = par[1].distancia(par[0]);
        if largo > 0.0 && d <= recorrido + largo {
            return Some(par[0].hacia(par[1], ((d - recorrido) / largo).clamp(0.0, 1.0)));
        }
        recorrido += largo;
    }
    camino.last().copied()
}

/// Las paredes de un recorte en la escena: todo lo demas, o solo `contra`.
fn victima_y_paredes(
    escena: &crate::escena::Escena,
    id: u64,
    contra: Option<&[u64]>,
) -> Option<(Elemento, Vec<Elemento>)> {
    let victima = escena.buscar(id).filter(|e| !e.borrado)?.clone();
    let otros = escena
        .elementos
        .iter()
        .filter(|e| e.id != id && !e.borrado && contra.is_none_or(|c| c.contains(&e.id)))
        .cloned()
        .collect();
    Some((victima, otros))
}

/// **Recortar en la escena**: quita el trozo de `id` que contiene `p`, hasta
/// donde lo cruzan las demas figuras (o solo las de `contra`). Un paso de
/// deshacer. `false` si no habia nada que recortar ahi.
///
/// Si el trozo estaba en medio, la raya se parte en dos y la segunda mitad es
/// un elemento nuevo —no la de antes: compartir el id las convertiria en la
/// misma raya dos veces, y borrar una borraria la otra—.
pub fn recortar_en_escena(
    escena: &mut crate::escena::Escena,
    id: u64,
    p: Punto2,
    contra: Option<&[u64]>,
) -> bool {
    let Some((victima, otros)) = victima_y_paredes(escena, id, contra) else {
        return false;
    };
    let Some(r) = recortar_en(&victima, &otros, p) else {
        return false;
    };
    escena.abrir_paso();
    match r.queda {
        Some(queda) => {
            escena.apuntar_edicion(id);
            if let Some(e) = escena.buscar_mut(id) {
                // El id es suyo y no se toca: sigue siendo la misma raya, mas
                // corta.
                let suyo = e.id;
                *e = queda;
                e.id = suyo;
            }
        }
        // Nada la cruzaba: el trozo tocado era la raya entera.
        None => {
            escena.borrar_apuntando(id);
        }
    }
    if let Some(nace) = r.nace {
        escena.anadir(nace);
    }
    escena.cerrar_paso();
    true
}

/// **Extender en la escena**: estira la punta de `id` mas cercana a `p` hasta
/// lo primero que tope (de todo lo demas, o solo de `contra`). Un paso de
/// deshacer. `false` si no hay nada en su camino: estirar hasta el infinito no
/// es extender.
pub fn extender_en_escena(
    escena: &mut crate::escena::Escena,
    id: u64,
    p: Punto2,
    contra: Option<&[u64]>,
    alcance: f32,
) -> bool {
    let Some((victima, otros)) = victima_y_paredes(escena, id, contra) else {
        return false;
    };
    let Some(estirada) = extender_en(&victima, &otros, p, alcance) else {
        return false;
    };
    escena.abrir_paso();
    escena.apuntar_edicion(id);
    if let Some(e) = escena.buscar_mut(id) {
        let suyo = e.id;
        *e = estirada;
        e.id = suyo;
    }
    escena.cerrar_paso();
    true
}

#[cfg(test)]
mod pruebas_en_la_escena {
    use super::*;
    use crate::escena::Escena;

    fn raya(a: (f32, f32), b: (f32, f32)) -> Elemento {
        Elemento {
            figura: Figura::Linea {
                puntos: vec![Punto2::nuevo(a.0, a.1), Punto2::nuevo(b.0, b.1)],
            },
            x: a.0,
            y: a.1,
            ..Default::default()
        }
    }

    #[test]
    fn recortar_solo_contra_una_figura_ignora_las_demas() {
        let mut e = Escena::nueva();
        let h = e.anadir(raya((0.0, 50.0), (200.0, 50.0)));
        let v1 = e.anadir(raya((100.0, 0.0), (100.0, 100.0)));
        let _v2 = e.anadir(raya((150.0, 0.0), (150.0, 100.0)));
        // Contra la de x=100 solamente: el final se va entero hasta x=100.
        assert!(recortar_en_escena(
            &mut e,
            h,
            Punto2::nuevo(180.0, 50.0),
            Some(&[v1])
        ));
        assert!((largo_del_recorrido(e.buscar(h).unwrap()) - 100.0).abs() < 0.5);
        assert!(e.deshacer());
        assert!((largo_del_recorrido(e.buscar(h).unwrap()) - 200.0).abs() < 0.5);
    }

    #[test]
    fn extender_sin_nada_delante_no_toca_la_escena() {
        // Caso negativo.
        let mut e = Escena::nueva();
        let h = e.anadir(raya((0.0, 0.0), (10.0, 0.0)));
        let antes = e.pasos_cerrados();
        assert!(!extender_en_escena(
            &mut e,
            h,
            Punto2::nuevo(10.0, 0.0),
            None,
            ALCANCE_EXTENDER
        ));
        assert_eq!(e.pasos_cerrados(), antes);
    }

    #[test]
    fn el_punto_del_recorrido_va_por_los_dobleces() {
        let mut l = raya((0.0, 0.0), (10.0, 0.0));
        if let Figura::Linea { puntos } = &mut l.figura {
            puntos.push(Punto2::nuevo(10.0, 10.0));
        }
        assert_eq!(largo_del_recorrido(&l), 20.0);
        let p = punto_del_recorrido(&l, 15.0).unwrap();
        assert!((p.x - 10.0).abs() < 1e-4 && (p.y - 5.0).abs() < 1e-4);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::ColorRgba;
    use crate::formas::TipoPunta;

    fn raya(id: u64, a: (f32, f32), b: (f32, f32)) -> Elemento {
        let puntos = vec![Punto2::nuevo(a.0, a.1), Punto2::nuevo(b.0, b.1)];
        Elemento {
            id,
            figura: Figura::Linea { puntos },
            x: a.0,
            y: a.1,
            ancho: (b.0 - a.0).abs(),
            alto: (b.1 - a.1).abs(),
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            grosor: 1.0,
            ..Default::default()
        }
    }

    fn puntos_de(e: &Elemento) -> Vec<Punto2> {
        e.puntos().unwrap().to_vec()
    }

    #[test]
    fn la_punta_que_sobra_se_va_y_la_raya_acaba_en_el_cruce() {
        // El gesto del plano: se traza de largo, se cruza con lo que tenga que
        // cruzar, y lo que asoma se quita.
        let larga = raya(1, (0.0, 50.0), (200.0, 50.0));
        let pared = raya(2, (100.0, 0.0), (100.0, 100.0));
        let r = recortar_en(&larga, &[pared], Punto2::nuevo(160.0, 50.0)).unwrap();
        assert!(r.nace.is_none(), "quitar una punta no hace nacer nada");
        let queda = r.queda.expect("tiene que quedar el trozo de la izquierda");
        let p = puntos_de(&queda);
        assert_eq!(p[0], Punto2::nuevo(0.0, 50.0));
        assert!(
            (p[p.len() - 1].x - 100.0).abs() < 0.01,
            "la raya no acaba en el cruce: {:?}",
            p[p.len() - 1]
        );
    }

    #[test]
    fn el_trozo_de_en_medio_parte_la_raya_en_dos_y_la_segunda_mitad_nace_sin_id() {
        // Las dos mitades no pueden compartir el id: serian el mismo elemento
        // dos veces, y borrar una borraria la otra.
        let larga = raya(1, (0.0, 50.0), (300.0, 50.0));
        let a = raya(2, (100.0, 0.0), (100.0, 100.0));
        let b = raya(3, (200.0, 0.0), (200.0, 100.0));
        let r = recortar_en(&larga, &[a, b], Punto2::nuevo(150.0, 50.0)).unwrap();
        let izquierda = r.queda.expect("izquierda");
        let derecha = r.nace.expect("derecha");
        assert_eq!(izquierda.id, 1);
        assert_eq!(derecha.id, 0, "la mitad nueva llega sin id");
        assert_ne!(
            izquierda.semilla, derecha.semilla,
            "con la misma semilla las dos mitades tiemblan igual y parecen una copia"
        );
        assert!((puntos_de(&izquierda)[1].x - 100.0).abs() < 0.01);
        assert!((puntos_de(&derecha)[0].x - 200.0).abs() < 0.01);
    }

    #[test]
    fn una_raya_que_no_cruza_nada_se_va_entera() {
        // No es un fallo: si nada la corta, el trozo tocado es la raya entera.
        let sola = raya(1, (0.0, 0.0), (100.0, 0.0));
        let lejos = raya(2, (500.0, 500.0), (600.0, 600.0));
        let r = recortar_en(&sola, &[lejos], Punto2::nuevo(50.0, 0.0)).unwrap();
        assert!(r.se_va_entero());
    }

    #[test]
    fn recortar_una_raya_girada_no_la_manda_a_otro_sitio() {
        // El fallo del movil, clavado: los puntos se guardan sin girar y el
        // angulo va aparte, asi que el trozo que quedaba volvia a girarse al
        // pintarlo y aparecia dando una vuelta de mas.
        let mut girada = raya(1, (0.0, 0.0), (200.0, 0.0));
        girada.angulo = std::f32::consts::FRAC_PI_2;
        // Con el giro, la raya va de (100,-100) a (100,100) en el mundo.
        let pared = raya(2, (50.0, 50.0), (150.0, 50.0));
        let r = recortar_en(&girada, &[pared], Punto2::nuevo(100.0, 90.0)).unwrap();
        let queda = r.queda.expect("queda la mitad de arriba");
        assert_eq!(
            queda.angulo, 0.0,
            "el angulo se queda a cero o se gira dos veces"
        );
        let p = puntos_de(&queda);
        assert!(
            (p[p.len() - 1].y - 50.0).abs() < 0.5,
            "no acabo en la pared: {:?}",
            p
        );
    }

    #[test]
    fn recortar_un_rectangulo_lo_convierte_en_una_raya_abierta() {
        // Quitarle un lado ya no es un rectangulo por mucho que se llame asi.
        let caja = Elemento {
            id: 1,
            figura: Figura::Rectangulo,
            x: 0.0,
            y: 0.0,
            ancho: 100.0,
            alto: 100.0,
            redondo: true,
            relleno: Some(ColorRgba::opaco(1.0, 0.0, 0.0)),
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            grosor: 1.0,
            ..Default::default()
        };
        let a = raya(2, (-20.0, 20.0), (20.0, 20.0));
        let b = raya(3, (-20.0, 80.0), (20.0, 80.0));
        let r = recortar_en(&caja, &[a, b], Punto2::nuevo(0.0, 50.0)).unwrap();
        let queda = r.queda.expect("queda la caja abierta");
        assert!(matches!(queda.figura, Figura::Linea { .. }));
        assert!(!queda.redondo, "redondear dos veces deforma las esquinas");
        assert!(queda.relleno.is_none(), "media caja no encierra nada");
    }

    #[test]
    fn recortar_un_ovalo_devuelve_un_arco_y_no_cien_rayas() {
        // Un trozo de circunferencia sigue siendo una circunferencia: en
        // polilinea se perderia para siempre la posibilidad de estirarlo.
        let ovalo = Elemento {
            id: 1,
            figura: Figura::Elipse,
            x: 0.0,
            y: 0.0,
            ancho: 200.0,
            alto: 200.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            grosor: 1.0,
            ..Default::default()
        };
        // Una raya horizontal por el centro lo corta en dos puntos.
        let corte = raya(2, (-50.0, 100.0), (250.0, 100.0));
        let r = recortar_en(&ovalo, &[corte], Punto2::nuevo(100.0, 10.0)).unwrap();
        let queda = r.queda.expect("queda la mitad de abajo");
        let Figura::Arco { barrido, .. } = queda.figura else {
            panic!(
                "el ovalo recortado tiene que ser un arco: {:?}",
                queda.figura
            );
        };
        let b = barrido.expect("un arco recortado ya no es la guia");
        assert!(
            (b.abs() - std::f32::consts::PI).abs() < 0.3,
            "media circunferencia son ~pi, no {b}"
        );
    }

    #[test]
    fn extender_lleva_la_punta_hasta_lo_primero_que_topa_y_no_hasta_lo_segundo() {
        // «Hasta que topes»: pasarse hasta la segunda dejaria la raya cruzando
        // por encima de la figura que tenia delante.
        let corta = raya(1, (0.0, 50.0), (50.0, 50.0));
        let cerca = raya(2, (100.0, 0.0), (100.0, 100.0));
        let lejos = raya(3, (300.0, 0.0), (300.0, 100.0));
        let e = extender_en(
            &corta,
            &[cerca, lejos],
            Punto2::nuevo(50.0, 50.0),
            ALCANCE_EXTENDER,
        )
        .expect("hay dos paredes en su camino");
        let p = puntos_de(&e);
        assert!((p[1].x - 100.0).abs() < 0.01, "llego a {:?}", p[1]);
    }

    #[test]
    fn extender_por_el_principio_estira_el_extremo_que_se_toca() {
        let corta = raya(1, (100.0, 50.0), (200.0, 50.0));
        let pared = raya(2, (20.0, 0.0), (20.0, 100.0));
        let e = extender_en(
            &corta,
            &[pared],
            Punto2::nuevo(101.0, 50.0),
            ALCANCE_EXTENDER,
        )
        .expect("la pared esta a la izquierda");
        let p = puntos_de(&e);
        assert!((p[0].x - 20.0).abs() < 0.01, "empieza en {:?}", p[0]);
        assert_eq!(p[1], Punto2::nuevo(200.0, 50.0), "la otra punta no se toca");
    }

    #[test]
    fn extender_sin_nada_en_el_camino_no_hace_nada() {
        // Caso negativo, y el que da sentido al tope: estirar hasta el
        // infinito no es extender, es tirar una raya al vacio.
        let sola = raya(1, (0.0, 0.0), (50.0, 0.0));
        let paralela = raya(2, (0.0, 80.0), (50.0, 80.0));
        assert!(
            extender_en(
                &sola,
                &[paralela],
                Punto2::nuevo(50.0, 0.0),
                ALCANCE_EXTENDER
            )
            .is_none()
        );
    }

    #[test]
    fn recortar_una_figura_que_no_se_recorta_no_toca_nada() {
        // Caso negativo: un texto o una imagen no tienen recorrido que cortar,
        // y devolver «se va entero» los borraria de un clic.
        let texto = Elemento {
            id: 1,
            figura: Figura::Texto {
                texto: "hola".into(),
                tam: 20.0,
                familia: "Segoe UI".into(),
            },
            x: 0.0,
            y: 0.0,
            ancho: 40.0,
            alto: 20.0,
            ..Default::default()
        };
        let pared = raya(2, (20.0, -10.0), (20.0, 30.0));
        assert!(recortar_en(&texto, &[pared], Punto2::nuevo(10.0, 10.0)).is_none());
    }

    #[test]
    fn recortar_rompe_los_enganches_de_las_puntas() {
        // La punta ya no esta donde estaba: una flecha atada a una caja por un
        // extremo que acaba de desaparecer seguiria tirando de ella.
        let mut flecha = raya(1, (0.0, 50.0), (300.0, 50.0));
        flecha.figura = Figura::Flecha {
            puntos: puntos_de(&flecha),
            punta_inicio: TipoPunta::Ninguna,
            punta_fin: TipoPunta::Flecha,
            codos: false,
        };
        flecha.extras.enganche_fin = Some(crate::elemento::Enganche {
            elemento: "otro".into(),
            foco: 0.0,
            hueco: 1.0,
            punto_fijo: None,
            modo: Default::default(),
        });
        let pared = raya(2, (100.0, 0.0), (100.0, 100.0));
        let r = recortar_en(&flecha, &[pared], Punto2::nuevo(200.0, 50.0)).unwrap();
        assert!(r.queda.unwrap().extras.enganche_fin.is_none());
    }

    #[test]
    fn los_cortes_de_un_vertice_no_se_cuentan_dos_veces() {
        // El vertice donde se juntan dos lados de un rectangulo corta la raya
        // dos veces en el mismo punto: tratarlos como dos dejaria entre ellos
        // un trozo de largo cero.
        let larga = raya(1, (0.0, 0.0), (200.0, 200.0));
        let caja = Elemento {
            id: 2,
            figura: Figura::Rectangulo,
            x: 100.0,
            y: 100.0,
            ancho: 60.0,
            alto: 60.0,
            ..Default::default()
        };
        let cortes = cortes_de(&larga, &[caja]);
        // La diagonal entra por la esquina (100,100) y sale por (160,160): dos
        // cortes, aunque cada esquina sea el cruce de dos lados.
        assert_eq!(cortes.len(), 2, "cortes: {cortes:?}");
    }
}
