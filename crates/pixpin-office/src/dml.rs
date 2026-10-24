//! **DrawingML comun** a Word y PowerPoint: rellenos (`a:solidFill`,
//! `a:gradFill`, `a:noFill`), lineas (`a:ln`), los estilos de forma del tema
//! (`fillRef`, `lnRef`, `fontRef`, `effectRef`), los efectos (`a:effectLst`)
//! y la geometria de una `a:xfrm`.

use std::collections::HashMap;

use pixpin_docs::xml::Nodo;

use crate::geometria;
use crate::pagina::{Color, Discontinuo, Efecto, Elemento, Figura, Rect, Relleno, Trazo};
use crate::paquete::{attr_num, emu};
use crate::tema::{self, Tema};

/// Lo necesario para resolver colores: el tema y el mapa de colores.
#[derive(Clone, Copy)]
pub struct Contexto<'a> {
    pub tema: &'a Tema,
    pub mapa: &'a HashMap<String, String>,
}

/// Lo que dice un nodo de relleno: `None` si no es de relleno; `Some(None)`
/// si es `noFill`.
pub fn relleno_de(n: &Nodo, ctx: Contexto, ph: Option<Color>) -> Option<Option<Relleno>> {
    match n.nombre.as_str() {
        "noFill" => Some(None),
        "solidFill" => Some(tema::color_hijo(n, ctx.tema, ctx.mapa, ph).map(Relleno::Solido)),
        "gradFill" => {
            let mut paradas: Vec<(f32, Color)> = n
                .hijo("gsLst")
                .map(|l| {
                    l.elementos()
                        .filter(|g| g.nombre == "gs")
                        .filter_map(|g| {
                            let pos = attr_num(g, "pos", 1.0).unwrap_or(0.0) / 100000.0;
                            tema::color_hijo(g, ctx.tema, ctx.mapa, ph).map(|c| (pos, c))
                        })
                        .collect()
                })
                .unwrap_or_default();
            paradas.sort_by(|a, b| a.0.total_cmp(&b.0));
            if paradas.is_empty() {
                return Some(None);
            }
            if n.hijo("path").is_some() {
                return Some(Some(Relleno::Radial { paradas }));
            }
            let angulo = n
                .hijo("lin")
                .and_then(|l| attr_num(l, "ang", 1.0))
                .unwrap_or(0.0)
                / 60000.0;
            Some(Some(Relleno::Lineal { paradas, angulo }))
        }
        "pattFill" => {
            // Una trama se resume en su color de fondo mezclado con el de
            // delante.
            let fg = n
                .hijo("fgClr")
                .and_then(|c| tema::color_hijo(c, ctx.tema, ctx.mapa, ph));
            let bg = n
                .hijo("bgClr")
                .and_then(|c| tema::color_hijo(c, ctx.tema, ctx.mapa, ph));
            Some(match (fg, bg) {
                (Some(f), Some(b)) => Some(Relleno::Solido(Color::rgb(
                    ((f.r as u16 + 3 * b.r as u16) / 4) as u8,
                    ((f.g as u16 + 3 * b.g as u16) / 4) as u8,
                    ((f.b as u16 + 3 * b.b as u16) / 4) as u8,
                ))),
                (Some(c), None) | (None, Some(c)) => Some(Relleno::Solido(c)),
                _ => None,
            })
        }
        "grpFill" => None,
        _ => None,
    }
}

/// El relleno que dice un `spPr` (o cualquier padre de un relleno).
pub fn relleno(sppr: &Nodo, ctx: Contexto, ph: Option<Color>) -> Option<Option<Relleno>> {
    sppr.elementos().find_map(|e| relleno_de(e, ctx, ph))
}

/// La linea de un `a:ln`: `None` si no dice nada de color (hereda),
/// `Some(None)` si es `noFill`.
pub fn linea(ln: &Nodo, ctx: Contexto, ph: Option<Color>) -> Option<Option<Trazo>> {
    let grosor = attr_num(ln, "w", 1.0).map(emu);
    let color = match ln.elementos().find_map(|e| relleno_de(e, ctx, ph)) {
        Some(None) => return Some(None),
        Some(Some(r)) => Some(r.color_medio()),
        None => None,
    };
    let discontinuo = ln.hijo("prstDash").map(|d| match d.valor() {
        "dot" | "sysDot" => Discontinuo::Puntos,
        "dash" | "sysDash" | "lgDash" => Discontinuo::Rayas,
        "dashDot" | "lgDashDot" | "lgDashDotDot" | "sysDashDot" | "sysDashDotDot" => {
            Discontinuo::RayaPunto
        }
        _ => Discontinuo::Solida,
    });
    let c = color?;
    Some(Some(Trazo {
        color: c,
        grosor: grosor.unwrap_or(0.75).max(0.25),
        discontinuo: discontinuo.unwrap_or_default(),
    }))
}

/// Los efectos de un `a:effectLst` (o de lo que tenga los efectos como
/// hijos). `ph` sustituye a `phClr` (los del tema) y `giro` es el de la
/// forma: con `rotWithShape` (lo de por defecto en el estandar) el
/// desplazamiento de la sombra gira con ella.
pub fn efectos(lst: &Nodo, ctx: Contexto, ph: Option<Color>, giro: f32) -> Vec<Efecto> {
    let mut v = Vec::new();
    for e in lst.elementos() {
        let color = || tema::color_hijo(e, ctx.tema, ctx.mapa, ph).unwrap_or(Color::NEGRO);
        let pt = |k: &str| attr_num(e, k, 1.0).map(emu).unwrap_or(0.0).max(0.0);
        let grados = |k: &str| attr_num(e, k, 1.0).unwrap_or(0.0) / 60000.0;
        let fraccion = |k: &str| attr_num(e, k, 1.0).map(|x| x / 100000.0).unwrap_or(1.0);
        let desplazamiento = || {
            let (dx, dy) = Efecto::desplazamiento(pt("dist"), grados("dir"));
            if giro != 0.0 && !matches!(e.atributo("rotWithShape"), "0" | "false") {
                let (s, c) = giro.to_radians().sin_cos();
                (dx * c - dy * s, dx * s + dy * c)
            } else {
                (dx, dy)
            }
        };
        match e.nombre.as_str() {
            "outerShdw" | "prstShdw" => {
                let (dx, dy) = desplazamiento();
                let ancla = match e.atributo("algn") {
                    "tl" => (0.0, 0.0),
                    "t" => (0.5, 0.0),
                    "tr" => (1.0, 0.0),
                    "l" => (0.0, 0.5),
                    "ctr" => (0.5, 0.5),
                    "r" => (1.0, 0.5),
                    "bl" => (0.0, 1.0),
                    "br" => (1.0, 1.0),
                    _ => (0.5, 1.0),
                };
                // Las predefinidas (`shdw1`...) se aproximan a una sombra
                // sin desenfoque con su distancia y direccion.
                let preset = e.nombre == "prstShdw";
                v.push(Efecto::SombraExterior {
                    color: color(),
                    desenfoque: if preset { 0.0 } else { pt("blurRad") },
                    dx,
                    dy,
                    escala: if preset {
                        (1.0, 1.0)
                    } else {
                        (fraccion("sx"), fraccion("sy"))
                    },
                    sesgo: if preset {
                        (0.0, 0.0)
                    } else {
                        (grados("kx"), grados("ky"))
                    },
                    ancla,
                });
            }
            "innerShdw" => {
                let (dx, dy) = desplazamiento();
                v.push(Efecto::SombraInterior {
                    color: color(),
                    desenfoque: pt("blurRad"),
                    dx,
                    dy,
                });
            }
            "glow" => {
                let radio = pt("rad");
                if radio > 0.0 {
                    v.push(Efecto::Brillo {
                        color: color(),
                        radio,
                    });
                }
            }
            "softEdge" => {
                let radio = pt("rad");
                if radio > 0.0 {
                    v.push(Efecto::BordeSuave { radio });
                }
            }
            _ => {}
        }
    }
    v
}

/// Los efectos de un `effectRef` del estilo de una forma: los del
/// `effectStyle` numero `idx` del tema (0 = ninguno).
pub fn efectos_ref(r: &Nodo, ctx: Contexto) -> Vec<Efecto> {
    let idx = r.atributo("idx").parse::<usize>().unwrap_or(0);
    if idx == 0 {
        return Vec::new();
    }
    let ph = tema::color_hijo(r, ctx.tema, ctx.mapa, None);
    ctx.tema
        .efectos
        .get(idx - 1)
        .and_then(|st| st.hijo("effectLst"))
        .map(|l| efectos(l, ctx, ph, 0.0))
        .unwrap_or_default()
}

/// Los efectos de una forma: los de su `spPr` si los dice (un
/// `effectLst` vacio o un `effectDag` = ninguno), si no los de su estilo
/// (`effectRef`), y si no tiene estilo los heredados (`spPr` de sus
/// marcadores, del mas lejano al mas cercano).
pub fn efectos_forma(
    sppr: Option<&Nodo>,
    estilo: Option<&Nodo>,
    heredados: &[&Nodo],
    ctx: Contexto,
    giro: f32,
) -> Vec<Efecto> {
    if let Some(s) = sppr {
        if let Some(l) = s.hijo("effectLst") {
            return efectos(l, ctx, None, giro);
        }
        if s.hijo("effectDag").is_some() {
            return Vec::new();
        }
    }
    if let Some(st) = estilo {
        return st
            .hijo("effectRef")
            .map(|r| efectos_ref(r, ctx))
            .unwrap_or_default();
    }
    heredados
        .iter()
        .rev()
        .find_map(|s| s.hijo("effectLst"))
        .map(|l| efectos(l, ctx, None, giro))
        .unwrap_or_default()
}

/// Lo que aporta `p:style`/`wps:style` (referencias al tema).
#[derive(Debug, Clone, Default)]
pub struct EstiloForma {
    pub relleno: Option<Relleno>,
    pub linea: Option<Trazo>,
    pub color_letra: Option<Color>,
}

pub fn estilo_forma(st: &Nodo, ctx: Contexto) -> EstiloForma {
    let mut e = EstiloForma::default();
    if let Some(f) = st.hijo("fillRef") {
        let idx = f.atributo("idx").parse::<usize>().unwrap_or(0);
        let ph = tema::color_hijo(f, ctx.tema, ctx.mapa, None);
        // idx 1..3 = fillStyleLst; 1001.. = bgFillStyleLst; 0 = sin relleno.
        let lista = if idx >= 1000 {
            &ctx.tema.rellenos_fondo
        } else {
            &ctx.tema.rellenos
        };
        let i = if idx >= 1000 {
            idx - 1001
        } else {
            idx.wrapping_sub(1)
        };
        e.relleno = if idx == 0 {
            None
        } else {
            match lista.get(i).and_then(|n| relleno_de(n, ctx, ph)) {
                Some(r) => r,
                None => ph.map(Relleno::Solido),
            }
        };
    }
    if let Some(l) = st.hijo("lnRef") {
        let idx = l.atributo("idx").parse::<usize>().unwrap_or(0);
        let ph = tema::color_hijo(l, ctx.tema, ctx.mapa, None);
        if idx > 0 {
            e.linea = match ctx.tema.lineas.get(idx - 1).and_then(|n| linea(n, ctx, ph)) {
                Some(t) => t,
                None => ph.map(|c| Trazo::solido(c, 0.75)),
            };
        }
    }
    if let Some(f) = st.hijo("fontRef") {
        e.color_letra = tema::color_hijo(f, ctx.tema, ctx.mapa, None);
    }
    e
}

/// Posicion y tamano de una `a:xfrm`, en puntos, y el giro y volteos.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Xfrm {
    pub x: f32,
    pub y: f32,
    pub ancho: f32,
    pub alto: f32,
    /// Grados en sentido horario.
    pub giro: f32,
    pub voltear_h: bool,
    pub voltear_v: bool,
    /// Para grupos: el sistema de coordenadas de los hijos.
    pub hijo_x: f32,
    pub hijo_y: f32,
    pub hijo_ancho: f32,
    pub hijo_alto: f32,
}

pub fn xfrm(n: &Nodo) -> Xfrm {
    let mut x = Xfrm {
        giro: attr_num(n, "rot", 1.0).unwrap_or(0.0) / 60000.0,
        voltear_h: matches!(n.atributo("flipH"), "1" | "true"),
        voltear_v: matches!(n.atributo("flipV"), "1" | "true"),
        ..Default::default()
    };
    if let Some(o) = n.hijo("off") {
        x.x = emu(attr_num(o, "x", 1.0).unwrap_or(0.0));
        x.y = emu(attr_num(o, "y", 1.0).unwrap_or(0.0));
    }
    if let Some(e) = n.hijo("ext") {
        x.ancho = emu(attr_num(e, "cx", 1.0).unwrap_or(0.0));
        x.alto = emu(attr_num(e, "cy", 1.0).unwrap_or(0.0));
    }
    if let Some(o) = n.hijo("chOff") {
        x.hijo_x = emu(attr_num(o, "x", 1.0).unwrap_or(0.0));
        x.hijo_y = emu(attr_num(o, "y", 1.0).unwrap_or(0.0));
    }
    if let Some(e) = n.hijo("chExt") {
        x.hijo_ancho = emu(attr_num(e, "cx", 1.0).unwrap_or(0.0));
        x.hijo_alto = emu(attr_num(e, "cy", 1.0).unwrap_or(0.0));
    }
    x
}

/// Las figuras de la geometria de un `spPr` a `w` x `h`, y si es una linea.
pub fn figuras(sppr: &Nodo, w: f32, h: f32) -> (Vec<Figura>, bool) {
    if let Some(p) = sppr.hijo("prstGeom") {
        let nombre = p.atributo("prst");
        let adj = geometria::ajustes(Some(p));
        return (
            geometria::predefinida(nombre, w, h, &adj),
            geometria::es_linea(nombre),
        );
    }
    if let Some(c) = sppr.hijo("custGeom") {
        let f = geometria::propia(c, w, h);
        let linea = !f.is_empty() && f.iter().all(|f| !f.cerrada);
        return (f, linea);
    }
    (geometria::predefinida("rect", w, h, &HashMap::new()), false)
}

/// Envuelve elementos dibujados en (0,0)-(w,h) para ponerlos en `caja` con
/// el giro y los volteos de la forma.
pub fn colocar(
    elementos: Vec<Elemento>,
    caja: Rect,
    giro: f32,
    voltear_h: bool,
    voltear_v: bool,
) -> Elemento {
    let (cx, cy) = (caja.ancho / 2.0, caja.alto / 2.0);
    let (s, c) = giro.to_radians().sin_cos();
    let fx = if voltear_h { -1.0 } else { 1.0 };
    let fy = if voltear_v { -1.0 } else { 1.0 };
    // p' = R * F * (p - centro) + centro + origen
    let a = c * fx;
    let b = s * fx;
    let cc = -s * fy;
    let d = c * fy;
    let e = caja.x + cx - (a * cx + cc * cy);
    let f = caja.y + cy - (b * cx + d * cy);
    Elemento::Grupo {
        transformada: [a, b, cc, d, e, f],
        recorte: None,
        elementos,
    }
}

/// La caja de recorte de una imagen (`a:srcRect`), en fracciones.
pub fn recorte(blip_fill: &Nodo) -> Option<[f32; 4]> {
    let s = blip_fill.hijo("srcRect")?;
    let v = |k: &str| attr_num(s, k, 1.0).unwrap_or(0.0) / 100000.0;
    let r = [v("l"), v("t"), v("r"), v("b")];
    if r.iter().all(|x| x.abs() < 1e-6) {
        None
    } else {
        Some(r)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_docs::xml::leer;

    #[test]
    fn un_degradado_se_lee_con_sus_paradas() {
        let n = leer(
            r#"<a:gradFill><a:gsLst><a:gs pos="100000"><a:srgbClr val="0000FF"/></a:gs><a:gs pos="0"><a:srgbClr val="FF0000"/></a:gs></a:gsLst><a:lin ang="5400000"/></a:gradFill>"#,
        )
        .unwrap();
        let t = Tema::default();
        let m = HashMap::new();
        let r = relleno_de(&n, Contexto { tema: &t, mapa: &m }, None)
            .unwrap()
            .unwrap();
        match r {
            Relleno::Lineal { paradas, angulo } => {
                assert_eq!(paradas[0].1, Color::rgb(255, 0, 0));
                assert_eq!(angulo, 90.0);
            }
            _ => panic!(),
        }
    }

    fn tema_con_efectos() -> Tema {
        Tema::leer(
            &leer(
                r#"<a:theme><a:themeElements><a:fmtScheme><a:effectStyleLst>
                <a:effectStyle><a:effectLst/></a:effectStyle>
                <a:effectStyle><a:effectLst><a:outerShdw blurRad="40000" dist="23000" dir="5400000" rotWithShape="0"><a:srgbClr val="000000"><a:alpha val="35000"/></a:srgbClr></a:outerShdw></a:effectLst></a:effectStyle>
                <a:effectStyle><a:effectLst><a:glow rad="63500"><a:schemeClr val="phClr"><a:alpha val="40000"/></a:schemeClr></a:glow></a:effectLst></a:effectStyle>
                </a:effectStyleLst></a:fmtScheme></a:themeElements></a:theme>"#,
            )
            .unwrap(),
        )
    }

    #[test]
    fn la_sombra_exterior_se_lee_con_su_desplazamiento() {
        let t = Tema::default();
        let m = HashMap::new();
        let ctx = Contexto { tema: &t, mapa: &m };
        let n = leer(
            r#"<a:effectLst><a:outerShdw blurRad="50800" dist="38100" dir="2700000" sx="90000" sy="-50000" kx="1200000" algn="tl" rotWithShape="0"><a:prstClr val="black"><a:alpha val="40000"/></a:prstClr></a:outerShdw></a:effectLst>"#,
        )
        .unwrap();
        let v = efectos(&n, ctx, None, 30.0);
        let [
            Efecto::SombraExterior {
                color,
                desenfoque,
                dx,
                dy,
                escala,
                sesgo,
                ancla,
            },
        ] = v.as_slice()
        else {
            panic!("{v:?}")
        };
        assert_eq!((color.r, color.a), (0, 102));
        assert!((desenfoque - 4.0).abs() < 1e-4);
        // 3 pt a 45 grados: hacia abajo a la derecha. Sin rotWithShape el
        // giro de la forma no lo cambia.
        let k = 3.0 / std::f32::consts::SQRT_2;
        assert!((dx - k).abs() < 1e-4 && (dy - k).abs() < 1e-4, "{dx} {dy}");
        assert_eq!(*escala, (0.9, -0.5));
        assert!((sesgo.0 - 20.0).abs() < 1e-4 && sesgo.1 == 0.0);
        assert_eq!(*ancla, (0.0, 0.0));
    }

    #[test]
    fn el_desplazamiento_sale_de_la_direccion_y_la_distancia() {
        let (dx, dy) = Efecto::desplazamiento(10.0, 90.0);
        assert!(dx.abs() < 1e-4 && (dy - 10.0).abs() < 1e-4);
        let (dx, dy) = Efecto::desplazamiento(10.0, 180.0);
        assert!((dx + 10.0).abs() < 1e-4 && dy.abs() < 1e-4);
        let (dx, dy) = Efecto::desplazamiento(0.0, 45.0);
        assert_eq!((dx, dy), (0.0, 0.0));
        // Con rotWithShape (el valor por defecto) el desplazamiento gira
        // con la forma: 0 grados y forma girada 90 = hacia abajo.
        let t = Tema::default();
        let m = HashMap::new();
        let n = leer(r#"<a:effectLst><a:outerShdw dist="127000" dir="0"/></a:effectLst>"#).unwrap();
        let v = efectos(&n, Contexto { tema: &t, mapa: &m }, None, 90.0);
        let [Efecto::SombraExterior { dx, dy, .. }] = v.as_slice() else {
            panic!()
        };
        assert!(dx.abs() < 1e-4 && (dy - 10.0).abs() < 1e-4, "{dx} {dy}");
    }

    #[test]
    fn brillo_borde_suave_y_sombra_interior() {
        let t = Tema::default();
        let m = HashMap::new();
        let n = leer(
            r#"<a:effectLst><a:glow rad="101600"><a:srgbClr val="FFC000"/></a:glow><a:innerShdw blurRad="63500" dist="50800" dir="13500000"/><a:softEdge rad="127000"/><a:glow rad="0"/><a:reflection/></a:effectLst>"#,
        )
        .unwrap();
        let v = efectos(&n, Contexto { tema: &t, mapa: &m }, None, 0.0);
        assert_eq!(v.len(), 3, "{v:?}");
        assert_eq!(
            v[0],
            Efecto::Brillo {
                color: Color::rgb(0xFF, 0xC0, 0),
                radio: 8.0
            }
        );
        let Efecto::SombraInterior {
            color,
            desenfoque,
            dx,
            dy,
        } = v[1]
        else {
            panic!()
        };
        assert_eq!(color, Color::NEGRO);
        assert!((desenfoque - 5.0).abs() < 1e-4 && dx < 0.0 && dy < 0.0);
        assert_eq!(v[2], Efecto::BordeSuave { radio: 10.0 });
    }

    #[test]
    fn los_efectos_se_heredan_del_tema_por_effectref() {
        let t = tema_con_efectos();
        let m = HashMap::new();
        let ctx = Contexto { tema: &t, mapa: &m };
        let estilo = leer(
            r#"<p:style><a:fillRef idx="1"><a:schemeClr val="accent1"/></a:fillRef><a:effectRef idx="2"><a:schemeClr val="accent1"/></a:effectRef></p:style>"#,
        )
        .unwrap();
        // Sin effectLst propio: los del tema.
        let sppr = leer(r#"<p:spPr><a:prstGeom prst="rect"/></p:spPr>"#).unwrap();
        let v = efectos_forma(Some(&sppr), Some(&estilo), &[], ctx, 0.0);
        assert!(
            matches!(v.as_slice(), [Efecto::SombraExterior { .. }]),
            "{v:?}"
        );
        // El color de la referencia sustituye a phClr.
        let brillo = leer(
            r#"<p:style><a:effectRef idx="3"><a:srgbClr val="FF0000"/></a:effectRef></p:style>"#,
        )
        .unwrap();
        let v = efectos_forma(None, Some(&brillo), &[], ctx, 0.0);
        let [Efecto::Brillo { color, .. }] = v.as_slice() else {
            panic!("{v:?}")
        };
        assert_eq!((color.r, color.g, color.a), (255, 0, 102));
        // Casos negativos: un effectLst vacio propio manda sobre el tema; el
        // idx 0 y el primer estilo (vacio) no ponen nada.
        let vacio = leer(r#"<p:spPr><a:effectLst/></p:spPr>"#).unwrap();
        assert!(efectos_forma(Some(&vacio), Some(&estilo), &[], ctx, 0.0).is_empty());
        let cero = leer(r#"<p:style><a:effectRef idx="0"/></p:style>"#).unwrap();
        assert!(efectos_forma(None, Some(&cero), &[], ctx, 0.0).is_empty());
        let uno = leer(r#"<p:style><a:effectRef idx="1"/></p:style>"#).unwrap();
        assert!(efectos_forma(None, Some(&uno), &[], ctx, 0.0).is_empty());
        // Sin estilo: lo heredado del marcador.
        let heredado =
            leer(r#"<p:spPr><a:effectLst><a:softEdge rad="12700"/></a:effectLst></p:spPr>"#)
                .unwrap();
        let v = efectos_forma(Some(&sppr), None, &[&heredado], ctx, 0.0);
        assert_eq!(v, vec![Efecto::BordeSuave { radio: 1.0 }]);
    }

    #[test]
    fn colocar_sin_giro_solo_traslada() {
        let e = colocar(vec![], Rect::nuevo(10.0, 20.0, 4.0, 2.0), 0.0, false, false);
        let Elemento::Grupo { transformada, .. } = e else {
            panic!()
        };
        assert_eq!(transformada, [1.0, 0.0, 0.0, 1.0, 10.0, 20.0]);
    }
}
