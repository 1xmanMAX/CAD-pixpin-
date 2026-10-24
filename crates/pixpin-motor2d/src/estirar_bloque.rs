//! **Estirar varios elementos como un bloque** (`resizeMultipleElements` de
//! `motor/Transform.kt` del movil).
//!
//! Hasta ahora, con varios elementos elegidos, cada uno se estiraba por su
//! cuenta hasta el cursor (`transformar::escalar` mide la caja de cada uno):
//! en una grafica, una tabla o una figura de la biblioteca —que llegan
//! agrupadas— todas las piezas acababan con la esquina en el mismo sitio, y
//! la figura quedaba hecha un amasijo. El movil las estira juntas, con el
//! ancla en el borde del marco comun, y eso es lo que se porta aqui.
//!
//! **Por una esquina en proporcion; por un lado, aplastando ese eje**: la
//! esquina conserva la forma y el lado la estruja. La letra crece con la
//! menor de las dos escalas, asi que tirar de un lado no la deforma.

use pixpin_geom::Tirador;

use crate::elemento::{Elemento, Figura};
use crate::transformar::MINIMO;
use crate::vector::Punto2;

/// La caja comun de unos elementos.
fn caja_comun(elementos: &[Elemento]) -> Option<(f32, f32, f32, f32)> {
    elementos.iter().filter(|e| !e.borrado).fold(None, |c, e| {
        let (a, b, x, y) = e.caja();
        Some(match c {
            None => (a, b, x, y),
            Some((c0, c1, c2, c3)) => (c0.min(a), c1.min(b), c2.max(x), c3.max(y)),
        })
    })
}

/// Que ejes mueve el tirador y si su ancla queda a la derecha y abajo
/// (`anchorsRight`, `anchorsBottom`, `affectsX`, `affectsY` del movil).
fn ejes(t: Tirador) -> (bool, bool, bool, bool) {
    use Tirador::*;
    let derecha = matches!(t, NoroesteEsquina | SuroesteEsquina | OesteBorde);
    let abajo = matches!(t, NoroesteEsquina | NoresteEsquina | NorteBorde);
    let mueve_x = !matches!(t, NorteBorde | SurBorde);
    let mueve_y = !matches!(t, EsteBorde | OesteBorde);
    (derecha, abajo, mueve_x, mueve_y)
}

/// **Los `originales` estirados como un bloque** tirando de `tirador` hasta
/// `p`. `proporcional`: `Some(true)` la fuerza (Mayus), `None` la decide el
/// tirador (esquina si, lado no). Se calcula SIEMPRE desde los originales
/// —los del momento de pulsar—, como el movil: aplicado paso a paso, la letra
/// que se encoge al estrechar no volveria a crecer al ensanchar.
pub fn estirar(
    originales: &[Elemento],
    tirador: Tirador,
    p: Punto2,
    proporcional: Option<bool>,
) -> Vec<Elemento> {
    let Some((x0, y0, x1, y1)) = caja_comun(originales) else {
        return originales.to_vec();
    };
    let (ancho, alto) = (x1 - x0, y1 - y0);
    if originales.len() < 2 || ancho <= 0.0 || alto <= 0.0 {
        return originales.to_vec();
    }
    let (derecha, abajo, mueve_x, mueve_y) = ejes(tirador);
    let (mut nx0, mut ny0, mut nx1, mut ny1) = (x0, y0, x1, y1);
    if mueve_x {
        if derecha {
            nx0 = p.x;
        } else {
            nx1 = p.x;
        }
    }
    if mueve_y {
        if abajo {
            ny0 = p.y;
        } else {
            ny1 = p.y;
        }
    }
    // El eje que el tirador no toca no se escala, y pasado el ancla no se
    // voltea: se queda en lo minimo, como en el movil.
    let mut sx = if mueve_x {
        (nx1 - nx0).max(MINIMO) / ancho
    } else {
        1.0
    };
    let mut sy = if mueve_y {
        (ny1 - ny0).max(MINIMO) / alto
    } else {
        1.0
    };
    if proporcional.unwrap_or(mueve_x && mueve_y) {
        // Manda el eje que mas ha cambiado: si no, la seleccion «resbala»
        // cuando el cursor va casi en diagonal.
        let s = if (sx - 1.0).abs() > (sy - 1.0).abs() {
            sx
        } else {
            sy
        };
        sx = s;
        sy = s;
    }
    let ancla = Punto2::nuevo(if derecha { x1 } else { x0 }, if abajo { y1 } else { y0 });
    escalar_desde(originales, ancla, sx, sy, ancla)
}

/// **Los `originales` escalados `sx, sy` desde `ancla`** y llevados de modo
/// que el ancla caiga en `destino`: la cuenta de [`estirar`], suelta para
/// quien tiene que encajar algo en un sitio (una figura en la celda de una
/// tabla). Las trazas escalan sus puntos, los textos su letra con la menor
/// de las dos escalas, y lo bloqueado no se toca.
pub fn escalar_desde(
    originales: &[Elemento],
    ancla: Punto2,
    sx: f32,
    sy: f32,
    destino: Punto2,
) -> Vec<Elemento> {
    let mapa = |q: Punto2| {
        Punto2::nuevo(
            destino.x + (q.x - ancla.x) * sx,
            destino.y + (q.y - ancla.y) * sy,
        )
    };
    let letra = sx.min(sy);
    originales
        .iter()
        .map(|o| {
            let mut e = o.clone();
            if e.bloqueado || e.borrado {
                return e;
            }
            match &mut e.figura {
                Figura::Lapiz { puntos, .. }
                | Figura::Resaltador { puntos }
                | Figura::Linea { puntos }
                | Figura::Flecha { puntos, .. }
                | Figura::Cota { puntos } => {
                    for q in puntos.iter_mut() {
                        *q = mapa(*q);
                    }
                    // Como en `transformar::escalar`: la caja de un trazo sale
                    // de sus puntos, con el margen de su grosor.
                    let (a, b, c, d) = e.caja();
                    (e.x, e.y, e.ancho, e.alto) = (a, b, c - a, d - b);
                }
                Figura::Texto { tam, .. } => {
                    // La letra con la menor de las dos escalas
                    // (`fontSize * minOf(scaleX, scaleY)`), y la caja con
                    // ella: aqui la caja de un texto es lo que mide lo
                    // escrito, y estirada sin la letra se despegaria de el.
                    *tam *= letra;
                    let esquina = mapa(Punto2::nuevo(e.x, e.y));
                    (e.x, e.y) = (esquina.x, esquina.y);
                    e.ancho *= letra;
                    e.alto *= letra;
                }
                _ => {
                    let esquina = mapa(Punto2::nuevo(e.x, e.y));
                    (e.x, e.y) = (esquina.x, esquina.y);
                    e.ancho = (e.ancho * sx).max(MINIMO);
                    e.alto = (e.alto * sy).max(MINIMO);
                }
            }
            // La letra de lo que rotula sin ser un texto (la cota), con
            // la misma escala: en el movil todo `fontSize` crece igual.
            if let Some(t) = e.extras.tam_letra.as_mut() {
                *t *= letra;
            }
            e.tocar();
            e
        })
        .collect()
}

/// **Encaja unos elementos en `caja`**, sin deformarlos: la mayor escala que
/// los deja dentro con `aire` a cada lado, y centrados. Es lo que hace
/// «meter en la celda» con una figura.
pub fn encajar(originales: &[Elemento], caja: (f32, f32, f32, f32), aire: f32) -> Vec<Elemento> {
    let Some((x0, y0, x1, y1)) = caja_comun(originales) else {
        return originales.to_vec();
    };
    let (w, h) = ((x1 - x0).max(MINIMO), (y1 - y0).max(MINIMO));
    let hueco_w = (caja.2 - caja.0 - 2.0 * aire).max(MINIMO);
    let hueco_h = (caja.3 - caja.1 - 2.0 * aire).max(MINIMO);
    let s = (hueco_w / w).min(hueco_h / h);
    let destino = Punto2::nuevo(
        (caja.0 + caja.2) / 2.0 - w * s / 2.0,
        (caja.1 + caja.3) / 2.0 - h * s / 2.0,
    );
    escalar_desde(originales, Punto2::nuevo(x0, y0), s, s, destino)
}
