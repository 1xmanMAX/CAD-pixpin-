//! El relleno de una figura cerrada: solido o rayado.
//!
//! Excalidraw no rellena de un color plano salvo que se lo pidas. Por omision
//! raya el interior en diagonal (`fillStyle: "hachure"`), y ese rayado es
//! parte de como se lee un dibujo suyo: lo que esta relleno se distingue de lo
//! que no sin tapar lo que hay debajo. Hasta ahora aqui solo habia relleno
//! solido, asi que un plano rayado del movil se abria como una mancha de
//! color.
//!
//! # Los numeros, y de donde salen
//!
//! Son los de Excalidraw sobre `rough.js`, que es quien dibuja sus figuras:
//!
//! - **El angulo, -41 grados.** Es el `hachureAngle` por omision de rough.js.
//!   Un numero raro a proposito: 45 exactos se alinean con las diagonales de
//!   los rectangulos y con la rejilla de pixeles, y el rayado se ve como un
//!   moire en vez de como rayas.
//! - **La separacion, cuatro veces el grosor del trazo.** Excalidraw la
//!   calcula asi (`hachureGap: strokeWidth * 4`) en vez de dejar la de
//!   rough.js: asi una figura de trazo grueso se raya mas suelta y no se
//!   convierte en un relleno solido a la fuerza.
//! - **La pluma del rayado, la mitad del grosor** (`fillWeight:
//!   strokeWidth / 2`): las rayas del relleno son mas finas que el contorno,
//!   o el interior pesaria mas que la figura.
//! - **El cruzado son dos rayados**, el segundo a noventa grados del primero.
//!   Es literalmente lo que hace `HatchFiller` de rough.js.
//!
//! # Por que se recorta a la figura y no a su caja
//!
//! Porque una elipse rayada hasta las esquinas de su caja no es una elipse
//! rayada: es un cuadrado rayado con una elipse encima. El recorte se hace
//! con barrido por lineas sobre el contorno de la figura, que sirve igual
//! para el rectangulo y para la elipse -y para cualquier contorno cerrado que
//! venga despues.

use serde::{Deserialize, Serialize};

use crate::azar::Azar;
use crate::vector::Punto2;

/// Como se rellena el interior de una figura cerrada.
///
/// El valor por omision es **rayado** y no solido: es el de Excalidraw
/// (`currentItemFillStyle: "hachure"` en su `appState`), y este formato es el
/// que hablan los dos lados del puente. Un elemento del movil sin `fillStyle`
/// lo dibuja Excalidraw rayado, asi que aqui tiene que salir rayado tambien o
/// el mismo fichero se veria de dos maneras.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EstiloRelleno {
    Solido,
    #[default]
    Rayado,
    Cruzado,
    /// Galones encadenados (`zigzag` de Excalidraw): ver [`lineas_de_zigzag`].
    Zigzag,
    /// El rayado limpio, sin temblor (`pixpin-lines`, propio de PixPin): ver
    /// [`lineas_a_tiralineas`].
    #[serde(rename = "lineaspixpin")]
    LineasPixpin,
}

/// El angulo del rayado, en grados. El `hachureAngle` de rough.js.
pub const ANGULO_RAYADO: f32 = -41.0;

/// La separacion entre rayas es este multiplo del grosor del trazo, como el
/// `hachureGap` que fija Excalidraw.
pub const FACTOR_SEPARACION: f32 = 4.0;

/// Separacion entre rayas para un grosor de trazo dado.
///
/// Con suelo: una separacion de cero -o negativa, si llega un grosor
/// disparatado- seria un bucle infinito de rayas.
pub fn separacion(grosor: f32) -> f32 {
    (grosor * FACTOR_SEPARACION).max(1.0)
}

/// El grosor de las rayas: la mitad del trazo (`fillWeight`), nunca por
/// debajo de un pixel, o a tamano pequeno el relleno desapareceria.
pub fn grosor_de_rayado(grosor: f32) -> f32 {
    (grosor / 2.0).max(1.0)
}

/// Las rayas del relleno de una figura, recortadas a la figura.
///
/// `caja` es `(x, y, ancho, alto)`; `elipse` dice si la figura inscrita en esa
/// caja es una elipse o el rectangulo entero. `rugosidad` y `azar` son los del
/// elemento: mismo elemento, mismas rayas, en cualquier apertura (D38).
///
/// Un estilo solido no tiene rayas y devuelve la lista vacia: el relleno
/// plano lo sigue haciendo `Orden::Relleno`, aqui no hay nada que hacer.
///
/// **Es la unica puerta**: las cinco tramas se reparten aqui dentro, y no en
/// quien pinta. Con el reparto en `pintado.rs`, una trama nueva compilaria sin
/// tocar nada —el `match` de alla tiene brazo comodin para el grosor y la
/// separacion— y saldria como un rayado normal sin que nadie se enterara. Con
/// el reparto aqui, el `match` es exhaustivo y el compilador pregunta.
///
/// `lineas_a_tiralineas` no gasta ni un numero de `azar` y eso se conserva al
/// pasar por aqui: lo que decide si el garabato del contorno sale igual es
/// cuantos numeros se consumieron antes, no quien los pidio.
pub fn lineas_de_rayado(
    caja: (f32, f32, f32, f32),
    elipse: bool,
    estilo: EstiloRelleno,
    grosor: f32,
    rugosidad: f32,
    azar: &mut Azar,
) -> Vec<(Punto2, Punto2)> {
    let (_, _, ancho, alto) = caja;
    if estilo == EstiloRelleno::Solido || ancho.abs() < 1e-3 || alto.abs() < 1e-3 {
        return Vec::new();
    }
    match estilo {
        EstiloRelleno::Zigzag => return lineas_de_zigzag(caja, elipse, grosor, rugosidad, azar),
        EstiloRelleno::LineasPixpin => return lineas_a_tiralineas(caja, elipse, grosor),
        EstiloRelleno::Solido | EstiloRelleno::Rayado | EstiloRelleno::Cruzado => {}
    }
    let contorno = contorno(caja, elipse);
    let sep = separacion(grosor);

    // El cruzado es el mismo rayado dos veces, el segundo a noventa grados.
    let angulos: &[f32] = match estilo {
        EstiloRelleno::Cruzado => &[ANGULO_RAYADO, ANGULO_RAYADO + 90.0],
        _ => &[ANGULO_RAYADO],
    };

    let mut salida = Vec::new();
    for grados in angulos {
        for (a, b) in barrido(&contorno, *grados, sep) {
            salida.push(temblar(a, b, rugosidad, azar));
        }
    }
    salida
}

/// **El relleno en zigzag** (`zigzag` de Excalidraw, `renderZigzag` de
/// `Rough.kt:464-478`).
///
/// Es el mismo barrido del rayado, pero cada raya se dibuja **dos veces
/// desplazada media separacion a un lado y al otro**, las dos acabando en el
/// mismo punto. Lo que sale son galones encadenados: el relleno de «a mano»
/// que no se confunde con un rayado tecnico.
///
/// Se llega aqui por [`EstiloRelleno::Zigzag`] y por [`lineas_de_rayado`],
/// que es quien reparte; sigue siendo publica porque una prueba quiere
/// medir los galones sin montar un elemento entero.
pub fn lineas_de_zigzag(
    caja: (f32, f32, f32, f32),
    elipse: bool,
    grosor: f32,
    rugosidad: f32,
    azar: &mut Azar,
) -> Vec<(Punto2, Punto2)> {
    let (_, _, ancho, alto) = caja;
    if ancho.abs() < 1e-3 || alto.abs() < 1e-3 {
        return Vec::new();
    }
    let sep = separacion(grosor);
    let angulo = ANGULO_RAYADO.to_radians();
    // Medio hueco a cada lado: con el entero los galones se pegarian al de la
    // fila de al lado y el relleno se leeria como un rayado doble.
    let (dx, dy) = (sep * 0.5 * angulo.cos(), sep * 0.5 * angulo.sin());

    let mut salida = Vec::new();
    for (a, b) in barrido(&contorno(caja, elipse), ANGULO_RAYADO, sep) {
        if a.distancia(b) <= f32::EPSILON {
            continue;
        }
        // La ida y la vuelta del galon: las dos ramas nacen separadas y se
        // juntan en el mismo extremo.
        let ida = Punto2::nuevo(a.x - dx, a.y + dy);
        let vuelta = Punto2::nuevo(a.x + dx, a.y - dy);
        salida.push(temblar(ida, b, rugosidad, azar));
        salida.push(temblar(vuelta, b, rugosidad, azar));
    }
    salida
}

/// **El rayado a tiralineas** (`pixpin-lines`, propio de PixPin).
///
/// Las mismas rayas del barrido **sin pasar por el generador de ruido**. No
/// esta en Excalidraw y por eso lleva prefijo: el rayado del original imita el
/// rotulador, y eso es lo que se quiere en un esquema; pero rellenando la
/// seccion de un plano o el hueco entre dos piezas, un rayado tembloroso «se
/// lee como suciedad» (`Element.kt:383-392`).
///
/// No toma `azar` a proposito: **no gasta ni un numero del generador**. Es lo
/// que garantiza que sea reproducible y, de paso, lo que hace que el garabato
/// del contorno salga como si la figura no tuviera relleno.
pub fn lineas_a_tiralineas(
    caja: (f32, f32, f32, f32),
    elipse: bool,
    grosor: f32,
) -> Vec<(Punto2, Punto2)> {
    let (_, _, ancho, alto) = caja;
    if ancho.abs() < 1e-3 || alto.abs() < 1e-3 {
        return Vec::new();
    }
    barrido(&contorno(caja, elipse), ANGULO_RAYADO, separacion(grosor))
}

/// El contorno cerrado de la figura inscrita en la caja.
fn contorno((x, y, ancho, alto): (f32, f32, f32, f32), elipse: bool) -> Vec<Punto2> {
    if !elipse {
        return vec![
            Punto2::nuevo(x, y),
            Punto2::nuevo(x + ancho, y),
            Punto2::nuevo(x + ancho, y + alto),
            Punto2::nuevo(x, y + alto),
        ];
    }
    // Suficientes lados para que el borde del rayado no se vea poligonal:
    // con 64, el error de la cuerda es del orden de una milesima del radio.
    const LADOS: usize = 64;
    let (cx, cy) = (x + ancho / 2.0, y + alto / 2.0);
    let (rx, ry) = (ancho / 2.0, alto / 2.0);
    (0..LADOS)
        .map(|i| {
            let t = std::f32::consts::TAU * i as f32 / LADOS as f32;
            Punto2::nuevo(cx + rx * t.cos(), cy + ry * t.sin())
        })
        .collect()
}

/// Barrido por lineas: los tramos de recta que caen DENTRO del contorno.
///
/// Se gira el contorno para que las rayas queden horizontales, se corta con
/// rectas separadas `sep`, y los cortes se giran de vuelta. Girar el problema
/// en vez de la solucion evita tener que resolver a mano la interseccion de
/// una recta inclinada con cada lado.
fn barrido(contorno: &[Punto2], grados: f32, sep: f32) -> Vec<(Punto2, Punto2)> {
    let angulo = grados.to_radians();
    let centro = centro_de(contorno);
    let girado: Vec<Punto2> = contorno.iter().map(|p| p.girar(centro, -angulo)).collect();

    let y0 = girado.iter().map(|p| p.y).fold(f32::MAX, f32::min);
    let y1 = girado.iter().map(|p| p.y).fold(f32::MIN, f32::max);
    if !y0.is_finite() || !y1.is_finite() {
        return Vec::new();
    }

    let mut salida = Vec::new();
    // Se empieza a media separacion del borde: arrancar justo en el minimo
    // dejaria una raya de longitud cero pegada al contorno.
    let mut y = y0 + sep / 2.0;
    while y < y1 {
        let mut cortes = Vec::new();
        for i in 0..girado.len() {
            let (a, b) = (girado[i], girado[(i + 1) % girado.len()]);
            let (arriba, abajo) = (a.y.min(b.y), a.y.max(b.y));
            // Media abierta: un vertice se cuenta una sola vez, o al pasar la
            // raya justo por el se contarian dos cortes y el tramo de dentro
            // saldria marcado como de fuera.
            if y < arriba || y >= abajo || (b.y - a.y).abs() < f32::EPSILON {
                continue;
            }
            cortes.push(a.x + (y - a.y) / (b.y - a.y) * (b.x - a.x));
        }
        cortes.sort_by(|p, q| p.partial_cmp(q).unwrap_or(std::cmp::Ordering::Equal));
        // Par a par: entre el primer corte y el segundo se esta dentro, entre
        // el segundo y el tercero fuera... (regla par-impar).
        for par in cortes.chunks_exact(2) {
            if par[1] - par[0] < 1e-3 {
                continue;
            }
            let a = Punto2::nuevo(par[0], y).girar(centro, angulo);
            let b = Punto2::nuevo(par[1], y).girar(centro, angulo);
            salida.push((a, b));
        }
        y += sep;
    }
    salida
}

fn centro_de(puntos: &[Punto2]) -> Punto2 {
    let x0 = puntos.iter().map(|p| p.x).fold(f32::MAX, f32::min);
    let x1 = puntos.iter().map(|p| p.x).fold(f32::MIN, f32::max);
    let y0 = puntos.iter().map(|p| p.y).fold(f32::MAX, f32::min);
    let y1 = puntos.iter().map(|p| p.y).fold(f32::MIN, f32::max);
    Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0)
}

/// Desvia los extremos de una raya para que no parezca impresa.
///
/// Una sola pasada, al contrario que `formas::linea`: el contorno son cuatro
/// lineas y el rayado son decenas, y repasarlas todas dos veces multiplicaria
/// la geometria del dibujo sin que se note. El desvio es el mismo que el de
/// una linea suelta, y con rugosidad cero no hay ninguno.
fn temblar(a: Punto2, b: Punto2, rugosidad: f32, azar: &mut Azar) -> (Punto2, Punto2) {
    if rugosidad <= 0.0 {
        return (a, b);
    }
    let d = (a.distancia(b) / 40.0).clamp(1.0, 4.0) * rugosidad;
    (
        Punto2::nuevo(a.x + azar.desvio(d), a.y + azar.desvio(d)),
        Punto2::nuevo(b.x + azar.desvio(d), b.y + azar.desvio(d)),
    )
}

/// Recorta el segmento `a`-`b` al poligono CONVEXO `tapa`, dado en sentido
/// horario o antihorario indistintamente.
///
/// Hace falta porque el rayado se genera siempre para la caja de la figura, y
/// hay figuras que no llenan su caja: un rombo ocupa la mitad. Sin recortar,
/// el sombreado se sale por las cuatro esquinas y el rombo se lee como un
/// cuadrado mal dibujado.
///
/// Devuelve `None` si el segmento se queda entero fuera, que es lo que le
/// pasa a la mitad de las rayas de un rombo.
pub fn recortar_a_convexo(a: Punto2, b: Punto2, tapa: &[Punto2]) -> Option<(Punto2, Punto2)> {
    if tapa.len() < 3 {
        return Some((a, b));
    }
    // El area con signo dice de que lado queda el dentro, asi que el mismo
    // codigo vale para un poligono dado en un sentido y en el otro.
    let doble_area: f32 = (0..tapa.len())
        .map(|i| {
            let (p, q) = (tapa[i], tapa[(i + 1) % tapa.len()]);
            p.x * q.y - q.x * p.y
        })
        .sum();
    let signo = if doble_area >= 0.0 { 1.0 } else { -1.0 };

    let (dx, dy) = (b.x - a.x, b.y - a.y);
    // Liang-Barsky: el segmento vive en `t` de 0 a 1 y cada lado le recorta
    // un trozo por delante o por detras.
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for i in 0..tapa.len() {
        let (p, q) = (tapa[i], tapa[(i + 1) % tapa.len()]);
        // Normal que apunta hacia FUERA del poligono.
        let (nx, ny) = (signo * (q.y - p.y), signo * -(q.x - p.x));
        let numerador = nx * (p.x - a.x) + ny * (p.y - a.y);
        let denominador = nx * dx + ny * dy;
        if denominador.abs() < f32::EPSILON {
            // Paralelo al lado: o esta dentro del todo o fuera del todo.
            if numerador < 0.0 {
                return None;
            }
            continue;
        }
        // Con la normal hacia fuera, estar dentro es `n·(X - p) <= 0`. Si el
        // segmento avanza HACIA fuera por este lado, el corte es el final; si
        // avanza hacia dentro, es el principio.
        let t = numerador / denominador;
        if denominador > 0.0 {
            t1 = t1.min(t);
        } else {
            t0 = t0.max(t);
        }
        if t0 > t1 {
            return None;
        }
    }
    Some((
        Punto2::nuevo(a.x + dx * t0, a.y + dy * t0),
        Punto2::nuevo(a.x + dx * t1, a.y + dy * t1),
    ))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Los cuatro vertices de un rombo de 100x100 en el origen.
    fn rombo() -> [Punto2; 4] {
        crate::formas::vertices_de_rombo(0.0, 0.0, 100.0, 100.0)
    }

    #[test]
    fn una_raya_que_cruza_el_rombo_se_queda_solo_con_lo_de_dentro() {
        // Por el medio a lo ancho, el rombo mide sus cien: la raya entra
        // entera. Mas arriba mide menos, y ahi es donde se recorta.
        let (a, b) = recortar_a_convexo(
            Punto2::nuevo(-50.0, 25.0),
            Punto2::nuevo(150.0, 25.0),
            &rombo(),
        )
        .expect("cruza el rombo");
        assert!((a.x - 25.0).abs() < 0.01, "entra por {a:?}");
        assert!((b.x - 75.0).abs() < 0.01, "sale por {b:?}");
    }

    #[test]
    fn una_raya_por_la_esquina_de_la_caja_se_va_entera() {
        // Caso negativo, y el que da sentido a todo esto: la esquina de la
        // caja esta FUERA del rombo, asi que esa raya no se pinta.
        assert_eq!(
            recortar_a_convexo(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(10.0, 0.0), &rombo()),
            None
        );
    }

    #[test]
    fn sin_poligono_no_se_recorta_nada() {
        // Una figura que si llena su caja no paga por esto.
        let (a, b) = (Punto2::nuevo(0.0, 0.0), Punto2::nuevo(1.0, 1.0));
        assert_eq!(recortar_a_convexo(a, b, &[]), Some((a, b)));
    }

    fn rayas(estilo: EstiloRelleno, elipse: bool) -> Vec<(Punto2, Punto2)> {
        let mut azar = Azar::nuevo(7);
        lineas_de_rayado(
            (0.0, 0.0, 200.0, 100.0),
            elipse,
            estilo,
            2.0,
            0.0,
            &mut azar,
        )
    }

    #[test]
    fn el_estilo_por_omision_es_el_rayado_de_excalidraw() {
        // Si fuera solido, un elemento del movil sin `fillStyle` se veria aqui
        // como una mancha de color y alli como rayas: el mismo fichero de dos
        // maneras.
        assert_eq!(EstiloRelleno::default(), EstiloRelleno::Rayado);
    }

    #[test]
    fn un_relleno_solido_no_genera_ni_una_raya() {
        // Caso negativo: el relleno plano lo sigue haciendo `Orden::Relleno`.
        // Si ademas saliera rayado, se veria el rayado encima de la mancha.
        assert!(rayas(EstiloRelleno::Solido, false).is_empty());
    }

    #[test]
    fn una_figura_sin_tamano_no_genera_rayas_ni_se_cuelga() {
        // Caso negativo: una caja de ancho cero no tiene interior. Sin el
        // corte, el barrido daria vueltas sobre una separacion inutil.
        let mut azar = Azar::nuevo(1);
        let vacia = lineas_de_rayado(
            (10.0, 10.0, 0.0, 0.0),
            false,
            EstiloRelleno::Rayado,
            2.0,
            1.0,
            &mut azar,
        );
        assert!(vacia.is_empty());
    }

    #[test]
    fn el_rayado_llena_el_rectangulo_con_rayas_inclinadas() {
        let r = rayas(EstiloRelleno::Rayado, false);
        assert!(r.len() > 5, "solo salieron {} rayas", r.len());
        for (a, b) in &r {
            let dx = b.x - a.x;
            let dy = b.y - a.y;
            assert!(dx.abs() > 1e-3, "una raya vertical no es diagonal");
            let grados = (dy / dx).atan().to_degrees();
            assert!(
                (grados - ANGULO_RAYADO).abs() < 1.0,
                "la raya va a {grados} grados y no a {ANGULO_RAYADO}"
            );
        }
    }

    #[test]
    fn la_separacion_crece_con_el_grosor_del_trazo() {
        // `hachureGap: strokeWidth * 4`: con trazo grueso las rayas se
        // sueltan, o el relleno se convertiria en una mancha solida.
        let cuantas = |grosor| {
            let mut azar = Azar::nuevo(3);
            lineas_de_rayado(
                (0.0, 0.0, 200.0, 100.0),
                false,
                EstiloRelleno::Rayado,
                grosor,
                0.0,
                &mut azar,
            )
            .len()
        };
        assert!(
            cuantas(1.0) > cuantas(4.0),
            "con trazo fino tiene que haber mas rayas"
        );
        assert_eq!(separacion(2.0), 8.0);
        assert_eq!(grosor_de_rayado(2.0), 1.0);
    }

    #[test]
    fn el_cruzado_raya_en_las_dos_direcciones() {
        let cruzado = rayas(EstiloRelleno::Cruzado, false);
        let simple = rayas(EstiloRelleno::Rayado, false);
        assert!(cruzado.len() > simple.len(), "el cruzado raya dos veces");

        let pendiente = |(a, b): &(Punto2, Punto2)| (b.y - a.y) / (b.x - a.x);
        let primera = pendiente(&cruzado[0]);
        assert!(
            cruzado.iter().any(|l| (pendiente(l) - primera).abs() > 0.5),
            "las dos tandas de rayas van en la misma direccion"
        );
    }

    #[test]
    fn el_rayado_de_una_elipse_termina_en_la_elipse_y_no_en_su_caja() {
        // Lo que separa una elipse rayada de un cuadrado rayado con una
        // elipse encima. Se mide sin rugosidad: el temblor moveria los
        // extremos y no se podria distinguir del fallo.
        let (cx, cy, rx, ry) = (100.0f32, 50.0f32, 100.0f32, 50.0f32);
        for (a, b) in rayas(EstiloRelleno::Rayado, true) {
            for p in [a, b] {
                let dentro = ((p.x - cx) / rx).powi(2) + ((p.y - cy) / ry).powi(2);
                assert!(
                    dentro <= 1.01,
                    "la raya acaba en {p:?}, fuera de la elipse ({dentro})"
                );
            }
            // Y las esquinas de la caja se quedan sin rayar: si el recorte no
            // existiera, alguna raya llegaria hasta ellas.
            assert!(a.x > -1.0 && b.x < 201.0);
        }
    }

    #[test]
    fn la_misma_semilla_raya_exactamente_igual() {
        // D38: reabrir el documento no cambia ni un punto.
        let hacer = || {
            let mut azar = Azar::nuevo(555);
            lineas_de_rayado(
                (0.0, 0.0, 200.0, 100.0),
                false,
                EstiloRelleno::Rayado,
                2.0,
                1.0,
                &mut azar,
            )
        };
        assert_eq!(hacer(), hacer());
    }

    #[test]
    fn con_rugosidad_las_rayas_tiemblan_y_sin_ella_no() {
        let con = {
            let mut azar = Azar::nuevo(9);
            lineas_de_rayado(
                (0.0, 0.0, 200.0, 100.0),
                false,
                EstiloRelleno::Rayado,
                2.0,
                1.0,
                &mut azar,
            )
        };
        let sin = rayas(EstiloRelleno::Rayado, false);
        assert_eq!(con.len(), sin.len(), "el temblor no cambia cuantas son");
        assert_ne!(con, sin, "con rugosidad las rayas tienen que desviarse");
    }

    #[test]
    fn el_zigzag_dibuja_dos_ramas_por_raya_y_las_dos_acaban_en_el_mismo_sitio() {
        // Es lo que hace un galon: dos ramas que nacen separadas media
        // separacion y se juntan. Si fuera una sola, seria un rayado normal.
        let mut azar = Azar::nuevo(7);
        let z = lineas_de_zigzag((0.0, 0.0, 200.0, 100.0), false, 2.0, 0.0, &mut azar);
        let mut azar = Azar::nuevo(7);
        let r = lineas_de_rayado(
            (0.0, 0.0, 200.0, 100.0),
            false,
            EstiloRelleno::Rayado,
            2.0,
            0.0,
            &mut azar,
        );
        assert_eq!(z.len(), r.len() * 2, "cada raya son dos ramas");
        for par in z.chunks_exact(2) {
            assert_eq!(par[0].1, par[1].1, "las dos ramas no se juntan");
            assert_ne!(par[0].0, par[1].0, "las dos ramas nacen en el mismo sitio");
        }
    }

    #[test]
    fn el_zigzag_no_se_sale_de_la_elipse_mas_de_medio_hueco() {
        // Los galones nacen desplazados, asi que pueden asomar media
        // separacion —y solo eso—: mas seria el relleno saliendose de la
        // figura, que es el fallo que ya tenia el rayado de la elipse.
        let mut azar = Azar::nuevo(3);
        let z = lineas_de_zigzag((0.0, 0.0, 200.0, 100.0), true, 2.0, 0.0, &mut azar);
        assert!(!z.is_empty());
        let margen = separacion(2.0);
        for (a, b) in z {
            for p in [a, b] {
                let d = ((p.x - 100.0) / (100.0 + margen)).powi(2)
                    + ((p.y - 50.0) / (50.0 + margen)).powi(2);
                assert!(d <= 1.05, "el galon acaba en {p:?} ({d})");
            }
        }
    }

    #[test]
    fn el_tiralineas_no_tiembla_ni_gasta_un_numero_del_azar() {
        // Las dos mitades de lo que promete: rectas de verdad —un plano con el
        // rayado tembloroso «se lee como suciedad»— y sin tocar el generador,
        // para que el garabato del contorno no dependa del relleno.
        let caja = (0.0, 0.0, 200.0, 100.0);
        let rectas = lineas_a_tiralineas(caja, false, 2.0);
        assert!(rectas.len() > 5, "solo salieron {}", rectas.len());
        for (a, b) in &rectas {
            let grados = ((b.y - a.y) / (b.x - a.x)).atan().to_degrees();
            assert!(
                (grados - ANGULO_RAYADO).abs() < 1e-3,
                "la raya va a {grados} y no a {ANGULO_RAYADO}: esta temblando"
            );
        }
        // Y llamarlo dos veces da exactamente lo mismo, sin generador de por
        // medio: es puro.
        assert_eq!(rectas, lineas_a_tiralineas(caja, false, 2.0));
    }

    #[test]
    fn el_tiralineas_y_el_rayado_ponen_las_mismas_rayas_y_solo_cambia_el_pulso() {
        // Caso negativo del anterior: si el tiralineas pusiera OTRAS rayas,
        // cambiar de trama en el panel movería el relleno de sitio.
        let caja = (0.0, 0.0, 200.0, 100.0);
        let mut azar = Azar::nuevo(1);
        let rayado = lineas_de_rayado(caja, false, EstiloRelleno::Rayado, 2.0, 0.0, &mut azar);
        let rectas = lineas_a_tiralineas(caja, false, 2.0);
        assert_eq!(rayado.len(), rectas.len());
        assert_eq!(rayado, rectas, "sin rugosidad tienen que ser las mismas");
    }

    #[test]
    fn una_figura_sin_tamano_no_da_galones_ni_tiralineas() {
        // Caso negativo: sin interior no hay nada que rellenar, y sin el corte
        // el barrido daria vueltas sobre una separacion inutil.
        let mut azar = Azar::nuevo(1);
        assert!(lineas_de_zigzag((0.0, 0.0, 0.0, 50.0), false, 2.0, 1.0, &mut azar).is_empty());
        assert!(lineas_a_tiralineas((0.0, 0.0, 50.0, 0.0), false, 2.0).is_empty());
    }

    #[test]
    fn el_estilo_va_y_vuelve_por_json_con_las_palabras_del_formato() {
        let texto = serde_json::to_string(&EstiloRelleno::Cruzado).unwrap();
        assert_eq!(texto, "\"cruzado\"");
        let vuelta: EstiloRelleno = serde_json::from_str(&texto).unwrap();
        assert_eq!(vuelta, EstiloRelleno::Cruzado);
    }
}
