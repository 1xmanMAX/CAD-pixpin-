//! De elemento a ordenes de dibujo.
//!
//! El motor **no dibuja**: produce la lista de poligonos y polilineas que hay
//! que pintar, con su color y su grosor. Quien las pinta es el consumidor —el
//! pin, la capa de pantalla, el PDF—, que ya tiene su pintor.
//!
//! Esto no es un rodeo, son dos ventajas concretas: el motor entero queda puro
//! y se prueba sin GPU (aqui, en CI, sin escritorio), y las mismas ordenes
//! valen para Direct2D hoy y para exportar a SVG manana sin tocar una linea de
//! geometria.

use crate::azar::Azar;
use crate::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};
use crate::escena::Escena;
use crate::formas;
use crate::medida::Escala;
use crate::relleno::EstiloRelleno;
use crate::vector::Punto2;

/// Una cosa que pintar. El consumidor traduce cada variante a su API.
#[derive(Debug, Clone, PartialEq)]
pub enum Orden {
    /// Contorno cerrado que se rellena. Es como se pinta la tinta de un
    /// trazo a mano: no es una linea gruesa, es una mancha con forma.
    Poligono {
        puntos: Vec<Punto2>,
        color: ColorRgba,
    },
    /// El contorno cerrado de un trazo a mano (E1). Se rellena con curvas
    /// por puntos medios y regla *winding* (`pixpin_render::tinta`), no
    /// como un poligono de rectas: es lo que hace que se vea como en
    /// Excalidraw.
    Tinta {
        contorno: Vec<Punto2>,
        color: ColorRgba,
    },
    /// Linea abierta de grosor constante.
    Polilinea {
        puntos: Vec<Punto2>,
        color: ColorRgba,
        grosor: f32,
        estilo: EstiloTrazo,
    },
    /// Relleno de una figura cerrada (el interior de un rectangulo).
    Relleno {
        puntos: Vec<Punto2>,
        color: ColorRgba,
    },
    /// Oscurece TODO el lienzo salvo el poligono `hueco` (D51). El motor
    /// no conoce el tamano del lienzo; el consumidor si.
    Velo {
        hueco: Vec<Punto2>,
        color: ColorRgba,
    },
    /// Texto en su caja.
    Texto {
        texto: String,
        x: f32,
        y: f32,
        tam: f32,
        familia: String,
        color: ColorRgba,
        /// Donde se parten los renglones. `texto::SIN_PARTIR` o mas: no se
        /// parten (un texto suelto del lienzo).
        ancho_max: f32,
        /// La cara de la letra (`extras.negrita`/`cursiva` del texto). Van
        /// en la orden y no se dejan a quien pinta porque son cinco los que
        /// pintan texto (pantalla, PNG, SVG, PDF, pin) y cada uno tendria que
        /// ir a buscarlas al elemento.
        negrita: bool,
        cursiva: bool,
    },
    /// **El numero de una cota**: un renglon girado con la raya y con halo
    /// (`dibujarRotulo` de `Renderer.kt`).
    ///
    /// Va aparte de `Texto` y no como dos campos mas porque son cinco los que
    /// pintan texto (pantalla, PNG, SVG, PDF, pin) y un campo nuevo lo
    /// ignoraria en silencio quien no lo mirara: el numero volveria a salir
    /// tumbado y sin halo en algun formato. Una variante nueva obliga a
    /// todos a saber pintarla.
    ///
    /// Como se pinta: se gira `angulo` (radianes, como `Punto2::girar`)
    /// alrededor de `centro`; en ese marco girado el renglon ocupa su caja
    /// con la esquina en `(x, y)`, igual que un `Texto` sin partir. Primero
    /// el halo —el trazo del contorno de las letras, de `grosor_halo` de
    /// ancho y color `halo`— y encima las letras en `color`: al reves el halo
    /// se comeria los perfiles.
    Rotulo {
        texto: String,
        x: f32,
        y: f32,
        tam: f32,
        familia: String,
        color: ColorRgba,
        halo: ColorRgba,
        grosor_halo: f32,
        centro: Punto2,
        angulo: f32,
    },
    /// Un bitmap que el consumidor tiene que resolver por su id.
    Imagen {
        id_objeto: u64,
        x: f32,
        y: f32,
        ancho: f32,
        alto: f32,
        opacidad: f32,
        /// El trozo del original que se estira dentro de la caja (`crop`).
        /// Va en la orden y no se deja a quien pinta porque son cinco los que
        /// pintan una imagen (pantalla, PNG, SVG, PDF, papel) y, si cada uno
        /// fuera a buscarlo al elemento, bastaria que uno se olvidara para
        /// que un formato ensenara lo que el recorte habia quitado.
        recorte: Option<crate::elemento::RecorteImagen>,
        /// El giro del bitmap, en radianes y alrededor del centro de su caja
        /// (`x, y, ancho, alto`, que va SIN girar). Como el `angle` de
        /// Excalidraw y el `canvas.rotate` de `Renderer.kt`. Sin el, una
        /// foto girada en el movil se veia derecha aqui: la orden solo movia
        /// su esquina y nadie giraba el bitmap.
        angulo: f32,
    },
}

/// **El grano de una tinta: la tela que se estampa DENTRO del trazo.**
///
/// No lleva geometria, y eso es lo importante: la silueta a la que hay que
/// recortar la tela es **la que ya salio** en la `Orden::Tinta` (o en el
/// contorno de la figura), asi que repetirla aqui seria calcular dos veces lo
/// mismo y, peor, pagarla dos veces en la cache de teselado. Quien pinta ya
/// tiene la silueta en la mano: esto solo le dice con que tenirla.
///
/// Por eso tampoco es una variante mas de [`Orden`]: una variante nueva
/// obligaria a que todos los que pintan ordenes —la capa de pantalla, el pin,
/// el chat, el editor— la supieran pintar antes de poder compilar, y el grano
/// no tiene sentido en todos. Asi, quien quiera grano lo pide; quien no,
/// sigue igual que estaba.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grano {
    /// De que esta hecha la tinta.
    pub material: crate::tinta::MaterialTinta,
    /// El color de la tela: la **misma** tinta, oscurecida. Un rayado negro
    /// sobre una tinta azul son dos tintas y se lee como una mancha sucia;
    /// oscureciendo la suya, lo que se ve es relieve.
    pub color: ColorRgba,
    /// Cada cuantos pixeles del documento se repite el cuadro.
    pub paso: f32,
    /// Si la tela va inclinada 45 grados (`GRADOS_DEL_GRANO`).
    pub inclinada: bool,
}

/// Lo ancho que sale de verdad la tinta de `e`, que no siempre es su grosor.
///
/// Aqui estaba, en el movil, lo de «la tinta de luz no hace nada»: el lapiz
/// guarda el `strokeWidth` de Excalidraw y se pinta como una mancha de
/// `FACTOR_VARIABLE` veces eso, asi que con el numero guardado el grano salia
/// a la escala equivocada —repartido como si el trazo fuera cuatro veces mas
/// fino de lo que se ve—.
fn ancho_de_la_tinta(e: &Elemento) -> f32 {
    match &e.figura {
        Figura::Lapiz {
            opciones: Some(_), ..
        } => e.grosor * crate::tinta::FACTOR_VARIABLE,
        Figura::Resaltador { .. } => e.grosor * crate::tinta::FACTOR_VARIABLE,
        _ => e.grosor,
    }
    .max(1.0)
}

/// El grano que hay que estampar dentro de `e`, si lleva alguno.
///
/// `None` para la tinta lisa y para las encendidas: lo que se ve de un tubo
/// encendido es el resplandor, y un rayado dentro de un resplandor no llega a
/// la pantalla.
pub fn grano_de(e: &Elemento) -> Option<Grano> {
    if e.borrado || !e.material.hay_grano() {
        return None;
    }
    let oscuro = 1.0 - crate::tinta::material::CUANTO_OSCURECE_EL_GRANO;
    Some(Grano {
        material: e.material,
        color: ColorRgba {
            r: e.trazo.r * oscuro,
            g: e.trazo.g * oscuro,
            b: e.trazo.b * oscuro,
            // El grano va a plena tinta aunque el cuerpo vaya flojo: es lo
            // que se tiene que leer.
            a: e.trazo.a * e.opacidad.clamp(0.0, 1.0),
        },
        paso: crate::tinta::paso_del_grano(ancho_de_la_tinta(e)),
        inclinada: e.material.se_inclina(),
    })
}

/// Cuanto de su opacidad conserva una linea de referencia, en tanto por uno
/// (`REFERENCIA_OPACIDAD = 35` del movil, `Renderer.kt`).
pub const OPACIDAD_DE_REFERENCIA: f32 = 0.35;

/// **La referencia se pinta translucida** (`Renderer.kt`: «no es decoracion:
/// es lo que dice que no es dibujo»). Devuelve la copia con la opacidad ya
/// rebajada y sin la marca, o `None` si no es referencia.
fn como_referencia(e: &Elemento) -> Option<Elemento> {
    if !e.extras.referencia {
        return None;
    }
    let mut t = e.clone();
    t.extras.referencia = false;
    t.opacidad *= OPACIDAD_DE_REFERENCIA;
    Some(t)
}

/// **La letra del rotulo de una figura que no es un texto** (la cifra de
/// una cota, la letra de un punto, los numeros de la escala grafica): la que
/// se eligio en el panel (`fontFamily` del movil, en `extras.familia`) o,
/// sin elegir, la del sistema de siempre.
fn letra_de_rotulo(e: &Elemento) -> String {
    e.extras
        .familia
        .clone()
        .unwrap_or_else(|| "Segoe UI".to_string())
}

/// **La rugosidad con la que se traza una raya** (`adjustRoughness` de
/// `Shapes.kt`): en las formas pequenas baja, porque un temblor de 2 px en
/// algo de 8 no parece dibujado, parece roto. Las lineales (linea, flecha y
/// cota) la conservan entera en cuanto miden 50 px.
fn rugosidad_ajustada(e: &Elemento) -> f32 {
    let lineal = matches!(
        e.figura,
        Figura::Linea { .. } | Figura::Flecha { .. } | Figura::Cota { .. }
    );
    let (w, h) = (e.ancho.abs(), e.alto.abs());
    let (mayor, menor) = (w.max(h), w.min(h));
    // La segunda condicion del original: redondeada y de mas de 15 de lado.
    let redondeable = matches!(e.figura, Figura::Rectangulo | Figura::Rombo) || lineal;
    let de_sobra = (menor >= 20.0 && mayor >= 50.0)
        || (menor >= 15.0 && e.redondo && redondeable)
        || (lineal && mayor >= 50.0);
    if de_sobra {
        e.rugosidad
    } else {
        (e.rugosidad / if mayor < 10.0 { 3.0 } else { 2.0 }).min(2.5)
    }
}

/// `preserveVertices` del movil: por debajo de la rugosidad de dibujante
/// (2) los extremos de cada raya no se mueven.
fn vertices_quietos(e: &Elemento) -> bool {
    e.rugosidad < 2.0
}

/// **Las opciones de rough.js de una figura** (`roughOptionsFor` del movil,
/// `generateRoughOptions` de Excalidraw): la rugosidad ajustada al tamano,
/// los vertices quietos por debajo de la de dibujante y una sola pasada si
/// el trazo es discontinuo o punteado.
fn opciones_rough(e: &Elemento) -> crate::rough::Opciones {
    crate::rough::Opciones::nuevas(
        rugosidad_ajustada(e),
        vertices_quietos(e),
        !matches!(e.estilo, EstiloTrazo::Solido),
    )
}

/// Aplica la opacidad del elemento a un color.
fn con_opacidad(c: ColorRgba, opacidad: f32) -> ColorRgba {
    ColorRgba {
        a: c.a * opacidad.clamp(0.0, 1.0),
        ..c
    }
}

/// Gira todos los puntos de una orden alrededor de `centro`.
///
/// Se gira **el resultado** y no se le pasa el angulo a cada generador de
/// geometria. Dos razones, y la segunda es la que manda:
///
/// 1. Es un solo sitio en vez de diez.
/// 2. La semilla tiene que seguir mandando sobre el aspecto (D38). Si el
///    angulo entrara en los generadores de rugosidad, girar un rectangulo lo
///    redibujaria con otro garabato y el dibujo temblaria al girarlo.
fn girar_orden(o: &mut Orden, centro: Punto2, angulo: f32) {
    let gira = |p: &mut Punto2| *p = p.girar(centro, angulo);
    match o {
        Orden::Poligono { puntos, .. }
        | Orden::Tinta {
            contorno: puntos, ..
        }
        | Orden::Polilinea { puntos, .. }
        | Orden::Relleno { puntos, .. }
        | Orden::Velo { hueco: puntos, .. } => puntos.iter_mut().for_each(gira),
        // La imagen da la vuelta entera: su centro gira alrededor de
        // `centro` y el bitmap suma el angulo. Su caja sigue sin girar.
        Orden::Imagen {
            x,
            y,
            ancho,
            alto,
            angulo: suyo,
            ..
        } => {
            let mut c = Punto2::nuevo(*x + *ancho / 2.0, *y + *alto / 2.0);
            gira(&mut c);
            *x = c.x - *ancho / 2.0;
            *y = c.y - *alto / 2.0;
            *suyo += angulo;
        }
        Orden::Texto { x, y, .. } => {
            // El texto gira por su esquina; quien pinta aplica el resto con
            // su propia transformacion.
            let mut p = Punto2::nuevo(*x, *y);
            gira(&mut p);
            *x = p.x;
            *y = p.y;
        }
        // El rotulo, como la imagen: su centro gira y su caja se lleva con
        // el (sin girar, en su marco), y el angulo se suma.
        Orden::Rotulo {
            x,
            y,
            centro,
            angulo: suyo,
            ..
        } => {
            let antes = *centro;
            gira(centro);
            *x += centro.x - antes.x;
            *y += centro.y - antes.y;
            *suyo += angulo;
        }
    }
}

/// Lo que hay que pintar DENTRO de una figura cerrada, antes de su contorno.
///
/// Con estilo solido es una mancha; con rayado o cruzado, una polilinea por
/// raya —del color del relleno y con la pluma mas fina del relleno, no con la
/// del contorno.
///
/// Usa un generador propio sembrado con la misma semilla en vez del del
/// elemento: si gastara numeros del otro, el garabato del contorno cambiaria
/// segun como este relleno el interior, y rellenar una figura ya dibujada la
/// redibujaria distinta (D38).
fn ordenes_de_relleno(e: &Elemento, elipse: bool) -> Vec<Orden> {
    // Con degradado se rellena aunque no haya color de relleno: el
    // degradado ya trae los suyos.
    let degradado = e.extras.degradado;
    let Some(r) = e
        .relleno
        .filter(|c| c.a > 0.0)
        .or(degradado.map(|d| d.desde))
    else {
        return Vec::new();
    };
    let color = con_opacidad(r, e.opacidad);
    let mut azar = Azar::nuevo(e.semilla);
    let rombo = matches!(e.figura, Figura::Rombo);
    // **Redondeada, el relleno sigue la MISMA ruta que el contorno**, como en
    // Excalidraw (el `fill` de rough.js va sobre la ruta redondeada). Antes
    // se rellenaba la caja recta y el color asomaba por las cuatro esquinas,
    // fuera de su contenedor.
    let redondeo = match e.figura {
        Figura::Rectangulo if e.redondo => {
            Some(formas::rectangulo_redondo(e.x, e.y, e.ancho, e.alto))
        }
        Figura::Rombo if e.redondo => Some(formas::rombo_redondo(e.x, e.y, e.ancho, e.alto)),
        _ => None,
    };

    if e.estilo_relleno == EstiloRelleno::Solido || degradado.is_some() {
        // La figura LISA, no la rugosa: rellenar la temblorosa deja huecos
        // por donde se escapa el fondo.
        let puntos = if elipse {
            formas::elipse(e.x, e.y, e.ancho, e.alto, 0.0, &mut azar)
                .into_iter()
                .next()
                .unwrap_or_default()
        } else if let Some(contorno) = &redondeo {
            contorno.clone()
        } else if rombo {
            formas::vertices_de_rombo(e.x, e.y, e.ancho, e.alto).to_vec()
        } else {
            vec![
                Punto2::nuevo(e.x, e.y),
                Punto2::nuevo(e.x + e.ancho, e.y),
                Punto2::nuevo(e.x + e.ancho, e.y + e.alto),
                Punto2::nuevo(e.x, e.y + e.alto),
            ]
        };
        if puntos.is_empty() {
            return Vec::new();
        }
        if let Some(d) = &degradado {
            return crate::geometria_exacta::ordenes_de_degradado(&puntos, e.caja(), d, e.opacidad);
        }
        return vec![Orden::Relleno { puntos, color }];
    }

    // Las rayas salen RECTAS del barrido y se recortan a la figura; el
    // temblor se lo pone despues rough.js (`doubleLine` del relleno del
    // movil), con los extremos quietos: antes se sacudian los extremos hasta
    // 4 px y las rayas se salian del contorno.
    let rough_relleno = (e.rugosidad > 0.0 && e.estilo_relleno != EstiloRelleno::LineasPixpin)
        .then(|| crate::rough::Opciones::nuevas(rugosidad_ajustada(e), vertices_quietos(e), false));
    let mut rough_rayas = rough_relleno.map(|o| crate::rough::Rough::con_azar(o, azar.clone()));
    crate::relleno::lineas_de_rayado(
        (e.x, e.y, e.ancho, e.alto),
        elipse,
        e.estilo_relleno,
        e.grosor,
        0.0,
        &mut azar,
    )
    .into_iter()
    // El rayado se genera para la caja; el rombo ocupa la mitad de ella, asi
    // que cada raya se recorta a sus cuatro lados. Sin esto el sombreado se
    // saldria por las cuatro esquinas y el rombo se leeria como un cuadrado.
    .filter_map(|(a, b)| {
        // Las dos rutas redondeadas son convexas, asi que el mismo recorte
        // del rombo en pico vale para ellas.
        if let Some(contorno) = &redondeo {
            crate::relleno::recortar_a_convexo(a, b, contorno)
        } else if rombo {
            crate::relleno::recortar_a_convexo(
                a,
                b,
                &formas::vertices_de_rombo(e.x, e.y, e.ancho, e.alto),
            )
        } else {
            Some((a, b))
        }
    })
    .flat_map(|(a, b)| match rough_rayas.as_mut() {
        Some(r) => {
            let pt = |p: Punto2| crate::rough::Pt::nuevo(p.x as f64, p.y as f64);
            crate::rough::a_pasadas(&r.doble_linea(pt(a), pt(b)))
        }
        None => vec![vec![a, b]],
    })
    .map(|puntos| Orden::Polilinea {
        puntos,
        color,
        grosor: crate::relleno::grosor_de_rayado(e.grosor),
        // Siempre solidas: unas rayas de relleno discontinuas se leerian
        // como suciedad y no como relleno.
        estilo: EstiloTrazo::Solido,
    })
    .collect()
}

/// **El contorno de una region con sus agujeros recortados, en UN solo
/// poligono.**
///
/// El primer intento fue mandar el contorno y cada agujero como ordenes de
/// relleno seguidas, el segundo con alfa cero, contando con que quien pinta
/// los restara por la regla par/impar. **Se probo y no**: `Pintor::poligono`
/// pinta una figura por llamada, asi que el agujero salia pintado igual y la
/// prueba de muestra lo enseno —un anillo relleno de lado a lado—.
///
/// Lo que si funciona es el ojo de cerradura de toda la vida: se recorre el
/// contorno, se sale por un puente hasta el agujero mas cercano, se le da la
/// vuelta entera al agujero y se vuelve por el mismo puente. Queda un solo
/// contorno cerrado, y las dos reglas de relleno —la par/impar de Direct2D y
/// la del giro— dejan el agujero vacio: el puente se recorre dos veces y se
/// anula solo.
///
/// El puente se busca entre el par de vertices MAS CERCANO, que es el que
/// menos se nota y el que menos riesgo corre de cruzar otro trozo del
/// contorno.
/// [`anillo_con_huecos`] para el resto del motor (los degradados).
pub(crate) fn anillo_con_huecos_pub(contorno: &[Punto2], huecos: &[Vec<Punto2>]) -> Vec<Punto2> {
    anillo_con_huecos(contorno, huecos)
}

fn anillo_con_huecos(contorno: &[Punto2], huecos: &[Vec<Punto2>]) -> Vec<Punto2> {
    let mut anillo = contorno.to_vec();
    for hueco in huecos.iter().filter(|h| h.len() >= 3) {
        let (mut mi, mut mj, mut mejor) = (0usize, 0usize, f32::MAX);
        for (i, a) in anillo.iter().enumerate() {
            for (j, b) in hueco.iter().enumerate() {
                let d = a.distancia(*b);
                if d < mejor {
                    (mejor, mi, mj) = (d, i, j);
                }
            }
        }
        let n = hueco.len();
        // El agujero al reves: con la par/impar da igual el sentido, pero al
        // reves vale tambien para la regla del giro, y asi este poligono
        // sirve sea cual sea la que use quien lo pinte.
        let mut puente: Vec<Punto2> = (0..=n).map(|k| hueco[(mj + n - (k % n)) % n]).collect();
        // Y de vuelta al contorno por donde se salio.
        puente.push(anillo[mi]);
        anillo.splice(mi + 1..mi + 1, puente);
    }
    anillo
}

/// El arco de `e` muestreado en tramos rectos, en coordenadas del documento.
///
/// Con `barrido` puesto sale el tramo repasado; sin el, **el ovalo entero**,
/// que es la guia que todavia no se ha repasado y que tiene que verse para
/// poder repasarla.
///
/// Es una sola linea porque la cuenta esta en `arco.rs`, que es el puerto del
/// `Arco.kt` del movil. Aqui habia un segundo muestreo con sus propios 64
/// tramos: dos verdades sobre por donde pasa un arco significan que se pica
/// donde no se ve, porque `impacto.rs` usa esta y el reajuste usa la otra.
pub(crate) fn arco_muestreado(e: &Elemento, inicio: f32, barrido: Option<f32>) -> Vec<Punto2> {
    crate::arco::puntos_del_arco(
        (e.x, e.y, e.ancho, e.alto),
        inicio,
        barrido,
        pasos_del_arco(e.ancho.max(e.alto) / 2.0),
    )
}

/// **Cuantos tramos por vuelta** para un arco de radio `r`: los 72 del movil
/// como minimo y, en uno grande, los que hagan falta para que ningun tramo
/// pase de 3 unidades (una semicircunferencia de 700 se veia a facetas,
/// 15-oct). Con tope: un arco enorme no gasta por gastar.
pub(crate) fn pasos_del_arco(r: f32) -> usize {
    let por_largo = (std::f32::consts::TAU * r.abs() / 3.0).ceil();
    (por_largo as usize).clamp(crate::arco::PASOS_DEL_ARCO, 1440)
}

/// Las ordenes de dibujo de un elemento, en orden de pintado.
pub fn ordenes(e: &Elemento) -> Vec<Orden> {
    if e.borrado {
        return Vec::new();
    }
    if let Some(t) = como_referencia(e) {
        return ordenes(&t);
    }
    // Un generador propio, sembrado con la semilla del elemento: asi su
    // aspecto no depende de cuantos elementos se dibujaron antes (D38).
    let mut azar = Azar::nuevo(e.semilla);
    // **Las porosas pintan el cuerpo mas flojo.** En la tiza, el lapiz
    // blando y el rotulador seco lo que se tiene que leer es el grano; con el
    // cuerpo a plena tinta el grano se pierde encima de un trazo macizo y las
    // tres se ven iguales que la lisa. Ver `tinta::material`.
    let color = con_opacidad(e.trazo, e.opacidad * e.material.cuerpo());
    let mut salida = Vec::new();

    match &e.figura {
        Figura::Lapiz {
            puntos,
            presiones,
            opciones,
        } => {
            // **La presion firme.** Un trazo con `presionFirme` va de ancho
            // constante: es lo que hace usable escribir a mano, porque sin
            // ella la tinta adelgaza en las curvas y la letra se rompe. Se
            // pide por `contorno_de_lapiz_firme` y no aplicando el campo aqui
            // para que no pueda quedarse puesta en un sitio y no en el otro,
            // que es como se veia el fallo en el movil: firme mientras se
            // escribe y adelgazando al soltar.
            let contorno = crate::tinta::contorno_de_lapiz_firme(
                puntos,
                presiones,
                e.grosor,
                *opciones,
                e.extras.presion_firme,
            );
            if !contorno.is_empty() {
                salida.push(Orden::Tinta { contorno, color });
            }
        }

        Figura::Resaltador { puntos } => {
            // El del movil: el lapiz gordo (x5) y al 40 %, con la misma tinta
            // que adelgaza con la velocidad (`tinta::contorno_de_resaltador`).
            let contorno = crate::tinta::contorno_de_resaltador(puntos, e.grosor);
            if !contorno.is_empty() {
                salida.push(Orden::Tinta {
                    contorno,
                    color: ColorRgba {
                        a: crate::tinta::OPACIDAD_DEL_RESALTADOR * e.opacidad,
                        ..e.trazo
                    },
                });
            }
        }

        // **La linea curva** (`roundness` en una linea): la spline que pasa
        // por sus puntos, como en Excalidraw y en el movil. Sin esto una
        // linea redondeada del movil —su estilo de fabrica lo es— salia aqui
        // quebrada en cada vertice.
        // Con dos puntos tambien: el movil la pasa por `curveThrough` igual
        // (`Shapes.kt`, rama `roundness`), y esa curva de dos puntos es la
        // recta con los extremos apenas sacudidos. Antes caia en la raya de
        // `formas::linea`, que en una linea larga se torcia a la vista.
        Figura::Linea { puntos } if e.redondo && puntos.len() >= 2 => {
            for pasada in crate::curva::pasadas_a_mano(puntos, rugosidad_ajustada(e), &mut azar) {
                salida.push(Orden::Polilinea {
                    puntos: pasada,
                    color,
                    grosor: e.grosor,
                    estilo: e.estilo,
                });
            }
        }

        // Sin redondeo, tramo a tramo con la raya de rough.js del movil
        // (`linearPath`: un solo generador para toda la linea).
        Figura::Linea { puntos } => {
            let (rug, preservar) = (rugosidad_ajustada(e), vertices_quietos(e));
            for par in puntos.windows(2) {
                for pasada in formas::linea_rough(par[0], par[1], rug, preservar, &mut azar) {
                    salida.push(Orden::Polilinea {
                        puntos: pasada,
                        color,
                        grosor: e.grosor,
                        estilo: e.estilo,
                    });
                }
            }
        }

        Figura::Flecha {
            puntos,
            punta_inicio,
            punta_fin,
            codos,
        } => {
            // El camino de verdad. Con codos no es la lista de puntos: es la
            // escalera ortogonal que sale de ellos, y las puntas tienen que
            // mirar ESA -si miraran la lista, la punta saldria apuntando a la
            // diagonal que el conector no llega a dibujar-.
            //
            // Sin codos se trabaja sobre la lista tal cual, sin copiarla: una
            // flecha normal es el caso de siempre y no tiene por que pagar la
            // reserva de la de codos.
            let escalera;
            // **La flecha curva**: el camino es la spline que pasa por los
            // puntos (`curva.rs`), y las puntas miran su final y no el
            // ultimo tramo recto, que en una curva apunta a otro sitio.
            let curva = if *codos {
                None
            } else {
                crate::curva::trazado_curvo(e)
            };
            let trazado: &[Punto2] = if *codos {
                escalera = crate::codo::trazado_de_flecha(puntos, true);
                &escalera
            } else if let Some(c) = &curva {
                c
            } else {
                puntos
            };
            // Redonda y de dos puntos (la flecha de siempre del movil) va por la
            // curva igual que alli (`curveThrough`): es la recta con los
            // extremos apenas sacudidos, no la raya torcida de `formas::linea`.
            let curva_recta = !*codos && e.redondo && puntos.len() == 2;
            if curva.is_some() || curva_recta {
                for pasada in crate::curva::pasadas_a_mano(puntos, rugosidad_ajustada(e), &mut azar)
                {
                    salida.push(Orden::Polilinea {
                        puntos: pasada,
                        color,
                        grosor: e.grosor,
                        estilo: e.estilo,
                    });
                }
            }
            let (rug, preservar) = (rugosidad_ajustada(e), vertices_quietos(e));
            for par in trazado
                .windows(2)
                .filter(|_| curva.is_none() && !curva_recta)
            {
                for pasada in formas::linea_rough(par[0], par[1], rug, preservar, &mut azar) {
                    salida.push(Orden::Polilinea {
                        puntos: pasada,
                        color,
                        grosor: e.grosor,
                        estilo: e.estilo,
                    });
                }
            }
            // Las dos puntas, cada una del tipo que le toque. El principio
            // primero y el final despues, que es el orden en el que estan en
            // el fichero.
            for (al_final, tipo) in [(false, *punta_inicio), (true, *punta_fin)] {
                let Some(forma) = formas::forma_de_punta(trazado, al_final, tipo, e.grosor) else {
                    continue;
                };
                let contorno = formas::contorno_de_punta(&forma);
                if contorno.len() < 2 {
                    continue;
                }
                // Las macizas van rellenas Y trazadas: solo rellenas, una
                // punta pequena se queda mas fina que la raya que remata y
                // parece que la flecha no llega.
                if tipo.es_maciza() {
                    salida.push(Orden::Relleno {
                        puntos: contorno.clone(),
                        color,
                    });
                }
                salida.push(Orden::Polilinea {
                    puntos: contorno,
                    color,
                    grosor: e.grosor,
                    // La punta siempre solida: una punta punteada no se lee
                    // como punta.
                    estilo: EstiloTrazo::Solido,
                });
            }
        }

        // **El mosaico: una banda opaca, y a propósito.**
        //
        // El movil tapa remuestreando los pixeles de debajo —bloques o
        // mancha, segun `desenfoque`—, y para eso hacen falta los pixeles,
        // que aqui no estan: el motor es puro y solo produce ordenes. Asi que
        // por ahora se tapa con una mancha maciza.
        //
        // Se parece menos, pero **tapa lo mismo**, que es lo unico que un
        // mosaico promete. Lo que habia antes era no pintar nada, y eso
        // significaba ensenar en el escritorio el numero de cuenta que el
        // usuario habia tapado en el telefono. Entre parecerse y tapar, tapa.
        Figura::Mosaico { .. } => {
            let tapa = e
                .relleno
                .filter(|c| c.a > 0.0)
                .unwrap_or(crate::mosaico::TAPA_MACIZA);
            // **Girada con el elemento.** El mosaico lleva tirador de giro
            // como cualquier otro y el marco de seleccion si giraba, asi que
            // la banda sin girar dejaba asomar las cuatro puntas de lo que el
            // usuario habia tapado. Se giran los cuatro puntos, no la caja
            // envolvente: el relleno es un poligono y tapa exactamente lo que
            // el marco promete.
            let centro = Punto2::nuevo(e.x + e.ancho / 2.0, e.y + e.alto / 2.0);
            let girar = |p: Punto2| {
                if e.angulo == 0.0 {
                    p
                } else {
                    p.girar(centro, e.angulo)
                }
            };
            salida.push(Orden::Relleno {
                puntos: vec![
                    girar(Punto2::nuevo(e.x, e.y)),
                    girar(Punto2::nuevo(e.x + e.ancho, e.y)),
                    girar(Punto2::nuevo(e.x + e.ancho, e.y + e.alto)),
                    girar(Punto2::nuevo(e.x, e.y + e.alto)),
                ],
                // **Sin la opacidad del elemento**: un mosaico a medio tapar
                // no tapa. Es la unica figura del lienzo a la que la
                // opacidad no se le aplica, y es por lo que existe.
                color: ColorRgba { a: 1.0, ..tapa },
            });
        }

        Figura::Rombo => {
            // El relleno va PRIMERO, como en el rectangulo: si fuera despues
            // taparia el trazo por dentro.
            salida.extend(ordenes_de_relleno(e, false));
            // **El rombo redondeado**, con la ruta de Excalidraw y con
            // cualquier rugosidad: liso con 0, a mano alzada por tramos con
            // mas (como rough.js sobre la ruta `L … C …`). El estilo de
            // fabrica del movil trae `roundness`, asi que un rombo pide
            // puntas redondeadas desde el primer dia.
            let pasadas = if e.redondo {
                formas::rombo_redondo_a_mano(
                    e.x,
                    e.y,
                    e.ancho,
                    e.alto,
                    opciones_rough(e),
                    &mut azar,
                )
            } else {
                formas::rombo_a_mano(e.x, e.y, e.ancho, e.alto, opciones_rough(e), &mut azar)
            };
            for pasada in pasadas {
                salida.push(Orden::Polilinea {
                    puntos: pasada,
                    color,
                    grosor: e.grosor,
                    estilo: e.estilo,
                });
            }
        }

        Figura::Rectangulo => {
            // El relleno va PRIMERO: si fuera despues taparia el trazo.
            salida.extend(ordenes_de_relleno(e, false));
            // Redondeado, con cualquier rugosidad: antes solo se redondeaba
            // con rugosidad 0 y «Bordes: redondo» con el trazo «a mano» de
            // fabrica no cambiaba nada de lo que se veia. Con 0 sale en una
            // sola pasada, que es el recuadro de una «zona» del movil.
            let pasadas = if e.redondo {
                formas::rectangulo_redondo_a_mano(
                    e.x,
                    e.y,
                    e.ancho,
                    e.alto,
                    opciones_rough(e),
                    &mut azar,
                )
            } else {
                formas::rectangulo_a_mano(e.x, e.y, e.ancho, e.alto, opciones_rough(e), &mut azar)
            };
            for pasada in pasadas {
                salida.push(Orden::Polilinea {
                    puntos: pasada,
                    color,
                    grosor: e.grosor,
                    estilo: e.estilo,
                });
            }
        }

        Figura::Elipse => {
            salida.extend(ordenes_de_relleno(e, true));
            // La de rough.js (ver `formas::elipse_a_mano`): una curva lisa,
            // no 32 vertices sacudidos unidos con rectas.
            let (pasadas, _) =
                formas::elipse_a_mano(e.x, e.y, e.ancho, e.alto, opciones_rough(e), &mut azar);
            for pasada in pasadas {
                salida.push(Orden::Polilinea {
                    puntos: pasada,
                    color,
                    grosor: e.grosor,
                    estilo: e.estilo,
                });
            }
        }

        Figura::Foco { cristal } => {
            // **El anillo entre el marco y el hueco** (`drawSpotlights` del
            // movil): el marco es la caja del elemento, derecho, y el hueco la
            // figura que se toco (`puntos_del_foco`, la misma geometria que la
            // lupa). Antes se oscurecia TODO el lienzo menos la caja, y el
            // usuario lo vio: «en el celular se oscurece dentro de una caja».
            // Un solo poligono con el agujero recortado (`anillo_con_huecos`,
            // el de las regiones): todo pintor lo sabe rellenar.
            let caja = (e.x, e.y, e.ancho, e.alto);
            let marco = vec![
                Punto2::nuevo(e.x, e.y),
                Punto2::nuevo(e.x + e.ancho, e.y),
                Punto2::nuevo(e.x + e.ancho, e.y + e.alto),
                Punto2::nuevo(e.x, e.y + e.alto),
            ];
            let hueco = crate::lupa_elemento::puntos_del_foco(cristal, caja);
            // Cuanto apaga lo dice el propio foco (10-90 %, 45 de fabrica), y
            // su opacidad no: en el movil el foco no pinta tinta.
            let cuanto = crate::lupa_elemento::oscurecimiento_de(cristal) as f32 / 100.0;
            salida.push(Orden::Relleno {
                puntos: anillo_con_huecos(&marco, &[hueco]),
                color: ColorRgba {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: cuanto,
                },
            });
        }

        // La lupa: montura, zona mirada y guia. Lo de dentro no es una orden:
        // lo repinta agrandado quien tiene la escena (`imagen::lupa` del
        // editor), igual que el mosaico lee lo de debajo.
        Figura::Lupa { cristal } => salida.extend(crate::lupa_elemento::ordenes_de_la_lupa(
            cristal,
            (e.x, e.y, e.ancho, e.alto),
            color,
            e.grosor,
        )),

        // **El texto rico** (`texto_rico`): tramos de color y formulas en
        // linea, compuestos en ordenes de las de siempre (textos y rayas),
        // asi que sale igual en pantalla, PNG, SVG y PDF.
        Figura::Texto { tam, familia, .. } if e.extras.rico.is_some() => {
            let estilo = crate::texto::EstiloDeTexto {
                negrita: e.extras.negrita,
                cursiva: e.extras.cursiva,
                tachado: e.extras.tachado,
            };
            let ancho_max = if e.extras.contenedor.is_none() {
                crate::texto::SIN_PARTIR
            } else {
                e.ancho.max(1.0)
            };
            let m = crate::texto_rico::componer(
                e.extras.rico.as_deref().unwrap_or_default(),
                &crate::texto_rico::Opciones {
                    tam: *tam,
                    familia,
                    estilo,
                    alineacion: e.extras.alineacion,
                    ancho_max,
                },
            );
            // Centrado o a la derecha, el bloque dentro de su caja (si la
            // caja es mas ancha que lo escrito).
            let dx = e
                .extras
                .alineacion
                .map_or(0.0, |a| ((e.ancho - m.ancho) * a.fraccion()).max(0.0));
            salida.extend(crate::texto_rico::ordenes(
                &m,
                e.x + dx,
                e.y,
                familia,
                color,
            ));
        }

        Figura::Texto {
            texto,
            tam,
            familia,
        } => {
            let estilo_texto = crate::texto::EstiloDeTexto {
                negrita: e.extras.negrita,
                cursiva: e.extras.cursiva,
                tachado: e.extras.tachado,
            };
            // **Un texto suelto no parte renglones**, como en Excalidraw: sus
            // renglones son los que se escribieron y la caja se ajusta a
            // ellos (`texto::medida`), no al reves. Partirlo al ancho de su
            // caja hacia que una caja medida un pelo corta mandara la ultima
            // palabra al renglon de abajo. El de dentro de una figura si se
            // parte, al hueco de su figura, como siempre.
            let ancho_max = if e.extras.contenedor.is_none() {
                crate::texto::SIN_PARTIR
            } else {
                e.ancho.max(1.0)
            };
            // **Centrado o a la derecha, renglon a renglon** (`textAlign`).
            // `Orden::Texto` solo sabe pintar pegado a la izquierda, y darle
            // un campo nuevo tocaria a todos los que la pintan; asi que cada
            // renglon sale en su propia orden, apartado lo que le toca. Va
            // precedido de tantos saltos como renglones tiene encima: quien
            // pinta lo coloca entonces a la altura exacta que le daria su
            // propio interlineado, sin que aqui haya que adivinarlo.
            //
            // A la izquierda —o sin decirlo— sigue saliendo en una sola
            // orden, como siempre: es el caso de casi todo texto y no tiene
            // por que pagar el reparto.
            let alineado = e
                .extras
                .alineacion
                .filter(|a| *a != crate::texto::AlineacionTexto::Izquierda);
            let apartados = alineado
                .map(|a| {
                    crate::texto::apartados_de_renglones(
                        texto,
                        *tam,
                        familia,
                        estilo_texto,
                        e.ancho,
                        a,
                    )
                })
                .unwrap_or_default();
            if alineado.is_some() {
                for (fila, (renglon, dx)) in texto.split('\n').zip(&apartados).enumerate() {
                    salida.push(Orden::Texto {
                        texto: format!("{}{renglon}", "\n".repeat(fila)),
                        x: e.x + dx,
                        y: e.y,
                        tam: *tam,
                        familia: familia.clone(),
                        color,
                        ancho_max,
                        negrita: estilo_texto.negrita,
                        cursiva: estilo_texto.cursiva,
                    });
                }
            } else {
                salida.push(Orden::Texto {
                    texto: texto.clone(),
                    x: e.x,
                    y: e.y,
                    tam: *tam,
                    familia: familia.clone(),
                    color,
                    ancho_max,
                    negrita: estilo_texto.negrita,
                    cursiva: estilo_texto.cursiva,
                });
            }
            // **El tachado, renglon a renglon.** No es un adorno de la fuente
            // sino una raya del dibujo: `Orden::Texto` no lleva tachado y el
            // motor es puro, asi que la altura la da `texto::raya_del_tachado`
            // -la misma cuenta para el editor, la exportacion y la miniatura,
            // porque tres copias acabarian poniendo la raya a tres alturas-.
            if estilo_texto.tachado {
                let paso = *tam
                    * crate::texto::interlineado_de(familia).unwrap_or(crate::texto::INTERLINEADO);
                for (fila, renglon) in texto.split('\n').enumerate() {
                    let y = e.y + fila as f32 * paso;
                    // La raya va con su renglon: centrado, centrada.
                    let x = e.x + apartados.get(fila).copied().unwrap_or(0.0);
                    let Some((a, b, gordo)) =
                        crate::texto::raya_del_tachado(renglon, x, y, *tam, familia, estilo_texto)
                    else {
                        continue;
                    };
                    salida.push(Orden::Polilinea {
                        puntos: vec![Punto2::nuevo(a.0, a.1), Punto2::nuevo(b.0, b.1)],
                        color,
                        grosor: gordo,
                        estilo: EstiloTrazo::Solido,
                    });
                }
            }
        }

        // Un emoji es texto con la fuente de color de Windows. El 0,8 deja
        // sitio al glifo, que asoma por encima y por debajo de su cuerpo.
        Figura::Emoji { caracter } => salida.push(Orden::Texto {
            texto: caracter.clone(),
            x: e.x,
            y: e.y,
            tam: e.alto * 0.8,
            familia: "Segoe UI Emoji".to_string(),
            color,
            ancho_max: e.ancho.max(1.0),
            negrita: false,
            cursiva: false,
        }),

        // **El marco no es dibujo** (22-oct): es andamiaje para ordenar e
        // imprimir laminas, y lo impreso va sin su raya, como los marcos de
        // los visores (`marcos_doc`). En el editor lo pinta el (raya azul a
        // trazos de grosor fijo y su numero en la esquina, con la escena):
        // con el mundo, la raya engordaba al acercarse y el numero, que iba
        // en la interfaz, llegaba tarde al mover el lienzo.
        Figura::Marco { .. } => {}

        Figura::Imagen { id_objeto } => salida.push(Orden::Imagen {
            id_objeto: *id_objeto,
            x: e.x,
            y: e.y,
            ancho: e.ancho,
            alto: e.alto,
            opacidad: e.opacidad,
            recorte: e.extras.recorte,
            // Aun sin girar: el angulo lo suma `girar_orden` al final, como
            // al resto de figuras.
            angulo: 0.0,
        }),

        // **La cota no sale de aqui**, ni la raya: su numero depende de la
        // escala y su tinta del papel, y ninguna de las dos cosas cambia la
        // version del elemento, asi que no pueden ir a la cache. Sale entera
        // de `ordenes_medibles` (`cota_entera`), que todos los caminos que
        // pintan encadenan.
        Figura::Cota { .. } => {}
        // **El arco: el tramo del ovalo, y la guia entera si aun no se ha
        // repasado.** Liso y no tembloroso a proposito: el arco se usa como
        // plantilla y un transportador que tiembla no sirve de plantilla. El
        // detalle —el repasado a mano, el reajuste al estirar— es del grupo
        // que lo herede; esto es lo que hace falta para que se VEA.
        // El sector, el segmento y la marca de angulo de las laminas
        // (`geometria_exacta`); el arco de siempre, como siempre.
        Figura::Arco { .. } if crate::geometria_exacta::ordenes_de_arco(e, color).is_some() => {
            salida.extend(crate::geometria_exacta::ordenes_de_arco(e, color).unwrap_or_default());
        }
        Figura::Arco { inicio, barrido } => {
            let puntos = arco_muestreado(e, *inicio, *barrido);
            if puntos.len() >= 2 {
                salida.push(Orden::Polilinea {
                    puntos,
                    color,
                    grosor: e.grosor,
                    // La guia sin repasar se ensena punteada: es un andamio,
                    // no una raya del dibujo, y confundirlas seria dejar
                    // ovalos enteros donde solo hay un arco a medias.
                    estilo: if barrido.is_some() {
                        e.estilo
                    } else {
                        EstiloTrazo::Punteado
                    },
                });
            }
        }

        // **El circulito con su numero, como el del movil** (`drawSerial`):
        // un disco macizo del color del trazo y el numero en negrita, blanco
        // o negro segun lo que se lea sobre ese color, de `radio * 1,25` y
        // centrado de verdad. Antes era un aro con el numero del color del
        // trazo, que sobre un circulo relleno de otro color no se leia. El
        // radio sale de la caja: estirarlo agranda el numero con el.
        Figura::Serie { numero } => {
            let radio = e.ancho.min(e.alto) / 2.0;
            if radio > 0.0 {
                let (cx, cy) = (e.x + e.ancho / 2.0, e.y + e.alto / 2.0);
                let mut lisa = Azar::nuevo(e.semilla);
                let disco = formas::elipse(
                    cx - radio,
                    cy - radio,
                    radio * 2.0,
                    radio * 2.0,
                    0.0,
                    &mut lisa,
                )
                .into_iter()
                .next()
                .unwrap_or_default();
                salida.push(Orden::Relleno {
                    puntos: disco,
                    color,
                });
                let texto = numero.to_string();
                let tam = radio * crate::serie::TEXTO_POR_RADIO;
                let familia = crate::serie::familia_de(e);
                let negrita = crate::texto::EstiloDeTexto {
                    negrita: true,
                    ..Default::default()
                };
                let (ancho, alto) = crate::texto::medida(&texto, tam, &familia, negrita);
                salida.push(Orden::Texto {
                    texto,
                    x: cx - ancho / 2.0,
                    y: cy - alto / 2.0,
                    tam,
                    familia,
                    color: crate::serie::tinta_sobre(color),
                    ancho_max: ancho.max(1.0) * 1.05,
                    negrita: true,
                    cursiva: false,
                });
            }
        }

        // Lo que encontro el bote: el contorno macizo con sus agujeros
        // RECORTADOS DE VERDAD. Ver `anillo_con_huecos`.
        Figura::Region { contorno, huecos } => {
            if contorno.len() >= 3 {
                let relleno = e.relleno.filter(|c| c.a > 0.0);
                if let Some(d) = &e.extras.degradado {
                    salida.extend(crate::geometria_exacta::ordenes_de_degradado(
                        &anillo_con_huecos(contorno, huecos),
                        e.caja(),
                        d,
                        e.opacidad,
                    ));
                } else if let Some(r) = relleno {
                    salida.push(Orden::Relleno {
                        puntos: anillo_con_huecos(contorno, huecos),
                        color: con_opacidad(r, e.opacidad),
                    });
                }
                // El borde, siempre, y **tambien el de cada agujero**: sin
                // el, una region sin relleno es invisible y no habria como
                // seleccionarla, y un agujero sin borde no se distingue de
                // un trozo que nadie relleno.
                for anillo in std::iter::once(contorno).chain(huecos.iter()) {
                    if anillo.len() < 3 {
                        continue;
                    }
                    let mut borde = anillo.clone();
                    if let Some(p) = borde.first().copied() {
                        borde.push(p);
                    }
                    salida.push(Orden::Polilinea {
                        puntos: borde,
                        color,
                        grosor: e.grosor,
                        estilo: e.estilo,
                    });
                }
            }
        }

        // El punto y su letra. La letra ORBITA en polares alrededor del
        // punto: es lo que hace que al mover el punto la letra lo siga sin
        // pisar el dibujo.
        Figura::Punto {
            letra,
            angulo,
            radio,
        } => {
            let r = (e.grosor * 1.6).max(2.0);
            let mut circulo = Azar::nuevo(e.semilla);
            let disco = formas::elipse(e.x - r, e.y - r, r * 2.0, r * 2.0, 0.0, &mut circulo)
                .into_iter()
                .next()
                .unwrap_or_default();
            if disco.len() >= 3 {
                salida.push(Orden::Relleno {
                    puntos: disco,
                    color,
                });
            }
            if !letra.is_empty() {
                // El tamano del pincel si lo trae (`fontSize`); si no, el de
                // siempre, que sale del grosor.
                let tam = e.extras.tam_letra.unwrap_or((e.grosor * 5.0).max(10.0));
                salida.push(Orden::Texto {
                    texto: letra.clone(),
                    x: e.x + radio * angulo.cos(),
                    y: e.y + radio * angulo.sin() - tam / 2.0,
                    tam,
                    familia: letra_de_rotulo(e),
                    color,
                    ancho_max: tam * letra.chars().count() as f32,
                    negrita: false,
                    cursiva: false,
                });
            }
        }

        // La barra entera depende de la escala: va en `ordenes_medibles`.
        Figura::EscalaGrafica => {}

        // El instrumento de graficar, con uno, dos o tres ejes: un solo
        // pintado para los tres (`instrumento::ordenes`), y por tanto el
        // mismo en pantalla, PNG, SVG, PDF y la pagina web.
        Figura::Instrumento(i) => {
            salida.extend(crate::instrumento::ordenes(e, i, color, letra_de_rotulo(e)));
        }
    }

    // El raya-punto, partido en tramos lisos: asi lo pintan igual los cinco
    // que pintan ordenes sin saber de el.
    crate::geometria_exacta::desplegar_raya_punto(&mut salida);

    // El angulo se aplica aqui, una vez, sobre la geometria ya generada.
    if e.angulo != 0.0 {
        let (x0, y0, x1, y1) = e.caja();
        let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        for o in salida.iter_mut() {
            girar_orden(o, centro, e.angulo);
        }
    }

    salida
}

/// Las ordenes de la escena entera, de abajo arriba.
///
/// Encadena `ordenes_medibles`: sin esto, una cota sale sin su rotulo y una
/// barra de escala no sale en absoluto (D37, «sale en la exportacion»). Esta
/// funcion la usan la capa de pantalla y la recarga de un pin, no solo el
/// editor -y era justo ahi donde el rotulo desaparecia, invisible porque
/// nada llamaba a esta ruta a comprobarlo en pantalla.
pub fn ordenes_de_escena(escena: &Escena) -> Vec<Orden> {
    escena
        .a_la_vista()
        .flat_map(|e| {
            ordenes(e).into_iter().chain(ordenes_medibles(
                e,
                escena.escala.as_ref(),
                ',',
                escena.fondo,
            ))
        })
        .collect()
}

/// Holgura que se acepta perder al simplificar, en pixeles de pantalla.
///
/// Medio pixel es la mitad de lo que el ojo puede distinguir, asi que lo que
/// se quita por debajo de eso no se puede notar por definicion.
pub const HOLGURA_DETALLE: f32 = 0.5;

/// Por debajo de este tamano en pantalla un trazo de tinta se pinta como
/// una raya.
///
/// Era 48 px y era la economia del lienzo infinito, pero borraba la tinta
/// de todo lo pequeno: letras, tics y firmas salian como rayas de grosor
/// fijo y puntas cortadas (diagnostico de E1). Excalidraw no simplifica
/// nunca. Aqui se queda en 2 px (D115), donde de verdad no se distingue, y
/// la economia de lejos pasa a la rejilla, la capa congelada y la cache de
/// realizaciones de Direct2D.
pub const TINTA_MINIMA_PX: f32 = 2.0;

/// Las ordenes de un elemento visto a este aumento.
///
/// Hace dos cosas, y solo a las figuras que tienen puntos:
///
/// - Un trazo de tinta —lapiz o resaltador— que en pantalla queda por debajo
///   de `TINTA_MINIMA_PX` pasa a dibujarse como linea, adelgazada.
/// - Una linea o una flecha se adelgazan siempre, porque su salida es punto
///   por punto y ahi quitar puntos si quita trabajo.
///
/// Un `zoom` de cero o menos devuelve el dibujo entero: es la forma de pedir
/// «sin simplificar», que es lo que quiere quien exporta a un fichero.
///
/// Se trabaja sobre una **copia**: los puntos guardados del elemento no se
/// tocan nunca. Ver `aligerar` para por que eso no es negociable.
pub fn ordenes_a_distancia(e: &Elemento, zoom: f32) -> Vec<Orden> {
    if let Some(t) = como_referencia(e) {
        return ordenes_a_distancia(&t, zoom);
    }
    if zoom <= 0.0 || e.borrado {
        return ordenes(e);
    }
    let tolerancia = HOLGURA_DETALLE / zoom;
    let flacos = |puntos: &[Punto2]| crate::aligerar::aligerar(puntos, tolerancia);

    // Cuanto ocupa en pantalla, por su lado mayor.
    let (x0, y0, x1, y1) = e.caja();
    let en_pantalla = (x1 - x0).max(y1 - y0) * zoom;

    match &e.figura {
        Figura::Lapiz { puntos, .. } | Figura::Resaltador { puntos }
            if en_pantalla < TINTA_MINIMA_PX =>
        {
            let mut puntos = flacos(puntos);
            if puntos.len() < 2 {
                return Vec::new();
            }
            // Los puntos crudos estan en marco local, igual que en `ordenes`:
            // esta rama no delega en ella, asi que gira aqui su propio
            // resultado.
            if e.angulo != 0.0 {
                let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
                for p in puntos.iter_mut() {
                    *p = p.girar(centro, e.angulo);
                }
            }
            vec![Orden::Polilinea {
                puntos,
                color: con_opacidad(e.trazo, e.opacidad),
                grosor: e.grosor.max(1.0),
                estilo: EstiloTrazo::Solido,
            }]
        }
        Figura::Linea { puntos } => ordenes(&Elemento {
            figura: Figura::Linea {
                puntos: flacos(puntos),
            },
            ..e.clone()
        }),
        Figura::Flecha {
            puntos,
            punta_inicio,
            punta_fin,
            codos,
        } => ordenes(&Elemento {
            figura: Figura::Flecha {
                puntos: flacos(puntos),
                punta_inicio: *punta_inicio,
                punta_fin: *punta_fin,
                codos: *codos,
            },
            ..e.clone()
        }),
        _ => ordenes(e),
    }
}

/// Las ordenes de lo que se ve, y solo de lo que se ve.
///
/// Es la via que usa un lienzo infinito, y hace las dos economias que lo
/// sostienen: no mira siquiera los elementos que caen fuera de la pantalla, y
/// de los que entran no dibuja el detalle que a ese aumento no se distingue.
pub fn ordenes_de_escena_vista(
    escena: &Escena,
    camara: &crate::camara::Camara,
    ancho_px: f32,
    alto_px: f32,
) -> Vec<Orden> {
    crate::camara::recortar(escena, camara.ventana(ancho_px, alto_px))
        .flat_map(|e| {
            ordenes_a_distancia(e, camara.zoom)
                .into_iter()
                .chain(ordenes_medibles(
                    e,
                    escena.escala.as_ref(),
                    ',',
                    escena.fondo,
                ))
        })
        .collect()
}

/// Holgura del marco de seleccion alrededor de la caja del elemento, en
/// pixeles logicos: pegado al borde no se distingue del propio trazo.
pub const HOLGURA_SELECCION: f32 = 4.0;

/// El azul del marco de seleccion. Un color que no esta en la paleta de
/// dibujo: asi se lee como interfaz y no como algo dibujado.
pub const COLOR_SELECCION: ColorRgba = ColorRgba {
    r: 0.36,
    g: 0.42,
    b: 0.95,
    a: 1.0,
};

/// Tamano del rotulo de una cota que no trae `fontSize` (`MEASURE_TEXT_SIZE`).
pub const TAM_ROTULO_COTA: f32 = 20.0;
/// Aire a cada lado del numero dentro del hueco, en veces su tamano.
const AIRE_DEL_ROTULO: f32 = 0.45;
/// Que parte de la cota puede llegar a ser hueco: pasado eso son dos munones
/// con un numero en medio (`MAXIMO_HUECO`).
const MAXIMO_HUECO: f32 = 0.72;
/// Grosor del halo del numero, en veces su tamano (`MEASURE_HALO`).
pub const HALO_DEL_ROTULO: f32 = 0.22;
/// Por debajo de este largo la cota no se pinta (`MIN_MEASURE_LENGTH`).
const LARGO_MINIMO_COTA: f32 = 0.5;

/// **La cota como la pinta el movil** (`drawMeasure` y `dibujarRotulo` de
/// `Renderer.kt`): la raya abierta en el medio para el numero, los
/// banderines de los extremos, las medias puntas de flecha y el numero
/// girado con la raya y con halo. Todo con la tinta adaptada al papel.
///
/// Antes el numero iba tumbado en horizontal, encima de la raya, sin halo y
/// **en gris** cuando no habia escala (un aviso propio, D35, que el movil no
/// tiene: alli va siempre del color de la cota). El usuario lo vio al abrir
/// en Windows una foto medida en el movil (27-sep-2026).
fn cota_entera(
    e: &Elemento,
    a: Punto2,
    b: Punto2,
    escala: Option<&Escala>,
    coma: char,
    papel: ColorRgba,
) -> Vec<Orden> {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let largo = dx.hypot(dy);
    if largo < LARGO_MINIMO_COTA || !largo.is_finite() {
        return Vec::new();
    }
    let tinta = crate::contraste::adaptar(e.trazo, crate::contraste::papel_de(papel));
    let color = con_opacidad(tinta, e.opacidad);
    let texto = crate::medida::texto_de_cota(e, escala, coma);
    let tam = e.extras.tam_letra.unwrap_or(TAM_ROTULO_COTA);
    let familia = letra_de_rotulo(e);
    let (ancho_texto, alto_texto) = crate::texto::medida(
        &texto,
        tam.max(0.0),
        &familia,
        crate::texto::EstiloDeTexto::default(),
    );

    // **La raya se abre en el medio para dejar sitio al numero**, como se
    // acota en un plano. Si no cabe, entera y el numero encima.
    let hueco = if tam <= 0.0 {
        0.0
    } else {
        let h = ancho_texto + tam * AIRE_DEL_ROTULO * 2.0;
        if h > largo * MAXIMO_HUECO { 0.0 } else { h }
    };

    let mut salida = Vec::new();
    // Todas las rayas con el mismo pulso que una linea a mano del movil:
    // la de rough.js con los extremos quietos y el azar de la cota,
    // empezando de cero en cada trozo (`Rough(...)` nuevo en `trazo`).
    let rug = rugosidad_ajustada(e);
    let mut trazo = |desde: Punto2, hasta: Punto2| {
        let mut azar = Azar::nuevo(e.semilla);
        for pasada in formas::linea_rough(desde, hasta, rug, true, &mut azar) {
            salida.push(Orden::Polilinea {
                puntos: pasada,
                color,
                grosor: e.grosor,
                estilo: EstiloTrazo::Solido,
            });
        }
    };
    let (ux, uy) = (dx / largo, dy / largo);
    if hueco <= 0.0 {
        trazo(a, b);
    } else {
        let corte = (largo - hueco) / 2.0;
        trazo(a, Punto2::nuevo(a.x + ux * corte, a.y + uy * corte));
        trazo(Punto2::nuevo(b.x - ux * corte, b.y - uy * corte), b);
    }
    // Los banderines: la perpendicular que marca donde empieza y acaba.
    let (nx, ny) = (-uy, ux);
    let ala = (e.grosor * 3.0).max(6.0);
    for p in [a, b] {
        trazo(
            Punto2::nuevo(p.x - nx * ala, p.y - ny * ala),
            Punto2::nuevo(p.x + nx * ala, p.y + ny * ala),
        );
    }
    // Las medias puntas, de cada extremo hacia fuera de la raya.
    let largo_punta = (e.grosor * 5.0).max(9.0);
    for (en, hacia) in [(a, b), (b, a)] {
        let ang = (hacia.y - en.y).atan2(hacia.x - en.x);
        for s in [-1.0f32, 1.0] {
            let giro = ang + s * 20f32.to_radians();
            trazo(
                en,
                Punto2::nuevo(
                    en.x + largo_punta * giro.cos(),
                    en.y + largo_punta * giro.sin(),
                ),
            );
        }
    }

    // El numero, **a lo largo de la raya y nunca del reves**: se gira con la
    // direccion de la raya en pantalla y, si eso lo dejaria boca abajo, media
    // vuelta mas (`rotuloDelReves`). La decision cuenta tambien el giro del
    // elemento, que `ordenes_medibles` suma despues.
    if tam > 0.0 && !texto.is_empty() {
        let grados = dy.atan2(dx).to_degrees();
        let del_reves = crate::medida::rotulo_del_reves(grados + e.angulo.to_degrees());
        let angulo = if del_reves { grados + 180.0 } else { grados }.to_radians();
        let centro = a.hacia(b, 0.5);
        salida.push(Orden::Rotulo {
            texto,
            x: centro.x - ancho_texto / 2.0,
            y: centro.y - alto_texto / 2.0,
            tam,
            familia,
            color,
            halo: crate::contraste::color_de_halo(color),
            grosor_halo: tam * HALO_DEL_ROTULO,
            centro,
            angulo,
        });
    }
    salida
}

/// Marco alrededor de lo seleccionado, para que se vea que esta elegido.
/// Devuelve `None` si el elemento no esta o esta borrado.
pub fn marco_de_seleccion(escena: &Escena, id: u64, escala: f32) -> Option<Orden> {
    let e = escena.buscar(id).filter(|e| !e.borrado)?;
    let (x0, y0, x1, y1) = e.caja();
    let h = HOLGURA_SELECCION * escala.max(0.01);
    let (x0, y0, x1, y1) = (x0 - h, y0 - h, x1 + h, y1 + h);
    Some(Orden::Polilinea {
        // Cerrado: el ultimo punto repite el primero.
        puntos: vec![
            Punto2::nuevo(x0, y0),
            Punto2::nuevo(x1, y0),
            Punto2::nuevo(x1, y1),
            Punto2::nuevo(x0, y1),
            Punto2::nuevo(x0, y0),
        ],
        color: COLOR_SELECCION,
        grosor: (1.5 * escala.max(0.01)).max(1.0),
        estilo: EstiloTrazo::Discontinuo,
    })
}

/// Lo que hay que pintar y **depende de la escala**, asi que no se cachea.
///
/// La cache guarda geometria indexada por `(id, version, nivel)`. Calibrar
/// cambia la escala sin tocar ningun elemento, asi que la version no sube y
/// la cache no se enteraria: se seguirian viendo pixeles despues de calibrar.
///
/// Por eso el rotulo de la cota y los cuadros de la barra salen por aqui,
/// fuera de la cache. Es barato: un texto por cota visible y unos rectangulos
/// por barra, contra el garabato con ruido que si es caro y si se cachea.
///
/// **La cota entera sale de aqui, raya incluida**, porque su color depende
/// del papel (`papel`, el fondo de la escena): el movil pinta raya y numero
/// con la tinta adaptada a ese papel (`tema(...)` en `drawMeasure`), y el
/// papel puede cambiar sin que cambie la cota, igual que la escala.
pub fn ordenes_medibles(
    e: &Elemento,
    escala: Option<&Escala>,
    coma: char,
    papel: ColorRgba,
) -> Vec<Orden> {
    if e.borrado {
        return Vec::new();
    }
    let mut salida = match &e.figura {
        Figura::Cota { puntos } if puntos.len() >= 2 => {
            cota_entera(e, puntos[0], puntos[puntos.len() - 1], escala, coma, papel)
        }
        Figura::EscalaGrafica => barra(e, escala, coma),
        _ => Vec::new(),
    };

    // Mismo bloque que cierra `ordenes()`: el tirador de giro se ofrece para
    // cualquier elemento, `Cota` y `EscalaGrafica` incluidas, y esta salida
    // vive fuera de esa funcion (D-cache-escala), asi que no hereda su giro
    // por delegacion. Hay que aplicarlo aqui tambien o el rotulo gira sin la
    // raya y la barra gira sin sus cuadros.
    if e.angulo != 0.0 {
        let (x0, y0, x1, y1) = e.caja();
        let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        for o in salida.iter_mut() {
            girar_orden(o, centro, e.angulo);
        }
    }

    salida
}

/// Los cuadros de la barra y sus numeros.
///
/// Sin escala no dibuja nada: una barra que no puede decir cuanto mide cada
/// cuadro es un adorno que engana.
fn barra(e: &Elemento, escala: Option<&Escala>, coma: char) -> Vec<Orden> {
    let Some(esc) = escala.filter(|x| x.valida()) else {
        return Vec::new();
    };
    let Some(rep) = crate::escalabarra::repartir(e.ancho, esc, 4) else {
        return Vec::new();
    };
    let ancho_cuadro = rep.ancho_usado_px / rep.cuadros as f32;
    let alto = e.alto.max(4.0) * 0.5;
    let tinta = con_opacidad(e.trazo, e.opacidad);
    let claro = ColorRgba {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: tinta.a,
    };

    let mut fuera = Vec::with_capacity(rep.cuadros as usize * 2 + 2);
    for i in 0..rep.cuadros {
        let x0 = e.x + i as f32 * ancho_cuadro;
        let x1 = x0 + ancho_cuadro;
        // A cuadros alternos: todos iguales no se podrian contar.
        let color = if i % 2 == 0 { tinta } else { claro };
        fuera.push(Orden::Relleno {
            puntos: vec![
                Punto2::nuevo(x0, e.y),
                Punto2::nuevo(x1, e.y),
                Punto2::nuevo(x1, e.y + alto),
                Punto2::nuevo(x0, e.y + alto),
            ],
            color,
        });
    }
    // El numero del principio y el del final: con esos dos se lee la barra.
    for (i, valor) in [(0u32, 0.0f32), (rep.cuadros, rep.cuadros as f32 * rep.paso)] {
        let x = e.x + i as f32 * ancho_cuadro;
        fuera.push(Orden::Texto {
            texto: crate::medida::formatear_valor(valor, &esc.unidad, esc.decimales, coma),
            x,
            y: e.y + alto + 2.0,
            tam: alto,
            familia: letra_de_rotulo(e),
            color: tinta,
            ancho_max: ancho_cuadro * 2.0,
            negrita: false,
            cursiva: false,
        });
    }
    fuera
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::formas::TipoPunta;

    #[test]
    fn el_marco_de_seleccion_rodea_la_caja_con_holgura() {
        let mut escena = Escena::nueva();
        let id = escena.anadir(base());
        let (x0, y0, x1, y1) = escena.buscar(id).unwrap().caja();

        let Some(Orden::Polilinea { puntos, .. }) = marco_de_seleccion(&escena, id, 1.0) else {
            panic!("tiene que haber marco");
        };
        assert_eq!(puntos.len(), 5, "cerrado: el ultimo repite el primero");
        assert_eq!(puntos[0], puntos[4]);
        let h = HOLGURA_SELECCION;
        assert_eq!((puntos[0].x, puntos[0].y), (x0 - h, y0 - h));
        assert_eq!((puntos[2].x, puntos[2].y), (x1 + h, y1 + h));

        // Caso negativo: lo borrado no se marca, y un id que no existe
        // tampoco. Si no, quedaria un marco flotando sobre nada.
        escena.borrar(id);
        assert!(marco_de_seleccion(&escena, id, 1.0).is_none());
        assert!(marco_de_seleccion(&escena, 9999, 1.0).is_none());
    }

    fn base() -> Elemento {
        Elemento {
            id: 1,
            figura: Figura::Rectangulo,
            x: 10.0,
            y: 10.0,
            ancho: 100.0,
            alto: 50.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            estilo_relleno: Default::default(),
            relleno: None,
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 99,
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

    fn textos_con_sitio(o: &[Orden]) -> Vec<(String, f32, f32)> {
        o.iter()
            .filter_map(|x| match x {
                Orden::Texto { texto, x, y, .. } => Some((texto.clone(), *x, *y)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn un_texto_centrado_aparta_cada_renglon_y_uno_a_la_izquierda_no() {
        use crate::texto::AlineacionTexto;
        let mut e = Elemento {
            figura: Figura::Texto {
                texto: "abcd\nab".into(),
                tam: 10.0,
                familia: "Excalifont".into(),
            },
            ancho: 24.8,
            ..base()
        };
        // Sin decir nada, una sola orden como siempre.
        assert_eq!(
            textos_con_sitio(&ordenes(&e)),
            vec![("abcd\nab".into(), 10.0, 10.0)]
        );
        e.extras.alineacion = Some(AlineacionTexto::Izquierda);
        assert_eq!(
            textos_con_sitio(&ordenes(&e)).len(),
            1,
            "a la izquierda no se reparte"
        );

        e.extras.alineacion = Some(AlineacionTexto::Centro);
        let t = textos_con_sitio(&ordenes(&e));
        assert_eq!(t.len(), 2, "un renglon por orden");
        assert!((t[0].1 - 10.0).abs() < 1e-3, "el largo llena la caja");
        assert!((t[1].1 - 16.2).abs() < 1e-3, "el corto, centrado: {t:?}");
        // El segundo lleva un salto delante: asi cae en su renglon con el
        // interlineado de quien pinta, sin adivinarlo aqui.
        assert_eq!(t[1].0, "\nab");
        assert_eq!(t[0].2, t[1].2);

        e.extras.alineacion = Some(AlineacionTexto::Derecha);
        let t = textos_con_sitio(&ordenes(&e));
        assert!((t[1].1 - 22.4).abs() < 1e-3, "{t:?}");
        // Y el tachado va con su renglon, no pegado a la izquierda.
        e.extras.tachado = true;
        let raya = ordenes(&e)
            .into_iter()
            .filter_map(|o| match o {
                Orden::Polilinea { puntos, grosor, .. } if grosor < 2.0 => Some(puntos),
                _ => None,
            })
            .next_back()
            .expect("hay tachado");
        assert!((raya[0].x - 22.4).abs() < 1e-3, "{raya:?}");
    }

    #[test]
    fn una_flecha_curva_se_pinta_por_la_curva_y_su_punta_mira_el_final_de_la_curva() {
        let puntos = vec![
            Punto2::nuevo(0.0, 0.0),
            Punto2::nuevo(100.0, 100.0),
            Punto2::nuevo(200.0, 0.0),
        ];
        let flecha = |redondo| Elemento {
            figura: Figura::Flecha {
                puntos: puntos.clone(),
                punta_inicio: TipoPunta::Ninguna,
                punta_fin: TipoPunta::Flecha,
                codos: false,
            },
            redondo,
            rugosidad: 0.0,
            ..base()
        };
        let rayas = |e: &Elemento| -> Vec<Vec<Punto2>> {
            ordenes(e)
                .into_iter()
                .filter_map(|o| match o {
                    Orden::Polilinea { puntos, .. } => Some(puntos),
                    _ => None,
                })
                .collect()
        };
        let curva = rayas(&flecha(true));
        // Lisa: una pasada por la curva entera y la punta.
        assert_eq!(curva[0], crate::curva::muestrear(&puntos));
        assert_eq!(curva[0].last(), Some(&puntos[2]));
        // Caso negativo: la recta sigue siendo sus dos rectas de siempre.
        let recta = rayas(&flecha(false));
        assert_eq!(recta[0], vec![puntos[0], puntos[1]]);
        assert_ne!(curva, recta);
        // La punta cambia: la curva llega al final con otra direccion que
        // la ultima recta de la quebrada.
        assert_ne!(curva.last(), recta.last());
    }

    #[test]
    fn un_elemento_borrado_no_produce_ninguna_orden() {
        let e = Elemento {
            borrado: true,
            ..base()
        };
        assert!(ordenes(&e).is_empty());
    }

    #[test]
    fn el_relleno_se_pinta_antes_que_el_trazo() {
        // Al reves, el relleno taparia el borde y la figura se veria sin
        // contorno.
        let e = Elemento {
            relleno: Some(ColorRgba::opaco(1.0, 1.0, 0.0)),
            estilo_relleno: EstiloRelleno::Solido,
            ..base()
        };
        let o = ordenes(&e);
        assert!(matches!(o[0], Orden::Relleno { .. }), "primero el relleno");
        assert!(o[1..].iter().any(|x| matches!(x, Orden::Polilinea { .. })));
    }

    /// Las polilineas del color del relleno: las rayas, y no el contorno.
    fn rayas_de(e: &Elemento) -> Vec<Vec<Punto2>> {
        let relleno = e.relleno.expect("el elemento del caso tiene relleno");
        ordenes(e)
            .into_iter()
            .filter_map(|o| match o {
                Orden::Polilinea { puntos, color, .. } if color == relleno => Some(puntos),
                _ => None,
            })
            .collect()
    }

    /// Si `q` queda dentro del contorno convexo `c` (cerrado), con una
    /// centesima de holgura por el redondeo de los flotantes.
    fn dentro_de(c: &[Punto2], q: Punto2) -> bool {
        let area: f32 = c
            .windows(2)
            .map(|w| w[0].x * w[1].y - w[1].x * w[0].y)
            .sum();
        c.windows(2).all(|w| {
            let cruz = (w[1].x - w[0].x) * (q.y - w[0].y) - (w[1].y - w[0].y) * (q.x - w[0].x);
            let largo = w[0].distancia(w[1]).max(1e-6);
            cruz * area.signum() / largo >= -0.01
        })
    }

    /// Dentro del rectangulo redondeado de `e`: la ruta de Excalidraw, que
    /// es la que se ve como contorno.
    fn dentro_del_redondeado(e: &Elemento, q: Punto2) -> bool {
        dentro_de(&formas::rectangulo_redondo(e.x, e.y, e.ancho, e.alto), q)
    }

    /// Lo que `q` queda del contorno redondeado, si esta fuera.
    fn fuera_por(e: &Elemento, q: Punto2) -> f32 {
        let c = formas::rectangulo_redondo(e.x, e.y, e.ancho, e.alto);
        c.windows(2)
            .map(|w| crate::vector::distancia_a_segmento(q, w[0], w[1]))
            .fold(f32::MAX, f32::min)
    }

    fn redondo(estilo_relleno: EstiloRelleno, rugosidad: f32) -> Elemento {
        Elemento {
            redondo: true,
            rugosidad,
            relleno: Some(ColorRgba::opaco(1.0, 0.0, 0.0)),
            estilo_relleno,
            ..base()
        }
    }

    #[test]
    fn el_rectangulo_redondo_se_ve_redondo_tambien_a_mano() {
        // Antes solo se redondeaba con rugosidad 0: con el «a mano» de
        // fabrica el boton de bordes no cambiaba nada de lo que se ve.
        for rugosidad in [0.0, 1.0, 2.0] {
            let e = redondo(EstiloRelleno::Solido, rugosidad);
            let pico = Elemento {
                redondo: false,
                ..e.clone()
            };
            assert_ne!(ordenes(&e), ordenes(&pico), "rugosidad {rugosidad}");
            // Ninguna pasada del contorno llega a la esquina de la caja: la
            // mas cercana se queda a mas de medio radio.
            let esquina = Punto2::nuevo(e.x, e.y);
            let cerca = ordenes(&e)
                .into_iter()
                .filter_map(|o| match o {
                    Orden::Polilinea { puntos, color, .. } if color == e.trazo => Some(puntos),
                    _ => None,
                })
                .flatten()
                .map(|p| p.distancia(esquina))
                .fold(f32::MAX, f32::min);
            assert!(
                cerca > 2.0,
                "rugosidad {rugosidad}: el trazo llega al pico ({cerca})"
            );
        }
    }

    #[test]
    fn el_relleno_de_un_redondo_no_se_sale_por_las_esquinas() {
        for rugosidad in [0.0, 1.0, 2.0] {
            let e = redondo(EstiloRelleno::Solido, rugosidad);
            let mancha = ordenes(&e)
                .into_iter()
                .find_map(|o| match o {
                    Orden::Relleno { puntos, .. } => Some(puntos),
                    _ => None,
                })
                .expect("relleno solido");
            for q in &mancha {
                assert!(dentro_del_redondeado(&e, *q), "{q:?} se sale");
            }
            for estilo in [EstiloRelleno::Rayado, EstiloRelleno::Cruzado] {
                let e = redondo(estilo, rugosidad);
                let rayas = rayas_de(&e);
                assert!(!rayas.is_empty());
                // A mano, las rayas son las de rough.js: extremos quietos en
                // el borde y una panza de un par de pixeles que puede rozarlo
                // por fuera, debajo del propio trazo del contorno.
                let holgura = e.grosor * rugosidad;
                for q in rayas.iter().flatten() {
                    assert!(
                        dentro_del_redondeado(&e, *q) || fuera_por(&e, *q) <= holgura,
                        "{estilo:?} r={rugosidad}: {q:?} se sale {}",
                        fuera_por(&e, *q)
                    );
                }
            }
        }
        // Caso negativo: en pico el relleno SI llega a la esquina de la caja.
        let pico = Elemento {
            redondo: false,
            ..redondo(EstiloRelleno::Solido, 0.0)
        };
        let llega = ordenes(&pico).into_iter().any(|o| {
            matches!(o, Orden::Relleno { puntos, .. }
                if puntos.contains(&Punto2::nuevo(pico.x, pico.y)))
        });
        assert!(llega);
    }

    #[test]
    fn el_rombo_redondo_rellena_y_raya_por_su_ruta_redondeada() {
        let e = Elemento {
            figura: Figura::Rombo,
            ..redondo(EstiloRelleno::Solido, 1.0)
        };
        let contorno = formas::rombo_redondo(e.x, e.y, e.ancho, e.alto);
        let mancha = ordenes(&e)
            .into_iter()
            .find_map(|o| match o {
                Orden::Relleno { puntos, .. } => Some(puntos),
                _ => None,
            })
            .unwrap();
        assert_eq!(mancha, contorno, "el relleno va por la misma ruta");
        let e = Elemento {
            estilo_relleno: EstiloRelleno::Cruzado,
            ..e
        };
        let rayas = rayas_de(&e);
        assert!(!rayas.is_empty());
        for q in rayas.iter().flatten() {
            assert!(
                dentro_de(&contorno, *q),
                "una raya del rombo redondo se sale: {q:?}"
            );
        }
    }

    #[test]
    fn un_relleno_rayado_sale_como_rayas_y_no_como_mancha() {
        // Lo que faltaba: un dibujo rayado del movil se veia aqui como una
        // mancha de color plano.
        let e = Elemento {
            relleno: Some(ColorRgba::opaco(1.0, 1.0, 0.0)),
            estilo_relleno: EstiloRelleno::Rayado,
            ..base()
        };
        let o = ordenes(&e);
        assert!(
            !o.iter().any(|x| matches!(x, Orden::Relleno { .. })),
            "el rayado no puede traer ademas la mancha solida"
        );
        let rayas = rayas_de(&e);
        assert!(rayas.len() > 3, "solo salieron {} rayas", rayas.len());
        // Cada raya es la de rough.js: dos pasadas de una cubica casi recta
        // (antes, un segmento con los extremos sacudidos que se salia).
        for r in &rayas {
            assert!(r.len() >= 2);
            let (a, b) = (r[0], *r.last().unwrap());
            for q in r {
                assert!(
                    crate::vector::distancia_a_segmento(*q, a, b) < 3.0,
                    "la raya se tuerce"
                );
            }
        }
        // Y van antes del contorno, como el relleno solido.
        assert!(matches!(o[0], Orden::Polilinea { .. }));
    }

    #[test]
    fn las_rayas_del_relleno_son_mas_finas_que_el_contorno() {
        // `fillWeight: strokeWidth / 2`: con la pluma del contorno, el
        // interior pesaria mas que la propia figura.
        let e = Elemento {
            relleno: Some(ColorRgba::opaco(1.0, 1.0, 0.0)),
            estilo_relleno: EstiloRelleno::Rayado,
            grosor: 4.0,
            ..base()
        };
        for o in ordenes(&e) {
            if let Orden::Polilinea { color, grosor, .. } = o {
                if color == e.relleno.unwrap() {
                    assert_eq!(grosor, 2.0, "la raya tiene que ir a media pluma");
                }
            }
        }
    }

    #[test]
    fn una_figura_sin_relleno_no_produce_ni_una_raya() {
        // Caso negativo: el estilo de relleno por omision es rayado, asi que
        // sin esto CUALQUIER rectangulo saldria rayado por dentro.
        let e = base();
        assert!(e.relleno.is_none());
        assert_eq!(e.estilo_relleno, EstiloRelleno::Rayado);
        let solo_contorno = ordenes(&e);
        let sin_estilo = ordenes(&Elemento {
            estilo_relleno: EstiloRelleno::Solido,
            ..base()
        });
        assert_eq!(
            solo_contorno, sin_estilo,
            "sin relleno, el estilo de relleno no puede cambiar nada"
        );
    }

    #[test]
    fn el_rayado_de_una_elipse_se_queda_dentro_de_la_elipse() {
        let e = Elemento {
            figura: Figura::Elipse,
            relleno: Some(ColorRgba::opaco(0.0, 0.0, 1.0)),
            estilo_relleno: EstiloRelleno::Cruzado,
            rugosidad: 0.0,
            ..base()
        };
        // base(): x 10, y 10, ancho 100, alto 50.
        let (cx, cy, rx, ry) = (60.0f32, 35.0f32, 50.0f32, 25.0f32);
        let rayas = rayas_de(&e);
        assert!(!rayas.is_empty(), "una elipse rellena tiene que rayarse");
        for p in rayas.iter().flatten() {
            let dentro = ((p.x - cx) / rx).powi(2) + ((p.y - cy) / ry).powi(2);
            assert!(dentro <= 1.01, "la raya sale de la elipse en {p:?}");
        }
    }

    #[test]
    fn rellenar_una_figura_no_le_cambia_el_garabato_del_contorno() {
        // El rayado usa su propio generador: si gastara numeros del azar del
        // elemento, rellenar una figura ya dibujada la redibujaria con otro
        // temblor y el dibujo daria un salto.
        let contorno_de = |e: &Elemento| -> Vec<Vec<Punto2>> {
            let tinta = e.trazo;
            ordenes(e)
                .into_iter()
                .filter_map(|o| match o {
                    Orden::Polilinea { puntos, color, .. } if color == tinta => Some(puntos),
                    _ => None,
                })
                .collect()
        };
        let vacia = base();
        let rayada = Elemento {
            relleno: Some(ColorRgba::opaco(1.0, 0.0, 0.0)),
            estilo_relleno: EstiloRelleno::Rayado,
            ..base()
        };
        assert_eq!(contorno_de(&vacia), contorno_de(&rayada));
    }

    #[test]
    fn el_mismo_elemento_produce_siempre_las_mismas_ordenes() {
        // La puerta de la fase, a nivel de dibujo: reabrir el documento no
        // cambia ni un punto.
        let e = base();
        assert_eq!(ordenes(&e), ordenes(&e));
    }

    #[test]
    fn cambiar_la_semilla_cambia_el_dibujo() {
        // Caso negativo: si la semilla no llegara hasta aqui, todas las
        // figuras del documento saldrian calcadas.
        let a = base();
        let b = Elemento {
            semilla: 100,
            ..base()
        };
        assert_ne!(ordenes(&a), ordenes(&b));
    }

    #[test]
    fn el_resaltador_es_translucido_al_cuarenta_por_ciento_como_el_movil() {
        // D45: si adelgazara o fuera opaco, el texto de debajo quedaria
        // ilegible, que es justo lo contrario de resaltar.
        let e = Elemento {
            figura: Figura::Resaltador {
                puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0)],
            },
            trazo: ColorRgba::opaco(1.0, 1.0, 0.0),
            ..base()
        };
        match &ordenes(&e)[0] {
            Orden::Tinta { color, .. } => {
                assert!(
                    (color.a - 0.40).abs() < 1e-6,
                    "HIGHLIGHTER_OPACITY es 40: alfa {}",
                    color.a
                )
            }
            otra => panic!("el resaltador deberia ser tinta, es {otra:?}"),
        }
    }

    #[test]
    fn una_flecha_con_las_dos_puntas_dibuja_mas_que_una_con_una() {
        let con_una = Elemento {
            figura: Figura::Flecha {
                puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0)],
                punta_inicio: TipoPunta::Ninguna,
                punta_fin: TipoPunta::Flecha,
                codos: false,
            },
            ..base()
        };
        let con_dos = Elemento {
            figura: Figura::Flecha {
                puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0)],
                punta_inicio: TipoPunta::Flecha,
                punta_fin: TipoPunta::Flecha,
                codos: false,
            },
            ..base()
        };
        assert!(ordenes(&con_dos).len() > ordenes(&con_una).len());
    }

    #[test]
    fn la_opacidad_del_elemento_llega_al_color() {
        let e = Elemento {
            opacidad: 0.5,
            ..base()
        };
        match &ordenes(&e)[0] {
            Orden::Polilinea { color, .. } => assert_eq!(color.a, 0.5),
            otra => panic!("se esperaba una polilinea, es {otra:?}"),
        }
    }

    #[test]
    fn la_escena_dibuja_de_abajo_arriba_y_salta_lo_borrado() {
        let mut escena = Escena::nueva();
        escena.anadir(base());
        let dos = escena.anadir(Elemento {
            figura: Figura::Elipse,
            ..base()
        });
        let con_ambos = ordenes_de_escena(&escena).len();
        escena.borrar(dos);
        assert!(ordenes_de_escena(&escena).len() < con_ambos);
    }

    #[test]
    fn un_emoji_se_pinta_como_texto_con_la_fuente_de_emojis_a_su_tamano() {
        let e = Elemento {
            figura: Figura::Emoji {
                caracter: "🪐".into(),
            },
            x: 10.0,
            y: 20.0,
            ancho: 100.0,
            alto: 100.0,
            ..base()
        };
        let o = ordenes(&e);
        assert!(o.iter().any(|o| matches!(o,
            Orden::Texto { texto, familia, tam, .. }
                if texto == "🪐" && familia == "Segoe UI Emoji" && (*tam - 80.0).abs() < 0.01)));
        // Caso negativo: un emoji no tiene contorno que pintar.
        assert!(
            !o.iter().any(|o| matches!(o, Orden::Polilinea { .. })),
            "un emoji no lleva trazo"
        );
    }

    #[test]
    fn un_texto_de_figura_produce_una_orden_de_texto_con_su_ancho() {
        // El suelto no se parte (`SIN_PARTIR`, en `texto_como_excalidraw.rs`);
        // el de dentro de una figura se parte al ancho de su caja.
        let mut e = Elemento {
            figura: Figura::Texto {
                texto: "hola".into(),
                tam: 14.0,
                familia: "Segoe UI".into(),
            },
            ..base()
        };
        e.extras.contenedor = Some("caja".into());
        match &ordenes(&e)[0] {
            Orden::Texto {
                texto, ancho_max, ..
            } => {
                assert_eq!(texto, "hola");
                assert_eq!(*ancho_max, 100.0);
            }
            otra => panic!("se esperaba texto, es {otra:?}"),
        }
    }

    /// Un foco como los que deja la varita: marco de 100 x 50 y, dentro, el
    /// hueco de 40 x 20 centrado en (60, 45).
    fn foco(redonda: bool) -> Elemento {
        Elemento {
            figura: Figura::Foco {
                cristal: crate::lupa_elemento::Cristal {
                    foco: Some(Punto2::nuevo(60.0, 45.0)),
                    foco_ancho: Some(40.0),
                    foco_alto: Some(20.0),
                    redonda,
                    ..Default::default()
                },
            },
            x: 10.0,
            y: 20.0,
            ancho: 100.0,
            alto: 50.0,
            rugosidad: 0.0,
            ..base()
        }
    }

    /// Lo que pinta un foco: el anillo y su color.
    fn anillo(e: &Elemento) -> (Vec<Punto2>, ColorRgba) {
        let o = ordenes(e);
        assert_eq!(o.len(), 1, "un foco es solo su sombra: {o:?}");
        let Orden::Relleno { puntos, color } = &o[0] else {
            panic!("el foco deberia ser un relleno, fue {:?}", o[0]);
        };
        (puntos.clone(), *color)
    }

    #[test]
    fn el_foco_oscurece_solo_dentro_de_su_marco_y_no_el_lienzo_entero() {
        // `drawSpotlights` del movil: el marco menos el hueco. Nada del
        // anillo sale de la caja del elemento (caso negativo del velo viejo,
        // que oscurecia hasta el borde de la ventana).
        let (puntos, color) = anillo(&foco(false));
        for p in &puntos {
            assert!(
                p.x >= 10.0 && p.x <= 110.0 && p.y >= 20.0 && p.y <= 70.0,
                "{p:?} fuera del marco"
            );
        }
        // Lleva las cuatro esquinas del marco y las cuatro del hueco.
        for esquina in [(10.0, 20.0), (110.0, 70.0), (40.0, 35.0), (80.0, 55.0)] {
            assert!(
                puntos.contains(&Punto2::nuevo(esquina.0, esquina.1)),
                "falta {esquina:?} en {puntos:?}"
            );
        }
        assert_eq!(color.r + color.g + color.b, 0.0, "sombra negra");
    }

    #[test]
    fn el_hueco_de_un_foco_redondo_es_un_ovalo() {
        let (recto, _) = anillo(&foco(false));
        let (redondo, _) = anillo(&foco(true));
        // Un ovalo lleva muchos mas vertices que un recuadro.
        assert!(
            redondo.len() > recto.len() + 16,
            "{} frente a {}",
            redondo.len(),
            recto.len()
        );
    }

    #[test]
    fn el_foco_oscurece_lo_que_dice_el_suyo_y_no_su_opacidad() {
        // De fabrica 45, como el movil.
        let (_, color) = anillo(&foco(false));
        let esperado = crate::lupa_elemento::OSCURECER_POR_DEFECTO as f32 / 100.0;
        assert!((color.a - esperado).abs() < 1e-6, "oscurece {}", color.a);
        // Con su numero puesto, ese; y la opacidad del elemento no cuenta.
        let mut e = foco(false);
        if let Figura::Foco { cristal } = &mut e.figura {
            cristal.oscurecer = Some(70);
        }
        e.opacidad = 0.2;
        let (_, color) = anillo(&e);
        assert!((color.a - 0.70).abs() < 1e-6, "oscurece {}", color.a);
    }

    /// Todos los puntos de una lista de ordenes, para poder compararlas.
    fn puntos_de(ordenes: &[Orden]) -> Vec<Punto2> {
        ordenes
            .iter()
            .flat_map(|o| match o {
                Orden::Poligono { puntos, .. }
                | Orden::Tinta {
                    contorno: puntos, ..
                }
                | Orden::Polilinea { puntos, .. }
                | Orden::Relleno { puntos, .. }
                | Orden::Velo { hueco: puntos, .. } => puntos.clone(),
                Orden::Texto { x, y, .. } => vec![Punto2::nuevo(*x, *y)],
                Orden::Rotulo { centro, .. } => vec![*centro],
                Orden::Imagen { x, y, .. } => vec![Punto2::nuevo(*x, *y)],
            })
            .collect()
    }

    #[test]
    fn un_elemento_girado_se_dibuja_girado() {
        // El fallo que motiva esta tarea: hasta ahora `angulo` no llegaba al
        // dibujo, asi que un plano girado en el movil se abria sin girar.
        let recto = base();
        let mut girado = base();
        girado.angulo = std::f32::consts::FRAC_PI_2;

        let a = puntos_de(&ordenes(&recto));
        let b = puntos_de(&ordenes(&girado));

        assert_eq!(a.len(), b.len(), "la misma geometria, en otro sitio");
        assert!(
            a.iter().zip(&b).any(|(p, q)| p.distancia(*q) > 1.0),
            "girar un cuarto de vuelta tiene que mover algo"
        );
    }

    #[test]
    fn girar_no_cambia_el_garabato_solo_lo_orienta() {
        // D38: la semilla manda sobre el aspecto. Si el angulo entrara en los
        // generadores de rugosidad, girar un rectangulo lo redibujaria distinto
        // y el dibujo "temblaria" al girarlo.
        let recto = base();
        let mut girado = base();
        girado.angulo = 0.7;

        let (x0, y0, x1, y1) = recto.caja();
        let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);

        let a = puntos_de(&ordenes(&recto));
        let b = puntos_de(&ordenes(&girado));

        for (p, q) in a.iter().zip(&b) {
            let esperado = p.girar(centro, 0.7);
            assert!(
                esperado.distancia(*q) < 1e-3,
                "cada punto es el mismo, girado: esperaba {esperado:?}, es {q:?}"
            );
        }
    }

    #[test]
    fn un_elemento_sin_girar_produce_exactamente_lo_de_antes() {
        // La red de seguridad: con angulo cero, ni un punto se mueve. Todo lo
        // que ya funcionaba tiene que seguir dando byte a byte lo mismo.
        let e = base();
        assert_eq!(e.angulo, 0.0, "la base no esta girada");
        let dos_veces = ordenes(&e);
        assert_eq!(ordenes(&e), dos_veces, "y sigue siendo reproducible");
    }

    #[test]
    fn un_lapiz_se_pinta_como_tinta_y_no_como_poligono() {
        let e = Elemento {
            figura: Figura::Lapiz {
                puntos: (0..20)
                    .map(|i| Punto2::nuevo(i as f32 * 4.0, 0.0))
                    .collect(),
                presiones: Vec::new(),
                opciones: Some(crate::tinta::OpcionesTinta::default()),
            },
            ..base()
        };
        assert!(matches!(ordenes(&e).first(), Some(Orden::Tinta { .. })));
    }

    #[test]
    fn una_firma_de_cuarenta_pixeles_sigue_siendo_tinta() {
        // Antes de E1, por debajo de 48 px salia una raya de grosor fijo: las
        // letras y las firmas pequenas nunca se veian como tinta.
        let e = Elemento {
            figura: Figura::Lapiz {
                puntos: (0..20)
                    .map(|i| Punto2::nuevo(i as f32 * 2.0, (i as f32).sin() * 5.0))
                    .collect(),
                presiones: Vec::new(),
                opciones: Some(crate::tinta::OpcionesTinta::default()),
            },
            ..base()
        };
        assert!(matches!(
            ordenes_a_distancia(&e, 1.0).first(),
            Some(Orden::Tinta { .. })
        ));
    }

    #[test]
    fn el_trazo_a_mano_tambien_se_gira() {
        // Es el caso que junta esta tarea con la 4: los puntos van en marco
        // local y es el dibujo quien los orienta.
        let mut e = Elemento {
            figura: Figura::Lapiz {
                puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0)],
                presiones: Vec::new(),
                opciones: None,
            },
            ..base()
        };
        let recto = puntos_de(&ordenes(&e));

        e.angulo = std::f32::consts::FRAC_PI_2;
        let girado = puntos_de(&ordenes(&e));

        assert_eq!(recto.len(), girado.len());
        assert!(
            recto
                .iter()
                .zip(&girado)
                .any(|(p, q)| p.distancia(*q) > 1.0),
            "un trazo girado no se dibuja igual que uno recto"
        );
    }

    #[test]
    fn el_trazo_simplificado_a_linea_tambien_se_gira() {
        // `ordenes_a_distancia` no siempre delega en `ordenes`: la rama que
        // convierte tinta pequena en linea construye su propia orden a partir
        // de los puntos crudos, y esos tambien estan en marco local.
        let mut e = Elemento {
            figura: Figura::Lapiz {
                puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(10.0, 0.0)],
                presiones: Vec::new(),
                opciones: None,
            },
            ..base()
        };
        let recto = puntos_de(&ordenes_a_distancia(&e, 1.0));

        e.angulo = std::f32::consts::FRAC_PI_2;
        let girado = puntos_de(&ordenes_a_distancia(&e, 1.0));

        assert_eq!(recto.len(), girado.len());
        assert!(
            recto
                .iter()
                .zip(&girado)
                .any(|(p, q)| p.distancia(*q) > 1.0),
            "un trazo simplificado y girado no se dibuja igual que uno recto"
        );
    }

    #[test]
    fn el_dibujo_y_el_picado_coinciden_en_un_elemento_girado() {
        // La prueba que cierra el agujero: donde se ve es donde se toca.
        // Antes de esta tarea, `impacto` des-giraba y `pintado` no giraba, asi
        // que el elemento se veia en un sitio y se tocaba en otro.
        let mut e = base();
        e.angulo = std::f32::consts::FRAC_PI_2;
        e.relleno = Some(ColorRgba::opaco(1.0, 0.0, 0.0));

        for p in puntos_de(&ordenes(&e)) {
            assert!(
                crate::impacto::toca(&e, p),
                "se dibuja en {p:?} pero ahi no se toca"
            );
        }
    }

    fn cota_de(a: Punto2, b: Punto2) -> Elemento {
        Elemento {
            figura: Figura::Cota { puntos: vec![a, b] },
            x: a.x.min(b.x),
            y: a.y.min(b.y),
            ancho: (b.x - a.x).abs(),
            alto: (b.y - a.y).abs(),
            ..base()
        }
    }

    fn metros(por_pixel: f32) -> Escala {
        Escala {
            unidades_por_pixel: por_pixel,
            unidad: "m".to_string(),
            decimales: 2,
        }
    }

    fn textos_de(ordenes: &[Orden]) -> Vec<String> {
        ordenes
            .iter()
            .filter_map(|o| match o {
                Orden::Texto { texto, .. } | Orden::Rotulo { texto, .. } => Some(texto.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn el_rotulo_dice_pixeles_sin_escala_y_unidades_con_ella() {
        let c = cota_de(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0));

        let sin = textos_de(&ordenes_medibles(&c, None, ',', BLANCO));
        assert_eq!(sin, vec!["100 px".to_string()]);

        let con = textos_de(&ordenes_medibles(&c, Some(&metros(0.01)), ',', BLANCO));
        assert_eq!(con, vec!["1,00 m".to_string()]);
    }

    #[test]
    fn una_cota_no_puede_mentir() {
        // LA prueba que justifica no guardar el texto (D34). Se mueve un
        // extremo y el numero cambia solo, porque se deriva al pintar.
        let mut c = cota_de(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0));
        let e = metros(0.01);
        let antes = textos_de(&ordenes_medibles(&c, Some(&e), ',', BLANCO));

        let Figura::Cota { puntos } = &mut c.figura else {
            panic!()
        };
        puntos[1] = Punto2::nuevo(200.0, 0.0);

        let despues = textos_de(&ordenes_medibles(&c, Some(&e), ',', BLANCO));
        assert_ne!(antes, despues, "el rotulo tiene que haber cambiado");
        assert_eq!(despues, vec!["2,00 m".to_string()]);
    }

    #[test]
    fn calibrar_cambia_los_rotulos_sin_tocar_el_elemento() {
        // El motivo de sacar el rotulo de la cache: la escala cambia y el
        // elemento no, asi que la version no sube y la cache no se enteraria.
        let c = cota_de(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0));
        let a = textos_de(&ordenes_medibles(&c, Some(&metros(0.01)), ',', BLANCO));
        let b = textos_de(&ordenes_medibles(&c, Some(&metros(0.02)), ',', BLANCO));
        assert_ne!(a, b, "otra escala, otro rotulo");
    }

    #[test]
    fn una_barra_sin_escala_no_dibuja_cuadros() {
        // Una barra que no puede decir cuanto mide cada cuadro es un adorno
        // que engana.
        let b = Elemento {
            figura: Figura::EscalaGrafica,
            x: 0.0,
            y: 0.0,
            ancho: 400.0,
            alto: 24.0,
            ..base()
        };
        assert!(ordenes_medibles(&b, None, ',', BLANCO).is_empty());
    }

    #[test]
    fn una_barra_con_escala_dibuja_sus_cuadros_y_sus_numeros() {
        let b = Elemento {
            figura: Figura::EscalaGrafica,
            x: 0.0,
            y: 0.0,
            ancho: 400.0,
            alto: 24.0,
            ..base()
        };
        let o = ordenes_medibles(&b, Some(&metros(0.01)), ',', BLANCO);
        let rellenos = o
            .iter()
            .filter(|x| matches!(x, Orden::Relleno { .. }))
            .count();
        assert!(rellenos >= 2, "al menos dos cuadros: {rellenos}");
        let textos = textos_de(&o);
        assert!(!textos.is_empty(), "y sus numeros");
        assert!(
            textos.iter().any(|t| t.contains('m')),
            "con la unidad: {textos:?}"
        );
    }

    #[test]
    fn los_cuadros_de_la_barra_alternan() {
        // La reglita es a cuadros blancos y negros: si fueran todos iguales
        // no se podrian contar.
        let b = Elemento {
            figura: Figura::EscalaGrafica,
            x: 0.0,
            y: 0.0,
            ancho: 400.0,
            alto: 24.0,
            ..base()
        };
        let o = ordenes_medibles(&b, Some(&metros(0.01)), ',', BLANCO);
        let colores: Vec<ColorRgba> = o
            .iter()
            .filter_map(|x| match x {
                Orden::Relleno { color, .. } => Some(*color),
                _ => None,
            })
            .collect();
        assert!(colores.len() >= 2);
        assert_ne!(
            colores[0], colores[1],
            "el segundo cuadro es del otro color"
        );
    }

    #[test]
    fn una_figura_que_no_mide_no_produce_ordenes_medibles() {
        assert!(ordenes_medibles(&base(), Some(&metros(0.01)), ',', BLANCO).is_empty());
    }

    #[test]
    fn una_barra_girada_dibuja_sus_cuadros_girados() {
        let recta = Elemento {
            figura: Figura::EscalaGrafica,
            x: 0.0,
            y: 0.0,
            ancho: 400.0,
            alto: 24.0,
            ..base()
        };
        let girada = Elemento {
            angulo: std::f32::consts::FRAC_PI_2,
            ..recta.clone()
        };

        let a = puntos_de(&ordenes_medibles(&recta, Some(&metros(0.01)), ',', BLANCO));
        let b = puntos_de(&ordenes_medibles(&girada, Some(&metros(0.01)), ',', BLANCO));

        assert_eq!(a.len(), b.len(), "la misma geometria, en otro sitio");
        assert!(
            a.iter().zip(&b).any(|(p, q)| p.distancia(*q) > 1.0),
            "girar la barra tiene que mover algo"
        );
    }

    #[test]
    fn ordenes_medibles_con_escala_invalida_cae_a_pixeles_y_la_barra_no_dibuja() {
        let invalida = Escala {
            unidades_por_pixel: 0.0,
            unidad: "m".to_string(),
            decimales: 2,
        };
        assert!(
            !invalida.valida(),
            "la escala del caso tiene que ser invalida"
        );

        let c = cota_de(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0));
        let textos = textos_de(&ordenes_medibles(&c, Some(&invalida), ',', BLANCO));
        assert_eq!(
            textos,
            vec!["100 px".to_string()],
            "escala invalida se trata como si no hubiera escala"
        );

        let b = Elemento {
            figura: Figura::EscalaGrafica,
            x: 0.0,
            y: 0.0,
            ancho: 400.0,
            alto: 24.0,
            ..base()
        };
        assert!(
            ordenes_medibles(&b, Some(&invalida), ',', BLANCO).is_empty(),
            "una barra que no puede decir cuanto mide cada cuadro no dibuja nada"
        );
    }

    #[test]
    fn una_escena_con_cota_y_barra_saca_texto_y_cuadros_por_los_dos_caminos() {
        // El hallazgo 2 de la revision final: `ordenes_medibles` es la unica
        // que produce el rotulo de una cota y los cuadros de una barra, y
        // solo la llamaba la ventana del editor. La capa de pantalla, la
        // recarga de un pin y la exportacion pasan por `ordenes_de_escena` o
        // `ordenes_de_escena_vista`, que no la encadenaban: una escena con
        // una barra calibrada salia sin un solo texto, y la barra sin sus
        // cuadros era invisible del todo (D37: «sale en la exportacion»).
        let mut escena = Escena::nueva();
        escena.escala = Some(metros(0.01));
        escena.anadir(cota_de(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0)));
        escena.anadir(Elemento {
            figura: Figura::EscalaGrafica,
            x: 200.0,
            y: 0.0,
            ancho: 400.0,
            alto: 24.0,
            ..base()
        });

        let de_escena = ordenes_de_escena(&escena);
        assert!(
            de_escena.iter().any(|o| matches!(o, Orden::Texto { .. })),
            "el rotulo de la cota tiene que salir en ordenes_de_escena"
        );
        assert!(
            de_escena
                .iter()
                .filter(|o| matches!(o, Orden::Relleno { .. }))
                .count()
                >= 2,
            "los cuadros de la barra tienen que salir en ordenes_de_escena"
        );

        let camara = crate::camara::Camara {
            x: 200.0,
            y: 0.0,
            zoom: 1.0,
        };
        let de_vista = ordenes_de_escena_vista(&escena, &camara, 2000.0, 2000.0);
        assert!(
            de_vista.iter().any(|o| matches!(o, Orden::Texto { .. })),
            "el rotulo de la cota tiene que salir en ordenes_de_escena_vista"
        );
        assert!(
            de_vista
                .iter()
                .filter(|o| matches!(o, Orden::Relleno { .. }))
                .count()
                >= 2,
            "los cuadros de la barra tienen que salir en ordenes_de_escena_vista"
        );

        for o in de_escena.iter().chain(de_vista.iter()) {
            if let Orden::Texto { texto, .. } = o {
                assert!(
                    texto.contains(','),
                    "el separador decimal es la coma (D39): {texto}"
                );
            }
        }
    }

    // ---- La cota como la del movil (`drawMeasure`, 27-sep-2026) ----

    const BLANCO: ColorRgba = ColorRgba::opaco(1.0, 1.0, 1.0);

    fn hex(h: u32) -> ColorRgba {
        ColorRgba::opaco(
            ((h >> 16) & 0xff) as f32 / 255.0,
            ((h >> 8) & 0xff) as f32 / 255.0,
            (h & 0xff) as f32 / 255.0,
        )
    }

    /// El rotulo de una cota: `(texto, color, halo, grosor_halo, centro, angulo)`.
    fn rotulo_de(o: &[Orden]) -> (String, ColorRgba, ColorRgba, f32, Punto2, f32) {
        let mut rotulos = o.iter().filter_map(|x| match x {
            Orden::Rotulo {
                texto,
                color,
                halo,
                grosor_halo,
                centro,
                angulo,
                ..
            } => Some((texto.clone(), *color, *halo, *grosor_halo, *centro, *angulo)),
            _ => None,
        });
        let r = rotulos.next().expect("hay rotulo");
        assert!(rotulos.next().is_none(), "uno solo");
        r
    }

    fn grados(rad: f32) -> f32 {
        rad.to_degrees().rem_euclid(360.0)
    }

    #[test]
    fn la_cota_entera_sale_de_ordenes_medibles_y_no_de_la_cache() {
        let c = cota_de(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(400.0, 0.0));
        assert!(
            ordenes(&c).is_empty(),
            "su tinta depende del papel: fuera de la cache"
        );
        let o = ordenes_medibles(&c, None, ',', BLANCO);
        // Dos trozos de raya, dos banderines y cuatro medias puntas, cada uno
        // con sus dos pasadas a mano.
        let rayas = o
            .iter()
            .filter(|x| matches!(x, Orden::Polilinea { .. }))
            .count();
        assert_eq!(rayas, (2 + 2 + 4) * 2, "{rayas}");
        rotulo_de(&o);
    }

    #[test]
    fn la_raya_se_abre_en_el_medio_para_el_numero_y_una_corta_no() {
        let c = cota_de(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(400.0, 0.0));
        let o = ordenes_medibles(&c, Some(&metros(0.01)), ',', BLANCO);
        let (_, _, _, _, centro, _) = rotulo_de(&o);
        assert_eq!(
            centro,
            Punto2::nuevo(200.0, 0.0),
            "el numero, en medio de la raya"
        );
        let rayas: Vec<Orden> = o
            .iter()
            .filter(|x| matches!(x, Orden::Polilinea { .. }))
            .cloned()
            .collect();
        let cerca_del_medio = puntos_de(&rayas).iter().any(|p| (p.x - 200.0).abs() < 20.0);
        assert!(!cerca_del_medio, "la raya deja el hueco del numero");

        // Una de 40 px no cabe: raya entera y el numero encima.
        let corta = cota_de(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(40.0, 0.0));
        let o = ordenes_medibles(&corta, Some(&metros(0.01)), ',', BLANCO);
        assert!(
            puntos_de(&o)
                .iter()
                .any(|p| (p.x - 20.0).abs() < 2.0 && p.y.abs() < 3.0)
        );
    }

    #[test]
    fn el_numero_va_a_lo_largo_de_la_cota_y_nunca_boca_abajo() {
        let angulo = |a: (f32, f32), b: (f32, f32)| {
            let c = cota_de(Punto2::nuevo(a.0, a.1), Punto2::nuevo(b.0, b.1));
            grados(rotulo_de(&ordenes_medibles(&c, None, ',', BLANCO)).5)
        };
        let cerca = |g: f32, esperado: f32| {
            let d = (g - esperado).rem_euclid(360.0);
            d.min(360.0 - d) < 0.1
        };
        assert!(cerca(angulo((0.0, 0.0), (100.0, 0.0)), 0.0));
        // Hacia la izquierda: media vuelta, para que no se lea del reves.
        assert!(cerca(angulo((100.0, 0.0), (0.0, 0.0)), 0.0));
        // Vertical hacia abajo: a lo largo, 90 grados (en el limite no se voltea).
        assert!(cerca(angulo((0.0, 0.0), (0.0, 36.6)), 90.0));
        // La del usuario, un pelo hacia la izquierda: se voltea y se lee de
        // abajo arriba (91 + 180).
        let g = angulo((0.0, 0.0), (-0.6955, 36.6305));
        assert!(cerca(g, 271.09), "{g}");
        // En diagonal, con la raya.
        assert!(cerca(angulo((0.0, 0.0), (100.0, 100.0)), 45.0));
    }

    #[test]
    fn el_halo_es_el_color_contrario_de_la_tinta_y_mide_022_de_la_letra() {
        let mut c = cota_de(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(0.0, 36.6));
        c.trazo = hex(0x0edeff);
        c.extras.tam_letra = Some(9.0);
        let (_, color, halo, grosor, _, _) = rotulo_de(&ordenes_medibles(&c, None, ',', BLANCO));
        assert_eq!(
            halo,
            hex(0xffffff),
            "tinta oscura (ya adaptada), halo blanco"
        );
        assert!((grosor - 9.0 * 0.22).abs() < 1e-4, "{grosor}");
        assert_ne!(color, halo);
        // Sin `fontSize`, el tamano de fabrica del movil (20).
        c.extras.tam_letra = None;
        let (_, _, _, grosor, _, _) = rotulo_de(&ordenes_medibles(&c, None, ',', BLANCO));
        assert!((grosor - 20.0 * 0.22).abs() < 1e-4, "{grosor}");
        // Sobre papel de noche el cian se lee tal cual y su halo es negro.
        let (_, color, halo, _, _, _) = rotulo_de(&ordenes_medibles(&c, None, ',', hex(0x121212)));
        assert_eq!((color, halo), (hex(0x0edeff), hex(0x000000)));
    }

    #[test]
    fn el_color_es_el_de_la_cota_adaptado_al_papel_con_escala_o_sin_ella() {
        let mut c = cota_de(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(300.0, 0.0));
        c.trazo = hex(0x0edeff);
        let adaptado = crate::contraste::adaptar(c.trazo, BLANCO);
        for escala in [None, Some(metros(0.01))] {
            let o = ordenes_medibles(&c, escala.as_ref(), ',', BLANCO);
            let (_, color, _, _, _, _) = rotulo_de(&o);
            assert_eq!(
                color, adaptado,
                "ni gris sin escala ni el cian claro tal cual"
            );
            for x in &o {
                if let Orden::Polilinea { color, .. } = x {
                    assert_eq!(*color, adaptado, "la raya, de la misma tinta que el numero");
                }
            }
        }
        // Una tinta que ya se lee no se toca.
        c.trazo = hex(0xe03131);
        let (_, color, _, _, _, _) = rotulo_de(&ordenes_medibles(&c, None, ',', BLANCO));
        assert_eq!(color, hex(0xe03131));
    }

    #[test]
    fn el_rotulo_gira_con_el_elemento_y_sigue_en_medio_de_su_raya() {
        let mut c = cota_de(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0));
        c.angulo = std::f32::consts::FRAC_PI_2;
        let (x0, y0, x1, y1) = c.caja();
        let giro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let o = ordenes_medibles(&c, Some(&metros(0.01)), ',', BLANCO);
        let (texto, _, _, _, centro, angulo) = rotulo_de(&o);
        let esperado = Punto2::nuevo(50.0, 0.0).girar(giro, c.angulo);
        assert!(
            centro.distancia(esperado) < 1e-3,
            "{centro:?} vs {esperado:?}"
        );
        assert!(
            (grados(angulo) - 90.0).abs() < 0.1,
            "a lo largo de la raya girada"
        );
        // Y la caja del renglon se lleva con su centro.
        let Some(Orden::Rotulo { x, y, .. }) = o.iter().find(|x| matches!(x, Orden::Rotulo { .. }))
        else {
            panic!()
        };
        let (ancho, alto) =
            crate::texto::medida(&texto, 20.0, &letra_de_rotulo(&c), Default::default());
        assert!(
            (x + ancho / 2.0 - centro.x).abs() < 1e-3 && (y + alto / 2.0 - centro.y).abs() < 1e-3
        );
    }

    #[test]
    fn una_cota_de_longitud_cero_no_pinta_nada_ni_da_nan() {
        let c = cota_de(Punto2::nuevo(5.0, 5.0), Punto2::nuevo(5.0, 5.0));
        assert!(ordenes_medibles(&c, Some(&metros(0.01)), ',', BLANCO).is_empty());
    }
}

/// **Una foto girada en el movil se ve girada aqui.** La orden de la imagen
/// solo llevaba su esquina: `girar_orden` la movia alrededor del centro y
/// nadie giraba el bitmap, asi que la foto salia derecha (y corrida). Ahora
/// la orden lleva su caja SIN girar y el angulo, y quien pinta gira el
/// bitmap alrededor del centro de esa caja (`canvas.rotate` de
/// `Renderer.kt`, sobre el centro del elemento).
#[cfg(test)]
mod imagen_girada {
    use super::*;

    fn foto(angulo: f32) -> Elemento {
        Elemento {
            id: 1,
            figura: Figura::Imagen { id_objeto: 7 },
            x: 100.0,
            y: 50.0,
            ancho: 400.0,
            alto: 200.0,
            angulo,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            estilo_relleno: Default::default(),
            relleno: None,
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 99,
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

    fn la_imagen(e: &Elemento) -> (f32, f32, f32, f32, f32) {
        match ordenes(e).as_slice() {
            [
                Orden::Imagen {
                    x,
                    y,
                    ancho,
                    alto,
                    angulo,
                    ..
                },
            ] => (*x, *y, *ancho, *alto, *angulo),
            otras => panic!("una imagen es una orden de imagen: {otras:?}"),
        }
    }

    #[test]
    fn una_imagen_girada_lleva_su_angulo_y_su_caja_sin_girar_en_su_sitio() {
        let giro = std::f32::consts::FRAC_PI_2 + 0.01;
        let (x, y, ancho, alto, angulo) = la_imagen(&foto(giro));
        assert!(
            (angulo - giro).abs() < 1e-6,
            "el angulo llega a quien pinta"
        );
        // La caja es la del elemento: el giro va alrededor de su centro.
        assert!(
            (x - 100.0).abs() < 1e-3 && (y - 50.0).abs() < 1e-3,
            "({x}, {y})"
        );
        assert_eq!((ancho, alto), (400.0, 200.0));
    }

    #[test]
    fn una_imagen_derecha_sigue_con_angulo_cero() {
        // Caso negativo: sin giro no cambia nada de lo de siempre.
        assert_eq!(la_imagen(&foto(0.0)), (100.0, 50.0, 400.0, 200.0, 0.0));
    }

    #[test]
    fn girar_una_orden_de_imagen_gira_su_centro_y_suma_el_angulo() {
        // Lo que hace un grupo girado entero: la imagen da la vuelta
        // alrededor de un centro que no es el suyo.
        let mut o = Orden::Imagen {
            id_objeto: 1,
            x: 0.0,
            y: 0.0,
            ancho: 20.0,
            alto: 10.0,
            opacidad: 1.0,
            recorte: None,
            angulo: 0.1,
        };
        girar_orden(&mut o, Punto2::nuevo(0.0, 0.0), std::f32::consts::PI);
        let Orden::Imagen {
            x,
            y,
            ancho,
            alto,
            angulo,
            ..
        } = o
        else {
            unreachable!()
        };
        // El centro (10, 5) da media vuelta hasta (-10, -5).
        assert!(
            (x + 20.0).abs() < 1e-3 && (y + 10.0).abs() < 1e-3,
            "({x}, {y})"
        );
        assert_eq!((ancho, alto), (20.0, 10.0));
        assert!((angulo - (0.1 + std::f32::consts::PI)).abs() < 1e-5);
    }
}

#[cfg(test)]
mod pruebas_pasos_del_arco {
    #[test]
    fn un_arco_grande_lleva_mas_tramos_y_ninguno_largo() {
        assert_eq!(super::pasos_del_arco(20.0), crate::arco::PASOS_DEL_ARCO);
        let n = super::pasos_del_arco(700.0);
        assert!(n > 1000, "{n}");
        assert!(std::f32::consts::TAU * 700.0 / n as f32 <= 3.1);
        assert_eq!(super::pasos_del_arco(1.0e6), 1440);
    }
}
