//! **Soldar vertices: el alfiler, un clavo que atraviesa dos figuras** (puerto
//! de `motor/Nudos.kt` del movil, `Tool.NUDO`).
//!
//! La analogia es literal y de ella sale todo el comportamiento. Dos listones
//! de madera clavados por un punto:
//!
//! - **no se pueden separar**: muevas el que muevas, va el otro detras;
//! - **si pueden girar** uno respecto del otro, alrededor del clavo;
//! - **con dos clavos ya no gira nada**: dos puntos en comun fijan la
//!   posicion relativa entera.
//!
//! Se clava **en un punto** y lo que se clava es cualquier figura: por su
//! vertice si lo tiene ahi, y si no, por el sitio de su recorrido o de su
//! caja por donde le entra el clavo.
//!
//! # Lo que cambia respecto al movil
//!
//! Alli los puntos de una raya son relativos y el giro va aparte; aqui los
//! puntos son ABSOLUTOS y el giro se aplica alrededor del centro de la caja.
//! Mover un vertice de una raya girada movia su caja, y con ella el centro del
//! giro y todos los demas vertices. Por eso, antes de tocar un vertice o de
//! girar una raya sobre un clavo, la raya se **endereza**: su giro se pasa a
//! los puntos y el angulo queda a cero ([`enderezar`]). Se ve exactamente
//! igual y a partir de ahi mover un punto mueve ese punto y ninguno mas.
//!
//! Los clavos no van en el elemento sino en la escena (`Escena::alfileres`) y
//! viajan al movil en la clave `alfileres` del fichero (`Scene.alfileres`).

use serde::{Deserialize, Serialize};

use crate::ColorRgba;
use crate::elemento::{Elemento, Figura};
use crate::escena::Escena;
use crate::perimetros::{self, PASO_PERIMETRO};
use crate::pintado::Orden;
use crate::vector::{Punto2, distancia_a_segmento};

/// Cuanto se acerca el cursor a un cruce para clavar alli, o a un clavo para
/// cogerlo, en pixeles de pantalla. El movil usa 28 dp para el dedo
/// (`UMBRAL_GUIA`); con raton basta el mismo radio que agarra el punto
/// etiquetado.
pub const RADIO_DEL_CLAVO: f32 = 16.0;

/// El rojo de la cabeza del clavo (`NUDO_COLOR` del movil, `#e03131`).
pub const COLOR_DEL_CLAVO: ColorRgba = ColorRgba {
    r: 0xe0 as f32 / 255.0,
    g: 0x31 as f32 / 255.0,
    b: 0x31 as f32 / 255.0,
    a: 1.0,
};

/// El radio de la cabeza, en pixeles de pantalla (2,5 dp en el movil; con
/// raton un pelo mas, que la pantalla se mira de mas lejos).
pub const RADIO_DE_LA_CABEZA: f32 = 3.5;

/// **Por donde agarra el alfiler a una figura.**
///
/// Tres formas, de la mejor a la peor (ver [`agarrar_en`]):
///
/// - por un **vertice** (`indice`): el clavo se mueve con ese punto y la raya
///   puede estirarse desde el otro;
/// - por la **fraccion de su recorrido** (`t`, de 0 a 1): «a un tercio de la
///   raya», que sigue significando lo mismo la estires, la gires o le muevas
///   una punta. Guardarlo como proporcion de la caja fallaba en las rayas
///   horizontales (sin alto no hay proporcion) y como distancia al origen, en
///   cuanto se movia la primera punta;
/// - por **proporcion de la caja** (`local`, de 0 a 1 en cada eje) en las
///   figuras con caja de verdad, que es lo unico que aguanta que ademas se
///   las estire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Agarre {
    /// El id de la escena de la figura atravesada.
    pub elemento: u64,
    #[serde(default)]
    pub indice: Option<usize>,
    #[serde(default)]
    pub local: Option<Punto2>,
    #[serde(default)]
    pub t: Option<f32>,
}

impl Agarre {
    pub fn por_vertice(elemento: u64, indice: usize) -> Agarre {
        Agarre {
            elemento,
            indice: Some(indice),
            local: None,
            t: None,
        }
    }
}

/// Un clavo, con todo lo que atraviesa.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Alfiler {
    /// Donde esta clavado, en coordenadas del documento.
    pub punto: Punto2,
    pub agarres: Vec<Agarre>,
}

impl Alfiler {
    /// Un clavo que sujeta una sola cosa no sujeta nada.
    pub fn valido(&self) -> bool {
        let mut ids: Vec<u64> = self.agarres.iter().map(|a| a.elemento).collect();
        ids.sort_unstable();
        ids.dedup();
        ids.len() >= 2
    }

    fn atraviesa(&self, id: u64) -> bool {
        self.agarres.iter().any(|a| a.elemento == id)
    }
}

/// **Cuanto se puede mover una figura, segun cuantos clavos la atraviesan.**
///
/// Es la ley entera del alfiler:
///
/// | Clavos  | Que puede hacer                                     |
/// |---------|-----------------------------------------------------|
/// | 0       | Todo: se traslada, gira sobre su centro, se estira. |
/// | 1       | **Solo girar**, y alrededor del clavo.              |
/// | 2 o mas | Nada: dos puntos fijos fijan la figura entera.      |
///
/// La traslacion desaparece con el primer clavo: un liston clavado por un
/// punto no se puede llevar a otro sitio. Para mover lo clavado se mueve **el
/// clavo** ([`mover_alfiler`]), que es lo que se hace en la realidad.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Libertad {
    Libre,
    Gira,
    Fija,
}

/// Los clavos que atraviesan `id`.
pub fn alfileres_de(alfileres: &[Alfiler], id: u64) -> impl Iterator<Item = &Alfiler> {
    alfileres.iter().filter(move |a| a.atraviesa(id))
}

pub fn libertad_de(alfileres: &[Alfiler], id: u64) -> Libertad {
    match alfileres_de(alfileres, id).count() {
        0 => Libertad::Libre,
        1 => Libertad::Gira,
        _ => Libertad::Fija,
    }
}

// -------------------------------------------------------------------------
// Geometria de una figura en el mundo
// -------------------------------------------------------------------------

/// Si la figura se dibuja con puntos propios (raya, flecha, lapiz, cota).
fn es_de_puntos(e: &Elemento) -> bool {
    matches!(
        e.figura,
        Figura::Linea { .. }
            | Figura::Flecha { .. }
            | Figura::Lapiz { .. }
            | Figura::Resaltador { .. }
            | Figura::Cota { .. }
    )
}

fn puntos_mut(e: &mut Elemento) -> Option<&mut Vec<Punto2>> {
    match &mut e.figura {
        Figura::Linea { puntos }
        | Figura::Flecha { puntos, .. }
        | Figura::Lapiz { puntos, .. }
        | Figura::Resaltador { puntos }
        | Figura::Cota { puntos } => Some(puntos),
        _ => None,
    }
}

/// El centro alrededor del que se pinta girada la figura: el de su caja,
/// como `pintado` y `transformar::girar`.
fn centro_de(e: &Elemento) -> Punto2 {
    let (x0, y0, x1, y1) = e.caja();
    Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0)
}

/// Los vertices de una figura de puntos donde se ven, con el giro puesto.
pub fn vertices_en_el_mundo(e: &Elemento) -> Vec<Punto2> {
    let Some(puntos) = e.puntos() else {
        return Vec::new();
    };
    if e.angulo == 0.0 {
        return puntos.to_vec();
    }
    let c = centro_de(e);
    puntos.iter().map(|p| p.girar(c, e.angulo)).collect()
}

/// Rehace `x`, `y`, `ancho` y `alto` de una figura de puntos a partir de sus
/// puntos, como hace el recorte con lo que corta.
fn recolocar_caja(e: &mut Elemento) {
    let Some(puntos) = e.puntos() else { return };
    if puntos.is_empty() {
        return;
    }
    let x0 = puntos.iter().map(|p| p.x).fold(f32::MAX, f32::min);
    let y0 = puntos.iter().map(|p| p.y).fold(f32::MAX, f32::min);
    let x1 = puntos.iter().map(|p| p.x).fold(f32::MIN, f32::max);
    let y1 = puntos.iter().map(|p| p.y).fold(f32::MIN, f32::max);
    e.x = x0;
    e.y = y0;
    e.ancho = x1 - x0;
    e.alto = y1 - y0;
}

/// **Pasa el giro de una figura de puntos a sus puntos** y deja el angulo a
/// cero. Se ve igual; a partir de ahi mover un vertice no mueve los demas.
/// A las figuras con caja no les hace nada: su giro es parte de ellas.
pub fn enderezar(e: &mut Elemento) {
    if e.angulo == 0.0 || !es_de_puntos(e) {
        return;
    }
    let mundo = vertices_en_el_mundo(e);
    if let Some(p) = puntos_mut(e) {
        *p = mundo;
    }
    e.angulo = 0.0;
    recolocar_caja(e);
    e.tocar();
}

/// Lleva el vertice `i` de `e` exactamente a `destino`, en el mundo.
pub fn con_punto_en_el_mundo(e: &mut Elemento, i: usize, destino: Punto2) {
    enderezar(e);
    let Some(puntos) = puntos_mut(e) else { return };
    let Some(q) = puntos.get_mut(i) else { return };
    *q = destino;
    recolocar_caja(e);
    e.tocar();
}

/// El recorrido de la figura en el mundo: su primer contorno.
fn recorrido(e: &Elemento) -> Vec<Punto2> {
    perimetros::contornos_de(e, PASO_PERIMETRO)
        .into_iter()
        .next()
        .map(|c| c.puntos)
        .unwrap_or_default()
}

fn largo_de(puntos: &[Punto2]) -> f32 {
    puntos.windows(2).map(|p| p[0].distancia(p[1])).sum()
}

/// El punto que esta a la fraccion `t` del recorrido de `e`, ya girado.
fn punto_en_el_recorrido(e: &Elemento, t: f32) -> Option<Punto2> {
    let pts = recorrido(e);
    if pts.len() < 2 {
        return pts.first().copied();
    }
    let total = largo_de(&pts);
    if total <= 0.0 {
        return pts.first().copied();
    }
    let mut falta = t.clamp(0.0, 1.0) * total;
    for i in 0..pts.len() - 1 {
        let tramo = pts[i].distancia(pts[i + 1]);
        if falta <= tramo || i == pts.len() - 2 {
            let f = if tramo <= 0.0 {
                0.0
            } else {
                (falta / tramo).clamp(0.0, 1.0)
            };
            return Some(pts[i].hacia(pts[i + 1], f));
        }
        falta -= tramo;
    }
    pts.last().copied()
}

/// A que fraccion del recorrido de `e` cae `p`.
fn fraccion_del_recorrido(e: &Elemento, p: Punto2) -> f32 {
    let pts = recorrido(e);
    if pts.len() < 2 {
        return 0.0;
    }
    let total = largo_de(&pts);
    if total <= 0.0 {
        return 0.0;
    }
    let (mut hecho, mut mejor, mut respuesta) = (0.0_f32, f32::MAX, 0.0_f32);
    for par in pts.windows(2) {
        let (a, b) = (par[0], par[1]);
        let largo = a.distancia(b);
        let d = distancia_a_segmento(p, a, b);
        if d < mejor {
            mejor = d;
            let f = if largo <= 0.0 {
                0.0
            } else {
                (p.restar(a).producto(b.restar(a)) / (largo * largo)).clamp(0.0, 1.0)
            };
            respuesta = (hecho + f * largo) / total;
        }
        hecho += largo;
    }
    respuesta
}

/// **Donde cae ahora mismo el agarre `a` sobre `e`**, en el mundo.
pub fn punto_del_agarre(e: &Elemento, a: &Agarre) -> Option<Punto2> {
    if let Some(i) = a.indice {
        return vertices_en_el_mundo(e).get(i).copied();
    }
    if let Some(t) = a.t {
        return punto_en_el_recorrido(e, t);
    }
    let l = a.local?;
    let q = Punto2::nuevo(e.x + l.x * e.ancho, e.y + l.y * e.alto);
    Some(if e.angulo == 0.0 {
        q
    } else {
        q.girar(centro_de(e), e.angulo)
    })
}

/// Como agarra el clavo a `e` si entra por `donde`. Ver [`Agarre`].
fn agarrar_en(e: &Elemento, donde: Punto2, radio: f32) -> Agarre {
    let mut agarre = Agarre {
        elemento: e.id,
        indice: None,
        local: None,
        t: None,
    };
    if es_de_puntos(e) {
        // El vertice MAS cercano y no el primero que caiga en el radio: en
        // un trazo a mano hay muchos dentro y el primero no es el del cruce.
        let mejor = vertices_en_el_mundo(e)
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i, v.distancia(donde)))
            .filter(|(_, d)| *d <= radio)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        match mejor {
            Some((i, _)) => agarre.indice = Some(i),
            None => agarre.t = Some(fraccion_del_recorrido(e, donde)),
        }
        return agarre;
    }
    // El arco se guarda como tramo del ovalo, sin caja propia que valga;
    // una caja sin superficie no admite proporciones. Los dos, por recorrido.
    if matches!(e.figura, Figura::Arco { .. }) || e.ancho == 0.0 || e.alto == 0.0 {
        agarre.t = Some(fraccion_del_recorrido(e, donde));
        return agarre;
    }
    let sin_girar = if e.angulo == 0.0 {
        donde
    } else {
        donde.girar(centro_de(e), -e.angulo)
    };
    agarre.local = Some(Punto2::nuevo(
        (sin_girar.x - e.x) / e.ancho,
        (sin_girar.y - e.y) / e.alto,
    ));
    agarre
}

/// Si se puede clavar en esta figura. El marco es andamio; la mancha del bote
/// no se deja girar (sus puntos no siguen al giro de su caja); y lo que no
/// tiene borde visible (texto, punto, foco) no da contorno por el que entrar.
fn se_puede_clavar(e: &Elemento) -> bool {
    !e.borrado && !e.bloqueado && !matches!(e.figura, Figura::Marco { .. } | Figura::Region { .. })
}

/// **Clava un alfiler en `p`, o lo quita si ya habia uno ahi.**
///
/// Se clava donde de verdad se cruzan las figuras: si hay un cruce o un
/// vertice cerca, el clavo va exactamente ahi. Clavarlo «casi» en el cruce
/// uniria las figuras por un punto que no esta en ninguna de las dos.
///
/// `None` si no hay al menos **dos figuras distintas** que atravesar.
pub fn clavar_en(
    elementos: &[Elemento],
    alfileres: &[Alfiler],
    p: Punto2,
    radio: f32,
) -> Option<Vec<Alfiler>> {
    // Un toque sobre un clavo lo saca: es la misma pregunta al reves.
    if let Some(i) = alfileres.iter().position(|a| a.punto.distancia(p) <= radio) {
        let mut sin = alfileres.to_vec();
        sin.remove(i);
        return Some(sin);
    }
    let candidatas: Vec<Elemento> = elementos
        .iter()
        .filter(|e| se_puede_clavar(e) && perimetros::punto_en_el_perimetro(e, p, radio).is_some())
        .cloned()
        .collect();
    let mut ids: Vec<u64> = candidatas.iter().map(|e| e.id).collect();
    ids.dedup();
    if ids.len() < 2 {
        return None;
    }
    // Afinar: primero un cruce entre las candidatas, luego un vertice, y si
    // no hay nada, el punto tal cual.
    let cruce = perimetros::intersecciones_cerca(&candidatas, p, radio, None, false)
        .into_iter()
        .map(|(q, _)| q)
        .min_by(|a, b| a.distancia(p).total_cmp(&b.distancia(p)));
    let vertice = candidatas
        .iter()
        .flat_map(vertices_en_el_mundo)
        .filter(|v| v.distancia(p) <= radio)
        .min_by(|a, b| a.distancia(p).total_cmp(&b.distancia(p)));
    let donde = cruce.or(vertice).unwrap_or(p);

    let agarres = candidatas
        .iter()
        .map(|e| agarrar_en(e, donde, radio))
        .collect();
    let mut con = alfileres.to_vec();
    con.push(Alfiler {
        punto: donde,
        agarres,
    });
    Some(con)
}

// -------------------------------------------------------------------------
// Mover lo clavado
// -------------------------------------------------------------------------

/// Cuanto hay que girar alrededor de `eje` para ir de `desde` a `hasta`.
fn angulo_entre(eje: Punto2, desde: Punto2, hasta: Punto2) -> f32 {
    let a = (desde.y - eje.y).atan2(desde.x - eje.x);
    let b = (hasta.y - eje.y).atan2(hasta.x - eje.x);
    if !a.is_finite() || !b.is_finite() {
        return 0.0;
    }
    let mut d = b - a;
    while d > std::f32::consts::PI {
        d -= std::f32::consts::TAU;
    }
    while d < -std::f32::consts::PI {
        d += std::f32::consts::TAU;
    }
    d
}

/// **Gira `e` alrededor de `centro`** sumando `delta`. Una figura de puntos
/// gira sus puntos (enderezada antes); una con caja mueve su centro y suma el
/// angulo, que es como la pinta el motor.
pub fn girar_elemento(e: &mut Elemento, centro: Punto2, delta: f32) {
    if delta == 0.0 || e.bloqueado {
        return;
    }
    if es_de_puntos(e) {
        enderezar(e);
        if let Some(p) = puntos_mut(e) {
            for q in p.iter_mut() {
                *q = q.girar(centro, delta);
            }
        }
        recolocar_caja(e);
        e.tocar();
    } else {
        crate::transformar::girar(e, centro, delta);
    }
}

/// Recoloca `e` para que su agarre vuelva a caer en `eje`: girar mueve el
/// centro de la caja, y el punto del clavo tiene que acabar exactamente donde
/// estaba o el eje se iria desplazando a cada aviso del raton.
fn devolver_al_eje(e: &mut Elemento, agarre: &Agarre, eje: Punto2) {
    if let Some(despues) = punto_del_agarre(e, agarre) {
        e.mover(eje.x - despues.x, eje.y - despues.y);
    }
}

/// El unico agarre de `e` en su unico clavo, si tiene uno solo.
fn agarre_unico(alfileres: &[Alfiler], id: u64) -> Option<Agarre> {
    let mut suyos = alfileres_de(alfileres, id);
    let a = suyos.next()?;
    if suyos.next().is_some() {
        return None;
    }
    a.agarres.iter().find(|g| g.elemento == id).cloned()
}

/// **Arrastra lo elegido respetando lo que cada figura puede hacer**: sin
/// clavos sigue al cursor; con uno gira a su alrededor (el cursor marca hacia
/// donde apunta, como la aguja de un reloj); con dos o mas no se mueve.
pub fn arrastrar_con_alfileres(
    elementos: &mut [Elemento],
    alfileres: &[Alfiler],
    elegido: impl Fn(u64) -> bool,
    desde: Punto2,
    hasta: Punto2,
) {
    let (dx, dy) = (hasta.x - desde.x, hasta.y - desde.y);
    for e in elementos
        .iter_mut()
        .filter(|e| elegido(e.id) && !e.bloqueado)
    {
        match libertad_de(alfileres, e.id) {
            Libertad::Libre => e.mover(dx, dy),
            Libertad::Fija => {}
            Libertad::Gira => {
                let Some(agarre) = agarre_unico(alfileres, e.id) else {
                    continue;
                };
                let Some(eje) = punto_del_agarre(e, &agarre) else {
                    continue;
                };
                girar_elemento(e, eje, angulo_entre(eje, desde, hasta));
                devolver_al_eje(e, &agarre, eje);
            }
        }
    }
}

/// **Gira lo elegido con el tirador de giro**, respetando los clavos: sin
/// ellos, alrededor de `centro` como siempre; con uno, alrededor del clavo (la
/// articulacion); con dos, nada.
pub fn girar_con_alfileres(
    elementos: &mut [Elemento],
    alfileres: &[Alfiler],
    elegido: impl Fn(u64) -> bool,
    centro: Punto2,
    delta: f32,
) {
    for e in elementos
        .iter_mut()
        .filter(|e| elegido(e.id) && !e.bloqueado)
    {
        match libertad_de(alfileres, e.id) {
            Libertad::Libre => crate::transformar::girar(e, centro, delta),
            Libertad::Fija => {}
            Libertad::Gira => {
                let Some(agarre) = agarre_unico(alfileres, e.id) else {
                    continue;
                };
                let Some(eje) = punto_del_agarre(e, &agarre) else {
                    continue;
                };
                girar_elemento(e, eje, delta);
                devolver_al_eje(e, &agarre, eje);
            }
        }
    }
}

/// **Mueve el clavo a `destino` y se lleva todo lo que atraviesa.**
///
/// Un agarre por vertice mueve **ese punto** (un triangulo de tres rayas se
/// deforma, no gira); si la figura tiene otro clavo, gira sobre el (la
/// manivela); y si no, se lleva la figura entera.
pub fn mover_alfiler(
    elementos: &mut [Elemento],
    alfileres: &[Alfiler],
    indice: usize,
    destino: Punto2,
) {
    let Some(alfiler) = alfileres.get(indice) else {
        return;
    };
    for e in elementos.iter_mut() {
        let agarres: Vec<&Agarre> = alfiler
            .agarres
            .iter()
            .filter(|a| a.elemento == e.id)
            .collect();
        if agarres.is_empty() || e.bloqueado {
            continue;
        }
        let por_vertice: Vec<usize> = agarres.iter().filter_map(|a| a.indice).collect();
        if !por_vertice.is_empty() {
            for i in por_vertice {
                con_punto_en_el_mundo(e, i, destino);
            }
            continue;
        }
        let otro = alfileres
            .iter()
            .enumerate()
            .find(|(i, a)| *i != indice && a.atraviesa(e.id))
            .and_then(|(_, a)| a.agarres.iter().find(|g| g.elemento == e.id).cloned());
        if let Some(eje_agarre) = otro {
            let (Some(eje), Some(mano)) = (
                punto_del_agarre(e, &eje_agarre),
                punto_del_agarre(e, agarres[0]),
            ) else {
                continue;
            };
            girar_elemento(e, eje, angulo_entre(eje, mano, destino));
            devolver_al_eje(e, &eje_agarre, eje);
            continue;
        }
        if let Some(actual) = punto_del_agarre(e, agarres[0]) {
            e.mover(destino.x - actual.x, destino.y - actual.y);
        }
    }
}

/// **Devuelve las figuras a su sitio para que el clavo no se mueva.**
///
/// Tras estirar una figura clavada, su punto clavado se habra ido: se la
/// recoloca hasta que vuelve al clavo, que es lo que hace una madera clavada
/// cuando le tiras de un extremo. Un vertice clavado vuelve el solo; una
/// figura agarrada por caja o por recorrido se traslada entera.
pub fn fijar_alfileres(
    elementos: &mut [Elemento],
    alfileres: &[Alfiler],
    elegido: impl Fn(u64) -> bool,
) {
    if alfileres.is_empty() {
        return;
    }
    for e in elementos
        .iter_mut()
        .filter(|e| elegido(e.id) && !e.bloqueado)
    {
        let id = e.id;
        for a in alfileres {
            for agarre in a.agarres.iter().filter(|g| g.elemento == id) {
                let Some(actual) = punto_del_agarre(e, agarre) else {
                    continue;
                };
                if actual.distancia(a.punto) < 1e-4 {
                    continue;
                }
                match agarre.indice {
                    Some(i) => con_punto_en_el_mundo(e, i, a.punto),
                    None => e.mover(a.punto.x - actual.x, a.punto.y - actual.y),
                }
            }
        }
    }
}

/// **Recoloca cada clavo donde este ahora su primer agarre** y tira los que
/// ya no atan dos figuras vivas.
///
/// Se recalcula en vez de moverse a mano por lo mismo que la cota no guarda su
/// numero: un dato deducible guardado aparte acaba discrepando, y aqui
/// discrepar es una marca roja donde ya no hay ninguna union.
pub fn refrescar_alfileres(elementos: &[Elemento], alfileres: &[Alfiler]) -> Vec<Alfiler> {
    let vivo = |id: u64| elementos.iter().find(|e| e.id == id && !e.borrado);
    alfileres
        .iter()
        .filter_map(|a| {
            let vivos: Vec<Agarre> = a
                .agarres
                .iter()
                .filter(|g| vivo(g.elemento).is_some())
                .cloned()
                .collect();
            let nuevo = Alfiler {
                punto: vivos
                    .iter()
                    .find_map(|g| punto_del_agarre(vivo(g.elemento)?, g))
                    .unwrap_or(a.punto),
                agarres: vivos,
            };
            nuevo.valido().then_some(nuevo)
        })
        .collect()
}

/// Donde hay que pintar las cabezas de los clavos.
pub fn puntos_de_alfileres(elementos: &[Elemento], alfileres: &[Alfiler]) -> Vec<Punto2> {
    refrescar_alfileres(elementos, alfileres)
        .into_iter()
        .map(|a| a.punto)
        .collect()
}

// -------------------------------------------------------------------------
// Sobre la escena: lo que llaman la herramienta y el gesto
// -------------------------------------------------------------------------

/// **La herramienta de soldar**: clava (o desclava) en `p`. Un paso de
/// deshacer. `false` si alli no habia dos figuras que atravesar.
pub fn soldar(escena: &mut Escena, p: Punto2, radio: f32) -> bool {
    let Some(nuevos) = clavar_en(&escena.elementos, &escena.alfileres, p, radio) else {
        return false;
    };
    escena.abrir_paso();
    escena.apuntar_alfileres();
    escena.alfileres = refrescar_alfileres(&escena.elementos, &nuevos);
    escena.cerrar_paso();
    true
}

/// El clavo que hay bajo `p`, si hay alguno.
pub fn clavo_en(escena: &Escena, p: Punto2, radio: f32) -> Option<usize> {
    escena
        .alfileres
        .iter()
        .enumerate()
        .filter(|(_, a)| a.punto.distancia(p) <= radio)
        .min_by(|a, b| a.1.punto.distancia(p).total_cmp(&b.1.punto.distancia(p)))
        .map(|(i, _)| i)
}

/// Apunta para deshacer lo que un gesto con clavos puede cambiar: los clavos
/// y las figuras que atraviesan (girar lo elegido mueve su clavo, y llevarse
/// un clavo mueve figuras que no estaban elegidas).
fn apuntar(escena: &mut Escena, ids: &[u64]) {
    escena.apuntar_alfileres();
    for &id in ids {
        escena.apuntar_edicion(id);
    }
}

/// Todas las figuras que atraviesa algun clavo.
fn clavadas(escena: &Escena) -> Vec<u64> {
    let mut ids: Vec<u64> = escena
        .alfileres
        .iter()
        .flat_map(|a| a.agarres.iter().map(|g| g.elemento))
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// Arrastrar el clavo `indice` hasta `destino`, con todo lo que lleva.
pub fn mover_clavo(escena: &mut Escena, indice: usize, destino: Punto2) {
    let ids = clavadas(escena);
    apuntar(escena, &ids);
    let alfileres = escena.alfileres.clone();
    mover_alfiler(&mut escena.elementos, &alfileres, indice, destino);
    if let Some(a) = escena.alfileres.get_mut(indice) {
        a.punto = destino;
    }
    escena.alfileres = refrescar_alfileres(&escena.elementos, &escena.alfileres);
}

/// Un aviso del raton moviendo lo elegido, de `desde` a `hasta`.
pub fn arrastrar(escena: &mut Escena, elegidos: &[u64], desde: Punto2, hasta: Punto2) {
    apuntar(escena, elegidos);
    let alfileres = escena.alfileres.clone();
    arrastrar_con_alfileres(
        &mut escena.elementos,
        &alfileres,
        |id| elegidos.contains(&id),
        desde,
        hasta,
    );
    escena.alfileres = refrescar_alfileres(&escena.elementos, &alfileres);
}

/// Un aviso del raton girando lo elegido con el tirador de giro.
pub fn girar(escena: &mut Escena, elegidos: &[u64], centro: Punto2, delta: f32) {
    apuntar(escena, elegidos);
    let alfileres = escena.alfileres.clone();
    girar_con_alfileres(
        &mut escena.elementos,
        &alfileres,
        |id| elegidos.contains(&id),
        centro,
        delta,
    );
    escena.alfileres = refrescar_alfileres(&escena.elementos, &alfileres);
}

/// Tras estirar lo elegido: lo clavado vuelve a su clavo.
pub fn sujetar(escena: &mut Escena, elegidos: &[u64]) {
    apuntar(escena, elegidos);
    let alfileres = escena.alfileres.clone();
    fijar_alfileres(&mut escena.elementos, &alfileres, |id| {
        elegidos.contains(&id)
    });
    escena.alfileres = refrescar_alfileres(&escena.elementos, &alfileres);
}

/// **Las cabezas de los clavos**, para encadenarlas detras de la escena. Un
/// circulito rojo con un filo blanco, del mismo tamano en pantalla a
/// cualquier zoom: es una marca de la interfaz, no dibujo.
pub fn ordenes_de_clavos(escena: &Escena, zoom: f32) -> Vec<Orden> {
    if escena.alfileres.is_empty() {
        return Vec::new();
    }
    let z = zoom.max(0.0001);
    let circulo = |c: Punto2, r: f32| -> Vec<Punto2> {
        (0..16)
            .map(|i| {
                let t = i as f32 * std::f32::consts::TAU / 16.0;
                Punto2::nuevo(c.x + r * t.cos(), c.y + r * t.sin())
            })
            .collect()
    };
    let mut salida = Vec::new();
    for p in puntos_de_alfileres(&escena.elementos, &escena.alfileres) {
        salida.push(Orden::Poligono {
            puntos: circulo(p, (RADIO_DE_LA_CABEZA + 1.5) / z),
            color: ColorRgba::opaco(1.0, 1.0, 1.0),
        });
        salida.push(Orden::Poligono {
            puntos: circulo(p, RADIO_DE_LA_CABEZA / z),
            color: COLOR_DEL_CLAVO,
        });
    }
    salida
}

// -------------------------------------------------------------------------
// El fichero: la clave `alfileres` del movil
// -------------------------------------------------------------------------

/// **Los clavos que trae el fichero**, con sus figuras ya traducidas a los
/// ids de la escena.
///
/// En el fichero cada agarre dice su figura por el `id` de TEXTO del JSON
/// (`elementId`), y en la escena las figuras tienen numero: se casan por
/// `enlace::tiene_id_de_texto`, como los enganches de flecha. Un clavo que
/// atraviesa algo que aqui no se sabe representar no se entiende y **no
/// entra**: se queda en el fichero tal cual (ver [`escribir_al_fichero`]).
pub fn leer_del_fichero(
    resto: &serde_json::Map<String, serde_json::Value>,
    elementos: &[Elemento],
) -> Vec<Alfiler> {
    crudos(resto)
        .iter()
        .filter_map(|v| alfiler_desde(v, elementos))
        .collect()
}

fn crudos(resto: &serde_json::Map<String, serde_json::Value>) -> Vec<serde_json::Value> {
    match resto.get("alfileres") {
        Some(serde_json::Value::Array(lista)) => lista.clone(),
        _ => Vec::new(),
    }
}

fn punto_desde(v: &serde_json::Value) -> Option<Punto2> {
    Some(Punto2::nuevo(
        v.get("x")?.as_f64()? as f32,
        v.get("y")?.as_f64()? as f32,
    ))
}

fn alfiler_desde(v: &serde_json::Value, elementos: &[Elemento]) -> Option<Alfiler> {
    let punto = punto_desde(v.get("punto")?)?;
    let mut agarres = Vec::new();
    for g in v.get("agarres")?.as_array()? {
        let texto = g.get("elementId")?.as_str()?;
        let e = elementos
            .iter()
            .find(|e| crate::enlace::tiene_id_de_texto(e, texto))?;
        agarres.push(Agarre {
            elemento: e.id,
            indice: g.get("indice").and_then(|i| i.as_u64()).map(|i| i as usize),
            local: g.get("local").and_then(punto_desde),
            t: g.get("t").and_then(|t| t.as_f64()).map(|t| t as f32),
        });
    }
    let a = Alfiler { punto, agarres };
    a.valido().then_some(a)
}

fn alfiler_hacia(a: &Alfiler, elementos: &[Elemento]) -> Option<serde_json::Value> {
    use serde_json::json;
    let mut agarres = Vec::new();
    for g in &a.agarres {
        let e = elementos.iter().find(|e| e.id == g.elemento)?;
        let mut m = serde_json::Map::new();
        m.insert("elementId".into(), json!(crate::enlace::id_de_texto_de(e)));
        if let Some(i) = g.indice {
            m.insert("indice".into(), json!(i));
        }
        if let Some(l) = g.local {
            m.insert("local".into(), json!({ "x": l.x as f64, "y": l.y as f64 }));
        }
        if let Some(t) = g.t {
            m.insert("t".into(), json!(t as f64));
        }
        agarres.push(serde_json::Value::Object(m));
    }
    Some(json!({
        "punto": { "x": a.punto.x as f64, "y": a.punto.y as f64 },
        "agarres": agarres,
    }))
}

/// **Deja en el fichero los clavos de la escena**, solo si cambiaron.
///
/// Un lienzo del movil que nadie toco no cambia ni un byte por haberlo
/// abierto aqui. Los clavos que no se entendieron al leer (atraviesan algo
/// ajeno) vuelven intactos.
pub fn escribir_al_fichero(
    resto: &mut serde_json::Map<String, serde_json::Value>,
    escena: &Escena,
) {
    let vivos = refrescar_alfileres(&escena.elementos, &escena.alfileres);
    let leidos = leer_del_fichero(resto, &escena.elementos);
    if vivos == refrescar_alfileres(&escena.elementos, &leidos) {
        return;
    }
    let ajenos: Vec<serde_json::Value> = crudos(resto)
        .into_iter()
        .filter(|v| alfiler_desde(v, &escena.elementos).is_none())
        .collect();
    let mut lista: Vec<serde_json::Value> = vivos
        .iter()
        .filter_map(|a| alfiler_hacia(a, &escena.elementos))
        .collect();
    lista.extend(ajenos);
    if lista.is_empty() && !resto.contains_key("alfileres") {
        return;
    }
    resto.insert("alfileres".into(), serde_json::Value::Array(lista));
}

#[cfg(test)]
mod pruebas {
    //! Las de `NudosTest.kt` del movil, con la misma analogia: dos listones
    //! clavados por un punto no se separan, pueden girar uno respecto del
    //! otro, y con dos clavos no gira nada.
    use super::*;
    use crate::vector::distancia_a_segmento;

    fn raya(id: u64, a: (f32, f32), b: (f32, f32)) -> Elemento {
        Elemento {
            id,
            figura: Figura::Linea {
                puntos: vec![Punto2::nuevo(a.0, a.1), Punto2::nuevo(b.0, b.1)],
            },
            x: a.0.min(b.0),
            y: a.1.min(b.1),
            ancho: (b.0 - a.0).abs(),
            alto: (b.1 - a.1).abs(),
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            grosor: 2.0,
            ..Default::default()
        }
    }

    fn circulo(id: u64, cx: f32, cy: f32, r: f32) -> Elemento {
        Elemento {
            id,
            figura: Figura::Elipse,
            x: cx - r,
            y: cy - r,
            ancho: 2.0 * r,
            alto: 2.0 * r,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            grosor: 2.0,
            ..Default::default()
        }
    }

    /// Tres rayas que dibujan un triangulo, cada una por su cuenta.
    fn triangulo() -> Vec<Elemento> {
        vec![
            raya(1, (0.0, 100.0), (100.0, 100.0)),
            raya(2, (100.0, 100.0), (50.0, 0.0)),
            raya(3, (50.0, 0.0), (0.0, 100.0)),
        ]
    }

    fn p(x: f32, y: f32) -> Punto2 {
        Punto2::nuevo(x, y)
    }

    fn cerca(a: Punto2, b: Punto2, tol: f32, que: &str) {
        assert!(
            a.distancia(b) <= tol,
            "{que}: {a:?} y {b:?} estan a {}",
            a.distancia(b)
        );
    }

    fn escena_con(elementos: Vec<Elemento>) -> Escena {
        let mut e = Escena::nueva();
        for x in elementos {
            e.anadir(x);
        }
        e
    }

    fn puntas(escena: &Escena, id: u64) -> Vec<Punto2> {
        vertices_en_el_mundo(escena.buscar(id).unwrap())
    }

    // ---- Clavar ----

    #[test]
    fn el_clavo_atraviesa_las_figuras_que_pasan_por_el_punto() {
        let a = clavar_en(&triangulo(), &[], p(100.0, 100.0), 10.0).expect("no clavo nada");
        let a = &a[0];
        assert!(a.valido());
        let mut ids: Vec<u64> = a.agarres.iter().map(|g| g.elemento).collect();
        ids.sort_unstable();
        assert_eq!(ids, vec![1, 2]);
        // Las dos tienen un vertice ahi: se agarran por el vertice.
        assert!(a.agarres.iter().all(|g| g.indice.is_some()));
    }

    #[test]
    fn una_figura_sola_no_se_puede_clavar() {
        let sola = vec![raya(1, (0.0, 0.0), (100.0, 0.0))];
        assert!(clavar_en(&sola, &[], p(0.0, 0.0), 10.0).is_none());
    }

    #[test]
    fn lejos_de_todo_no_se_clava_nada() {
        assert!(clavar_en(&triangulo(), &[], p(400.0, 400.0), 10.0).is_none());
    }

    #[test]
    fn tocar_un_clavo_lo_quita() {
        let con = clavar_en(&triangulo(), &[], p(100.0, 100.0), 10.0).unwrap();
        assert_eq!(
            clavar_en(&triangulo(), &con, p(100.0, 100.0), 10.0),
            Some(vec![])
        );
    }

    #[test]
    fn una_figura_bloqueada_no_se_deja_clavar() {
        // Caso negativo: el candado dice «esto no se toca», y un clavo es
        // tocarlo para siempre.
        let mut t = triangulo();
        t[1].bloqueado = true;
        assert!(clavar_en(&t, &[], p(100.0, 100.0), 10.0).is_none());
    }

    #[test]
    fn el_clavo_se_afina_hasta_la_interseccion() {
        let cruz = vec![
            raya(1, (0.0, 100.0), (200.0, 100.0)),
            raya(2, (120.0, 0.0), (120.0, 200.0)),
        ];
        let a = &clavar_en(&cruz, &[], p(123.0, 103.0), 14.0).unwrap()[0];
        cerca(a.punto, p(120.0, 100.0), 0.001, "el clavo no fue al cruce");
        // Ahi no hay vertice de nadie: las dos se agarran por su recorrido.
        assert!(a.agarres.iter().all(|g| g.indice.is_none()));
    }

    #[test]
    fn dos_circunferencias_se_clavan_en_su_interseccion() {
        let dos = vec![circulo(1, 0.0, 0.0, 100.0), circulo(2, 100.0, 0.0, 100.0)];
        let a = &clavar_en(&dos, &[], p(52.0, -84.0), 14.0).unwrap()[0];
        cerca(a.punto, p(50.0, -86.60), 1.5, "no clavo en el cruce");
        assert_eq!(a.agarres.len(), 2);
    }

    #[test]
    fn el_clavo_de_una_raya_se_mide_sobre_su_recorrido() {
        let escena = vec![
            raya(1, (0.0, 0.0), (200.0, 0.0)),
            raya(2, (50.0, -50.0), (50.0, 50.0)),
        ];
        let a = &clavar_en(&escena, &[], p(50.0, 0.0), 14.0).unwrap()[0];
        let g = a.agarres.iter().find(|g| g.elemento == 1).unwrap();
        assert!(g.local.is_none(), "no deberia agarrarla por la caja");
        assert!((g.t.unwrap() - 0.25).abs() < 0.02, "t = {:?}", g.t);
    }

    // ---- La ley de las libertades ----

    #[test]
    fn cuantos_clavos_decide_cuanto_se_puede_mover() {
        let uno = vec![Alfiler {
            punto: p(0.0, 0.0),
            agarres: vec![Agarre::por_vertice(1, 0), Agarre::por_vertice(2, 0)],
        }];
        let mut dos = uno.clone();
        dos.push(Alfiler {
            punto: p(9.0, 9.0),
            agarres: vec![Agarre::por_vertice(1, 1), Agarre::por_vertice(3, 0)],
        });
        assert_eq!(libertad_de(&[], 1), Libertad::Libre);
        assert_eq!(libertad_de(&uno, 1), Libertad::Gira);
        assert_eq!(libertad_de(&dos, 1), Libertad::Fija);
        assert_eq!(libertad_de(&dos, 3), Libertad::Gira);
    }

    #[test]
    fn con_un_clavo_la_figura_gira_sobre_el_en_vez_de_trasladarse() {
        let mut escena = escena_con(triangulo());
        assert!(soldar(&mut escena, p(100.0, 100.0), 10.0));
        let clavo = escena.alfileres[0].punto;
        // Se agarra la base por su mitad, lejos del clavo, y se baja.
        arrastrar(&mut escena, &[1], p(50.0, 100.0), p(50.0, 160.0));
        let a = puntas(&escena, 1);
        cerca(a[1], clavo, 0.5, "el clavo se ha soltado");
        // Ha girado: la punta libre ya no esta en la horizontal del clavo...
        assert!(a[0].y > 110.0, "no ha girado: {:?}", a[0]);
        // ...y sigue midiendo lo mismo: gira, no se estira.
        assert!((a[0].distancia(a[1]) - 100.0).abs() < 0.5);
        // La vecina no se ha movido: es una articulacion, no un bloque.
        cerca(
            puntas(&escena, 2)[0],
            p(100.0, 100.0),
            0.001,
            "la vecina se movio",
        );
    }

    #[test]
    fn con_dos_clavos_la_figura_no_se_mueve() {
        let cuatro = vec![
            raya(1, (0.0, 100.0), (200.0, 100.0)),
            raya(2, (0.0, 100.0), (0.0, 300.0)),
            raya(3, (200.0, 100.0), (200.0, 300.0)),
        ];
        let mut escena = escena_con(cuatro);
        soldar(&mut escena, p(0.0, 100.0), 10.0);
        soldar(&mut escena, p(200.0, 100.0), 10.0);
        assert_eq!(escena.alfileres.len(), 2);
        assert_eq!(libertad_de(&escena.alfileres, 1), Libertad::Fija);
        arrastrar(&mut escena, &[1], p(100.0, 100.0), p(140.0, 180.0));
        cerca(
            puntas(&escena, 1)[0],
            p(0.0, 100.0),
            0.001,
            "se movio con dos clavos",
        );
    }

    #[test]
    fn sin_clavos_la_figura_se_traslada() {
        let mut escena = escena_con(triangulo());
        arrastrar(&mut escena, &[1], p(50.0, 100.0), p(50.0, 160.0));
        cerca(
            puntas(&escena, 1)[0],
            p(0.0, 160.0),
            0.001,
            "no se traslado",
        );
        // Sin clavo, cada una por su lado: el triangulo se abre.
        cerca(
            puntas(&escena, 2)[0],
            p(100.0, 100.0),
            0.001,
            "la otra no deberia moverse",
        );
    }

    #[test]
    fn un_circulo_clavado_gira_alrededor_del_clavo() {
        let mut escena = escena_con(vec![
            circulo(1, 0.0, 0.0, 100.0),
            circulo(2, 100.0, 0.0, 100.0),
        ]);
        assert!(soldar(&mut escena, p(52.0, -84.0), 14.0));
        let clavo = escena.alfileres[0].punto;
        arrastrar(&mut escena, &[1], p(-100.0, 0.0), p(-90.0, 60.0));
        let despues = escena.alfileres[0].punto;
        cerca(despues, clavo, 0.5, "el clavo se ha movido");
        let o = escena.buscar(1).unwrap();
        assert!(o.angulo != 0.0, "no ha girado");
        let centro = Punto2::nuevo(o.x + o.ancho / 2.0, o.y + o.alto / 2.0);
        assert!(
            (despues.distancia(centro) - 100.0).abs() < 1.0,
            "el circulo se ha soltado del clavo"
        );
        assert_eq!(escena.buscar(2).unwrap().x, 0.0, "el otro circulo se movio");
    }

    #[test]
    fn la_raya_clavada_a_un_circulo_gira_sobre_el_clavo() {
        let mut escena = escena_con(vec![
            circulo(1, 0.0, 0.0, 100.0),
            raya(2, (-200.0, 0.0), (200.0, 0.0)),
        ]);
        soldar(&mut escena, p(100.0, 0.0), 14.0);
        let clavo = escena.alfileres[0].punto;
        cerca(clavo, p(100.0, 0.0), 0.6, "no clavo en el cruce");
        arrastrar(&mut escena, &[2], p(-150.0, 0.0), p(-140.0, 80.0));
        let r = puntas(&escena, 2);
        assert!((r[0].y).abs() > 1.0, "la raya no ha girado");
        cerca(escena.alfileres[0].punto, clavo, 0.6, "el clavo se movio");
        assert!(
            distancia_a_segmento(clavo, r[0], r[1]) < 1.0,
            "el clavo se ha salido de la raya"
        );
        assert_eq!(escena.buscar(1).unwrap().angulo, 0.0);
    }

    // ---- Llevarse el clavo ----

    #[test]
    fn tocar_el_clavo_lo_arranca_y_lo_lleva() {
        let mut escena = escena_con(triangulo());
        soldar(&mut escena, p(100.0, 100.0), 10.0);
        let i = clavo_en(&escena, p(100.0, 100.0), 10.0).expect("no encuentra el clavo");
        mover_clavo(&mut escena, i, p(160.0, 140.0));
        cerca(
            escena.alfileres[0].punto,
            p(160.0, 140.0),
            0.001,
            "el clavo no se fue",
        );
        cerca(
            puntas(&escena, 1)[1],
            p(160.0, 140.0),
            0.001,
            "a no siguio al clavo",
        );
        cerca(
            puntas(&escena, 2)[0],
            p(160.0, 140.0),
            0.001,
            "b no siguio al clavo",
        );
        // Las otras puntas no se mueven: es un clavo, no un arrastre.
        cerca(
            puntas(&escena, 1)[0],
            p(0.0, 100.0),
            0.001,
            "se movio la otra punta",
        );
    }

    #[test]
    fn mover_el_clavo_de_una_esquina_estira_el_triangulo() {
        let mut escena = escena_con(triangulo());
        soldar(&mut escena, p(100.0, 100.0), 10.0);
        soldar(&mut escena, p(50.0, 0.0), 10.0);
        soldar(&mut escena, p(0.0, 100.0), 10.0);
        assert_eq!(escena.alfileres.len(), 3);
        let i = clavo_en(&escena, p(100.0, 100.0), 10.0).unwrap();
        mover_clavo(&mut escena, i, p(190.0, 130.0));
        cerca(
            puntas(&escena, 1)[1],
            p(190.0, 130.0),
            0.001,
            "la esquina no fue al cursor",
        );
        cerca(
            puntas(&escena, 2)[0],
            p(190.0, 130.0),
            0.001,
            "el otro lado se quedo",
        );
        // Las otras dos esquinas siguen donde estaban: se deforma, no gira.
        cerca(
            puntas(&escena, 1)[0],
            p(0.0, 100.0),
            0.001,
            "giro en vez de estirarse",
        );
        cerca(
            puntas(&escena, 3)[0],
            p(50.0, 0.0),
            0.001,
            "se movio el vertice de arriba",
        );
    }

    // ---- El clavo no se despega ----

    #[test]
    fn el_clavo_se_queda_pegado_a_la_raya_aunque_gire() {
        let escena = vec![
            raya(1, (0.0, 0.0), (200.0, 0.0)),
            circulo(2, 60.0, 0.0, 40.0),
        ];
        let a = &clavar_en(&escena, &[], p(100.0, 0.0), 14.0).unwrap()[0];
        let g = a.agarres.iter().find(|g| g.elemento == 1).unwrap().clone();
        assert!(g.local.is_none(), "no deberia agarrarla por proporcion");
        let mut r = escena[0].clone();
        let antes = punto_del_agarre(&r, &g).unwrap();
        girar_elemento(&mut r, p(100.0, 0.0), std::f32::consts::FRAC_PI_2);
        let despues = punto_del_agarre(&r, &g).unwrap();
        let v = vertices_en_el_mundo(&r);
        assert!(
            distancia_a_segmento(despues, v[0], v[1]) < 0.5,
            "el clavo se ha ido de la raya"
        );
        assert!(
            (antes.distancia(p(0.0, 0.0)) - despues.distancia(v[0])).abs() < 0.5,
            "el clavo cambio de sitio dentro de la raya"
        );
    }

    #[test]
    fn un_clavo_en_el_medio_no_se_mueve_al_estirar_la_raya() {
        let mut escena = escena_con(vec![
            raya(1, (0.0, 100.0), (200.0, 100.0)),
            raya(2, (100.0, 0.0), (100.0, 200.0)),
        ]);
        soldar(&mut escena, p(100.0, 100.0), 10.0);
        let clavo = escena.alfileres[0].punto;
        // Se estira la horizontal por la derecha (como un tirador de caja) y
        // se le pide que vuelva a su clavo.
        con_punto_en_el_mundo(escena.buscar_mut(1).unwrap(), 1, p(240.0, 40.0));
        sujetar(&mut escena, &[1]);
        cerca(
            escena.alfileres[0].punto,
            clavo,
            1.0,
            "el clavo se ha movido",
        );
        let r = puntas(&escena, 1);
        assert!(
            distancia_a_segmento(clavo, r[0], r[1]) < 1.0,
            "la raya se solto del clavo"
        );
    }

    #[test]
    fn mover_un_punto_de_una_raya_girada_es_exacto() {
        let mut r = raya(1, (0.0, 0.0), (100.0, 0.0));
        r.angulo = std::f32::consts::PI / 5.0;
        con_punto_en_el_mundo(&mut r, 1, p(160.0, 40.0));
        cerca(
            vertices_en_el_mundo(&r)[1],
            p(160.0, 40.0),
            0.01,
            "no llego donde se pidio",
        );
    }

    #[test]
    fn enderezar_una_raya_no_cambia_lo_que_se_ve() {
        let mut r = raya(1, (0.0, 0.0), (100.0, 40.0));
        r.angulo = 0.7;
        let antes = vertices_en_el_mundo(&r);
        enderezar(&mut r);
        assert_eq!(r.angulo, 0.0);
        for (a, b) in antes.iter().zip(vertices_en_el_mundo(&r)) {
            cerca(*a, b, 0.001, "enderezar movio la raya");
        }
    }

    // ---- Girar con el tirador ----

    #[test]
    fn con_un_clavo_el_tirador_de_giro_gira_sobre_el_clavo() {
        let mut escena = escena_con(vec![
            raya(1, (100.0, 100.0), (200.0, 100.0)),
            raya(2, (100.0, 100.0), (100.0, 200.0)),
        ]);
        soldar(&mut escena, p(100.0, 100.0), 10.0);
        girar(&mut escena, &[1], p(150.0, 100.0), 0.5);
        cerca(
            puntas(&escena, 1)[0],
            p(100.0, 100.0),
            0.5,
            "el eje se movio",
        );
        assert!(puntas(&escena, 1)[1].y > 101.0, "no ha girado");
    }

    #[test]
    fn con_dos_clavos_ya_no_gira_y_sin_clavos_gira_como_siempre() {
        let mut escena = escena_con(vec![
            raya(1, (0.0, 0.0), (100.0, 0.0)),
            raya(2, (0.0, 0.0), (0.0, 100.0)),
            raya(3, (100.0, 0.0), (100.0, 100.0)),
            raya(4, (300.0, 0.0), (400.0, 0.0)),
        ]);
        soldar(&mut escena, p(0.0, 0.0), 10.0);
        soldar(&mut escena, p(100.0, 0.0), 10.0);
        let antes = escena.buscar(1).unwrap().clone();
        girar(&mut escena, &[1], p(50.0, 0.0), 0.5);
        assert_eq!(
            escena.buscar(1).unwrap().figura,
            antes.figura,
            "giro con dos clavos"
        );
        girar(&mut escena, &[4], p(350.0, 0.0), 0.5);
        assert_eq!(
            escena.buscar(4).unwrap().angulo,
            0.5,
            "sin clavos no giro sobre su centro"
        );
    }

    // ---- Que no se quede basura ----

    #[test]
    fn los_clavos_de_lo_que_ya_no_existe_se_tiran() {
        let mut t = triangulo();
        let alfileres = clavar_en(&t, &[], p(100.0, 100.0), 10.0).unwrap();
        t[1].borrado = true;
        assert!(refrescar_alfileres(&t, &alfileres).is_empty());
    }

    #[test]
    fn cada_clavo_da_un_punto_que_pintar() {
        let t = triangulo();
        let alfileres = clavar_en(&t, &[], p(100.0, 100.0), 10.0).unwrap();
        let marcas = puntos_de_alfileres(&t, &alfileres);
        assert_eq!(marcas, vec![p(100.0, 100.0)]);
        let escena = {
            let mut e = escena_con(t);
            e.alfileres = alfileres;
            e
        };
        // Dos circulos por clavo: el filo blanco y la cabeza roja.
        assert_eq!(ordenes_de_clavos(&escena, 1.0).len(), 2);
        assert!(ordenes_de_clavos(&Escena::nueva(), 1.0).is_empty());
    }

    #[test]
    fn soldar_se_deshace_de_un_golpe_y_uno_en_el_vacio_no_gasta_un_deshacer() {
        let mut escena = escena_con(triangulo());
        let pasos = escena.pasos_cerrados();
        assert!(!soldar(&mut escena, p(400.0, 400.0), 10.0));
        assert_eq!(
            escena.pasos_cerrados(),
            pasos,
            "un toque en el vacio dejo paso"
        );
        assert!(soldar(&mut escena, p(100.0, 100.0), 10.0));
        assert_eq!(escena.alfileres.len(), 1);
        assert!(escena.deshacer());
        assert!(escena.alfileres.is_empty(), "deshacer no quito el clavo");
        assert!(escena.rehacer());
        assert_eq!(escena.alfileres.len(), 1);
    }

    #[test]
    fn deshacer_el_arrastre_de_un_clavo_devuelve_figuras_y_clavo() {
        let mut escena = escena_con(triangulo());
        soldar(&mut escena, p(100.0, 100.0), 10.0);
        escena.abrir_paso();
        mover_clavo(&mut escena, 0, p(160.0, 140.0));
        escena.cerrar_paso();
        assert!(escena.deshacer());
        cerca(
            escena.alfileres[0].punto,
            p(100.0, 100.0),
            0.001,
            "el clavo no volvio",
        );
        cerca(
            puntas(&escena, 2)[0],
            p(100.0, 100.0),
            0.001,
            "la raya no volvio",
        );
    }

    // ---- El fichero del movil ----

    /// Dos rayas del movil clavadas por su punta comun, y un clavo que
    /// atraviesa algo que aqui no se sabe dibujar.
    const DEL_MOVIL: &str = r##"{"type":"excalidraw","elements":[
        {"id":"a","type":"line","x":0,"y":100,"width":100,"height":0,
         "strokeColor":"#000000","points":[{"x":0,"y":0},{"x":100,"y":0}]},
        {"id":"b","type":"line","x":100,"y":100,"width":50,"height":100,
         "strokeColor":"#000000","points":[{"x":0,"y":0},{"x":-50,"y":-100}]},
        {"id":"s","type":"pixpin-solid","x":0,"y":0,"width":10,"height":10}
    ],
    "alfileres":[
        {"punto":{"x":100.0,"y":100.0},"agarres":[{"elementId":"a","indice":1},{"elementId":"b","indice":0}]},
        {"punto":{"x":5.0,"y":5.0},"agarres":[{"elementId":"a","t":0.1},{"elementId":"s","local":{"x":0.5,"y":0.5}}]}
    ]}"##;

    #[test]
    fn los_clavos_del_movil_llegan_con_sus_figuras_de_la_escena() {
        let lienzo = crate::excalidraw::leer(DEL_MOVIL).unwrap();
        let escena = crate::excalidraw::a_escena(&lienzo);
        // El segundo atraviesa el solido, que aqui es ajeno: no entra.
        assert_eq!(escena.alfileres.len(), 1);
        let a = &escena.alfileres[0];
        let ids: Vec<u64> = a.agarres.iter().map(|g| g.elemento).collect();
        assert_eq!(
            ids,
            vec![1, 2],
            "no se tradujeron a los numeros de la escena"
        );
        assert_eq!(a.agarres[0].indice, Some(1));
        // Y ya sujetan: llevarse el clavo se lleva las dos puntas.
        let mut escena = escena;
        mover_clavo(&mut escena, 0, p(130.0, 90.0));
        cerca(puntas(&escena, 1)[1], p(130.0, 90.0), 0.001, "a no siguio");
        cerca(puntas(&escena, 2)[0], p(130.0, 90.0), 0.001, "b no siguio");
    }

    #[test]
    fn un_lienzo_con_clavos_que_nadie_toco_sale_igual() {
        let lienzo = crate::excalidraw::leer(DEL_MOVIL).unwrap();
        let escena = crate::excalidraw::a_escena(&lienzo);
        let salida = crate::excalidraw::con_escena(&lienzo, &escena);
        assert_eq!(salida.resto.get("alfileres"), lienzo.resto.get("alfileres"));
    }

    #[test]
    fn quitar_un_clavo_se_guarda_y_el_ajeno_sobrevive() {
        let lienzo = crate::excalidraw::leer(DEL_MOVIL).unwrap();
        let mut escena = crate::excalidraw::a_escena(&lienzo);
        assert!(
            soldar(&mut escena, p(100.0, 100.0), 10.0),
            "tocar el clavo no lo quito"
        );
        assert!(escena.alfileres.is_empty());
        let salida = crate::excalidraw::con_escena(&lienzo, &escena);
        let lista = salida.resto["alfileres"].as_array().unwrap();
        assert_eq!(
            lista.len(),
            1,
            "se perdio el clavo ajeno o quedo el quitado"
        );
        assert_eq!(lista[0]["agarres"][1]["elementId"], "s");
    }

    #[test]
    fn un_clavo_nacido_aqui_viaja_con_los_ids_de_texto() {
        let lienzo = crate::excalidraw::leer(DEL_MOVIL).unwrap();
        let mut escena = crate::excalidraw::a_escena(&lienzo);
        let nueva = escena.anadir(raya(0, (50.0, 50.0), (50.0, 150.0)));
        assert!(soldar(&mut escena, p(50.0, 100.0), 10.0));
        let salida = crate::excalidraw::con_escena(&lienzo, &escena);
        let json = crate::excalidraw::escribir(&salida);
        let otra = crate::excalidraw::leer(&json).unwrap();
        let vuelta = crate::excalidraw::a_escena(&otra);
        assert_eq!(vuelta.alfileres.len(), 2, "el clavo nuevo no volvio");
        assert!(
            vuelta
                .alfileres
                .iter()
                .any(|a| a.punto.distancia(p(50.0, 100.0)) < 0.01),
            "el clavo nuevo volvio en otro sitio: {:?}",
            vuelta.alfileres
        );
        let texto = serde_json::to_string(&salida.resto["alfileres"]).unwrap();
        assert!(
            texto.contains(&crate::enlace::id_de_texto(nueva)),
            "{texto}"
        );
        assert!(texto.contains("\"a\""), "{texto}");
    }
}
