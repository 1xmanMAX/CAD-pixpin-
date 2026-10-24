//! **Pintar una pagina de Office** (`pixpin_office::Pagina`) con Direct2D, y
//! el medidor de verdad (DirectWrite) con el que se maqueta.
//!
//! El texto se pinta glifo a glifo (`DrawGlyphRun`) con las mismas caras y
//! los mismos avances con que se midio: asi lo que la maquetacion decidio
//! (donde parte cada renglon, donde cae cada palabra) es exactamente lo que
//! se ve. Si una letra del documento no esta instalada se usa una sustituta
//! de las mismas medidas si la hay (Carlito por Calibri...) y si no, Arial o
//! Times New Roman segun sea de palo seco o con remates.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use pixpin_office::medidor::{Medidor, Metricas};
use pixpin_office::pagina::{
    Campo, Color as ColorO, Discontinuo, Efecto, Elemento, Figura, Letra, Pagina, Recursos,
    Relleno, Sombra, Subrayado, Texto, Tramo, Trazo,
};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_BEZIER_SEGMENT, D2D1_COLOR_F, D2D1_COMPOSITE_MODE_DESTINATION_IN,
    D2D1_COMPOSITE_MODE_SOURCE_OUT, D2D1_COMPOSITE_MODE_SOURCE_OVER, D2D1_FIGURE_BEGIN_FILLED,
    D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_CLOSED, D2D1_FIGURE_END_OPEN,
    D2D1_FILL_MODE_ALTERNATE, D2D1_GRADIENT_STOP,
};
use windows::Win32::Graphics::Direct2D::{
    CLSID_D2D1AlphaMask, CLSID_D2D1Composite, CLSID_D2D1Crop, CLSID_D2D1Flood,
    CLSID_D2D1GaussianBlur, CLSID_D2D1Morphology, CLSID_D2D1Shadow, CLSID_D2D12DAffineTransform,
    D2D1_2DAFFINETRANSFORM_PROP_TRANSFORM_MATRIX, D2D1_COMPOSITE_PROP_MODE, D2D1_CROP_PROP_RECT,
    D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_FLOOD_PROP_COLOR,
    D2D1_GAUSSIANBLUR_PROP_STANDARD_DEVIATION, D2D1_INTERPOLATION_MODE_LINEAR,
    D2D1_MORPHOLOGY_MODE_DILATE, D2D1_MORPHOLOGY_MODE_ERODE, D2D1_MORPHOLOGY_PROP_HEIGHT,
    D2D1_MORPHOLOGY_PROP_MODE, D2D1_MORPHOLOGY_PROP_WIDTH, D2D1_PROPERTY_TYPE,
    D2D1_PROPERTY_TYPE_ENUM, D2D1_PROPERTY_TYPE_FLOAT, D2D1_PROPERTY_TYPE_MATRIX_3X2,
    D2D1_PROPERTY_TYPE_UINT32, D2D1_PROPERTY_TYPE_VECTOR4,
    D2D1_SHADOW_PROP_BLUR_STANDARD_DEVIATION, D2D1_SHADOW_PROP_COLOR,
    D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE, ID2D1CommandList, ID2D1Effect, ID2D1Image,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_CAP_STYLE_FLAT, D2D1_DASH_STYLE_DASH,
    D2D1_DASH_STYLE_DASH_DOT, D2D1_DASH_STYLE_DOT, D2D1_DASH_STYLE_SOLID, D2D1_EXTEND_MODE_CLAMP,
    D2D1_GAMMA_2_2, D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC, D2D1_LINE_JOIN_MITER,
    D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES, D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES,
    D2D1_STROKE_STYLE_PROPERTIES1, ID2D1Bitmap1, ID2D1Brush, ID2D1DeviceContext, ID2D1Geometry,
    ID2D1PathGeometry, ID2D1RenderTarget, ID2D1SolidColorBrush, ID2D1StrokeStyle,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_METRICS, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_ITALIC,
    DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_NORMAL,
    DWRITE_GLYPH_METRICS, DWRITE_GLYPH_RUN, DWRITE_MEASURING_MODE_NATURAL, IDWriteFactory,
    IDWriteFontCollection, IDWriteFontFace,
};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICBitmapSource, IWICImagingFactory,
    WICBitmapDitherTypeNone, WICBitmapInterpolationModeHighQualityCubic,
    WICBitmapPaletteTypeMedianCut, WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::core::{BOOL, HSTRING, Interface};
use windows_numerics::{Matrix3x2, Vector2};

use crate::lienzo::Pintor;
use crate::motor::MotorRender;

/// Una cara de letra ya abierta, con lo que se le pregunta a menudo.
pub struct Cara {
    cara: IDWriteFontFace,
    em: f32,
    /// Ascenso, descenso, hueco, posicion y grosor del subrayado, posicion
    /// y grosor del tachado, todo en fraccion de em.
    m: [f32; 7],
    /// char → (glifo, avance en em). 0 = no lo tiene.
    glifos: RefCell<HashMap<char, (u16, f32)>>,
}

impl Cara {
    fn glifo(&self, c: char) -> (u16, f32) {
        if let Some(g) = self.glifos.borrow().get(&c) {
            return *g;
        }
        let cp = c as u32;
        let mut g = [0u16; 1];
        // SAFETY: un codigo y un glifo de salida, locales; la cara vive.
        let _ = unsafe { self.cara.GetGlyphIndices(&cp, 1, g.as_mut_ptr()) };
        let mut avance = 0.0;
        if g[0] != 0 {
            let mut gm = [DWRITE_GLYPH_METRICS::default(); 1];
            // SAFETY: un glifo valido de esta cara y su metrica de salida.
            if unsafe {
                self.cara
                    .GetDesignGlyphMetrics(g.as_ptr(), 1, gm.as_mut_ptr(), false)
            }
            .is_ok()
            {
                avance = gm[0].advanceWidth as f32 / self.em;
            }
        }
        self.glifos.borrow_mut().insert(c, (g[0], avance));
        (g[0], avance)
    }
}

/// Familia, negrita y cursiva.
type ClaveCara = (String, bool, bool);

/// **Las letras de Office**: buscar caras por nombre (con sustitutas),
/// medirlas y dar los glifos para pintarlas. Es el [`Medidor`] de la
/// maquetacion.
pub struct LetrasOffice {
    coleccion: IDWriteFontCollection,
    caras: RefCell<HashMap<ClaveCara, Option<Rc<Cara>>>>,
    /// Caras de reserva para los caracteres que la letra pedida no tiene.
    reserva: RefCell<Vec<Rc<Cara>>>,
    reserva_hecha: std::cell::Cell<bool>,
    existentes: RefCell<HashMap<String, bool>>,
}

/// Las sustitutas de las medidas de las letras de Office, y la que se usa
/// si tampoco esta.
fn sustitutas(familia: &str) -> &'static [&'static str] {
    match familia.to_ascii_lowercase().as_str() {
        "calibri" => &["Carlito"],
        "cambria" => &["Caladea"],
        "arial" | "helvetica" => &["Liberation Sans", "Arimo"],
        "times new roman" | "times" => &["Liberation Serif", "Tinos"],
        "courier new" | "courier" => &["Liberation Mono", "Cousine"],
        "aptos" | "aptos display" | "aptos narrow" => &["Calibri", "Carlito"],
        "calibri light" => &["Calibri", "Carlito"],
        "segoe ui light" | "segoe ui semibold" => &["Segoe UI"],
        "symbol" => &["Segoe UI Symbol"],
        _ => &[],
    }
}

fn es_serif(familia: &str) -> bool {
    let f = familia.to_ascii_lowercase();
    [
        "times",
        "serif",
        "georgia",
        "garamond",
        "cambria",
        "book",
        "roman",
        "palatino",
        "bodoni",
        "minion",
        "century",
        "baskerville",
        "mincho",
        "song",
    ]
    .iter()
    .any(|k| f.contains(k))
        && !f.contains("sans")
}

impl LetrasOffice {
    /// Las de la fabrica compartida de DirectWrite, sin motor ni ventana:
    /// para maquetar un Word en un hilo de fondo y saber donde cae cada
    /// letra (el asistente buscando un texto, 14-oct-2026).
    pub fn sin_motor() -> Option<LetrasOffice> {
        use windows::Win32::Graphics::DirectWrite::{
            DWRITE_FACTORY_TYPE_SHARED, DWriteCreateFactory,
        };
        // SAFETY: crear la fabrica compartida no toma punteros del llamante.
        let f: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }.ok()?;
        LetrasOffice::nuevo(&f)
    }

    pub fn nuevo(dwrite: &IDWriteFactory) -> Option<LetrasOffice> {
        let mut coleccion = None;
        // SAFETY: la factoria vive; `coleccion` es la salida local.
        unsafe { dwrite.GetSystemFontCollection(&mut coleccion, false).ok()? };
        Some(LetrasOffice {
            coleccion: coleccion?,
            caras: RefCell::new(HashMap::new()),
            reserva: RefCell::new(Vec::new()),
            reserva_hecha: std::cell::Cell::new(false),
            existentes: RefCell::new(HashMap::new()),
        })
    }

    fn abrir(&self, familia: &str, negrita: bool, cursiva: bool) -> Option<Rc<Cara>> {
        // SAFETY: consultas sobre la coleccion del sistema, viva; todas las
        // salidas son locales.
        unsafe {
            let (mut indice, mut existe) = (0u32, BOOL(0));
            self.coleccion
                .FindFamilyName(&HSTRING::from(familia), &mut indice, &mut existe)
                .ok()?;
            if !existe.as_bool() {
                return None;
            }
            let fuente = self
                .coleccion
                .GetFontFamily(indice)
                .ok()?
                .GetFirstMatchingFont(
                    if negrita {
                        DWRITE_FONT_WEIGHT_BOLD
                    } else {
                        DWRITE_FONT_WEIGHT_NORMAL
                    },
                    DWRITE_FONT_STRETCH_NORMAL,
                    if cursiva {
                        DWRITE_FONT_STYLE_ITALIC
                    } else {
                        DWRITE_FONT_STYLE_NORMAL
                    },
                )
                .ok()?;
            let cara = fuente.CreateFontFace().ok()?;
            let mut m = DWRITE_FONT_METRICS::default();
            cara.GetMetrics(&mut m);
            let em = m.designUnitsPerEm.max(1) as f32;
            Some(Rc::new(Cara {
                cara,
                em,
                m: [
                    m.ascent as f32 / em,
                    m.descent as f32 / em,
                    m.lineGap.max(0) as f32 / em,
                    m.underlinePosition as f32 / em,
                    m.underlineThickness.max(1) as f32 / em,
                    m.strikethroughPosition as f32 / em,
                    m.strikethroughThickness.max(1) as f32 / em,
                ],
                glifos: RefCell::new(HashMap::new()),
            }))
        }
    }

    /// La cara de una letra: la pedida, una sustituta o la de reserva.
    pub fn cara(&self, letra: &Letra) -> Option<Rc<Cara>> {
        let familia = if letra.familia.trim().is_empty() {
            "Calibri"
        } else {
            letra.familia.trim()
        };
        let clave = (familia.to_string(), letra.negrita, letra.cursiva);
        if let Some(c) = self.caras.borrow().get(&clave) {
            return c.clone();
        }
        let mut c = self.abrir(familia, letra.negrita, letra.cursiva);
        if c.is_none() {
            for s in sustitutas(familia) {
                c = self.abrir(s, letra.negrita, letra.cursiva);
                if c.is_some() {
                    break;
                }
            }
        }
        if c.is_none() {
            let generica = if es_serif(familia) {
                "Times New Roman"
            } else {
                "Arial"
            };
            c = self
                .abrir(generica, letra.negrita, letra.cursiva)
                .or_else(|| self.abrir("Segoe UI", letra.negrita, letra.cursiva));
        }
        self.caras.borrow_mut().insert(clave, c.clone());
        c
    }

    fn reservas(&self) -> std::cell::Ref<'_, Vec<Rc<Cara>>> {
        if !self.reserva_hecha.get() {
            self.reserva_hecha.set(true);
            let mut v = Vec::new();
            for f in [
                "Segoe UI",
                "Segoe UI Symbol",
                "Cambria Math",
                "Microsoft YaHei",
                "Yu Gothic",
                "Malgun Gothic",
                "Nirmala UI",
                "Ebrima",
                "Segoe UI Historic",
                "Segoe UI Emoji",
                "Arial Unicode MS",
            ] {
                if let Some(c) = self.abrir(f, false, false) {
                    v.push(c);
                }
            }
            *self.reserva.borrow_mut() = v;
        }
        self.reserva.borrow()
    }

    /// Para cada caracter: la cara que lo tiene, su glifo y su avance en em.
    pub fn glifos(&self, texto: &str, letra: &Letra, salida: &mut Vec<(Rc<Cara>, u16, f32)>) {
        let Some(principal) = self.cara(letra) else {
            return;
        };
        for c in texto.chars() {
            let (g, a) = principal.glifo(c);
            if g != 0 || c == '\u{200B}' || c == '\u{AD}' {
                salida.push((principal.clone(), g, a));
                continue;
            }
            // Los simbolos de Symbol/Wingdings en el area privada (F0xx) a
            // su equivalente Unicode si la letra no los tiene.
            let c2 = simbolo_a_unicode(c).unwrap_or(c);
            if c2 != c {
                let (g, a) = principal.glifo(c2);
                if g != 0 {
                    salida.push((principal.clone(), g, a));
                    continue;
                }
            }
            let reservas = self.reservas();
            let mut hecho = false;
            for r in reservas.iter() {
                let (g, a) = r.glifo(c2);
                if g != 0 {
                    salida.push((r.clone(), g, a));
                    hecho = true;
                    break;
                }
            }
            if !hecho {
                salida.push((principal.clone(), 0, principal.glifo('?').1));
            }
        }
    }
}

/// Los simbolos mas comunes de Symbol y Wingdings (en F0xx) a Unicode.
fn simbolo_a_unicode(c: char) -> Option<char> {
    let n = c as u32;
    if !(0xF000..=0xF0FF).contains(&n) {
        return None;
    }
    Some(match n - 0xF000 {
        0xB7 => '•',
        0xA7 => '▪',
        0xA8 => '◆',
        0x6E => '■',
        0x71 => '❑',
        0x76 => '❖',
        0xD8 => '➢',
        0xFC => '✓',
        0xFB => '✗',
        0x6F => '○',
        0x6C => '●',
        0xE0 => '→',
        0xE8 => '➔',
        0x2D => '−',
        b if (0x20..0x7F).contains(&b) => char::from_u32(b)?,
        _ => '•',
    })
}

impl Medidor for LetrasOffice {
    fn avances(&self, texto: &str, letra: &Letra, salida: &mut Vec<f32>) {
        let mut g = Vec::with_capacity(texto.len());
        self.glifos(texto, letra, &mut g);
        if g.is_empty() {
            salida.extend(texto.chars().map(|_| letra.tam * 0.5));
            return;
        }
        salida.extend(g.iter().map(|(_, _, a)| a * letra.tam));
    }

    fn existe(&self, familia: &str) -> bool {
        if let Some(e) = self.existentes.borrow().get(familia) {
            return *e;
        }
        let (mut indice, mut existe) = (0u32, BOOL(0));
        // SAFETY: consulta sobre la coleccion viva con salidas locales.
        let ok = unsafe {
            self.coleccion
                .FindFamilyName(&HSTRING::from(familia), &mut indice, &mut existe)
        }
        .is_ok()
            && existe.as_bool();
        self.existentes.borrow_mut().insert(familia.to_string(), ok);
        ok
    }

    fn metricas(&self, letra: &Letra) -> Metricas {
        match self.cara(letra) {
            Some(c) => Metricas {
                ascenso: c.m[0] * letra.tam,
                descenso: c.m[1] * letra.tam,
                hueco: c.m[2] * letra.tam,
            },
            None => Metricas {
                ascenso: letra.tam * 0.9,
                descenso: letra.tam * 0.25,
                hueco: 0.0,
            },
        }
    }
}

/// **El pintor de paginas de Office.** Guarda lo que cuesta crear (las
/// imagenes decodificadas, los pinceles, los estilos de trazo); vive lo que
/// viva el `MotorRender` con que se creo.
pub struct PintorOffice {
    pub letras: LetrasOffice,
    wic: Option<IWICImagingFactory>,
    imagenes: RefCell<crate::cache_presupuesto::CachePresupuesto<Arc<str>, Option<ID2D1Bitmap1>>>,
    /// Paginas pintadas en el fotograma actual (por direccion): si vuelve
    /// una, empieza otro fotograma y lo anterior ya se puede expulsar.
    paginas_fotograma: RefCell<Vec<*const Pagina>>,
    pinceles: RefCell<HashMap<[u8; 4], ID2D1SolidColorBrush>>,
    trazos: RefCell<HashMap<u8, Option<ID2D1StrokeStyle>>>,
    /// Contextos aparte del mismo dispositivo para grabar lo que lleva
    /// efectos en una lista de ordenes (uno por nivel de anidamiento): al
    /// principal no se le puede cambiar el destino a mitad de fotograma.
    /// Se crean la primera vez que una pagina trae efectos.
    auxiliares: RefCell<Vec<ID2D1DeviceContext>>,
    /// Modo oscuro pedido ([`PintorOffice::poner_oscuro`]).
    oscuro: Cell<bool>,
    /// La pagina que se esta pintando va en oscuro: se pidio y no trae
    /// fondo propio.
    en_oscuro: Cell<bool>,
    /// Lo que ya tiene un relleno propio en esta pagina (una celda
    /// sombreada, una forma, una imagen), en pixeles del destino: el texto
    /// y las rayas que caen encima se quedan con sus colores.
    rellenos: RefCell<Vec<D2D_RECT_F>>,
}

/// Cuantos `Elemento::Efecto` anidados se pintan con sus efectos; los de
/// mas adentro se pintan sin ellos.
const NIVELES_EFECTO: usize = 3;

impl PintorOffice {
    pub fn nuevo(motor: &MotorRender) -> Option<PintorOffice> {
        // SAFETY: crear la fabrica de WIC no tiene precondiciones (COM ya
        // iniciado por quien llama; si no, queda sin imagenes).
        let wic: Option<IWICImagingFactory> =
            unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).ok() };
        Some(PintorOffice {
            letras: LetrasOffice::nuevo(motor.dwrite())?,
            wic,
            imagenes: RefCell::new(Default::default()),
            paginas_fotograma: RefCell::new(Vec::new()),
            pinceles: RefCell::new(HashMap::new()),
            trazos: RefCell::new(HashMap::new()),
            auxiliares: RefCell::new(Vec::new()),
            oscuro: Cell::new(false),
            en_oscuro: Cell::new(false),
            rellenos: RefCell::new(Vec::new()),
        })
    }

    /// Olvida las imagenes ya subidas (al cambiar de documento).
    pub fn olvidar_imagenes(&self) {
        self.imagenes.borrow_mut().vaciar();
    }

    /// **Modo oscuro** (`crate::oscuro`), como el de Word: la hoja sin
    /// fondo propio pasa a gris muy oscuro y el texto y las rayas oscuras
    /// que no tienen un relleno debajo se aclaran conservando su tono; las
    /// imagenes se invierten igual (19-oct-2026); los rellenos y los
    /// graficos se quedan con sus colores (y lo que va encima de ellos
    /// tambien). Un relleno casi blanco se toma por
    /// papel y se oscurece con el. Vale para lo que se pinte despues.
    pub fn poner_oscuro(&self, si: bool) {
        self.oscuro.set(si);
    }

    /// La caja `r` (en las coordenadas de ahora de `c`) en pixeles del
    /// destino.
    fn en_destino(c: &ID2D1DeviceContext, r: D2D_RECT_F) -> D2D_RECT_F {
        let mut m = Matrix3x2::default();
        // SAFETY: GetTransform rellena la matriz local; el contexto vive.
        unsafe { c.GetTransform(&mut m) };
        let p = |x: f32, y: f32| (x * m.M11 + y * m.M21 + m.M31, x * m.M12 + y * m.M22 + m.M32);
        let esquinas = [
            p(r.left, r.top),
            p(r.right, r.top),
            p(r.left, r.bottom),
            p(r.right, r.bottom),
        ];
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for (x, y) in esquinas {
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
        D2D_RECT_F {
            left: x0,
            top: y0,
            right: x1,
            bottom: y1,
        }
    }

    /// Apunta que en `r` hay un relleno propio (solo en oscuro).
    fn apuntar_relleno(&self, c: &ID2D1DeviceContext, r: D2D_RECT_F) {
        if self.en_oscuro.get() {
            self.rellenos.borrow_mut().push(Self::en_destino(c, r));
        }
    }

    /// El color de un texto o una raya (`raya`) que ocupa `r`: en oscuro y
    /// sin un relleno propio debajo de su centro, legible sobre el papel
    /// oscuro (de las rayas, solo las casi negras: las de color son del
    /// grafico).
    fn color_sobre_papel(
        &self,
        c: &ID2D1DeviceContext,
        col: ColorO,
        r: D2D_RECT_F,
        raya: bool,
    ) -> ColorO {
        if !self.en_oscuro.get() {
            return col;
        }
        let d = Self::en_destino(c, r);
        let (cx, cy) = ((d.left + d.right) / 2.0, (d.top + d.bottom) / 2.0);
        let encima = self
            .rellenos
            .borrow()
            .iter()
            .rev()
            .any(|f| cx >= f.left && cx <= f.right && cy >= f.top && cy <= f.bottom);
        if encima {
            return col;
        }
        let (r, g, b) = if raya {
            crate::oscuro::raya_legible_u8(col.r, col.g, col.b)
        } else {
            crate::oscuro::legible_u8(col.r, col.g, col.b)
        };
        ColorO { r, g, b, a: col.a }
    }

    /// En oscuro, un relleno liso casi blanco es papel: se pinta del papel
    /// oscuro (`Some`) y no cuenta como relleno propio.
    fn relleno_de_papel(&self, r: &Relleno) -> Option<Relleno> {
        (self.en_oscuro.get() && casi_blanco(r))
            .then(|| Relleno::Solido(a_office(crate::oscuro::PAPEL)))
    }

    /// Pinta `pagina` con su esquina en `origen` (pixeles del destino) a
    /// `escala` pixeles por punto. `total` es el numero de paginas para los
    /// campos `NUMPAGES`. Hay que llamarlo dentro de `MotorRender::dibujar`.
    pub fn pintar(
        &self,
        p: &Pintor,
        pagina: &Pagina,
        recursos: &Recursos,
        escala: f32,
        origen: (f32, f32),
        total: u32,
    ) {
        let c = p.motor.contexto();
        {
            let mut vistas = self.paginas_fotograma.borrow_mut();
            let esta = pagina as *const Pagina;
            if vistas.contains(&esta) {
                vistas.clear();
                self.imagenes.borrow_mut().nuevo_fotograma();
            }
            vistas.push(esta);
        }
        let mut previa = Matrix3x2::default();
        // SAFETY: GetTransform rellena la matriz local; el contexto vive.
        unsafe { c.GetTransform(&mut previa) };
        let base = componer(
            &Matrix3x2 {
                M11: escala,
                M12: 0.0,
                M21: 0.0,
                M22: escala,
                M31: origen.0,
                M32: origen.1,
            },
            &previa,
        );
        // SAFETY: dentro del fotograma; se restaura al acabar.
        unsafe { c.SetTransform(&base) };
        let hoja = D2D_RECT_F {
            left: 0.0,
            top: 0.0,
            right: pagina.ancho,
            bottom: pagina.alto,
        };
        // En oscuro, la hoja de papel: la que no trae fondo (la de Word) o lo
        // trae blanco liso (las diapositivas sencillas). La que trae un
        // fondo de color, un degradado o una imagen se ve como la diseno su
        // autor, como en PowerPoint: solo se oscurece alrededor.
        self.en_oscuro
            .set(self.oscuro.get() && pagina.fondo.as_ref().is_none_or(casi_blanco));
        self.rellenos.borrow_mut().clear();
        match &pagina.fondo {
            Some(r) if !self.en_oscuro.get() => self.rellenar_rect(c, hoja, r),
            _ => {
                let papel = if self.en_oscuro.get() {
                    a_office(crate::oscuro::PAPEL)
                } else {
                    ColorO::BLANCO
                };
                if let Some(b) = self.pincel(c, papel) {
                    // SAFETY: dentro del fotograma.
                    unsafe { c.FillRectangle(&hoja, &b) };
                }
            }
        }
        // Lo que se salga de la hoja no se ve.
        // SAFETY: dentro del fotograma; se quita al acabar.
        unsafe { c.PushAxisAlignedClip(&hoja, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE) };
        let ctx = Ctx {
            c,
            recursos,
            total,
            nivel: 0,
        };
        for e in &pagina.elementos {
            self.elemento(&ctx, e, &base);
        }
        // SAFETY: empareja el Push de arriba y restaura la transformada.
        unsafe {
            c.PopAxisAlignedClip();
            c.SetTransform(&previa);
        }
    }

    fn pincel(&self, c: &ID2D1DeviceContext, col: ColorO) -> Option<ID2D1SolidColorBrush> {
        let k = [col.r, col.g, col.b, col.a];
        if let Some(b) = self.pinceles.borrow().get(&k) {
            return Some(b.clone());
        }
        // SAFETY: dentro del fotograma; el color es local.
        let b = unsafe { c.CreateSolidColorBrush(&d2d(col), None) }.ok()?;
        self.pinceles.borrow_mut().insert(k, b.clone());
        Some(b)
    }

    fn estilo_trazo(&self, c: &ID2D1DeviceContext, d: Discontinuo) -> Option<ID2D1StrokeStyle> {
        let k = d as u8;
        if let Some(s) = self.trazos.borrow().get(&k) {
            return s.clone();
        }
        let dash = match d {
            Discontinuo::Solida => D2D1_DASH_STYLE_SOLID,
            Discontinuo::Rayas => D2D1_DASH_STYLE_DASH,
            Discontinuo::Puntos => D2D1_DASH_STYLE_DOT,
            Discontinuo::RayaPunto => D2D1_DASH_STYLE_DASH_DOT,
        };
        let props = D2D1_STROKE_STYLE_PROPERTIES1 {
            startCap: D2D1_CAP_STYLE_FLAT,
            endCap: D2D1_CAP_STYLE_FLAT,
            dashCap: D2D1_CAP_STYLE_FLAT,
            lineJoin: D2D1_LINE_JOIN_MITER,
            miterLimit: 10.0,
            dashStyle: dash,
            dashOffset: 0.0,
            ..Default::default()
        };
        // SAFETY: la fabrica del contexto vive; las propiedades son locales.
        let s = unsafe {
            c.GetFactory()
                .ok()
                .and_then(|f| {
                    f.cast::<windows::Win32::Graphics::Direct2D::ID2D1Factory1>()
                        .ok()
                })
                .and_then(|f| f.CreateStrokeStyle(&props, None).ok())
                .and_then(|s| s.cast::<ID2D1StrokeStyle>().ok())
        };
        self.trazos.borrow_mut().insert(k, s.clone());
        s
    }

    fn pincel_relleno(
        &self,
        c: &ID2D1DeviceContext,
        r: &Relleno,
        caja: D2D_RECT_F,
    ) -> Option<ID2D1Brush> {
        let rt: &ID2D1RenderTarget = c;
        match r {
            Relleno::Solido(col) => self.pincel(c, *col).and_then(|b| b.cast().ok()),
            Relleno::Lineal { paradas, .. } if paradas.len() == 1 => {
                self.pincel(c, paradas[0].1).and_then(|b| b.cast().ok())
            }
            Relleno::Radial { paradas } if paradas.len() == 1 => {
                self.pincel(c, paradas[0].1).and_then(|b| b.cast().ok())
            }
            Relleno::Lineal { paradas, angulo } => {
                let stops: Vec<D2D1_GRADIENT_STOP> = paradas
                    .iter()
                    .map(|(p, col)| D2D1_GRADIENT_STOP {
                        position: *p,
                        color: d2d(*col),
                    })
                    .collect();
                // SAFETY: dentro del fotograma; las paradas se copian.
                let col = unsafe {
                    rt.CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP)
                }
                .ok()?;
                let (w, h) = (caja.right - caja.left, caja.bottom - caja.top);
                let (cx, cy) = (caja.left + w / 2.0, caja.top + h / 2.0);
                let (s, co) = angulo.to_radians().sin_cos();
                let mitad = (w * co.abs() + h * s.abs()) / 2.0;
                let props = D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                    startPoint: Vector2 {
                        X: cx - co * mitad,
                        Y: cy - s * mitad,
                    },
                    endPoint: Vector2 {
                        X: cx + co * mitad,
                        Y: cy + s * mitad,
                    },
                };
                // SAFETY: propiedades y coleccion vivas.
                unsafe { rt.CreateLinearGradientBrush(&props, None, &col) }
                    .ok()
                    .and_then(|b| b.cast().ok())
            }
            Relleno::Radial { paradas } => {
                let stops: Vec<D2D1_GRADIENT_STOP> = paradas
                    .iter()
                    .map(|(p, col)| D2D1_GRADIENT_STOP {
                        position: *p,
                        color: d2d(*col),
                    })
                    .collect();
                // SAFETY: dentro del fotograma; las paradas se copian.
                let col = unsafe {
                    rt.CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP)
                }
                .ok()?;
                let (w, h) = (caja.right - caja.left, caja.bottom - caja.top);
                let props = D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES {
                    center: Vector2 {
                        X: caja.left + w / 2.0,
                        Y: caja.top + h / 2.0,
                    },
                    gradientOriginOffset: Vector2 { X: 0.0, Y: 0.0 },
                    radiusX: w / 2.0 * std::f32::consts::SQRT_2,
                    radiusY: h / 2.0 * std::f32::consts::SQRT_2,
                };
                // SAFETY: propiedades y coleccion vivas.
                unsafe { rt.CreateRadialGradientBrush(&props, None, &col) }
                    .ok()
                    .and_then(|b| b.cast().ok())
            }
        }
    }

    fn rellenar_rect(&self, c: &ID2D1DeviceContext, r: D2D_RECT_F, relleno: &Relleno) {
        if let Some(b) = self.pincel_relleno(c, relleno, r) {
            // SAFETY: dentro del fotograma.
            unsafe { c.FillRectangle(&r, &b) };
        }
    }

    fn elemento(&self, ctx: &Ctx, e: &Elemento, transformada: &Matrix3x2) {
        let c = ctx.c;
        match e {
            Elemento::Texto(t) => self.texto(ctx, t),
            Elemento::Rect {
                rect,
                relleno,
                borde,
            } => {
                let r = D2D_RECT_F {
                    left: rect.x,
                    top: rect.y,
                    right: rect.x + rect.ancho,
                    bottom: rect.y + rect.alto,
                };
                if let Some(rel) = relleno {
                    match self.relleno_de_papel(rel) {
                        Some(papel) => self.rellenar_rect(c, r, &papel),
                        None => {
                            self.rellenar_rect(c, r, rel);
                            self.apuntar_relleno(c, r);
                        }
                    }
                }
                if let Some(t) = borde
                    && let Some(b) = self.pincel(c, self.color_sobre_papel(c, t.color, r, true))
                {
                    let s = self.estilo_trazo(c, t.discontinuo);
                    // SAFETY: dentro del fotograma.
                    unsafe { c.DrawRectangle(&r, &b, t.grosor, s.as_ref()) };
                }
            }
            Elemento::Linea {
                desde,
                hasta,
                trazo,
            } => {
                let caja = D2D_RECT_F {
                    left: desde.0.min(hasta.0),
                    top: desde.1.min(hasta.1),
                    right: desde.0.max(hasta.0),
                    bottom: desde.1.max(hasta.1),
                };
                let color = self.color_sobre_papel(c, trazo.color, caja, true);
                self.linea(c, *desde, *hasta, &Trazo { color, ..*trazo })
            }
            Elemento::Imagen {
                id,
                rect,
                recorte,
                opacidad,
            } => {
                self.imagen(ctx, id, *rect, *recorte, *opacidad);
                // En oscuro la imagen tambien se invierte: lo de encima se
                // aclara como sobre el papel.
                if !self.oscuro.get() {
                    self.apuntar_relleno(
                        c,
                        D2D_RECT_F {
                            left: rect.x,
                            top: rect.y,
                            right: rect.x + rect.ancho,
                            bottom: rect.y + rect.alto,
                        },
                    );
                }
            }
            Elemento::Trayecto {
                figuras,
                relleno,
                borde,
            } => self.trayecto(c, figuras, relleno.as_ref(), borde.as_ref()),
            Elemento::Grupo {
                transformada: t,
                recorte,
                elementos,
            } => {
                let propia = Matrix3x2 {
                    M11: t[0],
                    M12: t[1],
                    M21: t[2],
                    M22: t[3],
                    M31: t[4],
                    M32: t[5],
                };
                let nueva = componer(&propia, transformada);
                // SAFETY: dentro del fotograma; se restaura abajo.
                unsafe { c.SetTransform(&nueva) };
                if let Some(r) = recorte {
                    let d = D2D_RECT_F {
                        left: r.x,
                        top: r.y,
                        right: r.x + r.ancho,
                        bottom: r.y + r.alto,
                    };
                    // SAFETY: dentro del fotograma; se quita abajo.
                    unsafe { c.PushAxisAlignedClip(&d, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE) };
                }
                for h in elementos {
                    self.elemento(ctx, h, &nueva);
                }
                // SAFETY: empareja lo de arriba.
                unsafe {
                    if recorte.is_some() {
                        c.PopAxisAlignedClip();
                    }
                    c.SetTransform(transformada);
                }
            }
            Elemento::Efecto { efectos, elementos } => {
                self.con_efectos(ctx, efectos, elementos, transformada)
            }
        }
    }

    /// El contexto auxiliar del nivel `n` (ver `auxiliares`).
    fn auxiliar(&self, c: &ID2D1DeviceContext, n: usize) -> Option<ID2D1DeviceContext> {
        if n >= NIVELES_EFECTO {
            return None;
        }
        let mut v = self.auxiliares.borrow_mut();
        while v.len() <= n {
            // SAFETY: el dispositivo del contexto vivo da otro contexto suyo.
            let nuevo = unsafe {
                c.GetDevice()
                    .and_then(|d| d.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE))
            }
            .ok()?;
            v.push(nuevo);
        }
        Some(v[n].clone())
    }

    /// Graba `elementos` (con la transformada `m`, en DIP del destino) en una
    /// lista de ordenes, para usarla de entrada de los efectos.
    fn grabar(&self, ctx: &Ctx, elementos: &[Elemento], m: &Matrix3x2) -> Option<ID2D1CommandList> {
        let c2 = self.auxiliar(ctx.c, ctx.nivel)?;
        // SAFETY: protocolo SetTarget/BeginDraw/EndDraw completo sobre el
        // contexto auxiliar de este nivel, que no tiene nada empujado; el
        // destino se desliga siempre, tambien si algo falla.
        unsafe {
            let lista = c2.CreateCommandList().ok()?;
            let (mut dx, mut dy) = (96.0, 96.0);
            ctx.c.GetDpi(&mut dx, &mut dy);
            c2.SetDpi(dx, dy);
            c2.SetTarget(&lista);
            c2.BeginDraw();
            // La silueta no necesita ClearType (y en una lista de ordenes
            // transparente no se puede).
            c2.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            c2.SetTransform(m);
            let sub = Ctx {
                c: &c2,
                recursos: ctx.recursos,
                total: ctx.total,
                nivel: ctx.nivel + 1,
            };
            for e in elementos {
                self.elemento(&sub, e, m);
            }
            let fin = c2.EndDraw(None, None);
            c2.SetTarget(None);
            fin.ok()?;
            lista.Close().ok()?;
            Some(lista)
        }
    }

    /// **Lo que lleva efectos** (`a:effectLst`): debajo el brillo y la
    /// sombra exterior, encima lo envuelto (desvanecido por el borde si lo
    /// pide) y la sombra interior. Todo con los efectos de Direct2D sobre
    /// una lista de ordenes con lo envuelto, en DIP del destino.
    fn con_efectos(&self, ctx: &Ctx, efectos: &[Efecto], elementos: &[Elemento], m: &Matrix3x2) {
        let c = ctx.c;
        let pintar_tal_cual = || {
            for h in elementos {
                self.elemento(ctx, h, m);
            }
        };
        let Some(lista) = self.grabar(ctx, elementos, m) else {
            pintar_tal_cual();
            return;
        };
        let Ok(lista) = lista.cast::<ID2D1Image>() else {
            pintar_tal_cual();
            return;
        };
        // De puntos de la pagina a DIP del destino.
        let k = (m.M11 * m.M22 - m.M12 * m.M21).abs().sqrt();
        let lineal = |dx: f32, dy: f32| (dx * m.M11 + dy * m.M21, dx * m.M12 + dy * m.M22);
        // SAFETY: dentro del fotograma; la lista y los efectos son del mismo
        // dispositivo; la transformada se restaura al final.
        unsafe {
            c.SetTransform(&Matrix3x2::identity());
            let caja = c.GetImageLocalBounds(&lista).unwrap_or_default();
            if !(caja.right > caja.left && caja.bottom > caja.top) {
                c.SetTransform(m);
                return;
            }
            for ef in efectos {
                if let Efecto::Brillo { color, radio } = ef
                    && let Some(img) = brillo(c, &lista, *color, radio * k)
                {
                    c.DrawImage(
                        &img,
                        None,
                        None,
                        D2D1_INTERPOLATION_MODE_LINEAR,
                        D2D1_COMPOSITE_MODE_SOURCE_OVER,
                    );
                }
            }
            for ef in efectos {
                if let Efecto::SombraExterior {
                    color,
                    desenfoque,
                    dx,
                    dy,
                    escala,
                    sesgo,
                    ancla,
                } = ef
                    && let Some(img) = sombra(c, &lista, *color, desenfoque * k)
                {
                    let (ox, oy) = lineal(*dx, *dy);
                    let (ax, ay) = (
                        caja.left + ancla.0 * (caja.right - caja.left),
                        caja.top + ancla.1 * (caja.bottom - caja.top),
                    );
                    let (tx, ty) = (sesgo.0.to_radians().tan(), sesgo.1.to_radians().tan());
                    // p' = (p - ancla) * [sx, tan ky; tan kx, sy] + ancla + desplazamiento
                    let (a, b, cc, d) = (escala.0, ty, tx, escala.1);
                    let t = Matrix3x2 {
                        M11: a,
                        M12: b,
                        M21: cc,
                        M22: d,
                        M31: ax - (ax * a + ay * cc) + ox,
                        M32: ay - (ax * b + ay * d) + oy,
                    };
                    c.SetTransform(&t);
                    c.DrawImage(
                        &img,
                        None,
                        None,
                        D2D1_INTERPOLATION_MODE_LINEAR,
                        D2D1_COMPOSITE_MODE_SOURCE_OVER,
                    );
                    c.SetTransform(&Matrix3x2::identity());
                }
            }
            let suave = efectos.iter().find_map(|e| {
                if let Efecto::BordeSuave { radio } = e {
                    Some(*radio)
                } else {
                    None
                }
            });
            match suave.and_then(|r| borde_suave(c, &lista, r * k)) {
                Some(img) => c.DrawImage(
                    &img,
                    None,
                    None,
                    D2D1_INTERPOLATION_MODE_LINEAR,
                    D2D1_COMPOSITE_MODE_SOURCE_OVER,
                ),
                None => {
                    // Lo envuelto se pinta como siempre, directamente.
                    c.SetTransform(m);
                    pintar_tal_cual();
                    c.SetTransform(&Matrix3x2::identity());
                }
            }
            for ef in efectos {
                if let Efecto::SombraInterior {
                    color,
                    desenfoque,
                    dx,
                    dy,
                } = ef
                {
                    let desplazamiento = lineal(*dx, *dy);
                    if let Some(img) =
                        sombra_interior(c, &lista, caja, *color, desenfoque * k, desplazamiento)
                    {
                        c.DrawImage(
                            &img,
                            None,
                            None,
                            D2D1_INTERPOLATION_MODE_LINEAR,
                            D2D1_COMPOSITE_MODE_SOURCE_OVER,
                        );
                    }
                }
            }
            c.SetTransform(m);
        }
    }

    fn linea(&self, c: &ID2D1DeviceContext, a: (f32, f32), b: (f32, f32), t: &Trazo) {
        let Some(p) = self.pincel(c, t.color) else {
            return;
        };
        let s = self.estilo_trazo(c, t.discontinuo);
        // SAFETY: dentro del fotograma.
        unsafe {
            c.DrawLine(
                Vector2 { X: a.0, Y: a.1 },
                Vector2 { X: b.0, Y: b.1 },
                &p,
                t.grosor,
                s.as_ref(),
            )
        };
    }

    fn geometria(
        &self,
        c: &ID2D1DeviceContext,
        figuras: &[&Figura],
        rellenas: bool,
    ) -> Option<ID2D1PathGeometry> {
        let v = |p: (f32, f32)| Vector2 { X: p.0, Y: p.1 };
        // SAFETY: la geometria se crea, se rellena y se cierra aqui; si algo
        // falla se descarta.
        unsafe {
            let g = c.GetFactory().ok()?.CreatePathGeometry().ok()?;
            let s = g.Open().ok()?;
            s.SetFillMode(D2D1_FILL_MODE_ALTERNATE);
            for fig in figuras {
                s.BeginFigure(
                    v(fig.inicio),
                    if rellenas {
                        D2D1_FIGURE_BEGIN_FILLED
                    } else {
                        D2D1_FIGURE_BEGIN_HOLLOW
                    },
                );
                for t in &fig.tramos {
                    match *t {
                        Tramo::Linea(x, y) => s.AddLine(Vector2 { X: x, Y: y }),
                        Tramo::Bezier(a, b, d) => s.AddBezier(&D2D1_BEZIER_SEGMENT {
                            point1: v(a),
                            point2: v(b),
                            point3: v(d),
                        }),
                    }
                }
                s.EndFigure(if fig.cerrada {
                    D2D1_FIGURE_END_CLOSED
                } else {
                    D2D1_FIGURE_END_OPEN
                });
            }
            s.Close().ok()?;
            Some(g)
        }
    }

    fn trayecto(
        &self,
        c: &ID2D1DeviceContext,
        figuras: &[Figura],
        relleno: Option<&Relleno>,
        borde: Option<&Trazo>,
    ) {
        if let Some(r) = relleno {
            // En oscuro, un relleno casi blanco es papel (y no cuenta).
            let papel = self.relleno_de_papel(r);
            let r = papel.as_ref().unwrap_or(r);
            let llenas: Vec<&Figura> = figuras
                .iter()
                .filter(|f| f.rellenar && f.sombra == Sombra::Ninguna)
                .collect();
            let todas: Vec<&Figura> = figuras.iter().filter(|f| f.rellenar).collect();
            // La caja del degradado es la de toda la forma, no la de cada grupo.
            let caja = self
                .geometria(c, &todas, true)
                .and_then(|g| g.cast::<ID2D1Geometry>().ok())
                // SAFETY: geometria viva; Bounds rellena el rect local.
                .map(|geo| unsafe { geo.GetBounds(None) }.unwrap_or_default());
            if let Some(caja) = caja
                && let Some(b) = self.pincel_relleno(c, r, caja)
            {
                if !llenas.is_empty()
                    && let Some(g) = self.geometria(c, &llenas, true)
                {
                    let geo: ID2D1Geometry = g
                        .cast()
                        .expect("una geometria de trayecto es una geometria");
                    // SAFETY: dentro del fotograma.
                    unsafe { c.FillGeometry(&geo, &b, None) };
                }
                // Las caras tenidas (cubo, bisel) y las que van aparte: cada
                // clase en su geometria, con el relleno y encima el tinte.
                for (s, tinte) in [
                    (Sombra::Aparte, None),
                    (
                        Sombra::ClaraSuave,
                        Some(ColorO {
                            r: 255,
                            g: 255,
                            b: 255,
                            a: 46,
                        }),
                    ),
                    (
                        Sombra::Clara,
                        Some(ColorO {
                            r: 255,
                            g: 255,
                            b: 255,
                            a: 92,
                        }),
                    ),
                    (
                        Sombra::OscuraSuave,
                        Some(ColorO {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 46,
                        }),
                    ),
                    (
                        Sombra::Oscura,
                        Some(ColorO {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 102,
                        }),
                    ),
                ] {
                    let grupo: Vec<&Figura> = figuras
                        .iter()
                        .filter(|f| f.rellenar && f.sombra == s)
                        .collect();
                    if grupo.is_empty() {
                        continue;
                    }
                    let Some(g) = self.geometria(c, &grupo, true) else {
                        continue;
                    };
                    let geo: ID2D1Geometry = g
                        .cast()
                        .expect("una geometria de trayecto es una geometria");
                    // SAFETY: dentro del fotograma.
                    unsafe { c.FillGeometry(&geo, &b, None) };
                    if let Some(t) = tinte
                        && let Some(p) = self.pincel(c, t)
                    {
                        // SAFETY: dentro del fotograma.
                        unsafe { c.FillGeometry(&geo, &p, None) };
                    }
                }
                if papel.is_none() {
                    self.apuntar_relleno(c, caja);
                }
            }
        }
        if let Some(t) = borde {
            let trazadas: Vec<&Figura> = figuras.iter().filter(|f| f.trazar).collect();
            if !trazadas.is_empty()
                && let Some(g) = self.geometria(c, &trazadas, false)
                && let Some(p) = self.pincel(c, {
                    // SAFETY: geometria viva; Bounds rellena el rect local.
                    let caja = g
                        .cast::<ID2D1Geometry>()
                        .ok()
                        .map(|geo| unsafe { geo.GetBounds(None) }.unwrap_or_default())
                        .unwrap_or_default();
                    self.color_sobre_papel(c, t.color, caja, true)
                })
            {
                let s = self.estilo_trazo(c, t.discontinuo);
                // SAFETY: dentro del fotograma.
                unsafe { c.DrawGeometry(&g, &p, t.grosor, s.as_ref()) };
            }
        }
    }

    fn texto(&self, ctx: &Ctx, t: &Texto) {
        let c = ctx.c;
        let total;
        let texto: &str = match t.campo {
            Some(Campo::TotalPaginas) => {
                total = ctx.total.to_string();
                &total
            }
            _ => &t.texto,
        };
        let mut glifos = Vec::with_capacity(texto.len());
        self.letras.glifos(texto, &t.letra, &mut glifos);
        // En oscuro, legible sobre el papel si no lleva un relleno debajo.
        let color = self.color_sobre_papel(
            c,
            t.color,
            D2D_RECT_F {
                left: t.x,
                top: t.y - t.letra.tam * 0.9,
                right: t.x + t.ancho.max(1.0),
                bottom: t.y + t.letra.tam * 0.3,
            },
            false,
        );
        let Some(pincel) = self.pincel(c, color) else {
            return;
        };
        let tam = t.letra.tam;
        let mut x = t.x;
        let chars: Vec<char> = texto.chars().collect();
        let mut i = 0;
        while i < glifos.len() {
            let cara = glifos[i].0.clone();
            let mut indices = Vec::new();
            let mut avances = Vec::new();
            let inicio = x;
            while i < glifos.len() && Rc::ptr_eq(&glifos[i].0, &cara) {
                let (_, g, a) = &glifos[i];
                let mut av = a * tam + t.espaciado;
                if chars.get(i).is_some_and(|ch| *ch == ' ') {
                    av += t.extra_espacio;
                }
                indices.push(*g);
                avances.push(av);
                x += av;
                i += 1;
            }
            let corrida = DWRITE_GLYPH_RUN {
                fontFace: std::mem::ManuallyDrop::new(Some(cara.cara.clone())),
                fontEmSize: tam,
                glyphCount: indices.len() as u32,
                glyphIndices: indices.as_ptr(),
                glyphAdvances: avances.as_ptr(),
                glyphOffsets: std::ptr::null(),
                isSideways: BOOL(0),
                bidiLevel: 0,
            };
            // SAFETY: los arrays viven hasta despues de la llamada; la cara
            // se suelta abajo (ManuallyDrop).
            unsafe {
                c.DrawGlyphRun(
                    Vector2 { X: inicio, Y: t.y },
                    &corrida,
                    None,
                    &pincel,
                    DWRITE_MEASURING_MODE_NATURAL,
                )
            };
            drop(std::mem::ManuallyDrop::into_inner(corrida.fontFace));
        }
        // Subrayado y tachado, con las medidas de la letra.
        let ancho = if t.campo == Some(Campo::TotalPaginas) {
            x - t.x
        } else {
            t.ancho.max(x - t.x - t.extra_espacio)
        };
        let cara = glifos.first().map(|g| g.0.clone());
        let m = cara
            .map(|c| c.m)
            .unwrap_or([0.9, 0.25, 0.0, -0.1, 0.05, 0.3, 0.05]);
        if let Some(s) = t.subrayado {
            let fin = texto.trim_end().chars().count();
            let ancho_sub = if fin < chars.len() {
                // Sin los espacios del final.
                let quitar: f32 = glifos[fin.min(glifos.len())..]
                    .iter()
                    .map(|(_, _, a)| a * tam + t.espaciado + t.extra_espacio)
                    .sum();
                (ancho - quitar).max(0.0)
            } else {
                ancho
            };
            let grosor = (m[4] * tam).max(0.5) * if s == Subrayado::Grueso { 2.0 } else { 1.0 };
            let y = t.y - m[3] * tam;
            let trazo = Trazo {
                color,
                grosor,
                discontinuo: match s {
                    Subrayado::Puntos => Discontinuo::Puntos,
                    Subrayado::Rayas => Discontinuo::Rayas,
                    _ => Discontinuo::Solida,
                },
            };
            self.linea(c, (t.x, y), (t.x + ancho_sub, y), &trazo);
            if s == Subrayado::Doble {
                self.linea(
                    c,
                    (t.x, y + grosor * 2.0),
                    (t.x + ancho_sub, y + grosor * 2.0),
                    &trazo,
                );
            }
        }
        if t.tachado {
            let y = t.y - m[5] * tam;
            self.linea(
                c,
                (t.x, y),
                (t.x + ancho, y),
                &Trazo::solido(color, (m[6] * tam).max(0.5)),
            );
        }
    }

    fn imagen(
        &self,
        ctx: &Ctx,
        id: &Arc<str>,
        rect: pixpin_office::pagina::Rect,
        recorte: Option<[f32; 4]>,
        opacidad: f32,
    ) {
        let c = ctx.c;
        let bitmap = self.imagenes.borrow_mut().obtener(id);
        let bitmap = match bitmap {
            Some(b) => b,
            None => {
                let b = ctx
                    .recursos
                    .bytes(id)
                    .and_then(|bytes| self.decodificar(c, &bytes));
                // Peso en memoria: ancho x alto x 4 de lo decodificado.
                let bytes = b.as_ref().map_or(0, |b| {
                    // SAFETY: GetPixelSize es una consulta del bitmap vivo.
                    let t = unsafe { b.GetPixelSize() };
                    t.width as usize * t.height as usize * 4
                });
                self.imagenes
                    .borrow_mut()
                    .insertar(id.clone(), b.clone(), bytes);
                b
            }
        };
        let Some(b) = bitmap else {
            // Una imagen que no se sabe leer (EMF, WMF...): un marco gris
            // claro en su sitio.
            if let Some(p) = self.pincel(c, ColorO::rgb(0xF0, 0xF0, 0xF0)) {
                let r = D2D_RECT_F {
                    left: rect.x,
                    top: rect.y,
                    right: rect.x + rect.ancho,
                    bottom: rect.y + rect.alto,
                };
                // SAFETY: dentro del fotograma.
                unsafe { c.FillRectangle(&r, &p) };
            }
            return;
        };
        // SAFETY: GetSize es una consulta del bitmap vivo.
        let tam = unsafe { b.GetSize() };
        let (w, h) = (tam.width, tam.height);
        let mut destino = D2D_RECT_F {
            left: rect.x,
            top: rect.y,
            right: rect.x + rect.ancho,
            bottom: rect.y + rect.alto,
        };
        let mut fuente = D2D_RECT_F {
            left: 0.0,
            top: 0.0,
            right: w,
            bottom: h,
        };
        if let Some([l, t, r, bo]) = recorte {
            // La parte de la imagen que va en la caja; lo que cae fuera de
            // la imagen (recorte negativo) deja hueco.
            let (x0, x1) = (l * w, (1.0 - r) * w);
            let (y0, y1) = (t * h, (1.0 - bo) * h);
            if x1 - x0 > 0.0 && y1 - y0 > 0.0 {
                let sx = rect.ancho / (x1 - x0);
                let sy = rect.alto / (y1 - y0);
                let (fx0, fx1) = (x0.max(0.0), x1.min(w));
                let (fy0, fy1) = (y0.max(0.0), y1.min(h));
                destino = D2D_RECT_F {
                    left: rect.x + (fx0 - x0) * sx,
                    top: rect.y + (fy0 - y0) * sy,
                    right: rect.x + (fx1 - x0) * sx,
                    bottom: rect.y + (fy1 - y0) * sy,
                };
                fuente = D2D_RECT_F {
                    left: fx0,
                    top: fy0,
                    right: fx1,
                    bottom: fy1,
                };
            }
        }
        if self.oscuro.get() && Self::imagen_oscura(c, &b, destino, fuente) {
            return;
        }
        // SAFETY: dentro del fotograma; bitmap y rectangulos vivos.
        unsafe {
            c.DrawBitmap(
                &b,
                Some(&destino),
                opacidad,
                D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC,
                Some(&fuente),
                None,
            )
        };
    }

    /// **La imagen en modo oscuro** (19-oct-2026, el usuario: «las imagenes
    /// tambien se tienen que invertir para que este completamente en modo
    /// noche»): la luz invertida y el tono conservado, la misma matriz que
    /// las hojas de un PDF (`Pintor::bitmap_oscuro`). Si el efecto no se
    /// puede crear, `false` y se pinta tal cual.
    fn imagen_oscura(
        c: &ID2D1DeviceContext,
        b: &ID2D1Bitmap1,
        destino: D2D_RECT_F,
        fuente: D2D_RECT_F,
    ) -> bool {
        use windows::Win32::Graphics::Direct2D::Common::D2D1_COMPOSITE_MODE_SOURCE_OVER;
        use windows::Win32::Graphics::Direct2D::{
            CLSID_D2D1ColorMatrix, D2D1_COLORMATRIX_PROP_CLAMP_OUTPUT,
            D2D1_COLORMATRIX_PROP_COLOR_MATRIX, D2D1_PROPERTY_TYPE_BOOL,
            D2D1_PROPERTY_TYPE_MATRIX_5X4,
        };
        let (fw, fh) = (fuente.right - fuente.left, fuente.bottom - fuente.top);
        let (dw, dh) = (destino.right - destino.left, destino.bottom - destino.top);
        if !(fw > 0.0 && fh > 0.0 && dw > 0.0 && dh > 0.0) {
            return true;
        }
        let datos: Vec<u8> = crate::oscuro::matriz_d2d()
            .iter()
            .flatten()
            .flat_map(|x| x.to_ne_bytes())
            .collect();
        // SAFETY: el efecto integrado existe en todo Direct2D 1.1; su entrada
        // es un bitmap vivo del mismo dispositivo; la matriz lleva los 20
        // flotantes que pide su tipo.
        let img = unsafe {
            c.CreateEffect(&CLSID_D2D1ColorMatrix).ok().and_then(|e| {
                e.SetInput(0, b, false);
                let _ = e.SetValue(
                    D2D1_COLORMATRIX_PROP_COLOR_MATRIX.0 as u32,
                    D2D1_PROPERTY_TYPE_MATRIX_5X4,
                    &datos,
                );
                let _ = e.SetValue(
                    D2D1_COLORMATRIX_PROP_CLAMP_OUTPUT.0 as u32,
                    D2D1_PROPERTY_TYPE_BOOL,
                    &1u32.to_ne_bytes(),
                );
                e.GetOutput().ok()
            })
        };
        let Some(img) = img else { return false };
        let (sx, sy) = (dw / fw, dh / fh);
        let (dx, dy) = (
            destino.left - fuente.left * sx,
            destino.top - fuente.top * sy,
        );
        let mut antes = windows_numerics::Matrix3x2::default();
        // SAFETY: dentro del fotograma; la transformada se lee y se repone.
        unsafe {
            c.GetTransform(&mut antes);
            let m = windows_numerics::Matrix3x2 {
                M11: sx * antes.M11,
                M12: sx * antes.M12,
                M21: sy * antes.M21,
                M22: sy * antes.M22,
                M31: dx * antes.M11 + dy * antes.M21 + antes.M31,
                M32: dx * antes.M12 + dy * antes.M22 + antes.M32,
            };
            c.SetTransform(&m);
            c.DrawImage(
                &img,
                None,
                Some(&fuente),
                D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC,
                D2D1_COMPOSITE_MODE_SOURCE_OVER,
            );
            c.SetTransform(&antes);
        }
        true
    }

    fn decodificar(&self, c: &ID2D1DeviceContext, bytes: &[u8]) -> Option<ID2D1Bitmap1> {
        let wic = self.wic.as_ref()?;
        // SAFETY: la cadena de WIC se crea y se usa aqui; los bytes viven
        // mientras dura la decodificacion (el bitmap de D2D copia los
        // pixeles).
        unsafe {
            let flujo = wic.CreateStream().ok()?;
            flujo.InitializeFromMemory(bytes).ok()?;
            let deco = wic
                .CreateDecoderFromStream(&flujo, std::ptr::null(), WICDecodeMetadataCacheOnDemand)
                .ok()?;
            let marco = deco.GetFrame(0).ok()?;
            let (mut w, mut h) = (0u32, 0u32);
            marco.GetSize(&mut w, &mut h).ok()?;
            let mut fuente: IWICBitmapSource = marco.cast().ok()?;
            // Las enormes se reducen: D2D no pasa de 16384 y una foto de 40
            // megapixeles en una pagina no necesita mas que esto.
            let tope = 4096u32;
            if w.max(h) > tope {
                let k = tope as f32 / w.max(h) as f32;
                let esc = wic.CreateBitmapScaler().ok()?;
                esc.Initialize(
                    &fuente,
                    ((w as f32 * k) as u32).max(1),
                    ((h as f32 * k) as u32).max(1),
                    WICBitmapInterpolationModeHighQualityCubic,
                )
                .ok()?;
                fuente = esc.cast().ok()?;
            }
            let conv = wic.CreateFormatConverter().ok()?;
            conv.Initialize(
                &fuente,
                &GUID_WICPixelFormat32bppPBGRA,
                WICBitmapDitherTypeNone,
                None,
                0.0,
                WICBitmapPaletteTypeMedianCut,
            )
            .ok()?;
            c.CreateBitmapFromWicBitmap(&conv, None).ok()
        }
    }
}

struct Ctx<'a> {
    c: &'a ID2D1DeviceContext,
    recursos: &'a Recursos,
    total: u32,
    /// Cuantos `Elemento::Efecto` hay por encima (el contexto auxiliar que
    /// toca para grabar uno mas).
    nivel: usize,
}

// --- Los efectos de Direct2D -------------------------------------------------

fn poner(e: &ID2D1Effect, i: u32, tipo: D2D1_PROPERTY_TYPE, datos: &[u8]) {
    // SAFETY: el indice y el tipo son los documentados de cada efecto y los
    // datos tienen su tamano.
    let _ = unsafe { e.SetValue(i, tipo, datos) };
}

fn poner_f32(e: &ID2D1Effect, i: u32, v: f32) {
    poner(e, i, D2D1_PROPERTY_TYPE_FLOAT, &v.to_ne_bytes());
}

fn poner_u32(e: &ID2D1Effect, i: u32, tipo: D2D1_PROPERTY_TYPE, v: u32) {
    poner(e, i, tipo, &v.to_ne_bytes());
}

fn poner_flotantes(e: &ID2D1Effect, i: u32, tipo: D2D1_PROPERTY_TYPE, v: &[f32]) {
    let b: Vec<u8> = v.iter().flat_map(|x| x.to_ne_bytes()).collect();
    poner(e, i, tipo, &b);
}

fn color_v4(c: ColorO) -> [f32; 4] {
    [
        c.r as f32 / 255.0,
        c.g as f32 / 255.0,
        c.b as f32 / 255.0,
        c.a as f32 / 255.0,
    ]
}

fn efecto(
    c: &ID2D1DeviceContext,
    clsid: &windows::core::GUID,
    entradas: &[&ID2D1Image],
) -> Option<ID2D1Effect> {
    // SAFETY: los efectos integrados existen en todo Direct2D 1.1; las
    // entradas son imagenes vivas del mismo dispositivo.
    unsafe {
        let e = c.CreateEffect(clsid).ok()?;
        for (i, img) in entradas.iter().enumerate() {
            e.SetInput(i as u32, *img, false);
        }
        Some(e)
    }
}

fn salida(e: &ID2D1Effect) -> Option<ID2D1Image> {
    // SAFETY: el efecto vive; la salida es una referencia nueva.
    unsafe { e.GetOutput() }.ok()
}

/// La desviacion tipica del desenfoque de Direct2D para un `blurRad` de
/// DrawingML (en DIP): la mitad, como lo ve PowerPoint a ojo.
fn desviacion(radio: f32) -> f32 {
    (radio * 0.5).clamp(0.0, 250.0)
}

/// La silueta de `img` de `color`, desenfocada `radio` DIP.
fn sombra(
    c: &ID2D1DeviceContext,
    img: &ID2D1Image,
    color: ColorO,
    radio: f32,
) -> Option<ID2D1Image> {
    let e = efecto(c, &CLSID_D2D1Shadow, &[img])?;
    poner_f32(
        &e,
        D2D1_SHADOW_PROP_BLUR_STANDARD_DEVIATION.0 as u32,
        desviacion(radio),
    );
    poner_flotantes(
        &e,
        D2D1_SHADOW_PROP_COLOR.0 as u32,
        D2D1_PROPERTY_TYPE_VECTOR4,
        &color_v4(color),
    );
    salida(&e)
}

/// Engorda (o adelgaza) la silueta `radio` DIP con una morfologia (que no
/// pasa de 100 de nucleo).
fn morfologia(
    c: &ID2D1DeviceContext,
    img: &ID2D1Image,
    radio: f32,
    engordar: bool,
) -> Option<ID2D1Image> {
    let n = (radio * 2.0).round().clamp(1.0, 100.0) as u32;
    let e = efecto(c, &CLSID_D2D1Morphology, &[img])?;
    let modo = if engordar {
        D2D1_MORPHOLOGY_MODE_DILATE
    } else {
        D2D1_MORPHOLOGY_MODE_ERODE
    };
    poner_u32(
        &e,
        D2D1_MORPHOLOGY_PROP_MODE.0 as u32,
        D2D1_PROPERTY_TYPE_ENUM,
        modo.0 as u32,
    );
    poner_u32(
        &e,
        D2D1_MORPHOLOGY_PROP_WIDTH.0 as u32,
        D2D1_PROPERTY_TYPE_UINT32,
        n,
    );
    poner_u32(
        &e,
        D2D1_MORPHOLOGY_PROP_HEIGHT.0 as u32,
        D2D1_PROPERTY_TYPE_UINT32,
        n,
    );
    salida(&e)
}

/// El brillo (`a:glow`): la silueta engordada la mitad del radio y
/// desenfocada el resto, del color del brillo.
fn brillo(
    c: &ID2D1DeviceContext,
    img: &ID2D1Image,
    color: ColorO,
    radio: f32,
) -> Option<ID2D1Image> {
    if radio < 0.5 {
        return None;
    }
    let gorda = morfologia(c, img, radio * 0.5, true)?;
    let e = efecto(c, &CLSID_D2D1Shadow, &[&gorda])?;
    poner_f32(
        &e,
        D2D1_SHADOW_PROP_BLUR_STANDARD_DEVIATION.0 as u32,
        (radio * 0.25).clamp(0.0, 250.0),
    );
    poner_flotantes(
        &e,
        D2D1_SHADOW_PROP_COLOR.0 as u32,
        D2D1_PROPERTY_TYPE_VECTOR4,
        &color_v4(color),
    );
    salida(&e)
}

/// El borde suave (`a:softEdge`): lo envuelto con la opacidad de su propia
/// silueta adelgazada y desenfocada, que se apaga hacia el borde.
fn borde_suave(c: &ID2D1DeviceContext, img: &ID2D1Image, radio: f32) -> Option<ID2D1Image> {
    if radio < 0.5 {
        return None;
    }
    let flaca = morfologia(c, img, radio * 0.5, false)?;
    let desenfoque = efecto(c, &CLSID_D2D1GaussianBlur, &[&flaca])?;
    poner_f32(
        &desenfoque,
        D2D1_GAUSSIANBLUR_PROP_STANDARD_DEVIATION.0 as u32,
        (radio * 0.25).clamp(0.0, 250.0),
    );
    let mascara = salida(&desenfoque)?;
    let e = efecto(c, &CLSID_D2D1AlphaMask, &[img, &mascara])?;
    salida(&e)
}

/// La sombra interior (`a:innerShdw`): el color fuera de la silueta
/// desplazada, desenfocado, y recortado por la silueta sin desplazar.
fn sombra_interior(
    c: &ID2D1DeviceContext,
    img: &ID2D1Image,
    caja: D2D_RECT_F,
    color: ColorO,
    radio: f32,
    (dx, dy): (f32, f32),
) -> Option<ID2D1Image> {
    let margen = desviacion(radio) * 3.0 + dx.abs().max(dy.abs()) + 2.0;
    let relleno = efecto(c, &CLSID_D2D1Flood, &[])?;
    poner_flotantes(
        &relleno,
        D2D1_FLOOD_PROP_COLOR.0 as u32,
        D2D1_PROPERTY_TYPE_VECTOR4,
        &color_v4(color),
    );
    let relleno = salida(&relleno)?;
    let recorte = efecto(c, &CLSID_D2D1Crop, &[&relleno])?;
    poner_flotantes(
        &recorte,
        D2D1_CROP_PROP_RECT.0 as u32,
        D2D1_PROPERTY_TYPE_VECTOR4,
        &[
            caja.left - margen,
            caja.top - margen,
            caja.right + margen,
            caja.bottom + margen,
        ],
    );
    let relleno = salida(&recorte)?;
    let mover = efecto(c, &CLSID_D2D12DAffineTransform, &[img])?;
    poner_flotantes(
        &mover,
        D2D1_2DAFFINETRANSFORM_PROP_TRANSFORM_MATRIX.0 as u32,
        D2D1_PROPERTY_TYPE_MATRIX_3X2,
        &[1.0, 0.0, 0.0, 1.0, dx, dy],
    );
    let movida = salida(&mover)?;
    // Composite: la entrada 0 es el destino y la 1 la fuente.
    let fuera = efecto(c, &CLSID_D2D1Composite, &[&movida, &relleno])?;
    poner_u32(
        &fuera,
        D2D1_COMPOSITE_PROP_MODE.0 as u32,
        D2D1_PROPERTY_TYPE_ENUM,
        D2D1_COMPOSITE_MODE_SOURCE_OUT.0 as u32,
    );
    let fuera = salida(&fuera)?;
    let desenfoque = efecto(c, &CLSID_D2D1GaussianBlur, &[&fuera])?;
    poner_f32(
        &desenfoque,
        D2D1_GAUSSIANBLUR_PROP_STANDARD_DEVIATION.0 as u32,
        desviacion(radio),
    );
    let borrosa = salida(&desenfoque)?;
    let dentro = efecto(c, &CLSID_D2D1Composite, &[&borrosa, img])?;
    poner_u32(
        &dentro,
        D2D1_COMPOSITE_PROP_MODE.0 as u32,
        D2D1_PROPERTY_TYPE_ENUM,
        D2D1_COMPOSITE_MODE_DESTINATION_IN.0 as u32,
    );
    salida(&dentro)
}

/// Un relleno liso casi blanco: en oscuro, es papel.
fn casi_blanco(r: &Relleno) -> bool {
    matches!(r, Relleno::Solido(col) if col.a > 200 && col.r.min(col.g).min(col.b) >= 0xF0)
}

fn d2d(c: ColorO) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: c.r as f32 / 255.0,
        g: c.g as f32 / 255.0,
        b: c.b as f32 / 255.0,
        a: c.a as f32 / 255.0,
    }
}

/// Un color del motor (0..1) como color de Office (0..255).
fn a_office(c: crate::motor::Color) -> ColorO {
    let q = |x: f32| (x * 255.0).round().clamp(0.0, 255.0) as u8;
    ColorO {
        r: q(c.r),
        g: q(c.g),
        b: q(c.b),
        a: q(c.a),
    }
}

/// `a` y despues `b` (como en Direct2D: el punto se multiplica por `a`
/// primero).
fn componer(a: &Matrix3x2, b: &Matrix3x2) -> Matrix3x2 {
    Matrix3x2 {
        M11: a.M11 * b.M11 + a.M12 * b.M21,
        M12: a.M11 * b.M12 + a.M12 * b.M22,
        M21: a.M21 * b.M11 + a.M22 * b.M21,
        M22: a.M21 * b.M12 + a.M22 * b.M22,
        M31: a.M31 * b.M11 + a.M32 * b.M21 + b.M31,
        M32: a.M31 * b.M12 + a.M32 * b.M22 + b.M32,
    }
}

#[cfg(test)]
mod pruebas_oscuro {
    use super::*;

    #[test]
    fn en_oscuro_solo_el_blanco_liso_es_papel() {
        assert!(casi_blanco(&Relleno::Solido(ColorO::BLANCO)));
        assert!(casi_blanco(&Relleno::Solido(ColorO::rgb(0xF2, 0xF2, 0xF2))));
        // Caso negativo: un color, un gris medio o un degradado son del
        // autor y se quedan.
        assert!(!casi_blanco(&Relleno::Solido(ColorO::rgb(
            0xFF, 0xF2, 0xCC
        ))));
        assert!(!casi_blanco(&Relleno::Solido(ColorO::rgb(
            0xD9, 0xD9, 0xD9
        ))));
        assert!(!casi_blanco(&Relleno::Lineal {
            paradas: vec![(0.0, ColorO::BLANCO), (1.0, ColorO::BLANCO)],
            angulo: 0.0
        }));
        // Un blanco transparente no tapa nada: no es papel.
        assert!(!casi_blanco(&Relleno::Solido(ColorO {
            r: 255,
            g: 255,
            b: 255,
            a: 40
        })));
    }
}
