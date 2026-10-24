//! **Geometria exacta de lamina**: arcos por centro y angulos, semicircunferencias,
//! cuadrantes, sectores y segmentos circulares, marcas de angulo (con uno o dos
//! arquitos, rellenas, con su valor), la marca del angulo recto, puntos con su
//! letra, el trazo raya-punto y el relleno degradado (lineal y radial).
//!
//! Pedido del usuario (15-oct-2026): «mejora las otras herramientas para que
//! puedan hacer todo eso», con una lamina de geometria de colegio delante
//! (circunferencias, cuadrantes rellenos con degradado, arcos de color sobre la
//! circunferencia, marcas de angulo, el cuadradito del angulo recto...).
//!
//! # Un motor con variables, no figuras nuevas
//!
//! Nada de esto es una `Figura` nueva. Un sector, un cuadrante, una
//! semicircunferencia y una marca de angulo **son arcos** (`Figura::Arco`, el
//! `pixpin-arc` del movil) con tres variables mas en `Extras`:
//!
//! - [`CierreArco`] (`cierreArco`): abierto (sin la clave), sector (con los
//!   dos radios, el «trozo de pizza») o segmento (con la cuerda).
//! - [`MarcaAngulo`] (`marcaAngulo`): el arco es la marca de un angulo; se
//!   pinta con `arcos` arquitos concentricos, o como el cuadradito si es
//!   `recto`, y con su `valor` escrito al lado.
//! - [`Degradado`] (`degradado`): el relleno, en vez de liso, va de un color a
//!   otro (lineal o radial). Vale para todo lo que se rellena: rectangulo,
//!   rombo, ovalo, sector, segmento y la region del bote.
//!
//! Asi el movil, que todavia no conoce estas claves, **sigue viendo un arco**
//! en el mismo sitio (la reserva): un sector sin la clave es su arco, una marca
//! de angulo es un arquito y un degradado es el relleno liso de su
//! `backgroundColor`. Ver `docs/android/2026-10-15-geometria-exacta.md`.
//!
//! El trazo raya-punto es un `EstiloTrazo` mas, que viaja al movil como
//! `dashed` (lo mas parecido que entiende) con la clave `trazoRayaPunto`.
//!
//! # Las mismas piezas para el usuario y para el asistente
//!
//! Las constructoras de aqui ([`arco`], [`semicircunferencia`],
//! [`marca_de_angulo`]...) las usan la herramienta del lienzo (`gesto.rs`), el
//! asistente (`pixpin-ia`, `geometria_precisa`) y el calco de laminas
//! (`pixpin-ia/diagrama`): un solo sitio que sabe que es un cuadrante.
//!
//! # Angulos
//!
//! Como todo el motor: **radianes y en pantalla** (la y baja), asi que un
//! angulo positivo gira en el sentido de las agujas del reloj a la vista. El
//! asistente habla en grados de matematicas (antihorario, 90 arriba) y la
//! traduccion la hace el (cambiando el signo).

use std::f32::consts::{PI, TAU};

use serde::{Deserialize, Serialize};

use crate::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};
use crate::pintado::Orden;
use crate::vector::Punto2;

/// **Como traza la herramienta Arco** (`gesto/geometria.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModoArco {
    /// Repasando una guia (un ovalo de referencia), como el movil.
    #[default]
    Guia,
    /// Clic en el centro, clic donde empieza y clic donde acaba.
    CentroYPuntos,
    /// Donde empieza, un punto por donde pasa y donde acaba.
    TresPuntos,
}

/// Como se cierra un arco.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CierreArco {
    /// Con los dos radios hasta el centro: el sector (y el cuadrante).
    Sector,
    /// Con la cuerda entre sus puntas: el segmento circular.
    Segmento,
}

impl CierreArco {
    pub fn palabra(self) -> &'static str {
        match self {
            CierreArco::Sector => "sector",
            CierreArco::Segmento => "segmento",
        }
    }
    pub fn desde_palabra(s: &str) -> Option<Self> {
        match s {
            "sector" => Some(CierreArco::Sector),
            "segmento" | "segment" => Some(CierreArco::Segmento),
            _ => None,
        }
    }
}

/// **El arco es la marca de un angulo.** Su centro es el vertice y su tramo
/// va de un lado al otro del angulo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarcaAngulo {
    /// Cuantos arquitos concentricos (1 a 3): dos angulos con el mismo numero
    /// de arquitos son iguales, como se marca a mano.
    #[serde(default = "un_arco")]
    pub arcos: u8,
    /// El angulo recto: el cuadradito en vez del arco.
    #[serde(default)]
    pub recto: bool,
    /// Lo que se escribe al lado («45°», «θ», «a»). `None`: nada.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valor: Option<String>,
}

fn un_arco() -> u8 {
    1
}

impl Default for MarcaAngulo {
    fn default() -> Self {
        Self {
            arcos: 1,
            recto: false,
            valor: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TipoDegradado {
    Lineal,
    Radial,
}

impl TipoDegradado {
    pub fn palabra(self) -> &'static str {
        match self {
            TipoDegradado::Lineal => "lineal",
            TipoDegradado::Radial => "radial",
        }
    }
    pub fn desde_palabra(s: &str) -> Option<Self> {
        match s {
            "lineal" | "linear" => Some(TipoDegradado::Lineal),
            "radial" => Some(TipoDegradado::Radial),
            _ => None,
        }
    }
}

/// **Un relleno que va de un color a otro.**
///
/// `origen` y `destino` van en **fracciones de la caja** de la figura (0 a 1),
/// no en pixeles: asi el degradado se mueve y se estira con ella sin que haya
/// que tocarlo. Lineal: de `desde` en `origen` a `hasta` en `destino`. Radial:
/// `desde` en el centro (`origen`) y `hasta` a la distancia de `destino`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Degradado {
    pub tipo: TipoDegradado,
    pub desde: ColorRgba,
    pub hasta: ColorRgba,
    #[serde(default = "en_medio")]
    pub origen: (f32, f32),
    #[serde(default = "a_la_esquina")]
    pub destino: (f32, f32),
}

fn en_medio() -> (f32, f32) {
    (0.5, 0.5)
}
fn a_la_esquina() -> (f32, f32) {
    (1.0, 1.0)
}

impl Degradado {
    /// El radial de fabrica: `desde` en el centro de la figura y `hasta` en
    /// sus esquinas.
    pub fn radial(desde: ColorRgba, hasta: ColorRgba) -> Self {
        Self {
            tipo: TipoDegradado::Radial,
            desde,
            hasta,
            origen: (0.5, 0.5),
            destino: (1.0, 1.0),
        }
    }
    /// El lineal de fabrica: de izquierda a derecha.
    pub fn lineal(desde: ColorRgba, hasta: ColorRgba) -> Self {
        Self {
            tipo: TipoDegradado::Lineal,
            desde,
            hasta,
            origen: (0.0, 0.5),
            destino: (1.0, 0.5),
        }
    }
    /// El que se pone desde el panel con un color de relleno: de ese color a
    /// uno mucho mas claro (el «cuadrante coloreado» de las laminas).
    pub fn desde_relleno(tipo: TipoDegradado, c: ColorRgba) -> Self {
        let claro = mezclar(c, ColorRgba { a: c.a, ..BLANCO }, 0.8);
        match tipo {
            TipoDegradado::Radial => Self::radial(c, claro),
            TipoDegradado::Lineal => Self::lineal(c, claro),
        }
    }
    /// El color a la altura `t` (0 en `desde`, 1 en `hasta`).
    pub fn color_en(&self, t: f32) -> ColorRgba {
        mezclar(self.desde, self.hasta, t.clamp(0.0, 1.0))
    }
}

const BLANCO: ColorRgba = ColorRgba::opaco(1.0, 1.0, 1.0);

fn mezclar(a: ColorRgba, b: ColorRgba, t: f32) -> ColorRgba {
    ColorRgba {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
}

// ---------------------------------------------------------------------------
// Angulos

/// El angulo de `p` visto desde `c`, en radianes de pantalla.
pub fn angulo_de(c: Punto2, p: Punto2) -> f32 {
    (p.y - c.y).atan2(p.x - c.x)
}

/// Lleva un angulo a `[0, TAU)`.
pub fn a_una_vuelta(a: f32) -> f32 {
    let r = a.rem_euclid(TAU);
    if r >= TAU { 0.0 } else { r }
}

/// El punto de la circunferencia `(c, r)` en el angulo `a`.
pub fn en_la_circunferencia(c: Punto2, r: f32, a: f32) -> Punto2 {
    Punto2::nuevo(c.x + r * a.cos(), c.y + r * a.sin())
}

/// **El angulo menor entre dos rayos** que salen de `v` hacia `p1` y `p2`:
/// `(inicio, barrido)` con el barrido con signo y de modulo como mucho media
/// vuelta. Es lo que mide una marca de angulo.
pub fn angulo_menor(v: Punto2, p1: Punto2, p2: Punto2) -> (f32, f32) {
    let a1 = angulo_de(v, p1);
    let mut d = angulo_de(v, p2) - a1;
    while d > PI {
        d -= TAU;
    }
    while d <= -PI {
        d += TAU;
    }
    (a1, d)
}

/// Si `a` cae dentro del tramo que empieza en `inicio` y barre `barrido`.
pub fn dentro_del_barrido(a: f32, inicio: f32, barrido: f32) -> bool {
    if barrido.abs() >= TAU - 1e-5 {
        return true;
    }
    let d = if barrido >= 0.0 {
        a_una_vuelta(a - inicio)
    } else {
        a_una_vuelta(inicio - a)
    };
    d <= barrido.abs() + 1e-5
}

// ---------------------------------------------------------------------------
// Constructoras: las piezas que comparten la herramienta, el asistente y el
// calco de laminas.

/// La base de todo lo de aqui: liso (rugosidad 0), negro, grosor 2. Quien
/// llame cambia lo que quiera despues.
fn base(figura: Figura, centro: Punto2, r: f32) -> Elemento {
    Elemento {
        figura,
        x: centro.x - r,
        y: centro.y - r,
        ancho: 2.0 * r,
        alto: 2.0 * r,
        rugosidad: 0.0,
        trazo: ColorRgba::opaco(0.12, 0.12, 0.12),
        estilo_relleno: crate::relleno::EstiloRelleno::Solido,
        ..Default::default()
    }
}

/// Una circunferencia exacta de centro `c` y radio `r`.
pub fn circunferencia(c: Punto2, r: f32) -> Elemento {
    base(Figura::Elipse, c, r.abs())
}

/// **Un arco** de la circunferencia `(c, r)` que empieza en `inicio` y barre
/// `barrido` (radianes de pantalla; positivo, a favor del reloj a la vista).
pub fn arco(c: Punto2, r: f32, inicio: f32, barrido: f32) -> Elemento {
    base(
        Figura::Arco {
            inicio,
            barrido: Some(barrido.clamp(-TAU, TAU)),
        },
        c,
        r.abs(),
    )
}

/// Un arco cerrado como sector (los dos radios) o segmento (la cuerda).
pub fn arco_cerrado(c: Punto2, r: f32, inicio: f32, barrido: f32, cierre: CierreArco) -> Elemento {
    let mut e = arco(c, r, inicio, barrido);
    e.extras.cierre = Some(cierre);
    e
}

/// **El sector** de centro `c` y radio `r`.
pub fn sector(c: Punto2, r: f32, inicio: f32, barrido: f32) -> Elemento {
    arco_cerrado(c, r, inicio, barrido, CierreArco::Sector)
}

/// **El cuadrante**: el sector de un cuarto de vuelta que sale del radio
/// `c→a`, hacia el lado del reloj (`horario`) o en contra, a la vista.
pub fn cuadrante(c: Punto2, a: Punto2, horario: bool) -> Elemento {
    let s = if horario { 1.0 } else { -1.0 };
    sector(c, c.distancia(a), angulo_de(c, a), s * PI / 2.0)
}

/// **La semicircunferencia sobre el diametro `a`–`b`**, hacia el lado del
/// reloj desde `a` (`horario`) o en contra, a la vista. Con `a` a la izquierda
/// y `b` a la derecha, `horario` es la de arriba.
pub fn semicircunferencia(a: Punto2, b: Punto2, horario: bool) -> Elemento {
    let c = a.hacia(b, 0.5);
    let s = if horario { 1.0 } else { -1.0 };
    arco(c, a.distancia(c), angulo_de(c, a), s * PI)
}

/// El arco de centro `c` que va de `a` a la direccion de `b`, con el radio de
/// `a`, a favor del reloj (`horario`) o en contra. Es el «centro y dos
/// puntos» del compas.
pub fn arco_por_centro_y_dos_puntos(c: Punto2, a: Punto2, b: Punto2, horario: bool) -> Elemento {
    let inicio = angulo_de(c, a);
    let d = a_una_vuelta(angulo_de(c, b) - inicio);
    let barrido = if horario {
        d
    } else if d == 0.0 {
        0.0
    } else {
        d - TAU
    };
    arco(c, c.distancia(a), inicio, barrido)
}

/// El centro de la circunferencia por tres puntos (`None` si estan en linea).
pub fn circuncentro(a: Punto2, b: Punto2, c: Punto2) -> Option<Punto2> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    let escala = a.distancia(b).max(b.distancia(c)).max(1.0);
    if d.abs() < 1e-4 * escala * escala {
        return None;
    }
    let (a2, b2, c2) = (
        a.x * a.x + a.y * a.y,
        b.x * b.x + b.y * b.y,
        c.x * c.x + c.y * c.y,
    );
    Some(Punto2::nuevo(
        (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d,
        (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d,
    ))
}

/// **El arco que pasa por tres puntos**: empieza en `a`, pasa por `m` y acaba
/// en `b`. `None` si estan en linea.
pub fn arco_por_tres_puntos(a: Punto2, m: Punto2, b: Punto2) -> Option<Elemento> {
    let c = circuncentro(a, m, b)?;
    let inicio = angulo_de(c, a);
    let hasta_b = a_una_vuelta(angulo_de(c, b) - inicio);
    let hasta_m = a_una_vuelta(angulo_de(c, m) - inicio);
    let barrido = if hasta_m <= hasta_b {
        hasta_b
    } else {
        hasta_b - TAU
    };
    Some(arco(c, c.distancia(a), inicio, barrido))
}

/// Radio de fabrica de una marca de angulo, en pixeles del documento.
pub const RADIO_MARCA: f32 = 22.0;

/// **La marca del angulo** de vertice `v` entre los rayos hacia `p1` y `p2`
/// (el menor de los dos), de radio `r`, con `arcos` arquitos. Sin relleno: lo
/// pone quien llame (en las laminas, translucido del color del trazo).
pub fn marca_de_angulo(v: Punto2, p1: Punto2, p2: Punto2, r: f32, arcos: u8) -> Elemento {
    let (inicio, barrido) = angulo_menor(v, p1, p2);
    let mut e = sector(v, r, inicio, barrido);
    e.grosor = 1.5;
    e.extras.marca_angulo = Some(MarcaAngulo {
        arcos: arcos.clamp(1, 3),
        ..Default::default()
    });
    e
}

/// **El cuadradito del angulo recto** en `v` entre los rayos hacia `p1` y
/// `p2`, de lado `lado`.
pub fn marca_de_angulo_recto(v: Punto2, p1: Punto2, p2: Punto2, lado: f32) -> Elemento {
    let mut e = marca_de_angulo(v, p1, p2, lado, 1);
    if let Some(m) = e.extras.marca_angulo.as_mut() {
        m.recto = true;
    }
    e
}

/// **Un punto con su letra**, la letra hacia `angulo_letra` (radianes de
/// pantalla) a la distancia de fabrica del movil.
pub fn punto_con_letra(p: Punto2, letra: &str, angulo_letra: f32) -> Elemento {
    Elemento {
        figura: Figura::Punto {
            letra: letra.to_string(),
            angulo: angulo_letra,
            radio: 14.0,
        },
        x: p.x,
        y: p.y,
        grosor: 2.0,
        rugosidad: 0.0,
        trazo: ColorRgba::opaco(0.12, 0.12, 0.12),
        ..Default::default()
    }
}

/// Una recta exacta (sin temblor) de `a` a `b`.
pub fn recta(a: Punto2, b: Punto2) -> Elemento {
    Elemento {
        figura: Figura::Linea { puntos: vec![a, b] },
        x: a.x.min(b.x),
        y: a.y.min(b.y),
        ancho: (a.x - b.x).abs(),
        alto: (a.y - b.y).abs(),
        rugosidad: 0.0,
        trazo: ColorRgba::opaco(0.12, 0.12, 0.12),
        ..Default::default()
    }
}

/// Una flecha exacta de `a` a `b` con las puntas pedidas.
pub fn flecha(
    a: Punto2,
    b: Punto2,
    punta_inicio: crate::formas::TipoPunta,
    punta_fin: crate::formas::TipoPunta,
) -> Elemento {
    let mut e = recta(a, b);
    e.figura = Figura::Flecha {
        puntos: vec![a, b],
        punta_inicio,
        punta_fin,
        codos: false,
    };
    e
}

// ---------------------------------------------------------------------------
// Lo que se lee de una figura

/// El centro y el radio de lo que es una circunferencia (un ovalo o un arco de
/// caja cuadrada). `None` en lo demas, y en los ovalos achatados.
pub fn centro_y_radio(e: &Elemento) -> Option<(Punto2, f32)> {
    if !matches!(e.figura, Figura::Elipse | Figura::Arco { .. }) {
        return None;
    }
    let (rx, ry) = (e.ancho / 2.0, e.alto / 2.0);
    if rx <= 0.0 || (rx - ry).abs() > 0.02 * rx.max(ry) + 0.5 {
        return None;
    }
    Some((Punto2::nuevo(e.x + rx, e.y + ry), (rx + ry) / 2.0))
}

/// El tramo de un arco: `(inicio, barrido)`, la vuelta entera para el ovalo.
pub fn tramo_de(e: &Elemento) -> Option<(f32, f32)> {
    match e.figura {
        Figura::Elipse => Some((0.0, TAU)),
        Figura::Arco { inicio, barrido } => Some((inicio, barrido.unwrap_or(TAU))),
        _ => None,
    }
}

/// **Los dos puntos de tangencia** desde `p` a la circunferencia `(c, r)`:
/// donde toca la recta que sale de `p` y la roza. `None` si `p` esta dentro.
pub fn tangentes_desde(p: Punto2, c: Punto2, r: f32) -> Option<[Punto2; 2]> {
    let d = p.distancia(c);
    if d <= r + 1e-3 || r <= 0.0 {
        return None;
    }
    // El angulo en el centro entre la recta al punto y el radio al punto de
    // tangencia: cos = r / d.
    let base = angulo_de(c, p);
    let abre = (r / d).acos();
    Some([
        en_la_circunferencia(c, r, base + abre),
        en_la_circunferencia(c, r, base - abre),
    ])
}

/// El contorno cerrado de un arco con cierre (sector o segmento), sin girar.
/// `None` si el arco va abierto.
pub fn contorno_cerrado(e: &Elemento) -> Option<Vec<Punto2>> {
    let Figura::Arco { inicio, barrido } = e.figura else {
        return None;
    };
    let cierre = e
        .extras
        .cierre
        .or(e.extras.marca_angulo.as_ref().map(|_| CierreArco::Sector))?;
    let mut puntos = crate::pintado::arco_muestreado(e, inicio, barrido);
    if puntos.len() < 2 {
        return None;
    }
    if cierre == CierreArco::Sector && barrido.is_some_and(|b| b.abs() < TAU - 1e-4) {
        puntos.push(Punto2::nuevo(e.x + e.ancho / 2.0, e.y + e.alto / 2.0));
    }
    Some(puntos)
}

// ---------------------------------------------------------------------------
// Pintado

fn con_opacidad(c: ColorRgba, o: f32) -> ColorRgba {
    ColorRgba {
        a: c.a * o.clamp(0.0, 1.0),
        ..c
    }
}

/// **Las ordenes de un arco** de lamina: su relleno (liso o degradado) si es
/// sector o segmento, y su trazo; o la marca de angulo con sus arquitos, su
/// cuadradito y su valor. `None` si es un arco de los de siempre (abierto,
/// sin marca y sin degradado), que pinta `pintado.rs` como siempre.
pub(crate) fn ordenes_de_arco(e: &Elemento, color: ColorRgba) -> Option<Vec<Orden>> {
    let Figura::Arco {
        inicio,
        barrido: Some(barrido),
    } = e.figura
    else {
        return None;
    };
    if e.extras.cierre.is_none() && e.extras.marca_angulo.is_none() {
        return None;
    }
    let c = Punto2::nuevo(e.x + e.ancho / 2.0, e.y + e.alto / 2.0);
    let r = e.ancho.min(e.alto) / 2.0;
    let mut salida = Vec::new();
    let relleno = e
        .relleno
        .filter(|c| c.a > 0.0)
        .map(|c| con_opacidad(c, e.opacidad));

    if let Some(m) = &e.extras.marca_angulo {
        if m.recto {
            let u1 = en_la_circunferencia(Punto2::nuevo(0.0, 0.0), 1.0, inicio);
            let u2 = en_la_circunferencia(Punto2::nuevo(0.0, 0.0), 1.0, inicio + barrido);
            let a = c.sumar(u1.escalar(r));
            let b = c.sumar(u1.escalar(r)).sumar(u2.escalar(r));
            let d = c.sumar(u2.escalar(r));
            if let Some(rel) = relleno {
                salida.extend(rellenar(e, &[c, a, b, d], rel));
            }
            salida.push(Orden::Polilinea {
                puntos: vec![a, b, d],
                color,
                grosor: e.grosor,
                estilo: e.estilo,
            });
        } else {
            if let Some(rel) = relleno
                && let Some(poligono) = contorno_cerrado(e)
            {
                salida.extend(rellenar(e, &poligono, rel));
            }
            let sep = (e.grosor * 2.2).max(3.0);
            for k in 0..m.arcos.clamp(1, 3) {
                let rk = r - k as f32 * sep;
                if rk <= 1.0 {
                    break;
                }
                let pasos = ((crate::pintado::pasos_del_arco(rk) as f32 * barrido.abs() / TAU)
                    as usize)
                    .max(8);
                let puntos = (0..=pasos)
                    .map(|i| {
                        en_la_circunferencia(c, rk, inicio + barrido * i as f32 / pasos as f32)
                    })
                    .collect();
                salida.push(Orden::Polilinea {
                    puntos,
                    color,
                    grosor: e.grosor,
                    estilo: e.estilo,
                });
            }
        }
        if let Some(valor) = m.valor.as_ref().filter(|v| !v.is_empty()) {
            let tam = e.extras.tam_letra.unwrap_or(14.0);
            let familia = e
                .extras
                .familia
                .clone()
                .unwrap_or_else(|| "Segoe UI".to_string());
            let estilo = crate::texto::EstiloDeTexto::default();
            let (ancho, alto) = crate::texto::medida(valor, tam, &familia, estilo);
            // Por fuera de la marca, sobre su bisectriz: es donde lo pone uno
            // a mano y donde no tapa los lados.
            let bis = inicio + barrido / 2.0;
            let lejos = r + 4.0 + 0.5 * (ancho * bis.cos().abs() + alto * bis.sin().abs());
            let centro_texto = en_la_circunferencia(c, lejos, bis);
            salida.push(Orden::Texto {
                texto: valor.clone(),
                x: centro_texto.x - ancho / 2.0,
                y: centro_texto.y - alto / 2.0,
                tam,
                familia,
                color,
                ancho_max: crate::texto::SIN_PARTIR,
                negrita: false,
                cursiva: false,
            });
        }
        return Some(salida);
    }

    // Sector o segmento.
    let poligono = contorno_cerrado(e)?;
    if let Some(rel) = relleno.or_else(|| e.extras.degradado.map(|d| d.desde)) {
        salida.extend(rellenar(e, &poligono, rel));
    }
    let mut borde = poligono;
    if let Some(p) = borde.first().copied() {
        borde.push(p);
    }
    salida.push(Orden::Polilinea {
        puntos: borde,
        color,
        grosor: e.grosor,
        estilo: e.estilo,
    });
    Some(salida)
}

/// El relleno de un poligono: liso, o en bandas si la figura lleva degradado.
fn rellenar(e: &Elemento, poligono: &[Punto2], liso: ColorRgba) -> Vec<Orden> {
    match &e.extras.degradado {
        Some(d) => ordenes_de_degradado(poligono, e.caja(), d, e.opacidad),
        None => vec![Orden::Relleno {
            puntos: poligono.to_vec(),
            color: liso,
        }],
    }
}

/// Cuantas bandas tiene un degradado. Con colores de lamina (claros y
/// parecidos) el salto entre dos bandas queda por debajo de lo que se ve.
pub const BANDAS_DEGRADADO: usize = 48;

/// **El degradado, en bandas de relleno liso.**
///
/// No es una orden nueva a proposito: son cinco los que pintan ordenes
/// (pantalla, PNG, SVG, PDF, pin) y con bandas de `Orden::Relleno` los cinco
/// lo pintan igual sin saber que es un degradado.
///
/// Con colores opacos las bandas se **apilan** (cada una encima de la
/// anterior, mas pequena): sin juntas entre bandas vecinas, que con el
/// suavizado dejarian rayitas claras. Con colores translucidos apilar
/// sumaria tinta, asi que van **una al lado de otra**, sin solaparse.
pub fn ordenes_de_degradado(
    poligono: &[Punto2],
    caja: (f32, f32, f32, f32),
    d: &Degradado,
    opacidad: f32,
) -> Vec<Orden> {
    if poligono.len() < 3 {
        return Vec::new();
    }
    let (x0, y0, x1, y1) = caja;
    let en = |f: (f32, f32)| Punto2::nuevo(x0 + f.0 * (x1 - x0), y0 + f.1 * (y1 - y0));
    let (o, q) = (en(d.origen), en(d.destino));
    let n = BANDAS_DEGRADADO;
    let color = |t: f32| con_opacidad(d.color_en(t), opacidad);
    let opaco = d.desde.a >= 0.999 && d.hasta.a >= 0.999 && opacidad >= 0.999;
    let mut salida = Vec::with_capacity(n + 1);
    match d.tipo {
        TipoDegradado::Lineal => {
            let eje = q.restar(o);
            let l2 = eje.producto(eje);
            if l2 < 1e-6 {
                return vec![Orden::Relleno {
                    puntos: poligono.to_vec(),
                    color: color(0.0),
                }];
            }
            // t(p) = (p - o)·eje / |eje|^2: 0 en el origen, 1 en el destino.
            let t = |p: Punto2| p.restar(o).producto(eje) / l2;
            if opaco {
                salida.push(Orden::Relleno {
                    puntos: poligono.to_vec(),
                    color: color(0.0),
                });
                for k in 1..=n {
                    let tk = k as f32 / n as f32;
                    let trozo = recortar_semiplano(poligono, |p| t(p) - tk);
                    if trozo.len() >= 3 {
                        salida.push(Orden::Relleno {
                            puntos: trozo,
                            color: color(tk),
                        });
                    }
                }
            } else {
                for k in 0..=n {
                    let (a, b) = (k as f32 / n as f32, (k + 1) as f32 / n as f32);
                    let mut trozo = poligono.to_vec();
                    if k > 0 {
                        trozo = recortar_semiplano(&trozo, |p| t(p) - a);
                    }
                    if k < n {
                        trozo = recortar_semiplano(&trozo, |p| b - t(p));
                    }
                    if trozo.len() >= 3 {
                        salida.push(Orden::Relleno {
                            puntos: trozo,
                            color: color(a),
                        });
                    }
                }
            }
        }
        TipoDegradado::Radial => {
            let radio = o.distancia(q).max(1.0);
            // Fuera del ultimo circulo, el color de fuera.
            let disco = |k: usize| -> Vec<Punto2> {
                let rk = radio * k as f32 / n as f32;
                recortar_a_disco(poligono, o, rk)
            };
            if opaco {
                salida.push(Orden::Relleno {
                    puntos: poligono.to_vec(),
                    color: color(1.0),
                });
                for k in (1..=n).rev() {
                    let trozo = disco(k);
                    if trozo.len() >= 3 {
                        salida.push(Orden::Relleno {
                            puntos: trozo,
                            color: color((k as f32 - 0.5) / n as f32),
                        });
                    }
                }
            } else {
                let mut dentro = Vec::new();
                for k in 1..=n + 1 {
                    let fuera = if k <= n { disco(k) } else { poligono.to_vec() };
                    if fuera.len() >= 3 {
                        let puntos = if dentro.len() >= 3 {
                            crate::pintado::anillo_con_huecos_pub(
                                &fuera,
                                std::slice::from_ref(&dentro),
                            )
                        } else {
                            fuera.clone()
                        };
                        salida.push(Orden::Relleno {
                            puntos,
                            color: color(((k as f32 - 0.5) / n as f32).min(1.0)),
                        });
                    }
                    dentro = fuera;
                }
            }
        }
    }
    salida
}

/// Sutherland–Hodgman contra un semiplano: se queda con lo que tiene
/// `f(p) >= 0` (`f` lineal). Vale para poligonos no convexos: lo que sobra son
/// aristas de ida y vuelta sobre el corte, que no rellenan nada.
pub fn recortar_semiplano(poligono: &[Punto2], f: impl Fn(Punto2) -> f32) -> Vec<Punto2> {
    let n = poligono.len();
    let mut salida = Vec::with_capacity(n + 4);
    for i in 0..n {
        let a = poligono[i];
        let b = poligono[(i + 1) % n];
        let (fa, fb) = (f(a), f(b));
        if fa >= 0.0 {
            salida.push(a);
        }
        if (fa >= 0.0) != (fb >= 0.0) {
            let t = fa / (fa - fb);
            salida.push(a.hacia(b, t));
        }
    }
    salida
}

/// Lados del poligono con el que se aproxima un circulo al recortar.
const LADOS_DISCO: usize = 96;

/// El poligono recortado al circulo `(c, r)` (aproximado por fuera con un
/// poligono de [`LADOS_DISCO`] lados, para que no se coma el borde).
pub fn recortar_a_disco(poligono: &[Punto2], c: Punto2, r: f32) -> Vec<Punto2> {
    if r <= 0.0 {
        return Vec::new();
    }
    // Si el circulo se traga el poligono entero, no hay nada que cortar.
    if poligono.iter().all(|p| p.distancia(c) <= r) {
        return poligono.to_vec();
    }
    let mut trozo = poligono.to_vec();
    for k in 0..LADOS_DISCO {
        let a = TAU * k as f32 / LADOS_DISCO as f32;
        let normal = Punto2::nuevo(a.cos(), a.sin());
        // Lado tangente al circulo: se queda lo que esta a menos de `r` en
        // esa direccion.
        trozo = recortar_semiplano(&trozo, |p| r - p.restar(c).producto(normal));
        if trozo.len() < 3 {
            return Vec::new();
        }
    }
    trozo
}

/// **El trazo raya-punto**, partido en tramos lisos: una raya, un hueco, un
/// punto, un hueco. Se parte aqui y no en quien pinta para que los cinco que
/// pintan (pantalla, PNG, SVG, PDF, pin) lo hagan igual sin saber de el.
pub fn partir_raya_punto(puntos: &[Punto2], grosor: f32) -> Vec<Vec<Punto2>> {
    let g = grosor.max(1.0);
    let patron = [
        (true, 5.0 * g + 4.0),
        (false, 2.0 * g + 2.0),
        (true, g),
        (false, 2.0 * g + 2.0),
    ];
    let mut salida = Vec::new();
    let mut fase = 0usize;
    let mut queda = patron[0].1;
    let mut actual: Vec<Punto2> = Vec::new();
    for par in puntos.windows(2) {
        let (mut a, b) = (par[0], par[1]);
        let mut largo = a.distancia(b);
        while largo > 1e-6 {
            let paso = queda.min(largo);
            let siguiente = a.hacia(b, paso / largo);
            if patron[fase].0 {
                if actual.is_empty() {
                    actual.push(a);
                }
                actual.push(siguiente);
            }
            queda -= paso;
            largo -= paso;
            a = siguiente;
            if queda <= 1e-6 {
                if patron[fase].0 && actual.len() >= 2 {
                    salida.push(std::mem::take(&mut actual));
                }
                actual.clear();
                fase = (fase + 1) % patron.len();
                queda = patron[fase].1;
            }
        }
    }
    if actual.len() >= 2 {
        salida.push(actual);
    }
    salida
}

/// Cambia cada polilinea raya-punto de `ordenes` por sus tramos lisos.
pub(crate) fn desplegar_raya_punto(ordenes: &mut Vec<Orden>) {
    if !ordenes.iter().any(|o| {
        matches!(
            o,
            Orden::Polilinea {
                estilo: EstiloTrazo::RayaPunto,
                ..
            }
        )
    }) {
        return;
    }
    let viejas = std::mem::take(ordenes);
    for o in viejas {
        match o {
            Orden::Polilinea {
                puntos,
                color,
                grosor,
                estilo: EstiloTrazo::RayaPunto,
            } => {
                for tramo in partir_raya_punto(&puntos, grosor) {
                    ordenes.push(Orden::Polilinea {
                        puntos: tramo,
                        color,
                        grosor,
                        estilo: EstiloTrazo::Solido,
                    });
                }
            }
            otra => ordenes.push(otra),
        }
    }
}

/// **Extiende un arco por su circunferencia** desde la punta mas cercana a
/// `p` hasta lo primero que toca de `otros` (sin pasar de la vuelta entera).
/// `None` si no topa con nada.
pub fn extender_arco(e: &Elemento, otros: &[Elemento], p: Punto2) -> Option<Elemento> {
    let (c, r) = centro_y_radio(e)?;
    let Figura::Arco {
        inicio,
        barrido: Some(barrido),
    } = e.figura
    else {
        return None;
    };
    let giro = e.angulo;
    let mundo = |t: f32| en_la_circunferencia(c, r, t + giro);
    let fin = inicio + barrido;
    let por_inicio = p.distancia(mundo(inicio)) < p.distancia(mundo(fin));
    let s = if barrido >= 0.0 { 1.0 } else { -1.0 };
    // Hacia fuera: desde el fin, en el sentido del barrido; desde el inicio,
    // al reves.
    let (desde, sentido) = if por_inicio { (inicio, -s) } else { (fin, s) };
    let resto = TAU - barrido.abs();
    if resto < 1e-3 {
        return None;
    }
    let paredes: Vec<(Punto2, Punto2)> = otros
        .iter()
        .filter(|o| !o.borrado && o.id != e.id)
        .flat_map(|o| crate::perimetros::segmentos_de(o, crate::perimetros::PASO_PERIMETRO))
        .collect();
    let paso = 0.5_f32.to_radians();
    let pasos = (resto / paso).ceil() as usize;
    let mut anterior = mundo(desde);
    for k in 1..=pasos {
        let t = desde + sentido * (k as f32 * paso).min(resto);
        let q = mundo(t);
        let mut mejor: Option<(f32, Punto2)> = None;
        for (a, b) in &paredes {
            if let Some(x) = cruce_de_segmentos(anterior, q, *a, *b) {
                let d = x.distancia(anterior);
                // El cruce justo en la punta (lo que ya toca) no cuenta.
                if (k > 1 || d > 0.5) && mejor.is_none_or(|m| d < m.0) {
                    mejor = Some((d, x));
                }
            }
        }
        if let Some((_, x)) = mejor {
            let ang = angulo_de(c, x) - giro;
            let extra = if sentido > 0.0 {
                a_una_vuelta(ang - desde)
            } else {
                a_una_vuelta(desde - ang)
            }
            .min(resto);
            let mut nuevo = e.clone();
            let (i2, b2) = if por_inicio {
                (inicio - s * extra, barrido + s * extra)
            } else {
                (inicio, barrido + s * extra)
            };
            nuevo.figura = Figura::Arco {
                inicio: i2,
                barrido: Some(b2),
            };
            nuevo.tocar();
            return Some(nuevo);
        }
        anterior = q;
    }
    None
}

// ---------------------------------------------------------------------------
// El fichero del movil (Excalidraw): las claves nuevas y su reserva

/// `cierreArco`: `"sector"` o `"segmento"`.
pub fn cierre_desde(v: &serde_json::Value) -> Option<CierreArco> {
    v.get("cierreArco")?
        .as_str()
        .and_then(CierreArco::desde_palabra)
}

/// `marcaAngulo`: `{"arcos": 2, "recto": false, "valor": "45°"}`.
pub fn marca_desde(v: &serde_json::Value) -> Option<MarcaAngulo> {
    let m = v.get("marcaAngulo")?;
    if !m.is_object() {
        return None;
    }
    Some(MarcaAngulo {
        arcos: m
            .get("arcos")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1)
            .clamp(1, 3) as u8,
        recto: m
            .get("recto")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        valor: m
            .get("valor")
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
    })
}

fn par(v: Option<&serde_json::Value>) -> Option<(f32, f32)> {
    let a = v?.as_array()?;
    Some((a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32))
}

/// `degradado`: `{"tipo": "radial", "desde": "#rrggbb", "hasta": "#rrggbb",
/// "origen": [0.5, 0.5], "destino": [1, 1]}` (fracciones de la caja).
pub fn degradado_desde(v: &serde_json::Value) -> Option<Degradado> {
    let d = v.get("degradado")?;
    let tipo = TipoDegradado::desde_palabra(d.get("tipo")?.as_str()?)?;
    let desde = crate::excalidraw::color_desde(d.get("desde"))?;
    let hasta = crate::excalidraw::color_desde(d.get("hasta"))?;
    let base = match tipo {
        TipoDegradado::Radial => Degradado::radial(desde, hasta),
        TipoDegradado::Lineal => Degradado::lineal(desde, hasta),
    };
    Some(Degradado {
        origen: par(d.get("origen")).unwrap_or(base.origen),
        destino: par(d.get("destino")).unwrap_or(base.destino),
        ..base
    })
}

/// **Las claves de las laminas, de vuelta al JSON.** Las que hay se escriben;
/// las que no, se quitan **solo si las entendiamos**: una de una version
/// futura del movil (un cierre que aqui no se conoce) vuelve intacta, la
/// misma regla que la `dureza` y el `material`.
pub fn laminas_hacia(mapa: &mut serde_json::Map<String, serde_json::Value>, e: &Elemento) {
    use serde_json::{Value, json};
    let original = Value::Object(mapa.clone());
    match e.extras.cierre {
        Some(c) => {
            mapa.insert("cierreArco".into(), Value::String(c.palabra().into()));
        }
        None if cierre_desde(&original).is_some() => {
            mapa.remove("cierreArco");
        }
        None => {}
    }
    match &e.extras.marca_angulo {
        Some(m) => {
            let mut o = json!({ "arcos": m.arcos, "recto": m.recto });
            if let Some(v) = &m.valor {
                o["valor"] = Value::String(v.clone());
            }
            mapa.insert("marcaAngulo".into(), o);
        }
        None if marca_desde(&original).is_some() => {
            mapa.remove("marcaAngulo");
        }
        None => {}
    }
    match &e.extras.degradado {
        Some(d) => {
            mapa.insert(
                "degradado".into(),
                json!({
                    "tipo": d.tipo.palabra(),
                    "desde": crate::excalidraw::color_hacia(d.desde),
                    "hasta": crate::excalidraw::color_hacia(d.hasta),
                    "origen": [d.origen.0, d.origen.1],
                    "destino": [d.destino.0, d.destino.1],
                }),
            );
            // **La reserva del movil**: sin degradado, el que no lo entienda
            // rellena liso con su `backgroundColor`. Que sea el del centro
            // (o el de salida), no transparente.
            let sin_fondo = mapa
                .get("backgroundColor")
                .and_then(Value::as_str)
                .is_none_or(|s| s == "transparent");
            if sin_fondo {
                mapa.insert(
                    "backgroundColor".into(),
                    Value::String(crate::excalidraw::color_hacia(d.desde)),
                );
                mapa.insert("fillStyle".into(), Value::String("solid".into()));
            }
        }
        None if degradado_desde(&original).is_some() => {
            mapa.remove("degradado");
        }
        None => {}
    }
    // El raya-punto: `strokeStyle` ya dice `dashed`; la verdad, aqui.
    if e.estilo == EstiloTrazo::RayaPunto {
        mapa.insert("trazoRayaPunto".into(), Value::Bool(true));
    } else {
        mapa.remove("trazoRayaPunto");
    }
    // El tamano del valor de una marca de angulo (el del punto lo escribe su
    // rama).
    if matches!(e.figura, Figura::Arco { .. })
        && let Some(t) = e.extras.tam_letra
    {
        mapa.insert("fontSize".into(), Value::from(t as f64));
    }
}

// ---------------------------------------------------------------------------
// La herramienta «Angulo»: donde se cruzan dos rayas cerca del clic

/// Los tramos rectos de las figuras de rayas (linea, flecha, cota y los lados
/// de rectangulos y rombos), con el id de quien son.
fn tramos_rectos(elementos: &[Elemento]) -> Vec<(Punto2, Punto2, u64)> {
    let mut v = Vec::new();
    for e in elementos.iter().filter(|e| !e.borrado) {
        match &e.figura {
            Figura::Linea { puntos } | Figura::Flecha { puntos, .. } | Figura::Cota { puntos } => {
                for par in puntos.windows(2) {
                    if par[0].distancia(par[1]) > 0.5 {
                        v.push((par[0], par[1], e.id));
                    }
                }
            }
            Figura::Rectangulo | Figura::Rombo => {
                for (a, b) in crate::perimetros::segmentos_de(e, crate::perimetros::PASO_PERIMETRO)
                {
                    v.push((a, b, e.id));
                }
            }
            _ => {}
        }
    }
    v
}

/// Donde se cortan los segmentos `a1b1` y `a2b2` (`None` si no se tocan).
pub fn cruce_de_segmentos(a1: Punto2, b1: Punto2, a2: Punto2, b2: Punto2) -> Option<Punto2> {
    let d1 = b1.restar(a1);
    let d2 = b2.restar(a2);
    let den = d1.x * d2.y - d1.y * d2.x;
    if den.abs() < 1e-6 {
        return None;
    }
    let w = a2.restar(a1);
    let t = (w.x * d2.y - w.y * d2.x) / den;
    let u = (w.x * d1.y - w.y * d1.x) / den;
    let tol = 1e-3;
    ((-tol..=1.0 + tol).contains(&t) && (-tol..=1.0 + tol).contains(&u))
        .then(|| a1.sumar(d1.escalar(t)))
}

/// **El angulo que se quiere marcar con un clic en `p`**: el vertice mas
/// cercano donde se juntan dos rayas (a menos de `alcance`), y los dos lados
/// que encierran el clic. Devuelve `(vertice, punto del lado 1, punto del
/// lado 2)`, listo para [`marca_de_angulo`].
pub fn angulo_en(
    elementos: &[Elemento],
    p: Punto2,
    alcance: f32,
) -> Option<(Punto2, Punto2, Punto2)> {
    let tramos = tramos_rectos(elementos);
    let mut mejor: Option<(f32, Punto2, usize, usize)> = None;
    for i in 0..tramos.len() {
        for j in i + 1..tramos.len() {
            let (a1, b1, _) = tramos[i];
            let (a2, b2, _) = tramos[j];
            let Some(v) = cruce_de_segmentos(a1, b1, a2, b2) else {
                continue;
            };
            let d = v.distancia(p);
            if d <= alcance && mejor.is_none_or(|m| d < m.0) {
                mejor = Some((d, v, i, j));
            }
        }
    }
    let (_, v, _, _) = mejor?;
    // Los rayos que salen del vertice por **todos** los tramos que pasan por
    // el (no solo los dos que dieron el cruce): en un vertice de donde salen
    // cuatro rectas, el clic cae entre las dos vecinas, no entre las dos
    // primeras que se encontraron. Uno o dos por tramo (dos si el vertice
    // cae en medio).
    let mut rayos: Vec<Punto2> = Vec::new();
    for &(a, b, _) in tramos
        .iter()
        .filter(|(a, b, _)| crate::vector::distancia_a_segmento(v, *a, *b) <= 0.75)
    {
        for extremo in [a, b] {
            if extremo.distancia(v) > 1.0 {
                rayos.push(extremo);
            }
        }
    }
    if rayos.len() < 2 {
        return None;
    }
    // El sector que contiene el clic: entre el rayo inmediatamente antes y el
    // inmediatamente despues del angulo del clic.
    let ac = angulo_de(v, p);
    let mut antes: Option<(f32, Punto2)> = None;
    let mut despues: Option<(f32, Punto2)> = None;
    for r in &rayos {
        let d = a_una_vuelta(angulo_de(v, *r) - ac);
        if despues.is_none_or(|m| d < m.0) {
            despues = Some((d, *r));
        }
        let d2 = a_una_vuelta(ac - angulo_de(v, *r));
        if antes.is_none_or(|m| d2 < m.0) {
            antes = Some((d2, *r));
        }
    }
    let (p1, p2) = (antes?.1, despues?.1);
    if angulo_de(v, p1) == angulo_de(v, p2) {
        return None;
    }
    Some((v, p1, p2))
}

/// La marca que pone la herramienta Angulo en `v` entre `p1` y `p2`: el
/// cuadradito si es recto (a medio grado), y si no los arquitos pedidos.
pub fn marca_para(v: Punto2, p1: Punto2, p2: Punto2, r: f32, arcos: u8) -> Elemento {
    let (_, barrido) = angulo_menor(v, p1, p2);
    if (barrido.abs() - PI / 2.0).abs() < 0.5_f32.to_radians() {
        marca_de_angulo_recto(v, p1, p2, r * 0.6)
    } else {
        marca_de_angulo(v, p1, p2, r, arcos)
    }
}

// ---------------------------------------------------------------------------
// Los angulos con su letra griega (17-oct-2026)
//
// El usuario: «poner los angulos con letras griegas de forma rapida, como la
// funcion de los puntos, pero para poner rapidamente los angulos en elementos
// geometricos; ademas el de 90 grados con su cuadradito». Es la misma
// herramienta Angulo con una variable mas ([`ModoAngulo`]), no otra: la marca
// sigue siendo un `MarcaAngulo` y la letra va en su `valor`, como cuando la
// pone el asistente.

/// **Que pone la herramienta Angulo.**
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModoAngulo {
    /// La marca con la siguiente letra griega libre (α, β, θ...), como el
    /// punto pone A, B, C. El recto, con su cuadradito y sin letra.
    #[default]
    Griega,
    /// La marca sola, sin letra (lo de antes).
    Simple,
    /// Siempre el cuadradito del angulo recto, aunque el dibujo a mano no
    /// mida 90 justos.
    Recto,
}

/// **Las letras de los angulos, en el orden de los libros de colegio**: α, β,
/// θ, γ y ω primero (las que pidio el usuario, en su orden), y luego las que
/// se usan tambien para angulos.
pub const LETRAS_DE_ANGULO: [&str; 16] = [
    "α", "β", "θ", "γ", "ω", "δ", "φ", "ε", "λ", "μ", "ρ", "σ", "τ", "ψ", "η", "κ",
];

/// La letra numero `i` de la serie: pasada la ultima sigue con α1, β1... como
/// los puntos siguen con A1 tras la Z.
pub fn letra_de_angulo(i: usize) -> String {
    let n = LETRAS_DE_ANGULO.len();
    let letra = LETRAS_DE_ANGULO[i % n];
    match i / n {
        0 => letra.to_string(),
        vuelta => format!("{letra}{vuelta}"),
    }
}

/// Las letras que ya llevan las marcas de angulo del dibujo (sin `excepto`).
pub fn letras_de_angulo_usadas(elementos: &[Elemento], excepto: Option<u64>) -> Vec<String> {
    elementos
        .iter()
        .filter(|e| !e.borrado && Some(e.id) != excepto)
        .filter_map(|e| e.extras.marca_angulo.as_ref()?.valor.clone())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .collect()
}

/// **La siguiente letra libre**, mirando el dibujo y no un contador (lo
/// mismo que `puntos_etiquetados::siguiente_etiqueta`): borrar la marca β
/// deja libre la β para la siguiente.
pub fn siguiente_letra_de_angulo(usadas: &[String]) -> String {
    (0..10_000)
        .map(letra_de_angulo)
        .find(|l| !usadas.iter().any(|u| u == l))
        .unwrap_or_else(|| LETRAS_DE_ANGULO[0].to_string())
}

/// **La letra de al lado** (`paso` positivo, la siguiente; negativo, la
/// anterior) de la serie, saltando las que ya usan otras marcas: es lo que
/// hacen la rueda y las flechas sobre la marca recien puesta. Sin letra (o
/// con una que no es de la serie), la primera libre.
pub fn letra_vecina(actual: Option<&str>, paso: i32, usadas: &[String]) -> String {
    let n = LETRAS_DE_ANGULO.len() as i32;
    let Some(i) = actual.and_then(|a| LETRAS_DE_ANGULO.iter().position(|l| *l == a)) else {
        return siguiente_letra_de_angulo(usadas);
    };
    let paso = if paso >= 0 { 1 } else { -1 };
    let mut j = i as i32;
    for _ in 0..n {
        j = (j + paso).rem_euclid(n);
        let l = LETRAS_DE_ANGULO[j as usize];
        if !usadas.iter().any(|u| u == l) {
            return l.to_string();
        }
    }
    LETRAS_DE_ANGULO[i].to_string()
}

/// **La marca que pone la herramienta Angulo en `modo`**: la de siempre
/// ([`marca_para`]), el cuadradito a la fuerza o, con letra, la siguiente
/// griega libre de `usadas` (el recto no la lleva: el cuadradito ya lo dice).
pub fn marca_con_modo(
    modo: ModoAngulo,
    v: Punto2,
    p1: Punto2,
    p2: Punto2,
    r: f32,
    arcos: u8,
    usadas: &[String],
) -> Elemento {
    let mut e = match modo {
        ModoAngulo::Recto => marca_de_angulo_recto(v, p1, p2, r * 0.6),
        ModoAngulo::Griega | ModoAngulo::Simple => marca_para(v, p1, p2, r, arcos),
    };
    if modo == ModoAngulo::Griega
        && let Some(m) = e.extras.marca_angulo.as_mut()
        && !m.recto
    {
        m.valor = Some(siguiente_letra_de_angulo(usadas));
    }
    e
}

#[cfg(test)]
mod pruebas;
