//! **Las propiedades de Word y su herencia.** Un parrafo de Word no dice
//! todo lo suyo: lo que no dice lo hereda de su estilo, que lo hereda del
//! estilo en que se basa, y asi hasta los valores por defecto del documento
//! (`w:docDefaults`). Aqui se leen `w:pPr`, `w:rPr`, `w:tblPr`, `w:tcPr` con
//! todo opcional, y se mezclan en el orden de ECMA-376 (17.7.2):
//! valores del documento → estilo de tabla → numeracion → estilo de
//! parrafo → estilo de caracter → formato directo.

use std::cell::RefCell;
use std::collections::HashMap;

use pixpin_docs::xml::Nodo;

use crate::pagina::{Color, Discontinuo, Subrayado, Trazo};
use crate::paquete::{attr_num, interruptor, twips};
use crate::renglones::{Alineacion, TipoTope};
use crate::tema::{self, Tema};

/// Un color de Word: «auto» (negro sobre claro) o uno concreto.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorW {
    Auto,
    Fijo(Color),
}

/// Una letra pedida por nombre o por el tema.
#[derive(Debug, Clone, PartialEq)]
pub enum Familia {
    Nombre(String),
    /// `true`: la de titulos (`major`); `false`: la de texto (`minor`).
    Tema(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vertical {
    Normal,
    Super,
    Sub,
}

/// Propiedades de corrida (`w:rPr`), todas opcionales.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PropCorrida {
    pub estilo: Option<String>,
    pub familia: Option<Familia>,
    pub familia_ea: Option<Familia>,
    /// En puntos.
    pub tam: Option<f32>,
    pub negrita: Option<bool>,
    pub cursiva: Option<bool>,
    pub subrayado: Option<Option<Subrayado>>,
    pub tachado: Option<bool>,
    pub color: Option<ColorW>,
    pub resaltado: Option<Option<Color>>,
    pub sombreado: Option<Option<Color>>,
    pub mayusculas: Option<bool>,
    pub versalitas: Option<bool>,
    pub oculto: Option<bool>,
    pub vertical: Option<Vertical>,
    /// Interletrado en puntos.
    pub espaciado: Option<f32>,
    /// Subida de la linea base en puntos (`w:position`).
    pub posicion: Option<f32>,
}

macro_rules! mezclar_campos {
    ($a:ident, $b:ident, $($c:ident),*) => {
        $( if $b.$c.is_some() { $a.$c = $b.$c.clone(); } )*
    };
}

impl PropCorrida {
    /// `otro` manda sobre `self` en lo que diga.
    pub fn mezclar(&mut self, otro: &PropCorrida) {
        mezclar_campos!(
            self, otro, estilo, familia, familia_ea, tam, negrita, cursiva, subrayado, tachado,
            color, resaltado, sombreado, mayusculas, versalitas, oculto, vertical, espaciado,
            posicion
        );
    }

    pub fn leer(n: &Nodo, tema: &Tema) -> PropCorrida {
        let mut p = PropCorrida::default();
        for e in n.elementos() {
            match e.nombre.as_str() {
                "rStyle" => p.estilo = Some(e.valor().to_string()),
                "rFonts" => {
                    let t = e.atributo("asciiTheme");
                    if !t.is_empty() {
                        p.familia = Some(Familia::Tema(t.starts_with("major")));
                    } else {
                        let a = e.atributo("ascii");
                        let a = if a.is_empty() { e.atributo("hAnsi") } else { a };
                        if !a.is_empty() {
                            p.familia = Some(Familia::Nombre(a.to_string()));
                        }
                    }
                    let t = e.atributo("eastAsiaTheme");
                    if !t.is_empty() {
                        p.familia_ea = Some(Familia::Tema(t.starts_with("major")));
                    } else if !e.atributo("eastAsia").is_empty() {
                        p.familia_ea = Some(Familia::Nombre(e.atributo("eastAsia").to_string()));
                    }
                }
                "sz" => p.tam = attr_num(e, "val", 2.0).map(|v| v / 2.0),
                "b" => p.negrita = Some(interruptor(e)),
                "i" => p.cursiva = Some(interruptor(e)),
                "strike" | "dstrike" => p.tachado = Some(interruptor(e)),
                "caps" => p.mayusculas = Some(interruptor(e)),
                "smallCaps" => p.versalitas = Some(interruptor(e)),
                "vanish" | "specVanish" => p.oculto = Some(interruptor(e)),
                "webHidden" => {}
                "u" => {
                    p.subrayado = Some(match e.valor() {
                        "none" | "0" | "false" => None,
                        "double" => Some(Subrayado::Doble),
                        "thick" => Some(Subrayado::Grueso),
                        "dotted" | "dottedHeavy" => Some(Subrayado::Puntos),
                        "dash" | "dashedHeavy" | "dashLong" | "dashLongHeavy" | "dotDash"
                        | "dotDotDash" => Some(Subrayado::Rayas),
                        "wave" | "wavyHeavy" | "wavyDouble" => Some(Subrayado::Ondulado),
                        _ => Some(Subrayado::Simple),
                    })
                }
                "color" => p.color = color_w(e, tema),
                "highlight" => p.resaltado = Some(tema::color_con_nombre(e.valor())),
                "shd" => p.sombreado = Some(sombreado(e, tema)),
                "vertAlign" => {
                    p.vertical = Some(match e.valor() {
                        "superscript" => Vertical::Super,
                        "subscript" => Vertical::Sub,
                        _ => Vertical::Normal,
                    })
                }
                "spacing" => p.espaciado = attr_num(e, "val", 20.0).map(twips),
                "position" => p.posicion = attr_num(e, "val", 2.0).map(|v| v / 2.0),
                // El relleno de texto de Word 2010 (`w14:textFill`): el color
                // liso o la primera parada del degradado.
                "textFill" => {
                    let fuente = e.hijo("solidFill").or_else(|| {
                        e.hijo("gradFill")
                            .and_then(|g| g.hijo("gsLst"))
                            .and_then(|l| l.hijo("gs"))
                    });
                    if let Some(c) =
                        fuente.and_then(|f| tema::color_hijo(f, tema, &HashMap::new(), None))
                    {
                        p.color = Some(ColorW::Fijo(c));
                    }
                }
                _ => {}
            }
        }
        p
    }
}

/// `w:color` con sus tintes del tema.
pub fn color_w(e: &Nodo, tema: &Tema) -> Option<ColorW> {
    let tc = e.atributo("themeColor");
    if !tc.is_empty()
        && let Some(c) = tema.esquema(tc, &HashMap::new())
    {
        let hexf = |s: &str| u8::from_str_radix(s, 16).ok().map(|v| v as f32 / 255.0);
        return Some(ColorW::Fijo(tema::tinte_word(
            c,
            hexf(e.atributo("themeTint")),
            hexf(e.atributo("themeShade")),
        )));
    }
    match e.valor() {
        "auto" | "" => Some(ColorW::Auto),
        v => Color::hex(v).map(ColorW::Fijo),
    }
}

/// El color de fondo de un `w:shd` (el `fill`; los tramados se aproximan).
pub fn sombreado(e: &Nodo, tema: &Tema) -> Option<Color> {
    let tf = e.atributo("themeFill");
    let base = if !tf.is_empty() {
        tema.esquema(tf, &HashMap::new()).map(|c| {
            let hexf = |s: &str| u8::from_str_radix(s, 16).ok().map(|v| v as f32 / 255.0);
            tema::tinte_word(
                c,
                hexf(e.atributo("themeFillTint")),
                hexf(e.atributo("themeFillShade")),
            )
        })
    } else {
        match e.atributo("fill") {
            "auto" | "" => None,
            v => Color::hex(v),
        }
    };
    // Un tramado de porcentaje (`pct25`) con color de trama: se mezcla.
    let val = e.valor();
    if let Some(pct) = val.strip_prefix("pct").and_then(|v| v.parse::<f32>().ok()) {
        let trama = match e.atributo("color") {
            "auto" | "" => Color::NEGRO,
            v => Color::hex(v).unwrap_or(Color::NEGRO),
        };
        let fondo = base.unwrap_or(Color::BLANCO);
        let t = pct / 100.0;
        let m = |a: u8, b: u8| (a as f32 * (1.0 - t) + b as f32 * t).round() as u8;
        return Some(Color::rgb(
            m(fondo.r, trama.r),
            m(fondo.g, trama.g),
            m(fondo.b, trama.b),
        ));
    }
    if val == "solid" {
        return match e.atributo("color") {
            "auto" | "" => Some(Color::NEGRO),
            v => Color::hex(v),
        };
    }
    base
}

/// Un borde de Word (`w:top`, `w:bottom`...). `None` si es `nil`/`none`.
pub fn borde(e: &Nodo, tema: &Tema) -> Option<Trazo> {
    let v = e.valor();
    if matches!(v, "nil" | "none" | "") {
        return None;
    }
    // `sz` va en octavos de punto.
    let grosor = attr_num(e, "sz", 8.0)
        .map(|s| s / 8.0)
        .unwrap_or(0.5)
        .max(0.25);
    // En un borde, `w:val` es el tipo de linea y el color va en `w:color`.
    let tc = e.atributo("themeColor");
    let color = if !tc.is_empty()
        && let Some(c) = tema.esquema(tc, &HashMap::new())
    {
        let hexf = |s: &str| u8::from_str_radix(s, 16).ok().map(|v| v as f32 / 255.0);
        tema::tinte_word(
            c,
            hexf(e.atributo("themeTint")),
            hexf(e.atributo("themeShade")),
        )
    } else {
        Color::hex(e.atributo("color")).unwrap_or(Color::NEGRO)
    };
    let discontinuo = match v {
        "dotted" => Discontinuo::Puntos,
        "dashed" | "dashSmallGap" => Discontinuo::Rayas,
        "dotDash" | "dotDotDash" => Discontinuo::RayaPunto,
        _ => Discontinuo::Solida,
    };
    let grosor = match v {
        "double" => grosor * 3.0,
        "thick" => grosor.max(1.0),
        _ => grosor,
    };
    Some(Trazo {
        color,
        grosor,
        discontinuo,
    })
}

/// Un tope de tabulador de un `w:tabs`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TopeW {
    /// En puntos desde el margen.
    pub pos: f32,
    pub tipo: Option<TipoTope>,
    pub relleno: Option<char>,
    /// `w:val="clear"`: quita un tope heredado.
    pub quitar: bool,
}

/// Interlineado de un parrafo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Interlineado {
    /// Multiplo del renglon sencillo.
    Auto(f32),
    Exacto(f32),
    AlMenos(f32),
}

/// Bordes de parrafo o de celda.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Bordes {
    pub arriba: Option<Option<Trazo>>,
    pub abajo: Option<Option<Trazo>>,
    pub izq: Option<Option<Trazo>>,
    pub der: Option<Option<Trazo>>,
    pub entre: Option<Option<Trazo>>,
    /// Los interiores de una tabla.
    pub dentro_h: Option<Option<Trazo>>,
    pub dentro_v: Option<Option<Trazo>>,
}

impl Bordes {
    pub fn leer(n: &Nodo, tema: &Tema) -> Bordes {
        let mut b = Bordes::default();
        for e in n.elementos() {
            let t = Some(borde(e, tema));
            match e.nombre.as_str() {
                "top" => b.arriba = t,
                "bottom" => b.abajo = t,
                "left" | "start" => b.izq = t,
                "right" | "end" => b.der = t,
                "between" => b.entre = t,
                "insideH" => b.dentro_h = t,
                "insideV" => b.dentro_v = t,
                _ => {}
            }
        }
        b
    }

    pub fn mezclar(&mut self, o: &Bordes) {
        mezclar_campos!(self, o, arriba, abajo, izq, der, entre, dentro_h, dentro_v);
    }
}

/// Propiedades de parrafo (`w:pPr`), todas opcionales.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PropParrafo {
    pub estilo: Option<String>,
    pub alineacion: Option<Alineacion>,
    pub izq: Option<f32>,
    pub der: Option<f32>,
    /// Primer renglon: positivo = sangria, negativo = francesa.
    pub primera: Option<f32>,
    pub antes: Option<f32>,
    pub despues: Option<f32>,
    pub antes_auto: Option<bool>,
    pub despues_auto: Option<bool>,
    pub interlineado: Option<Interlineado>,
    pub topes: Vec<TopeW>,
    pub con_siguiente: Option<bool>,
    pub lineas_juntas: Option<bool>,
    pub salto_antes: Option<bool>,
    pub viudas: Option<bool>,
    pub contextual: Option<bool>,
    /// `(numId, ilvl)`; numId 0 quita la numeracion heredada.
    pub num_id: Option<u32>,
    pub nivel: Option<u32>,
    pub sombreado: Option<Option<Color>>,
    pub bordes: Bordes,
    /// La marca de parrafo (`w:pPr/w:rPr`).
    pub marca: Option<PropCorrida>,
}

impl PropParrafo {
    pub fn mezclar(&mut self, o: &PropParrafo) {
        mezclar_campos!(
            self,
            o,
            estilo,
            alineacion,
            izq,
            der,
            primera,
            antes,
            despues,
            antes_auto,
            despues_auto,
            interlineado,
            con_siguiente,
            lineas_juntas,
            salto_antes,
            viudas,
            contextual,
            num_id,
            nivel,
            sombreado
        );
        self.bordes.mezclar(&o.bordes);
        for t in &o.topes {
            // Un tope en la misma posicion sustituye al heredado.
            self.topes.retain(|x| (x.pos - t.pos).abs() > 0.05);
            self.topes.push(*t);
        }
        if let Some(m) = &o.marca {
            match &mut self.marca {
                Some(s) => s.mezclar(m),
                None => self.marca = Some(m.clone()),
            }
        }
    }

    pub fn leer(n: &Nodo, tema: &Tema) -> PropParrafo {
        let mut p = PropParrafo::default();
        for e in n.elementos() {
            match e.nombre.as_str() {
                "pStyle" => p.estilo = Some(e.valor().to_string()),
                "jc" => {
                    p.alineacion = Some(match e.valor() {
                        "center" => Alineacion::Centro,
                        "right" | "end" => Alineacion::Derecha,
                        "both" | "lowKashida" | "mediumKashida" | "highKashida"
                        | "thaiDistribute" => Alineacion::Justificada,
                        "distribute" => Alineacion::Distribuida,
                        _ => Alineacion::Izquierda,
                    })
                }
                "ind" => {
                    let leer = |k: &str| attr_num(e, k, 20.0).map(twips);
                    if let Some(v) = leer("left").or_else(|| leer("start")) {
                        p.izq = Some(v);
                    }
                    if let Some(v) = leer("right").or_else(|| leer("end")) {
                        p.der = Some(v);
                    }
                    if let Some(v) = leer("hanging") {
                        p.primera = Some(-v);
                    } else if let Some(v) = leer("firstLine") {
                        p.primera = Some(v);
                    }
                }
                "spacing" => {
                    if let Some(v) = attr_num(e, "before", 20.0) {
                        p.antes = Some(twips(v));
                    }
                    if let Some(v) = attr_num(e, "after", 20.0) {
                        p.despues = Some(twips(v));
                    }
                    if !e.atributo("beforeAutospacing").is_empty() {
                        p.antes_auto =
                            Some(!matches!(e.atributo("beforeAutospacing"), "0" | "false"));
                    }
                    if !e.atributo("afterAutospacing").is_empty() {
                        p.despues_auto =
                            Some(!matches!(e.atributo("afterAutospacing"), "0" | "false"));
                    }
                    if let Some(v) = attr_num(e, "line", 20.0) {
                        p.interlineado = Some(match e.atributo("lineRule") {
                            "exact" => Interlineado::Exacto(twips(v.abs())),
                            "atLeast" => Interlineado::AlMenos(twips(v)),
                            _ => Interlineado::Auto(v / 240.0),
                        });
                    }
                }
                "tabs" => {
                    for t in e.elementos().filter(|t| t.nombre == "tab") {
                        let pos = attr_num(t, "pos", 20.0).map(twips).unwrap_or(0.0);
                        let v = t.valor();
                        let tipo = match v {
                            "center" => Some(TipoTope::Centro),
                            "right" | "end" => Some(TipoTope::Derecha),
                            "decimal" => Some(TipoTope::Decimal),
                            "bar" => None,
                            "clear" => None,
                            _ => Some(TipoTope::Izquierda),
                        };
                        let relleno = match t.atributo("leader") {
                            "dot" => Some('.'),
                            "hyphen" => Some('-'),
                            "underscore" => Some('_'),
                            "middleDot" => Some('·'),
                            "heavy" => Some('_'),
                            _ => None,
                        };
                        p.topes.push(TopeW {
                            pos,
                            tipo,
                            relleno,
                            quitar: v == "clear",
                        });
                    }
                }
                "keepNext" => p.con_siguiente = Some(interruptor(e)),
                "keepLines" => p.lineas_juntas = Some(interruptor(e)),
                "pageBreakBefore" => p.salto_antes = Some(interruptor(e)),
                "widowControl" => p.viudas = Some(interruptor(e)),
                "contextualSpacing" => p.contextual = Some(interruptor(e)),
                "numPr" => {
                    if let Some(v) = e.hijo("numId").and_then(|x| x.valor().parse().ok()) {
                        p.num_id = Some(v);
                    }
                    if let Some(v) = e.hijo("ilvl").and_then(|x| x.valor().parse().ok()) {
                        p.nivel = Some(v);
                    }
                }
                "shd" => p.sombreado = Some(sombreado(e, tema)),
                "pBdr" => p.bordes = Bordes::leer(e, tema),
                "rPr" => p.marca = Some(PropCorrida::leer(e, tema)),
                _ => {}
            }
        }
        p
    }
}

/// Margenes de celda, en puntos (`None` = heredado).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Margenes {
    pub arriba: Option<f32>,
    pub abajo: Option<f32>,
    pub izq: Option<f32>,
    pub der: Option<f32>,
}

impl Margenes {
    pub fn leer(n: &Nodo) -> Margenes {
        let mut m = Margenes::default();
        for e in n.elementos() {
            let v = ancho_tabla(e).and_then(|a| {
                if let AnchoW::Puntos(p) = a {
                    Some(p)
                } else {
                    None
                }
            });
            match e.nombre.as_str() {
                "top" => m.arriba = v,
                "bottom" => m.abajo = v,
                "left" | "start" => m.izq = v,
                "right" | "end" => m.der = v,
                _ => {}
            }
        }
        m
    }
    pub fn mezclar(&mut self, o: &Margenes) {
        mezclar_campos!(self, o, arriba, abajo, izq, der);
    }
}

/// Un ancho de tabla (`w:tblW`, `w:tcW`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnchoW {
    Puntos(f32),
    /// Fraccion (0..1) del ancho disponible.
    Porcentaje(f32),
    Auto,
}

pub fn ancho_tabla(e: &Nodo) -> Option<AnchoW> {
    let w = e.atributo("w");
    match e.atributo("type") {
        "pct" => {
            let v = if let Some(p) = w.strip_suffix('%') {
                p.trim().parse::<f32>().ok()? / 100.0
            } else {
                w.parse::<f32>().ok()? / 5000.0
            };
            Some(AnchoW::Porcentaje(v))
        }
        "auto" => Some(AnchoW::Auto),
        "nil" => Some(AnchoW::Puntos(0.0)),
        _ => crate::paquete::numero(w, 20.0).map(|v| AnchoW::Puntos(twips(v))),
    }
}

/// Propiedades de tabla (`w:tblPr`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PropTabla {
    pub estilo: Option<String>,
    pub ancho: Option<AnchoW>,
    pub alineacion: Option<Alineacion>,
    pub sangria: Option<f32>,
    pub bordes: Bordes,
    pub margenes: Margenes,
    pub sombreado: Option<Option<Color>>,
    /// `w:tblLook`: que partes condicionales del estilo se aplican.
    pub look_primera_fila: Option<bool>,
    pub look_ultima_fila: Option<bool>,
    pub look_primera_col: Option<bool>,
    pub look_ultima_col: Option<bool>,
    pub look_sin_bandas_h: Option<bool>,
    pub look_sin_bandas_v: Option<bool>,
    pub espacio_celdas: Option<f32>,
}

impl PropTabla {
    pub fn leer(n: &Nodo, tema: &Tema) -> PropTabla {
        let mut p = PropTabla::default();
        for e in n.elementos() {
            match e.nombre.as_str() {
                "tblStyle" => p.estilo = Some(e.valor().to_string()),
                "tblW" => p.ancho = ancho_tabla(e),
                "jc" => {
                    p.alineacion = Some(match e.valor() {
                        "center" => Alineacion::Centro,
                        "right" | "end" => Alineacion::Derecha,
                        _ => Alineacion::Izquierda,
                    })
                }
                "tblInd" => {
                    p.sangria = ancho_tabla(e).and_then(|a| {
                        if let AnchoW::Puntos(v) = a {
                            Some(v)
                        } else {
                            None
                        }
                    })
                }
                "tblBorders" => p.bordes = Bordes::leer(e, tema),
                "tblCellMar" => p.margenes = Margenes::leer(e),
                "shd" => p.sombreado = Some(sombreado(e, tema)),
                "tblCellSpacing" => {
                    p.espacio_celdas = ancho_tabla(e).and_then(|a| {
                        if let AnchoW::Puntos(v) = a {
                            Some(v)
                        } else {
                            None
                        }
                    })
                }
                "tblLook" => {
                    let bit = |k: &str, mascara: u32| -> Option<bool> {
                        let a = e.atributo(k);
                        if !a.is_empty() {
                            return Some(!matches!(a, "0" | "false"));
                        }
                        let v = u32::from_str_radix(e.valor(), 16).ok()?;
                        Some(v & mascara != 0)
                    };
                    p.look_primera_fila = bit("firstRow", 0x0020);
                    p.look_ultima_fila = bit("lastRow", 0x0040);
                    p.look_primera_col = bit("firstColumn", 0x0080);
                    p.look_ultima_col = bit("lastColumn", 0x0100);
                    p.look_sin_bandas_h = bit("noHBand", 0x0200);
                    p.look_sin_bandas_v = bit("noVBand", 0x0400);
                }
                _ => {}
            }
        }
        p
    }

    pub fn mezclar(&mut self, o: &PropTabla) {
        mezclar_campos!(
            self,
            o,
            estilo,
            ancho,
            alineacion,
            sangria,
            sombreado,
            look_primera_fila,
            look_ultima_fila,
            look_primera_col,
            look_ultima_col,
            look_sin_bandas_h,
            look_sin_bandas_v,
            espacio_celdas
        );
        self.bordes.mezclar(&o.bordes);
        self.margenes.mezclar(&o.margenes);
    }
}

/// Propiedades de celda (`w:tcPr`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PropCelda {
    pub ancho: Option<AnchoW>,
    pub tramo: Option<u32>,
    /// `Some(true)` = empieza una fusion vertical; `Some(false)` = sigue.
    pub fusion_v: Option<bool>,
    pub bordes: Bordes,
    pub sombreado: Option<Option<Color>>,
    pub margenes: Margenes,
    /// `top`, `center`, `bottom`.
    pub vertical: Option<u8>,
}

impl PropCelda {
    pub fn leer(n: &Nodo, tema: &Tema) -> PropCelda {
        let mut p = PropCelda::default();
        for e in n.elementos() {
            match e.nombre.as_str() {
                "tcW" => p.ancho = ancho_tabla(e),
                "gridSpan" => p.tramo = e.valor().parse().ok(),
                "vMerge" => p.fusion_v = Some(e.valor() == "restart"),
                "tcBorders" => p.bordes = Bordes::leer(e, tema),
                "shd" => p.sombreado = Some(sombreado(e, tema)),
                "tcMar" => p.margenes = Margenes::leer(e),
                "vAlign" => {
                    p.vertical = Some(match e.valor() {
                        "center" => 1,
                        "bottom" => 2,
                        _ => 0,
                    })
                }
                _ => {}
            }
        }
        p
    }

    pub fn mezclar(&mut self, o: &PropCelda) {
        mezclar_campos!(self, o, ancho, tramo, fusion_v, sombreado, vertical);
        self.bordes.mezclar(&o.bordes);
        self.margenes.mezclar(&o.margenes);
    }
}

/// Una parte condicional de un estilo de tabla (`w:tblStylePr`).
#[derive(Debug, Clone, Default)]
pub struct Condicional {
    pub ppr: PropParrafo,
    pub rpr: PropCorrida,
    pub tcpr: PropCelda,
    pub tblpr: PropTabla,
}

/// Un estilo de `styles.xml`.
#[derive(Debug, Clone, Default)]
pub struct EstiloW {
    pub tipo: String,
    pub basado: Option<String>,
    pub ppr: PropParrafo,
    pub rpr: PropCorrida,
    pub tblpr: PropTabla,
    pub tcpr: PropCelda,
    /// `firstRow`, `lastRow`, `firstCol`, `lastCol`, `band1Horz`, ...
    pub condicionales: HashMap<String, Condicional>,
}

/// Todos los estilos, con los valores por defecto del documento.
#[derive(Debug, Default)]
pub struct Estilos {
    pub mapa: HashMap<String, EstiloW>,
    pub doc_ppr: PropParrafo,
    pub doc_rpr: PropCorrida,
    pub defecto_parrafo: Option<String>,
    pub defecto_tabla: Option<String>,
    pub defecto_caracter: Option<String>,
    cache_p: RefCell<HashMap<String, (PropParrafo, PropCorrida)>>,
    cache_r: RefCell<HashMap<String, PropCorrida>>,
}

impl Estilos {
    pub fn leer(raiz: Option<&Nodo>, tema: &Tema) -> Estilos {
        let mut e = Estilos::default();
        let Some(raiz) = raiz else {
            return e;
        };
        if let Some(dd) = raiz.hijo("docDefaults") {
            if let Some(r) = dd.hijo("rPrDefault").and_then(|x| x.hijo("rPr")) {
                e.doc_rpr = PropCorrida::leer(r, tema);
            }
            if let Some(p) = dd.hijo("pPrDefault").and_then(|x| x.hijo("pPr")) {
                e.doc_ppr = PropParrafo::leer(p, tema);
            }
        }
        for s in raiz.elementos().filter(|s| s.nombre == "style") {
            let id = s.atributo("styleId").to_string();
            let tipo = s.atributo("type").to_string();
            let defecto = matches!(s.atributo("default"), "1" | "true" | "on");
            let mut w = EstiloW {
                tipo: tipo.clone(),
                ..Default::default()
            };
            for h in s.elementos() {
                match h.nombre.as_str() {
                    "basedOn" => w.basado = Some(h.valor().to_string()),
                    "pPr" => w.ppr = PropParrafo::leer(h, tema),
                    "rPr" => w.rpr = PropCorrida::leer(h, tema),
                    "tblPr" => w.tblpr = PropTabla::leer(h, tema),
                    "tcPr" => w.tcpr = PropCelda::leer(h, tema),
                    "tblStylePr" => {
                        let mut c = Condicional::default();
                        for x in h.elementos() {
                            match x.nombre.as_str() {
                                "pPr" => c.ppr = PropParrafo::leer(x, tema),
                                "rPr" => c.rpr = PropCorrida::leer(x, tema),
                                "tcPr" => c.tcpr = PropCelda::leer(x, tema),
                                "tblPr" => c.tblpr = PropTabla::leer(x, tema),
                                _ => {}
                            }
                        }
                        w.condicionales.insert(h.atributo("type").to_string(), c);
                    }
                    _ => {}
                }
            }
            if defecto {
                match tipo.as_str() {
                    "paragraph" => e.defecto_parrafo = Some(id.clone()),
                    "table" => e.defecto_tabla = Some(id.clone()),
                    "character" => e.defecto_caracter = Some(id.clone()),
                    _ => {}
                }
            }
            e.mapa.insert(id, w);
        }
        e
    }

    /// La cadena de un estilo hasta la raiz (la raiz primero), sin ciclos.
    fn cadena(&self, id: &str) -> Vec<&EstiloW> {
        let mut v = Vec::new();
        let mut actual = Some(id.to_string());
        let mut vistos: Vec<String> = Vec::new();
        while let Some(a) = actual {
            if vistos.contains(&a) || vistos.len() > 20 {
                break;
            }
            let Some(s) = self.mapa.get(&a) else { break };
            vistos.push(a);
            v.push(s);
            actual = s.basado.clone();
        }
        v.reverse();
        v
    }

    /// Las propiedades de parrafo y de corrida de un estilo de parrafo con
    /// su herencia (sin los valores del documento).
    pub fn de_parrafo(&self, id: &str) -> (PropParrafo, PropCorrida) {
        if let Some(c) = self.cache_p.borrow().get(id) {
            return c.clone();
        }
        let mut p = PropParrafo::default();
        let mut r = PropCorrida::default();
        for s in self.cadena(id) {
            p.mezclar(&s.ppr);
            r.mezclar(&s.rpr);
        }
        self.cache_p
            .borrow_mut()
            .insert(id.to_string(), (p.clone(), r.clone()));
        (p, r)
    }

    /// Las propiedades de un estilo de caracter con su herencia.
    pub fn de_caracter(&self, id: &str) -> PropCorrida {
        if let Some(c) = self.cache_r.borrow().get(id) {
            return c.clone();
        }
        let mut r = PropCorrida::default();
        for s in self.cadena(id) {
            r.mezclar(&s.rpr);
        }
        self.cache_r.borrow_mut().insert(id.to_string(), r.clone());
        r
    }

    /// Un estilo de tabla resuelto: sus propiedades y sus condicionales,
    /// con la herencia aplicada.
    pub fn de_tabla(
        &self,
        id: &str,
    ) -> (
        PropTabla,
        PropCelda,
        PropParrafo,
        PropCorrida,
        HashMap<String, Condicional>,
    ) {
        let mut t = PropTabla::default();
        let mut c = PropCelda::default();
        let mut p = PropParrafo::default();
        let mut r = PropCorrida::default();
        let mut cond: HashMap<String, Condicional> = HashMap::new();
        for s in self.cadena(id) {
            t.mezclar(&s.tblpr);
            c.mezclar(&s.tcpr);
            p.mezclar(&s.ppr);
            r.mezclar(&s.rpr);
            for (k, v) in &s.condicionales {
                let e = cond.entry(k.clone()).or_default();
                e.ppr.mezclar(&v.ppr);
                e.rpr.mezclar(&v.rpr);
                e.tcpr.mezclar(&v.tcpr);
                e.tblpr.mezclar(&v.tblpr);
            }
        }
        (t, c, p, r, cond)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_docs::xml::leer;

    #[test]
    fn un_estilo_hereda_de_su_base_y_lo_directo_manda() {
        let x = leer(
            r#"<w:styles>
            <w:docDefaults><w:rPrDefault><w:rPr><w:sz w:val="22"/></w:rPr></w:rPrDefault></w:docDefaults>
            <w:style w:type="paragraph" w:styleId="Base"><w:pPr><w:spacing w:after="200"/></w:pPr><w:rPr><w:b/></w:rPr></w:style>
            <w:style w:type="paragraph" w:styleId="Hijo"><w:basedOn w:val="Base"/><w:pPr><w:jc w:val="center"/></w:pPr><w:rPr><w:sz w:val="32"/></w:rPr></w:style>
            </w:styles>"#,
        )
        .unwrap();
        let e = Estilos::leer(Some(&x), &Tema::default());
        assert_eq!(e.doc_rpr.tam, Some(11.0));
        let (p, r) = e.de_parrafo("Hijo");
        assert_eq!(p.despues, Some(10.0));
        assert_eq!(p.alineacion, Some(Alineacion::Centro));
        assert_eq!(r.negrita, Some(true));
        assert_eq!(r.tam, Some(16.0));
    }

    #[test]
    fn un_ciclo_de_estilos_no_cuelga() {
        let x = leer(
            r#"<w:styles><w:style w:type="paragraph" w:styleId="A"><w:basedOn w:val="B"/></w:style><w:style w:type="paragraph" w:styleId="B"><w:basedOn w:val="A"/></w:style></w:styles>"#,
        )
        .unwrap();
        let e = Estilos::leer(Some(&x), &Tema::default());
        let _ = e.de_parrafo("A");
    }

    #[test]
    fn la_sangria_francesa_es_negativa() {
        let p = PropParrafo::leer(
            &leer(r#"<w:pPr><w:ind w:left="720" w:hanging="360"/></w:pPr>"#).unwrap(),
            &Tema::default(),
        );
        assert_eq!(p.izq, Some(36.0));
        assert_eq!(p.primera, Some(-18.0));
    }

    #[test]
    fn el_color_del_tema_con_sombra_se_oscurece() {
        let c = color_w(
            &leer(r#"<w:color w:val="2F5496" w:themeColor="accent1" w:themeShade="BF"/>"#).unwrap(),
            &Tema::leer(&leer("<a:theme/>").unwrap()),
        );
        let Some(ColorW::Fijo(c)) = c else { panic!() };
        // El acento por defecto (4472C4) al 75 % de luminosidad: 2F5496.
        assert!(
            (c.r as i32 - 0x2F).abs() <= 2 && (c.b as i32 - 0x96).abs() <= 2,
            "{c:?}"
        );
    }
}
