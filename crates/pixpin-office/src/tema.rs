//! **El tema** (`theme1.xml`): los doce colores del esquema, las letras de
//! titulos y de texto, y como se resuelve un color de DrawingML con sus
//! modificadores (`lumMod`, `lumOff`, `tint`, `shade`, `alpha`...).

use std::collections::HashMap;

use pixpin_docs::xml::Nodo;

use crate::pagina::Color;

#[derive(Debug, Clone, Default)]
pub struct Tema {
    /// `dk1`, `lt1`, `dk2`, `lt2`, `accent1`..`accent6`, `hlink`, `folHlink`.
    pub colores: HashMap<String, Color>,
    /// Letra de titulos (`majorFont/latin`).
    pub letra_titulos: String,
    /// Letra del texto (`minorFont/latin`).
    pub letra_texto: String,
    pub letra_titulos_ea: String,
    pub letra_texto_ea: String,
    /// Los rellenos de `fmtScheme/bgFillStyleLst` y `fillStyleLst`, tal
    /// cual, para los `bgRef`/`fillRef` con `idx`.
    pub rellenos: Vec<Nodo>,
    pub rellenos_fondo: Vec<Nodo>,
    pub lineas: Vec<Nodo>,
    /// Los `a:effectStyle` de `fmtScheme/effectStyleLst`, para los
    /// `effectRef` con `idx` (1 = el primero; 0 = sin efectos).
    pub efectos: Vec<Nodo>,
}

impl Tema {
    pub fn leer(raiz: &Nodo) -> Tema {
        let mut t = Tema::default();
        let Some(elems) = raiz.hijo("themeElements") else {
            return t.con_huecos_rellenos();
        };
        if let Some(esquema) = elems.hijo("clrScheme") {
            for c in esquema.elementos() {
                if let Some(color) = c.elementos().next().and_then(color_base) {
                    t.colores.insert(c.nombre.clone(), color);
                }
            }
        }
        if let Some(letras) = elems.hijo("fontScheme") {
            let tipo = |n: &str, que: &str| {
                letras
                    .hijo(n)
                    .and_then(|f| f.hijo(que))
                    .map(|l| l.atributo("typeface").to_string())
                    .unwrap_or_default()
            };
            t.letra_titulos = tipo("majorFont", "latin");
            t.letra_texto = tipo("minorFont", "latin");
            t.letra_titulos_ea = tipo("majorFont", "ea");
            t.letra_texto_ea = tipo("minorFont", "ea");
        }
        if let Some(fmt) = elems.hijo("fmtScheme") {
            if let Some(l) = fmt.hijo("fillStyleLst") {
                t.rellenos = l.elementos().cloned().collect();
            }
            if let Some(l) = fmt.hijo("bgFillStyleLst") {
                t.rellenos_fondo = l.elementos().cloned().collect();
            }
            if let Some(l) = fmt.hijo("lnStyleLst") {
                t.lineas = l.elementos().cloned().collect();
            }
            if let Some(l) = fmt.hijo("effectStyleLst") {
                t.efectos = l.elementos().cloned().collect();
            }
        }
        t.con_huecos_rellenos()
    }

    /// El tema de Office por defecto para lo que falte.
    fn con_huecos_rellenos(mut self) -> Tema {
        for (k, v) in [
            ("dk1", "000000"),
            ("lt1", "FFFFFF"),
            ("dk2", "44546A"),
            ("lt2", "E7E6E6"),
            ("accent1", "4472C4"),
            ("accent2", "ED7D31"),
            ("accent3", "A5A5A5"),
            ("accent4", "FFC000"),
            ("accent5", "5B9BD5"),
            ("accent6", "70AD47"),
            ("hlink", "0563C1"),
            ("folHlink", "954F72"),
        ] {
            self.colores
                .entry(k.into())
                .or_insert_with(|| Color::hex(v).unwrap_or(Color::NEGRO));
        }
        if self.letra_titulos.is_empty() {
            self.letra_titulos = "Calibri Light".into();
        }
        if self.letra_texto.is_empty() {
            self.letra_texto = "Calibri".into();
        }
        self
    }

    /// Un color del esquema por nombre, pasando por el mapa de colores
    /// (`bg1` → `lt1`...). `mapa` es el `clrMap` del patron; vacio usa el
    /// de siempre.
    pub fn esquema(&self, nombre: &str, mapa: &HashMap<String, String>) -> Option<Color> {
        let nombre = mapa.get(nombre).map(|s| s.as_str()).unwrap_or(nombre);
        let nombre = match nombre {
            "bg1" | "background1" | "light1" => "lt1",
            "tx1" | "text1" | "dark1" => "dk1",
            "bg2" | "background2" | "light2" => "lt2",
            "tx2" | "text2" | "dark2" => "dk2",
            "hyperlink" => "hlink",
            "followedHyperlink" => "folHlink",
            otro => otro,
        };
        self.colores.get(nombre).copied()
    }

    /// Una referencia a letra del tema (`+mj-lt`, `+mn-ea`...) a su nombre.
    pub fn letra(&self, typeface: &str) -> String {
        match typeface {
            "+mj-lt" => self.letra_titulos.clone(),
            "+mn-lt" => self.letra_texto.clone(),
            "+mj-ea" => self.letra_titulos_ea.clone(),
            "+mn-ea" => self.letra_texto_ea.clone(),
            "+mj-cs" => self.letra_titulos.clone(),
            "+mn-cs" => self.letra_texto.clone(),
            otro => otro.to_string(),
        }
    }
}

/// El color de un elemento de color de DrawingML sin el esquema (srgbClr,
/// sysClr, prstClr, scrgbClr, hslClr), con sus modificadores.
fn color_base(n: &Nodo) -> Option<Color> {
    resolver(n, &Tema::default(), &HashMap::new(), None)
}

/// Resuelve un elemento de color de DrawingML (`a:srgbClr`, `a:schemeClr`...)
/// con sus modificadores. `ph` es el color que sustituye a `phClr` (el de
/// la forma que usa un estilo del tema).
pub fn resolver(
    n: &Nodo,
    tema: &Tema,
    mapa: &HashMap<String, String>,
    ph: Option<Color>,
) -> Option<Color> {
    let base = match n.nombre.as_str() {
        "srgbClr" => Color::hex(n.valor())?,
        "sysClr" => Color::hex(n.atributo("lastClr")).or_else(|| match n.valor() {
            "window" => Some(Color::BLANCO),
            _ => Some(Color::NEGRO),
        })?,
        "schemeClr" => {
            if n.valor() == "phClr" {
                ph.unwrap_or(Color::NEGRO)
            } else {
                tema.esquema(n.valor(), mapa)?
            }
        }
        "prstClr" => color_con_nombre(n.valor())?,
        "scrgbClr" => {
            let c = |k: &str| {
                let v = crate::paquete::numero(n.atributo(k), 1.0).unwrap_or(0.0) / 100000.0;
                (lineal_a_srgb(v.clamp(0.0, 1.0)) * 255.0).round() as u8
            };
            Color::rgb(c("r"), c("g"), c("b"))
        }
        "hslClr" => {
            let h = crate::paquete::numero(n.atributo("hue"), 1.0).unwrap_or(0.0) / 60000.0 / 360.0;
            let s = crate::paquete::numero(n.atributo("sat"), 1.0).unwrap_or(0.0) / 100000.0;
            let l = crate::paquete::numero(n.atributo("lum"), 1.0).unwrap_or(0.0) / 100000.0;
            let (r, g, b) = hsl_a_rgb(h, s, l);
            Color::rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
        }
        _ => return None,
    };
    Some(modificar(base, n))
}

/// Busca el primer color que cuelgue de `padre` (por ejemplo el de un
/// `a:solidFill`) y lo resuelve.
pub fn color_hijo(
    padre: &Nodo,
    tema: &Tema,
    mapa: &HashMap<String, String>,
    ph: Option<Color>,
) -> Option<Color> {
    padre.elementos().find_map(|e| resolver(e, tema, mapa, ph))
}

/// Aplica los modificadores hijos de un elemento de color.
pub fn modificar(c: Color, n: &Nodo) -> Color {
    let mut r = c.r as f32 / 255.0;
    let mut g = c.g as f32 / 255.0;
    let mut b = c.b as f32 / 255.0;
    let mut a = c.a as f32 / 255.0;
    for m in n.elementos() {
        let v = crate::paquete::numero(m.valor(), 1.0).unwrap_or(0.0) / 100000.0;
        match m.nombre.as_str() {
            "lumMod" | "lumOff" | "satMod" | "satOff" | "hueMod" | "hueOff" | "sat" | "lum"
            | "hue" => {
                let (mut h, mut s, mut l) = rgb_a_hsl(r, g, b);
                match m.nombre.as_str() {
                    "lumMod" => l *= v,
                    "lumOff" => l += v,
                    "lum" => l = v,
                    "satMod" => s *= v,
                    "satOff" => s += v,
                    "sat" => s = v,
                    "hueMod" => h *= v,
                    // `hueOff` y `hue` vienen en 60000 por grado.
                    "hueOff" => h += v * 100000.0 / 60000.0 / 360.0,
                    "hue" => h = v * 100000.0 / 60000.0 / 360.0,
                    _ => {}
                }
                let (nr, ng, nb) =
                    hsl_a_rgb(h.rem_euclid(1.0), s.clamp(0.0, 1.0), l.clamp(0.0, 1.0));
                (r, g, b) = (nr, ng, nb);
            }
            "tint" => {
                // Hacia el blanco, en RGB lineal.
                let f = |c: f32| lineal_a_srgb(srgb_a_lineal(c) * v + (1.0 - v));
                (r, g, b) = (f(r), f(g), f(b));
            }
            "shade" => {
                let f = |c: f32| lineal_a_srgb(srgb_a_lineal(c) * v);
                (r, g, b) = (f(r), f(g), f(b));
            }
            "alpha" => a = v,
            "alphaMod" => a *= v,
            "alphaOff" => a += v,
            "inv" => (r, g, b) = (1.0 - r, 1.0 - g, 1.0 - b),
            "gray" => {
                let y = 0.3 * r + 0.59 * g + 0.11 * b;
                (r, g, b) = (y, y, y);
            }
            "comp" => {
                let (h, s, l) = rgb_a_hsl(r, g, b);
                (r, g, b) = hsl_a_rgb((h + 0.5).rem_euclid(1.0), s, l);
            }
            _ => {}
        }
    }
    let q = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color {
        r: q(r),
        g: q(g),
        b: q(b),
        a: q(a),
    }
}

/// El `themeTint`/`themeShade` de Word (00..FF) sobre un color: en HSL, como
/// los «mas claro 40 %» y «mas oscuro 25 %» de la paleta.
pub fn tinte_word(c: Color, tint: Option<f32>, shade: Option<f32>) -> Color {
    let (h, s, mut l) = rgb_a_hsl(c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0);
    if let Some(t) = tint {
        l = l * t + (1.0 - t);
    }
    if let Some(sh) = shade {
        l *= sh;
    }
    let (r, g, b) = hsl_a_rgb(h, s, l.clamp(0.0, 1.0));
    let q = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color {
        r: q(r),
        g: q(g),
        b: q(b),
        a: c.a,
    }
}

fn srgb_a_lineal(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn lineal_a_srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

pub fn rgb_a_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < 1e-6 {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h / 6.0, s, l)
}

pub fn hsl_a_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s <= 0.0 {
        return (l, l, l);
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let f = |mut t: f32| {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    (f(h + 1.0 / 3.0), f(h), f(h - 1.0 / 3.0))
}

/// Los colores con nombre de DrawingML y de Word (`w:highlight`).
pub fn color_con_nombre(n: &str) -> Option<Color> {
    let hex = match n {
        "black" => "000000",
        "white" => "FFFFFF",
        "red" => "FF0000",
        "green" => "00FF00",
        "blue" => "0000FF",
        "yellow" => "FFFF00",
        "cyan" => "00FFFF",
        "magenta" => "FF00FF",
        "darkBlue" | "dkBlue" => "00008B",
        "darkCyan" | "dkCyan" => "008B8B",
        "darkGreen" | "dkGreen" => "006400",
        "darkMagenta" | "dkMagenta" => "8B008B",
        "darkRed" | "dkRed" => "8B0000",
        "darkYellow" => "808000",
        "darkGray" | "dkGray" => "A9A9A9",
        "lightGray" | "ltGray" => "D3D3D3",
        "gray" => "808080",
        "orange" => "FFA500",
        "navy" => "000080",
        "purple" => "800080",
        "silver" => "C0C0C0",
        "lime" => "00FF00",
        "teal" => "008080",
        "olive" => "808000",
        "maroon" => "800000",
        _ => return None,
    };
    Color::hex(hex)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_docs::xml::leer;

    #[test]
    fn un_color_del_esquema_con_lummod_se_oscurece() {
        let tema = Tema::leer(&leer(r#"<a:theme><a:themeElements><a:clrScheme><a:accent1><a:srgbClr val="4472C4"/></a:accent1></a:clrScheme></a:themeElements></a:theme>"#).unwrap());
        let n =
            leer(r#"<a:schemeClr val="accent1"><a:lumMod val="50000"/></a:schemeClr>"#).unwrap();
        let c = resolver(&n, &tema, &HashMap::new(), None).unwrap();
        // El acento a la mitad de luminosidad: mas oscuro que el original.
        assert!(c.r < 0x44 && c.b < 0xC4, "{c:?}");
        let tx = leer(r#"<a:schemeClr val="tx1"/>"#).unwrap();
        assert_eq!(
            resolver(&tx, &tema, &HashMap::new(), None),
            Some(Color::NEGRO)
        );
    }

    #[test]
    fn el_alfa_se_aplica() {
        let n = leer(r#"<a:srgbClr val="FF0000"><a:alpha val="50000"/></a:srgbClr>"#).unwrap();
        let c = resolver(&n, &Tema::default(), &HashMap::new(), None).unwrap();
        assert_eq!((c.r, c.a), (255, 128));
    }

    #[test]
    fn las_letras_del_tema_se_resuelven() {
        let tema = Tema::leer(&leer(r#"<a:theme><a:themeElements><a:fontScheme><a:majorFont><a:latin typeface="Aptos Display"/></a:majorFont><a:minorFont><a:latin typeface="Aptos"/></a:minorFont></a:fontScheme></a:themeElements></a:theme>"#).unwrap());
        assert_eq!(tema.letra("+mn-lt"), "Aptos");
        assert_eq!(tema.letra("+mj-lt"), "Aptos Display");
        assert_eq!(tema.letra("Arial"), "Arial");
    }
}
