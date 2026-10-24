//! `styles.xml` y el tema: fuentes, rellenos, bordes, alineacion y formatos
//! de numero de cada estilo de celda, con los colores ya resueltos (tema +
//! matiz, indexados, ARGB).

use std::collections::HashMap;
use std::sync::Arc;

use crate::formato::{self, Formato, Formatos, Regional};
use crate::xml::{self, Nodo};

/// Un color ya resuelto, RGB opaco (Excel no pinta celdas translucidas).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const NEGRO: Rgb = Rgb { r: 0, g: 0, b: 0 };
    pub const BLANCO: Rgb = Rgb {
        r: 255,
        g: 255,
        b: 255,
    };

    pub fn de_u32(c: u32) -> Rgb {
        Rgb {
            r: (c >> 16) as u8,
            g: (c >> 8) as u8,
            b: c as u8,
        }
    }

    /// Mezcla `self` sobre `fondo` con `t` (0 = fondo, 1 = self).
    pub fn mezcla(self, fondo: Rgb, t: f32) -> Rgb {
        let m = |a: u8, b: u8| {
            (a as f32 * t + b as f32 * (1.0 - t))
                .round()
                .clamp(0.0, 255.0) as u8
        };
        Rgb {
            r: m(self.r, fondo.r),
            g: m(self.g, fondo.g),
            b: m(self.b, fondo.b),
        }
    }
}

/// La paleta indexada por defecto (indices 0-65; 64 y 65 son el texto y el
/// fondo del sistema). Son los 56 colores clasicos de Excel, precedidos de
/// los 8 basicos.
pub const PALETA: [u32; 66] = [
    0x000000, 0xFFFFFF, 0xFF0000, 0x00FF00, 0x0000FF, 0xFFFF00, 0xFF00FF, 0x00FFFF, // 0-7
    0x000000, 0xFFFFFF, 0xFF0000, 0x00FF00, 0x0000FF, 0xFFFF00, 0xFF00FF, 0x00FFFF, // 8-15
    0x800000, 0x008000, 0x000080, 0x808000, 0x800080, 0x008080, 0xC0C0C0, 0x808080, // 16-23
    0x9999FF, 0x993366, 0xFFFFCC, 0xCCFFFF, 0x660066, 0xFF8080, 0x0066CC, 0xCCCCFF, // 24-31
    0x000080, 0xFF00FF, 0xFFFF00, 0x00FFFF, 0x800080, 0x800000, 0x008080, 0x0000FF, // 32-39
    0x00CCFF, 0xCCFFFF, 0xCCFFCC, 0xFFFF99, 0x99CCFF, 0xFF99CC, 0xCC99FF, 0xFFCC99, // 40-47
    0x3366FF, 0x33CCCC, 0x99CC00, 0xFFCC00, 0xFF9900, 0xFF6600, 0x666699, 0x969696, // 48-55
    0x003366, 0x339966, 0x003300, 0x333300, 0x993300, 0x993366, 0x333399, 0x333333, // 56-63
    0x000000, 0xFFFFFF, // 64-65
];

/// El tema: los doce colores y las dos fuentes.
#[derive(Debug, Clone)]
pub struct Tema {
    /// En el orden de los indices de SpreadsheetML: lt1, dk1, lt2, dk2,
    /// accent1-6, hlink, folHlink (los dos primeros pares van cambiados
    /// respecto al orden del XML del tema).
    pub colores: [Rgb; 12],
    pub fuente_cuerpo: String,
    pub fuente_titulos: String,
}

impl Default for Tema {
    fn default() -> Self {
        // Office 2013-2022.
        let c = [
            0xFFFFFF, 0x000000, 0xE7E6E6, 0x44546A, 0x4472C4, 0xED7D31, 0xA5A5A5, 0xFFC000,
            0x5B9BD5, 0x70AD47, 0x0563C1, 0x954F72,
        ];
        Tema {
            colores: c.map(Rgb::de_u32),
            fuente_cuerpo: "Calibri".into(),
            fuente_titulos: "Calibri Light".into(),
        }
    }
}

impl Tema {
    pub fn leer(bytes: &[u8]) -> Tema {
        let raiz = xml::arbol(bytes);
        let mut t = Tema::default();
        if let Some(esq) = raiz.buscar("clrScheme") {
            let color = |n: &str| -> Option<Rgb> {
                let nodo = esq.hijo(n)?;
                if let Some(s) = nodo.hijo("srgbClr") {
                    return hex(s.attr("val")?);
                }
                if let Some(s) = nodo.hijo("sysClr") {
                    return hex(s
                        .attr("lastClr")
                        .unwrap_or(if s.attr("val") == Some("window") {
                            "FFFFFF"
                        } else {
                            "000000"
                        }));
                }
                None
            };
            let nombres = [
                "lt1", "dk1", "lt2", "dk2", "accent1", "accent2", "accent3", "accent4", "accent5",
                "accent6", "hlink", "folHlink",
            ];
            for (i, n) in nombres.iter().enumerate() {
                if let Some(c) = color(n) {
                    t.colores[i] = c;
                }
            }
        }
        if let Some(f) = raiz
            .buscar("minorFont")
            .and_then(|n| n.hijo("latin"))
            .and_then(|n| n.attr("typeface"))
        {
            if !f.is_empty() {
                t.fuente_cuerpo = f.to_string();
            }
        }
        if let Some(f) = raiz
            .buscar("majorFont")
            .and_then(|n| n.hijo("latin"))
            .and_then(|n| n.attr("typeface"))
        {
            if !f.is_empty() {
                t.fuente_titulos = f.to_string();
            }
        }
        t
    }
}

fn hex(s: &str) -> Option<Rgb> {
    let s = s.trim();
    let s = if s.len() == 8 { &s[2..] } else { s };
    if s.len() != 6 {
        return None;
    }
    u32::from_str_radix(s, 16).ok().map(Rgb::de_u32)
}

/// Aplica un matiz (`tint`, -1..1) como lo describe ECMA-376: sobre la
/// luminosidad en HLS.
pub fn con_matiz(c: Rgb, tint: f64) -> Rgb {
    if tint == 0.0 {
        return c;
    }
    let (h, l, s) = a_hls(c);
    let l = if tint < 0.0 {
        l * (1.0 + tint)
    } else {
        l * (1.0 - tint) + tint
    };
    de_hls(h, l.clamp(0.0, 1.0), s)
}

fn a_hls(c: Rgb) -> (f64, f64, f64) {
    let r = c.r as f64 / 255.0;
    let g = c.g as f64 / 255.0;
    let b = c.b as f64 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if max == min {
        return (0.0, l, 0.0);
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
    } / 6.0;
    (h, l, s)
}

fn de_hls(h: f64, l: f64, s: f64) -> Rgb {
    if s == 0.0 {
        let v = (l * 255.0).round() as u8;
        return Rgb { r: v, g: v, b: v };
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let f = |mut t: f64| {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        let v = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        (v * 255.0).round().clamp(0.0, 255.0) as u8
    };
    Rgb {
        r: f(h + 1.0 / 3.0),
        g: f(h),
        b: f(h - 1.0 / 3.0),
    }
}

/// Resuelve un `<color>` (o `<fgColor>`, ...). `None` es automatico.
pub fn color_de(n: &Nodo, tema: &Tema, paleta: &[u32]) -> Option<Rgb> {
    let tint: f64 = n.attr("tint").and_then(|t| t.parse().ok()).unwrap_or(0.0);
    let base = if let Some(v) = n.attr("rgb") {
        hex(v)?
    } else if let Some(t) = n.attr("theme") {
        *tema.colores.get(t.parse::<usize>().ok()?)?
    } else {
        let i: usize = n.attr("indexed")?.parse().ok()?;
        if i == 64 || i == 65 {
            // Texto y fondo del sistema: lo mismo que automatico.
            return None;
        }
        Rgb::de_u32(*paleta.get(i)?)
    };
    Some(con_matiz(base, tint))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Subrayado {
    #[default]
    Ninguno,
    Simple,
    Doble,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Fuente {
    pub nombre: Arc<str>,
    /// Puntos.
    pub tam: f32,
    pub negrita: bool,
    pub cursiva: bool,
    pub subrayado: Subrayado,
    pub tachado: bool,
    /// `None`: automatico (negro).
    pub color: Option<Rgb>,
    /// Superindice (1) o subindice (-1).
    pub vertical: i8,
}

impl Default for Fuente {
    fn default() -> Self {
        Fuente {
            nombre: Arc::from("Calibri"),
            tam: 11.0,
            negrita: false,
            cursiva: false,
            subrayado: Subrayado::Ninguno,
            tachado: false,
            color: None,
            vertical: 0,
        }
    }
}

/// Lee una fuente (`<font>` o el `<rPr>` de un tramo de texto rico).
pub fn fuente_de(n: &Nodo, tema: &Tema, paleta: &[u32], base: &Fuente) -> Fuente {
    let mut f = base.clone();
    let val_bool = |h: &Nodo| h.attr("val").is_none_or(|v| v != "0" && v != "false");
    for h in &n.hijos {
        match h.nombre.as_str() {
            "name" | "rFont" => {
                if let Some(v) = h.attr("val") {
                    f.nombre = Arc::from(v);
                }
            }
            "sz" => {
                if let Some(v) = h.attr("val").and_then(|v| v.parse::<f32>().ok()) {
                    if v > 0.0 && v < 1000.0 {
                        f.tam = v;
                    }
                }
            }
            "b" => f.negrita = val_bool(h),
            "i" => f.cursiva = val_bool(h),
            "strike" => f.tachado = val_bool(h),
            "u" => {
                f.subrayado = match h.attr("val") {
                    Some("double") | Some("doubleAccounting") => Subrayado::Doble,
                    Some("none") => Subrayado::Ninguno,
                    _ => Subrayado::Simple,
                }
            }
            "color" => f.color = color_de(h, tema, paleta),
            "vertAlign" => {
                f.vertical = match h.attr("val") {
                    Some("superscript") => 1,
                    Some("subscript") => -1,
                    _ => 0,
                }
            }
            "scheme" => match h.attr("val") {
                Some("minor") => f.nombre = Arc::from(tema.fuente_cuerpo.as_str()),
                Some("major") => f.nombre = Arc::from(tema.fuente_titulos.as_str()),
                _ => {}
            },
            _ => {}
        }
    }
    f
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Relleno {
    #[default]
    Nada,
    /// Liso (o una trama ya mezclada a su gris medio).
    Liso(Rgb),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub enum EstiloLinea {
    #[default]
    Ninguno,
    Pelo,
    Punteado,
    RayaPuntoPunto,
    RayaPunto,
    Discontinuo,
    Fino,
    MedioRayaPuntoPunto,
    RayaPuntoInclinada,
    MedioRayaPunto,
    MedioDiscontinuo,
    Medio,
    Doble,
    Grueso,
}

impl EstiloLinea {
    fn de(s: &str) -> EstiloLinea {
        match s {
            "hair" => Self::Pelo,
            "dotted" => Self::Punteado,
            "dashDotDot" => Self::RayaPuntoPunto,
            "dashDot" => Self::RayaPunto,
            "dashed" => Self::Discontinuo,
            "thin" => Self::Fino,
            "mediumDashDotDot" => Self::MedioRayaPuntoPunto,
            "slantDashDot" => Self::RayaPuntoInclinada,
            "mediumDashDot" => Self::MedioRayaPunto,
            "mediumDashed" => Self::MedioDiscontinuo,
            "medium" => Self::Medio,
            "double" => Self::Doble,
            "thick" => Self::Grueso,
            _ => Self::Ninguno,
        }
    }

    /// Grosor en pixeles al 100 %.
    pub fn grosor(self) -> f32 {
        match self {
            Self::Ninguno => 0.0,
            Self::Medio
            | Self::MedioDiscontinuo
            | Self::MedioRayaPunto
            | Self::MedioRayaPuntoPunto
            | Self::RayaPuntoInclinada => 2.0,
            Self::Grueso => 3.0,
            Self::Doble => 3.0,
            _ => 1.0,
        }
    }
}

/// Una linea de borde. El orden (`Ord`) es la prioridad cuando dos celdas
/// vecinas piden lineas distintas en el mismo lado: gana la mas fuerte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Linea {
    pub estilo: EstiloLinea,
    pub color: Rgb,
}

impl PartialOrd for Linea {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}

impl Ord for Linea {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        // A igual estilo, la mas oscura (como Excel en la practica).
        let lum = |c: Rgb| -(c.r as i32 * 3 + c.g as i32 * 6 + c.b as i32);
        self.estilo
            .cmp(&o.estilo)
            .then(lum(self.color).cmp(&lum(o.color)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Borde {
    pub izquierda: Linea,
    pub derecha: Linea,
    pub arriba: Linea,
    pub abajo: Linea,
    pub diagonal: Linea,
    pub diagonal_sube: bool,
    pub diagonal_baja: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Horizontal {
    #[default]
    General,
    Izquierda,
    Centro,
    Derecha,
    Rellenar,
    Justificar,
    CentroSeleccion,
    Distribuido,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Vertical {
    Arriba,
    Centro,
    #[default]
    Abajo,
    Justificar,
    Distribuido,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Alineacion {
    pub horizontal: Horizontal,
    pub vertical: Vertical,
    pub ajustar: bool,
    pub encoger: bool,
    pub sangria: u8,
    /// Grados 0-180 como en el XML (91-180 son -1..-90), o 255 (apilado).
    pub giro: u16,
}

impl Alineacion {
    /// El giro en grados contrarreloj (-90..90), o `None` si es apilado.
    pub fn grados(&self) -> Option<f32> {
        match self.giro {
            255 => None,
            g if g <= 90 => Some(g as f32),
            g if g <= 180 => Some(-((g - 90) as f32)),
            _ => Some(0.0),
        }
    }
}

/// Un estilo de celda (`<xf>` de `cellXfs`).
#[derive(Debug, Clone)]
pub struct Xf {
    pub formato: Arc<Formato>,
    pub fuente: u32,
    pub relleno: u32,
    pub borde: u32,
    pub alineacion: Alineacion,
    /// `quotePrefix`: el numero guardado como texto se ve a la izquierda.
    pub comilla: bool,
}

/// Todo `styles.xml` ya resuelto.
pub struct Estilos {
    pub fuentes: Vec<Fuente>,
    pub rellenos: Vec<Relleno>,
    pub bordes: Vec<Borde>,
    pub xfs: Vec<Xf>,
    pub paleta: Vec<u32>,
    /// La fuente del estilo «Normal»: decide el ancho de las columnas.
    pub fuente_normal: Fuente,
    /// Por xf: cuantos puntos mide su fuente (para la altura automatica de
    /// las filas sin `ht`).
    pub tam_por_xf: Vec<f32>,
    pub formatos: Formatos,
    /// Formatos diferenciales (`<dxfs>`): los de formato condicional y los
    /// de los estilos de tabla.
    pub dxfs: Vec<Dxf>,
    /// Estilos de tabla propios del libro (`<tableStyles>`), por nombre.
    pub estilos_tabla: HashMap<String, EstiloTabla>,
}

/// Un formato diferencial: solo lo que cambia sobre lo que ya tiene la celda.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Dxf {
    pub color_fuente: Option<Rgb>,
    pub negrita: Option<bool>,
    pub cursiva: Option<bool>,
    pub tachado: Option<bool>,
    pub subrayado: Option<bool>,
    pub relleno: Option<Rgb>,
    pub izquierda: Option<Linea>,
    pub derecha: Option<Linea>,
    pub arriba: Option<Linea>,
    pub abajo: Option<Linea>,
    /// Lineas de dentro (entre filas y entre columnas) de una tabla.
    pub horizontal: Option<Linea>,
    pub vertical: Option<Linea>,
}

impl Dxf {
    /// Pone encima lo que traiga `o`.
    pub fn sobre(&mut self, o: &Dxf) {
        macro_rules! pisa {
            ($($c:ident),*) => {$(if o.$c.is_some() { self.$c = o.$c; })*};
        }
        pisa!(
            color_fuente,
            negrita,
            cursiva,
            tachado,
            subrayado,
            relleno,
            izquierda,
            derecha,
            arriba,
            abajo,
            horizontal,
            vertical
        );
    }
}

/// Un estilo de tabla: el dxf de cada parte.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EstiloTabla {
    pub todo: Option<Dxf>,
    pub encabezado: Option<Dxf>,
    pub total: Option<Dxf>,
    pub primera_columna: Option<Dxf>,
    pub ultima_columna: Option<Dxf>,
    pub banda_fila: Option<Dxf>,
    pub banda_fila2: Option<Dxf>,
    pub banda_columna: Option<Dxf>,
    pub banda_columna2: Option<Dxf>,
    pub alto_banda: u32,
}

fn dxf_de(n: &Nodo, tema: &Tema, paleta: &[u32]) -> Dxf {
    let mut d = Dxf::default();
    if let Some(f) = n.hijo("font") {
        let si = |h: &Nodo| h.attr("val").is_none_or(|v| v != "0" && v != "false");
        for h in &f.hijos {
            match h.nombre.as_str() {
                "b" => d.negrita = Some(si(h)),
                "i" => d.cursiva = Some(si(h)),
                "strike" => d.tachado = Some(si(h)),
                "u" => d.subrayado = Some(h.attr("val") != Some("none")),
                "color" => d.color_fuente = color_de(h, tema, paleta),
                _ => {}
            }
        }
    }
    if let Some(p) = n.hijo("fill").and_then(|f| f.hijo("patternFill")) {
        // En un dxf el color del relleno liso va en `bgColor` (al reves que
        // en los rellenos de celda); algunos generadores lo ponen en fgColor.
        let tipo = p.attr("patternType");
        let bg = p.hijo("bgColor").and_then(|c| color_de(c, tema, paleta));
        let fg = p.hijo("fgColor").and_then(|c| color_de(c, tema, paleta));
        d.relleno = match tipo {
            Some("none") => None,
            Some("solid") if fg.is_some() && bg.is_none() => fg,
            _ => bg.or(fg),
        };
    }
    if let Some(b) = n.hijo("border") {
        let linea = |k: &str| -> Option<Linea> {
            let h = b.hijo(k)?;
            let estilo = EstiloLinea::de(h.attr("style")?);
            Some(Linea {
                estilo,
                color: h
                    .hijo("color")
                    .and_then(|c| color_de(c, tema, paleta))
                    .unwrap_or(Rgb::NEGRO),
            })
        };
        d.izquierda = linea("left").or_else(|| linea("start"));
        d.derecha = linea("right").or_else(|| linea("end"));
        d.arriba = linea("top");
        d.abajo = linea("bottom");
        d.horizontal = linea("horizontal");
        d.vertical = linea("vertical");
    }
    d
}

impl Estilos {
    /// Los estilos de un libro sin `styles.xml`: todo por defecto.
    pub fn por_defecto(tema: &Tema, regional: &Regional) -> Estilos {
        Self::leer(b"", tema, regional)
    }

    pub fn leer(bytes: &[u8], tema: &Tema, regional: &Regional) -> Estilos {
        let raiz = xml::arbol(bytes);
        let hoja = raiz.hijo("styleSheet").cloned().unwrap_or_default();
        // Paleta propia (`<colors><indexedColors>`).
        let mut paleta: Vec<u32> = PALETA.to_vec();
        if let Some(ic) = hoja.hijo("colors").and_then(|c| c.hijo("indexedColors")) {
            for (i, c) in ic.hijos_de("rgbColor").enumerate() {
                if let (Some(slot), Some(rgb)) = (paleta.get_mut(i), c.attr("rgb").and_then(hex)) {
                    *slot = (rgb.r as u32) << 16 | (rgb.g as u32) << 8 | rgb.b as u32;
                }
            }
        }
        let mut codigos: HashMap<u32, String> = HashMap::new();
        if let Some(nf) = hoja.hijo("numFmts") {
            for n in nf.hijos_de("numFmt") {
                if let (Some(id), Some(c)) = (
                    n.attr("numFmtId").and_then(|v| v.parse().ok()),
                    n.attr("formatCode"),
                ) {
                    codigos.insert(id, c.to_string());
                }
            }
        }
        let base = Fuente::default();
        let mut fuentes: Vec<Fuente> = hoja
            .hijo("fonts")
            .map(|f| {
                f.hijos_de("font")
                    .map(|n| fuente_de(n, tema, &paleta, &base))
                    .collect()
            })
            .unwrap_or_default();
        if fuentes.is_empty() {
            fuentes.push(Fuente {
                nombre: Arc::from(tema.fuente_cuerpo.as_str()),
                ..Fuente::default()
            });
        }
        let rellenos: Vec<Relleno> = hoja
            .hijo("fills")
            .map(|f| {
                f.hijos_de("fill")
                    .map(|n| relleno_de(n, tema, &paleta))
                    .collect()
            })
            .unwrap_or_default();
        let bordes: Vec<Borde> = hoja
            .hijo("borders")
            .map(|f| {
                f.hijos_de("border")
                    .map(|n| borde_de(n, tema, &paleta))
                    .collect()
            })
            .unwrap_or_default();
        let mut formatos = Formatos::default();
        let mut xfs = Vec::new();
        if let Some(cx) = hoja.hijo("cellXfs") {
            for n in cx.hijos_de("xf") {
                xfs.push(xf_de(n, &codigos, &mut formatos, regional));
            }
        }
        if xfs.is_empty() {
            xfs.push(xf_de(&Nodo::default(), &codigos, &mut formatos, regional));
        }
        // La fuente del estilo Normal: la del primer `cellStyleXfs` (o la 0).
        let normal = hoja
            .hijo("cellStyleXfs")
            .and_then(|c| c.hijos_de("xf").next())
            .and_then(|x| x.attr("fontId"))
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);
        let fuente_normal = fuentes
            .get(normal)
            .or(fuentes.first())
            .cloned()
            .unwrap_or_default();
        let tam_por_xf = xfs
            .iter()
            .map(|x| {
                fuentes
                    .get(x.fuente as usize)
                    .map_or(fuente_normal.tam, |f| f.tam)
            })
            .collect();
        let dxfs: Vec<Dxf> = hoja
            .hijo("dxfs")
            .map(|d| {
                d.hijos_de("dxf")
                    .map(|n| dxf_de(n, tema, &paleta))
                    .collect()
            })
            .unwrap_or_default();
        // Estilos de tabla propios: cada elemento apunta a un dxf.
        let mut estilos_tabla = HashMap::new();
        if let Some(ts) = hoja.hijo("tableStyles") {
            for t in ts.hijos_de("tableStyle") {
                let Some(nombre) = t.attr("name") else {
                    continue;
                };
                let mut e = EstiloTabla::default();
                for el in t.hijos_de("tableStyleElement") {
                    let Some(d) = el
                        .attr("dxfId")
                        .and_then(|v| v.parse::<usize>().ok())
                        .and_then(|i| dxfs.get(i))
                    else {
                        continue;
                    };
                    let tam = el
                        .attr("size")
                        .and_then(|v| v.parse::<u32>().ok())
                        .unwrap_or(1)
                        .max(1);
                    match el.attr("type").unwrap_or("") {
                        "wholeTable" => e.todo = Some(d.clone()),
                        "headerRow" => e.encabezado = Some(d.clone()),
                        "totalRow" => e.total = Some(d.clone()),
                        "firstColumn" => e.primera_columna = Some(d.clone()),
                        "lastColumn" => e.ultima_columna = Some(d.clone()),
                        "firstRowStripe" => {
                            e.banda_fila = Some(d.clone());
                            e.alto_banda = tam;
                        }
                        "secondRowStripe" => e.banda_fila2 = Some(d.clone()),
                        "firstColumnStripe" => e.banda_columna = Some(d.clone()),
                        "secondColumnStripe" => e.banda_columna2 = Some(d.clone()),
                        _ => {}
                    }
                }
                estilos_tabla.insert(nombre.to_string(), e);
            }
        }
        Estilos {
            fuentes,
            rellenos,
            bordes,
            xfs,
            paleta,
            fuente_normal,
            tam_por_xf,
            formatos,
            dxfs,
            estilos_tabla,
        }
    }

    pub fn xf(&self, i: u32) -> &Xf {
        self.xfs.get(i as usize).unwrap_or(&self.xfs[0])
    }

    pub fn fuente(&self, xf: &Xf) -> &Fuente {
        self.fuentes
            .get(xf.fuente as usize)
            .unwrap_or(&self.fuente_normal)
    }

    pub fn relleno(&self, xf: &Xf) -> Relleno {
        self.rellenos
            .get(xf.relleno as usize)
            .copied()
            .unwrap_or_default()
    }

    pub fn borde(&self, xf: &Xf) -> Borde {
        self.bordes
            .get(xf.borde as usize)
            .copied()
            .unwrap_or_default()
    }
}

fn relleno_de(n: &Nodo, tema: &Tema, paleta: &[u32]) -> Relleno {
    if let Some(p) = n.hijo("patternFill") {
        let tipo = p.attr("patternType").unwrap_or("none");
        let fg = p.hijo("fgColor").and_then(|c| color_de(c, tema, paleta));
        let bg = p.hijo("bgColor").and_then(|c| color_de(c, tema, paleta));
        // Densidad de la trama: lo que pesa el color de delante.
        let densidad = match tipo {
            "none" => return Relleno::Nada,
            "solid" => 1.0,
            "gray0625" => 0.0625,
            "gray125" => 0.125,
            "lightGray" => 0.25,
            "mediumGray" => 0.5,
            "darkGray" => 0.75,
            "lightHorizontal" | "lightVertical" | "lightDown" | "lightUp" => 0.25,
            "lightGrid" | "lightTrellis" => 0.4,
            _ => 0.5,
        };
        let fg = fg.unwrap_or(Rgb::NEGRO);
        if densidad >= 1.0 {
            return Relleno::Liso(fg);
        }
        return Relleno::Liso(fg.mezcla(bg.unwrap_or(Rgb::BLANCO), densidad));
    }
    if let Some(g) = n.hijo("gradientFill") {
        // Degradado: su color medio.
        let cs: Vec<Rgb> = g
            .hijos_de("stop")
            .filter_map(|s| s.hijo("color").and_then(|c| color_de(c, tema, paleta)))
            .collect();
        if cs.is_empty() {
            return Relleno::Nada;
        }
        let n = cs.len() as u32;
        let s = cs.iter().fold((0u32, 0u32, 0u32), |a, c| {
            (a.0 + c.r as u32, a.1 + c.g as u32, a.2 + c.b as u32)
        });
        return Relleno::Liso(Rgb {
            r: (s.0 / n) as u8,
            g: (s.1 / n) as u8,
            b: (s.2 / n) as u8,
        });
    }
    Relleno::Nada
}

fn borde_de(n: &Nodo, tema: &Tema, paleta: &[u32]) -> Borde {
    let linea = |nombres: &[&str]| -> Linea {
        for nombre in nombres {
            if let Some(h) = n.hijo(nombre) {
                let estilo = EstiloLinea::de(h.attr("style").unwrap_or(""));
                if estilo != EstiloLinea::Ninguno {
                    let color = h
                        .hijo("color")
                        .and_then(|c| color_de(c, tema, paleta))
                        .unwrap_or(Rgb::NEGRO);
                    return Linea { estilo, color };
                }
            }
        }
        Linea::default()
    };
    let si = |a: &str| n.attr(a).is_some_and(|v| v == "1" || v == "true");
    Borde {
        izquierda: linea(&["left", "start"]),
        derecha: linea(&["right", "end"]),
        arriba: linea(&["top"]),
        abajo: linea(&["bottom"]),
        diagonal: linea(&["diagonal"]),
        diagonal_sube: si("diagonalUp"),
        diagonal_baja: si("diagonalDown"),
    }
}

fn xf_de(
    n: &Nodo,
    codigos: &HashMap<u32, String>,
    formatos: &mut Formatos,
    regional: &Regional,
) -> Xf {
    let num = |a: &str| n.attr(a).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
    let id = num("numFmtId");
    let codigo = codigos
        .get(&id)
        .map(String::as_str)
        .or_else(|| formato::integrado(id))
        .unwrap_or("General");
    let formato = formatos.obtener(codigo, regional);
    let mut al = Alineacion::default();
    if let Some(a) = n.hijo("alignment") {
        al.horizontal = match a.attr("horizontal") {
            Some("left") => Horizontal::Izquierda,
            Some("center") => Horizontal::Centro,
            Some("right") => Horizontal::Derecha,
            Some("fill") => Horizontal::Rellenar,
            Some("justify") => Horizontal::Justificar,
            Some("centerContinuous") => Horizontal::CentroSeleccion,
            Some("distributed") => Horizontal::Distribuido,
            _ => Horizontal::General,
        };
        al.vertical = match a.attr("vertical") {
            Some("top") => Vertical::Arriba,
            Some("center") => Vertical::Centro,
            Some("justify") => Vertical::Justificar,
            Some("distributed") => Vertical::Distribuido,
            _ => Vertical::Abajo,
        };
        let si = |k: &str| a.attr(k).is_some_and(|v| v == "1" || v == "true");
        al.ajustar = si("wrapText");
        al.encoger = si("shrinkToFit");
        al.sangria = a
            .attr("indent")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0)
            .min(250) as u8;
        al.giro = a
            .attr("textRotation")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0)
            .min(255) as u16;
    }
    Xf {
        formato,
        fuente: num("fontId"),
        relleno: num("fillId"),
        borde: num("borderId"),
        alineacion: al,
        comilla: n
            .attr("quotePrefix")
            .is_some_and(|v| v == "1" || v == "true"),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_matiz_aclara_y_oscurece() {
        let azul = Rgb::de_u32(0x4472C4);
        // «Accent1, lighter 80%» de Excel es D9E1F2 (con margen de redondeo).
        let claro = con_matiz(azul, 0.7999816888943144);
        assert!(
            (claro.r as i32 - 0xD9).abs() <= 2
                && (claro.g as i32 - 0xE1).abs() <= 2
                && (claro.b as i32 - 0xF2).abs() <= 2,
            "{claro:?}"
        );
        let oscuro = con_matiz(azul, -0.249977111117893);
        assert!(
            (oscuro.r as i32 - 0x30).abs() <= 2
                && (oscuro.g as i32 - 0x54).abs() <= 2
                && (oscuro.b as i32 - 0x96).abs() <= 2,
            "{oscuro:?}"
        );
    }

    #[test]
    fn lee_estilos_minimos() {
        let x = br#"<styleSheet><numFmts><numFmt numFmtId="164" formatCode="0.000"/></numFmts>
        <fonts><font><sz val="11"/><name val="Calibri"/><scheme val="minor"/></font><font><b/><sz val="14"/><color theme="4"/><name val="Arial"/></font></fonts>
        <fills><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill><fill><patternFill patternType="solid"><fgColor rgb="FFFFFF00"/></patternFill></fill></fills>
        <borders><border/><border><left style="thin"><color indexed="64"/></left><bottom style="double"><color rgb="FFFF0000"/></bottom></border></borders>
        <cellXfs><xf/><xf numFmtId="164" fontId="1" fillId="2" borderId="1"><alignment horizontal="center" wrapText="1" textRotation="135"/></xf></cellXfs></styleSheet>"#;
        let e = Estilos::leer(x, &Tema::default(), &Regional::en_us());
        let xf = e.xf(1);
        assert_eq!(
            xf.formato.numero(1.0, false, &Regional::en_us()).texto,
            "1.000"
        );
        assert!(e.fuente(xf).negrita);
        assert_eq!(e.fuente(xf).color, Some(Rgb::de_u32(0x4472C4)));
        assert_eq!(
            e.relleno(xf),
            Relleno::Liso(Rgb {
                r: 255,
                g: 255,
                b: 0
            })
        );
        assert_eq!(e.borde(xf).abajo.estilo, EstiloLinea::Doble);
        assert_eq!(e.borde(xf).izquierda.color, Rgb::NEGRO);
        assert!(xf.alineacion.ajustar);
        assert_eq!(xf.alineacion.grados(), Some(-45.0));
        assert_eq!(e.xf(99).fuente, 0);
    }
}
