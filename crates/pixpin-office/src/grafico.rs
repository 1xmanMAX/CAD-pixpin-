//! **Los graficos de Office** (`c:chartSpace`, ECMA-376 1.ª parte §21.2)
//! dentro de un Word o un PowerPoint, dibujados con los datos que el propio
//! archivo guarda en cache (`c:numCache`, `c:strCache`, `c:numLit`...): no
//! se evalua ninguna formula ni se abre la hoja incrustada.
//!
//! Se lee la parte del grafico a un [`Grafico`] y se pinta con los elementos
//! de siempre (`Rect`, `Linea`, `Trayecto`, `Texto`) en una caja de
//! `ancho` x `alto` puntos con origen (0, 0). Los textos se miden con el
//! [`Medidor`] de verdad (DirectWrite en la aplicacion).
//!
//! Tipos: columnas y barras (agrupadas, apiladas y al 100 %), lineas (con
//! marcadores), circular y anillo, area, dispersion, radar y burbuja; los
//! 3D se dibujan planos. Lo demas (cotizaciones, superficie, los `cx:chart`
//! nuevos) sale con un aviso en su caja, nunca como un hueco.

// Partes traducidas de deckcraft (https://github.com/storytold/deckcraft), MIT OR Apache-2.0, Copyright (c) 2026 ArtCraft Team and the Deckcraft contributors.
// Partes traducidas de wordcraft (https://github.com/storytold/wordcraft), MIT OR Apache-2.0, Copyright (c) 2026 ArtCraft Team and the Wordcraft contributors.

use std::f32::consts::{FRAC_PI_2, TAU};
use std::sync::Arc;

use pixpin_docs::xml::Nodo;

use crate::dml::{self, Contexto};
use crate::medidor::Medidor;
use crate::pagina::{
    Color, Discontinuo, Elemento, Figura, Letra, Rect, Relleno, Texto, Tramo, Trazo,
};
use crate::paquete::{Paquete, Relaciones, attr_num, emu};
use crate::tema;

/// Mas series que estas no se leen (Office tampoco deja mas de 255).
const MAX_SERIES: usize = 255;
/// Puntos por serie como mucho (indices y cuentas del archivo se recortan).
const MAX_PUNTOS: usize = 4000;
/// Marcas de un eje como mucho.
const MAX_MARCAS: usize = 60;
/// Lo que se separa el contenido del borde del grafico.
const MARGEN: f32 = 7.0;
/// Hueco entre una etiqueta y su eje.
const HUECO: f32 = 3.0;
/// Largo de las marcas de los ejes.
const MARCA: f32 = 4.0;
/// Grosor de una serie de lineas si el archivo no dice otro (el de Office).
const GROSOR_LINEA: f32 = 2.25;

const GRIS_TEXTO: Color = Color::rgb(0x59, 0x59, 0x59);
const GRIS_ETIQUETA: Color = Color::rgb(0x40, 0x40, 0x40);
const GRIS_REJILLA: Color = Color::rgb(0xD9, 0xD9, 0xD9);
const GRIS_EJE: Color = Color::rgb(0xBF, 0xBF, 0xBF);
const IDENTIDAD: [f32; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

// ---------------------------------------------------------------------
// El modelo
// ---------------------------------------------------------------------

/// Como se juntan las series de un grupo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agrupacion {
    /// Una al lado de otra (barras) o cada una a su altura (lineas).
    Normal,
    Apilada,
    /// Apilada y repartida al 100 %.
    Porcentaje,
}

/// Que dibuja un grupo (`c:barChart`, `c:lineChart`...).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Clase {
    /// Columnas (`horizontal: false`) o barras.
    Barras {
        horizontal: bool,
    },
    Lineas,
    Area,
    /// Circular (`agujero` 0) o anillo (`agujero`: fraccion del radio).
    /// `primer_angulo` en grados, desde las doce y en el sentido del reloj.
    Circular {
        agujero: f32,
        primer_angulo: f32,
    },
    Dispersion,
    Burbuja,
    Radar {
        relleno: bool,
    },
}

impl Clase {
    fn cartesiana(&self) -> bool {
        matches!(
            self,
            Clase::Barras { .. } | Clase::Lineas | Clase::Area | Clase::Dispersion | Clase::Burbuja
        )
    }
    fn xy(&self) -> bool {
        matches!(self, Clase::Dispersion | Clase::Burbuja)
    }
    /// En la leyenda: una caja de color (si no, una raya con marcador).
    fn muestra_caja(&self) -> bool {
        !matches!(
            self,
            Clase::Lineas | Clase::Dispersion | Clase::Radar { relleno: false }
        )
    }
}

/// Como se ve un texto del grafico.
#[derive(Debug, Clone, PartialEq)]
pub struct EstiloTexto {
    pub familia: String,
    pub tam: f32,
    pub negrita: bool,
    pub cursiva: bool,
    pub color: Color,
}

impl EstiloTexto {
    fn letra(&self) -> Letra {
        Letra {
            familia: Arc::from(self.familia.as_str()),
            tam: self.tam,
            negrita: self.negrita,
            cursiva: self.cursiva,
        }
    }
}

/// Un sitio puesto a mano (`c:manualLayout` en modo `edge`), en fracciones
/// del grafico.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Manual {
    /// `layoutTarget inner`: la caja es la del trazado sin las etiquetas.
    pub interior: bool,
    pub x: f32,
    pub y: f32,
    pub w: Option<f32>,
    pub h: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Titulo {
    pub lineas: Vec<String>,
    pub estilo: EstiloTexto,
    pub manual: Option<Manual>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Leyenda {
    /// `r`, `l`, `t`, `b` o `tr`.
    pub pos: String,
    pub manual: Option<Manual>,
    pub estilo: EstiloTexto,
    /// Entradas quitadas (`c:legendEntry` con `c:delete`).
    pub borradas: Vec<usize>,
}

/// Que pone en cada punto un `c:dLbls`.
#[derive(Debug, Clone, PartialEq)]
pub struct Etiquetas {
    pub valor: bool,
    pub porcentaje: bool,
    pub categoria: bool,
    pub serie: bool,
    /// `c:delete`: ninguna, aunque el grupo las pida.
    pub borrar: bool,
    /// `c:dLblPos` (`outEnd`, `ctr`, `inEnd`, `inBase`, `bestFit`, `t`,
    /// `b`, `l`, `r`), vacio si no lo dice.
    pub posicion: String,
    /// El formato propio (no vinculado al origen).
    pub formato: Option<String>,
    pub separador: String,
    pub estilo: EstiloTexto,
    /// Si el color de la letra lo dice el archivo (si no, se busca el que
    /// contraste dentro de una barra).
    pub color_propio: bool,
    /// Puntos con su etiqueta quitada (`c:dLbl` con `c:delete`).
    pub borrados: Vec<usize>,
}

impl Etiquetas {
    fn alguna(&self) -> bool {
        !self.borrar && (self.valor || self.porcentaje || self.categoria || self.serie)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Simbolo {
    Ninguno,
    Circulo,
    Cuadrado,
    Rombo,
    Triangulo,
    Equis,
    Estrella,
    Raya,
    Punto,
    Mas,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Marcador {
    /// `None`: el automatico.
    pub simbolo: Option<Simbolo>,
    pub tam: f32,
    pub relleno: Option<Option<Relleno>>,
    pub linea: Option<Option<Trazo>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Serie {
    /// `c:idx`: da el color automatico.
    pub indice: usize,
    pub nombre: String,
    pub valores: Vec<Option<f64>>,
    /// Las X de una dispersion (vacias si no son numeros).
    pub xs: Vec<Option<f64>>,
    pub tamanos: Vec<Option<f64>>,
    /// El `formatCode` de la cache de valores.
    pub formato: String,
    /// El relleno del `c:spPr`: `None` si no dice nada, `Some(None)` sin relleno.
    pub relleno: Option<Option<Relleno>>,
    pub linea: Option<Option<Trazo>>,
    pub grosor: Option<f32>,
    /// Rellenos propios de algunos puntos (`c:dPt`).
    pub puntos: Vec<(usize, Option<Relleno>)>,
    pub marcador: Option<Marcador>,
    pub suave: bool,
    pub etiquetas: Option<Etiquetas>,
    /// El color automatico (acento del tema por `c:idx`).
    pub auto: Color,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Grupo {
    pub clase: Clase,
    pub agrupacion: Agrupacion,
    pub series: Vec<Serie>,
    pub varia_colores: bool,
    /// `c:gapWidth` y `c:overlap` en fraccion (150 % → 1,5).
    pub hueco: f32,
    pub solape: f32,
    /// `c:marker` del grupo de lineas: marcadores por defecto.
    pub marcadores: bool,
    pub estilo_dispersion: String,
    pub escala_burbuja: f32,
    pub etiquetas: Option<Etiquetas>,
    /// Los `c:axId` del grupo.
    pub ejes: Vec<String>,
}

/// Donde corta un eje al otro.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cruce {
    Cero,
    Min,
    Max,
    En(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Eje {
    pub id: String,
    /// `c:valAx` (si no, de categorias o de fechas).
    pub valores: bool,
    /// `l`, `r`, `t` o `b`.
    pub pos: char,
    pub borrado: bool,
    pub invertido: bool,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub unidad: Option<f64>,
    /// El formato propio (no vinculado al origen).
    pub formato: Option<String>,
    pub cuadricula: Option<Trazo>,
    pub linea: Option<Trazo>,
    /// `c:tickLblPos`: `nextTo`, `low`, `high` o `none`.
    pub pos_etiquetas: String,
    /// `c:majorTickMark`: `out`, `in`, `cross` o `none`.
    pub marcas: String,
    pub titulo: Option<Titulo>,
    pub estilo: EstiloTexto,
    pub cruza: Cruce,
    /// `c:crossBetween between`: los puntos en el centro de cada categoria.
    pub entre: bool,
}

impl Eje {
    fn defecto(valores: bool, pos: char, base: &EstiloTexto) -> Eje {
        Eje {
            id: String::new(),
            valores,
            pos,
            borrado: false,
            invertido: false,
            min: None,
            max: None,
            unidad: None,
            formato: None,
            cuadricula: valores.then(|| Trazo::solido(GRIS_REJILLA, 0.75)),
            linea: Some(Trazo::solido(GRIS_EJE, 0.75)),
            pos_etiquetas: "nextTo".into(),
            marcas: "none".into(),
            titulo: None,
            estilo: base.clone(),
            cruza: Cruce::Cero,
            entre: true,
        }
    }

    fn visible(&self) -> bool {
        !self.borrado && self.pos_etiquetas != "none"
    }
}

/// Que hacer con un punto sin valor (`c:dispBlanksAs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Huecos {
    Hueco,
    Cero,
    Seguir,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Grafico {
    pub titulo: Option<Titulo>,
    pub leyenda: Option<Leyenda>,
    pub grupos: Vec<Grupo>,
    pub ejes: Vec<Eje>,
    pub categorias: Vec<String>,
    /// Colores automaticos por indice (acentos del tema y sus variaciones).
    pub paleta: Vec<Color>,
    pub fondo: Option<Relleno>,
    pub borde: Option<Trazo>,
    pub fondo_trazado: Option<Relleno>,
    pub borde_trazado: Option<Trazo>,
    pub manual_trazado: Option<Manual>,
    pub huecos: Huecos,
    pub texto: EstiloTexto,
    /// Los tipos que hay en el archivo y aqui no se dibujan.
    pub sin_dibujo: Vec<String>,
}

// ---------------------------------------------------------------------
// La entrada desde Word y PowerPoint
// ---------------------------------------------------------------------

/// Si un `a:graphicData` es un grafico de los de siempre (`c:chart`).
pub fn es_grafico(uri: &str) -> bool {
    uri.ends_with("/drawingml/2006/chart")
}

/// Si es uno de los graficos nuevos (`cx:chart`, «chartex»), que aqui no
/// se leen.
pub fn es_grafico_nuevo(uri: &str) -> bool {
    uri.contains("chartex")
}

/// Los elementos de un `a:graphicData` de grafico en una caja de `ancho` x
/// `alto` con origen (0, 0). `None` si no es un grafico. Un grafico que no
/// se puede leer da un aviso, no un hueco.
pub fn de_datos(
    paquete: &mut Paquete,
    rels: &Relaciones,
    datos: &Nodo,
    ctx: Contexto,
    medidor: &dyn Medidor,
    ancho: f32,
    alto: f32,
) -> Option<Vec<Elemento>> {
    let uri = datos.atributo("uri");
    if es_grafico_nuevo(uri) {
        return Some(aviso(
            ancho,
            alto,
            "Gráfico de un tipo nuevo de Office: sin vista previa",
            medidor,
        ));
    }
    if !es_grafico(uri) {
        return None;
    }
    let c = datos.hijo("chart");
    let rid = c.map(|c| {
        let r = c.atributo_con_prefijo("r", "id");
        if r.is_empty() { c.atributo("id") } else { r }
    });
    let raiz = rid
        .and_then(|r| rels.get(r))
        .filter(|r| !r.externa)
        .and_then(|r| paquete.xml(&r.destino));
    Some(match raiz.as_ref().and_then(|r| leer(r, ctx)) {
        Some(g) => pintar(&g, ancho, alto, medidor),
        None => aviso(ancho, alto, "Gráfico sin datos que se puedan leer", medidor),
    })
}

/// La rama de un `mc:AlternateContent` que se dibuja en Word: la primera
/// (`mc:Choice`) salvo que traiga un grafico nuevo que aqui no se lee y
/// haya respaldo (`mc:Fallback`).
pub fn rama_alternativa(ac: &Nodo) -> Option<&Nodo> {
    let eleccion = ac.hijo("Choice");
    let respaldo = ac.hijo("Fallback");
    match eleccion {
        Some(c)
            if respaldo.is_some()
                && c.descendientes("graphicData")
                    .iter()
                    .any(|d| es_grafico_nuevo(d.atributo("uri"))) =>
        {
            respaldo
        }
        Some(c) => Some(c),
        None => respaldo,
    }
}

/// Un recuadro con un aviso dentro, para lo que no se sabe dibujar.
pub fn aviso(ancho: f32, alto: f32, texto: &str, medidor: &dyn Medidor) -> Vec<Elemento> {
    if ancho < 1.0 || alto < 1.0 {
        return Vec::new();
    }
    let mut els = vec![Elemento::Rect {
        rect: Rect::nuevo(0.0, 0.0, ancho, alto),
        relleno: Some(Relleno::Solido(Color::rgb(0xF2, 0xF2, 0xF2))),
        borde: Some(Trazo {
            color: GRIS_EJE,
            grosor: 0.75,
            discontinuo: Discontinuo::Rayas,
        }),
    }];
    let mut estilo = EstiloTexto {
        familia: "Segoe UI".into(),
        tam: 10.0,
        negrita: false,
        cursiva: false,
        color: GRIS_TEXTO,
    };
    let p = Pincel { m: medidor };
    let w = p.ancho(texto, &estilo);
    if w > ancho - 8.0 && w > 0.0 {
        estilo.tam = (estilo.tam * (ancho - 8.0) / w).max(5.0);
    }
    if let Some((e, _)) = p.texto(texto, &estilo, ancho / 2.0, alto / 2.0, 0.5, 0.5) {
        els.push(e);
    }
    vec![Elemento::Grupo {
        transformada: IDENTIDAD,
        recorte: Some(Rect::nuevo(0.0, 0.0, ancho, alto)),
        elementos: els,
    }]
}

// ---------------------------------------------------------------------
// El lector
// ---------------------------------------------------------------------

/// `<c:x val="1"/>` y `<c:x/>` son verdad; `0`, `false` y `off`, no.
fn si(n: &Nodo) -> bool {
    !matches!(n.valor(), "0" | "false" | "off")
}

fn bandera(padre: &Nodo, k: &str) -> bool {
    padre.hijo(k).is_some_and(si)
}

fn num_hijo(n: &Nodo, k: &str) -> Option<f64> {
    n.hijo(k)
        .and_then(|x| x.valor().trim().parse::<f64>().ok())
        .filter(|v| v.is_finite())
}

fn entero_hijo(n: &Nodo, k: &str) -> Option<usize> {
    n.hijo(k)
        .and_then(|x| x.valor().trim().parse::<usize>().ok())
}

fn aplicar_rpr(e: &mut EstiloTexto, rpr: &Nodo, ctx: Contexto) {
    if let Some(sz) = attr_num(rpr, "sz", 1.0)
        && sz > 0.0
    {
        e.tam = (sz / 100.0).clamp(1.0, 400.0);
    }
    match rpr.atributo("b") {
        "1" | "true" => e.negrita = true,
        "0" | "false" => e.negrita = false,
        _ => {}
    }
    match rpr.atributo("i") {
        "1" | "true" => e.cursiva = true,
        "0" | "false" => e.cursiva = false,
        _ => {}
    }
    if let Some(Some(r)) = dml::relleno(rpr, ctx, None) {
        e.color = r.color_medio();
    }
    if let Some(l) = rpr.hijo("latin") {
        let f = ctx.tema.letra(l.atributo("typeface"));
        if !f.is_empty() {
            e.familia = f;
        }
    }
}

/// El estilo de un `c:txPr` (o de un `c:rich`) sobre `base`: el `defRPr`
/// del primer parrafo y el `rPr` de su primera corrida.
fn estilo_de(txpr: Option<&Nodo>, base: &EstiloTexto, ctx: Contexto) -> EstiloTexto {
    let mut e = base.clone();
    if let Some(p) = txpr.and_then(|t| t.hijo("p")) {
        if let Some(d) = p.hijo("pPr").and_then(|x| x.hijo("defRPr")) {
            aplicar_rpr(&mut e, d, ctx);
        }
        if let Some(r) = p
            .elementos()
            .find(|x| x.nombre == "r")
            .and_then(|r| r.hijo("rPr"))
        {
            aplicar_rpr(&mut e, r, ctx);
        }
    }
    e
}

/// Los puntos de una cache (`numCache`, `strCache`, `numLit`, `strLit` o el
/// primer nivel de `multiLvlStrCache`), por indice, y su `formatCode`.
fn cache(n: &Nodo) -> (Vec<Option<String>>, String) {
    let simple = ["numCache", "strCache", "numLit", "strLit"]
        .iter()
        .find_map(|k| n.descendientes(k).into_iter().next())
        .map(|c| (c, c));
    let multi = || {
        n.descendientes("multiLvlStrCache")
            .into_iter()
            .next()
            .and_then(|m| m.hijo("lvl").map(|l| (l, m)))
    };
    let Some((c, padre)) = simple.or_else(multi) else {
        return (Vec::new(), String::new());
    };
    let formato = c
        .hijo("formatCode")
        .map(|f| f.texto().trim().to_string())
        .unwrap_or_default();
    let cuenta = entero_hijo(padre, "ptCount").unwrap_or(0).min(MAX_PUNTOS);
    let mut v: Vec<Option<String>> = vec![None; cuenta];
    for (k, pt) in c.elementos().filter(|e| e.nombre == "pt").enumerate() {
        let i = pt.atributo("idx").trim().parse::<usize>().unwrap_or(k);
        if i >= MAX_PUNTOS {
            continue;
        }
        if i >= v.len() {
            v.resize(i + 1, None);
        }
        v[i] = Some(pt.hijo("v").map(|x| x.texto()).unwrap_or_default());
    }
    (v, formato)
}

/// Los numeros de una cache: lo que no es un numero queda como hueco.
pub fn numeros(n: &Nodo) -> (Vec<Option<f64>>, String) {
    let (v, f) = cache(n);
    (
        v.into_iter()
            .map(|s| {
                s.and_then(|s| s.trim().parse::<f64>().ok())
                    .filter(|x| x.is_finite())
            })
            .collect(),
        f,
    )
}

fn es_cache_numerica(n: &Nodo) -> bool {
    !n.descendientes("numCache").is_empty() || !n.descendientes("numLit").is_empty()
}

/// El texto de un `c:tx` (nombre de serie): su `c:v` o el primer punto de
/// su cache.
fn texto_de_tx(tx: &Nodo) -> Option<String> {
    if let Some(v) = tx.hijo("v") {
        return Some(v.texto());
    }
    if let Some(r) = tx.hijo("rich") {
        return Some(lineas_rich(r).join(" "));
    }
    cache(tx).0.into_iter().flatten().next()
}

fn lineas_rich(rich: &Nodo) -> Vec<String> {
    rich.elementos()
        .filter(|e| e.nombre == "p")
        .map(|p| {
            p.elementos()
                .filter(|r| r.nombre == "r" || r.nombre == "fld")
                .map(|r| r.hijo("t").map(|t| t.texto()).unwrap_or_default())
                .collect()
        })
        .collect()
}

fn manual(n: Option<&Nodo>) -> Option<Manual> {
    let m = n?.hijo("layout")?.hijo("manualLayout")?;
    let modo = |k: &str| {
        m.hijo(k)
            .map(|x| x.valor().to_string())
            .unwrap_or_else(|| "factor".into())
    };
    if modo("xMode") != "edge" || modo("yMode") != "edge" {
        return None;
    }
    let f = |k: &str| num_hijo(m, k).map(|v| (v as f32).clamp(0.0, 1.0));
    Some(Manual {
        interior: m.hijo("layoutTarget").map(|t| t.valor()) == Some("inner"),
        x: f("x")?,
        y: f("y")?,
        w: f("w"),
        h: f("h"),
    })
}

fn leer_titulo(t: &Nodo, base: &EstiloTexto, factor: f32, ctx: Contexto, defecto: &str) -> Titulo {
    let mut est = base.clone();
    est.tam = (base.tam * factor).round().max(1.0);
    est = estilo_de(t.hijo("txPr"), &est, ctx);
    let mut lineas = Vec::new();
    if let Some(tx) = t.hijo("tx") {
        if let Some(rich) = tx.hijo("rich") {
            est = estilo_de(Some(rich), &est, ctx);
            lineas = lineas_rich(rich);
        } else if let Some(s) = texto_de_tx(tx) {
            lineas.push(s);
        }
    }
    if lineas.iter().all(|l| l.trim().is_empty()) {
        lineas = vec![defecto.to_string()];
    }
    Titulo {
        lineas,
        estilo: est,
        manual: manual(Some(t)),
    }
}

fn leer_etiquetas(d: &Nodo, base: &EstiloTexto, ctx: Contexto) -> Etiquetas {
    let mut estilo = base.clone();
    estilo.color = GRIS_ETIQUETA;
    let estilo_propio = estilo_de(d.hijo("txPr"), &estilo, ctx);
    let color_propio = estilo_propio.color != estilo.color;
    let nf = d.hijo("numFmt");
    let vinculado = nf.is_some_and(|n| matches!(n.atributo("sourceLinked"), "1" | "true"));
    Etiquetas {
        valor: bandera(d, "showVal"),
        porcentaje: bandera(d, "showPercent"),
        categoria: bandera(d, "showCatName"),
        serie: bandera(d, "showSerName"),
        borrar: bandera(d, "delete"),
        posicion: d
            .hijo("dLblPos")
            .map(|p| p.valor().to_string())
            .unwrap_or_default(),
        formato: nf
            .filter(|_| !vinculado)
            .map(|n| n.atributo("formatCode").to_string())
            .filter(|s| !s.is_empty()),
        separador: d
            .hijo("separator")
            .map(|s| s.texto())
            .unwrap_or_else(|| ", ".into()),
        estilo: estilo_propio,
        color_propio,
        borrados: d
            .elementos()
            .filter(|e| e.nombre == "dLbl" && bandera(e, "delete"))
            .filter_map(|e| entero_hijo(e, "idx"))
            .collect(),
    }
}

fn leer_marcador(m: &Nodo, ctx: Contexto) -> Marcador {
    let simbolo = m.hijo("symbol").map(|s| match s.valor() {
        "none" => Simbolo::Ninguno,
        "square" => Simbolo::Cuadrado,
        "diamond" => Simbolo::Rombo,
        "triangle" => Simbolo::Triangulo,
        "x" => Simbolo::Equis,
        "star" => Simbolo::Estrella,
        "dash" => Simbolo::Raya,
        "dot" => Simbolo::Punto,
        "plus" => Simbolo::Mas,
        _ => Simbolo::Circulo,
    });
    let sppr = m.hijo("spPr");
    Marcador {
        simbolo,
        tam: num_hijo(m, "size")
            .map(|v| v as f32)
            .unwrap_or(5.0)
            .clamp(2.0, 72.0),
        relleno: sppr.and_then(|p| dml::relleno(p, ctx, None)),
        linea: sppr
            .and_then(|p| p.hijo("ln"))
            .and_then(|l| dml::linea(l, ctx, None)),
    }
}

fn leer_eje(a: &Nodo, base: &EstiloTexto, ctx: Contexto) -> Eje {
    let valores = a.nombre == "valAx";
    let sc = a.hijo("scaling");
    let nf = a.hijo("numFmt");
    let vinculado = nf.is_some_and(|n| matches!(n.atributo("sourceLinked"), "1" | "true"));
    let linea_de = |sppr: Option<&Nodo>, defecto: Option<Trazo>| -> Option<Trazo> {
        let ln = sppr.and_then(|p| p.hijo("ln"));
        match ln.and_then(|l| dml::linea(l, ctx, None)) {
            Some(t) => t,
            None => {
                let g = ln.and_then(|l| attr_num(l, "w", 1.0)).map(emu);
                defecto.map(|t| Trazo {
                    grosor: g.unwrap_or(t.grosor),
                    ..t
                })
            }
        }
    };
    let cruza = match num_hijo(a, "crossesAt") {
        Some(v) => Cruce::En(v),
        None => match a.hijo("crosses").map(|c| c.valor()) {
            Some("min") => Cruce::Min,
            Some("max") => Cruce::Max,
            _ => Cruce::Cero,
        },
    };
    let estilo = estilo_de(a.hijo("txPr"), base, ctx);
    Eje {
        id: a
            .hijo("axId")
            .map(|i| i.valor().to_string())
            .unwrap_or_default(),
        valores,
        pos: a
            .hijo("axPos")
            .and_then(|p| p.valor().chars().next())
            .unwrap_or(if valores { 'l' } else { 'b' }),
        borrado: bandera(a, "delete"),
        invertido: sc
            .and_then(|s| s.hijo("orientation"))
            .is_some_and(|o| o.valor() == "maxMin"),
        min: sc.and_then(|s| num_hijo(s, "min")),
        max: sc.and_then(|s| num_hijo(s, "max")),
        unidad: num_hijo(a, "majorUnit").filter(|u| *u > 0.0),
        formato: nf
            .filter(|_| !vinculado)
            .map(|n| n.atributo("formatCode").to_string())
            .filter(|s| !s.is_empty()),
        cuadricula: a
            .hijo("majorGridlines")
            .and_then(|m| linea_de(m.hijo("spPr"), Some(Trazo::solido(GRIS_REJILLA, 0.75)))),
        linea: linea_de(a.hijo("spPr"), Some(Trazo::solido(GRIS_EJE, 0.75))),
        pos_etiquetas: a
            .hijo("tickLblPos")
            .map(|p| p.valor().to_string())
            .unwrap_or_else(|| "nextTo".into()),
        marcas: a
            .hijo("majorTickMark")
            .map(|p| p.valor().to_string())
            .unwrap_or_else(|| "out".into()),
        titulo: a
            .hijo("title")
            .map(|t| leer_titulo(t, &estilo, 1.0, ctx, "Título del eje")),
        estilo,
        cruza,
        entre: a
            .hijo("crossBetween")
            .map(|c| c.valor() != "midCat")
            .unwrap_or(true),
    }
}

/// Un color con la luminosidad cambiada (`lumMod`, `lumOff`).
fn luminosidad(c: Color, m: f32, o: f32) -> Color {
    if m == 1.0 && o == 0.0 {
        return c;
    }
    let (h, s, l) = tema::rgb_a_hsl(c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0);
    let (r, g, b) = tema::hsl_a_rgb(h, s, (l * m + o).clamp(0.0, 1.0));
    let q = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color {
        r: q(r),
        g: q(g),
        b: q(b),
        a: c.a,
    }
}

/// Los colores automaticos de Office: accent1..6 y, a partir del septimo,
/// los mismos con las variaciones de `colors1.xml` (mas oscuros, mas claros).
pub fn paleta(ctx: Contexto, n: usize) -> Vec<Color> {
    const VARIACIONES: [(f32, f32); 9] = [
        (1.0, 0.0),
        (0.6, 0.0),
        (0.8, 0.2),
        (0.8, 0.0),
        (0.6, 0.4),
        (0.5, 0.0),
        (0.7, 0.3),
        (0.7, 0.0),
        (0.5, 0.5),
    ];
    (0..n.clamp(6, 256))
        .map(|i| {
            let base = ctx
                .tema
                .esquema(&format!("accent{}", i % 6 + 1), ctx.mapa)
                .unwrap_or(Color::NEGRO);
            let (m, o) = VARIACIONES[(i / 6) % VARIACIONES.len()];
            luminosidad(base, m, o)
        })
        .collect()
}

/// Lee un `c:chartSpace`. `None` si no es un grafico.
pub fn leer(raiz: &Nodo, ctx: Contexto) -> Option<Grafico> {
    if raiz.nombre != "chartSpace" {
        return None;
    }
    let chart = raiz.hijo("chart")?;
    let pa = chart.hijo("plotArea")?;
    let base = EstiloTexto {
        familia: ctx.tema.letra_texto.clone(),
        tam: 10.0,
        negrita: false,
        cursiva: false,
        color: GRIS_TEXTO,
    };
    let base = estilo_de(raiz.hijo("txPr"), &base, ctx);
    let ejes: Vec<Eje> = pa
        .elementos()
        .filter(|e| matches!(e.nombre.as_str(), "catAx" | "valAx" | "dateAx" | "serAx"))
        .map(|a| leer_eje(a, &base, ctx))
        .collect();

    let mut grupos = Vec::new();
    let mut sin_dibujo = Vec::new();
    let mut categorias: Vec<String> = Vec::new();
    let mut vistas = 0usize;
    for gn in pa.elementos().filter(|e| e.nombre.ends_with("Chart")) {
        let agrupacion = match gn.hijo("grouping").map(|g| g.valor()) {
            Some("stacked") => Agrupacion::Apilada,
            Some("percentStacked") => Agrupacion::Porcentaje,
            _ => Agrupacion::Normal,
        };
        let clase = match gn.nombre.as_str() {
            "barChart" | "bar3DChart" => Clase::Barras {
                horizontal: gn.hijo("barDir").is_some_and(|b| b.valor() == "bar"),
            },
            "lineChart" | "line3DChart" => Clase::Lineas,
            "areaChart" | "area3DChart" => Clase::Area,
            "pieChart" | "pie3DChart" | "ofPieChart" => Clase::Circular {
                agujero: 0.0,
                primer_angulo: num_hijo(gn, "firstSliceAng").unwrap_or(0.0) as f32,
            },
            "doughnutChart" => Clase::Circular {
                agujero: (num_hijo(gn, "holeSize").unwrap_or(50.0) as f32 / 100.0).clamp(0.1, 0.9),
                primer_angulo: num_hijo(gn, "firstSliceAng").unwrap_or(0.0) as f32,
            },
            "scatterChart" => Clase::Dispersion,
            "bubbleChart" => Clase::Burbuja,
            "radarChart" => Clase::Radar {
                relleno: gn.hijo("radarStyle").is_some_and(|r| r.valor() == "filled"),
            },
            otro => {
                sin_dibujo.push(otro.to_string());
                continue;
            }
        };
        let ejes_grupo: Vec<String> = gn
            .elementos()
            .filter(|e| e.nombre == "axId")
            .map(|e| e.valor().to_string())
            .collect();
        let mut etq_base = base.clone();
        etq_base.color = GRIS_ETIQUETA;
        let mut series: Vec<(f64, Serie)> = Vec::new();
        for (k, s) in gn
            .elementos()
            .filter(|e| e.nombre == "ser")
            .take(MAX_SERIES)
            .enumerate()
        {
            let indice = entero_hijo(s, "idx").unwrap_or(vistas + k).min(10_000);
            let orden = num_hijo(s, "order").unwrap_or(indice as f64);
            let nombre = s
                .hijo("tx")
                .and_then(texto_de_tx)
                .unwrap_or_else(|| format!("Serie{}", indice + 1));
            let (valores, formato) = s
                .hijo("val")
                .or_else(|| s.hijo("yVal"))
                .map(numeros)
                .unwrap_or_default();
            let xs = if clase.xy() {
                s.hijo("xVal")
                    .filter(|x| es_cache_numerica(x))
                    .map(|x| numeros(x).0)
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            if categorias.is_empty()
                && !clase.xy()
                && let Some(c) = s.hijo("cat")
            {
                categorias = textos_categoria(c, &ejes, &ejes_grupo);
            }
            let sppr = s.hijo("spPr");
            let ln = sppr.and_then(|p| p.hijo("ln"));
            let serie = Serie {
                indice,
                nombre,
                valores,
                xs,
                tamanos: s
                    .hijo("bubbleSize")
                    .map(|b| numeros(b).0)
                    .unwrap_or_default(),
                formato,
                relleno: sppr.and_then(|p| dml::relleno(p, ctx, None)),
                linea: ln.and_then(|l| dml::linea(l, ctx, None)),
                grosor: ln.and_then(|l| attr_num(l, "w", 1.0)).map(emu),
                puntos: s
                    .elementos()
                    .filter(|e| e.nombre == "dPt")
                    .filter_map(|d| {
                        Some((
                            entero_hijo(d, "idx")?,
                            d.hijo("spPr").and_then(|p| dml::relleno(p, ctx, None))?,
                        ))
                    })
                    .collect(),
                marcador: s.hijo("marker").map(|m| leer_marcador(m, ctx)),
                suave: s.hijo("smooth").is_some_and(si),
                etiquetas: s.hijo("dLbls").map(|d| leer_etiquetas(d, &etq_base, ctx)),
                auto: Color::NEGRO,
            };
            series.push((orden, serie));
        }
        vistas += series.len();
        series.sort_by(|a, b| a.0.total_cmp(&b.0));
        let apilada = agrupacion != Agrupacion::Normal;
        grupos.push(Grupo {
            clase,
            agrupacion,
            series: series.into_iter().map(|(_, s)| s).collect(),
            varia_colores: gn
                .hijo("varyColors")
                .map(si)
                .unwrap_or(matches!(clase, Clase::Circular { .. })),
            hueco: (num_hijo(gn, "gapWidth").unwrap_or(150.0) as f32 / 100.0).clamp(0.0, 5.0),
            solape: (num_hijo(gn, "overlap").unwrap_or(if apilada { 100.0 } else { 0.0 }) as f32
                / 100.0)
                .clamp(-1.0, 1.0),
            // Un radar «marker» lleva marcadores sin decirlo en el grupo.
            marcadores: bandera(gn, "marker")
                || gn.hijo("radarStyle").is_some_and(|r| r.valor() == "marker"),
            estilo_dispersion: gn
                .hijo("scatterStyle")
                .map(|s| s.valor().to_string())
                .unwrap_or_default(),
            escala_burbuja: (num_hijo(gn, "bubbleScale").unwrap_or(100.0) as f32 / 100.0)
                .clamp(0.0, 3.0),
            etiquetas: gn.hijo("dLbls").map(|d| leer_etiquetas(d, &etq_base, ctx)),
            ejes: ejes_grupo,
        });
    }
    if grupos.is_empty() && sin_dibujo.is_empty() {
        return None;
    }

    // Los colores automaticos.
    let mas_puntos = grupos
        .iter()
        .flat_map(|g| g.series.iter())
        .map(|s| s.valores.len().max(s.indice + 1))
        .max()
        .unwrap_or(0);
    let paleta = paleta(ctx, mas_puntos.max(categorias.len()));
    for g in &mut grupos {
        for s in &mut g.series {
            s.auto = paleta[s.indice % paleta.len()];
        }
    }

    let total_series: usize = grupos.iter().map(|g| g.series.len()).sum();
    let unica = grupos
        .iter()
        .flat_map(|g| g.series.iter())
        .next()
        .map(|s| s.nombre.clone())
        .unwrap_or_default();
    let borrado = bandera(chart, "autoTitleDeleted");
    let titulo = match chart.hijo("title") {
        Some(t) => {
            let defecto = if total_series == 1 {
                unica.clone()
            } else {
                "Título del gráfico".into()
            };
            Some(leer_titulo(t, &base, 1.4, ctx, &defecto))
        }
        // Sin titulo y sin borrarlo, Office pone el nombre de la unica serie.
        None if !borrado && total_series == 1 => {
            let mut t = leer_titulo(&Nodo::default(), &base, 1.4, ctx, "");
            t.lineas = vec![unica.clone()];
            Some(t)
        }
        None => None,
    };
    let leyenda = chart.hijo("legend").map(|l| Leyenda {
        pos: l
            .hijo("legendPos")
            .map(|p| p.valor().to_string())
            .unwrap_or_else(|| "r".into()),
        manual: manual(Some(l)),
        estilo: estilo_de(l.hijo("txPr"), &base, ctx),
        borradas: l
            .elementos()
            .filter(|e| e.nombre == "legendEntry" && bandera(e, "delete"))
            .filter_map(|e| entero_hijo(e, "idx"))
            .collect(),
    });
    let relleno_de = |sppr: Option<&Nodo>| sppr.and_then(|p| dml::relleno(p, ctx, None)).flatten();
    let borde_de = |sppr: Option<&Nodo>| {
        sppr.and_then(|p| p.hijo("ln"))
            .and_then(|l| dml::linea(l, ctx, None))
            .flatten()
    };
    Some(Grafico {
        titulo,
        leyenda,
        grupos,
        ejes,
        categorias,
        paleta,
        fondo: relleno_de(raiz.hijo("spPr")),
        borde: borde_de(raiz.hijo("spPr")),
        fondo_trazado: relleno_de(pa.hijo("spPr")),
        borde_trazado: borde_de(pa.hijo("spPr")),
        manual_trazado: manual(Some(pa)),
        huecos: match chart.hijo("dispBlanksAs").map(|d| d.valor()) {
            Some("zero") => Huecos::Cero,
            Some("span") => Huecos::Seguir,
            _ => Huecos::Hueco,
        },
        texto: base,
        sin_dibujo,
    })
}

/// Los rotulos de las categorias: textos tal cual, o numeros (fechas) en
/// el formato del eje o de la cache.
fn textos_categoria(c: &Nodo, ejes: &[Eje], ids: &[String]) -> Vec<String> {
    let eje = ejes.iter().find(|e| !e.valores && ids.contains(&e.id));
    if es_cache_numerica(c) {
        let (v, formato) = numeros(c);
        let codigo = eje.and_then(|e| e.formato.clone()).unwrap_or(formato);
        return v
            .into_iter()
            .map(|x| x.map(|x| formatear(x, &codigo, None)).unwrap_or_default())
            .collect();
    }
    cache(c)
        .0
        .into_iter()
        .map(|s| s.unwrap_or_default())
        .collect()
}

// ---------------------------------------------------------------------
// Escalas: el paso «bonito»
// ---------------------------------------------------------------------

/// Un eje de valores: de `min` a `max` de `paso` en `paso`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Escala {
    pub min: f64,
    pub max: f64,
    pub paso: f64,
}

impl Escala {
    /// Los valores de las marcas, de `min` a `max`.
    pub fn marcas(&self) -> Vec<f64> {
        let n = ((self.max - self.min) / self.paso + 1e-6).floor();
        let n = if n.is_finite() && n >= 0.0 {
            (n as usize).min(MAX_MARCAS)
        } else {
            0
        };
        (0..=n)
            .map(|k| {
                // Sin los restos de coma flotante (0,30000000000000004).
                let d = (3 - self.paso.abs().log10().floor() as i32).clamp(0, 15);
                let f = 10f64.powi(d);
                let v = ((self.min + k as f64 * self.paso) * f).round() / f;
                if v == 0.0 { 0.0 } else { v }
            })
            .collect()
    }

    /// Donde cae `v` entre `min` (0) y `max` (1).
    pub fn fraccion(&self, v: f64) -> f64 {
        let d = self.max - self.min;
        if d.abs() < 1e-300 {
            0.5
        } else {
            (v - self.min) / d
        }
    }
}

/// El paso «bonito» mas pequeno que no baja de `bruto`: 1, 2 o 5 por una
/// potencia de diez, como los de Excel.
pub fn paso_bonito(bruto: f64) -> f64 {
    if !(bruto.is_finite() && bruto > 0.0) {
        return 1.0;
    }
    let mag = 10f64.powf(bruto.log10().floor());
    let r = bruto / mag;
    let n = if r <= 1.0 + 1e-9 {
        1.0
    } else if r <= 2.0 + 1e-9 {
        2.0
    } else if r <= 5.0 + 1e-9 {
        5.0
    } else {
        10.0
    };
    n * mag
}

/// La escala de unos datos de `dmin` a `dmax` como la elige Excel: el cero
/// dentro salvo que los datos esten lejos de el (el minimo pasa de 5/6 del
/// maximo), un 5 % de aire por el lado que se aleja del cero, y los extremos
/// redondeados al paso, que da como mucho unas `max_div` divisiones.
/// `min`, `max` y `paso` del archivo mandan sobre lo calculado.
pub fn escala(
    dmin: f64,
    dmax: f64,
    min: Option<f64>,
    max: Option<f64>,
    paso: Option<f64>,
    max_div: usize,
) -> Escala {
    let (mut lo, mut hi) = if dmin.is_finite() && dmax.is_finite() {
        (dmin.min(dmax), dmin.max(dmax))
    } else {
        (0.0, 0.0)
    };
    if lo >= 0.0 {
        if hi == 0.0 || lo < hi * 5.0 / 6.0 {
            lo = 0.0;
        }
    } else if hi <= 0.0 && (lo == 0.0 || hi > lo * 5.0 / 6.0) {
        hi = 0.0;
    }
    // Sin datos o todo ceros: de 0 a 1, sin aire.
    let vacio = lo == 0.0 && hi == 0.0;
    if lo == hi {
        if lo > 0.0 {
            lo = 0.0;
        } else if lo < 0.0 {
            hi = 0.0;
        } else {
            hi = 1.0;
        }
    }
    let span = hi - lo;
    // Lejos del cero, media amplitud de aire hacia el; si no, un 5 %.
    let mut a = if lo > 0.0 {
        (lo - span / 2.0).max(0.0)
    } else if lo < 0.0 {
        lo - 0.05 * span
    } else {
        0.0
    };
    let mut b = if hi < 0.0 {
        (hi + span / 2.0).min(0.0)
    } else if hi > 0.0 && !vacio {
        hi + 0.05 * span
    } else if hi > 0.0 {
        hi
    } else {
        0.0
    };
    if let Some(m) = min.filter(|v| v.is_finite()) {
        a = m;
    }
    if let Some(m) = max.filter(|v| v.is_finite()) {
        b = m;
    }
    if b <= a {
        b = a + if a == 0.0 { 1.0 } else { a.abs() };
    }
    let div = max_div.clamp(1, MAX_MARCAS) as f64;
    let p = paso
        .filter(|p| p.is_finite() && *p > 0.0 && (b - a) / p <= MAX_MARCAS as f64)
        .unwrap_or_else(|| paso_bonito((b - a) / div));
    let lo_r = if min.is_some() {
        a
    } else {
        (a / p + 1e-9).floor() * p
    };
    let mut hi_r = if max.is_some() {
        b
    } else {
        (b / p - 1e-9).ceil() * p
    };
    if hi_r.partial_cmp(&lo_r) != Some(std::cmp::Ordering::Greater) {
        hi_r = lo_r + p;
    }
    Escala {
        min: lo_r,
        max: hi_r,
        paso: p,
    }
}

// ---------------------------------------------------------------------
// Numeros con su formato
// ---------------------------------------------------------------------

/// Un numero con un codigo de formato de Excel (`General`, `0.0`, `#,##0`,
/// `0%`, `"€"#,##0.00`, `0.00E+00`, fechas...). `paso` es el de las marcas de
/// un eje: con `General` dice cuantos decimales hacen falta.
pub fn formatear(v: f64, codigo: &str, paso: Option<f64>) -> String {
    if !v.is_finite() {
        return String::new();
    }
    let codigo = codigo.trim();
    if codigo.is_empty() || codigo.eq_ignore_ascii_case("general") {
        return match paso {
            Some(p) => general_con_paso(v, p),
            None => general(v),
        };
    }
    let secciones = secciones(codigo);
    let (sec, v, signo) = match secciones.len() {
        n if n >= 3 && v == 0.0 => (secciones[2], v, false),
        n if n >= 2 && v < 0.0 => (secciones[1], -v, false),
        _ => (secciones[0], v, true),
    };
    if sec.eq_ignore_ascii_case("general") {
        let s = general(v.abs());
        return if signo && v < 0.0 { format!("-{s}") } else { s };
    }
    if es_fecha(sec) {
        return formatear_fecha(v, sec);
    }
    formatear_numero(v, sec, signo)
}

fn general(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    let a = v.abs();
    if !(1e-9..1e11).contains(&a) {
        let s = format!("{v:.4E}");
        let (m, e) = s.split_once('E').unwrap_or((&s, "0"));
        let m = if m.contains('.') {
            m.trim_end_matches('0').trim_end_matches('.')
        } else {
            m
        };
        let e: i32 = e.parse().unwrap_or(0);
        return format!("{m}E{}{:02}", if e < 0 { '-' } else { '+' }, e.abs());
    }
    let dec = (8 - a.log10().floor() as i32).clamp(0, 10) as usize;
    let s = format!("{v:.dec$}");
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    };
    if s == "-0" { "0".into() } else { s }
}

fn general_con_paso(v: f64, paso: f64) -> String {
    let mut d = 0usize;
    while d < 10
        && ((paso * 10f64.powi(d as i32)).round() - paso * 10f64.powi(d as i32)).abs()
            > 1e-6 * 10f64.powi(d as i32)
    {
        d += 1;
    }
    if v.abs() >= 1e11 {
        return general(v);
    }
    // Como Excel: sin los ceros de sobra (0, 0.5, 1, 1.5).
    let s = format!("{v:.d$}");
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    };
    if s.trim_start_matches('-')
        .chars()
        .all(|c| c == '0' || c == '.')
    {
        "0".to_string()
    } else {
        s
    }
}

/// Parte un codigo por los `;` que no van entre comillas ni corchetes.
fn secciones(codigo: &str) -> Vec<&str> {
    let mut v = Vec::new();
    let (mut ini, mut comillas, mut corchete) = (0, false, false);
    let b = codigo.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'"' if !corchete => comillas = !comillas,
            b'[' if !comillas => corchete = true,
            b']' if !comillas => corchete = false,
            b'\\' if !comillas => i += 1,
            b';' if !comillas && !corchete => {
                v.push(&codigo[ini..i]);
                ini = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    v.push(&codigo[ini.min(codigo.len())..]);
    v
}

/// Los caracteres que cuentan de una seccion (sin textos entre comillas,
/// escapes ni corchetes).
fn esqueleto(sec: &str) -> String {
    let mut s = String::new();
    let mut cs = sec.chars();
    while let Some(c) = cs.next() {
        match c {
            '"' => {
                for d in cs.by_ref() {
                    if d == '"' {
                        break;
                    }
                }
            }
            '\\' | '_' | '*' => {
                cs.next();
            }
            '[' => {
                for d in cs.by_ref() {
                    if d == ']' {
                        break;
                    }
                }
            }
            otro => s.push(otro),
        }
    }
    s
}

fn es_fecha(sec: &str) -> bool {
    let e = esqueleto(sec).to_ascii_lowercase();
    !e.contains('0')
        && !e.contains('#')
        && e.chars().any(|c| matches!(c, 'y' | 'd' | 'm' | 'h' | 's'))
}

fn formatear_numero(v: f64, sec: &str, signo: bool) -> String {
    let (mut pre, mut cuerpo, mut post) = (String::new(), String::new(), String::new());
    let mut en_cuerpo = false;
    let mut tras = false;
    let mut pct = false;
    let mut cs = sec.chars().peekable();
    let literal = |s: &str, en: bool, tras: &mut bool, pre: &mut String, post: &mut String| {
        if en {
            *tras = true;
        }
        if *tras {
            post.push_str(s)
        } else {
            pre.push_str(s)
        }
    };
    while let Some(c) = cs.next() {
        match c {
            '"' => {
                let mut t = String::new();
                for d in cs.by_ref() {
                    if d == '"' {
                        break;
                    }
                    t.push(d);
                }
                literal(&t, en_cuerpo, &mut tras, &mut pre, &mut post);
            }
            '\\' => {
                if let Some(d) = cs.next() {
                    literal(&d.to_string(), en_cuerpo, &mut tras, &mut pre, &mut post);
                }
            }
            '_' => {
                cs.next();
                literal(" ", en_cuerpo, &mut tras, &mut pre, &mut post);
            }
            '*' => {
                cs.next();
            }
            '[' => {
                let mut t = String::new();
                for d in cs.by_ref() {
                    if d == ']' {
                        break;
                    }
                    t.push(d);
                }
                // `[$€-407]`: el simbolo de moneda; `[Red]` y demas se tiran.
                if let Some(r) = t.strip_prefix('$') {
                    literal(
                        r.split('-').next().unwrap_or(""),
                        en_cuerpo,
                        &mut tras,
                        &mut pre,
                        &mut post,
                    );
                }
            }
            '%' => {
                pct = true;
                literal("%", en_cuerpo, &mut tras, &mut pre, &mut post);
            }
            '0' | '#' | '?' | '.' | ',' if !tras => {
                en_cuerpo = true;
                cuerpo.push(c);
            }
            'E' | 'e' if en_cuerpo && !tras && matches!(cs.peek(), Some('+' | '-')) => {
                cuerpo.push('E');
                if let Some(d) = cs.next() {
                    cuerpo.push(d);
                }
            }
            otro => literal(&otro.to_string(), en_cuerpo, &mut tras, &mut pre, &mut post),
        }
    }
    if cuerpo.is_empty() {
        return format!("{pre}{post}");
    }
    let mut v = if pct { v * 100.0 } else { v };
    // Las comas del final dividen por mil cada una.
    let sin_escala = cuerpo.trim_end_matches(',');
    for _ in 0..(cuerpo.len() - sin_escala.len()) {
        v /= 1000.0;
    }
    let cuerpo = sin_escala;
    let negativo = signo && v < 0.0;
    let a = v.abs();
    let numero = if let Some((mant, exp)) = cuerpo.split_once('E') {
        let dec = mant
            .split_once('.')
            .map(|(_, d)| d.chars().filter(|c| matches!(c, '0' | '#' | '?')).count())
            .unwrap_or(0);
        let digitos = exp
            .chars()
            .filter(|c| matches!(c, '0' | '#'))
            .count()
            .max(1);
        let s = format!("{a:.dec$e}");
        let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
        let e: i32 = e.parse().unwrap_or(0);
        let mas = exp.starts_with('+');
        format!(
            "{m}E{}{:0digitos$}",
            if e < 0 {
                "-"
            } else if mas {
                "+"
            } else {
                ""
            },
            e.abs()
        )
    } else {
        let (ent, frac) = cuerpo.split_once('.').unwrap_or((cuerpo, ""));
        let max_dec = frac
            .chars()
            .filter(|c| matches!(c, '0' | '#' | '?'))
            .count()
            .min(12);
        let min_dec = frac.chars().filter(|c| *c == '0').count().min(max_dec);
        // Redondeo de Excel: los medios, lejos del cero.
        let f = 10f64.powi(max_dec as i32);
        let s = format!("{:.max_dec$}", (a * f).round() / f);
        let (ip, fp) = s
            .split_once('.')
            .map(|(x, y)| (x.to_string(), y.to_string()))
            .unwrap_or((s.clone(), String::new()));
        let mut fp = fp;
        while fp.len() > min_dec && fp.ends_with('0') {
            fp.pop();
        }
        let ip = if ip == "0" && !ent.contains('0') && (!fp.is_empty() || a != 0.0) {
            String::new()
        } else {
            ip
        };
        let ip = if ent.contains(',') { miles(&ip) } else { ip };
        if fp.is_empty() {
            ip
        } else {
            format!("{ip}.{fp}")
        }
    };
    let cero = !numero.chars().any(|c| c.is_ascii_digit() && c != '0');
    let menos = if negativo && !cero { "-" } else { "" };
    format!("{menos}{pre}{numero}{post}")
}

fn miles(ip: &str) -> String {
    let n = ip.len();
    let mut s = String::with_capacity(n + n / 3);
    for (k, c) in ip.chars().enumerate() {
        if k > 0 && (n - k) % 3 == 0 {
            s.push(',');
        }
        s.push(c);
    }
    s
}

/// Un numero de serie de Excel (dias desde 1900, con su 29 de febrero que
/// no existio) a año, mes y dia.
fn fecha(serial: f64) -> (i64, u32, u32) {
    let dias = serial.floor() as i64;
    // De serie a dias desde 1970-01-01.
    let z = (if dias < 61 {
        dias - 25568
    } else {
        dias - 25569
    }) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    (y, m, d)
}

const MESES: [&str; 12] = [
    "enero",
    "febrero",
    "marzo",
    "abril",
    "mayo",
    "junio",
    "julio",
    "agosto",
    "septiembre",
    "octubre",
    "noviembre",
    "diciembre",
];
const DIAS: [&str; 7] = [
    "domingo",
    "lunes",
    "martes",
    "miércoles",
    "jueves",
    "viernes",
    "sábado",
];

fn formatear_fecha(v: f64, sec: &str) -> String {
    if !(0.0..2_958_466.0).contains(&v) {
        return general(v);
    }
    let (y, mes, d) = fecha(v);
    let segundos = ((v - v.floor()) * 86_400.0).round() as i64;
    let (h, mi, s) = (segundos / 3600 % 24, segundos / 60 % 60, segundos % 60);
    let dia_semana = ((v.floor() as i64 + 6).rem_euclid(7)) as usize;
    let cs: Vec<char> = sec.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let mut tras_hora = false;
    while i < cs.len() {
        let c = cs[i];
        let l = c.to_ascii_lowercase();
        match c {
            '"' => {
                i += 1;
                while i < cs.len() && cs[i] != '"' {
                    out.push(cs[i]);
                    i += 1;
                }
                i += 1;
                continue;
            }
            '\\' => {
                if let Some(d) = cs.get(i + 1) {
                    out.push(*d);
                }
                i += 2;
                continue;
            }
            '[' => {
                while i < cs.len() && cs[i] != ']' {
                    i += 1;
                }
                i += 1;
                continue;
            }
            '_' | '*' => {
                i += 2;
                continue;
            }
            _ => {}
        }
        if matches!(l, 'y' | 'm' | 'd' | 'h' | 's') {
            let mut n = 0;
            while i + n < cs.len() && cs[i + n].to_ascii_lowercase() == l {
                n += 1;
            }
            let siguiente_s = cs[i + n..]
                .iter()
                .find(|c| c.is_ascii_alphabetic())
                .is_some_and(|c| c.eq_ignore_ascii_case(&'s'));
            match l {
                'y' => out.push_str(&if n <= 2 {
                    format!("{:02}", y.rem_euclid(100))
                } else {
                    y.to_string()
                }),
                'm' if tras_hora || siguiente_s => out.push_str(&if n >= 2 {
                    format!("{mi:02}")
                } else {
                    mi.to_string()
                }),
                'm' => {
                    let nombre = MESES[(mes as usize).saturating_sub(1).min(11)];
                    match n {
                        1 => out.push_str(&mes.to_string()),
                        2 => out.push_str(&format!("{mes:02}")),
                        3 => out.extend(nombre.chars().take(3)),
                        4 => out.push_str(nombre),
                        _ => out.extend(nombre.chars().take(1)),
                    }
                }
                'd' => match n {
                    1 => out.push_str(&d.to_string()),
                    2 => out.push_str(&format!("{d:02}")),
                    3 => out.extend(DIAS[dia_semana].chars().take(3)),
                    _ => out.push_str(DIAS[dia_semana]),
                },
                'h' => out.push_str(&if n >= 2 {
                    format!("{h:02}")
                } else {
                    h.to_string()
                }),
                _ => out.push_str(&if n >= 2 {
                    format!("{s:02}")
                } else {
                    s.to_string()
                }),
            }
            tras_hora = l == 'h';
            i += n;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

// ---------------------------------------------------------------------
// El pintor
// ---------------------------------------------------------------------

/// Escribe y mide textos con el medidor de verdad.
struct Pincel<'a> {
    m: &'a dyn Medidor,
}

impl Pincel<'_> {
    fn ancho(&self, s: &str, e: &EstiloTexto) -> f32 {
        self.m.ancho(s, &e.letra())
    }

    /// Ascenso y descenso.
    fn vertical(&self, e: &EstiloTexto) -> (f32, f32) {
        let mt = self.m.metricas(&e.letra());
        (mt.ascenso, mt.descenso)
    }

    fn renglon(&self, e: &EstiloTexto) -> f32 {
        let (a, d) = self.vertical(e);
        a + d
    }

    /// Un texto con su caja anclada en (x, y): `ah` 0 izquierda, 0,5
    /// centro, 1 derecha; `av` 0 arriba, 0,5 centro, 1 abajo.
    fn texto(
        &self,
        s: &str,
        e: &EstiloTexto,
        x: f32,
        y: f32,
        ah: f32,
        av: f32,
    ) -> Option<(Elemento, Rect)> {
        if s.is_empty() {
            return None;
        }
        let letra = e.letra();
        let w = self.m.ancho(s, &letra);
        let mt = self.m.metricas(&letra);
        let h = mt.ascenso + mt.descenso;
        let (x0, y0) = (x - w * ah, y - h * av);
        let t = Texto {
            x: x0,
            y: y0 + mt.ascenso,
            letra,
            color: e.color,
            texto: s.to_string(),
            subrayado: None,
            tachado: false,
            espaciado: 0.0,
            extra_espacio: 0.0,
            ancho: w,
            campo: None,
            enlace: None,
        };
        Some((Elemento::Texto(t), Rect::nuevo(x0, y0, w, h)))
    }

    /// Recorta `s` con «…» para que quepa en `max`.
    fn recortar(&self, s: &str, e: &EstiloTexto, max: f32) -> String {
        if self.ancho(s, e) <= max {
            return s.to_string();
        }
        let cs: Vec<char> = s.chars().collect();
        for n in (0..cs.len()).rev() {
            let t: String = cs[..n].iter().collect::<String>() + "…";
            if self.ancho(&t, e) <= max || n == 0 {
                return t;
            }
        }
        String::new()
    }

    /// Parte un texto en renglones de `max` de ancho por los espacios.
    fn partir(&self, s: &str, e: &EstiloTexto, max: f32) -> Vec<String> {
        let mut lineas: Vec<String> = Vec::new();
        let mut actual = String::new();
        for p in s.split_whitespace() {
            let prueba = if actual.is_empty() {
                p.to_string()
            } else {
                format!("{actual} {p}")
            };
            if self.ancho(&prueba, e) <= max || actual.is_empty() {
                actual = prueba;
            } else {
                lineas.push(std::mem::take(&mut actual));
                actual = p.to_string();
            }
        }
        if !actual.is_empty() || lineas.is_empty() {
            lineas.push(actual);
        }
        lineas
    }
}

/// Una recta de un eje: donde cae la fraccion 0 y donde la 1.
#[derive(Debug, Clone, Copy)]
struct Recta {
    a: f32,
    b: f32,
}

impl Recta {
    fn en(&self, t: f64) -> f32 {
        self.a + (self.b - self.a) * t as f32
    }
}

fn figura(inicio: (f32, f32), tramos: Vec<Tramo>, cerrada: bool) -> Figura {
    Figura {
        inicio,
        tramos,
        cerrada,
        rellenar: cerrada,
        trazar: true,
        ..Default::default()
    }
}

fn poligono(p: &[(f32, f32)], cerrada: bool) -> Option<Figura> {
    let (primero, resto) = p.split_first()?;
    Some(figura(
        *primero,
        resto.iter().map(|q| Tramo::Linea(q.0, q.1)).collect(),
        cerrada,
    ))
}

/// Un arco de circunferencia de `a0` a `a1` (radianes, en el sentido del
/// reloj en pantalla) en beziers de un cuarto de vuelta como mucho. El
/// punto de partida ya debe ser el actual.
fn arco(cx: f32, cy: f32, r: f32, a0: f32, a1: f32, v: &mut Vec<Tramo>) {
    let total = a1 - a0;
    let n = ((total.abs() / FRAC_PI_2).ceil() as usize).max(1);
    let d = total / n as f32;
    let k = 4.0 / 3.0 * (d / 4.0).tan();
    for i in 0..n {
        let t0 = a0 + d * i as f32;
        let t1 = t0 + d;
        let (s0, c0) = t0.sin_cos();
        let (s1, c1) = t1.sin_cos();
        let p0 = (cx + r * c0, cy + r * s0);
        let p3 = (cx + r * c1, cy + r * s1);
        let p1 = (p0.0 - k * r * s0, p0.1 + k * r * c0);
        let p2 = (p3.0 + k * r * s1, p3.1 - k * r * c1);
        v.push(Tramo::Bezier(p1, p2, p3));
    }
}

fn circulo(cx: f32, cy: f32, r: f32) -> Figura {
    let mut t = Vec::new();
    arco(cx, cy, r, 0.0, TAU, &mut t);
    figura((cx + r, cy), t, true)
}

/// Une puntos con una curva suave (Catmull-Rom en beziers).
fn suave(p: &[(f32, f32)]) -> Vec<Tramo> {
    let mut v = Vec::new();
    for i in 0..p.len().saturating_sub(1) {
        let p0 = if i == 0 { p[i] } else { p[i - 1] };
        let (p1, p2) = (p[i], p[i + 1]);
        let p3 = p.get(i + 2).copied().unwrap_or(p2);
        let c1 = (p1.0 + (p2.0 - p0.0) / 6.0, p1.1 + (p2.1 - p0.1) / 6.0);
        let c2 = (p2.0 - (p3.0 - p1.0) / 6.0, p2.1 - (p3.1 - p1.1) / 6.0);
        v.push(Tramo::Bezier(c1, c2, p2));
    }
    v
}

/// La figura de un marcador centrado en (x, y) de `t` puntos, y si se
/// rellena.
fn figura_marcador(s: Simbolo, x: f32, y: f32, t: f32) -> Vec<Figura> {
    let r = t / 2.0;
    let abierta = |a: (f32, f32), b: (f32, f32)| Figura {
        inicio: a,
        tramos: vec![Tramo::Linea(b.0, b.1)],
        cerrada: false,
        rellenar: false,
        trazar: true,
        ..Default::default()
    };
    match s {
        Simbolo::Ninguno => Vec::new(),
        Simbolo::Circulo => vec![circulo(x, y, r)],
        Simbolo::Punto => vec![circulo(x, y, r * 0.5)],
        Simbolo::Cuadrado => poligono(
            &[
                (x - r, y - r),
                (x + r, y - r),
                (x + r, y + r),
                (x - r, y + r),
            ],
            true,
        )
        .into_iter()
        .collect(),
        Simbolo::Rombo => poligono(&[(x, y - r), (x + r, y), (x, y + r), (x - r, y)], true)
            .into_iter()
            .collect(),
        Simbolo::Triangulo => poligono(&[(x, y - r), (x + r, y + r), (x - r, y + r)], true)
            .into_iter()
            .collect(),
        Simbolo::Raya => poligono(
            &[
                (x - r, y - r * 0.25),
                (x + r, y - r * 0.25),
                (x + r, y + r * 0.25),
                (x - r, y + r * 0.25),
            ],
            true,
        )
        .into_iter()
        .collect(),
        Simbolo::Equis => vec![
            abierta((x - r, y - r), (x + r, y + r)),
            abierta((x - r, y + r), (x + r, y - r)),
        ],
        Simbolo::Mas => vec![
            abierta((x - r, y), (x + r, y)),
            abierta((x, y - r), (x, y + r)),
        ],
        Simbolo::Estrella => {
            vec![
                abierta((x - r, y - r), (x + r, y + r)),
                abierta((x - r, y + r), (x + r, y - r)),
                abierta((x, y - r), (x, y + r)),
            ]
        }
    }
}

/// Lo que se pinta en la leyenda delante de cada nombre.
#[derive(Debug, Clone)]
enum Muestra {
    Caja(Option<Relleno>, Option<Trazo>),
    Raya(
        Option<Trazo>,
        Option<(Simbolo, Option<Relleno>, Option<Trazo>)>,
    ),
}

fn relleno_serie(s: &Serie) -> Option<Relleno> {
    match &s.relleno {
        Some(r) => r.clone(),
        None => Some(Relleno::Solido(s.auto)),
    }
}

fn trazo_serie(s: &Serie) -> Option<Trazo> {
    let g = s.grosor.unwrap_or(GROSOR_LINEA).max(0.25);
    match s.linea {
        Some(None) => None,
        Some(Some(t)) => Some(Trazo { grosor: g, ..t }),
        None => Some(Trazo::solido(s.auto, g)),
    }
}

/// El relleno del punto `i`: el suyo propio, el de su color si el grupo
/// varia colores, o el de la serie.
fn relleno_punto(g: &Grafico, gr: &Grupo, s: &Serie, i: usize) -> Option<Relleno> {
    if let Some((_, r)) = s.puntos.iter().find(|(k, _)| *k == i) {
        return r.clone();
    }
    if gr.varia_colores
        && (gr.series.len() == 1 || matches!(gr.clase, Clase::Circular { .. }))
        && s.relleno.is_none()
    {
        return Some(Relleno::Solido(g.paleta[i % g.paleta.len()]));
    }
    relleno_serie(s)
}

/// El marcador de una serie, si lleva: (simbolo, tamano, relleno, borde).
fn marcador_de(gr: &Grupo, s: &Serie) -> Option<(Simbolo, f32, Option<Relleno>, Option<Trazo>)> {
    let por_defecto = match gr.clase {
        Clase::Lineas => gr.marcadores,
        Clase::Dispersion => true,
        Clase::Radar { relleno: false } => gr.marcadores,
        _ => false,
    };
    let m = s.marcador.as_ref();
    let simbolo = match m.and_then(|m| m.simbolo) {
        Some(Simbolo::Ninguno) => return None,
        Some(sim) => sim,
        None if por_defecto => Simbolo::Circulo,
        None => return None,
    };
    let tam = m.map(|m| m.tam).unwrap_or(5.0);
    let color = trazo_serie(s).map(|t| t.color).unwrap_or(s.auto);
    let relleno = match m.and_then(|m| m.relleno.clone()) {
        Some(r) => r,
        None => Some(Relleno::Solido(color)),
    };
    let borde = match m.and_then(|m| m.linea) {
        Some(t) => t,
        None => Some(Trazo::solido(color, 0.75)),
    };
    Some((simbolo, tam, relleno, borde))
}

fn etiquetas_de<'a>(gr: &'a Grupo, s: &'a Serie) -> Option<&'a Etiquetas> {
    let e = s.etiquetas.as_ref().or(gr.etiquetas.as_ref())?;
    e.alguna().then_some(e)
}

/// El texto de la etiqueta de un punto.
fn texto_etiqueta(e: &Etiquetas, s: &Serie, cat: &str, v: f64, parte: Option<f64>) -> String {
    let mut partes: Vec<String> = Vec::new();
    if e.serie {
        partes.push(s.nombre.clone());
    }
    if e.categoria && !cat.is_empty() {
        partes.push(cat.to_string());
    }
    if e.valor {
        let f = if e.porcentaje {
            s.formato.as_str()
        } else {
            e.formato.as_deref().unwrap_or(&s.formato)
        };
        partes.push(formatear(v, f, None));
    }
    if e.porcentaje
        && let Some(p) = parte
    {
        partes.push(formatear(
            p,
            e.formato
                .as_deref()
                .filter(|f| f.contains('%'))
                .unwrap_or("0%"),
            None,
        ));
    }
    partes.join(&e.separador)
}

/// Blanco o casi negro, el que mas contraste sobre `fondo`.
fn contraste(fondo: Option<&Relleno>) -> Color {
    let Some(c) = fondo.map(|r| r.color_medio()) else {
        return GRIS_ETIQUETA;
    };
    let l = 0.299 * c.r as f32 + 0.587 * c.g as f32 + 0.114 * c.b as f32;
    if l < 140.0 {
        Color::BLANCO
    } else {
        GRIS_ETIQUETA
    }
}

/// Pinta un grafico en una caja de `ancho` x `alto` puntos con origen
/// (0, 0). Todo va dentro de un grupo recortado a la caja.
pub fn pintar(g: &Grafico, ancho: f32, alto: f32, medidor: &dyn Medidor) -> Vec<Elemento> {
    if !(ancho >= 4.0 && alto >= 4.0) {
        return Vec::new();
    }
    let mut d = Dibujo {
        p: Pincel { m: medidor },
        els: Vec::new(),
        encima: Vec::new(),
        ancho,
        alto,
    };
    if g.fondo.is_some() || g.borde.is_some() {
        d.els.push(Elemento::Rect {
            rect: Rect::nuevo(0.0, 0.0, ancho, alto),
            relleno: g.fondo.clone(),
            borde: g.borde,
        });
    }
    let mut area = Rect::nuevo(
        MARGEN,
        MARGEN,
        (ancho - 2.0 * MARGEN).max(1.0),
        (alto - 2.0 * MARGEN).max(1.0),
    );
    if let Some(t) = &g.titulo {
        d.titulo(t, &mut area);
    }
    if let Some(l) = &g.leyenda {
        let entradas = entradas_leyenda(g, l);
        if !entradas.is_empty() {
            d.leyenda(l, &entradas, &mut area);
        }
    }
    let circular = g
        .grupos
        .iter()
        .find(|x| matches!(x.clase, Clase::Circular { .. }));
    let radar = g
        .grupos
        .iter()
        .find(|x| matches!(x.clase, Clase::Radar { .. }));
    let con_ejes = g.grupos.iter().any(|x| x.clase.cartesiana());
    // El trazado puesto a mano (el interior de uno con ejes lo coloca el
    // propio dibujo, con las etiquetas por fuera).
    if let Some(m) = g.manual_trazado
        && (!m.interior || !con_ejes)
    {
        area = d.caja_manual(m);
    }
    if con_ejes {
        d.cartesiano(g, area);
    } else if let Some(gr) = circular {
        d.circular(g, gr, area);
    } else if let Some(gr) = radar {
        d.radar(g, gr, area);
    } else {
        let nombres = g.sin_dibujo.join(", ");
        let estilo = EstiloTexto {
            color: GRIS_TEXTO,
            ..g.texto.clone()
        };
        let texto = format!("Gráfico sin vista previa ({nombres})");
        let texto = d.p.recortar(&texto, &estilo, area.ancho);
        if let Some((e, _)) = d.p.texto(
            &texto,
            &estilo,
            area.x + area.ancho / 2.0,
            area.y + area.alto / 2.0,
            0.5,
            0.5,
        ) {
            d.els.push(e);
        }
    }
    let Dibujo {
        mut els, encima, ..
    } = d;
    els.extend(encima);
    vec![Elemento::Grupo {
        transformada: IDENTIDAD,
        recorte: Some(Rect::nuevo(0.0, 0.0, ancho, alto)),
        elementos: els,
    }]
}

fn entradas_leyenda(g: &Grafico, l: &Leyenda) -> Vec<(String, Muestra)> {
    let mut v = Vec::new();
    // Un circular, o una sola serie que varia colores: una entrada por
    // categoria.
    let por_puntos = g.grupos.first().filter(|gr| {
        matches!(gr.clase, Clase::Circular { .. })
            || (gr.varia_colores && g.grupos.len() == 1 && gr.series.len() == 1 && !gr.clase.xy())
    });
    if let Some(gr) = por_puntos {
        let Some(s) = gr.series.first() else { return v };
        let n = g.categorias.len().max(s.valores.len());
        for i in 0..n.min(MAX_PUNTOS) {
            let nombre = g
                .categorias
                .get(i)
                .cloned()
                .filter(|c| !c.is_empty())
                .unwrap_or_else(|| (i + 1).to_string());
            let borde = if matches!(gr.clase, Clase::Circular { .. }) {
                None
            } else {
                s.linea.flatten()
            };
            v.push((nombre, Muestra::Caja(relleno_punto(g, gr, s, i), borde)));
        }
    } else {
        for gr in &g.grupos {
            for s in &gr.series {
                let m = if gr.clase.muestra_caja() {
                    let borde = if matches!(gr.clase, Clase::Burbuja) {
                        None
                    } else {
                        s.linea.flatten()
                    };
                    Muestra::Caja(relleno_serie(s), borde)
                } else {
                    let raya = if matches!(gr.clase, Clase::Dispersion)
                        && gr.estilo_dispersion == "marker"
                    {
                        None
                    } else {
                        trazo_serie(s)
                    };
                    Muestra::Raya(raya, marcador_de(gr, s).map(|(sim, _, r, b)| (sim, r, b)))
                };
                v.push((s.nombre.clone(), m));
            }
        }
    }
    v.into_iter()
        .enumerate()
        .filter(|(i, _)| !l.borradas.contains(i))
        .map(|(_, e)| e)
        .collect()
}

struct Dibujo<'a> {
    p: Pincel<'a>,
    els: Vec<Elemento>,
    /// Lo que va por encima de todo (etiquetas de datos).
    encima: Vec<Elemento>,
    ancho: f32,
    alto: f32,
}

impl Dibujo<'_> {
    fn poner(&mut self, e: Option<(Elemento, Rect)>) -> Option<Rect> {
        let (e, r) = e?;
        self.els.push(e);
        Some(r)
    }

    fn caja_manual(&self, m: Manual) -> Rect {
        let w = m.w.unwrap_or(1.0 - m.x).max(0.01);
        let h = m.h.unwrap_or(1.0 - m.y).max(0.01);
        Rect::nuevo(
            m.x * self.ancho,
            m.y * self.alto,
            w * self.ancho,
            h * self.alto,
        )
    }

    fn titulo(&mut self, t: &Titulo, area: &mut Rect) {
        let lh = self.p.renglon(&t.estilo);
        let alto = lh * t.lineas.len() as f32;
        let (x, y, centrar) = match t.manual {
            Some(m) => (m.x * self.ancho, m.y * self.alto, false),
            None => (self.ancho / 2.0, area.y, true),
        };
        for (k, l) in t.lineas.iter().enumerate() {
            let l = self.p.recortar(l, &t.estilo, area.ancho);
            let e = self.p.texto(
                &l,
                &t.estilo,
                x,
                y + lh * k as f32,
                if centrar { 0.5 } else { 0.0 },
                0.0,
            );
            self.poner(e);
        }
        if t.manual.is_none() {
            let usado = alto + 6.0;
            area.y += usado;
            area.alto = (area.alto - usado).max(1.0);
        }
    }

    fn muestra(&mut self, m: &Muestra, x: f32, y_centro: f32, lado: f32) -> f32 {
        match m {
            Muestra::Caja(relleno, borde) => {
                self.els.push(Elemento::Rect {
                    rect: Rect::nuevo(x, y_centro - lado / 2.0, lado, lado),
                    relleno: relleno.clone(),
                    borde: *borde,
                });
                lado
            }
            Muestra::Raya(trazo, marca) => {
                let w = lado * 2.6;
                if let Some(t) = trazo {
                    self.els.push(Elemento::Linea {
                        desde: (x, y_centro),
                        hasta: (x + w, y_centro),
                        trazo: Trazo {
                            grosor: t.grosor.min(3.0),
                            ..*t
                        },
                    });
                }
                if let Some((sim, relleno, borde)) = marca {
                    let figuras = figura_marcador(*sim, x + w / 2.0, y_centro, lado.min(7.0));
                    if !figuras.is_empty() {
                        self.els.push(Elemento::Trayecto {
                            figuras,
                            relleno: relleno.clone(),
                            borde: *borde,
                        });
                    }
                }
                w
            }
        }
    }

    fn leyenda(&mut self, l: &Leyenda, entradas: &[(String, Muestra)], area: &mut Rect) {
        let lh = self.p.renglon(&l.estilo);
        let lado = (l.estilo.tam * 0.7).clamp(4.0, 12.0);
        let ancho_muestra = |m: &Muestra| {
            if matches!(m, Muestra::Raya(..)) {
                lado * 2.6
            } else {
                lado
            }
        };
        let max_texto = area.ancho * 0.45;
        let items: Vec<(String, f32)> = entradas
            .iter()
            .map(|(t, m)| {
                let t = self.p.recortar(t, &l.estilo, max_texto);
                let w = ancho_muestra(m) + 4.0 + self.p.ancho(&t, &l.estilo);
                (t, w)
            })
            .collect();
        let vertical = matches!(l.pos.as_str(), "r" | "l" | "tr");
        let caja = l.manual.map(|m| self.caja_manual(m));
        if vertical {
            let ancho = items.iter().map(|i| i.1).fold(0.0, f32::max);
            let alto = lh * items.len() as f32;
            let (x, y0) = match caja {
                Some(c) => (c.x, c.y),
                None => {
                    let x = if l.pos == "l" {
                        area.x
                    } else {
                        area.derecha() - ancho
                    };
                    let y = if l.pos == "tr" {
                        area.y
                    } else {
                        area.y + (area.alto - alto).max(0.0) / 2.0
                    };
                    (x, y)
                }
            };
            for (k, ((t, _), (_, m))) in items.iter().zip(entradas).enumerate() {
                let yc = y0 + lh * (k as f32 + 0.5);
                if yc > self.alto {
                    break;
                }
                let w = self.muestra(m, x, yc, lado);
                let e = self.p.texto(t, &l.estilo, x + w + 4.0, yc, 0.0, 0.5);
                self.poner(e);
            }
            if caja.is_none() {
                let usado = ancho + 10.0;
                if l.pos == "l" {
                    area.x += usado;
                }
                area.ancho = (area.ancho - usado).max(1.0);
            }
        } else {
            // En filas centradas, partiendo cuando no caben.
            let limite = caja.map(|c| c.ancho).unwrap_or(area.ancho);
            let mut filas: Vec<Vec<usize>> = vec![Vec::new()];
            let mut ocupado = 0.0;
            for (k, (_, w)) in items.iter().enumerate() {
                let fila = filas.last_mut().expect("siempre hay una fila");
                if !fila.is_empty() && ocupado + 10.0 + w > limite {
                    filas.push(vec![k]);
                    ocupado = *w;
                } else {
                    ocupado += if fila.is_empty() { *w } else { 10.0 + w };
                    fila.push(k);
                }
            }
            let max_filas = ((area.alto / 3.0) / lh).floor().max(1.0) as usize;
            filas.truncate(max_filas);
            let alto = lh * filas.len() as f32;
            let (x0, y0, w0) = match caja {
                Some(c) => (c.x, c.y, c.ancho),
                None => (
                    area.x,
                    if l.pos == "t" {
                        area.y
                    } else {
                        area.abajo() - alto
                    },
                    area.ancho,
                ),
            };
            for (f, fila) in filas.iter().enumerate() {
                let ancho_fila: f32 = fila.iter().map(|k| items[*k].1).sum::<f32>()
                    + 10.0 * fila.len().saturating_sub(1) as f32;
                let mut x = x0 + (w0 - ancho_fila).max(0.0) / 2.0;
                let yc = y0 + lh * (f as f32 + 0.5);
                for k in fila {
                    let w = self.muestra(&entradas[*k].1, x, yc, lado);
                    let e = self
                        .p
                        .texto(&items[*k].0, &l.estilo, x + w + 4.0, yc, 0.0, 0.5);
                    self.poner(e);
                    x += items[*k].1 + 10.0;
                }
            }
            if caja.is_none() {
                let usado = alto + 6.0;
                if l.pos == "t" {
                    area.y += usado;
                }
                area.alto = (area.alto - usado).max(1.0);
            }
        }
    }

    /// Un texto girado 90° (se lee de abajo arriba) centrado en (x, y).
    fn texto_vertical(&mut self, s: &str, e: &EstiloTexto, x: f32, y: f32) {
        let Some((t, _)) = self.p.texto(s, e, 0.0, 0.0, 0.5, 0.5) else {
            return;
        };
        self.els.push(Elemento::Grupo {
            transformada: [0.0, -1.0, 1.0, 0.0, x, y],
            recorte: None,
            elementos: vec![t],
        });
    }

    // -----------------------------------------------------------------
    // Circular y anillo
    // -----------------------------------------------------------------

    fn circular(&mut self, g: &Grafico, gr: &Grupo, area: Rect) {
        let Clase::Circular {
            agujero,
            primer_angulo,
        } = gr.clase
        else {
            return;
        };
        let series: Vec<&Serie> = if agujero > 0.0 {
            gr.series.iter().collect()
        } else {
            gr.series.iter().take(1).collect()
        };
        if series.is_empty() {
            return;
        }
        let fuera = series
            .iter()
            .any(|s| etiquetas_de(gr, s).is_some_and(|e| e.posicion == "outEnd"));
        let mut r = area.ancho.min(area.alto) / 2.0;
        if fuera {
            let lh = series
                .iter()
                .filter_map(|s| etiquetas_de(gr, s))
                .map(|e| self.p.renglon(&e.estilo))
                .fold(0.0, f32::max);
            r = (r - lh * 1.6).max(r * 0.5);
        }
        let r = r.max(1.0);
        let (cx, cy) = (area.x + area.ancho / 2.0, area.y + area.alto / 2.0);
        let r_hueco = r * agujero;
        let grueso = (r - r_hueco) / series.len() as f32;
        let inicio = -FRAC_PI_2 + primer_angulo.to_radians();
        for (k, s) in series.iter().enumerate() {
            let (ri, ro) = if agujero > 0.0 {
                (
                    r_hueco + grueso * k as f32,
                    r_hueco + grueso * (k + 1) as f32,
                )
            } else {
                (0.0, r)
            };
            let vals: Vec<f64> = s
                .valores
                .iter()
                .map(|v| v.map(f64::abs).unwrap_or(0.0))
                .collect();
            let total: f64 = vals.iter().sum();
            if total.is_nan() || total <= 0.0 {
                continue;
            }
            let mut a0 = inicio;
            let etq = etiquetas_de(gr, s);
            for (i, v) in vals.iter().enumerate() {
                if *v <= 0.0 {
                    continue;
                }
                let barrido = (*v / total) as f32 * TAU;
                let a1 = a0 + barrido;
                let mut tramos = Vec::new();
                let fig = if ri <= 0.0 {
                    let p0 = (cx + ro * a0.cos(), cy + ro * a0.sin());
                    tramos.push(Tramo::Linea(p0.0, p0.1));
                    arco(cx, cy, ro, a0, a1, &mut tramos);
                    figura((cx, cy), tramos, true)
                } else {
                    arco(cx, cy, ro, a0, a1, &mut tramos);
                    let pi = (cx + ri * a1.cos(), cy + ri * a1.sin());
                    tramos.push(Tramo::Linea(pi.0, pi.1));
                    arco(cx, cy, ri, a1, a0, &mut tramos);
                    figura((cx + ro * a0.cos(), cy + ro * a0.sin()), tramos, true)
                };
                let relleno = relleno_punto(g, gr, s, i);
                self.els.push(Elemento::Trayecto {
                    figuras: vec![fig],
                    relleno: relleno.clone(),
                    borde: s.linea.flatten(),
                });
                if let Some(e) = etq
                    && !e.borrados.contains(&i)
                {
                    let cat = g.categorias.get(i).map(|c| c.as_str()).unwrap_or("");
                    let texto =
                        texto_etiqueta(e, s, cat, s.valores[i].unwrap_or(0.0), Some(*v / total));
                    let medio = a0 + barrido / 2.0;
                    let ancho_t = self.p.ancho(&texto, &e.estilo);
                    let afuera = match e.posicion.as_str() {
                        "outEnd" => true,
                        "bestFit" | "" => ri <= 0.0 && barrido * ro * 0.6 < ancho_t * 0.8,
                        _ => false,
                    };
                    let (rr, ah) = if afuera {
                        (ro + 4.0, if medio.cos() >= 0.0 { 0.0 } else { 1.0 })
                    } else if e.posicion == "inEnd" {
                        (ri + (ro - ri) * 0.8, 0.5)
                    } else {
                        (if ri > 0.0 { (ri + ro) / 2.0 } else { ro * 0.62 }, 0.5)
                    };
                    let mut est = e.estilo.clone();
                    if !afuera && !e.color_propio {
                        est.color = contraste(relleno.as_ref());
                    }
                    let (x, y) = (cx + rr * medio.cos(), cy + rr * medio.sin());
                    let av = if afuera { 0.5 - 0.5 * medio.sin() } else { 0.5 };
                    if let Some((t, _)) = self.p.texto(&texto, &est, x, y, ah, av) {
                        self.encima.push(t);
                    }
                }
                a0 = a1;
            }
        }
    }

    // -----------------------------------------------------------------
    // Radar
    // -----------------------------------------------------------------

    fn radar(&mut self, g: &Grafico, gr: &Grupo, area: Rect) {
        let n = g
            .categorias
            .len()
            .max(gr.series.iter().map(|s| s.valores.len()).max().unwrap_or(0))
            .clamp(1, MAX_PUNTOS);
        let ev = ejes_de(g, gr).1;
        let ec = ejes_de(g, gr).0;
        let lh = self.p.renglon(&ec.estilo);
        let max_cat = (0..n)
            .map(|i| {
                self.p.ancho(
                    g.categorias.get(i).map(|s| s.as_str()).unwrap_or(""),
                    &ec.estilo,
                )
            })
            .fold(0.0, f32::max);
        let r = ((area.ancho / 2.0 - max_cat.min(area.ancho / 4.0) - 6.0)
            .min(area.alto / 2.0 - lh - 6.0))
        .max(4.0);
        let (cx, cy) = (area.x + area.ancho / 2.0, area.y + area.alto / 2.0);
        let (lo, hi) = rango_grupo(gr, n).unwrap_or((0.0, 0.0));
        let lhv = self.p.renglon(&ev.estilo);
        let esc = escala(
            lo,
            hi,
            ev.min,
            ev.max,
            ev.unidad,
            ((r / (lhv * 1.5)).floor() as usize).clamp(2, 10),
        );
        let angulo = |i: usize| -FRAC_PI_2 + TAU * i as f32 / n as f32;
        let punto = |i: usize, f: f64| {
            let rr = r * (f.clamp(0.0, 1.0) as f32);
            (cx + rr * angulo(i).cos(), cy + rr * angulo(i).sin())
        };
        let rejilla = ev.cuadricula.unwrap_or(Trazo::solido(GRIS_REJILLA, 0.75));
        for m in esc.marcas() {
            let f = esc.fraccion(m);
            if f <= 0.0 {
                continue;
            }
            let pts: Vec<(f32, f32)> = (0..n).map(|i| punto(i, f)).collect();
            if let Some(fig) = poligono(&pts, true) {
                self.els.push(Elemento::Trayecto {
                    figuras: vec![Figura {
                        rellenar: false,
                        ..fig
                    }],
                    relleno: None,
                    borde: Some(rejilla),
                });
            }
        }
        for i in 0..n {
            let q = punto(i, 1.0);
            self.els.push(Elemento::Linea {
                desde: (cx, cy),
                hasta: q,
                trazo: ec.linea.unwrap_or(rejilla),
            });
            let a = angulo(i);
            let (x, y) = (cx + (r + 5.0) * a.cos(), cy + (r + 5.0) * a.sin());
            let ah = if a.cos().abs() < 0.2 {
                0.5
            } else if a.cos() > 0.0 {
                0.0
            } else {
                1.0
            };
            let av = 0.5 - 0.5 * a.sin();
            if ec.visible() {
                let e = self.p.texto(
                    g.categorias.get(i).map(|s| s.as_str()).unwrap_or(""),
                    &ec.estilo,
                    x,
                    y,
                    ah,
                    av,
                );
                self.poner(e);
            }
        }
        if ev.visible() {
            let formato = ev
                .formato
                .clone()
                .or_else(|| gr.series.first().map(|s| s.formato.clone()))
                .unwrap_or_default();
            for m in esc.marcas() {
                let (x, y) = punto(0, esc.fraccion(m));
                let e = self.p.texto(
                    &formatear(m, &formato, Some(esc.paso)),
                    &ev.estilo,
                    x - 3.0,
                    y,
                    1.0,
                    0.5,
                );
                self.poner(e);
            }
        }
        let Clase::Radar { relleno } = gr.clase else {
            return;
        };
        for s in &gr.series {
            let pts: Vec<(f32, f32)> = (0..n)
                .map(|i| {
                    punto(
                        i,
                        esc.fraccion(s.valores.get(i).copied().flatten().unwrap_or(0.0)),
                    )
                })
                .collect();
            let Some(fig) = poligono(&pts, true) else {
                continue;
            };
            if relleno {
                self.els.push(Elemento::Trayecto {
                    figuras: vec![fig],
                    relleno: relleno_serie(s),
                    borde: s.linea.flatten(),
                });
            } else {
                self.els.push(Elemento::Trayecto {
                    figuras: vec![Figura {
                        rellenar: false,
                        ..fig
                    }],
                    relleno: None,
                    borde: trazo_serie(s),
                });
                if let Some((sim, tam, rel, bor)) = marcador_de(gr, s) {
                    let figuras: Vec<Figura> = pts
                        .iter()
                        .flat_map(|p| figura_marcador(sim, p.0, p.1, tam))
                        .collect();
                    self.els.push(Elemento::Trayecto {
                        figuras,
                        relleno: rel,
                        borde: bor,
                    });
                }
            }
        }
    }

    // -----------------------------------------------------------------
    // Con ejes: columnas, barras, lineas, areas, dispersion y burbujas
    // -----------------------------------------------------------------

    fn cartesiano(&mut self, g: &Grafico, area: Rect) {
        let grupos: Vec<&Grupo> = g.grupos.iter().filter(|x| x.clase.cartesiana()).collect();
        let Some(primero) = grupos.first() else {
            return;
        };
        let horizontal = matches!(primero.clase, Clase::Barras { horizontal: true });
        let xy = primero.clase.xy();
        let (ec, ev) = ejes_de(g, primero);
        // El eje de valores secundario (otro grupo con otro eje), solo en
        // los graficos de columnas y lineas.
        let mut ev2: Option<Eje> = None;
        let secundario: Vec<bool> = grupos
            .iter()
            .map(|gr| {
                let v = ejes_de(g, gr).1;
                let otro = !horizontal && !v.id.is_empty() && v.id != ev.id;
                if otro && ev2.is_none() {
                    ev2 = Some(v);
                }
                otro
            })
            .collect();
        let n = if xy {
            1
        } else {
            g.categorias
                .len()
                .max(
                    grupos
                        .iter()
                        .filter(|x| !x.clase.xy())
                        .flat_map(|x| x.series.iter())
                        .map(|s| s.valores.len())
                        .max()
                        .unwrap_or(0),
                )
                .clamp(1, MAX_PUNTOS)
        };
        let (mut r1, mut r2) = (None, None);
        let (mut pct1, mut pct2) = (false, false);
        for (gr, sec) in grupos.iter().zip(&secundario) {
            let r = rango_grupo(gr, n);
            let pct = gr.agrupacion == Agrupacion::Porcentaje && !gr.clase.xy();
            if *sec {
                r2 = unir(r2, r);
                pct2 |= pct;
            } else {
                r1 = unir(r1, r);
                pct1 |= pct;
            }
        }
        let formato_de = |eje: &Eje, pct: bool, en_sec: bool| -> String {
            if pct {
                return eje.formato.clone().unwrap_or_else(|| "0%".into());
            }
            eje.formato.clone().unwrap_or_else(|| {
                grupos
                    .iter()
                    .zip(&secundario)
                    .filter(|(_, s)| **s == en_sec)
                    .flat_map(|(gr, _)| gr.series.first())
                    .map(|s| s.formato.clone())
                    .next()
                    .unwrap_or_default()
            })
        };
        // Los lados: horizontal (H) y vertical (V).
        let rx = if xy { rango_x(&grupos) } else { None };
        let mut lado_h = if horizontal {
            Lado::valores(ev.clone(), r1, pct1, formato_de(&ev, pct1, false))
        } else if xy {
            let f = ec.formato.clone().unwrap_or_default();
            Lado::valores(ec.clone(), rx, false, f)
        } else {
            Lado::categorias(ec.clone(), n, ev.entre)
        };
        let mut lado_v = if horizontal {
            Lado::categorias(ec.clone(), n, ev.entre)
        } else {
            Lado::valores(ev.clone(), r1, pct1, formato_de(&ev, pct1, false))
        };
        let mut lado_v2 = ev2.clone().map(|e| {
            let f = formato_de(&e, pct2, true);
            Lado::valores(e, r2, pct2, f)
        });
        let lh_h = self.p.renglon(&lado_h.eje.estilo);
        let lh_v = self.p.renglon(&lado_v.eje.estilo);
        let interior = g
            .manual_trazado
            .filter(|m| m.interior)
            .map(|m| self.caja_manual(m));
        // Dos pasadas: el largo de los ejes depende de lo que ocupan sus
        // etiquetas, y las etiquetas de valores del paso que cabe.
        let mut trazado = Rect::nuevo(
            area.x + 30.0,
            area.y + lh_v / 2.0,
            (area.ancho - 34.0).max(10.0),
            (area.alto - lh_h - 8.0).max(10.0),
        );
        let mut cats_h: Vec<Vec<String>> = Vec::new();
        let mut salto_h = 1usize;
        for _ in 0..2 {
            if let Some(i) = interior {
                trazado = i;
            }
            lado_h.calcular(trazado.ancho, lh_h, false);
            lado_v.calcular(trazado.alto, lh_v, true);
            if let Some(l) = lado_v2.as_mut() {
                l.calcular(trazado.alto, lh_v, true);
            }
            let textos_h = lado_h.textos(g);
            let textos_v = lado_v.textos(g);
            // Lo que ocupan las etiquetas.
            let mut izq = 0.0f32;
            let mut der = 0.0f32;
            let mut arriba = lh_v / 2.0;
            let mut abajo = 0.0f32;
            if lado_v.eje.visible() {
                let tope = area.ancho * 0.4;
                let w = textos_v
                    .iter()
                    .map(|t| self.p.ancho(t, &lado_v.eje.estilo))
                    .fold(0.0, f32::max)
                    .min(tope)
                    + HUECO
                    + 2.0;
                if lado_v.eje.pos == 'r' {
                    der += w;
                } else {
                    izq += w;
                }
            }
            if let Some(l) = lado_v2.as_ref().filter(|l| l.eje.visible()) {
                let w = l
                    .textos(g)
                    .iter()
                    .map(|t| self.p.ancho(t, &l.eje.estilo))
                    .fold(0.0, f32::max)
                    + HUECO
                    + 2.0;
                if lado_v.eje.pos == 'r' {
                    izq += w;
                } else {
                    der += w;
                }
            }
            // Los titulos de los ejes verticales.
            if let Some(t) = lado_v.eje.titulo.as_ref().filter(|_| !lado_v.eje.borrado) {
                let w = self.p.renglon(&t.estilo) + 4.0;
                if lado_v.eje.pos == 'r' {
                    der += w;
                } else {
                    izq += w;
                }
            }
            if let Some(l) = lado_v2.as_ref().filter(|l| !l.eje.borrado)
                && let Some(t) = &l.eje.titulo
            {
                let w = self.p.renglon(&t.estilo) + 4.0;
                if lado_v.eje.pos == 'r' {
                    izq += w;
                } else {
                    der += w;
                }
            }
            if lado_h.eje.visible() {
                let alto_h = if lado_h.escala.is_none() {
                    // Categorias: en renglones si caben partidas, o saltando.
                    let banda = if lado_h.entre {
                        trazado.ancho / n as f32
                    } else {
                        trazado.ancho / n.saturating_sub(1).max(1) as f32
                    };
                    let (lineas, salto) =
                        self.repartir_categorias(&textos_h, &lado_h.eje.estilo, banda);
                    let alto = lineas.iter().map(|l| l.len()).max().unwrap_or(1) as f32 * lh_h;
                    if !lado_h.entre {
                        // Los puntos van en los bordes: que quepa medio rotulo
                        // del primero y del ultimo.
                        let medio = |l: Option<&Vec<String>>| {
                            l.map(|l| {
                                l.iter()
                                    .map(|x| self.p.ancho(x, &lado_h.eje.estilo))
                                    .fold(0.0, f32::max)
                            })
                            .unwrap_or(0.0)
                                / 2.0
                        };
                        izq = izq.max(medio(lineas.first()));
                        der = der.max(medio(lineas.last()));
                    }
                    cats_h = lineas;
                    salto_h = salto;
                    alto
                } else {
                    let ultimo = textos_h
                        .last()
                        .map(|t| self.p.ancho(t, &lado_h.eje.estilo))
                        .unwrap_or(0.0);
                    der = der.max(ultimo / 2.0);
                    lh_h
                };
                let mut w = alto_h + HUECO + 2.0;
                // Con negativos, los rotulos van junto al eje, dentro del
                // trazado: abajo solo hace falta lo que no quepa.
                let f = fraccion_cruce(lado_h.eje.cruza, &lado_v);
                if lado_h.eje.pos_etiquetas == "nextTo" && lado_h.eje.pos != 't' && f > 0.001 {
                    w = (w - f as f32 * trazado.alto).max(0.0);
                }
                if lado_h.eje.pos == 't' {
                    arriba += w;
                } else {
                    abajo += w;
                }
            }
            if let Some(t) = &lado_h.eje.titulo
                && !lado_h.eje.borrado
            {
                abajo += self.p.renglon(&t.estilo) + 4.0;
            }
            if interior.is_none() {
                trazado = Rect::nuevo(
                    area.x + izq,
                    area.y + arriba,
                    (area.ancho - izq - der).max(10.0),
                    (area.alto - arriba - abajo).max(10.0),
                );
            }
        }
        lado_h.textos_cache = lado_h.textos(g);
        lado_v.textos_cache = lado_v.textos(g);
        if let Some(l) = lado_v2.as_mut() {
            l.textos_cache = l.textos(g);
        }

        // Las rectas de los ejes.
        let rh = if lado_h.eje.invertido {
            Recta {
                a: trazado.derecha(),
                b: trazado.x,
            }
        } else {
            Recta {
                a: trazado.x,
                b: trazado.derecha(),
            }
        };
        let rv = if lado_v.eje.invertido {
            Recta {
                a: trazado.y,
                b: trazado.abajo(),
            }
        } else {
            Recta {
                a: trazado.abajo(),
                b: trazado.y,
            }
        };
        let rv2 = lado_v2.as_ref().map(|l| {
            if l.eje.invertido {
                Recta {
                    a: trazado.y,
                    b: trazado.abajo(),
                }
            } else {
                Recta {
                    a: trazado.abajo(),
                    b: trazado.y,
                }
            }
        });

        // El fondo del trazado y las cuadriculas.
        if g.fondo_trazado.is_some() || g.borde_trazado.is_some() {
            self.els.push(Elemento::Rect {
                rect: trazado,
                relleno: g.fondo_trazado.clone(),
                borde: g.borde_trazado,
            });
        }
        if let Some(t) = lado_h.eje.cuadricula {
            for f in lado_h.fracciones_rejilla() {
                let x = rh.en(f);
                self.els.push(Elemento::Linea {
                    desde: (x, trazado.y),
                    hasta: (x, trazado.abajo()),
                    trazo: t,
                });
            }
        }
        if let Some(t) = lado_v.eje.cuadricula {
            for f in lado_v.fracciones_rejilla() {
                let y = rv.en(f);
                self.els.push(Elemento::Linea {
                    desde: (trazado.x, y),
                    hasta: (trazado.derecha(), y),
                    trazo: t,
                });
            }
        }

        // Las series, recortadas al trazado (con un margen para los
        // marcadores del borde).
        let mut series_els = Vec::new();
        let mut orden: Vec<(usize, &Grupo, bool)> = grupos
            .iter()
            .zip(&secundario)
            .map(|(gr, s)| {
                let peso = match gr.clase {
                    Clase::Area => 0,
                    Clase::Barras { .. } => 1,
                    Clase::Lineas => 2,
                    _ => 3,
                };
                (peso, *gr, *s)
            })
            .collect();
        orden.sort_by_key(|o| o.0);
        for (_, gr, sec) in orden {
            let (lado_val, recta_val) = match (sec, lado_v2.as_ref(), rv2) {
                (true, Some(l), Some(r)) => (l, r),
                _ if horizontal => (&lado_h, rh),
                _ => (&lado_v, rv),
            };
            let Some(esc) = lado_val.escala else { continue };
            let marco = Marco {
                cat: if horizontal { rv } else { rh },
                val: recta_val,
                escala: esc,
                n,
                entre: lado_val_entre(&lado_h, &lado_v, horizontal),
                horizontal,
                base: valor_cruce(ec.cruza, &esc),
            };
            match gr.clase {
                Clase::Barras { .. } => self.barras(g, gr, &marco, &mut series_els),
                Clase::Lineas => self.lineas(g, gr, &marco, &mut series_els),
                Clase::Area => self.areas(g, gr, &marco, &mut series_els),
                Clase::Dispersion | Clase::Burbuja => {
                    if let Some(ex) = lado_h.escala {
                        self.dispersion(g, gr, rh, ex, recta_val, esc, &trazado, &mut series_els);
                    }
                }
                _ => {}
            }
        }
        let holgura = 8.0;
        self.els.push(Elemento::Grupo {
            transformada: IDENTIDAD,
            recorte: Some(Rect::nuevo(
                trazado.x - holgura,
                trazado.y - holgura,
                trazado.ancho + 2.0 * holgura,
                trazado.alto + 2.0 * holgura,
            )),
            elementos: series_els,
        });

        // Las lineas de los ejes, sus marcas y sus etiquetas.
        let y_h = rv.en(fraccion_cruce(lado_h.eje.cruza, &lado_v));
        let x_v = rh.en(fraccion_cruce(lado_v.eje.cruza, &lado_h));
        let abajo = lado_h.eje.pos != 't';
        let izquierda = lado_v.eje.pos != 'r';
        // Eje H.
        if !lado_h.eje.borrado {
            if let Some(t) = lado_h.eje.linea {
                self.els.push(Elemento::Linea {
                    desde: (trazado.x, y_h),
                    hasta: (trazado.derecha(), y_h),
                    trazo: t,
                });
                let s = if abajo { 1.0 } else { -1.0 };
                for f in lado_h.fracciones_marcas() {
                    let x = rh.en(f);
                    if let Some((a, b)) = tramo_marca(&lado_h.eje.marcas, y_h, s) {
                        self.els.push(Elemento::Linea {
                            desde: (x, a),
                            hasta: (x, b),
                            trazo: t,
                        });
                    }
                }
            }
            if lado_h.eje.visible() {
                let y_et = match lado_h.eje.pos_etiquetas.as_str() {
                    "low" => rv.a.max(rv.b),
                    "high" => rv.a.min(rv.b),
                    _ => y_h,
                };
                let extra = if matches!(lado_h.eje.marcas.as_str(), "out" | "cross") {
                    MARCA
                } else {
                    0.0
                };
                let (y, av) = if abajo {
                    (y_et + HUECO + extra, 0.0)
                } else {
                    (y_et - HUECO - extra, 1.0)
                };
                if lado_h.escala.is_some() {
                    for (f, t) in lado_h
                        .fracciones_textos()
                        .into_iter()
                        .zip(&lado_h.textos_cache)
                    {
                        let e = self.p.texto(t, &lado_h.eje.estilo, rh.en(f), y, 0.5, av);
                        self.poner(e);
                    }
                } else {
                    for (i, f) in lado_h.fracciones_textos().into_iter().enumerate() {
                        if i % salto_h != 0 {
                            continue;
                        }
                        let Some(lineas) = cats_h.get(i) else {
                            continue;
                        };
                        let k = lineas.len() as f32;
                        for (j, l) in lineas.iter().enumerate() {
                            let yy = if abajo {
                                y + lh_h * j as f32
                            } else {
                                y - lh_h * (k - 1.0 - j as f32)
                            };
                            let e = self.p.texto(l, &lado_h.eje.estilo, rh.en(f), yy, 0.5, av);
                            self.poner(e);
                        }
                    }
                }
            }
        }
        // Eje V (y el secundario).
        let mut lados_v: Vec<(&Lado, Recta, f32, bool)> = vec![(&lado_v, rv, x_v, izquierda)];
        if let (Some(l), Some(r)) = (lado_v2.as_ref(), rv2) {
            let x = if izquierda {
                trazado.derecha()
            } else {
                trazado.x
            };
            lados_v.push((l, r, x, !izquierda));
        }
        for (lado, recta, x_eje, a_la_izq) in lados_v {
            if lado.eje.borrado {
                continue;
            }
            let s = if a_la_izq { -1.0 } else { 1.0 };
            if let Some(t) = lado.eje.linea {
                self.els.push(Elemento::Linea {
                    desde: (x_eje, trazado.y),
                    hasta: (x_eje, trazado.abajo()),
                    trazo: t,
                });
                for f in lado.fracciones_marcas() {
                    let y = recta.en(f);
                    if let Some((a, b)) = tramo_marca(&lado.eje.marcas, x_eje, s) {
                        self.els.push(Elemento::Linea {
                            desde: (a, y),
                            hasta: (b, y),
                            trazo: t,
                        });
                    }
                }
            }
            if lado.eje.visible() {
                let x_et = match lado.eje.pos_etiquetas.as_str() {
                    "low" => rh.a.min(rh.b),
                    "high" => rh.a.max(rh.b),
                    _ => x_eje,
                };
                let extra = if matches!(lado.eje.marcas.as_str(), "out" | "cross") {
                    MARCA
                } else {
                    0.0
                };
                let x = x_et + s * (HUECO + extra);
                let tope = area.ancho * 0.4;
                for (f, t) in lado.fracciones_textos().into_iter().zip(&lado.textos_cache) {
                    let t = self.p.recortar(t, &lado.eje.estilo, tope);
                    let e = self.p.texto(
                        &t,
                        &lado.eje.estilo,
                        x,
                        recta.en(f),
                        if a_la_izq { 1.0 } else { 0.0 },
                        0.5,
                    );
                    self.poner(e);
                }
            }
            if let Some(t) = &lado.eje.titulo {
                let lh = self.p.renglon(&t.estilo);
                let x = if a_la_izq {
                    area.x + lh / 2.0
                } else {
                    area.derecha() - lh / 2.0
                };
                let texto = t.lineas.join(" ");
                let texto = self.p.recortar(&texto, &t.estilo, trazado.alto);
                self.texto_vertical(&texto, &t.estilo, x, trazado.y + trazado.alto / 2.0);
            }
        }
        if let Some(t) = &lado_h.eje.titulo
            && !lado_h.eje.borrado
        {
            let texto = self.p.recortar(&t.lineas.join(" "), &t.estilo, area.ancho);
            let e = self.p.texto(
                &texto,
                &t.estilo,
                trazado.x + trazado.ancho / 2.0,
                area.abajo(),
                0.5,
                1.0,
            );
            self.poner(e);
        }
    }

    /// Reparte los rotulos de las categorias en su banda: partidos en
    /// renglones (tres como mucho) si asi caben; si no, uno de cada
    /// `salto`.
    fn repartir_categorias(
        &self,
        textos: &[String],
        e: &EstiloTexto,
        banda: f32,
    ) -> (Vec<Vec<String>>, usize) {
        let max = (banda - 2.0).max(1.0);
        let anchos: Vec<f32> = textos.iter().map(|t| self.p.ancho(t, e)).collect();
        if anchos.iter().all(|w| *w <= max) {
            return (textos.iter().map(|t| vec![t.clone()]).collect(), 1);
        }
        let partidas: Vec<Vec<String>> = textos.iter().map(|t| self.p.partir(t, e, max)).collect();
        let cabe = partidas
            .iter()
            .all(|l| l.len() <= 3 && l.iter().all(|x| self.p.ancho(x, e) <= max));
        if cabe {
            return (partidas, 1);
        }
        let mayor = anchos.iter().copied().fold(0.0, f32::max);
        let salto = ((mayor + 6.0) / banda.max(0.1)).ceil().max(1.0) as usize;
        let tope = max * salto as f32;
        (
            textos
                .iter()
                .map(|t| vec![self.p.recortar(t, e, tope)])
                .collect(),
            salto,
        )
    }

    fn barras(&mut self, g: &Grafico, gr: &Grupo, m: &Marco, salida: &mut Vec<Elemento>) {
        let ns = gr.series.len().max(1);
        let apilada = gr.agrupacion != Agrupacion::Normal;
        let pct = gr.agrupacion == Agrupacion::Porcentaje;
        let ov = if apilada { 1.0 } else { gr.solape };
        let k = if apilada { 1.0 } else { ns as f32 };
        let banda = 1.0 / m.n as f32;
        let w = banda / (k - (k - 1.0) * ov + gr.hueco).max(0.01);
        let grupo_w = w * (k - (k - 1.0) * ov);
        for i in 0..m.n {
            let centro = (i as f32 + 0.5) * banda;
            let ini = centro - grupo_w / 2.0;
            let total: f64 = if pct {
                gr.series
                    .iter()
                    .map(|s| s.valores.get(i).copied().flatten().unwrap_or(0.0).abs())
                    .sum()
            } else {
                1.0
            };
            let (mut pos, mut neg) = (0.0f64, 0.0f64);
            for (si, s) in gr.series.iter().enumerate() {
                let Some(v) = s.valores.get(i).copied().flatten() else {
                    continue;
                };
                let vv = if pct {
                    if total > 0.0 { v / total } else { 0.0 }
                } else {
                    v
                };
                let (a, b) = if apilada {
                    if vv >= 0.0 {
                        let r = (pos, pos + vv);
                        pos += vv;
                        r
                    } else {
                        let r = (neg, neg + vv);
                        neg += vv;
                        r
                    }
                } else {
                    (m.base, vv)
                };
                let f0 = ini
                    + if apilada {
                        0.0
                    } else {
                        si as f32 * w * (1.0 - ov)
                    };
                let (c0, c1) = (m.cat.en(f0 as f64), m.cat.en((f0 + w) as f64));
                let fa = m.escala.fraccion(a).clamp(0.0, 1.0);
                let fb = m.escala.fraccion(b).clamp(0.0, 1.0);
                let (v0, v1) = (m.val.en(fa), m.val.en(fb));
                let rect = if m.horizontal {
                    Rect::nuevo(v0.min(v1), c0.min(c1), (v1 - v0).abs(), (c1 - c0).abs())
                } else {
                    Rect::nuevo(c0.min(c1), v0.min(v1), (c1 - c0).abs(), (v1 - v0).abs())
                };
                let relleno = relleno_punto(g, gr, s, i);
                salida.push(Elemento::Rect {
                    rect,
                    relleno: relleno.clone(),
                    borde: s.linea.flatten(),
                });
                if let Some(e) = etiquetas_de(gr, s)
                    && !e.borrados.contains(&i)
                {
                    let cat = g.categorias.get(i).map(|c| c.as_str()).unwrap_or("");
                    let texto = texto_etiqueta(e, s, cat, v, pct.then_some(vv));
                    let posicion = if e.posicion.is_empty() {
                        if apilada { "ctr" } else { "outEnd" }
                    } else {
                        e.posicion.as_str()
                    };
                    // Hacia donde crece la barra en pantalla.
                    let dir = if v1 >= v0 { 1.0 } else { -1.0 };
                    let cc = (c0 + c1) / 2.0;
                    let (p, ancla_dentro) = match posicion {
                        "inEnd" => (v1 - dir * HUECO, -dir),
                        "inBase" => (v0 + dir * HUECO, dir),
                        "ctr" | "bestFit" => ((v0 + v1) / 2.0, 0.0),
                        _ => (v1 + dir * HUECO, dir),
                    };
                    let dentro = posicion != "outEnd" && !matches!(posicion, "t" | "b" | "l" | "r");
                    let mut est = e.estilo.clone();
                    if dentro && !e.color_propio {
                        est.color = contraste(relleno.as_ref());
                    }
                    // La fraccion de anclaje a lo largo del eje de valores.
                    let ancla = 0.5 - 0.5 * ancla_dentro;
                    let t = if m.horizontal {
                        self.p.texto(&texto, &est, p, cc, ancla, 0.5)
                    } else {
                        self.p.texto(&texto, &est, cc, p, 0.5, ancla)
                    };
                    if let Some((t, _)) = t {
                        self.encima.push(t);
                    }
                }
            }
        }
    }

    /// Los puntos de una serie de lineas o de areas (con lo apilado ya
    /// sumado en `acum`).
    #[allow(clippy::too_many_arguments)]
    fn puntos_serie(
        &self,
        g: &Grafico,
        gr: &Grupo,
        s: &Serie,
        m: &Marco,
        acum: &mut [f64],
        totales: &[f64],
        cero_si_falta: bool,
    ) -> Vec<Option<(f32, f32, f64)>> {
        (0..m.n)
            .map(|i| {
                let v = match s.valores.get(i).copied().flatten() {
                    Some(v) => v,
                    None if cero_si_falta || g.huecos == Huecos::Cero => 0.0,
                    None => return None,
                };
                let y = match gr.agrupacion {
                    Agrupacion::Normal => v,
                    Agrupacion::Apilada => {
                        acum[i] += v;
                        acum[i]
                    }
                    Agrupacion::Porcentaje => {
                        acum[i] += v;
                        if totales[i] > 0.0 {
                            acum[i] / totales[i]
                        } else {
                            0.0
                        }
                    }
                };
                let fc = m.fraccion_cat(i);
                let (x, yy) = m.punto(fc, m.escala.fraccion(y));
                Some((x, yy, v))
            })
            .collect()
    }

    fn lineas(&mut self, g: &Grafico, gr: &Grupo, m: &Marco, salida: &mut Vec<Elemento>) {
        let totales = totales(gr, m.n);
        let mut acum = vec![0.0; m.n];
        for s in &gr.series {
            let pts = self.puntos_serie(g, gr, s, m, &mut acum, &totales, false);
            if let Some(t) = trazo_serie(s) {
                let mut figuras = Vec::new();
                let mut tramo: Vec<(f32, f32)> = Vec::new();
                let cerrar = |tramo: &mut Vec<(f32, f32)>, figuras: &mut Vec<Figura>| {
                    if tramo.len() >= 2 {
                        let tramos = if s.suave {
                            suave(tramo)
                        } else {
                            tramo[1..].iter().map(|p| Tramo::Linea(p.0, p.1)).collect()
                        };
                        figuras.push(Figura {
                            inicio: tramo[0],
                            tramos,
                            cerrada: false,
                            rellenar: false,
                            trazar: true,
                            ..Default::default()
                        });
                    }
                    tramo.clear();
                };
                for p in &pts {
                    match p {
                        Some((x, y, _)) => tramo.push((*x, *y)),
                        None if g.huecos == Huecos::Seguir => {}
                        None => cerrar(&mut tramo, &mut figuras),
                    }
                }
                cerrar(&mut tramo, &mut figuras);
                if !figuras.is_empty() {
                    salida.push(Elemento::Trayecto {
                        figuras,
                        relleno: None,
                        borde: Some(t),
                    });
                }
            }
            if let Some((sim, tam, rel, bor)) = marcador_de(gr, s) {
                let figuras: Vec<Figura> = pts
                    .iter()
                    .flatten()
                    .flat_map(|p| figura_marcador(sim, p.0, p.1, tam))
                    .collect();
                if !figuras.is_empty() {
                    salida.push(Elemento::Trayecto {
                        figuras,
                        relleno: rel,
                        borde: bor,
                    });
                }
            }
            self.etiquetas_puntos(g, gr, s, &pts, m.horizontal);
        }
    }

    fn etiquetas_puntos(
        &mut self,
        g: &Grafico,
        gr: &Grupo,
        s: &Serie,
        pts: &[Option<(f32, f32, f64)>],
        _horizontal: bool,
    ) {
        let Some(e) = etiquetas_de(gr, s) else { return };
        for (i, p) in pts.iter().enumerate() {
            let Some((x, y, v)) = p else { continue };
            if e.borrados.contains(&i) {
                continue;
            }
            let cat = g.categorias.get(i).map(|c| c.as_str()).unwrap_or("");
            let texto = texto_etiqueta(e, s, cat, *v, None);
            let d = HUECO + 2.0;
            let t = match e.posicion.as_str() {
                "t" => self.p.texto(&texto, &e.estilo, *x, y - d, 0.5, 1.0),
                "b" => self.p.texto(&texto, &e.estilo, *x, y + d, 0.5, 0.0),
                "l" => self.p.texto(&texto, &e.estilo, x - d, *y, 1.0, 0.5),
                "ctr" => self.p.texto(&texto, &e.estilo, *x, *y, 0.5, 0.5),
                _ => self.p.texto(&texto, &e.estilo, x + d, *y, 0.0, 0.5),
            };
            if let Some((t, _)) = t {
                self.encima.push(t);
            }
        }
    }

    fn areas(&mut self, g: &Grafico, gr: &Grupo, m: &Marco, salida: &mut Vec<Elemento>) {
        let totales = totales(gr, m.n);
        let mut acum = vec![0.0; m.n];
        let mut debajo: Vec<(f32, f32)> = (0..m.n)
            .map(|i| m.punto(m.fraccion_cat(i), m.escala.fraccion(m.base)))
            .collect();
        for s in &gr.series {
            let pts = self.puntos_serie(g, gr, s, m, &mut acum, &totales, true);
            let arriba: Vec<(f32, f32)> = pts
                .iter()
                .map(|p| p.map(|p| (p.0, p.1)).unwrap_or((0.0, 0.0)))
                .collect();
            let mut contorno = arriba.clone();
            contorno.extend(debajo.iter().rev());
            if let Some(f) = poligono(&contorno, true) {
                salida.push(Elemento::Trayecto {
                    figuras: vec![Figura {
                        trazar: s.linea.flatten().is_some(),
                        ..f
                    }],
                    relleno: relleno_serie(s),
                    borde: s.linea.flatten(),
                });
            }
            self.etiquetas_puntos(g, gr, s, &pts, m.horizontal);
            if gr.agrupacion != Agrupacion::Normal {
                debajo = arriba;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn dispersion(
        &mut self,
        g: &Grafico,
        gr: &Grupo,
        rh: Recta,
        ex: Escala,
        rv: Recta,
        ey: Escala,
        trazado: &Rect,
        salida: &mut Vec<Elemento>,
    ) {
        let burbuja = matches!(gr.clase, Clase::Burbuja);
        let max_tam = gr
            .series
            .iter()
            .flat_map(|s| s.tamanos.iter().flatten())
            .map(|t| t.abs())
            .fold(0.0f64, f64::max);
        let diametro_max = trazado.ancho.min(trazado.alto) * 0.25 * gr.escala_burbuja;
        for s in &gr.series {
            let pts: Vec<Option<(f32, f32, f64)>> = s
                .valores
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    let v = (*v)?;
                    let x = s.xs.get(i).copied().flatten().unwrap_or((i + 1) as f64);
                    Some((rh.en(ex.fraccion(x)), rv.en(ey.fraccion(v)), v))
                })
                .collect();
            if burbuja {
                for (i, p) in pts.iter().enumerate() {
                    let Some((x, y, _)) = p else { continue };
                    let t = s.tamanos.get(i).copied().flatten().unwrap_or(0.0).abs();
                    if !(t > 0.0 && max_tam > 0.0) {
                        continue;
                    }
                    let d = diametro_max * (t / max_tam).sqrt() as f32;
                    let relleno = relleno_punto(g, gr, s, i).map(|r| match r {
                        Relleno::Solido(c) if s.relleno.is_none() => {
                            Relleno::Solido(c.con_alfa(0.75))
                        }
                        otro => otro,
                    });
                    salida.push(Elemento::Trayecto {
                        figuras: vec![circulo(*x, *y, d / 2.0)],
                        relleno,
                        borde: s.linea.flatten(),
                    });
                }
            } else {
                let con_linea = !matches!(gr.estilo_dispersion.as_str(), "marker" | "none");
                if con_linea && let Some(t) = trazo_serie(s) {
                    let mut figuras = Vec::new();
                    let mut tramo: Vec<(f32, f32)> = Vec::new();
                    let suave_s = s.suave || gr.estilo_dispersion.starts_with("smooth");
                    for p in pts.iter().chain(std::iter::once(&None)) {
                        match p {
                            Some((x, y, _)) => tramo.push((*x, *y)),
                            None => {
                                if tramo.len() >= 2 {
                                    let tramos = if suave_s {
                                        suave(&tramo)
                                    } else {
                                        tramo[1..].iter().map(|p| Tramo::Linea(p.0, p.1)).collect()
                                    };
                                    figuras.push(Figura {
                                        inicio: tramo[0],
                                        tramos,
                                        cerrada: false,
                                        rellenar: false,
                                        trazar: true,
                                        ..Default::default()
                                    });
                                }
                                tramo.clear();
                            }
                        }
                    }
                    if !figuras.is_empty() {
                        salida.push(Elemento::Trayecto {
                            figuras,
                            relleno: None,
                            borde: Some(t),
                        });
                    }
                }
                if let Some((sim, tam, rel, bor)) = marcador_de(gr, s) {
                    let figuras: Vec<Figura> = pts
                        .iter()
                        .flatten()
                        .flat_map(|p| figura_marcador(sim, p.0, p.1, tam))
                        .collect();
                    if !figuras.is_empty() {
                        salida.push(Elemento::Trayecto {
                            figuras,
                            relleno: rel,
                            borde: bor,
                        });
                    }
                }
            }
            self.etiquetas_puntos(g, gr, s, &pts, false);
        }
    }
}

/// Donde se dibuja un grupo con ejes.
struct Marco {
    /// La recta del eje de categorias y la del de valores.
    cat: Recta,
    val: Recta,
    escala: Escala,
    n: usize,
    /// Los puntos en el centro de cada categoria (si no, en sus bordes).
    entre: bool,
    /// Barras: las categorias van en vertical.
    horizontal: bool,
    /// Desde donde salen las barras y las areas.
    base: f64,
}

impl Marco {
    fn fraccion_cat(&self, i: usize) -> f64 {
        fraccion_categoria(i, self.n, self.entre)
    }

    fn punto(&self, fcat: f64, fval: f64) -> (f32, f32) {
        if self.horizontal {
            (self.val.en(fval), self.cat.en(fcat))
        } else {
            (self.cat.en(fcat), self.val.en(fval))
        }
    }
}

fn fraccion_categoria(i: usize, n: usize, entre: bool) -> f64 {
    if entre {
        (i as f64 + 0.5) / n.max(1) as f64
    } else if n > 1 {
        i as f64 / (n - 1) as f64
    } else {
        0.5
    }
}

fn lado_val_entre(h: &Lado, v: &Lado, horizontal: bool) -> bool {
    if horizontal { v.entre } else { h.entre }
}

/// Un eje ya calculado: de categorias (sin escala) o de valores.
struct Lado {
    eje: Eje,
    escala: Option<Escala>,
    rango: Option<(f64, f64)>,
    porcentaje: bool,
    formato: String,
    n: usize,
    entre: bool,
    textos_cache: Vec<String>,
}

impl Lado {
    fn valores(eje: Eje, rango: Option<(f64, f64)>, porcentaje: bool, formato: String) -> Lado {
        Lado {
            eje,
            escala: None,
            rango,
            porcentaje,
            formato,
            n: 0,
            entre: true,
            textos_cache: Vec::new(),
        }
    }

    fn categorias(eje: Eje, n: usize, entre: bool) -> Lado {
        Lado {
            eje,
            escala: None,
            rango: None,
            porcentaje: false,
            formato: String::new(),
            n,
            entre,
            textos_cache: Vec::new(),
        }
    }

    fn es_valores(&self) -> bool {
        self.n == 0
    }

    /// La escala para un eje de `largo` puntos con etiquetas de `alto`.
    fn calcular(&mut self, largo: f32, alto: f32, vertical: bool) {
        if !self.es_valores() {
            return;
        }
        let div = if vertical {
            largo / (alto * 1.8).max(1.0)
        } else {
            largo / (alto * 4.5).max(1.0)
        };
        let div = (div.floor() as usize).clamp(2, 10);
        let (lo, hi) = self.rango.unwrap_or((0.0, 0.0));
        let e = &self.eje;
        self.escala = Some(if self.porcentaje {
            let a = e.min.unwrap_or(if lo < -1e-9 { -1.0 } else { 0.0 });
            let b = e
                .max
                .unwrap_or(if hi > 1e-9 || lo >= -1e-9 { 1.0 } else { 0.0 });
            escala(a, b, Some(a), Some(b), e.unidad, 10)
        } else {
            escala(lo, hi, e.min, e.max, e.unidad, div)
        });
    }

    fn textos(&self, g: &Grafico) -> Vec<String> {
        match self.escala {
            Some(esc) => esc
                .marcas()
                .into_iter()
                .map(|m| formatear(m, &self.formato, Some(esc.paso)))
                .collect(),
            None if self.es_valores() => Vec::new(),
            None => (0..self.n)
                .map(|i| {
                    g.categorias
                        .get(i)
                        .cloned()
                        .unwrap_or_else(|| (i + 1).to_string())
                })
                .collect(),
        }
    }

    /// Donde van los textos (fracciones del eje).
    fn fracciones_textos(&self) -> Vec<f64> {
        match self.escala {
            Some(esc) => esc.marcas().into_iter().map(|m| esc.fraccion(m)).collect(),
            None => (0..self.n)
                .map(|i| fraccion_categoria(i, self.n, self.entre))
                .collect(),
        }
    }

    /// Donde van las marcas: en los valores, o entre categorias.
    fn fracciones_marcas(&self) -> Vec<f64> {
        match self.escala {
            Some(esc) => esc.marcas().into_iter().map(|m| esc.fraccion(m)).collect(),
            None if self.entre => (0..=self.n)
                .map(|k| k as f64 / self.n.max(1) as f64)
                .collect(),
            None => (0..self.n)
                .map(|i| fraccion_categoria(i, self.n, false))
                .collect(),
        }
    }

    fn fracciones_rejilla(&self) -> Vec<f64> {
        self.fracciones_marcas()
    }
}

/// El tramo de una marca de eje que sale de `en`; `s` es el sentido de
/// fuera del trazado.
fn tramo_marca(tipo: &str, en: f32, s: f32) -> Option<(f32, f32)> {
    match tipo {
        "out" => Some((en, en + s * MARCA)),
        "in" => Some((en, en - s * MARCA)),
        "cross" => Some((en - MARCA, en + MARCA)),
        _ => None,
    }
}

/// El valor de `esc` donde lo corta el otro eje.
fn valor_cruce(c: Cruce, esc: &Escala) -> f64 {
    match c {
        Cruce::Cero => 0f64.clamp(esc.min.min(esc.max), esc.max.max(esc.min)),
        Cruce::Min => esc.min,
        Cruce::Max => esc.max,
        Cruce::En(v) => v.clamp(esc.min.min(esc.max), esc.max.max(esc.min)),
    }
}

/// La fraccion de `otro` donde lo corta un eje que dice `c`.
fn fraccion_cruce(c: Cruce, otro: &Lado) -> f64 {
    match otro.escala {
        Some(esc) => esc.fraccion(valor_cruce(c, &esc)).clamp(0.0, 1.0),
        None => match c {
            Cruce::Max => 1.0,
            Cruce::En(v) if otro.n > 0 => ((v - 1.0) / otro.n as f64).clamp(0.0, 1.0),
            _ => 0.0,
        },
    }
}

/// Los dos ejes de un grupo: (categorias o X, valores o Y). Si el archivo
/// no los trae, unos por defecto.
fn ejes_de(g: &Grafico, gr: &Grupo) -> (Eje, Eje) {
    let mios: Vec<&Eje> = gr
        .ejes
        .iter()
        .filter_map(|id| g.ejes.iter().find(|e| &e.id == id))
        .collect();
    let horizontal = matches!(gr.clase, Clase::Barras { horizontal: true });
    let base = &g.texto;
    if gr.clase.xy() {
        let x = mios
            .iter()
            .find(|e| matches!(e.pos, 'b' | 't'))
            .or(mios.first())
            .copied();
        let y = mios
            .iter()
            .find(|e| Some(e.id.as_str()) != x.map(|x| x.id.as_str()))
            .copied();
        let mut dx = Eje::defecto(true, 'b', base);
        dx.cuadricula = None;
        return (
            x.cloned().unwrap_or(dx),
            y.cloned().unwrap_or_else(|| Eje::defecto(true, 'l', base)),
        );
    }
    let cat = mios.iter().find(|e| !e.valores).map(|e| (*e).clone());
    let val = mios.iter().find(|e| e.valores).map(|e| (*e).clone());
    (
        cat.unwrap_or_else(|| Eje::defecto(false, if horizontal { 'l' } else { 'b' }, base)),
        val.unwrap_or_else(|| Eje::defecto(true, if horizontal { 'b' } else { 'l' }, base)),
    )
}

fn unir(a: Option<(f64, f64)>, b: Option<(f64, f64)>) -> Option<(f64, f64)> {
    match (a, b) {
        (Some(x), Some(y)) => Some((x.0.min(y.0), x.1.max(y.1))),
        (x, None) => x,
        (None, y) => y,
    }
}

/// La suma de los valores absolutos de cada categoria (para el 100 %).
fn totales(gr: &Grupo, n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| {
            gr.series
                .iter()
                .map(|s| s.valores.get(i).copied().flatten().unwrap_or(0.0).abs())
                .sum()
        })
        .collect()
}

/// De donde a donde van los valores de un grupo (las sumas si apila).
fn rango_grupo(gr: &Grupo, n: usize) -> Option<(f64, f64)> {
    let mut r: Option<(f64, f64)> = None;
    let mut meter = |v: f64| {
        if v.is_finite() {
            r = Some(match r {
                Some((a, b)) => (a.min(v), b.max(v)),
                None => (v, v),
            });
        }
    };
    match gr.agrupacion {
        Agrupacion::Normal => gr
            .series
            .iter()
            .flat_map(|s| s.valores.iter().flatten())
            .for_each(|v| meter(*v)),
        Agrupacion::Apilada | Agrupacion::Porcentaje => {
            let totales = totales(gr, n);
            for (i, total) in totales.iter().enumerate() {
                let (mut pos, mut neg, mut hay) = (0.0, 0.0, false);
                for s in &gr.series {
                    if let Some(v) = s.valores.get(i).copied().flatten() {
                        hay = true;
                        if v >= 0.0 {
                            pos += v;
                        } else {
                            neg += v;
                        }
                    }
                }
                if hay {
                    let d = if gr.agrupacion == Agrupacion::Porcentaje && *total > 0.0 {
                        *total
                    } else {
                        1.0
                    };
                    meter(pos / d);
                    meter(neg / d);
                }
            }
        }
    }
    r
}

fn rango_x(grupos: &[&Grupo]) -> Option<(f64, f64)> {
    let mut r: Option<(f64, f64)> = None;
    for gr in grupos.iter().filter(|g| g.clase.xy()) {
        for s in &gr.series {
            for (i, v) in s.valores.iter().enumerate() {
                if v.is_none() {
                    continue;
                }
                let x = s.xs.get(i).copied().flatten().unwrap_or((i + 1) as f64);
                r = unir(r, Some((x, x)));
            }
        }
    }
    r
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::medidor::MedidorFijo;
    use crate::tema::Tema;
    use pixpin_docs::xml::leer as xml;
    use std::collections::HashMap;

    const C: &str = r#"xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main""#;

    fn grafico(cuerpo: &str) -> Option<Grafico> {
        let t = Tema::leer(&Nodo::default());
        let m = HashMap::new();
        let x = format!(r#"<c:chartSpace {C}><c:chart>{cuerpo}</c:chart></c:chartSpace>"#);
        leer(&xml(&x).unwrap(), Contexto { tema: &t, mapa: &m })
    }

    fn serie(nombre: &str, cats: &[&str], vals: &[&str]) -> String {
        let pts = |v: &[&str]| {
            v.iter()
                .enumerate()
                .map(|(i, x)| format!(r#"<c:pt idx="{i}"><c:v>{x}</c:v></c:pt>"#))
                .collect::<String>()
        };
        format!(
            r#"<c:ser><c:idx val="0"/><c:order val="0"/><c:tx><c:strRef><c:strCache><c:ptCount val="1"/><c:pt idx="0"><c:v>{nombre}</c:v></c:pt></c:strCache></c:strRef></c:tx>
            <c:cat><c:strRef><c:strCache><c:ptCount val="{}"/>{}</c:strCache></c:strRef></c:cat>
            <c:val><c:numRef><c:numCache><c:formatCode>General</c:formatCode><c:ptCount val="{}"/>{}</c:numCache></c:numRef></c:val></c:ser>"#,
            cats.len(),
            pts(cats),
            vals.len(),
            pts(vals)
        )
    }

    fn barras(series: &str) -> String {
        format!(
            r#"<c:plotArea><c:barChart><c:barDir val="col"/><c:grouping val="clustered"/>{series}<c:axId val="1"/><c:axId val="2"/></c:barChart>
            <c:catAx><c:axId val="1"/><c:axPos val="b"/><c:crossAx val="2"/></c:catAx>
            <c:valAx><c:axId val="2"/><c:axPos val="l"/><c:majorGridlines/><c:crossAx val="1"/></c:valAx></c:plotArea>"#
        )
    }

    fn textos(els: &[Elemento], v: &mut Vec<String>) {
        for e in els {
            match e {
                Elemento::Texto(t) => v.push(t.texto.clone()),
                Elemento::Grupo { elementos, .. } => textos(elementos, v),
                _ => {}
            }
        }
    }

    fn rects(els: &[Elemento], v: &mut Vec<Rect>) {
        for e in els {
            match e {
                Elemento::Rect { rect, .. } => v.push(*rect),
                Elemento::Grupo { elementos, .. } => rects(elementos, v),
                _ => {}
            }
        }
    }

    #[test]
    fn se_leen_las_series_de_la_cache() {
        let g = grafico(&barras(&serie(
            "Ventas",
            &["Ene", "Feb", "Mar"],
            &["4.5", "2", "7"],
        )))
        .unwrap();
        assert_eq!(g.categorias, vec!["Ene", "Feb", "Mar"]);
        let s = &g.grupos[0].series[0];
        assert_eq!(s.nombre, "Ventas");
        assert_eq!(s.valores, vec![Some(4.5), Some(2.0), Some(7.0)]);
        // El color automatico de la primera serie es el acento 1 del tema.
        assert_eq!(s.auto, Color::hex("4472C4").unwrap());
        // Una sola serie y sin titulo borrado: el titulo es su nombre.
        assert_eq!(g.titulo.as_ref().unwrap().lineas, vec!["Ventas"]);
    }

    #[test]
    fn un_valor_que_no_es_numero_queda_como_hueco() {
        let g = grafico(&barras(&serie(
            "S",
            &["a", "b", "c"],
            &["1", "#N/A", "abc"],
        )))
        .unwrap();
        assert_eq!(g.grupos[0].series[0].valores, vec![Some(1.0), None, None]);
        let els = pintar(&g, 300.0, 200.0, &MedidorFijo::default());
        assert!(!els.is_empty());
    }

    #[test]
    fn una_cache_vacia_no_rompe_y_dibuja_los_ejes() {
        let s = r#"<c:ser><c:idx val="0"/><c:order val="0"/><c:val><c:numRef><c:f>Hoja1!$B$2:$B$4</c:f></c:numRef></c:val></c:ser>"#;
        let g = grafico(&barras(s)).unwrap();
        assert!(g.grupos[0].series[0].valores.is_empty());
        let els = pintar(&g, 300.0, 200.0, &MedidorFijo::default());
        let mut t = Vec::new();
        textos(&els, &mut t);
        // Sin datos, el eje va de 0 a 1.
        assert!(
            t.contains(&"0".to_string()) && t.contains(&"1".to_string()),
            "{t:?}"
        );
    }

    #[test]
    fn todo_ceros_da_un_eje_de_cero_a_uno() {
        let g = grafico(&barras(&serie("S", &["a", "b"], &["0", "0"]))).unwrap();
        let els = pintar(&g, 300.0, 200.0, &MedidorFijo::default());
        let mut t = Vec::new();
        textos(&els, &mut t);
        assert!(t.contains(&"1".to_string()), "{t:?}");
    }

    #[test]
    fn las_barras_negativas_bajan_del_cero() {
        let g = grafico(&barras(&serie("S", &["a", "b"], &["5", "-3"]))).unwrap();
        let els = pintar(&g, 300.0, 200.0, &MedidorFijo::default());
        let mut r = Vec::new();
        rects(&els, &mut r);
        // Las dos barras (el fondo no lleva relleno ni borde: no hay rect).
        assert_eq!(r.len(), 2, "{r:?}");
        // La positiva acaba donde empieza la negativa: en el cero.
        assert!((r[0].abajo() - r[1].y).abs() < 0.01, "{r:?}");
    }

    #[test]
    fn el_texto_lo_mide_el_medidor() {
        let g = grafico(&barras(&serie("S", &["Uno"], &["3"]))).unwrap();
        let m = MedidorFijo { proporcion: 0.5 };
        let els = pintar(&g, 300.0, 200.0, &m);
        fn buscar(els: &[Elemento]) -> Option<Texto> {
            els.iter().find_map(|e| match e {
                Elemento::Texto(t) if t.texto == "Uno" => Some(t.clone()),
                Elemento::Grupo { elementos, .. } => buscar(elementos),
                _ => None,
            })
        }
        let t = buscar(&els).unwrap();
        assert_eq!(t.ancho, 3.0 * 0.5 * t.letra.tam);
    }

    #[test]
    fn circular_lee_variedad_de_colores_y_porcentajes() {
        let x = format!(
            r#"<c:autoTitleDeleted val="1"/><c:plotArea><c:pieChart><c:varyColors val="1"/>{}<c:dLbls><c:showPercent val="1"/></c:dLbls></c:pieChart></c:plotArea><c:legend><c:legendPos val="b"/></c:legend>"#,
            serie("S", &["a", "b", "c", "d"], &["1", "1", "1", "1"])
        );
        let g = grafico(&x).unwrap();
        assert!(g.titulo.is_none());
        let els = pintar(&g, 300.0, 200.0, &MedidorFijo::default());
        let mut t = Vec::new();
        textos(&els, &mut t);
        assert_eq!(t.iter().filter(|x| *x == "25%").count(), 4, "{t:?}");
        // La leyenda: una entrada por categoria.
        for c in ["a", "b", "c", "d"] {
            assert!(t.contains(&c.to_string()));
        }
    }

    #[test]
    fn no_es_un_grafico() {
        let t = Tema::leer(&Nodo::default());
        let m = HashMap::new();
        assert!(leer(&xml("<a/>").unwrap(), Contexto { tema: &t, mapa: &m }).is_none());
        assert!(grafico("<c:plotArea/>").is_none());
    }

    #[test]
    fn el_paso_bonito_es_uno_dos_o_cinco() {
        assert_eq!(paso_bonito(0.49), 0.5);
        assert_eq!(paso_bonito(1.26), 2.0);
        assert_eq!(paso_bonito(8.4), 10.0);
        assert_eq!(paso_bonito(3.0), 5.0);
        assert_eq!(paso_bonito(0.0), 1.0);
        assert_eq!(paso_bonito(-3.0), 1.0);
        assert_eq!(paso_bonito(f64::NAN), 1.0);
    }

    #[test]
    fn la_escala_es_la_de_excel() {
        assert_eq!(
            escala(0.0, 4.7, None, None, None, 10),
            Escala {
                min: 0.0,
                max: 5.0,
                paso: 0.5
            }
        );
        assert_eq!(
            escala(0.0, 80.0, None, None, None, 10),
            Escala {
                min: 0.0,
                max: 90.0,
                paso: 10.0
            }
        );
        // Todo ceros.
        let e = escala(0.0, 0.0, None, None, None, 10);
        assert!(e.min == 0.0 && e.max > 0.0);
        // Negativos: el cero arriba.
        let e = escala(-5.0, -1.0, None, None, None, 10);
        assert_eq!(e.max, 0.0);
        assert!(e.min <= -5.0);
        // Mezcla: el cero es una marca.
        let e = escala(-3.0, 7.0, None, None, None, 10);
        assert!(
            e.min < -3.0 && e.max > 7.0 && e.marcas().contains(&0.0),
            "{e:?}"
        );
        // Lejos del cero: no empieza en cero.
        let e = escala(90.0, 100.0, None, None, None, 10);
        assert!(e.min > 0.0 && e.min <= 90.0 && e.max >= 100.0, "{e:?}");
        // Lo que dice el archivo manda.
        let e = escala(0.0, 4.7, Some(1.0), Some(4.0), Some(1.0), 10);
        assert_eq!(
            e,
            Escala {
                min: 1.0,
                max: 4.0,
                paso: 1.0
            }
        );
        // Datos imposibles.
        let e = escala(f64::NAN, f64::INFINITY, None, None, None, 10);
        assert!(e.min.is_finite() && e.max.is_finite() && e.max > e.min);
        // Un paso del archivo absurdo no da millones de marcas.
        let e = escala(0.0, 1e9, None, None, Some(1e-9), 10);
        assert!(e.marcas().len() <= MAX_MARCAS + 1);
    }

    #[test]
    fn las_marcas_no_arrastran_restos_de_coma_flotante() {
        let e = Escala {
            min: 0.0,
            max: 1.0,
            paso: 0.1,
        };
        let m = e.marcas();
        assert_eq!(m.len(), 11);
        assert_eq!(m[3], 0.3);
        assert_eq!(formatear(m[3], "General", Some(0.1)), "0.3");
    }

    #[test]
    fn los_formatos_de_numero() {
        assert_eq!(formatear(1234.5, "#,##0", None), "1,235");
        assert_eq!(formatear(0.256, "0%", None), "26%");
        assert_eq!(formatear(0.256, "0.0%", None), "25.6%");
        assert_eq!(formatear(-12.0, "0.00", None), "-12.00");
        assert_eq!(formatear(-12.0, "#,##0;(#,##0)", None), "(12)");
        assert_eq!(formatear(5.0, "\"$\"#,##0.00", None), "$5.00");
        assert_eq!(formatear(5.0, "#,##0.00 [$€-407]", None), "5.00 €");
        assert_eq!(formatear(1500.0, "#,##0,\"k\"", None), "2k");
        assert_eq!(formatear(12345.0, "0.00E+00", None), "1.23E+04");
        assert_eq!(formatear(2.5, "General", None), "2.5");
        assert_eq!(formatear(1.0 / 3.0, "", None), "0.333333333");
        assert_eq!(formatear(2.0, "General", Some(0.5)), "2");
        assert_eq!(formatear(1.5, "General", Some(0.5)), "1.5");
        assert_eq!(formatear(-0.0, "General", Some(1.0)), "0");
        // Fechas: 45292 es el 1 de enero de 2024.
        assert_eq!(formatear(45292.0, "d/m/yyyy", None), "1/1/2024");
        assert_eq!(formatear(45292.0, "mmm-yy", None), "ene-24");
        assert_eq!(formatear(f64::NAN, "0", None), "");
    }

    #[test]
    fn el_grafico_nuevo_sale_con_aviso_o_su_respaldo() {
        let ac = xml(
            r#"<mc:AlternateContent><mc:Choice Requires="cx1"><w:drawing><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/drawing/2014/chartex"/></a:graphic></w:drawing></mc:Choice><mc:Fallback><w:t>respaldo</w:t></mc:Fallback></mc:AlternateContent>"#,
        )
        .unwrap();
        assert_eq!(rama_alternativa(&ac).unwrap().nombre, "Fallback");
        let ac2 = xml(r#"<mc:AlternateContent><mc:Choice Requires="wps"><x/></mc:Choice><mc:Fallback><y/></mc:Fallback></mc:AlternateContent>"#).unwrap();
        assert_eq!(rama_alternativa(&ac2).unwrap().nombre, "Choice");
        let els = aviso(200.0, 100.0, "sin vista previa", &MedidorFijo::default());
        let mut t = Vec::new();
        textos(&els, &mut t);
        assert_eq!(t, vec!["sin vista previa"]);
        assert!(aviso(0.0, 0.0, "x", &MedidorFijo::default()).is_empty());
    }

    #[test]
    fn las_apiladas_al_cien_llegan_arriba() {
        let mut series = serie("A", &["x", "y"], &["1", "3"]);
        series.push_str(&serie("B", &["x", "y"], &["3", "1"]).replace(
            r#"<c:idx val="0"/><c:order val="0"/>"#,
            r#"<c:idx val="1"/><c:order val="1"/>"#,
        ));
        let x = barras(&series).replace("clustered", "percentStacked");
        let g = grafico(&x).unwrap();
        assert_eq!(g.grupos[0].agrupacion, Agrupacion::Porcentaje);
        assert_ne!(g.grupos[0].series[1].auto, g.grupos[0].series[0].auto);
        let els = pintar(&g, 300.0, 200.0, &MedidorFijo::default());
        let mut t = Vec::new();
        textos(&els, &mut t);
        assert!(t.contains(&"100%".to_string()), "{t:?}");
        let mut r = Vec::new();
        rects(&els, &mut r);
        assert_eq!(r.len(), 4);
        // Cada columna apilada mide lo mismo (el 100 %).
        let alto0 = r[0].alto + r[1].alto;
        let alto1 = r[2].alto + r[3].alto;
        assert!((alto0 - alto1).abs() < 0.01, "{r:?}");
    }

    #[test]
    fn la_paleta_sigue_el_orden_de_office() {
        let t = Tema::leer(&Nodo::default());
        let m = HashMap::new();
        let p = paleta(Contexto { tema: &t, mapa: &m }, 14);
        assert_eq!(p[0], Color::hex("4472C4").unwrap());
        assert_eq!(p[5], Color::hex("70AD47").unwrap());
        // La septima es el acento 1 mas oscuro.
        let (a, b) = (p[0], p[6]);
        assert!(b.r as u32 + b.g as u32 + b.b as u32 <= a.r as u32 + a.g as u32 + a.b as u32);
        assert_ne!(a, b);
    }
}
