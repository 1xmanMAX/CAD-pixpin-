//! **La lupa y el foco como elementos del dibujo**, con los nueve campos que
//! el PC guardaba sin entender.
//!
//! Ojo con el nombre, porque hay dos cosas distintas que se llaman lupa: la
//! del PC (`Herramienta::Lupa`, `gesto.rs`) es una **vista** que agranda
//! alrededor del cursor y no deja rastro; la del movil es un **elemento del
//! dibujo** —dos recuadros: el cristal, que es la caja del elemento, y el
//! foco, que es a donde mira—. Este modulo es la segunda, y por eso se llama
//! `lupa_elemento` y no `lupa`.
//!
//! El foco (`pixpin-spotlight`) comparte con ella casi todo: la misma caja, el
//! mismo `focoAncho`/`focoAlto`, la misma forma. Lo que cambia es que el foco
//! no agranda nada, **oscurece el resto**. Por eso van juntos aqui: son la
//! misma geometria con dos finales.
//!
//! # Lo que aqui se decide y lo que no
//!
//! Se decide toda la geometria —a donde mira, que recuadro recoge, cuanto
//! agranda, cuanto oscurece— y la ida y vuelta al JSON del movil. **No** se
//! pinta el contenido ampliado: para eso hacen falta los pixeles de debajo,
//! que el motor no tiene (lo mismo que el mosaico, ver `mosaico.rs`). Quien
//! pinta pide `region` y `cristal` y copia de uno a otro.
//!
//! Todos los numeros salen de `Lupa.kt` de la v0.72.0, no de una estimacion.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::vector::Punto2;

/// Lo menos que agranda una lupa: nada.
pub const AUMENTO_MINIMO: f32 = 1.0;
/// Y lo mas. Mas alla se ve el grano de la pantalla, no el detalle.
pub const AUMENTO_MAXIMO: f32 = 12.0;
/// Con cuanto nace: el doble, que es lo que se pide sin pensarlo.
pub const AUMENTO_POR_DEFECTO: f32 = 2.0;

/// Lo menos que puede oscurecer un foco, en por ciento.
pub const OSCURECER_MINIMO: u8 = 10;
/// Y lo mas: por encima de esto lo de alrededor deja de verse.
pub const OSCURECER_MAXIMO: u8 = 90;
/// Con cuanto nace un foco: se nota sin esconder el contexto.
///
/// **No es el 60 % que usa hoy el pintado del PC.** Un foco del movil abierto
/// aqui oscurecia de mas; con esto, los dos aparatos apagan lo mismo.
pub const OSCURECER_POR_DEFECTO: u8 = 45;

/// Cuanto sobresale el marco de un foco recien puesto, respecto a su figura.
pub const MARCO_DEL_FOCO: f32 = 1.8;
/// Lo menos que puede ocupar la zona iluminada dentro del marco.
pub const ZONA_MINIMA: f32 = 0.15;
/// Y lo mas: pegada al marco no queda anillo que oscurecer.
pub const ZONA_MAXIMA: f32 = 0.92;

/// Con que se senala de donde sale lo que se ve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum GuiaDeLupa {
    #[default]
    Ninguna,
    /// Una flecha del detalle al cristal. Ocupa poco y se lee de lejos.
    Flecha,
    /// El cono de toda la vida en un plano: dos rayas que salen de los
    /// costados del detalle y se abren hasta los del cristal. Dice ademas
    /// **cuanto** se ha ampliado, porque la apertura se ve.
    DosLineas,
    /// Un punto en el sitio y una raya hasta la ventana: para senalar **un
    /// sitio y no una zona**, cuando lo mirado es una mota.
    Punto,
}

impl GuiaDeLupa {
    /// Si este modo dibuja ademas el contorno de la zona mirada.
    pub fn dibuja_la_zona(self) -> bool {
        matches!(self, GuiaDeLupa::Flecha | GuiaDeLupa::DosLineas)
    }

    pub fn palabra(self) -> &'static str {
        match self {
            GuiaDeLupa::Ninguna => "NINGUNA",
            GuiaDeLupa::Flecha => "FLECHA",
            GuiaDeLupa::DosLineas => "DOS_LINEAS",
            GuiaDeLupa::Punto => "PUNTO",
        }
    }

    pub fn desde_palabra(p: &str) -> Option<GuiaDeLupa> {
        Some(match p {
            "NINGUNA" => GuiaDeLupa::Ninguna,
            "FLECHA" => GuiaDeLupa::Flecha,
            "DOS_LINEAS" => GuiaDeLupa::DosLineas,
            "PUNTO" => GuiaDeLupa::Punto,
            _ => return None,
        })
    }
}

/// Los campos propios de una lupa o de un foco.
///
/// Van en un struct aparte y no sueltos en `Elemento` por lo mismo que
/// `Extras`: cada campo suelto obliga a tocar los ocho sitios del proyecto que
/// construyen un `Elemento` entero a mano.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Cristal {
    /// A donde mira, **en coordenadas absolutas del dibujo**. Es lo que
    /// permite apartar el resultado sin mover lo que se esta mirando.
    /// `None` = mira a su propio centro, como una lupa dejada sobre el papel.
    #[serde(default)]
    pub foco: Option<Punto2>,
    /// Cuanto agranda, si se fijo a mano. Ver [`aumento_de`], que prefiere la
    /// geometria.
    #[serde(default)]
    pub aumento: Option<f32>,
    /// Lo que mide la zona mirada. **Se guarda, no se calcula**: es lo que no
    /// se mueve por mucho que crezca el cristal.
    #[serde(default)]
    pub foco_ancho: Option<f32>,
    #[serde(default)]
    pub foco_alto: Option<f32>,
    /// Cuanto oscurece un foco, en por ciento (10-90).
    #[serde(default)]
    pub oscurecer: Option<u8>,
    /// El cristal es redondo en vez de rectangular.
    #[serde(default)]
    pub redonda: bool,
    #[serde(default)]
    pub guia: GuiaDeLupa,
    /// El contorno del cristal **en proporcion de su caja** (0 a 1). Es lo que
    /// permite que una lupa tenga la forma de cualquier figura cerrada que se
    /// toque con la varita, y aguanta estirarla y girarla sin deformarse.
    #[serde(default)]
    pub forma: Option<Vec<Punto2>>,
}

/// Una caja `(x, y, ancho, alto)` del documento.
pub type Caja = (f32, f32, f32, f32);

fn centro(c: Caja) -> Punto2 {
    Punto2::nuevo(c.0 + c.2 / 2.0, c.1 + c.3 / 2.0)
}

/// **Cuanto agranda esta lupa de verdad.**
///
/// Sale de la geometria —el cristal partido por el foco— y no del numero
/// guardado, porque el cristal se estira con los tiradores: un numero aparte
/// se quedaria diciendo x2 mientras se esta viendo x5.
pub fn aumento_de(cr: &Cristal, caja: Caja) -> f32 {
    match cr.foco_ancho {
        Some(a) if a > 0.0 => (caja.2.abs() / a).clamp(AUMENTO_MINIMO, AUMENTO_MAXIMO),
        _ => cr
            .aumento
            .unwrap_or(AUMENTO_POR_DEFECTO)
            .clamp(AUMENTO_MINIMO, AUMENTO_MAXIMO),
    }
}

/// A donde mira. Sin foco puesto, a su propio centro.
pub fn foco_de(cr: &Cristal, caja: Caja) -> Punto2 {
    cr.foco.unwrap_or_else(|| centro(caja))
}

/// **El recuadro que se recoge**, centrado en el foco, en coordenadas del
/// documento y como `(x, y, ancho, alto)`.
///
/// Mide lo guardado; y solo si no lo lleva —una lupa de antes— se deduce del
/// cristal partido por el aumento, que era la forma vieja de calcularlo.
pub fn region(cr: &Cristal, caja: Caja) -> Caja {
    let z = cr.aumento.unwrap_or(AUMENTO_POR_DEFECTO).max(f32::EPSILON);
    let ancho = cr.foco_ancho.unwrap_or(caja.2.abs() / z);
    let alto = cr.foco_alto.unwrap_or(caja.3.abs() / z);
    let f = foco_de(cr, caja);
    (f.x - ancho / 2.0, f.y - alto / 2.0, ancho, alto)
}

/// Si el punto cae en la zona mirada, con margen.
///
/// Se pica **el recuadro entero** y no solo su centro: en una lupa de mucho
/// aumento la zona es diminuta y acertarle a un punto seria imposible.
pub fn toca_el_foco(cr: &Cristal, caja: Caja, p: Punto2, margen: f32) -> bool {
    let (x, y, ancho, alto) = region(cr, caja);
    // Con forma propia, y tambien con cristal rectangular, se pica la caja: en
    // un contorno raro y diminuto exigir acertar dentro del garabato seria no
    // poder cogerlo.
    if cr.forma.is_some() || !cr.redonda {
        return p.x >= x - margen
            && p.x <= x + ancho + margen
            && p.y >= y - margen
            && p.y <= y + alto + margen;
    }
    // Redondo se pica el ovalo: con la caja, las cuatro esquinas respondian a
    // un sitio donde no hay foco dibujado.
    let (rx, ry) = (ancho / 2.0 + margen, alto / 2.0 + margen);
    if rx <= 0.0 || ry <= 0.0 {
        return false;
    }
    let dx = (p.x - (x + ancho / 2.0)) / rx;
    let dy = (p.y - (y + alto / 2.0)) / ry;
    dx * dx + dy * dy <= 1.0
}

/// Cuanto oscurece este foco, en por ciento y siempre dentro de lo razonable.
pub fn oscurecimiento_de(cr: &Cristal) -> u8 {
    cr.oscurecer
        .unwrap_or(OSCURECER_POR_DEFECTO)
        .clamp(OSCURECER_MINIMO, OSCURECER_MAXIMO)
}

/// Que parte del marco ocupa ahora la zona iluminada de un foco, de 0 a 1.
pub fn zona_de(cr: &Cristal, caja: Caja) -> f32 {
    let ancho = caja.2.abs();
    if ancho <= 0.0 {
        return 1.0 / MARCO_DEL_FOCO;
    }
    match cr.foco_ancho {
        Some(s) => (s / ancho).clamp(ZONA_MINIMA, ZONA_MAXIMA),
        None => 1.0 / MARCO_DEL_FOCO,
    }
}

/// El cristal que le toca a una lupa para agrandar `z` veces, y la caja nueva.
///
/// **Mueve la ventana, no la zona mirada**, y crece desde su propio centro
/// para que subir el aumento no la desplace de donde se habia dejado.
pub fn con_aumento(cr: &Cristal, caja: Caja, z: f32) -> (Cristal, Caja) {
    let (_, _, ra, rh) = region(cr, caja);
    if ra <= 0.0 || rh <= 0.0 {
        return (cr.clone(), caja);
    }
    let z = z.clamp(AUMENTO_MINIMO, AUMENTO_MAXIMO);
    let c = centro(caja);
    let (w, h) = (ra * z, rh * z);
    (
        Cristal {
            aumento: Some(z),
            // Se fija el tamano del foco al primer ajuste: hasta entonces
            // podia venir del cristal, y cambiar el cristal lo habria
            // cambiado tambien.
            foco_ancho: Some(ra),
            foco_alto: Some(rh),
            ..cr.clone()
        },
        (c.x - w / 2.0, c.y - h / 2.0, w, h),
    )
}

/// El foco con su zona iluminada mas grande o mas pequena, **dentro del mismo
/// marco**.
///
/// Es lo contrario de [`con_aumento`], y con motivo: en una lupa lo que se
/// ajusta es cuanto agranda, asi que crece la ventana; en un foco lo que se
/// ajusta es cuanto ilumina, con la sombra llegando siempre hasta el marco.
/// Cambiar el marco aqui moveria el borde de la sombra, que es lo unico que el
/// usuario habia colocado a mano.
pub fn con_zona(cr: &Cristal, caja: Caja, parte: f32) -> Cristal {
    let (ancho, alto) = (caja.2.abs(), caja.3.abs());
    if ancho <= 0.0 || alto <= 0.0 {
        return cr.clone();
    }
    let p = parte.clamp(ZONA_MINIMA, ZONA_MAXIMA);
    Cristal {
        foco_ancho: Some(ancho * p),
        foco_alto: Some(alto * p),
        ..cr.clone()
    }
}

/// El contorno del cristal en coordenadas del documento, si lleva forma
/// propia. `None` = el cristal de siempre, recuadro u ovalo segun `redonda`.
pub fn contorno_del_cristal(cr: &Cristal, caja: Caja) -> Option<Vec<Punto2>> {
    let forma = cr.forma.as_ref()?;
    if forma.len() < 3 {
        return None;
    }
    Some(
        forma
            .iter()
            .map(|p| Punto2::nuevo(caja.0 + p.x * caja.2, caja.1 + p.y * caja.3))
            .collect(),
    )
}

// ---------------------------------------------------------------------------
// El puente
// ---------------------------------------------------------------------------

fn num(v: &Value, clave: &str) -> Option<f32> {
    v.get(clave).and_then(Value::as_f64).map(|n| n as f32)
}

/// Lee los campos propios de una lupa o un foco del JSON del movil.
///
/// Lo que no venga se queda en `None`, que **no es lo mismo que un cero**:
/// `foco` ausente significa «mira a su centro» y `focoAncho` ausente significa
/// «deducelo del cristal». Machacarlos con ceros convertiria una lupa en una
/// ventana que no mira a ninguna parte.
pub fn leer(v: &Value) -> Cristal {
    Cristal {
        foco: match (
            v.get("foco").and_then(|f| num(f, "x")),
            v.get("foco").and_then(|f| num(f, "y")),
        ) {
            (Some(x), Some(y)) => Some(Punto2::nuevo(x, y)),
            _ => None,
        },
        aumento: num(v, "aumento"),
        foco_ancho: num(v, "focoAncho"),
        foco_alto: num(v, "focoAlto"),
        oscurecer: v
            .get("oscurecer")
            .and_then(Value::as_i64)
            .map(|n| n.clamp(0, 255) as u8),
        redonda: v.get("lupaRedonda").and_then(Value::as_bool) == Some(true),
        guia: v
            .get("guia")
            .and_then(Value::as_str)
            .and_then(GuiaDeLupa::desde_palabra)
            // Las lupas de antes no llevan campo: se traduce lo que tenian.
            .unwrap_or(
                if v.get("lupaFlecha").and_then(Value::as_bool) == Some(true) {
                    GuiaDeLupa::Flecha
                } else if v.get("lupaDosLineas").and_then(Value::as_bool) == Some(true) {
                    GuiaDeLupa::DosLineas
                } else {
                    GuiaDeLupa::Ninguna
                },
            ),
        forma: v.get("forma").and_then(Value::as_array).map(|a| {
            a.iter()
                .filter_map(|p| Some(Punto2::nuevo(num(p, "x")?, num(p, "y")?)))
                .collect()
        }),
    }
}

/// Escribe los campos propios sobre el mapa del elemento.
///
/// **Lo que vale `None` se borra en vez de escribirse a cero**, por lo mismo
/// que en `leer`. Y `lupaFlecha`/`lupaDosLineas` se reescriben a juego con
/// `guia` para que una lupa editada aqui se abra igual en un movil viejo.
pub fn escribir(mapa: &mut Map<String, Value>, cr: &Cristal) {
    fn poner(mapa: &mut Map<String, Value>, clave: &str, v: Option<f64>) {
        match v {
            Some(n) => {
                mapa.insert(clave.into(), Value::from(n));
            }
            None => {
                mapa.remove(clave);
            }
        }
    }
    match cr.foco {
        Some(p) => {
            let mut m = Map::new();
            m.insert("x".into(), Value::from(p.x as f64));
            m.insert("y".into(), Value::from(p.y as f64));
            mapa.insert("foco".into(), Value::Object(m));
        }
        None => {
            mapa.remove("foco");
        }
    }
    poner(mapa, "aumento", cr.aumento.map(|n| n as f64));
    poner(mapa, "focoAncho", cr.foco_ancho.map(|n| n as f64));
    poner(mapa, "focoAlto", cr.foco_alto.map(|n| n as f64));
    match cr.oscurecer {
        Some(n) => {
            mapa.insert("oscurecer".into(), Value::from(n));
        }
        None => {
            mapa.remove("oscurecer");
        }
    }
    mapa.insert("lupaRedonda".into(), Value::Bool(cr.redonda));
    mapa.insert("guia".into(), Value::String(cr.guia.palabra().to_string()));
    mapa.insert(
        "lupaFlecha".into(),
        Value::Bool(cr.guia == GuiaDeLupa::Flecha),
    );
    mapa.insert(
        "lupaDosLineas".into(),
        Value::Bool(cr.guia == GuiaDeLupa::DosLineas),
    );
    match &cr.forma {
        Some(f) => {
            let puntos: Vec<Value> = f
                .iter()
                .map(|p| {
                    let mut m = Map::new();
                    m.insert("x".into(), Value::from(p.x as f64));
                    m.insert("y".into(), Value::from(p.y as f64));
                    Value::Object(m)
                })
                .collect();
            mapa.insert("forma".into(), Value::Array(puntos));
        }
        None => {
            mapa.remove("forma");
        }
    }
}

// ---------------------------------------------------------------------------
// La lupa en el lienzo (`Lupa.kt`: la varita, el contorno y la guia)
// ---------------------------------------------------------------------------

/// Con cuantos lados se hace el aro de un cristal redondo (`LADOS_DEL_ARO`).
/// A 64 lados, a la vista es un ovalo, y los tres sitios que lo necesitan
/// —pantalla, picado y guia— saben de polilineas y no de curvas.
pub const LADOS_DEL_ARO: usize = 64;

/// Lo menos que puede medir de lado el cristal de una lupa
/// (`LADO_MINIMO_DE_LUPA`): mas pequeno no se ve nada dentro.
pub const LADO_MINIMO: f32 = 60.0;

/// La caja `(x, y, ancho, alto)` de un elemento, en el orden de este modulo.
pub fn caja_de(e: &crate::elemento::Elemento) -> Caja {
    let (x0, y0, x1, y1) = e.caja();
    (x0, y0, x1 - x0, y1 - y0)
}

fn ovalo(c: Punto2, rx: f32, ry: f32) -> Vec<Punto2> {
    (0..LADOS_DEL_ARO)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / LADOS_DEL_ARO as f32;
            Punto2::nuevo(c.x + rx * a.cos(), c.y + ry * a.sin())
        })
        .collect()
}

fn esquinas(c: Caja) -> Vec<Punto2> {
    vec![
        Punto2::nuevo(c.0, c.1),
        Punto2::nuevo(c.0 + c.2, c.1),
        Punto2::nuevo(c.0 + c.2, c.1 + c.3),
        Punto2::nuevo(c.0, c.1 + c.3),
    ]
}

/// Un contorno de proporciones (0-1) puesto en una caja del documento.
fn en_la_caja(forma: &[Punto2], c: Caja) -> Vec<Punto2> {
    forma
        .iter()
        .map(|p| Punto2::nuevo(c.0 + p.x * c.2, c.1 + p.y * c.3))
        .collect()
}

/// **El contorno del cristal**, en el documento (`puntosDelCristal`). La
/// forma copiada de una figura manda sobre todo lo demas; si no, un
/// recuadro o un ovalo segun `redonda`. Es LA lista que usan el pintado, el
/// recorte del contenido y la guia: si cada uno sacara la suya, el recorte
/// no coincidiria con el marco.
pub fn puntos_del_cristal(cr: &Cristal, caja: Caja) -> Vec<Punto2> {
    if let Some(f) = cr.forma.as_ref().filter(|f| f.len() >= 3) {
        return en_la_caja(f, caja);
    }
    if !cr.redonda {
        return esquinas(caja);
    }
    ovalo(centro(caja), caja.2 / 2.0, caja.3 / 2.0)
}

/// **El contorno de lo que se mira**, con la misma forma que el cristal
/// (`puntosDelFoco`): con el cristal redondo lo que entra es un ovalo, y
/// dibujar un recuadro seria ensenar una zona que no es la que se ve.
pub fn puntos_del_foco(cr: &Cristal, caja: Caja) -> Vec<Punto2> {
    let r = region(cr, caja);
    if let Some(f) = cr.forma.as_ref().filter(|f| f.len() >= 3) {
        return en_la_caja(f, r);
    }
    if !cr.redonda {
        return esquinas(r);
    }
    ovalo(centro(r), r.2 / 2.0, r.3 / 2.0)
}

/// **Si la lupa esta apoyada sobre lo que mira** (`laLupaEstaEncima`):
/// mientras lo este no hay nada que senalar —el detalle esta debajo—, asi
/// que ni contorno de la zona ni raya.
pub fn esta_encima(cr: &Cristal, caja: Caja) -> bool {
    let r = region(cr, caja);
    r.0 < caja.0 + caja.2 && r.0 + r.2 > caja.0 && r.1 < caja.1 + caja.3 && r.1 + r.3 > caja.1
}

/// Donde corta el rayo `centro + t·d` al tramo `a-b`.
fn corte_del_rayo(centro: Punto2, dx: f32, dy: f32, a: Punto2, b: Punto2) -> Option<f32> {
    let (ex, ey) = (b.x - a.x, b.y - a.y);
    let den = dx * ey - dy * ex;
    if den.abs() < 1e-9 {
        return None;
    }
    let (cx, cy) = (a.x - centro.x, a.y - centro.y);
    let t = (cx * ey - cy * ex) / den;
    let u = (cx * dy - cy * dx) / den;
    (t >= 0.0 && (0.0..=1.0).contains(&u)).then_some(t)
}

/// **Por donde sale de un contorno un rayo que parte de su centro**
/// (`salidaDelContorno`). El corte mas lejano: en una figura con entrantes
/// el primero dejaria la raya metida dentro de la propia figura.
pub fn salida_del_contorno(centro: Punto2, contorno: &[Punto2], dx: f32, dy: f32) -> Punto2 {
    if contorno.len() < 2 {
        return centro;
    }
    let mut mejor = 0.0f32;
    for i in 0..contorno.len() {
        let (a, b) = (contorno[i], contorno[(i + 1) % contorno.len()]);
        if let Some(t) = corte_del_rayo(centro, dx, dy, a, b) {
            mejor = mejor.max(t);
        }
    }
    if mejor <= 0.0 {
        return centro;
    }
    Punto2::nuevo(centro.x + dx * mejor, centro.y + dy * mejor)
}

/// **Las rayas de la guia, de borde a borde** (`lineasDeLaGuia`): cada una
/// va de la zona mirada (primer punto) al cristal (segundo). Una con la
/// flecha y el punto, dos con el cono, ninguna si esta apagada o si la lupa
/// esta encima de lo que mira.
pub fn lineas_de_la_guia(cr: &Cristal, caja: Caja) -> Vec<(Punto2, Punto2)> {
    if cr.guia == GuiaDeLupa::Ninguna || esta_encima(cr, caja) {
        return Vec::new();
    }
    let r = region(cr, caja);
    let (cc, cf) = (centro(caja), centro(r));
    let (dx, dy) = (cc.x - cf.x, cc.y - cf.y);
    let largo = dx.hypot(dy);
    if largo < 1e-6 {
        return Vec::new();
    }
    let del_cristal = puntos_del_cristal(cr, caja);
    let del_foco = puntos_del_foco(cr, caja);
    match cr.guia {
        GuiaDeLupa::Flecha | GuiaDeLupa::Punto => {
            // Con el punto la raya llega al punto, que es lo que la une a su
            // marca: el contorno de la zona ni se dibuja en ese modo.
            let desde = if cr.guia == GuiaDeLupa::Punto {
                cf
            } else {
                salida_del_contorno(cf, &del_foco, dx, dy)
            };
            vec![(desde, salida_del_contorno(cc, &del_cristal, -dx, -dy))]
        }
        GuiaDeLupa::DosLineas => {
            // El cono: de costado, perpendiculares a lo que une las dos
            // figuras, y por eso se abren en vez de cruzarse.
            let (px, py) = (-dy / largo, dx / largo);
            [1.0f32, -1.0]
                .iter()
                .map(|l| {
                    (
                        salida_del_contorno(cf, &del_foco, px * l, py * l),
                        salida_del_contorno(cc, &del_cristal, px * l, py * l),
                    )
                })
                .collect()
        }
        GuiaDeLupa::Ninguna => Vec::new(),
    }
}

/// Si el punto cae dentro de un contorno cerrado (par/impar).
pub fn dentro_del_contorno(p: Punto2, contorno: &[Punto2]) -> bool {
    let n = contorno.len();
    if n < 3 {
        return false;
    }
    let mut dentro = false;
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (contorno[i], contorno[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            dentro = !dentro;
        }
        j = i;
    }
    dentro
}

/// El primer contorno cerrado de una figura, ya girado, o `None`.
fn contorno_cerrado(e: &crate::elemento::Elemento) -> Option<Vec<Punto2>> {
    crate::perimetros::contornos_de(e, crate::perimetros::PASO_PERIMETRO)
        .into_iter()
        .find(|c| c.cerrado && c.puntos.len() >= 3)
        .map(|c| c.puntos)
}

/// **Si de esta figura se puede sacar una lupa** (`sirveDeLupa`): tiene que
/// encerrar algo. Una raya, una flecha o una cota no tienen dentro; y una
/// lupa o un foco ya son ventanas.
pub fn sirve_de_lupa(e: &crate::elemento::Elemento) -> bool {
    use crate::elemento::Figura;
    !e.borrado
        && !matches!(e.figura, Figura::Lupa { .. } | Figura::Foco { .. })
        && contorno_cerrado(e).is_some()
}

/// **La figura que la varita convierte al tocar `p`**: la de mas arriba
/// que encierra el punto (por dentro, no solo por su borde: un circulo sin
/// relleno se toca por donde se ve el hueco), o `None` sobre el vacio.
pub fn figura_bajo(elementos: &[crate::elemento::Elemento], p: Punto2) -> Option<u64> {
    elementos
        .iter()
        .rev()
        .filter(|e| sirve_de_lupa(e))
        .find(|e| {
            contorno_cerrado(e).is_some_and(|c| dentro_del_contorno(p, &c))
                || crate::impacto::toca(e, p)
        })
        .map(|e| e.id)
}

/// **Convierte una figura cerrada en una lupa con su misma forma**
/// (`lupaDesdeFigura`). El marco nace derecho, con la figura girada dentro
/// (el giro ya va en los puntos del contorno), agrandado `aumento` veces
/// desde el mismo centro; y lo mirado es la figura tocada, con su tamano
/// guardado, que es lo que no se mueve por mucho que crezca el cristal.
/// Devuelve el cristal y su caja, o `None` si la figura no encierra nada.
pub fn desde_figura(
    fuente: &crate::elemento::Elemento,
    aumento: f32,
    redonda: bool,
    guia: GuiaDeLupa,
) -> Option<(Cristal, Caja)> {
    if !sirve_de_lupa(fuente) {
        return None;
    }
    let contorno = contorno_cerrado(fuente)?;
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in &contorno {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    let (ancho, alto) = (x1 - x0, y1 - y0);
    if ancho < 1.0 || alto < 1.0 {
        return None;
    }
    let forma = contorno
        .iter()
        .map(|p| Punto2::nuevo((p.x - x0) / ancho, (p.y - y0) / alto))
        .collect();
    let z = aumento.clamp(AUMENTO_MINIMO, AUMENTO_MAXIMO);
    let c = Punto2::nuevo(x0 + ancho / 2.0, y0 + alto / 2.0);
    let cristal = Cristal {
        foco: Some(c),
        aumento: Some(z),
        foco_ancho: Some(ancho),
        foco_alto: Some(alto),
        oscurecer: None,
        redonda,
        guia,
        forma: Some(forma),
    };
    Some((
        cristal,
        (
            c.x - ancho * z / 2.0,
            c.y - alto * z / 2.0,
            ancho * z,
            alto * z,
        ),
    ))
}

/// El grueso de la montura, de la zona y de la guia: el del trazo por 1,6 y
/// nunca menos de 2,5 (`strokeWidth * 1.6` del movil). Es el borde de una
/// ventana y por donde se agarra: a un punto casi no se ve donde acaba.
pub fn grueso_de_montura(grosor: f32) -> f32 {
    (grosor * 1.6).max(2.5)
}

/// El rojo del punto gordo de la guia `Punto` (`argb(220, 38, 38)`).
pub const ROJO_DEL_PUNTO: crate::elemento::ColorRgba = crate::elemento::ColorRgba {
    r: 220.0 / 255.0,
    g: 38.0 / 255.0,
    b: 38.0 / 255.0,
    a: 1.0,
};

fn cerrar(mut v: Vec<Punto2>) -> Vec<Punto2> {
    if let Some(p) = v.first().copied() {
        v.push(p);
    }
    v
}

/// **Lo que se pinta de una lupa que no es su contenido** (`drawLupa` sin la
/// escena y `dibujarLaGuia`): la montura, el contorno de lo mirado (con la
/// flecha y el cono), las rayas de la guia y la punta o el punto rojo. El
/// contenido agrandado lo pinta quien tiene la escena, recortado a
/// [`puntos_del_cristal`], y vuelve a poner la montura encima.
pub fn ordenes_de_la_lupa(
    cr: &Cristal,
    caja: Caja,
    color: crate::elemento::ColorRgba,
    grosor: f32,
) -> Vec<crate::pintado::Orden> {
    use crate::elemento::EstiloTrazo;
    use crate::pintado::Orden;
    let g = grueso_de_montura(grosor);
    let mut salida = vec![Orden::Polilinea {
        puntos: cerrar(puntos_del_cristal(cr, caja)),
        color,
        grosor: g,
        estilo: EstiloTrazo::Solido,
    }];
    if esta_encima(cr, caja) {
        return salida;
    }
    if matches!(cr.guia, GuiaDeLupa::Flecha | GuiaDeLupa::DosLineas) {
        salida.push(Orden::Polilinea {
            puntos: cerrar(puntos_del_foco(cr, caja)),
            color,
            grosor: g,
            estilo: EstiloTrazo::Solido,
        });
    }
    let lineas = lineas_de_la_guia(cr, caja);
    for (a, b) in &lineas {
        salida.push(Orden::Polilinea {
            puntos: vec![*b, *a],
            color,
            grosor: g,
            estilo: EstiloTrazo::Solido,
        });
    }
    match cr.guia {
        GuiaDeLupa::Punto => {
            let r = region(cr, caja);
            let radio = g.max(6.0);
            salida.push(Orden::Poligono {
                puntos: ovalo(centro(r), radio, radio),
                color: ROJO_DEL_PUNTO,
            });
        }
        GuiaDeLupa::Flecha => {
            // La punta en el extremo del foco, que es lo que se senala; crece
            // con el grosor para no quedarse dentro de la propia raya.
            if let Some((desde, hasta)) = lineas.first().copied() {
                let ang = (desde.y - hasta.y).atan2(desde.x - hasta.x);
                let largo = (grosor * 4.0).max(12.0);
                let abre = 22.0f32.to_radians();
                salida.push(Orden::Poligono {
                    puntos: vec![
                        desde,
                        Punto2::nuevo(
                            desde.x - largo * (ang - abre).cos(),
                            desde.y - largo * (ang - abre).sin(),
                        ),
                        Punto2::nuevo(
                            desde.x - largo * (ang + abre).cos(),
                            desde.y - largo * (ang + abre).sin(),
                        ),
                    ],
                    color,
                });
            }
        }
        GuiaDeLupa::DosLineas | GuiaDeLupa::Ninguna => {}
    }
    salida
}

/// El cristal de un elemento, si es una lupa.
pub fn de(e: &crate::elemento::Elemento) -> Option<&Cristal> {
    match &e.figura {
        crate::elemento::Figura::Lupa { cristal } => Some(cristal),
        _ => None,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const CAJA: Caja = (100.0, 100.0, 200.0, 100.0);

    #[test]
    fn una_lupa_sin_foco_mira_a_su_propio_centro() {
        // Una lupa recien dejada sobre el papel agranda lo que tiene debajo.
        let cr = Cristal::default();
        assert_eq!(foco_de(&cr, CAJA), Punto2::nuevo(200.0, 150.0));
        // Y recoge el cristal partido por el aumento de fabrica.
        assert_eq!(region(&cr, CAJA), (150.0, 125.0, 100.0, 50.0));
    }

    #[test]
    fn el_aumento_sale_de_la_geometria_y_no_del_numero_guardado() {
        // El caso que da sentido a la regla: el cristal se estiro con los
        // tiradores y el numero guardado se quedo diciendo x2.
        let cr = Cristal {
            aumento: Some(2.0),
            foco_ancho: Some(50.0),
            ..Default::default()
        };
        assert_eq!(
            aumento_de(&cr, CAJA),
            4.0,
            "200 de cristal sobre 50 de zona"
        );
        // Sin zona guardada si manda el numero.
        let cr = Cristal {
            aumento: Some(3.0),
            ..Default::default()
        };
        assert_eq!(aumento_de(&cr, CAJA), 3.0);
    }

    #[test]
    fn el_aumento_no_se_sale_de_sus_topes_ni_con_una_zona_absurda() {
        // Caso negativo: una zona de un pixel daria x200 y se veria el grano
        // de la pantalla, no el detalle.
        let cr = Cristal {
            foco_ancho: Some(0.1),
            ..Default::default()
        };
        assert_eq!(aumento_de(&cr, CAJA), AUMENTO_MAXIMO);
        // Y una zona mas grande que el cristal no puede encoger el dibujo.
        let cr = Cristal {
            foco_ancho: Some(10_000.0),
            ..Default::default()
        };
        assert_eq!(aumento_de(&cr, CAJA), AUMENTO_MINIMO);
    }

    #[test]
    fn subir_el_aumento_mueve_la_ventana_y_no_la_zona_mirada() {
        let cr = Cristal {
            foco: Some(Punto2::nuevo(500.0, 500.0)),
            foco_ancho: Some(40.0),
            foco_alto: Some(20.0),
            ..Default::default()
        };
        let (nuevo, caja) = con_aumento(&cr, CAJA, 5.0);
        assert_eq!(
            region(&nuevo, caja),
            (480.0, 490.0, 40.0, 20.0),
            "la zona no se movio"
        );
        assert_eq!(
            (caja.2, caja.3),
            (200.0, 100.0),
            "la ventana es la zona por cinco"
        );
        // Y crece desde su propio centro.
        assert_eq!(centro(caja), centro(CAJA));
    }

    #[test]
    fn un_foco_sin_valor_oscurece_lo_mismo_que_el_movil() {
        // 45 y no 60: el 60 era el de fabrica del pintado del PC, y hacia que
        // un foco del movil apagara aqui mas de lo que apagaba alli.
        assert_eq!(oscurecimiento_de(&Cristal::default()), 45);
        // Y lo que venga fuera de rango se recorta, no se acepta.
        let cr = Cristal {
            oscurecer: Some(200),
            ..Default::default()
        };
        assert_eq!(oscurecimiento_de(&cr), OSCURECER_MAXIMO);
        let cr = Cristal {
            oscurecer: Some(0),
            ..Default::default()
        };
        assert_eq!(oscurecimiento_de(&cr), OSCURECER_MINIMO);
    }

    #[test]
    fn ajustar_la_zona_de_un_foco_no_mueve_su_marco() {
        // Es lo contrario que la lupa, y es lo que hace que el borde de la
        // sombra se quede donde el usuario lo puso.
        let cr = con_zona(&Cristal::default(), CAJA, 0.5);
        assert_eq!(cr.foco_ancho, Some(100.0));
        assert_eq!(cr.foco_alto, Some(50.0));
        assert_eq!(zona_de(&cr, CAJA), 0.5);
        // Y la zona no puede comerse el marco entero ni desaparecer.
        assert_eq!(zona_de(&con_zona(&cr, CAJA, 9.0), CAJA), ZONA_MAXIMA);
        assert_eq!(zona_de(&con_zona(&cr, CAJA, 0.0), CAJA), ZONA_MINIMA);
    }

    #[test]
    fn el_foco_redondo_no_se_agarra_por_las_esquinas_de_su_caja() {
        // Caso negativo: con la caja, las cuatro esquinas respondian a un
        // sitio donde no hay nada dibujado.
        let cr = Cristal {
            redonda: true,
            foco_ancho: Some(100.0),
            foco_alto: Some(100.0),
            ..Default::default()
        };
        let c = foco_de(&cr, CAJA);
        assert!(toca_el_foco(&cr, CAJA, c, 0.0), "el centro si");
        let esquina = Punto2::nuevo(c.x + 49.0, c.y + 49.0);
        assert!(!toca_el_foco(&cr, CAJA, esquina, 0.0), "la esquina no");
        // Rectangular, esa misma esquina si.
        let cr = Cristal {
            redonda: false,
            ..cr
        };
        assert!(toca_el_foco(&cr, CAJA, esquina, 0.0));
    }

    #[test]
    fn la_forma_propia_se_devuelve_en_coordenadas_del_dibujo() {
        let cr = Cristal {
            forma: Some(vec![
                Punto2::nuevo(0.0, 0.0),
                Punto2::nuevo(1.0, 0.0),
                Punto2::nuevo(0.5, 1.0),
            ]),
            ..Default::default()
        };
        let c = contorno_del_cristal(&cr, CAJA).unwrap();
        assert_eq!(c[0], Punto2::nuevo(100.0, 100.0));
        assert_eq!(c[1], Punto2::nuevo(300.0, 100.0));
        assert_eq!(c[2], Punto2::nuevo(200.0, 200.0));
        // Una forma que no encierra nada no es una forma.
        let cr = Cristal {
            forma: Some(vec![Punto2::nuevo(0.0, 0.0)]),
            ..Default::default()
        };
        assert!(contorno_del_cristal(&cr, CAJA).is_none());
    }

    #[test]
    fn la_lupa_del_movil_va_y_vuelve_con_sus_nueve_campos() {
        let json = serde_json::json!({
            "id": "l1", "type": "pixpin-lupa",
            "x": 100, "y": 100, "width": 200, "height": 100,
            "foco": {"x": 500.0, "y": 640.0},
            "aumento": 3.5,
            "focoAncho": 57.0, "focoAlto": 28.5,
            "oscurecer": 70,
            "lupaRedonda": true,
            "guia": "DOS_LINEAS",
            "forma": [{"x": 0.0, "y": 0.0}, {"x": 1.0, "y": 0.5}, {"x": 0.0, "y": 1.0}]
        });
        let cr = leer(&json);
        assert_eq!(cr.foco, Some(Punto2::nuevo(500.0, 640.0)));
        assert_eq!(cr.aumento, Some(3.5));
        assert_eq!(cr.foco_ancho, Some(57.0));
        assert_eq!(cr.oscurecer, Some(70));
        assert!(cr.redonda);
        assert_eq!(cr.guia, GuiaDeLupa::DosLineas);
        assert_eq!(cr.forma.as_ref().unwrap().len(), 3);

        let mut mapa = json.as_object().unwrap().clone();
        escribir(&mut mapa, &cr);
        let vuelta = leer(&Value::Object(mapa));
        assert_eq!(cr, vuelta, "la ida y vuelta cambio algo");
    }

    #[test]
    fn una_lupa_vieja_sin_guia_conserva_lo_que_tenia() {
        // Las de antes no llevan campo `guia`, solo los dos si/no. Sin esta
        // traduccion, abrir una lupa vieja aqui le quitaria la flecha.
        let cr = leer(&serde_json::json!({"lupaFlecha": true}));
        assert_eq!(cr.guia, GuiaDeLupa::Flecha);
        let cr = leer(&serde_json::json!({"lupaDosLineas": true}));
        assert_eq!(cr.guia, GuiaDeLupa::DosLineas);
        // Y al escribirla vuelve con los dos campos viejos a juego.
        let mut m = Map::new();
        escribir(&mut m, &cr);
        assert_eq!(m["guia"], Value::String("DOS_LINEAS".into()));
        assert_eq!(m["lupaDosLineas"], Value::Bool(true));
        assert_eq!(m["lupaFlecha"], Value::Bool(false));
    }

    #[test]
    fn lo_que_no_esta_no_se_inventa_a_cero() {
        // **La prueba que importa del puente.** Un `focoAncho` escrito a cero
        // seria una zona de area nula: la lupa dejaria de mirar a ninguna
        // parte y no habria forma de recuperar a donde miraba.
        let cr = leer(&serde_json::json!({}));
        assert_eq!(cr.foco, None);
        assert_eq!(cr.foco_ancho, None);
        assert_eq!(cr.aumento, None);
        assert_eq!(cr.oscurecer, None);
        let mut m = Map::new();
        m.insert("focoAncho".into(), Value::from(12.0));
        escribir(&mut m, &cr);
        assert!(!m.contains_key("focoAncho"), "se escribio un cero: {m:?}");
        assert!(!m.contains_key("foco"));
    }

    // --- La lupa en el lienzo ---

    use crate::elemento::{Elemento, Figura};

    fn figura(f: Figura, x: f32, y: f32, ancho: f32, alto: f32) -> Elemento {
        Elemento {
            id: 7,
            figura: f,
            x,
            y,
            ancho,
            alto,
            grosor: 2.0,
            ..Default::default()
        }
    }

    fn lupa(cr: Cristal, c: Caja) -> Elemento {
        Elemento {
            id: 9,
            figura: Figura::Lupa { cristal: cr },
            x: c.0,
            y: c.1,
            ancho: c.2,
            alto: c.3,
            grosor: 2.0,
            ..Default::default()
        }
    }

    #[test]
    fn la_varita_convierte_un_circulo_en_una_lupa_del_doble_con_su_misma_forma() {
        let circulo = figura(Figura::Elipse, 0.0, 0.0, 100.0, 50.0);
        let (cr, caja) = desde_figura(&circulo, 2.0, true, GuiaDeLupa::Flecha).unwrap();
        // Centrada donde estaba el circulo y el doble de grande.
        let c = centro(caja);
        assert!(
            (c.x - 50.0).abs() < 1.0 && (c.y - 25.0).abs() < 1.0,
            "{caja:?}"
        );
        assert!(
            (caja.2 - 200.0).abs() < 2.0 && (caja.3 - 100.0).abs() < 2.0,
            "{caja:?}"
        );
        // Mira a la figura tocada, con su tamano guardado.
        assert!((cr.foco_ancho.unwrap() - 100.0).abs() < 2.0);
        assert!((aumento_de(&cr, caja) - 2.0).abs() < 0.05);
        // Y el cristal tiene la forma del circulo, no la de su caja: el
        // punto de la esquina de la caja queda fuera.
        let contorno = puntos_del_cristal(&cr, caja);
        assert!(contorno.len() >= 16, "un ovalo, no cuatro esquinas");
        assert!(!dentro_del_contorno(
            Punto2::nuevo(caja.0 + 2.0, caja.1 + 2.0),
            &contorno
        ));
        assert!(dentro_del_contorno(c, &contorno));
    }

    #[test]
    fn una_raya_ni_una_lupa_sirven_de_lupa() {
        let raya = figura(
            Figura::Linea {
                puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0)],
            },
            0.0,
            0.0,
            100.0,
            0.0,
        );
        assert!(!sirve_de_lupa(&raya), "una raya no encierra nada");
        assert!(desde_figura(&raya, 2.0, true, GuiaDeLupa::Flecha).is_none());
        let otra = lupa(Cristal::default(), (0.0, 0.0, 100.0, 100.0));
        assert!(
            !sirve_de_lupa(&otra),
            "una lupa mirando a otra no tiene fondo"
        );
        // Y un rectangulo si.
        assert!(sirve_de_lupa(&figura(
            Figura::Rectangulo,
            0.0,
            0.0,
            50.0,
            50.0
        )));
    }

    #[test]
    fn la_varita_toca_un_circulo_sin_relleno_por_dentro_y_en_el_vacio_no_hace_nada() {
        let circulo = figura(Figura::Elipse, 0.0, 0.0, 100.0, 100.0);
        let v = vec![circulo];
        assert_eq!(
            figura_bajo(&v, Punto2::nuevo(50.0, 50.0)),
            Some(7),
            "por el hueco"
        );
        assert_eq!(
            figura_bajo(&v, Punto2::nuevo(300.0, 300.0)),
            None,
            "en el aire"
        );
        // La esquina de la caja no es del circulo.
        assert_eq!(figura_bajo(&v, Punto2::nuevo(3.0, 3.0)), None);
    }

    #[test]
    fn apartada_la_lupa_la_flecha_va_del_borde_de_lo_mirado_al_borde_del_cristal() {
        // Mira un recuadro de 40 x 20 en (100, 100) y el cristal esta a la
        // derecha, lejos.
        let cr = Cristal {
            foco: Some(Punto2::nuevo(100.0, 100.0)),
            aumento: Some(2.0),
            foco_ancho: Some(40.0),
            foco_alto: Some(20.0),
            guia: GuiaDeLupa::Flecha,
            ..Default::default()
        };
        let caja = (300.0, 80.0, 80.0, 40.0);
        let l = lineas_de_la_guia(&cr, caja);
        assert_eq!(l.len(), 1);
        let (desde, hasta) = l[0];
        assert!(
            (desde.x - 120.0).abs() < 0.01,
            "sale del borde derecho de lo mirado: {desde:?}"
        );
        assert!(
            (hasta.x - 300.0).abs() < 0.01,
            "llega al borde izquierdo del cristal: {hasta:?}"
        );
        // El cono da dos, y el punto una que llega al centro.
        let cono = Cristal {
            guia: GuiaDeLupa::DosLineas,
            ..cr.clone()
        };
        assert_eq!(lineas_de_la_guia(&cono, caja).len(), 2);
        let punto = Cristal {
            guia: GuiaDeLupa::Punto,
            ..cr.clone()
        };
        let (p, _) = lineas_de_la_guia(&punto, caja)[0];
        assert!((p.x - 100.0).abs() < 0.01 && (p.y - 100.0).abs() < 0.01);
    }

    #[test]
    fn apoyada_sobre_lo_que_mira_la_lupa_solo_pinta_su_montura() {
        let cr = Cristal {
            guia: GuiaDeLupa::Flecha,
            ..Default::default()
        };
        let caja = (0.0, 0.0, 100.0, 60.0);
        assert!(esta_encima(&cr, caja), "sin foco mira a su centro");
        assert!(lineas_de_la_guia(&cr, caja).is_empty());
        let o = ordenes_de_la_lupa(
            &cr,
            caja,
            crate::elemento::ColorRgba::opaco(0.0, 0.0, 0.0),
            2.0,
        );
        assert_eq!(o.len(), 1, "solo la montura");
        // Apartada: montura, zona, raya y punta.
        let lejos = Cristal {
            foco: Some(Punto2::nuevo(500.0, 500.0)),
            foco_ancho: Some(20.0),
            foco_alto: Some(20.0),
            ..cr
        };
        let o = ordenes_de_la_lupa(
            &lejos,
            caja,
            crate::elemento::ColorRgba::opaco(0.0, 0.0, 0.0),
            2.0,
        );
        assert_eq!(o.len(), 4, "{o:?}");
    }

    #[test]
    fn una_lupa_nacida_en_el_escritorio_llega_al_movil_como_pixpin_lupa_y_vuelve_igual() {
        use crate::excalidraw::{Entrada, Lienzo, escribir, leer};
        let circulo = figura(Figura::Elipse, 0.0, 0.0, 100.0, 50.0);
        let (cr, caja) = desde_figura(&circulo, 3.0, true, GuiaDeLupa::DosLineas).unwrap();
        let mut l = Lienzo::vacio();
        l.entradas.push(Entrada::Nuestro {
            elemento: lupa(cr.clone(), caja),
            original: Box::new(Value::Object(Map::new())),
        });
        let json = escribir(&l);
        assert!(json.contains("\"pixpin-lupa\""), "{json}");
        assert!(json.contains("\"DOS_LINEAS\""), "{json}");
        let vuelta = leer(&json).unwrap();
        let e = &vuelta.elementos()[0];
        let Figura::Lupa { cristal } = &e.figura else {
            panic!("volvio como otra cosa: {:?}", e.figura);
        };
        assert_eq!(cristal.guia, GuiaDeLupa::DosLineas);
        assert_eq!(
            cristal.forma.as_ref().map(Vec::len),
            cr.forma.as_ref().map(Vec::len)
        );
        assert!((cristal.aumento.unwrap() - 3.0).abs() < 1e-4);
    }
}
