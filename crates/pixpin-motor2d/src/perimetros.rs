//! El perimetro de **cualquier** figura, reducido a tramos rectos.
//!
//! Port de `Perimetros.kt` (520 lineas) del PixPin de Android.
//!
//! **No es una herramienta: es el cimiento de tres.** Lo necesitan, y por eso
//! vive aqui y no dentro de ninguna de ellas:
//!
//! - el **anclaje de interseccion** del iman, que es el de mayor prioridad
//!   del movil y el unico que hoy falta aqui;
//! - el **bote de relleno**, que para saber hasta donde derrama tiene que
//!   saber por donde pasan las paredes;
//! - **recortar y extender**, que cortan una raya justo donde la cruzan las
//!   demas.
//!
//! Si lo hiciera uno de los tres grupos, los otros dos esperarian.
//!
//! # Por que todo se reduce a rectas
//!
//! Porque cruzar «lo que sea contra lo que sea» pasa a ser un solo algoritmo
//! de dos segmentos en vez de un caso por pareja de tipos —que con quince
//! figuras serian ciento cinco parejas—. Las curvas se muestrean con el paso
//! acotado por arriba y por abajo, y el error de muestreo queda muy por
//! debajo del radio con el que el raton engancha, asi que no llega a notarse.
//!
//! # Lo que NO devuelve
//!
//! No devuelve `enganche::Anclaje`. `intersecciones_cerca` entrega puntos
//! pelados con el id de quien los puso, y es el iman —que es de otro dueno—
//! quien decide como se llaman y con que prioridad. Envolverlos aqui ataria
//! este cimiento al modulo que mas va a cambiar de los tres.

use crate::elemento::{Elemento, Figura};
use crate::vector::{Punto2, distancia_a_segmento};

/// Un trozo de perimetro ya muestreado. `cerrado` une el ultimo con el
/// primero.
#[derive(Debug, Clone, PartialEq)]
pub struct Contorno {
    pub puntos: Vec<Punto2>,
    pub cerrado: bool,
}

/// Paso de muestreo por omision, en pixeles del documento.
///
/// Seis pixeles es la distancia a la que una curva deja de leerse como
/// poligono a tamano normal, y sigue siendo menos de la mitad del radio con
/// el que el cursor se engancha.
pub const PASO_PERIMETRO: f32 = 6.0;

/// Tope de muestras por contorno: una elipse enorme no necesita mil tramos.
const MAX_MUESTRAS: usize = 240;

/// Minimo de muestras de una curva cerrada, para que no salga un poligono.
const MIN_MUESTRAS: usize = 24;

/// Cuando dos rectas se consideran paralelas.
const CASI_CERO: f32 = 1e-9;

/// Los contornos de `e` en coordenadas del documento y **ya girados**.
///
/// Una figura con caja da un contorno cerrado; una raya, uno abierto; y el
/// lapiz o la linea que se cierran sobre si mismos, uno cerrado.
///
/// Tres tipos no dan ninguno, y los tres por el mismo motivo —**un borde que
/// no se ve no puede comportarse como un borde**—:
///
/// - **el texto**, cuyo recuadro no se dibuja en ninguna parte: tratarlo como
///   contorno hacia aparecer cuatro enganches flotando alrededor de cada
///   rotulo y cruces con una raya invisible;
/// - **el foco**, que es una sombra sobre todo lo demas y no encierra nada;
/// - **el punto etiquetado**, que no es un borde sino un sitio: no corta a
///   nadie ni nadie lo corta, y de engancharse a el se encarga el iman
///   aparte.
pub fn contornos_de(e: &Elemento, paso: f32) -> Vec<Contorno> {
    if e.borrado {
        return Vec::new();
    }
    let (x0, y0, x1, y1) = (e.x, e.y, e.x + e.ancho, e.y + e.alto);
    let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let girar = |puntos: Vec<Punto2>| -> Vec<Punto2> {
        if e.angulo == 0.0 {
            puntos
        } else {
            puntos
                .into_iter()
                .map(|p| p.girar(centro, e.angulo))
                .collect()
        }
    };
    let cerrado = |puntos: Vec<Punto2>| -> Vec<Contorno> {
        if puntos.len() < 2 {
            Vec::new()
        } else {
            vec![Contorno {
                puntos: girar(puntos),
                cerrado: true,
            }]
        }
    };
    let caja = || {
        vec![
            Punto2::nuevo(x0, y0),
            Punto2::nuevo(x1, y0),
            Punto2::nuevo(x1, y1),
            Punto2::nuevo(x0, y1),
        ]
    };

    match &e.figura {
        // Ver la nota de la cabecera: los tres que no tienen borde visible.
        Figura::Texto { .. }
        | Figura::Emoji { .. }
        | Figura::Foco { .. }
        | Figura::Punto { .. } => Vec::new(),

        // La lupa, por el contorno de su cristal: es lo que se ve de ella.
        Figura::Lupa { cristal } => cerrado(crate::lupa_elemento::puntos_del_cristal(
            cristal,
            (e.x, e.y, e.ancho, e.alto),
        )),

        // El rombo pasa por los puntos medios de los lados, no por las
        // esquinas: cruzarlo por su caja seria cruzarlo por donde no esta.
        Figura::Rombo => {
            cerrado(crate::formas::vertices_de_rombo(e.x, e.y, e.ancho, e.alto).to_vec())
        }

        // El numero de serie es un CIRCULO dentro de su caja, no la caja.
        Figura::Elipse | Figura::Serie { .. } => cerrado(muestrear_elipse(x0, y0, x1, y1, paso)),

        // El arco da su tramo, abierto; la guia sin repasar da el ovalo
        // entero, porque es lo que esta puesto en el papel.
        // La marca de un angulo no es un borde: es un adorno sobre dos rayas
        // que ya estan, y sus cruces con ellas serian enganches de ruido.
        Figura::Arco { .. } if e.extras.marca_angulo.is_some() => Vec::new(),
        // El sector y el segmento son cerrados: sus radios o su cuerda
        // tambien cortan y enganchan.
        Figura::Arco { .. } if e.extras.cierre.is_some() => {
            match crate::geometria_exacta::contorno_cerrado(e) {
                Some(p) => cerrado(p),
                None => Vec::new(),
            }
        }
        Figura::Arco { inicio, barrido } => {
            let puntos = muestrear_arco(x0, y0, x1, y1, *inicio, *barrido, paso);
            if puntos.len() < 2 {
                Vec::new()
            } else {
                vec![Contorno {
                    puntos: girar(puntos),
                    cerrado: false,
                }]
            }
        }

        // La raya se cierra sola si el trazo vuelve a su punto de partida,
        // que es lo mismo que decide si se puede rellenar.
        Figura::Linea { puntos } | Figura::Lapiz { puntos, .. } | Figura::Resaltador { puntos } => {
            if puntos.len() < 2 {
                Vec::new()
            } else {
                vec![Contorno {
                    cerrado: es_lazo(puntos),
                    puntos: girar(puntos.clone()),
                }]
            }
        }

        Figura::Flecha { puntos, .. } | Figura::Cota { puntos } => {
            if puntos.len() < 2 {
                Vec::new()
            } else {
                vec![Contorno {
                    puntos: girar(puntos.clone()),
                    cerrado: false,
                }]
            }
        }

        // **El relleno de una region tambien es pared**: rellenar el hueco
        // que deja otro relleno tiene que respetar su borde, y sus agujeros
        // son paredes igual que su contorno.
        Figura::Region { contorno, huecos } => std::iter::once(contorno)
            .chain(huecos.iter())
            .filter(|a| a.len() >= 3)
            .map(|a| Contorno {
                puntos: girar(a.clone()),
                cerrado: true,
            })
            .collect(),

        // Las que se pintan por su caja dan su caja: es lo que se ve de
        // ellas y por tanto lo que uno espera cruzar o usar de pared.
        Figura::Rectangulo
        | Figura::Imagen { .. }
        | Figura::Mosaico { .. }
        | Figura::Marco { .. }
        | Figura::EscalaGrafica => cerrado(caja()),

        // Los instrumentos dan sus ejes y no su caja, que no esta dibujada:
        // es a lo que se pega el iman (`Perimetros.kt`).
        Figura::Instrumento(i) => crate::instrumento::ejes_en_escena(e, i)
            .into_iter()
            .map(|[a, b]| Contorno {
                puntos: girar(vec![a, b]),
                cerrado: false,
            })
            .collect(),
    }
}

/// Los tramos rectos de `e`, listos para cruzarlos con los de otro.
///
/// Es la forma en la que lo consumen tanto el iman como el relleno: a los dos
/// les da igual de que figura venia cada tramo.
pub fn segmentos_de(e: &Elemento, paso: f32) -> Vec<(Punto2, Punto2)> {
    let mut salida = Vec::new();
    for c in contornos_de(e, paso) {
        let mut puntos = c.puntos;
        if c.cerrado && puntos.len() > 2 {
            if let Some(p) = puntos.first().copied() {
                puntos.push(p);
            }
        }
        salida.extend(puntos.windows(2).map(|par| (par[0], par[1])));
    }
    salida
}

/// El trazo vuelve a su punto de partida (sin repetirlo), asi que hay un
/// tramo de cierre.
///
/// El umbral es el mismo criterio que el `isPathALoop` del movil: cerca en
/// pixeles del documento, no identico. Un trazo a pulso nunca cierra exacto.
const CIERRE: f32 = 8.0;

fn es_lazo(puntos: &[Punto2]) -> bool {
    puntos.len() > 2 && puntos[0].distancia(puntos[puntos.len() - 1]) <= CIERRE
}

/// La elipse muestreada, con **el paso adaptado a su tamano**.
///
/// Un numero fijo de muestras se equivoca por los dos lados: en un circulo de
/// diez pixeles sobran setenta tramos, y en uno de dos mil se ve el poligono.
fn muestrear_elipse(x0: f32, y0: f32, x1: f32, y1: f32, paso: f32) -> Vec<Punto2> {
    let (rx, ry) = ((x1 - x0) / 2.0, (y1 - y0) / 2.0);
    if rx <= 0.0 || ry <= 0.0 {
        return Vec::new();
    }
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    // Aproximacion de Ramanujan al perimetro: sobra de sobra para contar
    // tramos.
    let h = ((rx - ry) * (rx - ry)) / ((rx + ry) * (rx + ry));
    let perimetro =
        std::f32::consts::PI * (rx + ry) * (1.0 + (3.0 * h) / (10.0 + (4.0 - 3.0 * h).sqrt()));
    let n = ((perimetro / paso.max(0.5)) as usize).clamp(MIN_MUESTRAS, MAX_MUESTRAS);
    (0..n)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / n as f32;
            Punto2::nuevo(cx + rx * a.cos(), cy + ry * a.sin())
        })
        .collect()
}

fn muestrear_arco(
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    inicio: f32,
    barrido: Option<f32>,
    paso: f32,
) -> Vec<Punto2> {
    let (rx, ry) = ((x1 - x0) / 2.0, (y1 - y0) / 2.0);
    if rx <= 0.0 || ry <= 0.0 {
        return Vec::new();
    }
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let (desde, abarca) = match barrido {
        Some(b) => (inicio, b),
        None => (0.0, std::f32::consts::TAU),
    };
    // El radio mayor manda en cuantas muestras hacen falta: es donde la
    // curva se aleja mas de la recta que la aproxima.
    let largo = rx.max(ry) * abarca.abs();
    let n = ((largo / paso.max(0.5)) as usize).clamp(2, MAX_MUESTRAS);
    (0..=n)
        .map(|i| {
            let a = desde + abarca * (i as f32 / n as f32);
            Punto2::nuevo(cx + rx * a.cos(), cy + ry * a.sin())
        })
        .collect()
}

/// Donde se cortan dos tramos, o `None` si no se cortan **dentro de los dos**.
///
/// Prolongar las rectas hasta que se toquen daria puntos en mitad de la nada,
/// lejos de cualquier trazo, y el iman tiraria del cursor hacia sitios que el
/// usuario no ve.
pub fn interseccion(a1: Punto2, a2: Punto2, b1: Punto2, b2: Punto2) -> Option<Punto2> {
    let (d1x, d1y) = (a2.x - a1.x, a2.y - a1.y);
    let (d2x, d2y) = (b2.x - b1.x, b2.y - b1.y);
    let den = d1x * d2y - d1y * d2x;
    if den.abs() < CASI_CERO {
        return None;
    }
    let t = ((b1.x - a1.x) * d2y - (b1.y - a1.y) * d2x) / den;
    let u = ((b1.x - a1.x) * d1y - (b1.y - a1.y) * d1x) / den;
    if !(0.0..=1.0).contains(&t) || !(0.0..=1.0).contains(&u) {
        return None;
    }
    Some(Punto2::nuevo(a1.x + d1x * t, a1.y + d1y * t))
}

/// Un tramo que **sabe de donde salio**.
///
/// Sin esto no se puede distinguir un cruce de verdad del vertice donde se
/// juntan dos tramos seguidos, y hay dos formas de equivocarse, las dos
/// vividas en el movil:
///
/// - mirando la posicion en la lista **ya filtrada**: al quedarse solo con
///   los tramos cercanos al cursor, dos que estaban lejisimos en el trazo
///   pasan a ser vecinos, y el cruce de verdad entre ellos se descarta por
///   «seguidos». Es lo que hacia que un garabato cruzado consigo mismo no
///   enganchara nunca;
/// - olvidando que en un contorno **cerrado el ultimo tramo va seguido del
///   primero**: como el muestreo de una elipse empieza en el angulo cero,
///   todo circulo fabricaba una «interseccion» consigo mismo justo ahi.
#[derive(Debug, Clone, Copy)]
struct TramoIndexado {
    a: Punto2,
    b: Punto2,
    /// A que contorno del elemento pertenece: dos distintos nunca van
    /// seguidos.
    contorno: usize,
    indice: usize,
    cerrado: bool,
    total: usize,
}

impl TramoIndexado {
    fn cerca(&self, p: Punto2, radio: f32) -> bool {
        distancia_a_segmento(p, self.a, self.b) <= radio
    }

    fn seguido_de(&self, otro: &TramoIndexado) -> bool {
        if self.contorno != otro.contorno {
            return false;
        }
        let d = self.indice.abs_diff(otro.indice);
        d <= 1 || (self.cerrado && self.total > 0 && d == self.total - 1)
    }
}

fn tramos_indexados(e: &Elemento, paso: f32) -> Vec<TramoIndexado> {
    let mut salida = Vec::new();
    for (c, contorno) in contornos_de(e, paso).into_iter().enumerate() {
        let mut puntos = contorno.puntos;
        if contorno.cerrado && puntos.len() > 2 {
            if let Some(p) = puntos.first().copied() {
                puntos.push(p);
            }
        }
        let total = puntos.len().saturating_sub(1);
        for i in 0..total {
            salida.push(TramoIndexado {
                a: puntos[i],
                b: puntos[i + 1],
                contorno: c,
                indice: i,
                cerrado: contorno.cerrado,
                total,
            });
        }
    }
    salida
}

/// Todos los cruces que hay a menos de `radio` de `p`, con el id del
/// elemento que los puso.
///
/// **Los tramos se filtran antes de cruzarlos.** Cruzar todo contra todo es
/// cuadratico en el numero de tramos, y ahora que las curvas aportan hasta
/// doscientos cada una eso se notaria en cada fotograma: dos elipses grandes
/// serian cuarenta mil parejas. Como el resultado solo interesa si cae cerca
/// del cursor, se tira antes todo tramo que ni siquiera pase por su entorno.
///
/// `consigo` busca ademas los cruces de una figura **consigo misma**: un
/// garabato que se cruza al volver, o una linea en forma de lazo. Son puntos
/// igual de dificiles de acertar que los de dos figuras distintas.
pub fn intersecciones_cerca(
    elementos: &[Elemento],
    p: Punto2,
    radio: f32,
    excluir: Option<u64>,
    consigo: bool,
) -> Vec<(Punto2, u64)> {
    // Primero la caja, que es una comparacion de cuatro numeros: montar los
    // tramos de un plano entero para tirarlos despues es justo el gasto que
    // este filtro evita.
    let cerca: Vec<(u64, Vec<TramoIndexado>)> = elementos
        .iter()
        .filter(|e| {
            Some(e.id) != excluir && !e.borrado && !matches!(e.figura, Figura::Marco { .. })
        })
        .filter(|e| {
            let (x0, y0, x1, y1) = e.caja();
            p.x >= x0 - radio && p.x <= x1 + radio && p.y >= y0 - radio && p.y <= y1 + radio
        })
        .map(|e| {
            let tramos: Vec<TramoIndexado> = tramos_indexados(e, PASO_PERIMETRO)
                .into_iter()
                .filter(|t| t.cerca(p, radio))
                .collect();
            (e.id, tramos)
        })
        .filter(|(_, t)| !t.is_empty())
        .collect();

    let mut salida = Vec::new();
    for i in 0..cerca.len() {
        let (id, tramos) = &cerca[i];
        if consigo {
            for a in 0..tramos.len() {
                for b in (a + 1)..tramos.len() {
                    // Dos tramos SEGUIDOS comparten un extremo, y ese
                    // «cruce» es el propio vertice: ya engancha por su
                    // cuenta y con su nombre.
                    if tramos[a].seguido_de(&tramos[b]) {
                        continue;
                    }
                    if let Some(q) =
                        interseccion(tramos[a].a, tramos[a].b, tramos[b].a, tramos[b].b)
                    {
                        salida.push((q, *id));
                    }
                }
            }
        }
        for (_, otros) in cerca.iter().skip(i + 1) {
            for x in tramos {
                for y in otros {
                    if let Some(q) = interseccion(x.a, x.b, y.a, y.b) {
                        salida.push((q, *id));
                    }
                }
            }
        }
    }
    salida
}

/// El punto del perimetro de `e` mas cercano a `p`, si cae a menos de
/// `radio`.
///
/// **Es la escuadra.** Los demas enganches llevan a puntos sueltos —un
/// vertice, un centro, un cruce—, y con eso se puede empezar y terminar un
/// trazo sobre una guia, pero no *recorrerla*: en medio de un lado no hay
/// ningun punto notable al que pegarse, asi que la raya salia torcida entre
/// esquina y esquina y la curva de un circulo no habia manera de repasarla.
///
/// Pegandose a todo el borde, la guia se comporta como una plantilla de
/// dibujo de las de siempre: se apoya el lapiz en el canto y el trazo sale
/// por donde va el canto, no por donde tiembla la mano.
pub fn punto_en_el_perimetro(e: &Elemento, p: Punto2, radio: f32) -> Option<Punto2> {
    let mut mejor = None;
    let mut mejor_distancia = radio;
    for (a, b) in segmentos_de(e, PASO_PERIMETRO) {
        let q = proyeccion_en_segmento(p, a, b);
        let d = q.distancia(p);
        if d < mejor_distancia {
            mejor_distancia = d;
            mejor = Some(q);
        }
    }
    mejor
}

/// El punto del segmento `ab` mas cercano a `p`. Con el pie fuera, el
/// extremo.
pub fn proyeccion_en_segmento(p: Punto2, a: Punto2, b: Punto2) -> Punto2 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let largo2 = dx * dx + dy * dy;
    if largo2 == 0.0 {
        return a;
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / largo2).clamp(0.0, 1.0);
    Punto2::nuevo(a.x + t * dx, a.y + t * dy)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::ColorRgba;

    fn caja(figura: Figura, x: f32, y: f32, ancho: f32, alto: f32) -> Elemento {
        Elemento {
            id: 1,
            figura,
            x,
            y,
            ancho,
            alto,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            ..Default::default()
        }
    }

    #[test]
    fn un_rectangulo_da_sus_cuatro_lados_y_no_cinco_puntos() {
        let c = contornos_de(
            &caja(Figura::Rectangulo, 0.0, 0.0, 100.0, 50.0),
            PASO_PERIMETRO,
        );
        assert_eq!(c.len(), 1);
        assert!(c[0].cerrado);
        assert_eq!(c[0].puntos.len(), 4, "el cierre lo pone `segmentos_de`");
        assert_eq!(
            segmentos_de(
                &caja(Figura::Rectangulo, 0.0, 0.0, 100.0, 50.0),
                PASO_PERIMETRO
            )
            .len(),
            4
        );
    }

    #[test]
    fn el_rombo_pasa_por_los_puntos_medios_y_no_por_las_esquinas() {
        // Cruzarlo por su caja seria cruzarlo por donde no esta: un rombo
        // rayado hasta las esquinas de su caja es un cuadrado.
        let c = contornos_de(&caja(Figura::Rombo, 0.0, 0.0, 100.0, 50.0), PASO_PERIMETRO);
        assert!(c[0].puntos.contains(&Punto2::nuevo(50.0, 0.0)));
        assert!(!c[0].puntos.contains(&Punto2::nuevo(0.0, 0.0)));
    }

    #[test]
    fn una_elipse_grande_se_muestrea_mas_fina_que_una_pequena_pero_con_tope() {
        let chica = contornos_de(&caja(Figura::Elipse, 0.0, 0.0, 10.0, 10.0), PASO_PERIMETRO);
        let grande = contornos_de(
            &caja(Figura::Elipse, 0.0, 0.0, 4000.0, 4000.0),
            PASO_PERIMETRO,
        );
        assert_eq!(chica[0].puntos.len(), MIN_MUESTRAS, "suelo");
        assert_eq!(grande[0].puntos.len(), MAX_MUESTRAS, "techo");
    }

    #[test]
    fn el_texto_el_foco_y_el_punto_no_tienen_borde() {
        // Caso negativo, y de los importantes: un borde que no se ve no
        // puede comportarse como un borde. Con la caja del texto como
        // contorno aparecian cuatro enganches flotando alrededor de cada
        // rotulo y el bote chocaba contra un muro que nadie ve.
        for f in [
            Figura::Texto {
                texto: "hola".into(),
                tam: 20.0,
                familia: "Segoe UI".into(),
            },
            Figura::Foco {
                cristal: Default::default(),
            },
            Figura::Punto {
                letra: "A".into(),
                angulo: 0.0,
                radio: 14.0,
            },
        ] {
            assert!(
                contornos_de(&caja(f.clone(), 0.0, 0.0, 40.0, 20.0), PASO_PERIMETRO).is_empty(),
                "{f:?} no deberia dar contorno"
            );
        }
    }

    #[test]
    fn una_region_da_su_contorno_y_tambien_sus_agujeros() {
        // El agujero de un anillo es pared: derramar dentro de el no puede
        // desbordarse al resto del anillo.
        let anillo = caja(
            Figura::Region {
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
            0.0,
            0.0,
            100.0,
            100.0,
        );
        assert_eq!(contornos_de(&anillo, PASO_PERIMETRO).len(), 2);
    }

    #[test]
    fn dos_rayas_en_cruz_dan_su_cruce_y_dos_que_no_se_tocan_no_dan_nada() {
        let mut horizontal = caja(
            Figura::Linea {
                puntos: vec![Punto2::nuevo(0.0, 50.0), Punto2::nuevo(100.0, 50.0)],
            },
            0.0,
            50.0,
            100.0,
            0.0,
        );
        horizontal.id = 1;
        let mut vertical = caja(
            Figura::Linea {
                puntos: vec![Punto2::nuevo(50.0, 0.0), Punto2::nuevo(50.0, 100.0)],
            },
            50.0,
            0.0,
            0.0,
            100.0,
        );
        vertical.id = 2;
        let cruces = intersecciones_cerca(
            &[horizontal.clone(), vertical.clone()],
            Punto2::nuevo(52.0, 48.0),
            20.0,
            None,
            true,
        );
        assert_eq!(cruces.len(), 1);
        assert_eq!(cruces[0].0, Punto2::nuevo(50.0, 50.0));

        // Caso negativo: lejos del cruce no hay nada que ofrecer, aunque las
        // rayas se sigan cruzando en otro sitio.
        assert!(
            intersecciones_cerca(
                &[horizontal, vertical],
                Punto2::nuevo(5.0, 5.0),
                10.0,
                None,
                true
            )
            .is_empty()
        );
    }

    #[test]
    fn un_circulo_no_se_cruza_consigo_mismo_en_la_juntura_del_muestreo() {
        // El fallo del movil, clavado: el muestreo empieza en el angulo cero
        // —el punto de la derecha— y el ultimo tramo va seguido del primero.
        // Sin saberlo, TODO circulo fabricaba ahi una «interseccion»
        // consigo mismo: enganchaba, si, pero por el motivo equivocado.
        let circulo = caja(Figura::Elipse, 0.0, 0.0, 100.0, 100.0);
        let cruces = intersecciones_cerca(
            std::slice::from_ref(&circulo),
            Punto2::nuevo(100.0, 50.0),
            8.0,
            None,
            true,
        );
        assert!(cruces.is_empty(), "juntura falsa: {cruces:?}");
    }

    #[test]
    fn excluir_deja_fuera_al_elemento_que_se_esta_moviendo() {
        let raya = |id: u64, y: f32| {
            let mut e = caja(
                Figura::Linea {
                    puntos: vec![Punto2::nuevo(0.0, y), Punto2::nuevo(100.0, y)],
                },
                0.0,
                y,
                100.0,
                0.0,
            );
            e.id = id;
            e
        };
        let mut diagonal = raya(2, 0.0);
        diagonal.figura = Figura::Linea {
            puntos: vec![Punto2::nuevo(50.0, 0.0), Punto2::nuevo(50.0, 100.0)],
        };
        diagonal.alto = 100.0;
        diagonal.ancho = 0.0;
        let todos = [raya(1, 50.0), diagonal];
        assert_eq!(
            intersecciones_cerca(&todos, Punto2::nuevo(50.0, 50.0), 20.0, Some(2), true).len(),
            0,
            "sin la otra raya no queda cruce ninguno"
        );
    }

    #[test]
    fn la_escuadra_se_pega_a_media_pared_y_no_solo_a_las_esquinas() {
        // Es la razon de ser de `punto_en_el_perimetro`: en medio de un lado
        // no hay ningun punto notable, y sin esto la raya salia torcida
        // entre esquina y esquina.
        let r = caja(Figura::Rectangulo, 0.0, 0.0, 100.0, 50.0);
        let q = punto_en_el_perimetro(&r, Punto2::nuevo(40.0, 3.0), 10.0).unwrap();
        assert_eq!(q, Punto2::nuevo(40.0, 0.0));
        // Caso negativo: lejos del borde no se engancha nada, o el cursor
        // saltaria solo en mitad del lienzo.
        assert!(punto_en_el_perimetro(&r, Punto2::nuevo(40.0, 25.0), 10.0).is_none());
    }

    #[test]
    fn una_figura_girada_da_su_contorno_ya_girado() {
        // Si el contorno saliera sin girar, cruzar y engancharse se harian
        // contra una figura que no esta en la pantalla.
        let mut r = caja(Figura::Rectangulo, 0.0, 0.0, 100.0, 100.0);
        r.angulo = std::f32::consts::FRAC_PI_2;
        let c = contornos_de(&r, PASO_PERIMETRO);
        // Un cuadrado girado un cuarto de vuelta sobre su centro vuelve a
        // sus mismas cuatro esquinas, en otro orden.
        for esquina in [
            Punto2::nuevo(0.0, 0.0),
            Punto2::nuevo(100.0, 0.0),
            Punto2::nuevo(100.0, 100.0),
            Punto2::nuevo(0.0, 100.0),
        ] {
            assert!(
                c[0].puntos.iter().any(|p| p.distancia(esquina) < 0.01),
                "falta {esquina:?} en {:?}",
                c[0].puntos
            );
        }
    }

    #[test]
    fn un_elemento_borrado_no_tiene_perimetro() {
        // Caso negativo: lo borrado sigue en la escena con su bandera, y si
        // diera pared el bote chocaria contra algo que ya no se ve.
        let mut r = caja(Figura::Rectangulo, 0.0, 0.0, 100.0, 50.0);
        r.borrado = true;
        assert!(contornos_de(&r, PASO_PERIMETRO).is_empty());
    }
}
