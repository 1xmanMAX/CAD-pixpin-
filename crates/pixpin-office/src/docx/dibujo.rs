//! **Los dibujos de Word**: imagenes, formas, cuadros de texto y grupos,
//! en linea (`wp:inline`, van en el renglon como una letra grande) o
//! anclados (`wp:anchor`, con su sitio en la pagina). Y el VML antiguo
//! (`w:pict`) en lo basico: la imagen.

use std::sync::Arc;

use pixpin_docs::xml::Nodo;

use crate::dml::{self, Contexto};
use crate::docx::bloques::{self, Ctx};
use crate::pagina::{Color, Elemento, Rect, Relleno, Trazo};
use crate::paquete::{attr_num, emu};

/// Un dibujo ya pintado en su caja, con origen (0, 0).
#[derive(Debug, Clone, Default)]
pub struct Objeto {
    pub ancho: f32,
    pub alto: f32,
    /// Lo que baja de la linea base (una formula); 0 en las imagenes.
    pub bajo: f32,
    pub elementos: Vec<Elemento>,
}

/// Respecto a que se coloca un dibujo anclado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Referencia {
    Pagina,
    Margen,
    Columna,
    Parrafo,
    /// Margenes interior/exterior, izquierdo/derecho... se tratan como el
    /// margen correspondiente.
    MargenIzq,
    MargenDer,
    MargenArriba,
    MargenAbajo,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Posicion {
    pub referencia: Referencia,
    /// Desplazamiento en puntos (si no hay alineacion).
    pub desplazamiento: f32,
    /// `left`, `center`, `right`, `top`, `bottom`, `inside`, `outside`.
    pub alineacion: Option<String>,
}

/// Como rodea el texto a un dibujo anclado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ajuste {
    /// Delante o detras del texto, sin apartarlo.
    Ninguno,
    /// El texto salta por encima y por debajo.
    ArribaAbajo,
    /// El texto rodea la caja (cuadrado, estrecho, transparente).
    Cuadrado,
}

/// A que lado del dibujo puede ir el texto (`wrapText`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lado {
    /// `bothSides`: a los dos lados.
    #[default]
    Ambos,
    /// `left`: solo a la izquierda.
    Izquierda,
    /// `right`: solo a la derecha.
    Derecha,
    /// `largest`: al lado con mas sitio.
    Mayor,
}

#[derive(Debug, Clone)]
pub struct Flotante {
    pub objeto: Objeto,
    pub h: Posicion,
    pub v: Posicion,
    pub detras: bool,
    pub ajuste: Ajuste,
    /// Distancia al texto (arriba, abajo, izquierda, derecha).
    pub margen: [f32; 4],
    pub lado: Lado,
    /// El contorno que sigue el texto (`wrapTight`/`wrapThrough` con
    /// `wp:wrapPolygon`), en fracciones del ancho y alto del dibujo.
    pub contorno: Option<Arc<[(f32, f32)]>>,
    /// `wrapThrough`: el texto entra tambien en los huecos del contorno.
    pub traspasa: bool,
}

pub enum Dibujo {
    EnLinea(Objeto),
    Anclado(Flotante),
    /// Una raya horizontal de lado a lado (`o:hr` del VML de las paginas
    /// web pegadas en Word).
    Regla {
        alto: f32,
        color: Color,
    },
}

/// Lee un `w:drawing` (o lo que haya dentro de un `mc:AlternateContent`).
pub fn dibujo(ctx: &mut Ctx, n: &Nodo) -> Option<Dibujo> {
    for e in n.elementos() {
        match e.nombre.as_str() {
            "inline" => {
                let (w, h) = extension(e)?;
                let elementos = grafico_de(ctx, e, w, h);
                return Some(Dibujo::EnLinea(Objeto {
                    ancho: w,
                    alto: h,
                    bajo: 0.0,
                    elementos,
                }));
            }
            "anchor" => return anclado(ctx, e).map(Dibujo::Anclado),
            _ => {}
        }
    }
    None
}

fn extension(e: &Nodo) -> Option<(f32, f32)> {
    let x = e.hijo("extent")?;
    Some((
        emu(attr_num(x, "cx", 1.0).unwrap_or(0.0)),
        emu(attr_num(x, "cy", 1.0).unwrap_or(0.0)),
    ))
}

fn anclado(ctx: &mut Ctx, e: &Nodo) -> Option<Flotante> {
    let (w, h) = extension(e)?;
    // Lo que el dibujo ocupa de mas por sus efectos (sombras...).
    let elementos = grafico_de(ctx, e, w, h);
    let pos = |nombre: &str| -> Posicion {
        let Some(p) = e.hijo(nombre) else {
            return Posicion {
                referencia: Referencia::Columna,
                desplazamiento: 0.0,
                alineacion: None,
            };
        };
        let referencia = match p.atributo("relativeFrom") {
            "page" => Referencia::Pagina,
            "margin" => Referencia::Margen,
            "paragraph" | "line" => Referencia::Parrafo,
            "leftMargin" | "insideMargin" => Referencia::MargenIzq,
            "rightMargin" | "outsideMargin" => Referencia::MargenDer,
            "topMargin" => Referencia::MargenArriba,
            "bottomMargin" => Referencia::MargenAbajo,
            _ => Referencia::Columna,
        };
        let desplazamiento = p
            .hijo("posOffset")
            .and_then(|o| o.texto().trim().parse::<f32>().ok())
            .map(emu)
            .unwrap_or(0.0);
        let alineacion = p.hijo("align").map(|a| a.texto().trim().to_string());
        Posicion {
            referencia,
            desplazamiento,
            alineacion,
        }
    };
    let (mut hpos, mut vpos) = (pos("positionH"), pos("positionV"));
    if matches!(e.atributo("simplePos"), "1" | "true")
        && let Some(sp) = e.hijo("simplePos")
    {
        hpos = Posicion {
            referencia: Referencia::Pagina,
            desplazamiento: emu(attr_num(sp, "x", 1.0).unwrap_or(0.0)),
            alineacion: None,
        };
        vpos = Posicion {
            referencia: Referencia::Pagina,
            desplazamiento: emu(attr_num(sp, "y", 1.0).unwrap_or(0.0)),
            alineacion: None,
        };
    }
    let (ajuste, forma) = if let Some(x) = e.hijo("wrapTopAndBottom") {
        (Ajuste::ArribaAbajo, Some(x))
    } else if let Some(x) = e
        .hijo("wrapSquare")
        .or_else(|| e.hijo("wrapTight"))
        .or_else(|| e.hijo("wrapThrough"))
    {
        (Ajuste::Cuadrado, Some(x))
    } else {
        (Ajuste::Ninguno, None)
    };
    // Las distancias al texto: las del ajuste mandan sobre las del ancla.
    let d = |k: &str| {
        let v = forma
            .and_then(|f| attr_num(f, k, 1.0))
            .or_else(|| attr_num(e, k, 1.0))
            .unwrap_or(0.0);
        emu(v).clamp(0.0, 1584.0)
    };
    let lado = match forma.map(|f| f.atributo("wrapText")).unwrap_or("") {
        "left" => Lado::Izquierda,
        "right" => Lado::Derecha,
        "largest" => Lado::Mayor,
        _ => Lado::Ambos,
    };
    let estrecho = forma.is_some_and(|f| f.nombre == "wrapTight" || f.nombre == "wrapThrough");
    let contorno = forma
        .filter(|_| estrecho)
        .and_then(|f| f.hijo("wrapPolygon"))
        .and_then(poligono);
    Some(Flotante {
        objeto: Objeto {
            ancho: w,
            alto: h,
            bajo: 0.0,
            elementos,
        },
        h: hpos,
        v: vpos,
        detras: matches!(e.atributo("behindDoc"), "1" | "true"),
        ajuste,
        margen: [d("distT"), d("distB"), d("distL"), d("distR")],
        lado,
        contorno,
        traspasa: forma.is_some_and(|f| f.nombre == "wrapThrough"),
    })
}

/// Los puntos de un `wp:wrapPolygon` (`wp:start` y `wp:lineTo`, en 21600
/// partes del dibujo) como fracciones de su ancho y alto. `None` con menos
/// de tres puntos.
fn poligono(p: &Nodo) -> Option<Arc<[(f32, f32)]>> {
    const PARTES: f32 = 21600.0;
    let v: Vec<(f32, f32)> = p
        .elementos()
        .filter(|x| x.nombre == "start" || x.nombre == "lineTo")
        .filter_map(|x| {
            Some((
                attr_num(x, "x", 1.0)? / PARTES,
                attr_num(x, "y", 1.0)? / PARTES,
            ))
        })
        .filter(|(x, y)| x.is_finite() && y.is_finite())
        .map(|(x, y)| (x.clamp(-1.0, 2.0), y.clamp(-1.0, 2.0)))
        .take(1000)
        .collect();
    (v.len() >= 3).then(|| Arc::from(v))
}

/// El contenido de un `a:graphic` dentro de `padre` (inline o anchor).
fn grafico_de(ctx: &mut Ctx, padre: &Nodo, w: f32, h: f32) -> Vec<Elemento> {
    let Some(datos) = padre.hijo("graphic").and_then(|g| g.hijo("graphicData")) else {
        return Vec::new();
    };
    // Un grafico: dibujado con los datos de su cache.
    let mapa = std::collections::HashMap::new();
    let dctx = Contexto {
        tema: ctx.tema,
        mapa: &mapa,
    };
    if let Some(els) =
        crate::grafico::de_datos(ctx.paquete, ctx.rels, datos, dctx, ctx.medidor, w, h)
    {
        return els;
    }
    let mut v = Vec::new();
    for hijo in datos.elementos() {
        forma(ctx, hijo, Rect::nuevo(0.0, 0.0, w, h), &mut v);
    }
    v
}

/// Una imagen de las relaciones del documento: la carga en los recursos y
/// devuelve su id.
pub fn imagen(ctx: &mut Ctx, rid: &str) -> Option<Arc<str>> {
    let rel = ctx.rels.get(rid)?;
    if rel.externa {
        return None;
    }
    let id: Arc<str> = Arc::from(rel.destino.as_str());
    if !ctx.recursos.tiene(&id) {
        let b = ctx.paquete.bytes(&rel.destino)?;
        ctx.recursos.poner(id.clone(), Arc::from(b));
    }
    Some(id)
}

/// Pinta una forma de DrawingML (`pic:pic`, `wps:wsp`, `wpg:wgp`...) en
/// `caja`.
fn forma(ctx: &mut Ctx, n: &Nodo, caja: Rect, salida: &mut Vec<Elemento>) {
    let mapa = std::collections::HashMap::new();
    let tema = ctx.tema;
    let dctx = Contexto { tema, mapa: &mapa };
    match n.nombre.as_str() {
        "pic" => {
            let sppr = n.hijo("spPr");
            let x = sppr
                .and_then(|s| s.hijo("xfrm"))
                .map(dml::xfrm)
                .unwrap_or_default();
            let Some(bf) = n.hijo("blipFill") else { return };
            let rid = bf
                .hijo("blip")
                .map(|b| b.atributo_con_prefijo("r", "embed").to_string())
                .unwrap_or_default();
            let Some(id) = imagen(ctx, &rid) else { return };
            let el = Elemento::Imagen {
                id,
                rect: Rect::nuevo(0.0, 0.0, caja.ancho, caja.alto),
                recorte: dml::recorte(bf),
                opacidad: 1.0,
            };
            let mut els = vec![el];
            if let Some(ln) = sppr
                .and_then(|s| s.hijo("ln"))
                .and_then(|l| dml::linea(l, dctx, None))
                .flatten()
            {
                els.push(Elemento::Rect {
                    rect: Rect::nuevo(0.0, 0.0, caja.ancho, caja.alto),
                    relleno: None,
                    borde: Some(ln),
                });
            }
            // La sombra de las imagenes (muy comun en Word).
            let efectos = dml::efectos_forma(sppr, n.hijo("style"), &[], dctx, x.giro);
            salida.push(
                dml::colocar(els, caja, x.giro, x.voltear_h, x.voltear_v).con_efectos(efectos),
            );
        }
        "wsp" => {
            let Some(sppr) = n.hijo("spPr") else { return };
            let x = sppr.hijo("xfrm").map(dml::xfrm).unwrap_or_default();
            let estilo = n
                .hijo("style")
                .map(|s| dml::estilo_forma(s, dctx))
                .unwrap_or_default();
            let relleno = match dml::relleno(sppr, dctx, None) {
                Some(r) => r,
                None => estilo.relleno.clone(),
            };
            let borde = match sppr
                .hijo("ln")
                .and_then(|l| dml::linea(l, dctx, estilo.linea.map(|t| t.color)))
            {
                Some(t) => t,
                None => {
                    // Un `a:ln` con grosor pero sin color toma el del estilo.
                    let g = sppr.hijo("ln").and_then(|l| attr_num(l, "w", 1.0)).map(emu);
                    estilo.linea.map(|t| Trazo {
                        grosor: g.unwrap_or(t.grosor),
                        ..t
                    })
                }
            };
            let (figuras, es_linea) = dml::figuras(sppr, caja.ancho, caja.alto);
            let mut els = Vec::new();
            if !figuras.is_empty() && (relleno.is_some() || borde.is_some()) {
                els.push(Elemento::Trayecto {
                    figuras,
                    relleno: if es_linea { None } else { relleno },
                    borde,
                });
            }
            // El cuadro de texto.
            if let Some(txbx) = n.hijo("txbx").and_then(|t| t.hijo("txbxContent")) {
                let bp = n.hijo("bodyPr");
                let ins =
                    |k: &str, d: f32| bp.and_then(|b| attr_num(b, k, 1.0)).map(emu).unwrap_or(d);
                let (l, t, r, b) = (
                    ins("lIns", 7.2),
                    ins("tIns", 3.6),
                    ins("rIns", 7.2),
                    ins("bIns", 3.6),
                );
                let ancho = (caja.ancho - l - r).max(1.0);
                let contenido = bloques::caja_con_flotantes(ctx, txbx.elementos(), ancho);
                let anclaje = bp.map(|b| b.atributo("anchor")).unwrap_or("t");
                let libre = (caja.alto - t - b - contenido.alto).max(0.0);
                let dy = match anclaje {
                    "ctr" => libre / 2.0,
                    "b" => libre,
                    _ => 0.0,
                };
                for mut e in contenido.elementos {
                    e.trasladar(l, t + dy);
                    els.push(e);
                }
            }
            let efectos = dml::efectos_forma(Some(sppr), n.hijo("style"), &[], dctx, x.giro);
            salida.push(
                dml::colocar(els, caja, x.giro, x.voltear_h, x.voltear_v).con_efectos(efectos),
            );
        }
        "wgp" | "grpSp" | "wpc" => {
            let gx = n.hijo("grpSpPr").map(dml::xfrm).unwrap_or_default();
            // Las coordenadas de los hijos van en el sistema `chOff/chExt`
            // del grupo, que se estira a la caja.
            let sx = if gx.hijo_ancho > 0.0 {
                caja.ancho / gx.hijo_ancho
            } else {
                1.0
            };
            let sy = if gx.hijo_alto > 0.0 {
                caja.alto / gx.hijo_alto
            } else {
                1.0
            };
            let mut hijos = Vec::new();
            for h in n.elementos() {
                let sppr = h.hijo("spPr").or_else(|| h.hijo("grpSpPr"));
                let Some(hx) = sppr.and_then(|s| s.hijo("xfrm")).map(dml::xfrm) else {
                    continue;
                };
                let r = Rect::nuevo(
                    (hx.x - gx.hijo_x) * sx,
                    (hx.y - gx.hijo_y) * sy,
                    hx.ancho * sx,
                    hx.alto * sy,
                );
                forma(ctx, h, r, &mut hijos);
            }
            let efectos = dml::efectos_forma(n.hijo("grpSpPr"), None, &[], dctx, gx.giro);
            salida.push(
                dml::colocar(hijos, caja, gx.giro, gx.voltear_h, gx.voltear_v).con_efectos(efectos),
            );
        }
        _ => {}
    }
}

/// Una imagen VML (`w:pict` / `w:object` con `v:imagedata`): ancho y alto
/// del `style` de la forma.
pub fn vml(ctx: &mut Ctx, n: &Nodo) -> Option<Dibujo> {
    let forma = n
        .descendientes("shape")
        .into_iter()
        .next()
        .or_else(|| n.descendientes("rect").into_iter().next())?;
    let estilo = forma.atributo("style");
    let prop = |k: &str| -> Option<&str> {
        estilo.split(';').find_map(|p| {
            let (a, b) = p.split_once(':')?;
            (a.trim() == k).then_some(b.trim())
        })
    };
    let medida = |k: &str| prop(k).and_then(|v| crate::paquete::numero(v, 1.0));
    if matches!(forma.atributo("hr"), "t" | "true") {
        let color = Color::hex(forma.atributo("fillcolor")).unwrap_or(Color::rgb(0xA0, 0xA0, 0xA0));
        return Some(Dibujo::Regla {
            alto: medida("height").unwrap_or(1.5).max(0.75),
            color,
        });
    }
    let w = medida("width")?;
    let h = medida("height")?;
    let mut elementos = Vec::new();
    if let Some(img) = forma.hijo("imagedata") {
        let rid = img.atributo_con_prefijo("r", "id");
        let rid = if rid.is_empty() {
            img.atributo("id")
        } else {
            rid
        };
        if let Some(id) = imagen(ctx, rid) {
            elementos.push(Elemento::Imagen {
                id,
                rect: Rect::nuevo(0.0, 0.0, w, h),
                recorte: None,
                opacidad: 1.0,
            });
        }
    } else {
        // Los valores por defecto del VML: relleno blanco y trazo negro de
        // 0,75 pt, salvo que se apaguen.
        let no = |k: &str| matches!(forma.atributo(k), "f" | "false");
        let relleno = if no("filled") {
            None
        } else {
            Some(Relleno::Solido(
                Color::hex(forma.atributo("fillcolor")).unwrap_or(Color::BLANCO),
            ))
        };
        let borde = if no("stroked") {
            None
        } else {
            let grosor =
                crate::paquete::numero(forma.atributo("strokeweight"), 1.0).unwrap_or(0.75);
            Some(crate::pagina::Trazo::solido(
                Color::hex(forma.atributo("strokecolor")).unwrap_or(Color::NEGRO),
                grosor,
            ))
        };
        let es_caja = forma.hijo("textbox").is_some();
        if (relleno.is_some() || borde.is_some())
            && (forma.nombre == "rect" || es_caja || forma.atributo("type").contains("t202"))
        {
            elementos.push(Elemento::Rect {
                rect: Rect::nuevo(0.0, 0.0, w, h),
                relleno,
                borde,
            });
        }
        // Un cuadro de texto VML.
        if let Some(tb) = forma.hijo("textbox")
            && let Some(contenido) = tb.hijo("txbxContent")
        {
            let ins: Vec<f32> = tb
                .atributo("inset")
                .split(',')
                .map(|v| crate::paquete::numero(v, 1.0).unwrap_or(f32::NAN))
                .collect();
            let m = |i: usize, d: f32| ins.get(i).copied().filter(|v| v.is_finite()).unwrap_or(d);
            let (l, t, r) = (m(0, 7.2), m(1, 3.6), m(2, 7.2));
            let caja = bloques::caja(ctx, contenido.elementos(), (w - l - r).max(1.0));
            for mut e in caja.elementos {
                e.trasladar(l, t);
                elementos.push(e);
            }
        }
    }
    if elementos.is_empty() {
        return None;
    }
    let objeto = Objeto {
        ancho: w,
        alto: h,
        bajo: 0.0,
        elementos,
    };
    if prop("position") != Some("absolute") {
        return Some(Dibujo::EnLinea(objeto));
    }
    let referencia = |k: &str| match prop(k) {
        Some("page") => Referencia::Pagina,
        Some("margin") => Referencia::Margen,
        Some("text") => Referencia::Columna,
        _ => Referencia::Parrafo,
    };
    let alin = |k: &str| prop(k).filter(|v| *v != "absolute").map(|s| s.to_string());
    Some(Dibujo::Anclado(Flotante {
        objeto,
        h: Posicion {
            referencia: match prop("mso-position-horizontal-relative") {
                None => Referencia::Columna,
                _ => referencia("mso-position-horizontal-relative"),
            },
            desplazamiento: medida("margin-left").unwrap_or(0.0),
            alineacion: alin("mso-position-horizontal"),
        },
        v: Posicion {
            referencia: referencia("mso-position-vertical-relative"),
            desplazamiento: medida("margin-top").unwrap_or(0.0),
            alineacion: alin("mso-position-vertical"),
        },
        detras: prop("z-index")
            .and_then(|z| z.parse::<i64>().ok())
            .is_some_and(|z| z < 0),
        ajuste: Ajuste::Ninguno,
        margen: [0.0; 4],
        lado: Lado::Ambos,
        contorno: None,
        traspasa: false,
    }))
}
