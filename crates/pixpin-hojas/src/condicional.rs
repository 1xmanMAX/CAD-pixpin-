//! Formato condicional: comparar con valores (`cellIs`), buscar texto, los
//! primeros/ultimos N (o %), por encima/debajo del promedio (con
//! desviaciones), duplicados y unicos, periodos de fechas (hoy, la semana
//! pasada...), vacios y errores, escalas de 2 y 3 colores, barras de datos
//! (con la extension x14: negativos, borde, solido o degradado, eje) y los 20
//! conjuntos de iconos de Excel. Las reglas con formula libre
//! (`expression`) no se evaluan: no hay motor de formulas.
//!
//! **Las cuentas de cada bloque se hacen UNA vez**, al acabar de cargar la
//! hoja ([`Condicional::calcular`]): minimo, maximo, media, desviacion, los
//! numeros ordenados (percentiles y primeros N), la cuenta de repetidos y el
//! dia de hoy. Despues, evaluar una celda es mirar esos numeros: no se
//! recorre nada por celda ni por fotograma. Solo se hacen las cuentas que
//! alguna regla del bloque necesita.
//!
//! Partes traducidas de gridcraft (https://github.com/storytold/gridcraft), MIT OR Apache-2.0, Copyright (c) 2026 ArtCraft Team and the Gridcraft contributors.
//! (`engine/src/cf.rs`, `CfCache`: la idea de las cuentas por regla, los
//! periodos de fechas y el reparto de iconos). El resto, escrito contra
//! ECMA-376.

use std::collections::HashMap;

use crate::estilos::{Dxf, Rgb, Tema, color_de};
use crate::hoja::{Celda, Hoja, Rango, Valor};
use crate::xml::Nodo;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OpCelda {
    Entre,
    NoEntre,
    Igual,
    Distinto,
    Mayor,
    MayorIgual,
    Menor,
    MenorIgual,
}

/// Un valor de una regla: numero o texto.
#[derive(Debug, Clone, PartialEq)]
pub enum Dato {
    Numero(f64),
    Texto(String),
}

/// Un punto de una escala, barra o icono (`cfvo`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Punto {
    Minimo,
    Maximo,
    /// El minimo «automatico» de las barras x14: 0 si no hay negativos.
    AutoMinimo,
    /// El maximo «automatico»: 0 si todo es negativo.
    AutoMaximo,
    Numero(f64),
    Porcentaje(f64),
    Percentil(f64),
}

/// Los periodos de fechas de Excel (`timePeriod`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Periodo {
    Ayer,
    Hoy,
    Manana,
    Ultimos7,
    SemanaPasada,
    EstaSemana,
    SemanaSiguiente,
    MesPasado,
    EsteMes,
    MesSiguiente,
}

impl Periodo {
    fn de(s: &str) -> Option<Periodo> {
        Some(match s {
            "yesterday" => Periodo::Ayer,
            "today" => Periodo::Hoy,
            "tomorrow" => Periodo::Manana,
            "last7Days" => Periodo::Ultimos7,
            "lastWeek" => Periodo::SemanaPasada,
            "thisWeek" => Periodo::EstaSemana,
            "nextWeek" => Periodo::SemanaSiguiente,
            "lastMonth" => Periodo::MesPasado,
            "thisMonth" => Periodo::EsteMes,
            "nextMonth" => Periodo::MesSiguiente,
            _ => return None,
        })
    }

    /// Si la fecha (numero de serie, sistema 1900) cae en el periodo, siendo
    /// `hoy` el numero de serie de hoy. La semana empieza el domingo.
    pub fn cumple(self, serie: f64, hoy: f64) -> bool {
        let d = serie.floor();
        let hoy = hoy.floor();
        let (_, _, _, sem) = crate::formato::fecha_de_serie(hoy as i64, false);
        let ini_semana = hoy - sem as f64;
        let mes = |s: f64| {
            let (y, m, _, _) = crate::formato::fecha_de_serie(s as i64, false);
            y * 12 + m as i64 - 1
        };
        let mes_hoy = mes(hoy);
        match self {
            Periodo::Ayer => d == hoy - 1.0,
            Periodo::Hoy => d == hoy,
            Periodo::Manana => d == hoy + 1.0,
            Periodo::Ultimos7 => d > hoy - 7.0 && d <= hoy,
            Periodo::SemanaPasada => d >= ini_semana - 7.0 && d < ini_semana,
            Periodo::EstaSemana => d >= ini_semana && d < ini_semana + 7.0,
            Periodo::SemanaSiguiente => d >= ini_semana + 7.0 && d < ini_semana + 14.0,
            Periodo::MesPasado => d >= 1.0 && mes(d) == mes_hoy - 1,
            Periodo::EsteMes => d >= 1.0 && mes(d) == mes_hoy,
            Periodo::MesSiguiente => d >= 1.0 && mes(d) == mes_hoy + 1,
        }
    }
}

/// Minutos al este de UTC de la hora local. Este crate no sabe de Windows:
/// quien pinta lo fija ([`fijar_desfase_horario`]) para que «hoy» sea el dia
/// de quien mira y no el de Greenwich.
static DESFASE_MIN: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

pub fn fijar_desfase_horario(minutos_este_de_utc: i32) {
    DESFASE_MIN.store(
        minutos_este_de_utc.clamp(-14 * 60, 14 * 60),
        std::sync::atomic::Ordering::Relaxed,
    );
}

/// El numero de serie de hoy (sistema 1900, dia local).
fn hoy_serie() -> f64 {
    let s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let local = s + i64::from(DESFASE_MIN.load(std::sync::atomic::Ordering::Relaxed)) * 60;
    local.div_euclid(86_400) as f64 + 25_569.0
}

/// Los 20 conjuntos de iconos de Excel (los tres ultimos, de Excel 2010).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Conjunto {
    Flechas3,
    FlechasGris3,
    Banderas3,
    Semaforo3,
    Semaforo3Borde,
    Senales3,
    Simbolos3,
    Simbolos3Sin,
    Flechas4,
    FlechasGris4,
    RojoNegro4,
    Valoracion4,
    Semaforo4,
    Flechas5,
    FlechasGris5,
    Valoracion5,
    Cuartos5,
    Estrellas3,
    Triangulos3,
    Cajas5,
}

impl Conjunto {
    pub const TODOS: [Conjunto; 20] = [
        Conjunto::Flechas3,
        Conjunto::FlechasGris3,
        Conjunto::Banderas3,
        Conjunto::Semaforo3,
        Conjunto::Semaforo3Borde,
        Conjunto::Senales3,
        Conjunto::Simbolos3,
        Conjunto::Simbolos3Sin,
        Conjunto::Flechas4,
        Conjunto::FlechasGris4,
        Conjunto::RojoNegro4,
        Conjunto::Valoracion4,
        Conjunto::Semaforo4,
        Conjunto::Flechas5,
        Conjunto::FlechasGris5,
        Conjunto::Valoracion5,
        Conjunto::Cuartos5,
        Conjunto::Estrellas3,
        Conjunto::Triangulos3,
        Conjunto::Cajas5,
    ];

    /// El nombre del XML (`3Arrows`...).
    pub fn de_nombre(s: &str) -> Option<Conjunto> {
        Some(match s {
            "3Arrows" => Conjunto::Flechas3,
            "3ArrowsGray" => Conjunto::FlechasGris3,
            "3Flags" => Conjunto::Banderas3,
            "3TrafficLights1" => Conjunto::Semaforo3,
            "3TrafficLights2" => Conjunto::Semaforo3Borde,
            "3Signs" => Conjunto::Senales3,
            "3Symbols" => Conjunto::Simbolos3,
            "3Symbols2" => Conjunto::Simbolos3Sin,
            "4Arrows" => Conjunto::Flechas4,
            "4ArrowsGray" => Conjunto::FlechasGris4,
            "4RedToBlack" => Conjunto::RojoNegro4,
            "4Rating" => Conjunto::Valoracion4,
            "4TrafficLights" => Conjunto::Semaforo4,
            "5Arrows" => Conjunto::Flechas5,
            "5ArrowsGray" => Conjunto::FlechasGris5,
            "5Rating" => Conjunto::Valoracion5,
            "5Quarters" => Conjunto::Cuartos5,
            "3Stars" => Conjunto::Estrellas3,
            "3Triangles" => Conjunto::Triangulos3,
            "5Boxes" => Conjunto::Cajas5,
            _ => return None,
        })
    }

    pub fn nombre(self) -> &'static str {
        match self {
            Conjunto::Flechas3 => "3Arrows",
            Conjunto::FlechasGris3 => "3ArrowsGray",
            Conjunto::Banderas3 => "3Flags",
            Conjunto::Semaforo3 => "3TrafficLights1",
            Conjunto::Semaforo3Borde => "3TrafficLights2",
            Conjunto::Senales3 => "3Signs",
            Conjunto::Simbolos3 => "3Symbols",
            Conjunto::Simbolos3Sin => "3Symbols2",
            Conjunto::Flechas4 => "4Arrows",
            Conjunto::FlechasGris4 => "4ArrowsGray",
            Conjunto::RojoNegro4 => "4RedToBlack",
            Conjunto::Valoracion4 => "4Rating",
            Conjunto::Semaforo4 => "4TrafficLights",
            Conjunto::Flechas5 => "5Arrows",
            Conjunto::FlechasGris5 => "5ArrowsGray",
            Conjunto::Valoracion5 => "5Rating",
            Conjunto::Cuartos5 => "5Quarters",
            Conjunto::Estrellas3 => "3Stars",
            Conjunto::Triangulos3 => "3Triangles",
            Conjunto::Cajas5 => "5Boxes",
        }
    }

    /// Cuantos iconos tiene (3, 4 o 5).
    pub fn cantidad(self) -> usize {
        match self.nombre().as_bytes()[0] {
            b'3' => 3,
            b'4' => 4,
            _ => 5,
        }
    }

    /// Los limites por defecto de Excel: porcentajes iguales (33, 67...).
    fn cortes_defecto(self) -> Vec<(Punto, bool)> {
        let k = self.cantidad();
        (1..k)
            .map(|i| {
                (
                    Punto::Porcentaje((100.0 * i as f64 / k as f64).floor()),
                    true,
                )
            })
            .collect()
    }
}

/// Un icono concreto: conjunto e indice (0 = el peor).
pub type Icono = (Conjunto, u8);

/// Donde va el eje de una barra con negativos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Eje {
    Automatico,
    Medio,
    Ninguno,
}

/// Lo que trae la extension x14 de una barra de datos.
#[derive(Debug, Clone, PartialEq)]
pub struct BarraX14 {
    pub degradado: bool,
    /// Con borde (del color de la barra o el suyo).
    pub borde: Option<Rgb>,
    pub negativo: Rgb,
    pub borde_negativo: Option<Rgb>,
    pub eje: Eje,
    pub color_eje: Rgb,
    pub derecha_a_izquierda: bool,
    /// Largo minimo y maximo de la barra, en fraccion de la celda.
    pub largo_min: f32,
    pub largo_max: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BarraCf {
    pub min: Punto,
    pub max: Punto,
    pub color: Rgb,
    pub ver_valor: bool,
    /// El `x14:id` que une la regla con su extension.
    pub ext: Option<String>,
    pub x14: Option<BarraX14>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Regla {
    Celda {
        op: OpCelda,
        datos: Vec<Dato>,
        dxf: Option<u32>,
    },
    /// 0 contiene, 1 no contiene, 2 empieza por, 3 acaba en.
    Texto {
        modo: u8,
        texto: String,
        dxf: Option<u32>,
    },
    Primeros {
        rango: u32,
        porcentaje: bool,
        ultimos: bool,
        dxf: Option<u32>,
    },
    Promedio {
        encima: bool,
        igual: bool,
        /// Desviaciones tipicas sobre/bajo la media (0 = la media).
        desv: u32,
        dxf: Option<u32>,
    },
    /// Repetidos (`unicos = false`) o unicos.
    Duplicados {
        unicos: bool,
        dxf: Option<u32>,
    },
    Periodo {
        periodo: Periodo,
        dxf: Option<u32>,
    },
    /// Celdas vacias (`si`) o no vacias.
    Vacios {
        si: bool,
        dxf: Option<u32>,
    },
    /// Con error (`si`) o sin error.
    Errores {
        si: bool,
        dxf: Option<u32>,
    },
    Escala {
        puntos: Vec<(Punto, Rgb)>,
    },
    Barra(BarraCf),
    Iconos {
        conjunto: Conjunto,
        /// Los limites de los iconos 2..n: (punto, `>=` o solo `>`).
        cortes: Vec<(Punto, bool)>,
        invertir: bool,
        ver_valor: bool,
        /// Iconos elegidos a mano (x14), uno por posicion.
        a_mano: Vec<Option<Icono>>,
        ext: Option<String>,
    },
}

impl Regla {
    fn ext(&self) -> Option<&str> {
        match self {
            Regla::Barra(b) => b.ext.as_deref(),
            Regla::Iconos { ext, .. } => ext.as_deref(),
            _ => None,
        }
    }

    fn mira_texto(&self) -> bool {
        match self {
            Regla::Texto { .. } => true,
            Regla::Celda { datos, .. } => datos.iter().any(|d| matches!(d, Dato::Texto(_))),
            _ => false,
        }
    }
}

/// La identidad de un valor para contar repetidos.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Clave {
    Numero(u64),
    Compartido(u32),
    Texto(String),
    Logico(bool),
    Error(u8),
}

fn clave_de(c: &Celda, hoja: &Hoja) -> Option<Clave> {
    Some(match c.valor {
        Valor::Vacio => return None,
        Valor::Numero(v) => Clave::Numero(if v == 0.0 { 0 } else { v.to_bits() }),
        Valor::Compartido(i) => Clave::Compartido(i),
        Valor::Propio(i) => Clave::Texto(hoja.textos.get(i as usize)?.to_lowercase()),
        Valor::Logico(b) => Clave::Logico(b),
        Valor::Error(e) => Clave::Error(e),
    })
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Estadistica {
    pub min: f64,
    pub max: f64,
    pub media: f64,
    /// Desviacion tipica de la muestra.
    pub desv: f64,
    /// Los numeros ordenados (para percentiles y primeros N).
    pub ordenados: Vec<f64>,
    /// Cuantas veces sale cada valor (solo si hay reglas de duplicados).
    pub cuentas: HashMap<Clave, u32>,
    /// El numero de serie de hoy (solo si hay reglas de periodos).
    pub hoy: f64,
}

/// Un bloque `<conditionalFormatting sqref>` con sus reglas por prioridad.
#[derive(Debug, Clone, PartialEq)]
pub struct Condicional {
    pub rangos: Vec<Rango>,
    pub reglas: Vec<(i32, Regla, bool)>,
    pub estadistica: Option<Estadistica>,
    /// Alguna regla mira el texto mostrado (para no darle forma si no).
    pub necesita_texto: bool,
    /// Alguna regla es de celdas vacias (las que si existen, con estilo).
    pub con_vacios: bool,
}

/// La barra ya medida para una celda: tramo horizontal en fraccion de la
/// celda (0 = izquierda, 1 = derecha).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarraPintada {
    pub desde: f32,
    pub hasta: f32,
    pub color: Rgb,
    pub borde: Option<Rgb>,
    pub degradado: bool,
    /// Linea del eje: posicion y color.
    pub eje: Option<(f32, Rgb)>,
}

/// Lo que el formato condicional hace a una celda.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Efecto {
    pub dxf: Dxf,
    /// Color de una escala (relleno).
    pub escala: Option<Rgb>,
    pub barra: Option<BarraPintada>,
    pub icono: Option<Icono>,
    /// La barra o el icono piden esconder el valor.
    pub ocultar_valor: bool,
}

// ---------------------------------------------------------------- lectura

fn dato(s: &str) -> Option<Dato> {
    let s = s.trim();
    if let Some(t) = s.strip_prefix('"').and_then(|t| t.strip_suffix('"')) {
        return Some(Dato::Texto(t.replace("\"\"", "\"")));
    }
    s.parse::<f64>().ok().map(Dato::Numero)
}

/// El nombre sin prefijo de espacio de nombres.
fn loc(n: &Nodo) -> &str {
    n.nombre.rsplit(':').next().unwrap_or("")
}

fn hijo_l<'a>(n: &'a Nodo, l: &str) -> Option<&'a Nodo> {
    n.hijos.iter().find(|h| loc(h) == l)
}

fn hijos_l<'a>(n: &'a Nodo, l: &'a str) -> impl Iterator<Item = &'a Nodo> + 'a {
    n.hijos.iter().filter(move |h| loc(h) == l)
}

/// Los bloques de la extension x14 (sin atributo `sqref`; el lector de XML
/// ya quita los prefijos de espacio de nombres).
fn todos_con_prefijo<'a>(n: &'a Nodo, l: &str, salida: &mut Vec<&'a Nodo>) {
    for h in &n.hijos {
        if h.attr("sqref").is_none() && loc(h) == l {
            salida.push(h);
        }
        todos_con_prefijo(h, l, salida);
    }
}

fn si(n: &Nodo, a: &str, defecto: bool) -> bool {
    match n.attr(a) {
        Some("1") | Some("true") => true,
        Some("0") | Some("false") => false,
        _ => defecto,
    }
}

fn punto(n: &Nodo) -> Punto {
    // En la extension x14 el valor va en un hijo `xm:f`.
    let val = n
        .attr("val")
        .map(str::to_string)
        .or_else(|| hijo_l(n, "f").map(|f| f.texto.clone()))
        .and_then(|v| v.trim().parse::<f64>().ok());
    match n.attr("type").unwrap_or("") {
        "min" => Punto::Minimo,
        "max" => Punto::Maximo,
        "autoMin" => Punto::AutoMinimo,
        "autoMax" => Punto::AutoMaximo,
        "percent" => Punto::Porcentaje(val.unwrap_or(0.0)),
        "percentile" => Punto::Percentil(val.unwrap_or(50.0)),
        _ => val.map(Punto::Numero).unwrap_or(Punto::Minimo),
    }
}

/// El `x14:id` de la extension de una regla.
fn id_ext(r: &Nodo) -> Option<String> {
    let l = hijo_l(r, "extLst")?;
    for e in &l.hijos {
        if let Some(i) = hijo_l(e, "id") {
            let t = i.texto.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

/// Lee los `<conditionalFormatting>` de la cola de una hoja (los de
/// siempre y los de la extension x14).
pub fn leer(raiz: &Nodo, tema: &Tema, paleta: &[u32]) -> Vec<Condicional> {
    let mut v = Vec::new();
    let mut bloques = Vec::new();
    raiz.todos("conditionalFormatting", &mut bloques);
    for b in bloques {
        let rangos: Vec<Rango> = b
            .attr("sqref")
            .unwrap_or("")
            .split_whitespace()
            .filter_map(Rango::leer)
            .collect();
        if rangos.is_empty() {
            continue;
        }
        let mut reglas = Vec::new();
        for r in b.hijos_de("cfRule") {
            let prioridad = r
                .attr("priority")
                .and_then(|p| p.parse().ok())
                .unwrap_or(i32::MAX);
            let parar = si(r, "stopIfTrue", false);
            let Some(regla) = regla_de(r, tema, paleta) else {
                continue;
            };
            reglas.push((prioridad, regla, parar));
        }
        if let Some(c) = Condicional::nuevo(rangos, reglas) {
            v.push(c);
        }
    }
    leer_x14(raiz, tema, paleta, &mut v);
    v
}

fn regla_de(r: &Nodo, tema: &Tema, paleta: &[u32]) -> Option<Regla> {
    let dxf = r.attr("dxfId").and_then(|d| d.parse().ok());
    let formulas: Vec<String> = r.hijos_de("formula").map(|f| f.texto.clone()).collect();
    Some(match r.attr("type").unwrap_or("") {
        "cellIs" => {
            let op = match r.attr("operator").unwrap_or("") {
                "between" => OpCelda::Entre,
                "notBetween" => OpCelda::NoEntre,
                "equal" => OpCelda::Igual,
                "notEqual" => OpCelda::Distinto,
                "greaterThan" => OpCelda::Mayor,
                "greaterThanOrEqual" => OpCelda::MayorIgual,
                "lessThan" => OpCelda::Menor,
                "lessThanOrEqual" => OpCelda::MenorIgual,
                _ => return None,
            };
            let datos: Vec<Dato> = formulas.iter().filter_map(|f| dato(f)).collect();
            // Una formula que no es un literal no se puede evaluar.
            if datos.is_empty() || datos.len() != formulas.len() {
                return None;
            }
            Regla::Celda { op, datos, dxf }
        }
        t @ ("containsText" | "notContainsText" | "beginsWith" | "endsWith") => {
            let texto = r.attr("text")?;
            let modo = match t {
                "containsText" => 0,
                "notContainsText" => 1,
                "beginsWith" => 2,
                _ => 3,
            };
            Regla::Texto {
                modo,
                texto: texto.to_lowercase(),
                dxf,
            }
        }
        "top10" => Regla::Primeros {
            rango: r.attr("rank").and_then(|v| v.parse().ok()).unwrap_or(10),
            porcentaje: si(r, "percent", false),
            ultimos: si(r, "bottom", false),
            dxf,
        },
        "aboveAverage" => Regla::Promedio {
            encima: si(r, "aboveAverage", true),
            igual: si(r, "equalAverage", false),
            desv: r.attr("stdDev").and_then(|v| v.parse().ok()).unwrap_or(0),
            dxf,
        },
        "duplicateValues" => Regla::Duplicados { unicos: false, dxf },
        "uniqueValues" => Regla::Duplicados { unicos: true, dxf },
        "timePeriod" => Regla::Periodo {
            periodo: Periodo::de(r.attr("timePeriod")?)?,
            dxf,
        },
        "containsBlanks" => Regla::Vacios { si: true, dxf },
        "notContainsBlanks" => Regla::Vacios { si: false, dxf },
        "containsErrors" => Regla::Errores { si: true, dxf },
        "notContainsErrors" => Regla::Errores { si: false, dxf },
        "colorScale" => {
            let e = r.hijo("colorScale")?;
            let ps: Vec<Punto> = e.hijos_de("cfvo").map(punto).collect();
            let cs: Vec<Rgb> = e
                .hijos_de("color")
                .map(|c| color_de(c, tema, paleta).unwrap_or(Rgb::BLANCO))
                .collect();
            if ps.len() < 2 || ps.len() != cs.len() {
                return None;
            }
            Regla::Escala {
                puntos: ps.into_iter().zip(cs).collect(),
            }
        }
        "dataBar" => {
            let e = r.hijo("dataBar")?;
            let ps: Vec<Punto> = e.hijos_de("cfvo").map(punto).collect();
            let color = e
                .hijo("color")
                .and_then(|c| color_de(c, tema, paleta))
                .unwrap_or(COLOR_BARRA);
            Regla::Barra(BarraCf {
                min: ps.first().copied().unwrap_or(Punto::Minimo),
                max: ps.get(1).copied().unwrap_or(Punto::Maximo),
                color,
                ver_valor: si(e, "showValue", true),
                ext: id_ext(r),
                x14: None,
            })
        }
        "iconSet" => {
            let e = r.hijo("iconSet")?;
            let conjunto = Conjunto::de_nombre(e.attr("iconSet").unwrap_or("3TrafficLights1"))?;
            iconos(conjunto, e, id_ext(r))
        }
        _ => return None,
    })
}

const COLOR_BARRA: Rgb = Rgb {
    r: 0x63,
    g: 0x8E,
    b: 0xC6,
};

fn iconos(conjunto: Conjunto, e: &Nodo, ext: Option<String>) -> Regla {
    let k = conjunto.cantidad();
    // El primer `cfvo` es el limite inferior del primer icono: no cuenta.
    let mut cortes: Vec<(Punto, bool)> = hijos_l(e, "cfvo")
        .skip(1)
        .map(|c| (punto(c), si(c, "gte", true)))
        .collect();
    if cortes.len() != k - 1 {
        cortes = conjunto.cortes_defecto();
    }
    let a_mano: Vec<Option<Icono>> = if si(e, "custom", false) {
        hijos_l(e, "cfIcon")
            .map(|i| {
                let c = Conjunto::de_nombre(i.attr("iconSet")?)?;
                let id: u8 = i.attr("iconId")?.parse().ok()?;
                (usize::from(id) < c.cantidad()).then_some((c, id))
            })
            .collect()
    } else {
        Vec::new()
    };
    Regla::Iconos {
        conjunto,
        cortes,
        invertir: si(e, "reverse", false),
        ver_valor: si(e, "showValue", true),
        a_mano,
        ext,
    }
}

/// La extension x14: barras con negativos y borde, e iconos extra o a mano.
fn leer_x14(raiz: &Nodo, tema: &Tema, paleta: &[u32], v: &mut Vec<Condicional>) {
    let mut bloques = Vec::new();
    todos_con_prefijo(raiz, "conditionalFormatting", &mut bloques);
    for b in bloques {
        let rangos: Vec<Rango> = hijo_l(b, "sqref")
            .map(|s| s.texto.split_whitespace().filter_map(Rango::leer).collect())
            .unwrap_or_default();
        for r in hijos_l(b, "cfRule") {
            let prioridad = r
                .attr("priority")
                .and_then(|p| p.parse().ok())
                .unwrap_or(i32::MAX);
            let parar = si(r, "stopIfTrue", false);
            let id = r.attr("id").map(str::to_string);
            let col = |n: Option<&Nodo>| n.and_then(|c| color_de(c, tema, paleta));
            let nueva = match r.attr("type").unwrap_or("") {
                "dataBar" => {
                    let Some(e) = hijo_l(r, "dataBar") else {
                        continue;
                    };
                    let ps: Vec<Punto> = hijos_l(e, "cfvo").map(punto).collect();
                    let borde_propio = col(hijo_l(e, "borderColor"));
                    let mismo_neg = si(e, "negativeBarColorSameAsPositive", false);
                    let x14 = BarraX14 {
                        degradado: si(e, "gradient", true),
                        borde: si(e, "border", false)
                            .then_some(borde_propio.unwrap_or(COLOR_BARRA)),
                        negativo: if mismo_neg {
                            COLOR_BARRA
                        } else {
                            col(hijo_l(e, "negativeFillColor")).unwrap_or(Rgb {
                                r: 0xFF,
                                g: 0,
                                b: 0,
                            })
                        },
                        borde_negativo: (si(e, "border", false)
                            && !si(e, "negativeBarBorderColorSameAsPositive", true))
                        .then(|| {
                            col(hijo_l(e, "negativeBorderColor")).unwrap_or(Rgb {
                                r: 0xFF,
                                g: 0,
                                b: 0,
                            })
                        }),
                        eje: match e.attr("axisPosition") {
                            Some("middle") => Eje::Medio,
                            Some("none") => Eje::Ninguno,
                            _ => Eje::Automatico,
                        },
                        color_eje: col(hijo_l(e, "axisColor")).unwrap_or(Rgb::NEGRO),
                        derecha_a_izquierda: e.attr("direction") == Some("rightToLeft"),
                        largo_min: e
                            .attr("minLength")
                            .and_then(|x| x.parse::<f32>().ok())
                            .unwrap_or(10.0)
                            .clamp(0.0, 100.0)
                            / 100.0,
                        largo_max: e
                            .attr("maxLength")
                            .and_then(|x| x.parse::<f32>().ok())
                            .unwrap_or(90.0)
                            .clamp(0.0, 100.0)
                            / 100.0,
                    };
                    let min = ps.first().copied().unwrap_or(Punto::AutoMinimo);
                    let max = ps.get(1).copied().unwrap_or(Punto::AutoMaximo);
                    Regla::Barra(BarraCf {
                        min,
                        max,
                        color: COLOR_BARRA,
                        ver_valor: true,
                        ext: id.clone(),
                        x14: Some(x14),
                    })
                }
                "iconSet" => {
                    let Some(e) = hijo_l(r, "iconSet") else {
                        continue;
                    };
                    let Some(conjunto) = Conjunto::de_nombre(e.attr("iconSet").unwrap_or(""))
                    else {
                        continue;
                    };
                    iconos(conjunto, e, id.clone())
                }
                _ => continue,
            };
            // Si la regla de siempre existe (mismo id), se completa con esta.
            let mut hecho = false;
            if let Some(id) = id.as_deref() {
                'busca: for c in v.iter_mut() {
                    for (_, regla, _) in c.reglas.iter_mut() {
                        if regla.ext() != Some(id) {
                            continue;
                        }
                        match (&mut *regla, &nueva) {
                            (Regla::Barra(a), Regla::Barra(n)) => {
                                a.x14 = n.x14.clone();
                                a.min = n.min;
                                a.max = n.max;
                                if let Some(x) = &mut a.x14 {
                                    // Los colores de la barra salen de la regla de siempre.
                                    if x.borde.is_some() && x.borde == Some(COLOR_BARRA) {
                                        x.borde = Some(a.color);
                                    }
                                    if x.negativo == COLOR_BARRA {
                                        x.negativo = a.color;
                                    }
                                }
                            }
                            (a, n @ Regla::Iconos { .. }) => *a = n.clone(),
                            _ => continue,
                        }
                        hecho = true;
                        break 'busca;
                    }
                }
            }
            if !hecho && !rangos.is_empty() {
                if let Some(c) = Condicional::nuevo(rangos.clone(), vec![(prioridad, nueva, parar)])
                {
                    v.push(c);
                }
            }
        }
    }
}

impl Condicional {
    fn nuevo(rangos: Vec<Rango>, mut reglas: Vec<(i32, Regla, bool)>) -> Option<Condicional> {
        if reglas.is_empty() {
            return None;
        }
        reglas.sort_by_key(|r| r.0);
        let necesita_texto = reglas.iter().any(|r| r.1.mira_texto());
        let con_vacios = reglas.iter().any(|r| matches!(r.1, Regla::Vacios { .. }));
        Some(Condicional {
            rangos,
            reglas,
            estadistica: None,
            necesita_texto,
            con_vacios,
        })
    }

    pub fn aplica(&self, fila: u32, col: u32) -> bool {
        self.rangos.iter().any(|r| r.contiene(fila, col))
    }

    /// Hace las cuentas del bloque UNA vez: minimo, maximo, media,
    /// desviacion y orden de los numeros de sus rangos; repetidos y hoy
    /// solo si alguna regla los pide.
    pub fn calcular(&mut self, hoja: &Hoja) {
        let (mut numeros, mut repetidos, mut fechas) = (false, false, false);
        for (_, r, _) in &self.reglas {
            match r {
                Regla::Primeros { .. }
                | Regla::Promedio { .. }
                | Regla::Escala { .. }
                | Regla::Barra(_)
                | Regla::Iconos { .. } => numeros = true,
                Regla::Duplicados { .. } => repetidos = true,
                Regla::Periodo { .. } => fechas = true,
                _ => {}
            }
        }
        let mut est = Estadistica::default();
        if fechas {
            est.hoy = hoy_serie();
        }
        if !numeros && !repetidos {
            self.estadistica = Some(est);
            return;
        }
        // Tope de celdas miradas: un rango absurdo no cuelga la apertura.
        let mut quedan = 5_000_000usize;
        let mut nums = Vec::new();
        for r in &self.rangos {
            for f in hoja.filas_entre(r.fila0, r.fila1) {
                let cs = hoja.celdas_de(f);
                let ini = cs.partition_point(|c| c.col < r.col0);
                for c in &cs[ini..] {
                    if c.col > r.col1 || quedan == 0 {
                        break;
                    }
                    quedan -= 1;
                    if numeros {
                        if let Valor::Numero(v) = c.valor {
                            if v.is_finite() {
                                nums.push(v);
                            }
                        }
                    }
                    if repetidos {
                        if let Some(k) = clave_de(c, hoja) {
                            *est.cuentas.entry(k).or_insert(0) += 1;
                        }
                    }
                }
            }
        }
        if !nums.is_empty() {
            nums.sort_by(|a, b| a.total_cmp(b));
            let n = nums.len() as f64;
            let media = nums.iter().sum::<f64>() / n;
            let var = if nums.len() > 1 {
                nums.iter().map(|x| (x - media).powi(2)).sum::<f64>() / (n - 1.0)
            } else {
                0.0
            };
            est.min = nums[0];
            est.max = nums[nums.len() - 1];
            est.media = media;
            est.desv = var.sqrt();
            est.ordenados = nums;
        }
        self.estadistica = Some(est);
    }

    fn valor_de(&self, p: Punto, e: &Estadistica) -> f64 {
        match p {
            Punto::Minimo => e.min,
            Punto::Maximo => e.max,
            Punto::AutoMinimo => e.min.min(0.0),
            Punto::AutoMaximo => e.max.max(0.0),
            Punto::Numero(n) => n,
            Punto::Porcentaje(q) => e.min + (e.max - e.min) * q / 100.0,
            Punto::Percentil(q) => {
                if e.ordenados.is_empty() {
                    return 0.0;
                }
                let pos = (q / 100.0).clamp(0.0, 1.0) * (e.ordenados.len() - 1) as f64;
                let i = pos.floor() as usize;
                let j = (i + 1).min(e.ordenados.len() - 1);
                e.ordenados[i] + (e.ordenados[j] - e.ordenados[i]) * (pos - i as f64)
            }
        }
    }

    /// Mide la barra de una celda con valor `v`.
    fn barra(&self, v: f64, e: &Estadistica, b: &BarraCf) -> BarraPintada {
        let (a, z) = (self.valor_de(b.min, e), self.valor_de(b.max, e));
        let Some(x) = &b.x14 else {
            // Barra de 2007: siempre desde la izquierda, con degradado; Excel
            // deja un 10 % minimo y un 90 % maximo.
            let t = if z > a {
                ((v - a) / (z - a)).clamp(0.0, 1.0)
            } else {
                1.0
            };
            return BarraPintada {
                desde: 0.0,
                hasta: 0.1 + 0.8 * t as f32,
                color: b.color,
                borde: None,
                degradado: true,
                eje: None,
            };
        };
        let (lmin, lmax) = (x.largo_min.min(x.largo_max), x.largo_max.max(x.largo_min));
        // Donde cae el eje (fraccion de la celda desde la izquierda).
        let eje_pos = match x.eje {
            Eje::Ninguno => 0.0,
            Eje::Medio => 0.5,
            Eje::Automatico => {
                if a < 0.0 {
                    if z <= 0.0 {
                        1.0
                    } else {
                        ((-a) / (z - a)).clamp(0.0, 1.0) as f32
                    }
                } else {
                    0.0
                }
            }
        };
        let con_eje = eje_pos > 0.0;
        let (desde, hasta, color, borde) = if v >= 0.0 {
            let lo = a.max(0.0);
            let t = if z > lo {
                ((v - lo) / (z - lo)).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let largo = (1.0 - eje_pos) * (lmin + (lmax - lmin) * t as f32);
            (eje_pos, eje_pos + largo, b.color, x.borde)
        } else {
            let t = if a < 0.0 {
                (v / a).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let sitio = if con_eje { eje_pos } else { 1.0 };
            let largo = sitio * (lmin + (lmax - lmin) * t as f32);
            let ini = if con_eje { eje_pos - largo } else { 0.0 };
            (
                ini,
                ini + largo,
                x.negativo,
                x.borde
                    .map(|_| x.borde_negativo.unwrap_or(x.borde.unwrap_or(x.negativo))),
            )
        };
        let (desde, hasta, eje) = if x.derecha_a_izquierda {
            (
                1.0 - hasta,
                1.0 - desde,
                con_eje.then_some((1.0 - eje_pos, x.color_eje)),
            )
        } else {
            (desde, hasta, con_eje.then_some((eje_pos, x.color_eje)))
        };
        BarraPintada {
            desde,
            hasta,
            color,
            borde,
            degradado: x.degradado,
            eje: if x.eje == Eje::Ninguno { None } else { eje },
        }
    }

    /// Lo que las reglas de este bloque hacen a la celda, sobre `ef`.
    /// Devuelve `true` si una regla pidio parar (`stopIfTrue`).
    pub fn evaluar(
        &self,
        hoja: &Hoja,
        celda: &Celda,
        texto: &str,
        dxfs: &[Dxf],
        ef: &mut Efecto,
    ) -> bool {
        let Some(e) = &self.estadistica else {
            return false;
        };
        let num = match celda.valor {
            Valor::Numero(v) => Some(v),
            _ => None,
        };
        for (_, regla, parar) in &self.reglas {
            let (cumple, dxf) = match regla {
                Regla::Celda { op, datos, dxf } => {
                    let cmp = |d: &Dato| -> Option<std::cmp::Ordering> {
                        match (d, num) {
                            (Dato::Numero(x), Some(v)) => v.partial_cmp(x),
                            (Dato::Texto(t), None) => {
                                Some(texto.to_lowercase().cmp(&t.to_lowercase()))
                            }
                            _ => None,
                        }
                    };
                    use std::cmp::Ordering::*;
                    let a = datos.first().and_then(cmp);
                    let b = datos.get(1).and_then(cmp);
                    let ok = match op {
                        OpCelda::Igual => a == Some(Equal),
                        OpCelda::Distinto => a.is_some_and(|o| o != Equal),
                        OpCelda::Mayor => a == Some(Greater),
                        OpCelda::MayorIgual => matches!(a, Some(Greater | Equal)),
                        OpCelda::Menor => a == Some(Less),
                        OpCelda::MenorIgual => matches!(a, Some(Less | Equal)),
                        OpCelda::Entre | OpCelda::NoEntre => {
                            // Entre en cualquier orden de los dos limites.
                            let dentro = matches!(
                                (a, b),
                                (Some(Greater | Equal), Some(Less | Equal))
                                    | (Some(Less | Equal), Some(Greater | Equal))
                            );
                            (a.is_some() && b.is_some()) && (dentro == (*op == OpCelda::Entre))
                        }
                    };
                    (ok, *dxf)
                }
                Regla::Texto {
                    modo,
                    texto: buscado,
                    dxf,
                } => {
                    let t = texto.to_lowercase();
                    let ok = match modo {
                        0 => t.contains(buscado.as_str()),
                        1 => !t.contains(buscado.as_str()),
                        2 => t.starts_with(buscado.as_str()),
                        _ => t.ends_with(buscado.as_str()),
                    };
                    (ok && celda.valor != Valor::Vacio, *dxf)
                }
                Regla::Primeros {
                    rango,
                    porcentaje,
                    ultimos,
                    dxf,
                } => {
                    let ok = num.is_some_and(|v| {
                        let n = e.ordenados.len();
                        let k = if *porcentaje {
                            ((n as f64 * *rango as f64 / 100.0).floor() as usize).max(1)
                        } else {
                            *rango as usize
                        }
                        .min(n);
                        if k == 0 {
                            return false;
                        }
                        if *ultimos {
                            v <= e.ordenados[k - 1]
                        } else {
                            v >= e.ordenados[n - k]
                        }
                    });
                    (ok, *dxf)
                }
                Regla::Promedio {
                    encima,
                    igual,
                    desv,
                    dxf,
                } => {
                    let ok = num.is_some_and(|v| {
                        let limite =
                            e.media + if *encima { 1.0 } else { -1.0 } * e.desv * f64::from(*desv);
                        if *encima {
                            v > limite || (*igual && v == limite)
                        } else {
                            v < limite || (*igual && v == limite)
                        }
                    });
                    (ok, *dxf)
                }
                Regla::Duplicados { unicos, dxf } => {
                    let veces = clave_de(celda, hoja)
                        .and_then(|k| e.cuentas.get(&k).copied())
                        .unwrap_or(0);
                    (if *unicos { veces == 1 } else { veces > 1 }, *dxf)
                }
                Regla::Periodo { periodo, dxf } => {
                    (num.is_some_and(|v| periodo.cumple(v, e.hoy)), *dxf)
                }
                Regla::Vacios { si, dxf } => ((celda.valor == Valor::Vacio) == *si, *dxf),
                Regla::Errores { si, dxf } => (matches!(celda.valor, Valor::Error(_)) == *si, *dxf),
                Regla::Escala { puntos } => {
                    if let (Some(v), None) = (num, ef.escala) {
                        let pos: Vec<(f64, Rgb)> = puntos
                            .iter()
                            .map(|(p, c)| (self.valor_de(*p, e), *c))
                            .collect();
                        ef.escala = Some(interpolar(v, &pos));
                    }
                    (false, None)
                }
                Regla::Barra(b) => {
                    if let (Some(v), None) = (num, ef.barra) {
                        ef.barra = Some(self.barra(v, e, b));
                        ef.ocultar_valor |= !b.ver_valor;
                    }
                    (false, None)
                }
                Regla::Iconos {
                    conjunto,
                    cortes,
                    invertir,
                    ver_valor,
                    a_mano,
                    ..
                } => {
                    if let (Some(v), None) = (num, ef.icono) {
                        let k = conjunto.cantidad();
                        // El icono mas alto cuyo limite se cumple.
                        let mut idx = 0usize;
                        for (i, (p, gte)) in cortes.iter().enumerate().rev() {
                            let c = self.valor_de(*p, e);
                            if if *gte { v >= c } else { v > c } {
                                idx = i + 1;
                                break;
                            }
                        }
                        let idx = idx.min(k - 1);
                        let idx = if *invertir { k - 1 - idx } else { idx };
                        ef.icono = Some(
                            a_mano
                                .get(idx)
                                .copied()
                                .flatten()
                                .unwrap_or((*conjunto, idx as u8)),
                        );
                        ef.ocultar_valor |= !ver_valor;
                    }
                    (false, None)
                }
            };
            if cumple {
                if let Some(d) = dxf.and_then(|i| dxfs.get(i as usize)) {
                    // Gana la de mas prioridad: solo se rellena lo que falte.
                    let mut nuevo = d.clone();
                    nuevo.sobre(&ef.dxf);
                    ef.dxf = nuevo;
                }
                if *parar {
                    return true;
                }
            }
        }
        false
    }
}

fn interpolar(v: f64, pos: &[(f64, Rgb)]) -> Rgb {
    if v <= pos[0].0 {
        return pos[0].1;
    }
    for w in pos.windows(2) {
        let ((a, ca), (b, cb)) = (w[0], w[1]);
        if v <= b {
            let t = if b > a {
                ((v - a) / (b - a)) as f32
            } else {
                1.0
            };
            return cb.mezcla(ca, t);
        }
    }
    pos[pos.len() - 1].1
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::xml;

    fn celda(v: f64) -> Celda {
        Celda {
            col: 0,
            xf: 0,
            valor: Valor::Numero(v),
        }
    }

    fn est(nums: &[f64]) -> Estadistica {
        let mut o = nums.to_vec();
        o.sort_by(|a, b| a.total_cmp(b));
        let n = o.len() as f64;
        let media = o.iter().sum::<f64>() / n;
        let desv = (o.iter().map(|x| (x - media).powi(2)).sum::<f64>() / (n - 1.0)).sqrt();
        Estadistica {
            min: o[0],
            max: o[o.len() - 1],
            media,
            desv,
            ordenados: o,
            ..Default::default()
        }
    }

    fn una(xml_regla: &str) -> Condicional {
        let x = format!(
            "<x><conditionalFormatting sqref=\"A1:A10\">{xml_regla}</conditionalFormatting></x>"
        );
        let mut cs = leer(
            &xml::arbol(x.as_bytes()),
            &Tema::default(),
            &crate::estilos::PALETA,
        );
        assert_eq!(cs.len(), 1, "{xml_regla}");
        cs.remove(0)
    }

    fn efecto(c: &Condicional, v: f64) -> Efecto {
        let hoja = Hoja::vacia_para_pruebas();
        let dxfs = vec![Dxf {
            negrita: Some(true),
            ..Default::default()
        }];
        let mut ef = Efecto::default();
        c.evaluar(&hoja, &celda(v), "", &dxfs, &mut ef);
        ef
    }

    #[test]
    fn reglas_de_comparar_escalas_y_barras() {
        let x = br#"<x><conditionalFormatting sqref="A1:A10"><cfRule type="cellIs" dxfId="0" priority="1" operator="greaterThan"><formula>5</formula></cfRule><cfRule type="colorScale" priority="2"><colorScale><cfvo type="min"/><cfvo type="max"/><color rgb="FFFFFFFF"/><color rgb="FF000000"/></colorScale></cfRule><cfRule type="dataBar" priority="3"><dataBar showValue="0"><cfvo type="min"/><cfvo type="max"/><color rgb="FF638EC6"/></dataBar></cfRule></conditionalFormatting></x>"#;
        let mut cs = leer(&xml::arbol(x), &Tema::default(), &crate::estilos::PALETA);
        assert_eq!(cs.len(), 1);
        let c = &mut cs[0];
        c.estadistica = Some(Estadistica {
            min: 0.0,
            max: 10.0,
            media: 5.0,
            ordenados: (0..=10).map(f64::from).collect(),
            ..Default::default()
        });
        let ef = efecto(c, 10.0);
        assert_eq!(ef.dxf.negrita, Some(true));
        assert_eq!(ef.escala, Some(Rgb::NEGRO));
        assert!(
            ef.barra
                .is_some_and(|b| (b.hasta - 0.9).abs() < 1e-6 && b.desde == 0.0 && b.degradado)
        );
        assert!(ef.ocultar_valor);
        let ef = efecto(c, 0.0);
        assert_eq!(ef.dxf.negrita, None);
        assert_eq!(ef.escala, Some(Rgb::BLANCO));
    }

    #[test]
    fn escala_de_tres_colores_pasa_por_el_medio() {
        let mut c = una(
            r#"<cfRule type="colorScale" priority="1"><colorScale><cfvo type="min"/><cfvo type="percentile" val="50"/><cfvo type="max"/><color rgb="FFFF0000"/><color rgb="FFFFFF00"/><color rgb="FF00FF00"/></colorScale></cfRule>"#,
        );
        c.estadistica = Some(est(&[0.0, 5.0, 10.0]));
        assert_eq!(efecto(&c, 0.0).escala, Some(Rgb { r: 255, g: 0, b: 0 }));
        assert_eq!(
            efecto(&c, 5.0).escala,
            Some(Rgb {
                r: 255,
                g: 255,
                b: 0
            })
        );
        assert_eq!(efecto(&c, 10.0).escala, Some(Rgb { r: 0, g: 255, b: 0 }));
        let m = efecto(&c, 7.5).escala.unwrap();
        assert_eq!((m.r, m.g, m.b), (128, 255, 0));
        // Caso negativo: un texto no tiene color.
        let hoja = Hoja::vacia_para_pruebas();
        let mut ef = Efecto::default();
        c.evaluar(
            &hoja,
            &Celda {
                col: 0,
                xf: 0,
                valor: Valor::Logico(true),
            },
            "x",
            &[],
            &mut ef,
        );
        assert_eq!(ef.escala, None);
        // Una escala con un solo color se descarta.
        let x = br#"<x><conditionalFormatting sqref="A1"><cfRule type="colorScale" priority="1"><colorScale><cfvo type="min"/><color rgb="FFFF0000"/></colorScale></cfRule></conditionalFormatting></x>"#;
        assert!(leer(&xml::arbol(x), &Tema::default(), &crate::estilos::PALETA).is_empty());
    }

    #[test]
    fn iconos_por_porcentaje_con_invertido() {
        let mut c = una(
            r#"<cfRule type="iconSet" priority="1"><iconSet iconSet="3Arrows"><cfvo type="percent" val="0"/><cfvo type="percent" val="33"/><cfvo type="percent" val="67"/></iconSet></cfRule>"#,
        );
        c.estadistica = Some(est(&[0.0, 100.0]));
        assert_eq!(efecto(&c, 10.0).icono, Some((Conjunto::Flechas3, 0)));
        assert_eq!(efecto(&c, 50.0).icono, Some((Conjunto::Flechas3, 1)));
        assert_eq!(efecto(&c, 90.0).icono, Some((Conjunto::Flechas3, 2)));
        assert_eq!(
            efecto(&c, 67.0).icono,
            Some((Conjunto::Flechas3, 2)),
            "el limite cuenta con >="
        );
        let mut c = una(
            r#"<cfRule type="iconSet" priority="1"><iconSet iconSet="4Rating" reverse="1" showValue="0"/></cfRule>"#,
        );
        c.estadistica = Some(est(&[0.0, 100.0]));
        let ef = efecto(&c, 99.0);
        assert_eq!(ef.icono, Some((Conjunto::Valoracion4, 0)));
        assert!(ef.ocultar_valor);
        // Con `gte="0"` el limite exacto no sube de icono.
        let mut c = una(
            r#"<cfRule type="iconSet" priority="1"><iconSet iconSet="3Flags"><cfvo type="num" val="0"/><cfvo type="num" val="5" gte="0"/><cfvo type="num" val="10" gte="0"/></iconSet></cfRule>"#,
        );
        c.estadistica = Some(est(&[0.0, 20.0]));
        assert_eq!(efecto(&c, 5.0).icono, Some((Conjunto::Banderas3, 0)));
        assert_eq!(efecto(&c, 5.5).icono, Some((Conjunto::Banderas3, 1)));
    }

    #[test]
    fn los_veinte_conjuntos_existen_y_se_leen() {
        let mut vistos = std::collections::HashSet::new();
        for c in Conjunto::TODOS {
            assert_eq!(Conjunto::de_nombre(c.nombre()), Some(c));
            assert!(vistos.insert(c.nombre()));
            assert_eq!(c.cortes_defecto().len(), c.cantidad() - 1);
        }
        assert_eq!(vistos.len(), 20);
        assert_eq!(Conjunto::de_nombre("9Cosas"), None);
        // Un conjunto desconocido descarta la regla.
        let x = br#"<x><conditionalFormatting sqref="A1"><cfRule type="iconSet" priority="1"><iconSet iconSet="9Cosas"/></cfRule></conditionalFormatting></x>"#;
        assert!(leer(&xml::arbol(x), &Tema::default(), &crate::estilos::PALETA).is_empty());
    }

    #[test]
    fn iconos_x14_a_mano_y_conjuntos_nuevos() {
        let x = br#"<x xmlns:x14="u" xmlns:xm="m"><extLst><ext><x14:conditionalFormattings><x14:conditionalFormatting><x14:cfRule type="iconSet" priority="1" id="{A}"><x14:iconSet iconSet="3Triangles" custom="1"><x14:cfvo type="percent"><xm:f>0</xm:f></x14:cfvo><x14:cfvo type="num"><xm:f>10</xm:f></x14:cfvo><x14:cfvo type="num"><xm:f>20</xm:f></x14:cfvo><x14:cfIcon iconSet="3Stars" iconId="2"/><x14:cfIcon iconSet="NoHay" iconId="0"/><x14:cfIcon iconSet="3Stars" iconId="9"/></x14:iconSet></x14:cfRule><xm:sqref>B2:B5</xm:sqref></x14:conditionalFormatting></x14:conditionalFormattings></ext></extLst></x>"#;
        let mut cs = leer(&xml::arbol(x), &Tema::default(), &crate::estilos::PALETA);
        assert_eq!(cs.len(), 1);
        assert_eq!(
            cs[0].rangos,
            vec![Rango {
                fila0: 1,
                col0: 1,
                fila1: 4,
                col1: 1
            }]
        );
        cs[0].estadistica = Some(est(&[0.0, 30.0]));
        assert_eq!(
            efecto(&cs[0], 5.0).icono,
            Some((Conjunto::Estrellas3, 2)),
            "icono a mano"
        );
        // El segundo (conjunto inexistente) y el tercero (indice fuera) caen en el de siempre.
        assert_eq!(efecto(&cs[0], 15.0).icono, Some((Conjunto::Triangulos3, 1)));
        assert_eq!(efecto(&cs[0], 25.0).icono, Some((Conjunto::Triangulos3, 2)));
    }

    #[test]
    fn barras_x14_con_negativos_y_eje() {
        let x = br#"<x xmlns:x14="u" xmlns:xm="m"><conditionalFormatting sqref="C1:C9"><cfRule type="dataBar" priority="1"><dataBar><cfvo type="min"/><cfvo type="max"/><color rgb="FF00B050"/></dataBar><extLst><ext uri="{B025F937-C7B1-47D3-B67F-A62EFF666E3E}"><x14:id>{ID1}</x14:id></ext></extLst></cfRule></conditionalFormatting><extLst><ext><x14:conditionalFormattings><x14:conditionalFormatting><x14:cfRule type="dataBar" id="{ID1}"><x14:dataBar minLength="0" maxLength="100" gradient="0" border="1" negativeBarBorderColorSameAsPositive="0"><x14:cfvo type="autoMin"/><x14:cfvo type="autoMax"/><x14:borderColor rgb="FF00B050"/><x14:negativeFillColor rgb="FFFF0000"/><x14:negativeBorderColor rgb="FF800000"/><x14:axisColor rgb="FF000000"/></x14:dataBar></x14:cfRule><xm:sqref>C1:C9</xm:sqref></x14:conditionalFormatting></x14:conditionalFormattings></ext></extLst></x>"#;
        let mut cs = leer(&xml::arbol(x), &Tema::default(), &crate::estilos::PALETA);
        assert_eq!(cs.len(), 1, "la extension se une a su regla, no crea otra");
        cs[0].estadistica = Some(est(&[-10.0, 30.0]));
        let b = efecto(&cs[0], 30.0).barra.unwrap();
        // Eje en 10/40 = 0,25; la positiva ocupa el resto.
        assert!(
            (b.desde - 0.25).abs() < 1e-6 && (b.hasta - 1.0).abs() < 1e-6,
            "{b:?}"
        );
        assert!(!b.degradado);
        assert_eq!(
            b.borde,
            Some(Rgb {
                r: 0,
                g: 0xB0,
                b: 0x50
            })
        );
        assert_eq!(
            b.color,
            Rgb {
                r: 0,
                g: 0xB0,
                b: 0x50
            },
            "el color sale de la regla de siempre"
        );
        assert_eq!(b.eje.map(|e| e.0), Some(0.25));
        let n = efecto(&cs[0], -10.0).barra.unwrap();
        assert!(
            (n.desde - 0.0).abs() < 1e-6 && (n.hasta - 0.25).abs() < 1e-6,
            "{n:?}"
        );
        assert_eq!(n.color, Rgb { r: 255, g: 0, b: 0 });
        assert_eq!(
            n.borde,
            Some(Rgb {
                r: 0x80,
                g: 0,
                b: 0
            })
        );
        // Un valor intermedio positivo.
        let m = efecto(&cs[0], 10.0).barra.unwrap();
        assert!(
            (m.desde - 0.25).abs() < 1e-6 && (m.hasta - 0.5).abs() < 1e-6,
            "{m:?}"
        );
    }

    #[test]
    fn barras_x14_sin_negativos_derecha_a_izquierda_y_sin_eje() {
        let x = br#"<x xmlns:x14="u" xmlns:xm="m"><extLst><ext><x14:conditionalFormattings><x14:conditionalFormatting><x14:cfRule type="dataBar" id="{ZZ}"><x14:dataBar direction="rightToLeft" axisPosition="none"><x14:cfvo type="autoMin"/><x14:cfvo type="autoMax"/></x14:dataBar></x14:cfRule><xm:sqref>A1:A3</xm:sqref></x14:conditionalFormatting></x14:conditionalFormattings></ext></extLst></x>"#;
        let mut cs = leer(&xml::arbol(x), &Tema::default(), &crate::estilos::PALETA);
        assert_eq!(
            cs.len(),
            1,
            "una extension sin regla de siempre se queda sola"
        );
        cs[0].estadistica = Some(est(&[10.0, 50.0]));
        let b = efecto(&cs[0], 50.0).barra.unwrap();
        // Minimo automatico = 0: 50 de 50 es el 90 %, pegado a la derecha.
        assert!(
            (b.desde - 0.1).abs() < 1e-6 && (b.hasta - 1.0).abs() < 1e-6,
            "{b:?}"
        );
        assert_eq!(b.eje, None);
        // Caso negativo: una extension de otro tipo no crea reglas.
        let x = br#"<x xmlns:x14="u" xmlns:xm="m"><extLst><ext><x14:conditionalFormattings><x14:conditionalFormatting><x14:cfRule type="expression" id="{Q}"/><xm:sqref>A1</xm:sqref></x14:conditionalFormatting></x14:conditionalFormattings></ext></extLst></x>"#;
        assert!(leer(&xml::arbol(x), &Tema::default(), &crate::estilos::PALETA).is_empty());
    }

    #[test]
    fn primeros_y_ultimos_n_y_por_ciento() {
        let mut c = una(r#"<cfRule type="top10" dxfId="0" priority="1" rank="2"/>"#);
        c.estadistica = Some(est(&[1.0, 2.0, 3.0, 4.0, 5.0]));
        assert_eq!(efecto(&c, 5.0).dxf.negrita, Some(true));
        assert_eq!(efecto(&c, 4.0).dxf.negrita, Some(true));
        assert_eq!(efecto(&c, 3.0).dxf.negrita, None);
        let mut c = una(
            r#"<cfRule type="top10" dxfId="0" priority="1" rank="40" percent="1" bottom="1"/>"#,
        );
        c.estadistica = Some(est(&[1.0, 2.0, 3.0, 4.0, 5.0]));
        assert_eq!(
            efecto(&c, 2.0).dxf.negrita,
            Some(true),
            "el 40 % de 5 son 2 celdas"
        );
        assert_eq!(efecto(&c, 3.0).dxf.negrita, None);
    }

    #[test]
    fn encima_y_debajo_del_promedio_con_desviaciones() {
        let mut c = una(r#"<cfRule type="aboveAverage" dxfId="0" priority="1"/>"#);
        c.estadistica = Some(est(&[1.0, 2.0, 3.0, 4.0, 5.0]));
        assert_eq!(efecto(&c, 3.5).dxf.negrita, Some(true));
        assert_eq!(
            efecto(&c, 3.0).dxf.negrita,
            None,
            "igual a la media no cuenta"
        );
        let mut c = una(
            r#"<cfRule type="aboveAverage" dxfId="0" priority="1" aboveAverage="0" equalAverage="1"/>"#,
        );
        c.estadistica = Some(est(&[1.0, 2.0, 3.0, 4.0, 5.0]));
        assert_eq!(efecto(&c, 3.0).dxf.negrita, Some(true));
        assert_eq!(efecto(&c, 3.1).dxf.negrita, None);
        // Desviacion tipica de la muestra de 1..5 = 1,5811.
        let mut c = una(r#"<cfRule type="aboveAverage" dxfId="0" priority="1" stdDev="1"/>"#);
        c.estadistica = Some(est(&[1.0, 2.0, 3.0, 4.0, 5.0]));
        assert_eq!(efecto(&c, 4.6).dxf.negrita, Some(true));
        assert_eq!(efecto(&c, 4.5).dxf.negrita, None);
    }

    #[test]
    fn periodos_de_fechas() {
        // 2023-03-15 era miercoles (serie 45000).
        let hoy = 45000.0;
        assert!(Periodo::Hoy.cumple(45000.7, hoy));
        assert!(!Periodo::Hoy.cumple(45001.0, hoy));
        assert!(Periodo::Ayer.cumple(44999.0, hoy));
        assert!(Periodo::Manana.cumple(45001.0, hoy));
        assert!(Periodo::Ultimos7.cumple(44994.0, hoy) && !Periodo::Ultimos7.cumple(44993.0, hoy));
        // La semana va de domingo (12) a sabado (18).
        assert!(
            Periodo::EstaSemana.cumple(44997.0, hoy) && Periodo::EstaSemana.cumple(45003.0, hoy)
        );
        assert!(
            !Periodo::EstaSemana.cumple(44996.0, hoy) && !Periodo::EstaSemana.cumple(45004.0, hoy)
        );
        assert!(
            Periodo::SemanaPasada.cumple(44996.0, hoy)
                && Periodo::SemanaPasada.cumple(44990.0, hoy)
        );
        assert!(!Periodo::SemanaPasada.cumple(44989.0, hoy));
        assert!(
            Periodo::SemanaSiguiente.cumple(45004.0, hoy)
                && !Periodo::SemanaSiguiente.cumple(45011.0, hoy)
        );
        // Marzo 2023: del 44986 (1-mar) al 45016 (31-mar).
        assert!(Periodo::EsteMes.cumple(44986.0, hoy) && Periodo::EsteMes.cumple(45016.0, hoy));
        assert!(
            Periodo::MesPasado.cumple(44985.0, hoy) && !Periodo::MesPasado.cumple(44986.0, hoy)
        );
        assert!(Periodo::MesSiguiente.cumple(45017.0, hoy));
        // Un numero que no es fecha (0 o negativo) no es de «este mes».
        assert!(
            !Periodo::EsteMes.cumple(0.0, 45000.0) && !Periodo::MesPasado.cumple(-5.0, 45000.0)
        );
        assert_eq!(Periodo::de("quizas"), None);
        // Y la regla se lee y se evalua con el hoy guardado en las cuentas.
        let mut c = una(r#"<cfRule type="timePeriod" dxfId="0" priority="1" timePeriod="today"/>"#);
        c.estadistica = Some(Estadistica {
            hoy: 45000.0,
            ..Default::default()
        });
        assert_eq!(efecto(&c, 45000.0).dxf.negrita, Some(true));
        assert_eq!(efecto(&c, 44999.0).dxf.negrita, None);
        let x = br#"<x><conditionalFormatting sqref="A1"><cfRule type="timePeriod" dxfId="0" priority="1" timePeriod="lastDecade"/></conditionalFormatting></x>"#;
        assert!(leer(&xml::arbol(x), &Tema::default(), &crate::estilos::PALETA).is_empty());
    }

    #[test]
    fn duplicados_y_unicos_se_cuentan_una_vez() {
        let mut c = una(r#"<cfRule type="duplicateValues" dxfId="0" priority="1"/>"#);
        assert!(!c.necesita_texto);
        let mut hoja = Hoja::vacia_para_pruebas();
        hoja.poner_numeros_para_pruebas(&[1.0, 2.0, 2.0, 3.0, 3.0, 3.0]);
        c.calcular(&hoja);
        let e = c.estadistica.clone().unwrap();
        assert_eq!(e.cuentas.len(), 3);
        let eval = |c: &Condicional, v: f64| {
            let mut ef = Efecto::default();
            c.evaluar(
                &hoja,
                &celda(v),
                "",
                &[Dxf {
                    negrita: Some(true),
                    ..Default::default()
                }],
                &mut ef,
            );
            ef.dxf.negrita == Some(true)
        };
        assert!(!eval(&c, 1.0));
        assert!(eval(&c, 2.0));
        assert!(eval(&c, 3.0));
        let mut u = una(r#"<cfRule type="uniqueValues" dxfId="0" priority="1"/>"#);
        u.calcular(&hoja);
        assert!(eval(&u, 1.0));
        assert!(!eval(&u, 2.0));
        // Un valor que no esta en el rango no es repetido ni unico.
        assert!(!eval(&c, 99.0) && !eval(&u, 99.0));
    }

    #[test]
    fn solo_se_hacen_las_cuentas_que_se_piden() {
        let hoja = Hoja::vacia_para_pruebas();
        let mut c = una(
            r#"<cfRule type="cellIs" dxfId="0" priority="1" operator="equal"><formula>1</formula></cfRule>"#,
        );
        c.calcular(&hoja);
        let e = c.estadistica.unwrap();
        assert!(e.ordenados.is_empty() && e.cuentas.is_empty() && e.hoy == 0.0);
        let mut c = una(r#"<cfRule type="timePeriod" dxfId="0" priority="1" timePeriod="today"/>"#);
        c.calcular(&hoja);
        assert!(c.estadistica.unwrap().hoy > 40_000.0);
    }

    #[test]
    fn vacios_y_errores_y_texto() {
        let mut c = una(r#"<cfRule type="containsErrors" dxfId="0" priority="1"/>"#);
        c.calcular(&Hoja::vacia_para_pruebas());
        let hoja = Hoja::vacia_para_pruebas();
        let mut ef = Efecto::default();
        c.evaluar(
            &hoja,
            &Celda {
                col: 0,
                xf: 0,
                valor: Valor::Error(2),
            },
            "",
            &[Dxf {
                negrita: Some(true),
                ..Default::default()
            }],
            &mut ef,
        );
        assert_eq!(ef.dxf.negrita, Some(true));
        let mut ef = Efecto::default();
        c.evaluar(
            &hoja,
            &celda(1.0),
            "",
            &[Dxf {
                negrita: Some(true),
                ..Default::default()
            }],
            &mut ef,
        );
        assert_eq!(ef.dxf.negrita, None);
        let mut c = una(
            r#"<cfRule type="containsText" dxfId="0" priority="1" operator="containsText" text="Ab"/>"#,
        );
        assert!(c.necesita_texto);
        c.calcular(&hoja);
        let mut ef = Efecto::default();
        c.evaluar(
            &hoja,
            &Celda {
                col: 0,
                xf: 0,
                valor: Valor::Compartido(0),
            },
            "xxABxx",
            &[Dxf {
                negrita: Some(true),
                ..Default::default()
            }],
            &mut ef,
        );
        assert_eq!(ef.dxf.negrita, Some(true));
    }

    #[test]
    fn una_formula_que_no_es_literal_descarta_la_regla() {
        let x = br#"<x><conditionalFormatting sqref="A1"><cfRule type="cellIs" dxfId="0" priority="1" operator="greaterThan"><formula>$B$1</formula></cfRule><cfRule type="expression" dxfId="0" priority="2"><formula>A1&gt;3</formula></cfRule></conditionalFormatting></x>"#;
        assert!(leer(&xml::arbol(x), &Tema::default(), &crate::estilos::PALETA).is_empty());
    }
}
