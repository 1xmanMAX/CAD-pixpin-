//! Lo que flota sobre la hoja (`xl/drawings/drawingN.xml`): imagenes,
//! graficos y formas, con su ancla a las celdas.

use crate::estilos::{Rgb, Tema};
use crate::paquete::Paquete;
use crate::xml::{self, Nodo};

/// EMU por pixel a 96 ppp.
pub const EMU_POR_PX: f64 = 9525.0;

/// Un punto anclado a una celda: columna/fila (base 0) y desplazamiento en
/// pixeles dentro de ella.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PuntoAncla {
    pub col: u32,
    pub dx: f64,
    pub fila: u32,
    pub dy: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ancla {
    /// De una celda a otra.
    DosCeldas {
        desde: PuntoAncla,
        hasta: PuntoAncla,
    },
    /// Desde una celda, con tamano fijo en pixeles.
    UnaCelda {
        desde: PuntoAncla,
        ancho: f64,
        alto: f64,
    },
    /// Posicion absoluta en pixeles desde A1.
    Absoluta {
        x: f64,
        y: f64,
        ancho: f64,
        alto: f64,
    },
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Serie {
    pub nombre: String,
    pub valores: Vec<f64>,
    pub color: Option<Rgb>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Contenido {
    /// Ruta de la imagen dentro del ZIP y recorte (izq, arr, der, aba) en
    /// fraccion del original.
    Imagen { ruta: String, recorte: [f32; 4] },
    /// Un grafico: su titulo y los datos guardados en cache, para pintar
    /// algo parecido (o al menos un recuadro con el titulo).
    Grafico {
        titulo: String,
        tipo: String,
        categorias: Vec<String>,
        series: Vec<Serie>,
    },
    /// Una forma o cuadro de texto.
    /// `alinear`: 0 izquierda, 1 centro, 2 derecha; `vertical`: 0 arriba,
    /// 1 centro, 2 abajo (lo de su primer parrafo y su `bodyPr`).
    Forma {
        texto: String,
        relleno: Option<Rgb>,
        linea: Option<Rgb>,
        alinear: u8,
        vertical: u8,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Dibujo {
    pub ancla: Ancla,
    /// Para los hijos de un grupo: su sitio dentro de la caja del ancla
    /// (x, y, ancho, alto en fraccion).
    pub dentro: Option<[f64; 4]>,
    pub contenido: Contenido,
}

fn emu(n: &Nodo, h: &str) -> f64 {
    n.hijo(h)
        .map(|x| x.texto.trim().parse::<f64>().unwrap_or(0.0))
        .unwrap_or(0.0)
        / EMU_POR_PX
}

fn punto(n: &Nodo) -> PuntoAncla {
    let u = |h: &str| {
        n.hijo(h)
            .and_then(|x| x.texto.trim().parse::<f64>().ok())
            .unwrap_or(0.0)
            .clamp(0.0, 1_048_575.0) as u32
    };
    PuntoAncla {
        col: u("col").min(16383),
        dx: emu(n, "colOff"),
        fila: u("row"),
        dy: emu(n, "rowOff"),
    }
}

fn ext(n: &Nodo) -> (f64, f64) {
    let a = |k: &str| n.attr(k).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) / EMU_POR_PX;
    (a("cx"), a("cy"))
}

/// Lee un dibujo y lo que cuelga de el.
pub fn leer(p: &Paquete, ruta: &str, tema: &Tema, paleta: &[u32]) -> Vec<Dibujo> {
    let Some(bytes) = p.leer(ruta) else {
        return Vec::new();
    };
    let raiz = xml::arbol(&bytes);
    let rels = p.relaciones(ruta);
    let Some(ws) = raiz.hijo("wsDr") else {
        return Vec::new();
    };
    let mut v = Vec::new();
    for a in &ws.hijos {
        let ancla = match a.nombre.as_str() {
            "twoCellAnchor" => {
                let (Some(d), Some(h)) = (a.hijo("from"), a.hijo("to")) else {
                    continue;
                };
                Ancla::DosCeldas {
                    desde: punto(d),
                    hasta: punto(h),
                }
            }
            "oneCellAnchor" => {
                let Some(d) = a.hijo("from") else {
                    continue;
                };
                let (w, h) = a.hijo("ext").map(ext).unwrap_or((0.0, 0.0));
                Ancla::UnaCelda {
                    desde: punto(d),
                    ancho: w,
                    alto: h,
                }
            }
            "absoluteAnchor" => {
                let pos = a.hijo("pos");
                let px = |k: &str| {
                    pos.and_then(|q| q.attr(k))
                        .and_then(|x| x.parse::<f64>().ok())
                        .unwrap_or(0.0)
                        / EMU_POR_PX
                };
                let (w, h) = a.hijo("ext").map(ext).unwrap_or((0.0, 0.0));
                Ancla::Absoluta {
                    x: px("x"),
                    y: px("y"),
                    ancho: w,
                    alto: h,
                }
            }
            _ => continue,
        };
        for h in &a.hijos {
            elemento(h, ancla, None, p, &rels, tema, paleta, &mut v, 0);
        }
    }
    v
}

#[allow(clippy::too_many_arguments)]
fn elemento(
    h: &Nodo,
    ancla: Ancla,
    dentro: Option<[f64; 4]>,
    p: &Paquete,
    rels: &[crate::paquete::Relacion],
    tema: &Tema,
    paleta: &[u32],
    v: &mut Vec<Dibujo>,
    nivel: u32,
) {
    if nivel > 8 {
        return;
    }
    match h.nombre.as_str() {
        "pic" => {
            let Some(blip) = h.buscar("blip") else {
                return;
            };
            let Some(r) = blip
                .attr("embed")
                .and_then(|id| rels.iter().find(|r| r.id == id))
            else {
                return;
            };
            let mut recorte = [0.0f32; 4];
            if let Some(s) = h.buscar("srcRect") {
                for (i, k) in ["l", "t", "r", "b"].iter().enumerate() {
                    recorte[i] =
                        s.attr(k).and_then(|x| x.parse::<f32>().ok()).unwrap_or(0.0) / 100_000.0;
                }
            }
            v.push(Dibujo {
                ancla,
                dentro: dentro.or_else(|| sitio_propio(h)),
                contenido: Contenido::Imagen {
                    ruta: r.destino.clone(),
                    recorte,
                },
            });
        }
        "graphicFrame" => {
            let Some(c) = h.buscar("chart") else {
                return;
            };
            let Some(r) = c.attr("id").and_then(|id| rels.iter().find(|r| r.id == id)) else {
                return;
            };
            let contenido = grafico(p, &r.destino, tema);
            v.push(Dibujo {
                ancla,
                dentro,
                contenido,
            });
        }
        "sp" | "cxnSp" => {
            let cuerpo = h.hijo("txBody");
            let texto = cuerpo.map(texto_de_cuerpo).unwrap_or_default();
            let vertical = match cuerpo
                .and_then(|c| c.hijo("bodyPr"))
                .and_then(|b| b.attr("anchor"))
            {
                Some("ctr") => 1,
                Some("b") => 2,
                _ => 0,
            };
            let alinear = match cuerpo
                .and_then(|c| c.hijo("p"))
                .and_then(|p| p.hijo("pPr"))
                .and_then(|p| p.attr("algn"))
            {
                Some("ctr") => 1,
                Some("r") => 2,
                _ => 0,
            };
            let sp = h.hijo("spPr");
            let relleno = sp
                .and_then(|s| s.hijo("solidFill"))
                .and_then(|f| color_dml(f, tema, paleta));
            let linea = sp
                .and_then(|s| s.hijo("ln"))
                .and_then(|l| l.hijo("solidFill"))
                .and_then(|f| color_dml(f, tema, paleta));
            // Sin relleno ni linea propios, la forma usa su estilo (`<xdr:style>`).
            let estilo = h.hijo("style");
            let relleno = relleno.or_else(|| {
                if sp.and_then(|s| s.hijo("noFill")).is_some() {
                    return None;
                }
                estilo
                    .and_then(|e| e.hijo("fillRef"))
                    .and_then(|f| color_dml(f, tema, paleta))
            });
            let linea = linea.or_else(|| {
                if sp
                    .and_then(|s| s.hijo("ln"))
                    .and_then(|l| l.hijo("noFill"))
                    .is_some()
                {
                    return None;
                }
                estilo
                    .and_then(|e| e.hijo("lnRef"))
                    .and_then(|f| color_dml(f, tema, paleta))
            });
            if texto.is_empty() && relleno.is_none() && linea.is_none() {
                return;
            }
            v.push(Dibujo {
                ancla,
                dentro: dentro.or_else(|| sitio_propio(h)),
                contenido: Contenido::Forma {
                    texto,
                    relleno,
                    linea,
                    alinear,
                    vertical,
                },
            });
        }
        "grpSp" => {
            // Los hijos de un grupo van en coordenadas del grupo: se pasan a
            // fracciones de la caja del ancla.
            let xf = h.hijo("grpSpPr").and_then(|g| g.hijo("xfrm"));
            let ch_off = xf.and_then(|x| x.hijo("chOff"));
            let ch_ext = xf.and_then(|x| x.hijo("chExt"));
            let n = |n: Option<&Nodo>, k: &str| {
                n.and_then(|q| q.attr(k))
                    .and_then(|x| x.parse::<f64>().ok())
                    .unwrap_or(0.0)
            };
            let (ox, oy, ew, eh) = (
                n(ch_off, "x"),
                n(ch_off, "y"),
                n(ch_ext, "cx"),
                n(ch_ext, "cy"),
            );
            for c in &h.hijos {
                let sitio = c
                    .hijo("spPr")
                    .or_else(|| c.hijo("grpSpPr"))
                    .and_then(|s| s.hijo("xfrm"))
                    .and_then(|x| {
                        if ew <= 0.0 || eh <= 0.0 {
                            return None;
                        }
                        let (Some(off), Some(ex)) = (x.hijo("off"), x.hijo("ext")) else {
                            return None;
                        };
                        Some([
                            (n(Some(off), "x") - ox) / ew,
                            (n(Some(off), "y") - oy) / eh,
                            n(Some(ex), "cx") / ew,
                            n(Some(ex), "cy") / eh,
                        ])
                    });
                let sitio = match (dentro, sitio) {
                    (Some(d), Some(s)) => Some([
                        d[0] + s[0] * d[2],
                        d[1] + s[1] * d[3],
                        s[2] * d[2],
                        s[3] * d[3],
                    ]),
                    (None, s) => s,
                    (d, None) => d,
                };
                elemento(c, ancla, sitio, p, rels, tema, paleta, v, nivel + 1);
            }
        }
        _ => {}
    }
}

fn sitio_propio(_h: &Nodo) -> Option<[f64; 4]> {
    None
}

fn texto_de_cuerpo(t: &Nodo) -> String {
    let mut s = String::new();
    for (i, p) in t.hijos_de("p").enumerate() {
        if i > 0 {
            s.push('\n');
        }
        let mut ts = Vec::new();
        p.todos("t", &mut ts);
        for x in ts {
            s.push_str(&x.texto);
        }
    }
    s
}

/// Un color de DrawingML (`srgbClr`, `schemeClr`, `sysClr`, con `lumMod` y
/// `lumOff` basicos).
fn color_dml(n: &Nodo, tema: &Tema, paleta: &[u32]) -> Option<Rgb> {
    let _ = paleta;
    for h in &n.hijos {
        let base = match h.nombre.as_str() {
            "srgbClr" => h
                .attr("val")
                .and_then(|v| u32::from_str_radix(v, 16).ok())
                .map(Rgb::de_u32),
            "sysClr" => h
                .attr("lastClr")
                .and_then(|v| u32::from_str_radix(v, 16).ok())
                .map(Rgb::de_u32),
            "schemeClr" => {
                let i = match h.attr("val").unwrap_or("") {
                    "lt1" | "bg1" => 0,
                    "dk1" | "tx1" => 1,
                    "lt2" | "bg2" => 2,
                    "dk2" | "tx2" => 3,
                    "accent1" => 4,
                    "accent2" => 5,
                    "accent3" => 6,
                    "accent4" => 7,
                    "accent5" => 8,
                    "accent6" => 9,
                    "hlink" => 10,
                    "folHlink" => 11,
                    _ => return None,
                };
                Some(tema.colores[i])
            }
            _ => None,
        };
        if let Some(mut c) = base {
            let m = |k: &str| {
                h.hijo(k)
                    .and_then(|x| x.attr("val"))
                    .and_then(|v| v.parse::<f64>().ok())
                    .map(|v| v / 100_000.0)
            };
            if let (Some(lm), off) = (m("lumMod"), m("lumOff")) {
                let ajusta = |x: u8| {
                    ((x as f64 / 255.0 * lm + off.unwrap_or(0.0)) * 255.0)
                        .round()
                        .clamp(0.0, 255.0) as u8
                };
                // Aproximacion en RGB del ajuste de luminosidad.
                c = Rgb {
                    r: ajusta(c.r),
                    g: ajusta(c.g),
                    b: ajusta(c.b),
                };
            } else if let Some(s) = m("shade") {
                c = Rgb::NEGRO.mezcla(c, 1.0 - s as f32);
            } else if let Some(t) = m("tint") {
                c = Rgb::BLANCO.mezcla(c, 1.0 - t as f32);
            }
            return Some(c);
        }
    }
    None
}

fn grafico(p: &Paquete, ruta: &str, tema: &Tema) -> Contenido {
    let vacio = Contenido::Grafico {
        titulo: String::new(),
        tipo: String::new(),
        categorias: Vec::new(),
        series: Vec::new(),
    };
    let Some(bytes) = p.leer(ruta) else {
        return vacio;
    };
    let raiz = xml::arbol(&bytes);
    let Some(chart) = raiz.buscar("chart") else {
        return vacio;
    };
    let mut titulo = String::new();
    if let Some(t) = chart.hijo("title") {
        let mut ts = Vec::new();
        t.todos("t", &mut ts);
        titulo = ts.iter().map(|x| x.texto.as_str()).collect();
        if titulo.is_empty() {
            if let Some(v) = t.buscar("v") {
                titulo = v.texto.clone();
            }
        }
    }
    let plot = chart.hijo("plotArea");
    let tipo_nodo = plot.and_then(|pa| pa.hijos.iter().find(|h| h.nombre.ends_with("Chart")));
    let mut tipo = tipo_nodo.map(|n| n.nombre.clone()).unwrap_or_default();
    if let Some(n) = tipo_nodo {
        if n.hijo("barDir").and_then(|b| b.attr("val")) == Some("bar") {
            tipo = "barHChart".into();
        }
    }
    let mut series = Vec::new();
    let mut categorias = Vec::new();
    if let Some(n) = tipo_nodo {
        for (i, s) in n.hijos_de("ser").enumerate() {
            let nombre = s.hijo("tx").map(|t| {
                let mut vs = Vec::new();
                t.todos("v", &mut vs);
                vs.first().map(|v| v.texto.clone()).unwrap_or_default()
            });
            let valores = s
                .hijo("val")
                .or_else(|| s.hijo("yVal"))
                .map(cache_numeros)
                .unwrap_or_default();
            if categorias.is_empty() {
                if let Some(c) = s.hijo("cat").or_else(|| s.hijo("xVal")) {
                    let mut pts = Vec::new();
                    c.todos("pt", &mut pts);
                    categorias = pts
                        .iter()
                        .filter_map(|p| p.hijo("v").map(|v| v.texto.clone()))
                        .collect();
                }
            }
            let color = s.hijo("spPr").and_then(|sp| {
                sp.hijo("solidFill")
                    .or_else(|| sp.hijo("ln").and_then(|l| l.hijo("solidFill")))
            });
            let color = color
                .and_then(|c| color_dml(c, tema, &[]))
                .or(Some(tema.colores[4 + i % 6]));
            series.push(Serie {
                nombre: nombre.unwrap_or_default(),
                valores,
                color,
            });
        }
    }
    if titulo.is_empty()
        && chart.hijo("autoTitleDeleted").and_then(|a| a.attr("val")) != Some("1")
        && series.len() == 1
    {
        titulo = series[0].nombre.clone();
    }
    Contenido::Grafico {
        titulo,
        tipo,
        categorias,
        series,
    }
}

fn cache_numeros(n: &Nodo) -> Vec<f64> {
    let mut pts = Vec::new();
    n.todos("pt", &mut pts);
    let total = n
        .buscar("ptCount")
        .and_then(|c| c.attr("val"))
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(pts.len())
        .min(100_000);
    let mut v = vec![f64::NAN; total.max(pts.len().min(100_000))];
    for (k, p) in pts.iter().enumerate() {
        let i = p
            .attr("idx")
            .and_then(|x| x.parse::<usize>().ok())
            .unwrap_or(k);
        if let (Some(slot), Some(x)) = (
            v.get_mut(i),
            p.hijo("v").and_then(|x| x.texto.trim().parse::<f64>().ok()),
        ) {
            *slot = x;
        }
    }
    v
}
