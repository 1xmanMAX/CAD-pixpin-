//! **Tapar de verdad**: el mosaico, como el del movil.
//!
//! Hasta esta tanda el PC tapaba con una banda opaca. Era un parche de
//! urgencia —y bueno, porque lo de antes era no pintar nada y ENSENAR en el
//! escritorio el numero de cuenta que el usuario habia tapado en el telefono—
//! pero no es lo que hace el movil: alli el mosaico **remuestrea los pixeles
//! de debajo**, en bloques o en mancha, y por eso se ve que hay algo tapado y
//! no un rectangulo gris pegado encima.
//!
//! # Por que la mitad de esto esta aqui y la otra mitad en quien pinta
//!
//! El motor 2D es puro: produce ordenes y no tiene los pixeles de debajo. Asi
//! que este modulo tiene las dos cosas que SI son puras —cuanto mide el
//! grano y que hacer con un cuadro de pixeles— y quien pinta pone los
//! pixeles. La misma receta vale para Direct2D (`pixpin_render::capa_estatica
//! ::tapar`, que lo hace con la GPU reduciendo y volviendo a estirar) y para
//! un PNG de prueba sin GPU (`tapar_rgba`, aqui mismo).
//!
//! # Las dos reglas que no se pueden romper
//!
//! 1. **La opacidad del elemento no se le aplica.** En ningun sitio de este
//!    modulo hay un parametro de opacidad, y eso es deliberado: un mosaico al
//!    20 % no es un mosaico discreto, es un dato legible. Es la unica figura
//!    del lienzo con esta excepcion, y es por lo que existe.
//! 2. **El grano nunca se queda en nada.** Con el zoom muy pequeno el cuadro
//!    del movil mediria menos de un pixel de pantalla y el mosaico dejaria de
//!    tapar justo cuando mas lejos se mira. Por eso `lado_en_pantalla` tiene
//!    suelo.
//!
//! # Hasta donde llega el mosaico, y donde NO llega
//!
//! **El mosaico tapa en pantalla y en todo lo que sale de pintar la escena**
//! —la vista previa del chat, las anotaciones del pin, las miniaturas, el
//! PNG del portapapeles, la lupa—, porque todos esos caminos promedian
//! pixeles ya compuestos. Lo que **no** hace, y hay que decirlo donde se lea,
//! es destruir el original: el `.excalidraw` guarda los elementos tal cual
//! —el texto, la imagen, la pagina del PDF— y **al lado** un elemento
//! `pixpin-mosaic` que dice que hay que taparlos.
//!
//! Eso es deliberado y es lo mismo que hace el movil: el mosaico se puede
//! mover, cambiar de grano y quitar, y para eso tiene que existir lo de
//! debajo. El precio es que **cualquier visor que no conozca `pixpin-mosaic`
//! —excalidraw.com, un editor de terceros, un `jq`— ensena lo tapado**, y ese
//! fichero es exactamente el que viaja por Wi-Fi y el que queda en disco.
//!
//! Se ha decidido **no** escribir la zona ya pixelada: hacerlo convertiria el
//! mosaico en destructivo, romperia la ida y vuelta con el movil —que espera
//! el original debajo— y haria que quitar un mosaico por error fuera
//! irreversible. La alternativa honesta es la otra que queda: **avisar al
//! exportar o compartir fuera del grupo**, y ofrecer el PNG rasterizado, que
//! ese si sale tapado opaco (`ordenes_de_escena`). Ese aviso es de quien
//! ensena la pantalla de compartir, no de este modulo; mientras no exista, lo
//! que el mosaico promete y lo que el fichero cumple no es lo mismo.

use crate::elemento::{Elemento, Figura};

/// **El cuadro del mosaico, en pixeles de la escena, a partir del grosor.**
///
/// El movil **no guarda un campo de celda** (comprobado en su
/// `Element.kt:1086`, `mosaicoGrano(strokeWidth)`): el grano sale del grosor
/// del trazo, que es el mando que ya estaba en el panel. Los cuatro escalones
/// son los suyos, y no se interpolan: un mosaico con grano continuo se veria
/// distinto en cada aparato segun como redondee.
pub fn grano(grosor: f32) -> f32 {
    if grosor <= 1.0 {
        8.0
    } else if grosor <= 2.0 {
        16.0
    } else if grosor <= 4.0 {
        32.0
    } else {
        64.0
    }
}

/// El suelo del cuadro en pixeles de pantalla.
///
/// Dos y no uno: con un solo pixel el «mosaico» seria la imagen original, que
/// es exactamente el fallo que este modulo viene a impedir.
pub const LADO_MINIMO: u32 = 2;

/// Lo que mide el cuadro EN PANTALLA, que es donde se remuestrea.
///
/// El grano del movil va en pixeles de la escena, asi que con el zoom se
/// encoge y se estira como el resto del dibujo —que es lo correcto: el
/// mosaico tiene el mismo grano mirando de cerca que de lejos—. Lo que no
/// puede es desaparecer, de ahi el suelo (regla 2 del modulo).
pub fn lado_en_pantalla(grano_escena: f32, zoom: f32) -> u32 {
    let l = grano_escena * zoom;
    if !l.is_finite() {
        return LADO_MINIMO;
    }
    (l.round().max(LADO_MINIMO as f32) as u32).max(LADO_MINIMO)
}

/// **El gris con el que se tapa cuando no hay pixeles que remuestrear.**
///
/// Es el mismo de la banda maciza de `pintado.rs`, para que el trozo tapado
/// dentro del cristal de la lupa no cante como un parche de otro color.
pub const TAPA_MACIZA: crate::elemento::ColorRgba = crate::elemento::ColorRgba {
    r: 0.42,
    g: 0.42,
    b: 0.45,
    a: 1.0,
};

/// Lo que hay que tapar: una caja del documento y con que grano.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tapado {
    /// `(x0, y0, x1, y1)` en coordenadas del documento.
    pub caja: (f32, f32, f32, f32),
    /// El cuadro, en pixeles del documento.
    pub grano: f32,
    /// `mosaicBlur` del movil: con el puesto se tapa con mancha; sin el, con
    /// bloques.
    pub desenfoque: bool,
}

/// **La caja que de verdad hay que tapar, contando el giro.**
///
/// `Elemento::caja()` devuelve `(x, y, x+ancho, y+alto)` e **ignora el
/// angulo**, pero el mosaico se puede girar como cualquier otro elemento
/// (`tiradores.rs` le da tirador de giro y nada lo excluye) y el marco de
/// seleccion si gira. Tapando la caja sin girar, las cuatro puntas de lo que
/// el usuario quiso tapar se quedaban fuera: un mosaico sobre un texto,
/// girado 30°, dejaba asomar las esquinas del texto.
///
/// Se devuelve la caja **de las cuatro esquinas ya giradas**, que tapa de mas
/// y nunca de menos. De mas es feo; de menos es un numero de cuenta legible.
pub fn caja_girada(e: &Elemento) -> (f32, f32, f32, f32) {
    let (x0, y0, x1, y1) = e.caja();
    if e.angulo == 0.0 || !e.angulo.is_finite() {
        return (x0, y0, x1, y1);
    }
    // El giro va alrededor del centro de la caja CRUDA, que es la convencion
    // del resto del motor (ver `perimetros::contornos_de`).
    let centro =
        crate::vector::Punto2::nuevo((e.x + e.x + e.ancho) / 2.0, (e.y + e.y + e.alto) / 2.0);
    let esquinas = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
        .map(|(x, y)| crate::vector::Punto2::nuevo(x, y).girar(centro, e.angulo));
    (
        esquinas.iter().map(|p| p.x).fold(f32::MAX, f32::min),
        esquinas.iter().map(|p| p.y).fold(f32::MAX, f32::min),
        esquinas.iter().map(|p| p.x).fold(f32::MIN, f32::max),
        esquinas.iter().map(|p| p.y).fold(f32::MIN, f32::max),
    )
}

/// El tapado de un elemento, si es un mosaico que hay que tapar.
///
/// Un mosaico borrado no tapa —no esta en el dibujo—, pero uno **bloqueado o
/// medio transparente si**: la opacidad no le quita el trabajo (regla 1).
pub fn tapado_de(e: &Elemento) -> Option<Tapado> {
    let Figura::Mosaico { desenfoque } = e.figura else {
        return None;
    };
    if e.borrado {
        return None;
    }
    let (x0, y0, x1, y1) = caja_girada(e);
    if !(x1 > x0 && y1 > y0) {
        return None;
    }
    Some(Tapado {
        caja: (x0, y0, x1, y1),
        grano: grano(e.grosor),
        desenfoque,
    })
}

/// **Las cajas de todos los mosaicos de una escena, ya giradas.**
///
/// Es lo que necesita quien pinta algo que pueda ensenar el original por
/// encima del tapado —la lupa— sin tener que conocer `Figura`.
pub fn cajas_tapadas(elementos: &[Elemento]) -> Vec<(f32, f32, f32, f32)> {
    tapados(elementos)
        .into_iter()
        .map(|(_, t)| t.caja)
        .collect()
}

/// **Lo que hay que volver a tapar DENTRO del cristal de una lupa.**
///
/// La lupa amplia el bitmap de origen —la foto congelada, el ultimo
/// fotograma, el bitmap del pin—, y ese bitmap **no tiene las anotaciones**:
/// el mosaico se pinta encima de el, no dentro. Asi que el cristal era,
/// literalmente, una ventana al original por encima del tapado: bastaba
/// pulsar la lupa y pasar el cursor para leer lo tapado.
///
/// Aqui se traduce cada caja de mosaico desde las coordenadas del documento a
/// las del cristal, con el mismo aumento que usa el bitmap, y se recorta al
/// cristal. Quien pinta solo tiene que rellenar esas cajas en opaco **despues**
/// de dibujar el bitmap ampliado.
///
/// `fuente` y `destino` son `(x, y, ancho, alto)`. Una fuente o un destino sin
/// area no da ninguna zona: no hay cristal donde pintar.
pub fn zonas_en_la_lupa(
    cajas: &[(f32, f32, f32, f32)],
    fuente: (f32, f32, f32, f32),
    destino: (f32, f32, f32, f32),
) -> Vec<(f32, f32, f32, f32)> {
    let (fx, fy, fa, fal) = fuente;
    let (dx, dy, da, dal) = destino;
    if !(fa > 0.0 && fal > 0.0 && da > 0.0 && dal > 0.0) {
        return Vec::new();
    }
    if ![fx, fy, fa, fal, dx, dy, da, dal]
        .iter()
        .all(|v| v.is_finite())
    {
        return Vec::new();
    }
    let (kx, ky) = (da / fa, dal / fal);
    cajas
        .iter()
        .filter_map(|&(x0, y0, x1, y1)| {
            // Primero al cristal, y despues se recorta a el. Al reves se
            // perderia el caso del mosaico que entra solo por una esquina.
            let a = (dx + (x0 - fx) * kx, dy + (y0 - fy) * ky);
            let b = (dx + (x1 - fx) * kx, dy + (y1 - fy) * ky);
            // Hacia FUERA, como `recortar`: media fila de pixeles sin tapar
            // en el borde del cristal es media fila legible.
            let zx0 = a.0.min(b.0).floor().max(dx);
            let zy0 = a.1.min(b.1).floor().max(dy);
            let zx1 = a.0.max(b.0).ceil().min(dx + da);
            let zy1 = a.1.max(b.1).ceil().min(dy + dal);
            (zx1 > zx0 && zy1 > zy0).then_some((zx0, zy0, zx1, zy1))
        })
        .collect()
}

/// Todos los tapados de una lista de elementos, **en su orden de pintado**.
///
/// El orden importa y no es un detalle: un mosaico tapa lo que hay DEBAJO, y
/// debajo es lo que se pinto antes que el. Tapar todos al final taparia
/// tambien lo que se dibujo encima a proposito —una flecha senalando el dato
/// tapado acabaria pixelada—.
pub fn tapados(elementos: &[Elemento]) -> Vec<(usize, Tapado)> {
    elementos
        .iter()
        .enumerate()
        .filter_map(|(i, e)| tapado_de(e).map(|t| (i, t)))
        .collect()
}

/// Una pasada de tapado ya traducida a pixeles de pantalla: lo que quien
/// pinta necesita para llamar a `pixpin_render::capa_estatica::tapar`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnPantalla {
    /// El identificador del mosaico, para saltarselo al pintar la escena: un
    /// mosaico que se pixela a si mismo se degrada en cada pasada.
    pub id: u64,
    /// Su sitio en el orden de pintado. Tapar va **justo despues** de pintar
    /// lo que esta antes que el en la lista.
    pub indice: usize,
    /// `(x0, y0, x1, y1)` en pixeles de pantalla, ya recortado al hueco.
    pub zona: (i32, i32, i32, i32),
    pub lado: u32,
    pub desenfoque: bool,
}

/// **El plan de tapado de un fotograma**, en pixeles de pantalla y en orden
/// de pintado.
///
/// Traduce de una vez las dos cosas que quien pinta no deberia tener que
/// recalcular: donde cae cada mosaico en la pantalla y de que tamano le sale
/// el cuadro con el aumento de ahora.
///
/// Los que quedan fuera del hueco no salen en el plan: no hay nada que tapar.
pub fn plan_en_pantalla(
    elementos: &[Elemento],
    camara: &crate::camara::Camara,
    ancho_px: u32,
    alto_px: u32,
) -> Vec<EnPantalla> {
    tapados(elementos)
        .into_iter()
        .filter_map(|(indice, t)| {
            let (x0, y0, x1, y1) = t.caja;
            let a = camara.a_pantalla(crate::vector::Punto2::nuevo(x0, y0));
            let b = camara.a_pantalla(crate::vector::Punto2::nuevo(x1, y1));
            // Se recorta al hueco **hacia fuera**, por lo mismo que
            // `recortar`: media fila de pixeles sin tapar es media fila
            // legible.
            let zx0 = a.x.floor().max(0.0) as i32;
            let zy0 = a.y.floor().max(0.0) as i32;
            let zx1 = (b.x.ceil() as i32).min(ancho_px as i32);
            let zy1 = (b.y.ceil() as i32).min(alto_px as i32);
            if zx1 <= zx0 || zy1 <= zy0 {
                return None;
            }
            Some(EnPantalla {
                id: elementos[indice].id,
                indice,
                zona: (zx0, zy0, zx1, zy1),
                lado: lado_en_pantalla(t.grano, camara.zoom),
                desenfoque: t.desenfoque,
            })
        })
        .collect()
}

/// Un rectangulo de pixeles, ya recortado a lo que de verdad existe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recuadro {
    pub x: u32,
    pub y: u32,
    pub ancho: u32,
    pub alto: u32,
}

/// Recorta `(x0, y0, x1, y1)` —que puede venir fuera o al reves— a lo que cabe
/// en un lienzo de `ancho` x `alto`. `None` si no queda nada.
///
/// Se redondea hacia FUERA (`floor` abajo, `ceil` arriba): media fila de
/// pixeles sin tapar en el borde de un mosaico es media fila de texto legible.
pub fn recortar(caja: (f32, f32, f32, f32), ancho: u32, alto: u32) -> Option<Recuadro> {
    let (ax, ay, bx, by) = caja;
    if !(ax.is_finite() && ay.is_finite() && bx.is_finite() && by.is_finite()) {
        return None;
    }
    let x0 = ax.min(bx).floor().max(0.0) as i64;
    let y0 = ay.min(by).floor().max(0.0) as i64;
    let x1 = (ax.max(bx).ceil() as i64).min(ancho as i64);
    let y1 = (ay.max(by).ceil() as i64).min(alto as i64);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    Some(Recuadro {
        x: x0 as u32,
        y: y0 as u32,
        ancho: (x1 - x0) as u32,
        alto: (y1 - y0) as u32,
    })
}

/// El cuadro que de verdad se usa sobre `r`.
///
/// Nunca cero —sin cuadros no hay promedio— ni mayor que la region, que es el
/// caso de una caja mas baja que su propio grano: ahi el mosaico es una sola
/// fila de cuadros, y tapa igual. Quien mire el resultado a cuadros necesita
/// este numero y no el pedido, o buscara una rejilla que no esta.
pub fn lado_efectivo(lado: u32, r: Recuadro) -> u32 {
    lado.max(LADO_MINIMO).min(r.ancho.max(1)).min(r.alto.max(1))
}

/// **Tapa un trozo de un mapa de bits RGBA, en el sitio.**
///
/// Es la receta entera del movil y cabe en un parrafo: se parte la region en
/// cuadros de `lado`, se promedia cada cuadro, y se devuelve ese promedio a
/// todos sus pixeles (bloques) o repartido con suavidad entre los centros de
/// los cuadros vecinos (mancha).
///
/// `pixeles` va fila tras fila, cuatro bytes por pixel. El alfa se promedia
/// como un canal mas: un mosaico sobre un trozo medio transparente tiene que
/// quedar medio transparente igual, o aparecerian bordes opacos donde no los
/// habia.
///
/// **No hay parametro de opacidad** (regla 1 del modulo).
pub fn tapar_rgba(
    pixeles: &mut [u8],
    ancho: u32,
    alto: u32,
    caja: (f32, f32, f32, f32),
    lado: u32,
    desenfoque: bool,
) -> bool {
    if pixeles.len() < (ancho as usize * alto as usize * 4) {
        return false;
    }
    let Some(r) = recortar(caja, ancho, alto) else {
        return false;
    };
    let lado = lado_efectivo(lado, r);
    let cuadros_x = r.ancho.div_ceil(lado);
    let cuadros_y = r.alto.div_ceil(lado);

    // Primero TODOS los promedios, y despues se escribe. Promediar y escribir
    // a la vez arrastraria el cuadro ya tapado al siguiente y el mosaico
    // saldria emborronado hacia la derecha.
    let mut medias = vec![[0f32; 4]; (cuadros_x * cuadros_y) as usize];
    for cy in 0..cuadros_y {
        for cx in 0..cuadros_x {
            let x0 = r.x + cx * lado;
            let y0 = r.y + cy * lado;
            let x1 = (x0 + lado).min(r.x + r.ancho);
            let y1 = (y0 + lado).min(r.y + r.alto);
            let mut suma = [0f32; 4];
            let mut n = 0f32;
            for y in y0..y1 {
                let fila = (y as usize * ancho as usize) * 4;
                for x in x0..x1 {
                    let i = fila + x as usize * 4;
                    for (s, p) in suma.iter_mut().zip(&pixeles[i..i + 4]) {
                        *s += *p as f32;
                    }
                    n += 1.0;
                }
            }
            if n > 0.0 {
                suma.iter_mut().for_each(|s| *s /= n);
            }
            medias[(cy * cuadros_x + cx) as usize] = suma;
        }
    }

    let media = |cx: i64, cy: i64| -> [f32; 4] {
        let cx = cx.clamp(0, cuadros_x as i64 - 1) as u32;
        let cy = cy.clamp(0, cuadros_y as i64 - 1) as u32;
        medias[(cy * cuadros_x + cx) as usize]
    };

    for y in r.y..r.y + r.alto {
        let fila = (y as usize * ancho as usize) * 4;
        for x in r.x..r.x + r.ancho {
            let i = fila + x as usize * 4;
            let cx = (x - r.x) / lado;
            let cy = (y - r.y) / lado;
            let v = if !desenfoque {
                media(cx as i64, cy as i64)
            } else {
                // La mancha es el mismo promedio por cuadros, interpolado
                // entre los CENTROS de los cuatro cuadros vecinos: es lo que
                // sale de reducir y volver a estirar con filtro lineal, que
                // es como lo hace la GPU, y asi las dos mitades de este
                // modulo dibujan lo mismo.
                let fx = (x - r.x) as f32 / lado as f32 - 0.5;
                let fy = (y - r.y) as f32 / lado as f32 - 0.5;
                let (ix, iy) = (fx.floor(), fy.floor());
                let (tx, ty) = (fx - ix, fy - iy);
                let (ix, iy) = (ix as i64, iy as i64);
                let (ai, bi) = (media(ix, iy), media(ix + 1, iy));
                let (ci, di) = (media(ix, iy + 1), media(ix + 1, iy + 1));
                let mut v = [0f32; 4];
                for (c, salida) in v.iter_mut().enumerate() {
                    let arriba = ai[c] * (1.0 - tx) + bi[c] * tx;
                    let abajo = ci[c] * (1.0 - tx) + di[c] * tx;
                    *salida = arriba * (1.0 - ty) + abajo * ty;
                }
                v
            };
            for (p, nuevo) in pixeles[i..i + 4].iter_mut().zip(v) {
                *p = nuevo.round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    true
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::ColorRgba;

    fn mosaico(grosor: f32, desenfoque: bool) -> Elemento {
        Elemento {
            id: 1,
            figura: Figura::Mosaico { desenfoque },
            x: 10.0,
            y: 20.0,
            ancho: 80.0,
            alto: 40.0,
            grosor,
            ..Default::default()
        }
    }

    #[test]
    fn el_grano_sale_del_grosor_en_los_cuatro_escalones_del_movil() {
        // Los mismos numeros de `Element.kt:1086`. Si aqui saliera otro, el
        // mismo mosaico tendria un cuadro en el telefono y otro en el PC.
        assert_eq!(grano(0.5), 8.0);
        assert_eq!(grano(1.0), 8.0);
        assert_eq!(grano(1.5), 16.0);
        assert_eq!(grano(2.0), 16.0);
        assert_eq!(grano(3.0), 32.0);
        assert_eq!(grano(4.0), 32.0);
        assert_eq!(grano(4.1), 64.0);
        assert_eq!(grano(100.0), 64.0);
    }

    #[test]
    fn el_cuadro_no_se_queda_en_nada_por_mucho_que_se_aleje_la_camara() {
        // **La prueba del zoom raro.** A 0,01 aumentos el cuadro del movil
        // mediria 0,08 pixeles de pantalla: redondeado seria cero, y un
        // mosaico de cuadro cero es la imagen original sin tocar.
        assert_eq!(lado_en_pantalla(8.0, 0.01), LADO_MINIMO);
        assert_eq!(lado_en_pantalla(64.0, 0.0), LADO_MINIMO);
        assert_eq!(lado_en_pantalla(8.0, f32::NAN), LADO_MINIMO);
        // Y con la camara encima, el cuadro crece con el dibujo.
        assert_eq!(lado_en_pantalla(8.0, 4.0), 32);
    }

    #[test]
    fn un_mosaico_girado_tapa_las_cuatro_puntas_y_no_su_caja_sin_girar() {
        // **El fallo:** `Elemento::caja()` ignora el angulo, pero el mosaico
        // lleva tirador de giro como cualquier otro y el marco de seleccion
        // SI giraba. Tapando la caja cruda, las cuatro puntas de lo que el
        // usuario quiso tapar asomaban por las esquinas.
        let mut e = mosaico(2.0, false);
        // Cuadrado, para que el cuarto de vuelta se vea de un vistazo.
        e.ancho = 40.0;
        e.alto = 40.0;
        let sin_girar = caja_girada(&e);
        assert_eq!(sin_girar, (10.0, 20.0, 50.0, 60.0));

        // Un cuarto de vuelta de un cuadrado da la MISMA caja: si esto
        // cambiara, el giro estaria tomando otro centro.
        e.angulo = std::f32::consts::FRAC_PI_2;
        let cuarto = caja_girada(&e);
        for (a, b) in [
            (cuarto.0, 10.0),
            (cuarto.1, 20.0),
            (cuarto.2, 50.0),
            (cuarto.3, 60.0),
        ] {
            assert!(
                (a - b).abs() < 1e-3,
                "el cuarto de vuelta movio la caja: {cuarto:?}"
            );
        }

        // A 45° la caja crece por las puntas: es justo lo que antes se
        // quedaba fuera. Un cuadrado de lado 40 girado 45° mide 40*sqrt(2).
        e.angulo = std::f32::consts::FRAC_PI_4;
        let (x0, y0, x1, y1) = caja_girada(&e);
        let diagonal = 40.0 * std::f32::consts::SQRT_2;
        assert!(
            (x1 - x0 - diagonal).abs() < 1e-2 && (y1 - y0 - diagonal).abs() < 1e-2,
            "la caja girada mide {}x{} y tenia que medir {diagonal}",
            x1 - x0,
            y1 - y0
        );
        // Y **contiene** a la de antes: tapar de mas es feo, tapar de menos
        // es un numero de cuenta legible.
        assert!(x0 <= sin_girar.0 && y0 <= sin_girar.1);
        assert!(x1 >= sin_girar.2 && y1 >= sin_girar.3);

        // El caso negativo: sin angulo no se toca nada, para no tapar de mas
        // en el caso normal, que es el de casi todos los mosaicos.
        e.angulo = 0.0;
        assert_eq!(caja_girada(&e), sin_girar);
        // Un angulo imposible tampoco descoloca la caja.
        e.angulo = f32::NAN;
        assert_eq!(caja_girada(&e), sin_girar);
    }

    #[test]
    fn el_plan_de_pantalla_de_un_mosaico_girado_cubre_sus_esquinas() {
        // La misma promesa, ya traducida a pixeles: `plan_en_pantalla` es lo
        // que usa quien pinta con GPU, y tiene que llevar la caja girada.
        let mut e = mosaico(2.0, false);
        e.ancho = 40.0;
        e.alto = 40.0;
        let camara = crate::camara::Camara::default();
        let recto = plan_en_pantalla(std::slice::from_ref(&e), &camara, 400, 400);
        e.angulo = std::f32::consts::FRAC_PI_4;
        let girado = plan_en_pantalla(std::slice::from_ref(&e), &camara, 400, 400);
        assert_eq!(recto.len(), 1);
        assert_eq!(girado.len(), 1);
        let (rx0, ry0, rx1, ry1) = recto[0].zona;
        let (gx0, gy0, gx1, gy1) = girado[0].zona;
        assert!(
            gx0 <= rx0 && gy0 <= ry0 && gx1 >= rx1 && gy1 >= ry1,
            "la zona girada {:?} no cubre la recta {:?}",
            girado[0].zona,
            recto[0].zona
        );
        assert!(
            gx1 - gx0 > rx1 - rx0,
            "a 45° la zona tiene que ser mas ancha, no igual"
        );
    }

    #[test]
    fn un_mosaico_medio_transparente_o_bloqueado_sigue_teniendo_que_taparse() {
        // Caso negativo del que mas importa: la opacidad no le quita el
        // trabajo. `Tapado` ni siquiera la lleva, que es la garantia de
        // verdad; esto comprueba que ademas no se filtra por ella.
        let mut e = mosaico(2.0, false);
        e.opacidad = 0.2;
        e.bloqueado = true;
        let t = tapado_de(&e).expect("un mosaico al 20 % sigue tapando");
        assert_eq!(t.grano, 16.0);
        assert_eq!(t.caja, (10.0, 20.0, 90.0, 60.0));
    }

    #[test]
    fn lo_que_no_es_un_mosaico_o_ya_no_esta_no_tapa_nada() {
        let mut e = mosaico(2.0, false);
        e.figura = Figura::Rectangulo;
        assert!(tapado_de(&e).is_none(), "un rectangulo no tapa");
        let mut e = mosaico(2.0, false);
        e.borrado = true;
        assert!(tapado_de(&e).is_none(), "lo borrado no esta en el dibujo");
        let mut e = mosaico(2.0, false);
        e.ancho = 0.0;
        assert!(tapado_de(&e).is_none(), "una caja sin area no tapa nada");
    }

    #[test]
    fn los_tapados_salen_en_el_orden_de_pintado_y_con_su_sitio() {
        // Un mosaico tapa lo de DEBAJO. Sin el indice, quien pinta no sabe
        // cuando aplicarlo y acabaria pixelando la flecha que senala el dato.
        let elementos = vec![
            Elemento {
                id: 1,
                ..Default::default()
            },
            mosaico(1.0, true),
            Elemento {
                id: 3,
                ..Default::default()
            },
            mosaico(8.0, false),
        ];
        let t = tapados(&elementos);
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].0, 1);
        assert_eq!(t[0].1.grano, 8.0);
        assert!(t[0].1.desenfoque);
        assert_eq!(t[1].0, 3);
        assert_eq!(t[1].1.grano, 64.0);
    }

    #[test]
    fn el_plan_traduce_caja_y_cuadro_al_aumento_de_ahora() {
        let mut e = mosaico(2.0, true);
        e.id = 7;
        let camara = crate::camara::Camara {
            x: 10.0,
            y: 20.0,
            zoom: 2.0,
        };
        let plan = plan_en_pantalla(&[e], &camara, 1000, 1000);
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].id, 7);
        // La caja del documento era (10,20)-(90,60): en pantalla, x2 desde
        // esa esquina, sale (0,0)-(160,80).
        assert_eq!(plan[0].zona, (0, 0, 160, 80));
        // Y el cuadro de 16 del documento mide 32 en pantalla.
        assert_eq!(plan[0].lado, 32);
        assert!(plan[0].desenfoque);
    }

    #[test]
    fn un_mosaico_que_se_salio_de_la_pantalla_no_entra_en_el_plan() {
        // Caso negativo: pedirle a la GPU que copie un rectangulo de fuera
        // del destino es lo que tumba el fotograma.
        let camara = crate::camara::Camara {
            x: 5_000.0,
            y: 5_000.0,
            zoom: 1.0,
        };
        assert!(plan_en_pantalla(&[mosaico(2.0, false)], &camara, 800, 600).is_empty());
        // Y uno que asoma a medias entra recortado al hueco.
        let camara = crate::camara::Camara {
            x: 50.0,
            y: 30.0,
            zoom: 1.0,
        };
        let plan = plan_en_pantalla(&[mosaico(2.0, false)], &camara, 20, 20);
        assert_eq!(plan[0].zona, (0, 0, 20, 20));
    }

    #[test]
    fn la_caja_se_recorta_a_lo_que_existe_y_hacia_fuera() {
        // Hacia fuera: media fila sin tapar en el borde es media fila de
        // texto legible.
        assert_eq!(
            recortar((10.4, 20.6, 30.1, 40.2), 100, 100),
            Some(Recuadro {
                x: 10,
                y: 20,
                ancho: 21,
                alto: 21
            })
        );
        // Medio fuera por la izquierda y por arriba.
        assert_eq!(
            recortar((-50.0, -50.0, 10.0, 10.0), 100, 100),
            Some(Recuadro {
                x: 0,
                y: 0,
                ancho: 10,
                alto: 10
            })
        );
        // Del todo fuera, al reves, o sin area: no hay nada que tapar.
        assert_eq!(recortar((200.0, 200.0, 300.0, 300.0), 100, 100), None);
        assert_eq!(recortar((5.0, 5.0, 5.0, 40.0), 100, 100), None);
        assert_eq!(recortar((f32::NAN, 0.0, 10.0, 10.0), 100, 100), None);
    }

    /// Un lienzo de damero de un pixel: el peor caso para tapar, porque
    /// cualquier resto de detalle se ve.
    fn damero(ancho: u32, alto: u32) -> Vec<u8> {
        let mut v = vec![0u8; (ancho * alto * 4) as usize];
        for y in 0..alto {
            for x in 0..ancho {
                let i = ((y * ancho + x) * 4) as usize;
                let c = if (x + y) % 2 == 0 { 0 } else { 255 };
                v[i] = c;
                v[i + 1] = c;
                v[i + 2] = c;
                v[i + 3] = 255;
            }
        }
        v
    }

    fn pix(v: &[u8], ancho: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * ancho + x) * 4) as usize;
        [v[i], v[i + 1], v[i + 2], v[i + 3]]
    }

    #[test]
    fn tapar_con_bloques_deja_cada_cuadro_de_un_solo_color() {
        let (w, h) = (32u32, 32u32);
        let mut v = damero(w, h);
        assert!(tapar_rgba(&mut v, w, h, (0.0, 0.0, 32.0, 32.0), 8, false));
        // Dentro de un cuadro no queda ni un pixel distinto de otro: eso es
        // que no queda detalle que leer.
        for cy in 0..4 {
            for cx in 0..4 {
                let base = pix(&v, w, cx * 8, cy * 8);
                for y in 0..8 {
                    for x in 0..8 {
                        assert_eq!(
                            pix(&v, w, cx * 8 + x, cy * 8 + y),
                            base,
                            "el cuadro ({cx},{cy}) conserva detalle"
                        );
                    }
                }
            }
        }
        // Y el promedio de un damero es gris medio, no negro ni blanco.
        assert!((126..=129).contains(&pix(&v, w, 3, 3)[0]));
    }

    #[test]
    fn tapar_no_toca_ni_un_pixel_de_fuera_de_la_caja() {
        // Caso negativo: un mosaico que se come el borde de lo que no tapaba
        // en el movil dibuja un fichero distinto en cada aparato.
        let (w, h) = (32u32, 32u32);
        let original = damero(w, h);
        let mut v = original.clone();
        assert!(tapar_rgba(&mut v, w, h, (8.0, 8.0, 24.0, 24.0), 8, false));
        for y in 0..h {
            for x in 0..w {
                let dentro = (8..24).contains(&x) && (8..24).contains(&y);
                if !dentro {
                    assert_eq!(
                        pix(&v, w, x, y),
                        pix(&original, w, x, y),
                        "toco el pixel ({x},{y}), que esta fuera"
                    );
                }
            }
        }
    }

    #[test]
    fn un_cuadro_mas_grande_que_la_caja_la_deja_de_un_color_y_no_se_cae() {
        // Con el zoom muy metido el cuadro puede pasarse de largo. No es un
        // error: es un mosaico de un solo cuadro, que tapa igual.
        let (w, h) = (16u32, 16u32);
        let mut v = damero(w, h);
        assert!(tapar_rgba(&mut v, w, h, (2.0, 2.0, 10.0, 6.0), 4096, false));
        let base = pix(&v, w, 2, 2);
        for y in 2..6 {
            for x in 2..10 {
                assert_eq!(pix(&v, w, x, y), base);
            }
        }
    }

    #[test]
    fn la_mancha_suaviza_pero_tampoco_deja_leer_el_damero() {
        // El desenfoque no tiene por que dejar cuadros planos, pero SI tiene
        // que haberse comido el detalle de un pixel: ningun pixel puede
        // seguir siendo negro puro ni blanco puro.
        let (w, h) = (32u32, 32u32);
        let mut v = damero(w, h);
        assert!(tapar_rgba(&mut v, w, h, (0.0, 0.0, 32.0, 32.0), 8, true));
        for y in 0..h {
            for x in 0..w {
                let c = pix(&v, w, x, y)[0];
                assert!(
                    (100..=155).contains(&c),
                    "el pixel ({x},{y}) vale {c}: queda detalle"
                );
            }
        }
    }

    #[test]
    fn tapar_una_caja_de_fuera_o_un_buffer_corto_no_entra_en_panico() {
        let (w, h) = (8u32, 8u32);
        let mut v = damero(w, h);
        assert!(!tapar_rgba(
            &mut v,
            w,
            h,
            (100.0, 100.0, 200.0, 200.0),
            4,
            false
        ));
        let mut corto = vec![0u8; 4];
        assert!(!tapar_rgba(
            &mut corto,
            w,
            h,
            (0.0, 0.0, 8.0, 8.0),
            4,
            false
        ));
    }

    #[test]
    fn el_alfa_se_promedia_como_un_canal_mas() {
        // Si el alfa no se promediara, un mosaico sobre un trozo transparente
        // saldria opaco y taparia ademas lo que hubiera debajo del todo.
        let (w, h) = (4u32, 4u32);
        let mut v = vec![0u8; (w * h * 4) as usize];
        for i in 0..(w * h) as usize {
            v[i * 4] = 200;
            v[i * 4 + 3] = if i % 2 == 0 { 0 } else { 200 };
        }
        assert!(tapar_rgba(&mut v, w, h, (0.0, 0.0, 4.0, 4.0), 4, false));
        assert_eq!(pix(&v, w, 0, 0)[3], 100, "el alfa medio de 0 y 200");
        // Y el color sigue siendo el que era: solo se promedio.
        assert_eq!(pix(&v, w, 0, 0)[0], 200);
    }

    #[test]
    fn el_color_del_elemento_no_pinta_nada_en_el_tapado() {
        // Un mosaico no tiene color propio: lo que ensena son los pixeles de
        // debajo remuestreados. Que `Tapado` no lleve color es la garantia.
        let mut e = mosaico(2.0, false);
        e.relleno = Some(ColorRgba::opaco(1.0, 0.0, 0.0));
        e.trazo = ColorRgba::opaco(0.0, 1.0, 0.0);
        let t = tapado_de(&e).unwrap();
        assert_eq!(t.grano, 16.0);
        assert!(!t.desenfoque);
    }
}
