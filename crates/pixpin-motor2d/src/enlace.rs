//! **La flecha atada a la figura**: que siga a la caja cuando la caja se
//! mueve.
//!
//! Port de la parte de anclaje de `Arrows.kt` del PixPin de Android
//! (`getHoveredElementForBinding`, `bindArrow`, `bindingPointOn`,
//! `updateBoundPoints`) y, por debajo, del `startBinding`/`endBinding` de
//! Excalidraw.
//!
//! # Por que es la pieza que mas se nota cuando falta
//!
//! Un organigrama es cajas y flechas. Sin esto, abrir en el PC un esquema
//! hecho en el movil y mover **una sola caja** descoloca todas las flechas
//! que salian de ella: siguen apuntando a donde la caja estaba. Y no es que
//! el dato se pierda —`startBinding` y `endBinding` viajan intactos desde
//! siempre por el carril de §5.1 del inventario—, es que quedan apuntando
//! donde ya no toca, que es peor: el fichero dice una cosa y el dibujo otra.
//!
//! # Las dos cifras que lo gobiernan
//!
//! - **`hueco` (`gap`)**: cuanto se queda la punta separada del borde. Sale
//!   del grosor de trazo del objetivo, porque es el grosor el que decide
//!   donde queda el borde de verdad.
//! - **`foco` (`focus`)**: cuanto se desvia del centro el punto de contacto,
//!   de -1 a 1, medido perpendicular a la flecha y normalizado contra el
//!   semitamano. Es lo que hace que al **agrandar** la caja la flecha siga
//!   senalando el mismo sitio relativo y no salte al centro.
//!
//! # Lo que aqui no se puede cerrar todavia
//!
//! `Enganche::elemento` guarda el id **de texto** del objetivo, que es como
//! lo escribe el fichero. Para un dibujo que viene del movil eso basta y
//! todo funciona: [`resolver`] deshace ese texto al id interno con la misma
//! cuenta que usa `excalidraw::leer`.
//!
//! Para un enganche nacido **aqui**, entre dos elementos que el PC creo, el
//! texto se inventa ([`id_de_texto`], `pc<hex>`) y **sobrevive a guardar y
//! reabrir**: `excalidraw::sellar` escribe ese mismo texto como `id` del
//! elemento, y al releer [`id_del_fichero`] devuelve el numero de vuelta. No
//! siempre fue asi —el id se inventaba con la hora dentro y cambiaba en cada
//! guardado, con lo que la flecha dejaba de seguir a su caja—; la prueba que
//! lo vigila es `un_enganche_hecho_en_el_pc_sigue_atado_despues_de_guardar_y_
//! reabrir`, en `excalidraw.rs`.
//!
//! Y, pase lo que pase, **un enganche que no resuelve no rompe nada**:
//! [`recolocar`] deja la flecha donde esta y el enganche viaja intacto al
//! movil.

use crate::elemento::{Elemento, Enganche, Figura, ModoEnganche};
use crate::escena::Escena;
use crate::perimetros::{self, PASO_PERIMETRO};
use crate::vector::Punto2;

/// Separacion base entre la punta y el borde (`BASE_BINDING_GAP`).
pub const HUECO_BASE: f32 = 5.0;

/// Cual de los dos extremos de la flecha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extremo {
    Inicio,
    Fin,
}

/// El hueco real contra esta figura (`getBindingGap`).
///
/// Depende de su grosor de trazo porque es el grosor el que decide donde
/// queda el borde que se ve.
pub fn hueco_de(objetivo: &Elemento) -> f32 {
    HUECO_BASE + objetivo.grosor / 2.0
}

/// Hasta donde atrae `objetivo` a una punta que se suelta **por fuera**.
///
/// **Es el propio hueco del anclaje, y ni un pixel mas.** La regla: si la
/// punta ya esta donde el anclaje la dejaria —rozando el borde—, atarla no la
/// mueve de sitio y se gana que la flecha siga a la caja para siempre. Un
/// poco mas lejos, atarla significa **llevarsela**, y ahi deja de ser una
/// ayuda: el sitio donde se posa lo decide un rayo trazado desde el otro
/// extremo, asi que ademas se corre a lo largo del contorno. Sueltas la punta
/// senalando un sitio y aparece senalando otro.
///
/// Se relaja al alejar la vista, hasta el doble: alejado, el mismo hueco son
/// menos pixeles de pantalla y no se acertaria nunca. Al acercar no se
/// aprieta —`z` se topa en uno—, porque de cerca ya se coloca la punta con
/// precision de sobra.
pub fn distancia_maxima(objetivo: &Elemento, zoom: f32) -> f32 {
    let roce = hueco_de(objetivo);
    let z = zoom.clamp(0.0001, 1.0);
    (roce / z).clamp(roce, roce * 2.0)
}

/// Si a esta figura se le puede atar una flecha.
///
/// Lo que encierra un area y se lee como «una cosa»: cajas, rombos, ovalos,
/// imagenes, rotulos, mosaicos y circulos numerados.
///
/// Se quedan fuera, y cada uno por su motivo:
///
/// - **las rayas y las propias flechas**, porque atar una flecha a otra
///   flecha hace cadenas que se persiguen y no se estabilizan;
/// - **el marco**, porque delimita hasta donde llega el dibujo y no es algo
///   dibujado: atar a sus bordes convertiria el papel en un iman;
/// - **el foco**, que es una sombra sobre todo lo demas y no encierra nada;
/// - **el punto etiquetado**, que no tiene area: su caja mide cero y el rayo
///   no cortaria nada.
pub fn se_puede_atar(e: &Elemento) -> bool {
    !e.borrado
        && !e.bloqueado
        && matches!(
            e.figura,
            Figura::Rectangulo
                | Figura::Rombo
                | Figura::Elipse
                | Figura::Imagen { .. }
                | Figura::Texto { .. }
                | Figura::Emoji { .. }
                | Figura::Mosaico { .. }
                | Figura::Serie { .. }
                | Figura::Arco { .. }
                | Figura::Region { .. }
        )
}

/// La figura a la que se engancharia la punta si se soltase en `p`
/// (`getHoveredElementForBinding`).
///
/// **Con la punta dentro tambien ancla**, que es como se comporta el original
/// y lo que lo hace usable: sueltas la flecha encima de la caja, no buscando
/// su borde con el raton.
///
/// Hubo en el movil un intento de probar **solo el contorno**, y venia de un
/// problema real: una flecha que atravesaba un rectangulo relleno para llegar
/// a otro sitio se quedaba atada al del medio. Pero la causa no era probar el
/// interior sino probarlo en el sitio equivocado. Las tres reglas que lo
/// resuelven, y que son las que se copian:
///
/// 1. se prueba **donde acaba la flecha**, no por donde pasa: una flecha que
///    cruza un rectangulo y termina mas alla tiene la punta fuera y no ancla;
/// 2. se recorre de delante hacia atras y **una figura opaca corta la
///    busqueda**: lo que queda tapado detras de ella no es candidato, porque
///    no se ve;
/// 3. entre los candidatos gana **el mas pequeno**: con una caja dentro de
///    otra, anclar a la de fuera nunca es lo que se quiere.
pub fn figura_bajo(elementos: &[Elemento], p: Punto2, zoom: f32, excluir: u64) -> Option<u64> {
    // Sin lista de candidatos: se lleva el mejor sobre la marcha. Esto corre
    // en cada aviso del raton mientras se dibuja una flecha o se arrastra su
    // punta, y una lista nueva por aviso es memoria pedida en el camino
    // caliente.
    let mut mejor: Option<(u64, f32)> = None;
    for e in elementos.iter().rev() {
        if e.id == excluir || !se_puede_atar(e) {
            continue;
        }
        // El rotulo de dentro de una caja no recibe flechas: la flecha va a
        // la caja, que es la que lo lleva. Atada al rotulo, se quedaria
        // apuntando al texto cuando la caja crece.
        if matches!(e.figura, Figura::Texto { .. }) && e.extras.contenedor.is_some() {
            continue;
        }
        let alcance = distancia_maxima(e, zoom);
        // Criba barata antes de la cara: un circulo que envuelve la figura
        // aunque este girada. Recortar el contorno de las dos mil figuras de
        // un plano en cada aviso del raton costaria milisegundos; con esto
        // solo pagan las que estan cerca de la punta.
        let (x0, y0, x1, y1) = e.caja();
        let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let radio = Punto2::nuevo(x0, y0).distancia(Punto2::nuevo(x1, y1)) / 2.0;
        if p.distancia(centro) > radio + alcance {
            continue;
        }
        let dentro = crate::impacto::dentro_de_la_caja_girada(e, p);
        if !dentro && distancia_al_borde(e, p) > alcance {
            continue;
        }
        // Regla 3: gana el mas pequeno. Con empate, el de delante, que es el
        // que se ve.
        let area = (x1 - x0) * (y1 - y0);
        if mejor.is_none_or(|(_, a)| area < a) {
            mejor = Some((e.id, area));
        }
        // Regla 2: lo que quede detras de una figura opaca no se ve, asi que
        // no es candidato.
        if dentro && e.tiene_relleno() {
            break;
        }
    }
    mejor.map(|(id, _)| id)
}

/// Los tramos del borde de `e` contra los que se posa una punta.
///
/// Los de [`perimetros`], y para el texto y el emoji —que alli no tienen
/// borde, porque su recuadro no se pinta— **su caja girada**: una flecha
/// atada a un rotulo suelto se posa donde acaba el rotulo, que es como lo
/// hace Excalidraw. Sin esto el rayo no cortaba nada y la punta se iba al
/// centro del texto, encima de las letras.
fn silueta(e: &Elemento) -> Vec<(Punto2, Punto2)> {
    let tramos = perimetros::segmentos_de(e, PASO_PERIMETRO);
    if !tramos.is_empty() {
        return tramos;
    }
    let (x0, y0, x1, y1) = e.caja();
    let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let esquinas = [
        Punto2::nuevo(x0, y0),
        Punto2::nuevo(x1, y0),
        Punto2::nuevo(x1, y1),
        Punto2::nuevo(x0, y1),
    ]
    .map(|q| q.girar(centro, e.angulo));
    (0..4)
        .map(|i| (esquinas[i], esquinas[(i + 1) % 4]))
        .collect()
}

/// Lo que se separa `p` del borde dibujado de `e`. Grande si no hay borde.
fn distancia_al_borde(e: &Elemento, p: Punto2) -> f32 {
    silueta(e)
        .into_iter()
        .map(|(a, b)| crate::vector::distancia_a_segmento(p, a, b))
        .fold(f32::MAX, f32::min)
}

/// El enganche que le tocaria a la punta `punta` de una flecha cuyo otro
/// extremo esta en `otro` (`bindArrow`).
///
/// `id_texto` es el id **del fichero** del objetivo: ver la nota de la
/// cabecera sobre por que no se deduce aqui.
pub fn enganche_para(
    objetivo: &Elemento,
    id_texto: String,
    punta: Punto2,
    otro: Punto2,
) -> Enganche {
    Enganche {
        elemento: id_texto,
        foco: determinar_foco(objetivo, punta, otro),
        hueco: hueco_de(objetivo),
        punto_fijo: Some(proporcion_en(objetivo, punta)),
        modo: modo_para(objetivo, punta),
    }
}

/// `p` como proporcion de la caja de `objetivo`: `(0,0)`..`(1,1)`.
///
/// Se mide **en el sistema sin girar**, deshaciendo su rotacion primero. Asi
/// el punto agarrado sigue siendo el mismo trozo de la figura aunque luego se
/// gire.
fn proporcion_en(objetivo: &Elemento, p: Punto2) -> (f32, f32) {
    let (x0, y0, x1, y1) = objetivo.caja();
    let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let local = p.girar(centro, -objetivo.angulo);
    let ancho = if x1 - x0 > 0.0 { x1 - x0 } else { 1.0 };
    let alto = if y1 - y0 > 0.0 { y1 - y0 } else { 1.0 };
    (
        ((local.x - x0) / ancho).clamp(0.0, 1.0),
        ((local.y - y0) / alto).clamp(0.0, 1.0),
    )
}

/// La punta se queda dentro o se posa en el contorno.
///
/// Dentro si se solto **bien adentro**; en el contorno si se solto en el
/// borde o cerca de el por fuera. El umbral es el mismo hueco de anclaje, asi
/// que la franja que cuenta como «el borde» es la que ya se usa para el
/// resto y no hay que inventar otra medida.
fn modo_para(objetivo: &Elemento, p: Punto2) -> ModoEnganche {
    let en_el_borde = distancia_al_borde(objetivo, p) <= hueco_de(objetivo) * 2.0;
    if !en_el_borde && crate::impacto::dentro_de_la_caja_girada(objetivo, p) {
        ModoEnganche::Dentro
    } else {
        ModoEnganche::Orbita
    }
}

/// Cuanto se desvia del centro el punto de contacto, en -1..1
/// (`determineFocusDistance`).
///
/// Se mide perpendicularmente a la linea de la flecha y se normaliza contra
/// el semitamano de la figura, para que el mismo foco senale el mismo sitio
/// relativo aunque la figura cambie de tamano.
fn determinar_foco(objetivo: &Elemento, punta: Punto2, otro: Punto2) -> f32 {
    let (x0, y0, x1, y1) = objetivo.caja();
    let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let (dx, dy) = (punta.x - otro.x, punta.y - otro.y);
    let largo = (dx * dx + dy * dy).sqrt();
    if largo == 0.0 {
        return 0.0;
    }
    // Distancia con signo del centro a la recta que forma la flecha.
    let con_signo = ((centro.x - otro.x) * dy - (centro.y - otro.y) * dx) / largo;
    let semi = ((x1 - x0) / 2.0).max((y1 - y0) / 2.0);
    if semi == 0.0 {
        return 0.0;
    }
    (-con_signo / semi).clamp(-1.0, 1.0)
}

/// Donde debe posarse la punta sobre el borde de `objetivo` (`bindingPointOn`).
///
/// Se traza un rayo desde `desde` hacia el punto de enfoque y se corta con la
/// silueta; luego se retrocede el hueco. El resultado es que la punta toca el
/// borde sin invadirlo, venga de donde venga.
pub fn punto_de_enganche(objetivo: &Elemento, desde: Punto2, b: &Enganche) -> Punto2 {
    let (x0, y0, x1, y1) = objetivo.caja();
    let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);

    // **Modo «dentro»: manda el punto agarrado, no el contorno.** Es lo que
    // hace que la flecha se quede senalando el sitio exacto donde se solto y
    // lo siga al mover o estirar la figura, en vez de saltar al borde mas
    // cercano.
    if b.modo == ModoEnganche::Dentro {
        if let Some((fx, fy)) = b.punto_fijo {
            let sin_girar = Punto2::nuevo(x0 + fx * (x1 - x0), y0 + fy * (y1 - y0));
            return sin_girar.girar(centro, objetivo.angulo);
        }
    }

    let (dx, dy) = (centro.x - desde.x, centro.y - desde.y);
    let largo = (dx * dx + dy * dy).sqrt();
    if largo == 0.0 {
        return centro;
    }
    // El enfoque desplaza el objetivo perpendicular a la linea.
    let semi = ((x1 - x0) / 2.0).max((y1 - y0) / 2.0);
    let desvio = b.foco * semi;
    let hacia = Punto2::nuevo(
        centro.x + (-dy / largo) * desvio,
        centro.y + (dx / largo) * desvio,
    );

    let Some(corte) = cortar_silueta(objetivo, desde, hacia) else {
        return hacia;
    };
    // Retroceder el hueco por la misma direccion de llegada.
    let (bx, by) = (corte.x - desde.x, corte.y - desde.y);
    let blargo = (bx * bx + by * by).sqrt();
    if blargo == 0.0 {
        return corte;
    }
    Punto2::nuevo(
        corte.x - (bx / blargo) * b.hueco,
        corte.y - (by / blargo) * b.hueco,
    )
}

/// El primer corte del segmento `desde -> hasta` con la silueta de `e`.
///
/// Va por [`crate::perimetros`] y no por una formula por tipo —el rectangulo
/// contra sus lados, la elipse contra su ecuacion, como hace el movil—
/// porque aquel ya reduce **cualquier** figura a tramos rectos y ya la
/// entrega girada. Una formula por tipo seria una segunda verdad sobre por
/// donde pasa cada figura, y el dia que una de las dos aprenda un tipo nuevo
/// y la otra no, la flecha se posaria en un borde que no es el que se pinta.
fn cortar_silueta(e: &Elemento, desde: Punto2, hasta: Punto2) -> Option<Punto2> {
    let mut mejor: Option<(f32, Punto2)> = None;
    for (a, b) in silueta(e) {
        if let Some(q) = perimetros::interseccion(desde, hasta, a, b) {
            let d = q.distancia(desde);
            if mejor.is_none_or(|(mejor_d, _)| d < mejor_d) {
                mejor = Some((d, q));
            }
        }
    }
    mejor.map(|(_, q)| q)
}

/// El id interno que le toca a un id de texto del fichero.
///
/// Es **la misma cuenta** que hace `excalidraw::leer` al entrar (FNV-1a de 64
/// bits). Esta repetida aqui porque alli es privada y `excalidraw.rs` no es
/// de este grupo; la prueba `un_enganche_del_movil_encuentra_a_su_figura` la
/// ata a la de alla, asi que si una cambia, la otra falla.
///
/// El prefijo `pc` es la puerta para lo que nace en el escritorio: ver la
/// nota de la cabecera sobre lo que todavia no se puede cerrar.
pub fn id_del_fichero(texto: &str) -> u64 {
    if let Some(hex) = texto.strip_prefix("pc") {
        if let Ok(n) = u64::from_str_radix(hex, 16) {
            return n;
        }
    }
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in texto.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// El id de texto con el que este grupo ata un elemento nacido en el PC.
pub fn id_de_texto(id: u64) -> String {
    format!("pc{id:x}")
}

/// Si `texto` es el id de fichero de `e` (el que devolveria
/// [`id_de_texto_de`]), sin montar ninguna cadena.
///
/// # Por que se compara el TEXTO y no el numero
///
/// Porque el numero de un elemento **no es el del fichero** cuando el dibujo
/// se abre para editarlo: `excalidraw::a_escena` los mete en la escena con
/// `Escena::anadir`, que les da 1, 2, 3... para que `con_escena` sepa luego
/// cual es cual. Comparando `id_del_fichero(texto)` con ese numero, ningun
/// enganche del movil encontraba nunca a su figura —el FNV de `"a1"` no es
/// un 3— y las flechas no seguian a nada. El texto, en cambio, viaja intacto
/// en `extras.id_de_fichero`.
pub fn tiene_id_de_texto(e: &Elemento, texto: &str) -> bool {
    match &e.extras.id_de_fichero {
        Some(suyo) => suyo == texto,
        None => texto
            .strip_prefix("pc")
            .and_then(|hex| u64::from_str_radix(hex, 16).ok())
            .is_some_and(|n| n == e.id),
    }
}

/// El elemento al que apunta un enganche, si sigue existiendo.
pub fn resolver<'a>(elementos: &'a [Elemento], b: &Enganche) -> Option<&'a Elemento> {
    elementos
        .iter()
        .find(|e| !e.borrado && tiene_id_de_texto(e, &b.elemento))
}

/// Recoloca los extremos atados de `flecha` (`updateBoundPoints`).
///
/// Devuelve si movio algo. **Un enganche que no resuelve no es un error**: la
/// figura puede estar borrada, o venir de un movil con un id que aqui no se
/// sabe deshacer. En ese caso la flecha se queda donde esta y su enganche
/// sigue viajando intacto al fichero, que es lo unico que se le puede
/// prometer.
pub fn recolocar(flecha: &mut Elemento, elementos: &[Elemento]) -> bool {
    let inicio = flecha
        .extras
        .enganche_inicio
        .as_ref()
        .and_then(|b| resolver(elementos, b));
    let fin = flecha
        .extras
        .enganche_fin
        .as_ref()
        .and_then(|b| resolver(elementos, b));
    let Some((a, b)) = puntos_atados(flecha, inicio, fin) else {
        return false;
    };
    poner_extremos(flecha, a, b)
}

/// Donde tienen que quedar los dos extremos de `flecha` con estas figuras
/// ya resueltas (`updateBoundPoint`). `None` si no hay nada que mover.
///
/// Separado de [`recolocar`] para que quien ya sabe donde esta cada figura
/// —el arrastre en vivo, con sus indices apuntados al empezar— no tenga que
/// buscarla por id en cada aviso del raton.
fn puntos_atados(
    flecha: &Elemento,
    inicio: Option<&Elemento>,
    fin: Option<&Elemento>,
) -> Option<(Option<Punto2>, Option<Punto2>)> {
    let Figura::Flecha { puntos, .. } = &flecha.figura else {
        return None;
    };
    if puntos.len() < 2 {
        return None;
    }
    let (primero, ultimo) = (puntos[0], puntos[puntos.len() - 1]);
    let nuevo_inicio = match (&flecha.extras.enganche_inicio, inicio) {
        (Some(b), Some(o)) => Some(punto_de_enganche(o, ultimo, b)),
        _ => None,
    };
    let nuevo_fin = match (&flecha.extras.enganche_fin, fin) {
        (Some(b), Some(o)) => Some(punto_de_enganche(o, primero, b)),
        _ => None,
    };
    if nuevo_inicio.is_none() && nuevo_fin.is_none() {
        return None;
    }
    Some((nuevo_inicio, nuevo_fin))
}

/// Pone los extremos calculados y sube la version, para que la cache del
/// pintado no sirva la flecha de antes.
fn poner_extremos(flecha: &mut Elemento, inicio: Option<Punto2>, fin: Option<Punto2>) -> bool {
    let Figura::Flecha { puntos, .. } = &mut flecha.figura else {
        return false;
    };
    let ultimo_indice = puntos.len() - 1;
    if let Some(p) = inicio {
        puntos[0] = p;
    }
    if let Some(p) = fin {
        puntos[ultimo_indice] = p;
    }
    // `x`/`y` pasan a ser el primer punto, como en el movil: los puntos se
    // escriben en el fichero relativos a ellos.
    flecha.x = puntos[0].x;
    flecha.y = puntos[0].y;
    flecha.tocar();
    true
}

/// Las flechas de la escena que cuelgan de alguno de estos elementos.
///
/// Se mira el enganche de la flecha y no la lista `atados` de la figura: son
/// dos verdades sobre lo mismo y la del movil puede venir desactualizada de
/// un editor de terceros, mientras que el enganche es el que de verdad decide
/// donde se posa la punta.
pub fn flechas_colgando_de(elementos: &[Elemento], ids: &[u64]) -> Vec<u64> {
    elementos
        .iter()
        .filter(|e| !e.borrado && matches!(e.figura, Figura::Flecha { .. }))
        .filter(|e| {
            [&e.extras.enganche_inicio, &e.extras.enganche_fin]
                .into_iter()
                .flatten()
                .filter_map(|b| resolver(elementos, b))
                .any(|o| ids.contains(&o.id))
        })
        .map(|e| e.id)
        .collect()
}

/// **Lo que hay que llamar despues de mover, estirar o girar algo.**
///
/// Recoloca las flechas que colgaban de `movidos` dentro del paso de deshacer
/// que este abierto: la flecha que sigue a la caja tiene que deshacerse con
/// ella, no en un segundo Ctrl+Z que descoloca el dibujo a medias.
///
/// Devuelve cuantas flechas se recolocaron.
///
/// Es la version de una sola vez de [`Seguidoras`] —para quien mueve algo
/// fuera de un arrastre, como alinear o las flechas del teclado—: la misma
/// cuenta, sin copiar la escena entera como hacia antes.
pub fn seguir(escena: &mut Escena, movidos: &[u64]) -> usize {
    let mut seguidoras = Seguidoras::default();
    // Ordenados para preguntar con busqueda binaria: `preparar` pregunta por
    // cada elemento de la escena, y alinear mil de dos mil con `contains`
    // serian dos millones de comparaciones.
    let mut orden = movidos.to_vec();
    orden.sort_unstable();
    // Una flecha que se mueve con su caja ya va donde tiene que ir:
    // recolocarla ademas la estiraria hacia un objetivo que tambien se
    // movio, y el arrastre de una seleccion entera se deformaria.
    seguidoras.preparar(&escena.elementos, |id| orden.binary_search(&id).is_ok());
    for &id in seguidoras.ids() {
        escena.apuntar_edicion(id);
    }
    seguidoras.seguir(&mut escena.elementos);
    seguidoras.ids().len()
}

/// **Lo que se llama despues de mover algo fuera de un arrastre** —alinear,
/// repartir, voltear—, dentro del paso de deshacer que lo movio.
///
/// Hace las dos mitades que el arrastre hace en dos momentos:
///
/// 1. las flechas que cuelgan de lo movido lo siguen ([`seguir`]);
/// 2. las flechas movidas **sin** su figura se revisan como al soltarlas
///    ([`revisar_flechas_movidas`]): si la punta sigue encima de la figura,
///    sigue atada; si se la llevo lejos, se suelta. Sin esto, la flecha
///    alineada lejos de su caja volveria sola a ella la proxima vez que la
///    caja se moviera, deshaciendo lo que se acaba de pedir.
///
/// Con nada atado no cuesta mas que un recorrido que no encuentra nada.
pub fn despues_de_mover(escena: &mut Escena, movidos: &[u64]) -> usize {
    let n = seguir(escena, movidos);
    let mut orden = movidos.to_vec();
    orden.sort_unstable();
    revisar_flechas_movidas(escena, |id| orden.binary_search(&id).is_ok(), 1.0);
    n
}

/// Revisa las puntas atadas de las flechas que se han movido **sin** la
/// figura a la que estaban atadas: se atan a lo que tengan debajo, o se
/// sueltan.
///
/// La punta que estaba **dentro** (Alt) sigue dentro si aun cae encima: el
/// modo lo eligio el usuario al atarla, y mover la flecha no es pedir otro.
///
/// Si la figura viajo con la flecha, esa punta no se toca. Y si el enganche
/// no resuelve —un tipo del movil que el PC no dibuja—, tampoco: un enganche
/// que no resuelve viaja intacto.
pub fn revisar_flechas_movidas(escena: &mut Escena, se_mueve: impl Fn(u64) -> bool, zoom: f32) {
    let elementos = &escena.elementos;
    let revisar: Vec<(u64, Extremo, ModoEnganche)> = elementos
        .iter()
        .filter(|e| !e.borrado && se_mueve(e.id) && matches!(e.figura, Figura::Flecha { .. }))
        .flat_map(|e| {
            [
                (&e.extras.enganche_inicio, Extremo::Inicio),
                (&e.extras.enganche_fin, Extremo::Fin),
            ]
            .into_iter()
            .filter_map(|(b, extremo)| {
                let b = b.as_ref()?;
                let quieta = resolver(elementos, b)?;
                (!se_mueve(quieta.id)).then_some((e.id, extremo, b.modo))
            })
            .collect::<Vec<_>>()
        })
        .collect();
    for (id, extremo, modo) in revisar {
        revisar_extremo_en(escena, id, extremo, zoom, modo);
    }
}

/// Vuelve a atar un extremo **a la misma figura y en el mismo modo**,
/// recalculando `foco` y `punto_fijo` desde donde esta ahora la punta.
///
/// Es para cuando la flecha y su figura se han transformado juntas de una
/// forma que cambia de lado el sitio al que apunta —voltear—: la figura no
/// cambia, pero lo guardado describe el lado de antes. No se busca la figura
/// bajo la punta, como en [`revisar_extremo`], porque una punta en orbita
/// esta justo a un hueco del borde, en el limite exacto de lo que atrae, y
/// un redondeo la soltaria.
///
/// Devuelve si ato; `false` si no habia enganche o no resuelve, y entonces
/// no toca nada.
pub fn reatar_a_su_figura(escena: &mut Escena, flecha: u64, extremo: Extremo) -> bool {
    let Some(f) = escena.buscar(flecha) else {
        return false;
    };
    let Some(puntos) = f.puntos().filter(|p| p.len() >= 2) else {
        return false;
    };
    let (punta, otro, b) = match extremo {
        Extremo::Inicio => (
            puntos[0],
            puntos[puntos.len() - 1],
            &f.extras.enganche_inicio,
        ),
        Extremo::Fin => (puntos[puntos.len() - 1], puntos[0], &f.extras.enganche_fin),
    };
    let Some(b) = b.as_ref() else {
        return false;
    };
    let Some(o) = resolver(&escena.elementos, b) else {
        return false;
    };
    let nuevo = Enganche {
        modo: b.modo,
        ..enganche_para(o, b.elemento.clone(), punta, otro)
    };
    escena.apuntar_edicion(flecha);
    let Some(mut copia) = escena.buscar(flecha).cloned() else {
        return false;
    };
    match extremo {
        Extremo::Inicio => copia.extras.enganche_inicio = Some(nuevo),
        Extremo::Fin => copia.extras.enganche_fin = Some(nuevo),
    }
    copia.tocar();
    recolocar(&mut copia, &escena.elementos);
    if let Some(f) = escena.buscar_mut(flecha) {
        *f = copia;
    }
    true
}

/// **Lo que hay que hacer al borrar `id`**, dentro del mismo paso de
/// deshacer que lo borra (`fixBindingsAfterDeletion` de Excalidraw):
///
/// - si es una **figura**, las flechas atadas a ella se sueltan de ese
///   extremo (se queda donde estaba, sin enganche). Si no, la flecha llevaria
///   al fichero un `startBinding` hacia algo que no existe, y el movil —o
///   Excalidraw— la recolocaria contra una figura borrada;
/// - si es una **flecha**, se quita de la lista `boundElements` de sus
///   figuras, que si no dirian al movil que les cuelga una flecha borrada.
///
/// Solo mira la escena entera cuando hay algo que buscar: una figura sin
/// nada en `atados` y una flecha sin enganches no cuestan un recorrido, que
/// es lo que hace que borrar mil trazos siga siendo borrar mil trazos.
///
/// Devuelve cuantos elementos, ademas del borrado, se tocaron.
pub fn soltar_lo_borrado(escena: &mut Escena, id: u64) -> usize {
    let Some(e) = escena.buscar(id) else {
        return 0;
    };
    let es_flecha = matches!(e.figura, Figura::Flecha { .. });
    let con_enganche = e.extras.enganche_inicio.is_some() || e.extras.enganche_fin.is_some();
    let con_flechas = e.extras.atados.iter().any(|a| a.tipo == "arrow");
    if es_flecha && con_enganche {
        // Se quita de TODA figura que la lleve, no solo de sus dos
        // objetivos: una lista del movil puede traerla de antes, y una flecha
        // borrada no le cuelga a nadie. Los enganches de la propia flecha se
        // quedan: si se deshace el borrado, vuelve atada como estaba.
        let texto = id_de_texto_de(e);
        let tocar: Vec<u64> = escena
            .elementos
            .iter()
            .filter(|o| o.id != id && o.extras.atados.iter().any(|a| a.id == texto))
            .map(|o| o.id)
            .collect();
        for o in &tocar {
            escena.apuntar_edicion(*o);
            if let Some(o) = escena.buscar_mut(*o) {
                o.extras.atados.retain(|a| a.id != texto);
                o.tocar();
            }
        }
        return tocar.len();
    }
    if !con_flechas {
        return 0;
    }
    // Se miran los enganches de las flechas y no solo la lista `atados`: la
    // lista es la pista para no recorrer en balde, pero la verdad de a quien
    // se ata una flecha la dice su enganche (ver `flechas_colgando_de`).
    let figura = e.clone();
    let sueltas: Vec<(u64, bool, bool)> = escena
        .elementos
        .iter()
        .filter(|f| !f.borrado && f.id != id && matches!(f.figura, Figura::Flecha { .. }))
        .filter_map(|f| {
            let apunta = |b: &Option<Enganche>| {
                b.as_ref()
                    .is_some_and(|b| tiene_id_de_texto(&figura, &b.elemento))
            };
            let (i, fin) = (
                apunta(&f.extras.enganche_inicio),
                apunta(&f.extras.enganche_fin),
            );
            (i || fin).then_some((f.id, i, fin))
        })
        .collect();
    for &(f, inicio, fin) in &sueltas {
        escena.apuntar_edicion(f);
        if let Some(f) = escena.buscar_mut(f) {
            if inicio {
                desatar(f, Extremo::Inicio);
            }
            if fin {
                desatar(f, Extremo::Fin);
            }
        }
    }
    // La figura borrada se queda con su lista tal cual: si se deshace el
    // borrado vuelve entera, y con ella las flechas, que estan en este paso.
    sueltas.len()
}

/// Suelta un extremo (`unbindBindingElement`).
pub fn desatar(flecha: &mut Elemento, extremo: Extremo) {
    match extremo {
        Extremo::Inicio => flecha.extras.enganche_inicio = None,
        Extremo::Fin => flecha.extras.enganche_fin = None,
    }
    flecha.tocar();
}

/// El id de TEXTO con el que `e` va al fichero: el que traia, o el `pc<hex>`
/// con el que `excalidraw::sellar` escribe lo nacido aqui.
///
/// Es el que tiene que ir en `elementId` y en `boundElements`: el movil los
/// compara con el `id` del JSON, no con nuestro numero.
pub fn id_de_texto_de(e: &Elemento) -> String {
    match &e.extras.id_de_fichero {
        Some(suyo) => suyo.clone(),
        None => id_de_texto(e.id),
    }
}

/// Lo que se hace con un extremo de flecha al soltarlo
/// (`bindOrUnbindBindingElement`): **se ata a la figura que tenga debajo, o
/// se suelta si no hay ninguna.**
///
/// Vale para los dos momentos en que una punta se deja en un sitio: al
/// terminar de dibujar la flecha y al terminar de arrastrar una punta de una
/// que ya existia. Y para cuando se mueve la flecha sola: si su punta sigue
/// encima de la caja, sigue atada; si se la ha llevado lejos, se suelta, que
/// es lo que el usuario acaba de pedir al llevarsela.
///
/// Todo va al paso de deshacer que este abierto, flecha y figura juntas: un
/// Ctrl+Z que devolviera la flecha y dejara a la caja creyendo que la tiene
/// atada dejaria un `boundElements` mintiendo en el fichero.
///
/// Devuelve si el extremo quedo atado.
pub fn revisar_extremo(escena: &mut Escena, flecha: u64, extremo: Extremo, zoom: f32) -> bool {
    revisar_extremo_en(escena, flecha, extremo, zoom, ModoEnganche::Orbita)
}

/// Como [`revisar_extremo`], pero eligiendo **como** se ata la punta.
///
/// `ModoEnganche::Dentro` es el `bindMode: "inside"` de Excalidraw, el que se
/// pide manteniendo **Alt** al soltar: la punta se queda clavada en el sitio
/// exacto de la figura donde se solto (`fixedPoint`) y lo sigue al mover o
/// estirar la figura, en vez de ir al borde. Sirve para senalar un detalle
/// de una imagen o de un mosaico, que es justo lo que el borde no puede.
/// Sin Alt, orbita: ver la nota de dentro.
pub fn revisar_extremo_en(
    escena: &mut Escena,
    flecha: u64,
    extremo: Extremo,
    zoom: f32,
    modo: ModoEnganche,
) -> bool {
    let Some(f) = escena.buscar(flecha) else {
        return false;
    };
    let Some(puntos) = f
        .puntos()
        .filter(|_| matches!(f.figura, Figura::Flecha { .. }))
    else {
        return false;
    };
    if puntos.len() < 2 {
        return false;
    }
    let (punta, otro) = match extremo {
        Extremo::Inicio => (puntos[0], puntos[puntos.len() - 1]),
        Extremo::Fin => (puntos[puntos.len() - 1], puntos[0]),
    };
    // Una flecha de un clic, sin largo, no dice hacia donde va: atarla haria
    // que el rayo de `punto_de_enganche` saliera de ella misma.
    let objetivo = if punta.distancia(otro) < 1.0 {
        None
    } else {
        figura_bajo(&escena.elementos, punta, zoom, flecha)
    };
    let tenia = match extremo {
        Extremo::Inicio => f.extras.enganche_inicio.is_some(),
        Extremo::Fin => f.extras.enganche_fin.is_some(),
    };
    // Ni lo estaba ni lo va a estar: no se toca nada. Tocarla subiria su
    // version y el paso de deshacer apuntaria un cambio que no se ve.
    if objetivo.is_none() && !tenia {
        return false;
    }
    // **En orbita salvo que se pida dentro**, que es lo que hace Excalidraw
    // (`bindingStrategyForNewSimpleArrowEndpointDragging`: `orbit` salvo que
    // se pida `inside` con Alt). La punta va al borde, mirando hacia donde se
    // solto —eso lo guardan `foco` y `punto_fijo`—, y no se queda clavada
    // dentro de la caja: una flecha que acaba a media caja tapa su rotulo, y
    // al estirar la caja se quedaria dentro en vez de seguir al borde. Por
    // eso dentro solo cuando se pide: nunca se adivina por donde cayo.
    let enganche = objetivo.and_then(|id| escena.buscar(id)).map(|o| Enganche {
        modo,
        ..enganche_para(o, id_de_texto_de(o), punta, otro)
    });
    let atado = enganche.is_some();

    escena.apuntar_edicion(flecha);
    if let Some(f) = escena.buscar_mut(flecha) {
        match extremo {
            Extremo::Inicio => f.extras.enganche_inicio = enganche,
            Extremo::Fin => f.extras.enganche_fin = enganche,
        }
        f.tocar();
    }
    sincronizar_atados(escena, flecha);
    // Y la punta se posa ya en el borde: es lo que ve el usuario al soltar
    // en Excalidraw, y lo que evita que el primer movimiento de la caja haga
    // saltar la punta a un sitio que nadie eligio.
    if atado {
        let Some(copia) = escena.buscar(flecha).cloned() else {
            return false;
        };
        let mut copia = copia;
        if recolocar(&mut copia, &escena.elementos) {
            if let Some(f) = escena.buscar_mut(flecha) {
                *f = copia;
            }
        }
    }
    atado
}

/// Pone al dia la otra mitad de la atadura: **la lista `boundElements` de
/// las figuras** (`applyBinding` / `unbindBindingElement`).
///
/// La flecha dice a quien se ata (`startBinding`/`endBinding`), y la figura
/// dice quien le cuelga. Excalidraw y el movil usan la segunda para saber
/// que flechas recolocar al mover una caja, asi que una figura que no la
/// tenga al dia deja a la flecha quieta **en el movil** aunque aqui la siga.
///
/// Recorre la escena una vez: se llama al soltar, no en cada aviso del
/// raton. Quita la flecha de toda figura que ya no sea su objetivo —la de
/// antes de arrastrar la punta, por ejemplo— y la pone en las que lo son,
/// sin repetirla.
pub fn sincronizar_atados(escena: &mut Escena, flecha: u64) {
    let Some(f) = escena.buscar(flecha) else {
        return;
    };
    let texto = id_de_texto_de(f);
    let objetivos: [Option<String>; 2] = [
        f.extras
            .enganche_inicio
            .as_ref()
            .map(|b| b.elemento.clone()),
        f.extras.enganche_fin.as_ref().map(|b| b.elemento.clone()),
    ];
    let es_objetivo = |e: &Elemento| objetivos.iter().flatten().any(|t| tiene_id_de_texto(e, t));
    let tocar: Vec<u64> = escena
        .elementos
        .iter()
        .filter(|e| e.id != flecha)
        .filter(|e| {
            let lo_tiene = e.extras.atados.iter().any(|a| a.id == texto);
            let debe = es_objetivo(e) && !e.borrado;
            lo_tiene != debe
        })
        .map(|e| e.id)
        .collect();
    for id in tocar {
        escena.apuntar_edicion(id);
        let Some(e) = escena.buscar_mut(id) else {
            continue;
        };
        if e.extras.atados.iter().any(|a| a.id == texto) {
            e.extras.atados.retain(|a| a.id != texto);
        } else {
            e.extras.atados.push(crate::elemento::Atado {
                id: texto.clone(),
                tipo: "arrow".into(),
            });
        }
        e.tocar();
    }
}

/// Una flecha que sigue en vivo a lo que se arrastra: donde esta ella y
/// donde estan sus dos figuras en `Escena::elementos`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Seguidora {
    flecha: usize,
    id: u64,
    inicio: Option<usize>,
    fin: Option<usize>,
}

/// **Las flechas que se re-trazan en cada aviso del raton** mientras se
/// mueve, estira o gira lo que tienen atado (`updateBoundElements`).
///
/// # Por que indices apuntados al empezar
///
/// Buscar en cada aviso que flechas cuelgan de lo elegido seria recorrer la
/// escena por cada elegido —O(n·k)— justo en el arrastre, que tiene una
/// puerta de 400 µs con mil elegidos de dos mil. Aqui se recorre **una vez
/// al pulsar** y se apunta, para cada flecha afectada, en que posicion de la
/// escena esta ella y en cual cada una de sus figuras. Durante el arrastre
/// la escena no gana ni pierde elementos, asi que esas posiciones valen
/// hasta soltar; y aun asi se comprueba el id antes de usarlas, que es un
/// comparar y no cuesta nada.
///
/// Con nada atado a lo que se mueve la lista esta vacia y [`Self::seguir`]
/// no hace nada: el arrastre de siempre no paga esto.
#[derive(Debug, Clone, Default)]
pub struct Seguidoras {
    lista: Vec<Seguidora>,
    /// Los ids de las flechas, ordenados, para contestar
    /// [`Self::contiene`] sin recorrer la lista: el pintado lo pregunta por
    /// cada elemento en cada fotograma.
    ids: Vec<u64>,
}

impl Seguidoras {
    /// Apunta las flechas atadas a lo que `se_mueve` sin moverse ellas.
    ///
    /// Una flecha que va dentro de lo que se mueve no se re-traza: ya viaja
    /// entera, y estirarla ademas hacia un objetivo que tambien se movio la
    /// deformaria (`simultaneouslyUpdated` de Excalidraw).
    pub fn preparar(&mut self, elementos: &[Elemento], se_mueve: impl Fn(u64) -> bool) {
        self.limpiar();
        // Lo normal es que no haya ni una flecha atada: se mira antes de
        // montar el indice, que si pide memoria.
        let alguna = elementos.iter().any(|e| {
            !e.borrado
                && (e.extras.enganche_inicio.is_some() || e.extras.enganche_fin.is_some())
                && !se_mueve(e.id)
        });
        if !alguna {
            return;
        }
        // Dos indices y no uno, para no montar una cadena por elemento: lo
        // que vino de un fichero se busca por su texto, lo nacido aqui por
        // su numero (ver `tiene_id_de_texto`, que es la misma regla).
        let mut por_texto: std::collections::HashMap<&str, usize> = Default::default();
        let mut por_numero: std::collections::HashMap<u64, usize> = Default::default();
        for (i, e) in elementos.iter().enumerate().filter(|(_, e)| !e.borrado) {
            match &e.extras.id_de_fichero {
                Some(texto) => {
                    por_texto.insert(texto.as_str(), i);
                }
                None => {
                    por_numero.insert(e.id, i);
                }
            }
        }
        let donde = |b: &Option<Enganche>| {
            let texto = b.as_ref()?.elemento.as_str();
            por_texto.get(texto).copied().or_else(|| {
                let n = u64::from_str_radix(texto.strip_prefix("pc")?, 16).ok()?;
                por_numero.get(&n).copied()
            })
        };
        for (i, e) in elementos.iter().enumerate() {
            if e.borrado || !matches!(e.figura, Figura::Flecha { .. }) || se_mueve(e.id) {
                continue;
            }
            let inicio = donde(&e.extras.enganche_inicio);
            let fin = donde(&e.extras.enganche_fin);
            let le_toca = [inicio, fin]
                .into_iter()
                .flatten()
                .any(|j| se_mueve(elementos[j].id));
            if le_toca {
                self.lista.push(Seguidora {
                    flecha: i,
                    id: e.id,
                    inicio,
                    fin,
                });
                self.ids.push(e.id);
            }
        }
        self.ids.sort_unstable();
    }

    /// Vacia la lista sin soltar su memoria: el siguiente arrastre la
    /// reutiliza.
    pub fn limpiar(&mut self) {
        self.lista.clear();
        self.ids.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.lista.is_empty()
    }

    /// Los ids de las flechas que se re-trazan, ordenados.
    pub fn ids(&self) -> &[u64] {
        &self.ids
    }

    pub fn contiene(&self, id: u64) -> bool {
        self.ids.binary_search(&id).is_ok()
    }

    /// Re-traza cada flecha contra donde estan ahora sus figuras.
    ///
    /// Es lo que se llama en cada aviso del raton despues de mover, estirar o
    /// girar. Cuesta lo que cuesten las flechas afectadas, no la escena.
    pub fn seguir(&self, elementos: &mut [Elemento]) {
        for s in &self.lista {
            let valida = |i: Option<usize>| i.filter(|&i| i < elementos.len());
            let (inicio, fin) = (valida(s.inicio), valida(s.fin));
            if elementos.get(s.flecha).is_none_or(|f| f.id != s.id) {
                continue;
            }
            let nuevos = puntos_atados(
                &elementos[s.flecha],
                inicio.map(|i| &elementos[i]),
                fin.map(|i| &elementos[i]),
            );
            if let Some((a, b)) = nuevos {
                poner_extremos(&mut elementos[s.flecha], a, b);
            }
        }
    }
}

/// Color del resaltado de la figura candidata: el morado de seleccion de
/// Excalidraw, translucido para que no tape el borde de la figura.
const COLOR_RESALTADO: crate::elemento::ColorRgba = crate::elemento::ColorRgba {
    r: 0x69 as f32 / 255.0,
    g: 0x65 as f32 / 255.0,
    b: 0xdb as f32 / 255.0,
    a: 0.45,
};

/// Ancho del resaltado, en pixeles de pantalla.
const GROSOR_RESALTADO_PX: f32 = 6.0;

/// **El aviso de «aqui se va a atar»**: el borde de la figura candidata,
/// ancho y translucido (`renderBindingHighlight` de Excalidraw).
///
/// Sin el, atar es una sorpresa: la punta salta al borde al soltar sin que
/// nada lo anunciara, y soltar cerca de una caja sin querer atarla no tiene
/// vuelta atras visible. Con el, se ve antes de soltar y se corrige.
///
/// El grosor va en pixeles de pantalla —dividido por el zoom—, igual que la
/// pista del iman: una marca de ayuda no crece al acercarse.
pub fn resaltado(e: &Elemento, zoom: f32) -> Vec<crate::pintado::Orden> {
    let grosor = GROSOR_RESALTADO_PX / zoom.max(0.0001);
    let mut contornos: Vec<Vec<Punto2>> = perimetros::contornos_de(e, PASO_PERIMETRO)
        .into_iter()
        .filter(|c| c.puntos.len() >= 2)
        .map(|c| {
            let mut puntos = c.puntos;
            if c.cerrado {
                puntos.push(puntos[0]);
            }
            puntos
        })
        .collect();
    if contornos.is_empty() {
        // El texto y el emoji: su caja, que es contra lo que se posa la punta.
        let tramos = silueta(e);
        let mut puntos: Vec<Punto2> = tramos.iter().map(|(a, _)| *a).collect();
        if let Some(p) = puntos.first().copied() {
            puntos.push(p);
        }
        contornos.push(puntos);
    }
    contornos
        .into_iter()
        .map(|puntos| crate::pintado::Orden::Polilinea {
            puntos,
            color: COLOR_RESALTADO,
            grosor,
            estilo: crate::elemento::EstiloTrazo::Solido,
        })
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::ColorRgba;
    use crate::formas::TipoPunta;

    fn caja(id: u64, figura: Figura, x: f32, y: f32, ancho: f32, alto: f32) -> Elemento {
        Elemento {
            id,
            figura,
            x,
            y,
            ancho,
            alto,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            ..Default::default()
        }
    }

    fn flecha(id: u64, a: Punto2, b: Punto2) -> Elemento {
        let mut e = caja(id, Figura::Rectangulo, a.x, a.y, b.x - a.x, b.y - a.y);
        e.figura = Figura::Flecha {
            puntos: vec![a, b],
            punta_inicio: TipoPunta::Ninguna,
            punta_fin: TipoPunta::Flecha,
            codos: false,
        };
        e
    }

    #[test]
    fn la_punta_se_posa_en_el_borde_y_no_dentro_de_la_caja() {
        let objetivo = caja(1, Figura::Rectangulo, 100.0, 0.0, 100.0, 100.0);
        let b = Enganche {
            elemento: id_de_texto(1),
            foco: 0.0,
            hueco: hueco_de(&objetivo),
            punto_fijo: None,
            modo: ModoEnganche::Orbita,
        };
        let q = punto_de_enganche(&objetivo, Punto2::nuevo(0.0, 50.0), &b);
        // El borde izquierdo esta en x=100; la punta se queda el hueco antes.
        assert!(
            (q.x - (100.0 - b.hueco)).abs() < 0.5,
            "se poso en {q:?}, no rozando el borde"
        );
        assert!((q.y - 50.0).abs() < 0.5);
    }

    #[test]
    fn mover_la_caja_arrastra_la_flecha_consigo() {
        // La razon de ser del modulo, y el fallo que se viene a arreglar: un
        // organigrama del movil abierto aqui se descolocaba en cuanto se
        // movia una caja.
        let objetivo = caja(1, Figura::Rectangulo, 100.0, 0.0, 100.0, 100.0);
        let mut f = flecha(2, Punto2::nuevo(0.0, 50.0), Punto2::nuevo(95.0, 50.0));
        f.extras.enganche_fin = Some(enganche_para(
            &objetivo,
            id_de_texto(1),
            Punto2::nuevo(95.0, 50.0),
            Punto2::nuevo(0.0, 50.0),
        ));

        let mut movida = objetivo.clone();
        movida.mover(200.0, 0.0);
        assert!(recolocar(&mut f, &[movida]));
        let Figura::Flecha { puntos, .. } = &f.figura else {
            unreachable!()
        };
        assert!(
            puntos[1].x > 280.0,
            "la punta tenia que seguir a la caja hasta x=300, esta en {:?}",
            puntos[1]
        );
    }

    #[test]
    fn el_foco_desvia_el_contacto_del_centro_tambien_con_la_caja_mas_grande() {
        // Para esto existe `foco`: sin el, todas las flechas se posan
        // apuntando al centro y un esquema con seis entrando por sitios
        // distintos se convierte en un abanico. El foco se guarda como
        // proporcion del semitamano, asi que **sigue desviando despues de
        // estirar la caja**, que es lo que aqui se comprueba.
        let objetivo = caja(1, Figura::Rectangulo, 100.0, 0.0, 100.0, 100.0);
        let punta = Punto2::nuevo(95.0, 20.0);
        let otro = Punto2::nuevo(0.0, 20.0);
        let b = enganche_para(&objetivo, id_de_texto(1), punta, otro);
        assert!(
            b.foco != 0.0,
            "entra por arriba del centro: el foco no es 0"
        );
        let centrado = Enganche {
            foco: 0.0,
            ..b.clone()
        };

        let mut grande = objetivo.clone();
        grande.ancho = 200.0;
        grande.alto = 200.0;
        let con_foco = punto_de_enganche(&grande, otro, &b);
        let al_centro = punto_de_enganche(&grande, otro, &centrado);
        assert!(
            con_foco.distancia(al_centro) > 1.0,
            "el foco dejo de desviar: los dos se posan en {con_foco:?}"
        );
    }

    #[test]
    fn el_modo_dentro_se_queda_donde_se_solto_y_sigue_a_la_figura() {
        // Soltar la punta bien dentro de una caja significa «senala AQUI», no
        // «senala a esta caja».
        let objetivo = caja(1, Figura::Rectangulo, 0.0, 0.0, 200.0, 200.0);
        let punta = Punto2::nuevo(150.0, 50.0);
        let b = enganche_para(
            &objetivo,
            id_de_texto(1),
            punta,
            Punto2::nuevo(-500.0, 50.0),
        );
        assert_eq!(b.modo, ModoEnganche::Dentro);
        let q = punto_de_enganche(&objetivo, Punto2::nuevo(-500.0, 50.0), &b);
        assert!(q.distancia(punta) < 0.01, "se movio a {q:?}");

        // Y al mover la figura, el punto agarrado va con ella.
        let mut movida = objetivo.clone();
        movida.mover(10.0, 10.0);
        let q = punto_de_enganche(&movida, Punto2::nuevo(-500.0, 50.0), &b);
        assert!(q.distancia(Punto2::nuevo(160.0, 60.0)) < 0.01, "{q:?}");
    }

    #[test]
    fn entre_una_caja_dentro_de_otra_gana_la_pequena() {
        // Regla 3 del original: anclar a la de fuera nunca es lo que se
        // quiere.
        let grande = caja(1, Figura::Rectangulo, 0.0, 0.0, 400.0, 400.0);
        let pequena = caja(2, Figura::Rectangulo, 150.0, 150.0, 60.0, 60.0);
        assert_eq!(
            figura_bajo(&[grande, pequena], Punto2::nuevo(180.0, 180.0), 1.0, 99),
            Some(2)
        );
    }

    #[test]
    fn una_punta_que_cae_lejos_no_ancla_a_nadie() {
        // Caso negativo: lo que se suelta a una distancia razonable se queda
        // donde se solto. Estuvo en quince pixeles —el numero del original— y
        // media pantalla alrededor de cada caja era zona de captura.
        let objetivo = caja(1, Figura::Rectangulo, 100.0, 0.0, 100.0, 100.0);
        assert_eq!(
            figura_bajo(
                std::slice::from_ref(&objetivo),
                Punto2::nuevo(60.0, 50.0),
                1.0,
                99
            ),
            None
        );
        // Rozando el borde si.
        assert_eq!(
            figura_bajo(&[objetivo], Punto2::nuevo(97.0, 50.0), 1.0, 99),
            Some(1)
        );
    }

    #[test]
    fn a_una_raya_o_a_un_marco_no_se_ata_nada() {
        // Caso negativo: atar flecha con flecha hace cadenas que se
        // persiguen, y el marco es el papel, no un dibujo.
        let mut raya = caja(1, Figura::Rectangulo, 0.0, 0.0, 100.0, 0.0);
        raya.figura = Figura::Linea {
            puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0)],
        };
        let marco = caja(
            2,
            Figura::Marco {
                nombre: "hoja".into(),
            },
            0.0,
            0.0,
            500.0,
            500.0,
        );
        assert!(!se_puede_atar(&raya));
        assert!(!se_puede_atar(&marco));
        assert_eq!(
            figura_bajo(&[raya, marco], Punto2::nuevo(50.0, 0.0), 1.0, 99),
            None
        );
    }

    #[test]
    fn lo_bloqueado_no_recibe_flechas() {
        let mut e = caja(1, Figura::Rectangulo, 0.0, 0.0, 100.0, 100.0);
        e.bloqueado = true;
        assert!(!se_puede_atar(&e));
    }

    #[test]
    fn un_enganche_que_no_resuelve_deja_la_flecha_quieta() {
        // Caso negativo y de los importantes: una figura borrada, o un id de
        // un movil que aqui no se sabe deshacer, no puede mover la punta a
        // ninguna parte ni hacer saltar nada.
        let mut f = flecha(2, Punto2::nuevo(0.0, 0.0), Punto2::nuevo(50.0, 0.0));
        f.extras.enganche_fin = Some(Enganche {
            elemento: "no-existe".into(),
            foco: 0.0,
            hueco: 5.0,
            punto_fijo: None,
            modo: ModoEnganche::Orbita,
        });
        let antes = f.clone();
        assert!(!recolocar(&mut f, &[]));
        assert_eq!(f, antes, "ni los puntos ni la version");
    }

    #[test]
    fn una_flecha_sin_enganches_no_se_toca() {
        let mut f = flecha(2, Punto2::nuevo(0.0, 0.0), Punto2::nuevo(50.0, 0.0));
        let antes = f.clone();
        assert!(!recolocar(&mut f, &[]));
        assert_eq!(f, antes);
    }

    #[test]
    fn un_enganche_del_movil_encuentra_a_su_figura() {
        // **La prueba que ata este modulo a `excalidraw.rs`.** El id interno
        // de un elemento leido del fichero lo calcula alli una funcion
        // privada; aqui se repite la cuenta para poder deshacer un
        // `startBinding`. Si una de las dos cambia, esto falla.
        let json = r##"{"elements":[
            {"id":"a1","type":"rectangle","x":0,"y":0,"width":10,"height":10,
             "strokeColor":"#000000","seed":1}
        ]}"##;
        let lienzo = crate::excalidraw::leer(json).expect("json valido");
        let id_interno = lienzo
            .entradas
            .iter()
            .find_map(|e| match e {
                crate::excalidraw::Entrada::Nuestro { elemento, .. } => Some(elemento.id),
                crate::excalidraw::Entrada::Ajeno(_) => None,
            })
            .expect("el rectangulo entra por el carril nuestro");
        assert_eq!(
            id_del_fichero("a1"),
            id_interno,
            "la cuenta de los ids se ha separado de la de excalidraw.rs"
        );
    }

    #[test]
    fn el_id_de_texto_del_pc_va_y_vuelve() {
        assert_eq!(id_del_fichero(&id_de_texto(48_879)), 48_879);
    }

    #[test]
    fn seguir_recoloca_dentro_del_mismo_paso_de_deshacer() {
        // La flecha que sigue a la caja tiene que deshacerse CON ella: si
        // abriera su propio paso, un solo Ctrl+Z dejaria la caja en su sitio
        // viejo y la flecha en el nuevo.
        let mut escena = Escena::nueva();
        let caja_id = escena.anadir(caja(0, Figura::Rectangulo, 100.0, 0.0, 100.0, 100.0));
        let objetivo = escena.buscar(caja_id).unwrap().clone();
        let mut f = flecha(0, Punto2::nuevo(0.0, 50.0), Punto2::nuevo(95.0, 50.0));
        f.extras.enganche_fin = Some(enganche_para(
            &objetivo,
            id_de_texto(caja_id),
            Punto2::nuevo(95.0, 50.0),
            Punto2::nuevo(0.0, 50.0),
        ));
        let flecha_id = escena.anadir(f);

        escena.abrir_paso();
        escena.apuntar_movimiento(caja_id, 200.0, 0.0);
        escena.buscar_mut(caja_id).unwrap().mover(200.0, 0.0);
        assert_eq!(seguir(&mut escena, &[caja_id]), 1);
        escena.cerrar_paso();

        let punta_movida = match &escena.buscar(flecha_id).unwrap().figura {
            Figura::Flecha { puntos, .. } => puntos[1],
            _ => unreachable!(),
        };
        assert!(punta_movida.x > 280.0);

        assert!(escena.deshacer());
        let punta_vuelta = match &escena.buscar(flecha_id).unwrap().figura {
            Figura::Flecha { puntos, .. } => puntos[1],
            _ => unreachable!(),
        };
        assert!(
            (punta_vuelta.x - 95.0).abs() < 0.01,
            "un solo Ctrl+Z tiene que devolver las dos cosas: {punta_vuelta:?}"
        );
    }

    #[test]
    fn una_flecha_que_se_mueve_con_su_caja_no_se_estira_ademas() {
        // Arrastrar la caja Y la flecha seleccionadas a la vez: si ademas se
        // recolocara la punta, el conjunto se deformaria.
        let mut escena = Escena::nueva();
        let caja_id = escena.anadir(caja(0, Figura::Rectangulo, 100.0, 0.0, 100.0, 100.0));
        let objetivo = escena.buscar(caja_id).unwrap().clone();
        let mut f = flecha(0, Punto2::nuevo(0.0, 50.0), Punto2::nuevo(95.0, 50.0));
        f.extras.enganche_fin = Some(enganche_para(
            &objetivo,
            id_de_texto(caja_id),
            Punto2::nuevo(95.0, 50.0),
            Punto2::nuevo(0.0, 50.0),
        ));
        let flecha_id = escena.anadir(f);
        escena.abrir_paso();
        assert_eq!(seguir(&mut escena, &[caja_id, flecha_id]), 0);
        escena.cerrar_paso();
    }
}
