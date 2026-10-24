//! **Pintar una hoja de Excel** (`pixpin_hojas`) con Direct2D y DirectWrite,
//! como la pinta Excel: cuadricula, rellenos, bordes con la prioridad de
//! Excel en los lados compartidos, texto con su letra, alineacion, ajuste,
//! recorte y desborde sobre las vecinas vacias, combinaciones, imagenes,
//! graficos sencillos, encabezados (A, B, C... / 1, 2, 3...), paneles
//! inmovilizados y la tira de pestanas.
//!
//! Solo se pinta lo que cae en la ventana: lo que cuesta un fotograma
//! depende del tamano de la pantalla, no del de la hoja. Las disposiciones
//! de texto y los formatos de letra se guardan en [`CacheHoja`] (de quien
//! llama, uno por visor) y se reutilizan entre fotogramas.
//!
//! Convencion de pixeles: la columna `c` ocupa en pantalla `[X(c), X(c+1))`
//! con `X` redondeado a entero, y su linea de la derecha (cuadricula o borde
//! fino) es el ultimo pixel, `X(c+1) - 1`. Un relleno tapa tambien la linea
//! de su izquierda y la de arriba, como en Excel.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use pixpin_hojas::dibujos::{Contenido as ContenidoDibujo, Dibujo};
use pixpin_hojas::estilos::{
    Borde, EstiloLinea, Fuente, Horizontal, Linea, Relleno, Rgb, Subrayado, Vertical,
};
use pixpin_hojas::{Hoja, Libro, Panel, Rango, Rejilla, Valor, Vista, nombre_columna};
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_ALIASED, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_CAP_STYLE_FLAT,
    D2D1_DASH_STYLE_CUSTOM, D2D1_DRAW_TEXT_OPTIONS_NONE,
    D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC, D2D1_LINE_JOIN_MITER,
    D2D1_STROKE_STYLE_PROPERTIES1, ID2D1Bitmap1, ID2D1DeviceContext, ID2D1SolidColorBrush,
    ID2D1StrokeStyle,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_ITALIC, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_ALIGNMENT,
    DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_JUSTIFIED, DWRITE_TEXT_ALIGNMENT_LEADING,
    DWRITE_TEXT_ALIGNMENT_TRAILING, DWRITE_TEXT_METRICS, DWRITE_TEXT_RANGE,
    DWRITE_WORD_WRAPPING_NO_WRAP, DWRITE_WORD_WRAPPING_WRAP, IDWriteFontCollection,
    IDWriteTextFormat, IDWriteTextLayout,
};
use windows::core::{HSTRING, IUnknown, Interface, w};
use windows_numerics::{Matrix3x2, Vector2};

use crate::lienzo::{Pintor, RectF};
use crate::motor::{Color, MotorRender};

/// Colores del marco de Excel (los de la interfaz, no los del libro).
const CUADRICULA: Rgb = Rgb {
    r: 0xD4,
    g: 0xD4,
    b: 0xD4,
};
const ENCABEZADO_FONDO: Rgb = Rgb {
    r: 0xF5,
    g: 0xF5,
    b: 0xF5,
};
const ENCABEZADO_LINEA: Rgb = Rgb {
    r: 0xD4,
    g: 0xD4,
    b: 0xD4,
};
const ENCABEZADO_TEXTO: Rgb = Rgb {
    r: 0x44,
    g: 0x44,
    b: 0x44,
};
const LINEA_FIJA: Rgb = Rgb {
    r: 0x9B,
    g: 0x9B,
    b: 0x9B,
};
const SELECCION: Rgb = Rgb {
    r: 0x21,
    g: 0x73,
    b: 0x46,
};

thread_local! {
    /// Si el fotograma que se esta pintando va en modo oscuro (lo pone
    /// [`pintar_hoja`] con [`OpcionesHoja::oscuro`]; lo leen las pestanas, que
    /// se pintan en el mismo fotograma). Un hilo pinta un fotograma a la vez.
    static OSCURO: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn oscuro() -> bool {
    OSCURO.with(|o| o.get())
}

/// Pone el modo oscuro para lo que se pinte despues en este hilo (lo hace
/// [`pintar_hoja`]; quien pinte solo las pestanas puede llamarlo antes).
pub fn poner_oscuro(si: bool) {
    OSCURO.with(|o| o.set(si));
}

fn rgb_de(c: (u8, u8, u8)) -> Rgb {
    Rgb {
        r: c.0,
        g: c.1,
        b: c.2,
    }
}

/// **Un color del marco de Excel** (fondo, rejilla, encabezados, pestanas):
/// en oscuro, con la luz invertida y su tono (`crate::oscuro`).
fn ui(c: Rgb) -> Rgb {
    if oscuro() {
        rgb_de(crate::oscuro::invertir_luz_u8(c.r, c.g, c.b))
    } else {
        c
    }
}

/// Lo blanco de la hoja (las celdas sin relleno).
fn papel() -> Rgb {
    ui(Rgb::BLANCO)
}

/// Un texto o una raya sobre el papel: en oscuro, si es oscura, se aclara.
fn legible(c: Rgb) -> Rgb {
    if oscuro() {
        rgb_de(crate::oscuro::legible_u8(c.r, c.g, c.b))
    } else {
        c
    }
}

/// Un borde de celda sobre el papel: en oscuro, el casi negro se aclara; los
/// de color se quedan.
fn raya(c: Rgb) -> Rgb {
    if oscuro() {
        rgb_de(crate::oscuro::raya_legible_u8(c.r, c.g, c.b))
    } else {
        c
    }
}

/// Un relleno liso casi blanco: en oscuro es papel (muchos libros rellenan
/// de blanco toda la hoja para esconder la cuadricula).
fn casi_blanco(c: Rgb) -> bool {
    c.r.min(c.g).min(c.b) >= 0xF0
}

/// El color con que se pinta el relleno de una celda: el suyo, salvo el
/// casi blanco en oscuro, que es papel.
fn relleno_de(c: Rgb) -> Rgb {
    if oscuro() && casi_blanco(c) {
        papel()
    } else {
        c
    }
}

fn color(c: Rgb) -> Color {
    Color {
        r: c.r as f32 / 255.0,
        g: c.g as f32 / 255.0,
        b: c.b as f32 / 255.0,
        a: 1.0,
    }
}

fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left: x0,
        top: y0,
        right: x1,
        bottom: y1,
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct ClaveFormato {
    familia: String,
    tam: u32,
    negrita: bool,
    cursiva: bool,
}

struct EntradaTexto {
    disposicion: IDWriteTextLayout,
    ancho: f32,
    alto: f32,
    usado: u64,
}

/// Lo que dura entre fotogramas: formatos de letra, disposiciones de texto,
/// estilos de trazo y las imagenes ya subidas a la GPU. Uno por visor (los
/// objetos son del dispositivo del motor con que se pinta).
#[derive(Default)]
pub struct CacheHoja {
    formatos: HashMap<ClaveFormato, IDWriteTextFormat>,
    textos: HashMap<u64, EntradaTexto>,
    fotograma: u64,
    trazos: HashMap<(u8, u32), Option<ID2D1StrokeStyle>>,
    imagenes: crate::cache_presupuesto::CachePresupuesto<String, Option<ID2D1Bitmap1>>,
    colecciones: HashMap<String, Option<IDWriteFontCollection>>,
    /// Los textos ya preparados de cada celda (ver `texto_cacheado`), validos
    /// para el libro, la hoja, su version y el zoom de `llave_celdas`.
    celdas: HashMap<(u32, u32), Preparada>,
    llave_celdas: (usize, usize, u64, u64),
    /// Lo que costo el ultimo fotograma.
    pub medidas: Medidas,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Medidas {
    pub celdas: u32,
    pub textos: u32,
    pub disposiciones_nuevas: u32,
    pub bordes: u32,
    /// Microsegundos de CPU en preparar los textos y en encargar su dibujo.
    pub us_preparar_textos: u32,
    pub us_dibujar_textos: u32,
}

/// Opciones de un fotograma.
#[derive(Debug, Clone, Default)]
pub struct OpcionesHoja {
    /// Pintar los encabezados (A, B, C / 1, 2, 3). Si la hoja los esconde
    /// (`showRowColHeaders=0`) no se pintan aunque se pida.
    pub encabezados: bool,
    /// Rango seleccionado (se marca con el verde de Excel).
    pub seleccion: Option<Rango>,
    /// La celda activa (lleva el boton de su lista desplegable, si la tiene).
    pub activa: Option<(u32, u32)>,
    /// La celda bajo el raton (tambien lleva el boton).
    pub bajo_raton: Option<(u32, u32)>,
    /// **Modo oscuro** (el boton luna/sol de los pines): papel, rejilla,
    /// encabezados y pestanas oscuros y el texto sin relleno debajo, claro.
    /// Los rellenos de las celdas, las imagenes y los graficos se quedan con
    /// sus colores (y el texto encima de un relleno, tambien: ya se lee).
    pub oscuro: bool,
}

/// Cuanto miden los encabezados en pantalla: (ancho de la columna de los
/// numeros de fila, alto de la fila de las letras). Ceros si no se pintan.
pub fn medidas_encabezados(
    hoja: &Hoja,
    rejilla: &Rejilla,
    vista: &Vista,
    encabezados: bool,
) -> (f32, f32) {
    if !encabezados || !hoja.vista().encabezados {
        return (0.0, 0.0);
    }
    let e = vista.escala.max(0.05);
    let ultima = rejilla.fila_en(vista.y + vista.alto / e) + 1;
    let digitos = ultima.max(1).ilog10() + 1;
    let ancho = ((digitos.max(3) as f64 * 7.0 + 10.0) * e * rejilla.ppp / 96.0).round() as f32;
    let alto = (20.0 * e * rejilla.ppp / 96.0).round() as f32;
    (ancho, alto)
}

/// La zona de celdas (sin encabezados) dentro de `caja`, para calcular la
/// [`Vista`] y hacer hit-test con las mismas cuentas que el pintor.
pub fn zona_de_celdas(
    hoja: &Hoja,
    rejilla: &Rejilla,
    vista: &Vista,
    caja: RectF,
    encabezados: bool,
) -> RectF {
    let (ew, eh) = medidas_encabezados(hoja, rejilla, vista, encabezados);
    RectF {
        x: caja.x + ew,
        y: caja.y + eh,
        ancho: (caja.ancho - ew).max(0.0),
        alto: (caja.alto - eh).max(0.0),
    }
}

/// **Autoajusta las filas sin `ht`** de `desde..=hasta` al texto partido en
/// lineas de sus celdas, como hace Excel al abrir un libro que no guardo
/// los altos (los que escriben openpyxl, POI, pandas...). Mide cada fila
/// una sola vez. Devuelve si cambio algun alto (hay que repintar).
///
/// Para llamar antes de pintar con las filas que se van a ver (o con todas,
/// en otro momento): solo cuesta en filas con celdas de texto ajustado.
pub fn autoajustar_filas(
    cache: &mut CacheHoja,
    motor: &MotorRender,
    libro: &Libro,
    hoja: &Hoja,
    rejilla: &mut Rejilla,
    desde: u32,
    hasta: u32,
) -> bool {
    let mut miradas = Vec::new();
    let mut altos = Vec::new();
    let k = rejilla.ppp as f32 / 96.0;
    for f in hoja.filas_entre(desde, hasta) {
        if !f.sin_alto || f.oculta || rejilla.ajustada(f.indice) {
            continue;
        }
        miradas.push(f.indice);
        let mut max_h = 0.0f32;
        for c in hoja.celdas_de(f) {
            if c.valor == Valor::Vacio {
                continue;
            }
            let xf = libro.estilos.xf(c.xf);
            if !xf.alineacion.ajustar
                || xf.alineacion.giro != 0
                || hoja.combinada_en(f.indice, c.col).is_some()
            {
                continue;
            }
            let Some(cont) = libro.contenido(hoja, c) else {
                continue;
            };
            let fuente = libro.estilos.fuente(xf).clone();
            let tam = fuente.tam * rejilla.ppp as f32 / 72.0;
            let margen = (2.0 * k).round().max(1.0);
            let ancho = (rejilla.ancho(c.col) as f32 - 1.0 - 2.0 * margen).max(1.0);
            if let Some((_, _, h)) = cache.disposicion(
                motor,
                &cont.mostrado.texto,
                &fuente,
                tam,
                Some(ancho),
                DWRITE_TEXT_ALIGNMENT_LEADING,
                cont.tramos.as_deref(),
                rejilla.ppp as f32 / 72.0,
            ) {
                max_h = max_h.max(h);
            }
        }
        if max_h > 0.0 {
            let px = (max_h + 2.0 * k).ceil() as f64;
            if px > rejilla.alto(f.indice) {
                altos.push((f.indice, px));
            }
        }
    }
    if miradas.is_empty() {
        return false;
    }
    rejilla.fijar_altos(hoja, &miradas, &altos)
}

/// Un texto listo para pintar.
#[derive(Clone)]
struct Trabajo {
    disposicion: IDWriteTextLayout,
    x: f32,
    y: f32,
    recorte: D2D_RECT_F,
    color: Rgb,
    /// Giro en grados contrarreloj y centro de giro.
    giro: Option<(f32, f32, f32)>,
    /// Ancho y alto del texto, si se saben: si cabe entero en el recorte,
    /// se pinta sin recortar (cambiar de recorte corta los lotes de
    /// Direct2D y es lo que mas cuesta con mil celdas a la vista).
    medida: Option<(f32, f32)>,
}

/// Como se traduce una zona de la hoja a pantalla.
#[derive(Clone, Copy)]
struct Mapa {
    ox: f64,
    oy: f64,
    hx: f64,
    hy: f64,
    e: f64,
}

impl Mapa {
    fn de(p: &Panel, celdas: RectF, e: f64) -> Mapa {
        Mapa {
            ox: celdas.x as f64 + p.pantalla[0],
            oy: celdas.y as f64 + p.pantalla[1],
            hx: p.origen.0,
            hy: p.origen.1,
            e,
        }
    }
    fn sx(&self, r: &Rejilla, col: u32) -> f32 {
        (self.ox + (r.x(col) - self.hx) * self.e).round() as f32
    }
    fn sy(&self, r: &Rejilla, fila: u32) -> f32 {
        (self.oy + (r.y(fila) - self.hy) * self.e).round() as f32
    }
    fn px(&self, x: f64) -> f32 {
        (self.ox + (x - self.hx) * self.e) as f32
    }
    fn py(&self, y: f64) -> f32 {
        (self.oy + (y - self.hy) * self.e) as f32
    }
}

/// Le dice al formato condicional en que huso horario estamos (para las
/// reglas de «hoy», «la semana pasada»...).
fn fijar_hora_local() {
    use windows::Win32::System::Time::{GetTimeZoneInformation, TIME_ZONE_INFORMATION};
    let mut tz = TIME_ZONE_INFORMATION::default();
    // SAFETY: salida local valida.
    let id = unsafe { GetTimeZoneInformation(&mut tz) };
    let extra = match id {
        1 => tz.StandardBias,
        2 => tz.DaylightBias,
        _ => 0,
    };
    pixpin_hojas::condicional::fijar_desfase_horario(-(tz.Bias + extra));
}

impl CacheHoja {
    pub fn nueva() -> CacheHoja {
        fijar_hora_local();
        CacheHoja::default()
    }

    /// Olvida lo que es del dispositivo (tras perderlo).
    pub fn olvidar_dispositivo(&mut self) {
        self.imagenes.vaciar();
        self.trazos.clear();
        self.textos.clear();
    }

    /// La coleccion donde esta la familia si NO esta en las del sistema: las
    /// fuentes «en la nube» de Office (Aptos, Aptos Narrow...) se descargan
    /// a `%LOCALAPPDATA%\Microsoft\FontCache\4\CloudFonts\<familia>` sin
    /// instalarse, y Excel las usa; sin esto DirectWrite pintaria Segoe UI.
    /// `None`: la del sistema (o ninguna: DirectWrite sustituye).
    fn coleccion(&mut self, motor: &MotorRender, familia: &str) -> Option<IDWriteFontCollection> {
        let clave = familia.to_lowercase();
        if let Some(c) = self.colecciones.get(&clave) {
            return c.clone();
        }
        let c = coleccion_nube(motor, familia);
        self.colecciones.insert(clave, c.clone());
        c
    }

    /// El ancho en pixeles (al 100 %) del digito mas ancho de la fuente, como
    /// lo mide Excel para pasar anchos de columna a pixeles. Medido con la
    /// fuente de verdad: mejor que la tabla de `pixpin_hojas::rejilla`.
    pub fn ancho_digito(&mut self, motor: &MotorRender, f: &Fuente, ppp: f64) -> Option<f64> {
        // Excel mide con GDI a 96 ppp: la letra a un numero entero de
        // pixeles (11 pt -> 15 px) y avances ajustados a la rejilla.
        let tam = (f.tam as f64 * ppp / 72.0).round().max(1.0) as f32;
        let formato = self.formato(motor, &f.nombre, tam, false, false)?;
        let mut max = 0.0f32;
        for d in ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"] {
            let u: Vec<u16> = d.encode_utf16().collect();
            // SAFETY: formato vivo; el texto se copia; `m` es local.
            let w = unsafe {
                let l = motor
                    .dwrite()
                    .CreateGdiCompatibleTextLayout(&u, &formato, 1.0e4, 1.0e4, 1.0, None, false)
                    .ok()?;
                let mut m = DWRITE_TEXT_METRICS::default();
                l.GetMetrics(&mut m).ok()?;
                m.widthIncludingTrailingWhitespace
            };
            max = max.max(w);
        }
        (max > 0.0).then(|| (max as f64).round().max(1.0))
    }

    fn formato(
        &mut self,
        motor: &MotorRender,
        familia: &str,
        tam: f32,
        negrita: bool,
        cursiva: bool,
    ) -> Option<IDWriteTextFormat> {
        let clave = ClaveFormato {
            familia: familia.to_string(),
            tam: tam.to_bits(),
            negrita,
            cursiva,
        };
        if let Some(f) = self.formatos.get(&clave) {
            return Some(f.clone());
        }
        let familia_ok = if familia.trim().is_empty() {
            "Calibri"
        } else {
            familia
        };
        let coleccion = self.coleccion(motor, familia_ok);
        // SAFETY: cadenas propias vivas durante la llamada; el formato copia
        // el nombre y guarda su referencia a la coleccion.
        let f = unsafe {
            motor
                .dwrite()
                .CreateTextFormat(
                    &HSTRING::from(familia_ok),
                    coleccion.as_ref(),
                    if negrita {
                        DWRITE_FONT_WEIGHT_BOLD
                    } else {
                        DWRITE_FONT_WEIGHT_NORMAL
                    },
                    if cursiva {
                        DWRITE_FONT_STYLE_ITALIC
                    } else {
                        DWRITE_FONT_STYLE_NORMAL
                    },
                    DWRITE_FONT_STRETCH_NORMAL,
                    tam.max(0.5),
                    w!("es-PE"),
                )
                .ok()?
        };
        if self.formatos.len() > 256 {
            self.formatos.clear();
        }
        self.formatos.insert(clave, f.clone());
        Some(f)
    }

    /// La disposicion de un texto (cacheada). `ajuste`: ancho para partir en
    /// lineas (None: una sola linea).
    #[allow(clippy::too_many_arguments)]
    fn disposicion(
        &mut self,
        motor: &MotorRender,
        texto: &str,
        f: &Fuente,
        tam: f32,
        ajuste: Option<f32>,
        alinear: DWRITE_TEXT_ALIGNMENT,
        tramos: Option<&[pixpin_hojas::textos::TramoRico]>,
        escala_tramos: f32,
    ) -> Option<(IDWriteTextLayout, f32, f32)> {
        // En una sola linea la colocacion la hace quien pinta: la disposicion
        // (de un millon de pixeles de ancho) va siempre a la izquierda.
        let alinear = if ajuste.is_none() {
            DWRITE_TEXT_ALIGNMENT_LEADING
        } else {
            alinear
        };
        let mut h = std::collections::hash_map::DefaultHasher::new();
        texto.hash(&mut h);
        f.nombre.hash(&mut h);
        (
            tam.to_bits(),
            f.negrita,
            f.cursiva,
            f.subrayado as u8,
            f.tachado,
            ajuste.map(f32::to_bits),
            alinear.0,
        )
            .hash(&mut h);
        if let Some(ts) = tramos {
            for t in ts {
                t.texto.hash(&mut h);
                t.fuente.nombre.hash(&mut h);
                (
                    t.fuente.tam.to_bits(),
                    t.fuente.negrita,
                    t.fuente.cursiva,
                    t.fuente.subrayado as u8,
                    t.fuente.tachado,
                    t.fuente.color,
                    t.fuente.vertical,
                )
                    .hash(&mut h);
            }
        }
        let clave = h.finish();
        if let Some(e) = self.textos.get_mut(&clave) {
            e.usado = self.fotograma;
            return Some((e.disposicion.clone(), e.ancho, e.alto));
        }
        let formato = self.formato(motor, &f.nombre, tam, f.negrita, f.cursiva)?;
        let u16s: Vec<u16> = texto.encode_utf16().collect();
        let todo = DWRITE_TEXT_RANGE {
            startPosition: 0,
            length: u16s.len() as u32,
        };
        // SAFETY: formato vivo; el texto se copia dentro de la disposicion.
        let d = unsafe {
            motor
                .dwrite()
                .CreateTextLayout(&u16s, &formato, ajuste.unwrap_or(1.0e6).max(1.0), 1.0e6)
                .ok()?
        };
        // SAFETY: la disposicion es nueva y propia; los rangos son del texto.
        unsafe {
            let _ = d.SetWordWrapping(if ajuste.is_some() {
                DWRITE_WORD_WRAPPING_WRAP
            } else {
                DWRITE_WORD_WRAPPING_NO_WRAP
            });
            let _ = d.SetTextAlignment(alinear);
            if f.subrayado != Subrayado::Ninguno {
                let _ = d.SetUnderline(true, todo);
            }
            if f.tachado {
                let _ = d.SetStrikethrough(true, todo);
            }
            if let Some(ts) = tramos {
                let mut ini = 0u32;
                for t in ts {
                    let n = t.texto.encode_utf16().count() as u32;
                    let r = DWRITE_TEXT_RANGE {
                        startPosition: ini,
                        length: n,
                    };
                    ini += n;
                    let tf = &t.fuente;
                    let _ = d.SetFontFamilyName(&HSTRING::from(&*tf.nombre), r);
                    if let Some(c) = self.coleccion(motor, &tf.nombre) {
                        let _ = d.SetFontCollection(&c, r);
                    }
                    let mut tam_t = tf.tam * escala_tramos;
                    if tf.vertical != 0 {
                        tam_t *= 0.65;
                    }
                    let _ = d.SetFontSize(tam_t.max(0.5), r);
                    let _ = d.SetFontWeight(
                        if tf.negrita {
                            DWRITE_FONT_WEIGHT_BOLD
                        } else {
                            DWRITE_FONT_WEIGHT_NORMAL
                        },
                        r,
                    );
                    let _ = d.SetFontStyle(
                        if tf.cursiva {
                            DWRITE_FONT_STYLE_ITALIC
                        } else {
                            DWRITE_FONT_STYLE_NORMAL
                        },
                        r,
                    );
                    let _ = d.SetUnderline(tf.subrayado != Subrayado::Ninguno, r);
                    let _ = d.SetStrikethrough(tf.tachado, r);
                    if let Some(c) = tf.color {
                        if let Some(p) = motor.pincel(color(c)) {
                            if let Ok(u) = p.cast::<IUnknown>() {
                                let _ = d.SetDrawingEffect(&u, r);
                            }
                        }
                    }
                }
            }
        }
        let mut m = DWRITE_TEXT_METRICS::default();
        // SAFETY: disposicion viva; `m` es la salida local.
        unsafe { d.GetMetrics(&mut m).ok()? };
        let ancho = m.widthIncludingTrailingWhitespace;
        self.medidas.disposiciones_nuevas += 1;
        if self.textos.len() >= 6000 {
            let f = self.fotograma;
            self.textos.retain(|_, e| e.usado + 2 >= f);
            if self.textos.len() >= 6000 {
                self.textos.clear();
            }
        }
        self.textos.insert(
            clave,
            EntradaTexto {
                disposicion: d.clone(),
                ancho,
                alto: m.height,
                usado: self.fotograma,
            },
        );
        Some((d, ancho, m.height))
    }

    fn trazo(
        &mut self,
        motor: &MotorRender,
        estilo: EstiloLinea,
        grosor: f32,
    ) -> Option<ID2D1StrokeStyle> {
        let rayas: &[f32] = match estilo {
            EstiloLinea::Pelo => &[1.0, 1.0],
            EstiloLinea::Punteado => &[1.0, 1.0],
            EstiloLinea::Discontinuo => &[3.0, 1.0],
            EstiloLinea::MedioDiscontinuo => &[9.0, 3.0],
            EstiloLinea::RayaPunto
            | EstiloLinea::MedioRayaPunto
            | EstiloLinea::RayaPuntoInclinada => &[9.0, 3.0, 3.0, 3.0],
            EstiloLinea::RayaPuntoPunto | EstiloLinea::MedioRayaPuntoPunto => {
                &[9.0, 3.0, 3.0, 3.0, 3.0, 3.0]
            }
            _ => return None,
        };
        let clave = (estilo as u8, grosor.to_bits());
        if let Some(t) = self.trazos.get(&clave) {
            return t.clone();
        }
        // Las rayas de Direct2D van en multiplos del grosor.
        let g = grosor.max(1.0);
        let rayas: Vec<f32> = rayas.iter().map(|r| r * (g.min(2.0)) / g).collect();
        let props = D2D1_STROKE_STYLE_PROPERTIES1 {
            startCap: D2D1_CAP_STYLE_FLAT,
            endCap: D2D1_CAP_STYLE_FLAT,
            dashCap: D2D1_CAP_STYLE_FLAT,
            lineJoin: D2D1_LINE_JOIN_MITER,
            miterLimit: 10.0,
            dashStyle: D2D1_DASH_STYLE_CUSTOM,
            dashOffset: 0.0,
            ..Default::default()
        };
        // SAFETY: factoria viva; las rayas son un slice local.
        let t = unsafe {
            motor
                .fabrica()
                .CreateStrokeStyle(&props, Some(&rayas))
                .ok()
                .and_then(|s| s.cast::<ID2D1StrokeStyle>().ok())
        };
        self.trazos.insert(clave, t.clone());
        t
    }

    /// La imagen de una parte del libro, descodificada con WIC y subida a la
    /// GPU la primera vez. `None` si no se puede (EMF/WMF, rota...).
    pub fn imagen(
        &mut self,
        motor: &MotorRender,
        libro: &Libro,
        ruta: &str,
    ) -> Option<ID2D1Bitmap1> {
        if let Some(b) = self.imagenes.obtener(&ruta.to_string()) {
            return b;
        }
        let b = libro
            .parte(ruta)
            .and_then(|bytes| bitmap_wic(motor, &bytes).or_else(|| bitmap_emf(motor, &bytes)));
        // Peso en memoria: ancho x alto x 4 de lo decodificado.
        let bytes = b.as_ref().map_or(0, |b| {
            // SAFETY: GetPixelSize es una consulta del bitmap vivo.
            let t = unsafe { b.GetPixelSize() };
            t.width as usize * t.height as usize * 4
        });
        self.imagenes.insertar(ruta.to_string(), b.clone(), bytes);
        b
    }
}

/// Si `familia` no esta en las fuentes del sistema, una coleccion con las de
/// su carpeta de fuentes en la nube de Office, si la hay.
fn coleccion_nube(motor: &MotorRender, familia: &str) -> Option<IDWriteFontCollection> {
    use windows::Win32::Graphics::DirectWrite::IDWriteFactory5;
    // SAFETY: consultas sobre la factoria viva; las salidas son locales.
    let en_sistema = unsafe {
        let mut sis: Option<IDWriteFontCollection> = None;
        motor
            .dwrite()
            .GetSystemFontCollection(&mut sis, false)
            .ok()?;
        let sis = sis?;
        let (mut i, mut hay) = (0u32, windows::core::BOOL(0));
        sis.FindFamilyName(&HSTRING::from(familia), &mut i, &mut hay)
            .ok()?;
        hay.as_bool()
    };
    if en_sistema {
        return None;
    }
    let base = std::env::var_os("LOCALAPPDATA")?;
    let dir = std::path::Path::new(&base)
        .join("Microsoft")
        .join("FontCache")
        .join("4")
        .join("CloudFonts");
    let carpeta = std::fs::read_dir(&dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.is_dir()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.eq_ignore_ascii_case(familia))
        })?;
    let ficheros: Vec<std::path::PathBuf> = std::fs::read_dir(&carpeta)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| matches!(x.to_ascii_lowercase().as_str(), "ttf" | "otf" | "ttc"))
        })
        .take(64)
        .collect();
    if ficheros.is_empty() {
        return None;
    }
    let f5: IDWriteFactory5 = motor.dwrite().cast().ok()?;
    // SAFETY: rutas propias vivas durante cada llamada; DirectWrite abre los
    // ficheros por su cuenta y la coleccion se queda con sus referencias.
    unsafe {
        let constructor = f5.CreateFontSetBuilder().ok()?;
        let mut alguna = false;
        for p in &ficheros {
            let Ok(fichero) = f5.CreateFontFileReference(&HSTRING::from(p.as_os_str()), None)
            else {
                continue;
            };
            if constructor.AddFontFile(&fichero).is_ok() {
                alguna = true;
            }
        }
        if !alguna {
            return None;
        }
        let juego = constructor.CreateFontSet().ok()?;
        f5.CreateFontCollectionFromFontSet(&juego).ok()?.cast().ok()
    }
}

/// Un metarchivo mejorado (EMF, tan comun en libros viejos) pintado con GDI
/// en un mapa de bits sobre blanco. WIC no los lee. (Los WMF necesitarian
/// `SetWinMetaFileBits`, de otra parte de la API: se quedan en recuadro.)
fn bitmap_emf(motor: &MotorRender, bytes: &[u8]) -> Option<ID2D1Bitmap1> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLACKNESS, CreateCompatibleDC, CreateDIBSection,
        DIB_RGB_COLORS, DeleteDC, DeleteEnhMetaFile, DeleteObject, ENHMETAHEADER, GdiFlush,
        GetEnhMetaFileHeader, HGDIOBJ, PatBlt, PlayEnhMetaFile, SelectObject, SetEnhMetaFileBits,
        WHITENESS,
    };
    // Cabecera EMR_HEADER (tipo 1) con la firma « EMF» en el byte 40.
    if bytes.len() < 88 || bytes[0..4] != [1, 0, 0, 0] || &bytes[40..44] != b" EMF" {
        return None;
    }
    // SAFETY: GDI con objetos propios que se liberan todos antes de salir;
    // `bits` apunta a la seccion DIB, que vive hasta el DeleteObject y solo
    // se lee despues de GdiFlush, con el tamano con que se creo.
    unsafe {
        let emf = SetEnhMetaFileBits(bytes);
        if emf.is_invalid() {
            return None;
        }
        let mut cab = ENHMETAHEADER::default();
        GetEnhMetaFileHeader(
            emf,
            std::mem::size_of::<ENHMETAHEADER>() as u32,
            Some(&mut cab),
        );
        let b = cab.rclBounds;
        let (mut w, mut h) = ((b.right - b.left + 1).max(1), (b.bottom - b.top + 1).max(1));
        // Ni enorme ni diminuto: hasta 2048 de lado, al menos 256.
        let lado = w.max(h) as f32;
        let k = if lado > 2048.0 {
            2048.0 / lado
        } else if lado < 256.0 {
            256.0 / lado
        } else {
            1.0
        };
        w = ((w as f32 * k).round() as i32).clamp(1, 2048);
        h = ((h as f32 * k).round() as i32).clamp(1, 2048);
        let dc = CreateCompatibleDC(None);
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let Ok(dib) = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0) else {
            let _ = DeleteDC(dc);
            let _ = DeleteEnhMetaFile(Some(emf));
            return None;
        };
        let antes = SelectObject(dc, HGDIOBJ(dib.0));
        // Dos pasadas, sobre blanco y sobre negro: lo que cambia entre una y
        // otra es el fondo, y de ahi sale la transparencia (GDI no tiene alfa).
        let n = (w * h * 4) as usize;
        let mut sobre_blanco = vec![0u8; n];
        let mut rgba = vec![0u8; n];
        if !bits.is_null() {
            let _ = PatBlt(dc, 0, 0, w, h, WHITENESS);
            let _ = PlayEnhMetaFile(
                dc,
                emf,
                &RECT {
                    left: 0,
                    top: 0,
                    right: w,
                    bottom: h,
                },
            );
            let _ = GdiFlush();
            sobre_blanco.copy_from_slice(std::slice::from_raw_parts(bits as *const u8, n));
            let _ = PatBlt(dc, 0, 0, w, h, BLACKNESS);
            let _ = PlayEnhMetaFile(
                dc,
                emf,
                &RECT {
                    left: 0,
                    top: 0,
                    right: w,
                    bottom: h,
                },
            );
            let _ = GdiFlush();
            let negro = std::slice::from_raw_parts(bits as *const u8, n);
            for ((d, b), k) in rgba
                .chunks_exact_mut(4)
                .zip(sobre_blanco.chunks_exact(4))
                .zip(negro.chunks_exact(4))
            {
                let dif = (b[0] as i32 - k[0] as i32)
                    .max(b[1] as i32 - k[1] as i32)
                    .max(b[2] as i32 - k[2] as i32)
                    .clamp(0, 255);
                let a = 255 - dif;
                let des = |c: u8| {
                    if a == 0 {
                        0
                    } else {
                        ((c as i32 * 255) / a).min(255) as u8
                    }
                };
                d.copy_from_slice(&[des(k[2]), des(k[1]), des(k[0]), a as u8]);
            }
        }
        SelectObject(dc, antes);
        let _ = DeleteObject(HGDIOBJ(dib.0));
        let _ = DeleteDC(dc);
        let _ = DeleteEnhMetaFile(Some(emf));
        if bits.is_null() {
            return None;
        }
        motor
            .bitmap_desde_pixeles_premultiplicado(w as u32, h as u32, &rgba)
            .ok()
    }
}

/// Descodifica una imagen en memoria con WIC (PNG, JPEG, GIF, BMP, TIFF...)
/// y la sube como bitmap de Direct2D, premultiplicada.
fn bitmap_wic(motor: &MotorRender, bytes: &[u8]) -> Option<ID2D1Bitmap1> {
    use windows::Win32::Graphics::Imaging::{
        CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICImagingFactory,
        WICBitmapDitherTypeNone, WICBitmapPaletteTypeMedianCut, WICDecodeMetadataCacheOnDemand,
    };
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
    if bytes.len() < 8 || bytes.len() > 256 * 1024 * 1024 {
        return None;
    }
    // SAFETY: protocolo de WIC con objetos propios; `bytes` vive mas que el
    // flujo, que solo se usa dentro de este bloque (el bitmap de Direct2D
    // copia los pixeles al crearse). COM tiene que estar iniciado en el hilo
    // (lo esta en el de la interfaz; el ejemplo lo inicia).
    unsafe {
        let wic: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).ok()?;
        let flujo = wic.CreateStream().ok()?;
        flujo.InitializeFromMemory(bytes).ok()?;
        let dec = wic
            .CreateDecoderFromStream(&flujo, std::ptr::null(), WICDecodeMetadataCacheOnDemand)
            .ok()?;
        let marco = dec.GetFrame(0).ok()?;
        let conv = wic.CreateFormatConverter().ok()?;
        conv.Initialize(
            &marco,
            &GUID_WICPixelFormat32bppPBGRA,
            WICBitmapDitherTypeNone,
            None,
            0.0,
            WICBitmapPaletteTypeMedianCut,
        )
        .ok()?;
        let (mut w, mut h) = (0u32, 0u32);
        conv.GetSize(&mut w, &mut h).ok()?;
        if w == 0 || h == 0 || w > motor.lado_maximo_bitmap() || h > motor.lado_maximo_bitmap() {
            return None;
        }
        motor.contexto().CreateBitmapFromWicBitmap(&conv, None).ok()
    }
}

fn ctx<'a>(p: &'a Pintor) -> &'a ID2D1DeviceContext {
    p.motor.contexto()
}

fn pincel(p: &Pintor, c: Rgb) -> Option<ID2D1SolidColorBrush> {
    p.motor.pincel(color(c))
}

fn llenar(p: &Pintor, r: D2D_RECT_F, c: Rgb) {
    if r.right <= r.left || r.bottom <= r.top {
        return;
    }
    if let Some(b) = pincel(p, c) {
        // SAFETY: dentro del fotograma; pincel y contexto vivos.
        unsafe { ctx(p).FillRectangle(&r, &b) };
    }
}

/// El boton de la flecha de una lista desplegable dentro de la celda
/// `(x0, y0, x1, y1)` en pantalla: pegado a su borde derecho y abajo, de
/// 17 x 20 px logicos como mucho (`k` = pixeles por pixel logico). `None` si
/// la celda es demasiado pequena para llevarlo.
pub fn caja_flecha(celda: [f32; 4], k: f32) -> Option<[f32; 4]> {
    let [x0, y0, x1, y1] = celda;
    let (ancho, alto) = (17.0 * k, (20.0 * k).min(y1 - y0 - 1.0));
    if x1 - x0 < ancho * 1.6 || alto < 8.0 * k.max(0.5) {
        return None;
    }
    Some([x1 - ancho - 1.0, y1 - alto - 1.0, x1 - 1.0, y1 - 1.0])
}

/// Donde cae la celda en pantalla (`x0, y0, x1, y1`) con esta vista, si esta
/// a la vista. `celdas` es la zona de celdas (sin encabezados), la misma con
/// que se pinta.
pub fn caja_de_celda(
    rejilla: &Rejilla,
    vista: &Vista,
    celdas: RectF,
    fila: u32,
    col: u32,
) -> Option<[f32; 4]> {
    let e = vista.escala.max(0.05);
    let v = Vista {
        ancho: celdas.ancho as f64,
        alto: celdas.alto as f64,
        ..*vista
    };
    for panel in &rejilla.paneles(&v) {
        if fila < panel.fila0 || fila > panel.fila1 || col < panel.col0 || col > panel.col1 {
            continue;
        }
        let m = Mapa::de(panel, celdas, e);
        return Some([
            m.sx(rejilla, col),
            m.sy(rejilla, fila),
            m.sx(rejilla, col + 1),
            m.sy(rejilla, fila + 1),
        ]);
    }
    None
}

const FLECHA_FONDO: Rgb = Rgb {
    r: 0xF2,
    g: 0xF2,
    b: 0xF2,
};
const FLECHA_BORDE: Rgb = Rgb {
    r: 0xA6,
    g: 0xA6,
    b: 0xA6,
};
const FLECHA_TINTA: Rgb = Rgb {
    r: 0x40,
    g: 0x40,
    b: 0x40,
};
const FLECHA_TENUE: Rgb = Rgb {
    r: 0x9A,
    g: 0x9A,
    b: 0x9A,
};

/// Un triangulo hacia abajo de `ancho` px centrado en `cx`, con la punta en
/// `y_punta`, hecho con tiras de una fila (sin geometrias de Direct2D).
fn triangulo_abajo(p: &Pintor, cx: f32, y_punta: f32, ancho: f32, c: Rgb) {
    let filas = (ancho / 2.0).round().max(1.0) as i32;
    for i in 0..filas {
        let w = ancho - 2.0 * i as f32;
        if w <= 0.0 {
            break;
        }
        let y = y_punta - (filas - i) as f32;
        llenar(p, rect(cx - w / 2.0, y, cx + w / 2.0, y + 1.0), c);
    }
}

/// **Las flechas de las listas desplegables** (solo para ver): el boton
/// completo en la celda activa y en la que esta bajo el raton, como Excel; y,
/// si no hay celda activa, un triangulito discreto en cada celda con lista.
fn pintar_flechas(
    p: &Pintor,
    hoja: &Hoja,
    rejilla: &Rejilla,
    paneles: &[Panel],
    celdas: RectF,
    e: f64,
    op: &OpcionesHoja,
) {
    use pixpin_hojas::validacion::TipoValidacion;
    let listas: Vec<_> = hoja
        .cola
        .validaciones
        .iter()
        .filter(|v| v.tipo == TipoValidacion::Lista && v.flecha)
        .collect();
    if listas.is_empty() {
        return;
    }
    let k = (e * rejilla.ppp / 96.0) as f32;
    p.empujar_recorte(celdas);
    for panel in paneles {
        let m = Mapa::de(panel, celdas, e);
        let caja = |f: u32, c: u32| {
            [
                m.sx(rejilla, c),
                m.sy(rejilla, f),
                m.sx(rejilla, c + 1),
                m.sy(rejilla, f + 1),
            ]
        };
        let en_panel = |f: u32, c: u32| {
            f >= panel.fila0 && f <= panel.fila1 && c >= panel.col0 && c <= panel.col1
        };
        let mut con_boton: Vec<(u32, u32)> = Vec::new();
        for celda in [op.activa, op.bajo_raton].into_iter().flatten() {
            if en_panel(celda.0, celda.1)
                && !con_boton.contains(&celda)
                && listas.iter().any(|v| v.afecta(celda.0, celda.1))
            {
                con_boton.push(celda);
            }
        }
        for &(f, c) in &con_boton {
            let Some([bx0, by0, bx1, by1]) = caja_flecha(caja(f, c), k) else {
                continue;
            };
            llenar(p, rect(bx0, by0, bx1, by1), ui(FLECHA_BORDE));
            llenar(
                p,
                rect(bx0 + 1.0, by0 + 1.0, bx1 - 1.0, by1 - 1.0),
                ui(FLECHA_FONDO),
            );
            triangulo_abajo(
                p,
                (bx0 + bx1) / 2.0,
                (by0 + by1) / 2.0 + 2.5 * k,
                (7.0 * k).round().max(3.0),
                ui(FLECHA_TINTA),
            );
        }
        if op.activa.is_some() {
            continue;
        }
        // Sin celda activa: un triangulo discreto en las celdas a la vista.
        let mut pintadas = 0u32;
        for v in &listas {
            for r in &v.rangos {
                let (f0, f1) = (r.fila0.max(panel.fila0), r.fila1.min(panel.fila1));
                let (c0, c1) = (r.col0.max(panel.col0), r.col1.min(panel.col1));
                for f in f0..=f1 {
                    for c in c0..=c1 {
                        if pintadas >= 4000 {
                            break;
                        }
                        if con_boton.contains(&(f, c)) {
                            continue;
                        }
                        let [x0, y0, x1, y1] = caja(f, c);
                        if x1 - x0 < 14.0 * k || y1 - y0 < 8.0 * k || rejilla.alto(f) <= 0.0 {
                            continue;
                        }
                        triangulo_abajo(
                            p,
                            x1 - 6.0 * k,
                            y1 - 3.0 * k,
                            (6.0 * k).round().max(3.0),
                            ui(FLECHA_TENUE),
                        );
                        pintadas += 1;
                    }
                }
            }
        }
    }
    p.soltar_recorte();
}

/// Un lado de celda en el mapa de bordes: horizontal (encima de `fila`) o
/// vertical (a la izquierda de `col`).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Lado {
    horizontal: bool,
    fila: u32,
    col: u32,
}

/// Pinta la hoja en `caja` (encabezados incluidos). La [`Vista`] dice el
/// desplazamiento y la escala; su ancho y alto se recalculan con la caja.
#[allow(clippy::too_many_arguments)]
pub fn pintar_hoja(
    p: &Pintor,
    cache: &mut CacheHoja,
    libro: &Libro,
    hoja: &Hoja,
    rejilla: &Rejilla,
    vista: &Vista,
    caja: RectF,
    op: &OpcionesHoja,
) {
    cache.fotograma += 1;
    cache.imagenes.nuevo_fotograma();
    cache.medidas = Medidas::default();
    poner_oscuro(op.oscuro);
    let e = vista.escala.max(0.05);
    let celdas = zona_de_celdas(hoja, rejilla, vista, caja, op.encabezados);
    let v = Vista {
        ancho: celdas.ancho as f64,
        alto: celdas.alto as f64,
        ..*vista
    };
    llenar(
        p,
        rect(caja.x, caja.y, caja.x + caja.ancho, caja.y + caja.alto),
        papel(),
    );
    let paneles = rejilla.paneles(&v);
    for panel in &paneles {
        let m = Mapa::de(panel, celdas, e);
        let recorte = RectF {
            x: m.ox as f32,
            y: m.oy as f32,
            ancho: panel.pantalla[2] as f32,
            alto: panel.pantalla[3] as f32,
        };
        p.empujar_recorte(recorte);
        pintar_panel(p, cache, libro, hoja, rejilla, panel, &m, recorte);
        p.soltar_recorte();
    }
    // Las lineas de los paneles inmovilizados.
    let (ff, cf) = (rejilla.filas_fijas, rejilla.columnas_fijas);
    if cf > 0 {
        let x = (celdas.x as f64 + rejilla.x(cf) * e).round() as f32;
        if x < celdas.x + celdas.ancho {
            llenar(
                p,
                rect(x - 1.0, celdas.y, x, celdas.y + celdas.alto),
                ui(LINEA_FIJA),
            );
        }
    }
    if ff > 0 {
        let y = (celdas.y as f64 + rejilla.y(ff) * e).round() as f32;
        if y < celdas.y + celdas.alto {
            llenar(
                p,
                rect(celdas.x, y - 1.0, celdas.x + celdas.ancho, y),
                ui(LINEA_FIJA),
            );
        }
    }
    if let Some(sel) = op.seleccion {
        p.empujar_recorte(celdas);
        for panel in &paneles {
            let m = Mapa::de(panel, celdas, e);
            let (x0, y0, x1, y1) = (
                m.sx(rejilla, sel.col0),
                m.sy(rejilla, sel.fila0),
                m.sx(rejilla, sel.col1 + 1),
                m.sy(rejilla, sel.fila1 + 1),
            );
            if let Some(b) = pincel(p, legible(SELECCION)) {
                // SAFETY: dentro del fotograma; objetos vivos.
                unsafe {
                    ctx(p).DrawRectangle(
                        &rect(x0 - 1.0, y0 - 1.0, x1 - 0.0, y1 - 0.0),
                        &b,
                        2.0,
                        None,
                    )
                };
            }
        }
        p.soltar_recorte();
    }
    pintar_flechas(p, hoja, rejilla, &paneles, celdas, e, op);
    pintar_marcas(p, hoja, rejilla, &paneles, celdas, e);
    if celdas.x > caja.x || celdas.y > caja.y {
        pintar_encabezados(
            p,
            cache,
            libro,
            rejilla,
            &paneles,
            celdas,
            caja,
            e * rejilla.ppp / 96.0,
            op.seleccion,
        );
    }
}

#[allow(clippy::too_many_arguments)]
/// Las tablas que tocan la zona, con su estilo resuelto.
fn tablas_visibles<'h>(
    libro: &Libro,
    hoja: &'h Hoja,
    f0: u32,
    f1: u32,
    c0: u32,
    c1: u32,
) -> Vec<(
    &'h pixpin_hojas::tablas::Tabla,
    pixpin_hojas::estilos::EstiloTabla,
)> {
    hoja.cola
        .tablas
        .iter()
        .filter(|t| {
            t.rango.fila1 >= f0 && t.rango.fila0 <= f1 && t.rango.col1 >= c0 && t.rango.col0 <= c1
        })
        .filter_map(|t| libro.estilo_de_tabla(t).map(|e| (t, e)))
        .collect()
}

/// Lo que la tabla y el formato condicional cambian del texto de una celda.
fn extra_de(
    libro: &Libro,
    hoja: &Hoja,
    fila: u32,
    c: &pixpin_hojas::Celda,
) -> (Option<pixpin_hojas::estilos::Dxf>, bool) {
    let mut d: Option<pixpin_hojas::estilos::Dxf> = None;
    for t in &hoja.cola.tablas {
        if t.rango.contiene(fila, c.col) {
            if let Some(e) = libro.estilo_de_tabla(t) {
                d = Some(pixpin_hojas::tablas::formato_en(t, &e, fila, c.col));
            }
            break;
        }
    }
    let mut ocultar = false;
    if let Some(ef) = libro.efecto_condicional(hoja, fila, c) {
        ocultar = ef.ocultar_valor;
        let mut base = d.unwrap_or_default();
        base.sobre(&ef.dxf);
        d = Some(base);
    }
    (d, ocultar)
}

fn color_de(c: Rgb) -> Color {
    color(c)
}

#[allow(clippy::too_many_arguments)]
fn pintar_panel(
    p: &Pintor,
    cache: &mut CacheHoja,
    libro: &Libro,
    hoja: &Hoja,
    rejilla: &Rejilla,
    panel: &Panel,
    m: &Mapa,
    zona: RectF,
) {
    let (f0, f1, c0, c1) = (panel.fila0, panel.fila1, panel.col0, panel.col1);
    let est = &libro.estilos;
    let combinadas: Vec<Rango> = hoja.combinadas_en(f0, f1, c0, c1).copied().collect();
    let en_combinada = |f: u32, c: u32| combinadas.iter().find(|r| r.contiene(f, c)).copied();
    let filas = hoja.filas_entre(f0, f1);
    let t0 = std::time::Instant::now();
    // 1. Los textos (antes que nada: deciden que lineas de la cuadricula
    // tapa un texto que se desborda).
    let mut trabajos: Vec<Trabajo> = Vec::new();
    let mut borrar: Vec<D2D_RECT_F> = Vec::new();
    for f in filas {
        if rejilla.alto(f.indice) <= 0.0 {
            continue;
        }
        let cs = hoja.celdas_de(f);
        let ini = cs.partition_point(|c| c.col < c0);
        let fin = cs.partition_point(|c| c.col <= c1);
        // La ultima con algo antes de la zona y la primera despues: pueden
        // desbordarse hasta aqui.
        let antes = cs[..ini].iter().rev().find(|c| c.valor != Valor::Vacio);
        let despues = cs[fin..].iter().find(|c| c.valor != Valor::Vacio);
        for c in antes.into_iter().chain(cs[ini..fin].iter()).chain(despues) {
            cache.medidas.celdas += 1;
            if c.valor == Valor::Vacio
                || en_combinada(f.indice, c.col).is_some()
                || hoja.combinada_en(f.indice, c.col).is_some()
            {
                continue;
            }
            let caja = Rango {
                fila0: f.indice,
                col0: c.col,
                fila1: f.indice,
                col1: c.col,
            };
            texto_cacheado(
                p,
                cache,
                libro,
                hoja,
                rejilla,
                m,
                c,
                caja,
                false,
                &mut trabajos,
                &mut borrar,
            );
        }
    }
    for r in &combinadas {
        if let Some(c) = hoja.celda(r.fila0, r.col0) {
            texto_cacheado(
                p,
                cache,
                libro,
                hoja,
                rejilla,
                m,
                c,
                *r,
                true,
                &mut trabajos,
                &mut borrar,
            );
        }
    }
    cache.medidas.us_preparar_textos += t0.elapsed().as_micros() as u32;
    // 2. Cuadricula.
    let (x_ini, x_fin) = (zona.x, zona.x + zona.ancho);
    let (y_ini, y_fin) = (zona.y, zona.y + zona.alto);
    if hoja.vista().cuadricula {
        if let Some(b) = pincel(p, ui(CUADRICULA)) {
            for c in c0..=c1 {
                if rejilla.ancho(c) <= 0.0 {
                    continue;
                }
                let x = m.sx(rejilla, c + 1);
                // SAFETY: dentro del fotograma.
                unsafe { ctx(p).FillRectangle(&rect(x - 1.0, y_ini, x, y_fin), &b) };
            }
            let ultima = f1.min(f0 + 2000);
            for f in f0..=ultima {
                if rejilla.alto(f) <= 0.0 {
                    continue;
                }
                let y = m.sy(rejilla, f + 1);
                // SAFETY: dentro del fotograma.
                unsafe { ctx(p).FillRectangle(&rect(x_ini, y - 1.0, x_fin, y), &b) };
            }
        }
        for r in &combinadas {
            let (x0, y0, x1, y1) = (
                m.sx(rejilla, r.col0),
                m.sy(rejilla, r.fila0),
                m.sx(rejilla, r.col1 + 1),
                m.sy(rejilla, r.fila1 + 1),
            );
            llenar(p, rect(x0, y0, x1 - 1.0, y1 - 1.0), papel());
        }
        for b in &borrar {
            llenar(p, *b, papel());
        }
    }
    // 3. Rellenos: columnas, filas, celdas, combinaciones.
    for col in &hoja.cabecera.columnas {
        if col.max < c0 || col.min > c1 {
            continue;
        }
        if let Relleno::Liso(color) = est.relleno(est.xf(col.xf)) {
            let (a, b) = (col.min.max(c0), col.max.min(c1));
            llenar(
                p,
                rect(m.sx(rejilla, a) - 1.0, y_ini, m.sx(rejilla, b + 1), y_fin),
                relleno_de(color),
            );
        }
    }
    for f in filas {
        let (y0, y1) = (m.sy(rejilla, f.indice), m.sy(rejilla, f.indice + 1));
        if y1 <= y0 {
            continue;
        }
        if let Some(x) = f.xf {
            if let Relleno::Liso(color) = est.relleno(est.xf(x)) {
                llenar(p, rect(x_ini, y0 - 1.0, x_fin, y1), relleno_de(color));
            }
        }
        let cs = hoja.celdas_de(f);
        let ini = cs.partition_point(|c| c.col < c0);
        let fin = cs.partition_point(|c| c.col <= c1);
        for c in &cs[ini..fin] {
            if c.xf == 0 {
                continue;
            }
            if let Relleno::Liso(color) = est.relleno(est.xf(c.xf)) {
                if en_combinada(f.indice, c.col).is_some() {
                    continue;
                }
                llenar(
                    p,
                    rect(
                        m.sx(rejilla, c.col) - 1.0,
                        y0 - 1.0,
                        m.sx(rejilla, c.col + 1),
                        y1,
                    ),
                    relleno_de(color),
                );
            }
        }
    }
    for r in &combinadas {
        let xf = hoja.xf_en(r.fila0, r.col0);
        if let Relleno::Liso(color) = est.relleno(est.xf(xf)) {
            llenar(
                p,
                rect(
                    m.sx(rejilla, r.col0) - 1.0,
                    m.sy(rejilla, r.fila0) - 1.0,
                    m.sx(rejilla, r.col1 + 1),
                    m.sy(rejilla, r.fila1 + 1),
                ),
                relleno_de(color),
            );
        }
    }
    // 3b. Tablas: su relleno donde la celda no trae uno propio (por debajo
    // del de la celda, como en Excel).
    let tablas = tablas_visibles(libro, hoja, f0, f1, c0, c1);
    for (t, e) in &tablas {
        let r = t.rango;
        for f in r.fila0.max(f0)..=r.fila1.min(f1) {
            let (y0, y1) = (m.sy(rejilla, f), m.sy(rejilla, f + 1));
            if y1 <= y0 {
                continue;
            }
            for c in r.col0.max(c0)..=r.col1.min(c1) {
                let Some(color) = pixpin_hojas::tablas::formato_en(t, e, f, c).relleno else {
                    continue;
                };
                if est.relleno(est.xf(hoja.xf_en(f, c))) != Relleno::Nada {
                    continue;
                }
                llenar(
                    p,
                    rect(m.sx(rejilla, c) - 1.0, y0 - 1.0, m.sx(rejilla, c + 1), y1),
                    relleno_de(color),
                );
            }
        }
    }
    // 3c. Formato condicional: relleno, escala de color y barras de datos.
    if !hoja.cola.condicionales.is_empty() {
        for f in filas {
            let (y0, y1) = (m.sy(rejilla, f.indice), m.sy(rejilla, f.indice + 1));
            if y1 <= y0 {
                continue;
            }
            let cs = hoja.celdas_de(f);
            let ini = cs.partition_point(|c| c.col < c0);
            let fin = cs.partition_point(|c| c.col <= c1);
            for c in &cs[ini..fin] {
                let Some(ef) = libro.efecto_condicional(hoja, f.indice, c) else {
                    continue;
                };
                // En una combinacion, toda su caja.
                let r = hoja.combinada_en(f.indice, c.col).unwrap_or(Rango {
                    fila0: f.indice,
                    col0: c.col,
                    fila1: f.indice,
                    col1: c.col,
                });
                let (x0, x1) = (m.sx(rejilla, r.col0), m.sx(rejilla, r.col1 + 1));
                let (y0, y1) = (m.sy(rejilla, r.fila0), m.sy(rejilla, r.fila1 + 1));
                if let Some(color) = ef.escala.or(ef.dxf.relleno) {
                    llenar(p, rect(x0 - 1.0, y0 - 1.0, x1, y1), relleno_de(color));
                }
                if let Some(b) = ef.barra {
                    let margen = (2.0 * m.e as f32).max(1.0);
                    let (ix0, iw) = (x0 + margen, x1 - x0 - 1.0 - 2.0 * margen);
                    let alto = (y1 - y0 - 1.0 - 2.0 * margen).max(1.0);
                    let r = RectF {
                        x: ix0 + iw * b.desde,
                        y: y0 + margen,
                        ancho: (iw * (b.hasta - b.desde)).max(1.0),
                        alto,
                    };
                    if b.degradado {
                        // Degradado del color al blanco, como las barras de 2007.
                        p.rect_degradado(
                            r,
                            (r.x, r.y),
                            (r.x + r.ancho, r.y),
                            color_de(b.color),
                            color_de(b.color.mezcla(Rgb::BLANCO, 0.1)),
                        );
                    } else {
                        llenar(p, rect(r.x, r.y, r.x + r.ancho, r.y + r.alto), b.color);
                    }
                    if let Some(bc) = b.borde {
                        p.trazar(
                            RectF {
                                x: r.x + 0.5,
                                y: r.y + 0.5,
                                ancho: (r.ancho - 1.0).max(1.0),
                                alto: (r.alto - 1.0).max(1.0),
                            },
                            1.0,
                            color_de(bc),
                        );
                    }
                    if let Some((pos, ce)) = b.eje {
                        let xe = (ix0 + iw * pos).round();
                        llenar(
                            p,
                            rect(xe, y0 + margen * 0.5, xe + 1.0, y1 - 1.0 - margen * 0.5),
                            ce,
                        );
                    }
                }
                if let Some((conj, idx)) = ef.icono {
                    let k = m.e as f32 * rejilla.ppp as f32 / 96.0;
                    let lado = ((y1 - y0 - 1.0) * 0.8).min(15.0 * k).max(4.0);
                    crate::hoja_iconos::pintar_icono(
                        p,
                        conj,
                        idx,
                        x0 + 2.0 * k + lado / 2.0,
                        (y0 + y1 - 1.0) / 2.0,
                        lado,
                    );
                }
            }
        }
    }
    pintar_minigraficos(p, libro, hoja, rejilla, m, f0, f1, c0, c1);
    // 4. Bordes: el mapa de lados con la linea mas fuerte de cada uno.
    let mut lados: HashMap<Lado, Linea> = HashMap::new();
    let mut diagonales: Vec<(u32, u32, Borde)> = Vec::new();
    let mut poner = |l: Lado, li: Linea| {
        if li.estilo == EstiloLinea::Ninguno {
            return;
        }
        let e = lados.entry(l).or_insert(li);
        if li > *e {
            *e = li;
        }
    };
    for f in hoja.filas_entre(f0.saturating_sub(1), f1.saturating_add(1)) {
        let cs = hoja.celdas_de(f);
        let ini = cs.partition_point(|c| c.col + 1 < c0);
        let fin = cs.partition_point(|c| c.col <= c1.saturating_add(1));
        for c in &cs[ini..fin] {
            if c.xf == 0 {
                continue;
            }
            let b = est.borde(est.xf(c.xf));
            let (r, k) = (f.indice, c.col);
            poner(
                Lado {
                    horizontal: true,
                    fila: r,
                    col: k,
                },
                b.arriba,
            );
            poner(
                Lado {
                    horizontal: true,
                    fila: r + 1,
                    col: k,
                },
                b.abajo,
            );
            poner(
                Lado {
                    horizontal: false,
                    fila: r,
                    col: k,
                },
                b.izquierda,
            );
            poner(
                Lado {
                    horizontal: false,
                    fila: r,
                    col: k + 1,
                },
                b.derecha,
            );
            if b.diagonal.estilo != EstiloLinea::Ninguno && (b.diagonal_sube || b.diagonal_baja) {
                diagonales.push((r, k, b));
            }
        }
    }
    // Los bordes de las tablas (los de la celda, si los tiene, ganan por
    // ser mas fuertes o iguales).
    for (t, e) in &tablas {
        let r = t.rango;
        for f in r.fila0.max(f0.saturating_sub(1))..=r.fila1.min(f1.saturating_add(1)) {
            for c in r.col0.max(c0.saturating_sub(1))..=r.col1.min(c1.saturating_add(1)) {
                let d = pixpin_hojas::tablas::formato_en(t, e, f, c);
                let lado = |o: Option<Linea>| o.unwrap_or_default();
                poner(
                    Lado {
                        horizontal: true,
                        fila: f,
                        col: c,
                    },
                    lado(d.arriba),
                );
                poner(
                    Lado {
                        horizontal: true,
                        fila: f + 1,
                        col: c,
                    },
                    lado(d.abajo),
                );
                poner(
                    Lado {
                        horizontal: false,
                        fila: f,
                        col: c,
                    },
                    lado(d.izquierda),
                );
                poner(
                    Lado {
                        horizontal: false,
                        fila: f,
                        col: c + 1,
                    },
                    lado(d.derecha),
                );
            }
        }
    }
    cache.medidas.bordes = lados.len() as u32;
    if !lados.is_empty() {
        // SAFETY: dentro del fotograma; se vuelve al modo normal al acabar.
        unsafe { ctx(p).SetAntialiasMode(D2D1_ANTIALIAS_MODE_ALIASED) };
        let mut lista: Vec<(Lado, Linea)> = lados.into_iter().collect();
        // Primero los finos: los gruesos tapan sus esquinas.
        lista.sort_by_key(|a| a.1);
        for (l, li) in lista {
            // Los lados de dentro de una combinacion no se pintan.
            let dentro = if l.horizontal {
                l.fila > 0
                    && combinadas
                        .iter()
                        .any(|r| r.contiene(l.fila - 1, l.col) && r.contiene(l.fila, l.col))
            } else {
                l.col > 0
                    && combinadas
                        .iter()
                        .any(|r| r.contiene(l.fila, l.col - 1) && r.contiene(l.fila, l.col))
            };
            if dentro {
                continue;
            }
            if l.horizontal {
                if rejilla.ancho(l.col) <= 0.0 {
                    continue;
                }
                let y = m.sy(rejilla, l.fila);
                linea_borde(
                    p,
                    cache,
                    true,
                    y,
                    m.sx(rejilla, l.col),
                    m.sx(rejilla, l.col + 1),
                    li,
                );
            } else {
                if rejilla.alto(l.fila) <= 0.0 {
                    continue;
                }
                let x = m.sx(rejilla, l.col);
                linea_borde(
                    p,
                    cache,
                    false,
                    x,
                    m.sy(rejilla, l.fila),
                    m.sy(rejilla, l.fila + 1),
                    li,
                );
            }
        }
        for (r, k, b) in diagonales {
            let (x0, y0, x1, y1) = (
                m.sx(rejilla, k),
                m.sy(rejilla, r),
                m.sx(rejilla, k + 1),
                m.sy(rejilla, r + 1),
            );
            if let Some(br) = pincel(p, b.diagonal.color) {
                let g = b.diagonal.estilo.grosor();
                // SAFETY: dentro del fotograma; objetos vivos.
                unsafe {
                    if b.diagonal_baja {
                        ctx(p).DrawLine(
                            Vector2 { X: x0, Y: y0 },
                            Vector2 { X: x1, Y: y1 },
                            &br,
                            g,
                            None,
                        );
                    }
                    if b.diagonal_sube {
                        ctx(p).DrawLine(
                            Vector2 { X: x0, Y: y1 },
                            Vector2 { X: x1, Y: y0 },
                            &br,
                            g,
                            None,
                        );
                    }
                }
            }
        }
        // SAFETY: vuelve al suavizado normal.
        unsafe { ctx(p).SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE) };
    }
    // 5. Textos.
    let t0 = std::time::Instant::now();
    cache.medidas.textos += trabajos.len() as u32;
    for t in &trabajos {
        dibujar_trabajo(p, t);
    }
    cache.medidas.us_dibujar_textos += t0.elapsed().as_micros() as u32;
    // 6. Dibujos (imagenes, graficos, formas).
    for d in &hoja.cola.dibujos {
        let [x, y, w, h] = rejilla.caja_dibujo(d);
        let (x0, y0, x1, y1) = (m.px(x), m.py(y), m.px(x + w), m.py(y + h));
        if x1 < zona.x
            || y1 < zona.y
            || x0 > zona.x + zona.ancho
            || y0 > zona.y + zona.alto
            || w <= 0.0
            || h <= 0.0
        {
            continue;
        }
        pintar_dibujo(
            p,
            cache,
            libro,
            d,
            RectF {
                x: x0,
                y: y0,
                ancho: x1 - x0,
                alto: y1 - y0,
            },
            (m.e * rejilla.ppp / 96.0) as f32,
        );
    }
}

/// Una linea de borde sobre la frontera `b` (pixel entero), de `a0` a `a1`.
fn linea_borde(
    p: &Pintor,
    cache: &mut CacheHoja,
    horizontal: bool,
    b: f32,
    a0: f32,
    a1: f32,
    li: Linea,
) {
    let Some(br) = pincel(p, raya(li.color)) else {
        return;
    };
    let g = li.estilo.grosor();
    let c = ctx(p);
    // Las solidas, como rectangulos exactos; la doble, dos de un pixel.
    let tramo = |desde: f32, hasta: f32, extra: f32| -> D2D_RECT_F {
        if horizontal {
            rect(a0 - 1.0 - extra, b + desde, a1 + extra, b + hasta)
        } else {
            rect(b + desde, a0 - 1.0 - extra, b + hasta, a1 + extra)
        }
    };
    match li.estilo {
        EstiloLinea::Fino => {
            // SAFETY: dentro del fotograma; objetos vivos.
            unsafe { c.FillRectangle(&tramo(-1.0, 0.0, 0.0), &br) }
        }
        EstiloLinea::Medio => {
            // SAFETY: idem.
            unsafe { c.FillRectangle(&tramo(-1.0, 1.0, 1.0), &br) }
        }
        EstiloLinea::Grueso => {
            // SAFETY: idem.
            unsafe { c.FillRectangle(&tramo(-2.0, 1.0, 1.0), &br) }
        }
        EstiloLinea::Doble => {
            // SAFETY: idem; el hueco del medio se deja blanco.
            unsafe {
                c.FillRectangle(&tramo(-2.0, 1.0, 1.0), &br);
            }
            llenar(p, tramo(-1.0, 0.0, -1.0), papel());
        }
        _ => {
            let estilo = cache.trazo(p.motor, li.estilo, g);
            let centro = if (g as i32) % 2 == 1 { b - 0.5 } else { b };
            let (p0, p1) = if horizontal {
                (
                    Vector2 {
                        X: a0 - 1.0,
                        Y: centro,
                    },
                    Vector2 { X: a1, Y: centro },
                )
            } else {
                (
                    Vector2 {
                        X: centro,
                        Y: a0 - 1.0,
                    },
                    Vector2 { X: centro, Y: a1 },
                )
            };
            // SAFETY: dentro del fotograma; objetos vivos.
            unsafe { c.DrawLine(p0, p1, &br, g, estilo.as_ref()) };
        }
    }
}

fn dibujar_trabajo(p: &Pintor, t: &Trabajo) {
    let Some(b) = pincel(p, t.color) else {
        return;
    };
    let c = ctx(p);
    let r = t.recorte;
    if t.giro.is_none()
        && t.medida.is_some_and(|(w, h)| {
            t.x >= r.left && t.x + w <= r.right && t.y >= r.top - 1.0 && t.y + h <= r.bottom + 1.0
        })
    {
        // SAFETY: dentro del fotograma; objetos vivos.
        unsafe {
            c.DrawTextLayout(
                Vector2 { X: t.x, Y: t.y },
                &t.disposicion,
                &b,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
            )
        };
        return;
    }
    // SAFETY: Push/Pop emparejados dentro del fotograma; la transformada se
    // guarda y se repone.
    unsafe {
        c.PushAxisAlignedClip(&t.recorte, D2D1_ANTIALIAS_MODE_ALIASED);
        match t.giro {
            None => c.DrawTextLayout(
                Vector2 { X: t.x, Y: t.y },
                &t.disposicion,
                &b,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
            ),
            Some((grados, cx, cy)) => {
                let mut antes = Matrix3x2::default();
                c.GetTransform(&mut antes);
                let a = -grados.to_radians();
                let (s, co) = a.sin_cos();
                let giro = Matrix3x2 {
                    M11: co,
                    M12: s,
                    M21: -s,
                    M22: co,
                    M31: cx,
                    M32: cy,
                };
                c.SetTransform(&(giro * antes));
                c.DrawTextLayout(
                    Vector2 { X: t.x, Y: t.y },
                    &t.disposicion,
                    &b,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                );
                c.SetTransform(&antes);
            }
        }
        c.PopAxisAlignedClip();
    }
}

/// Lo preparado de una celda, relativo a la esquina de su caja: se guarda
/// entre fotogramas y al desplazar solo se traslada.
#[derive(Clone)]
struct Preparada {
    trabajos: Vec<Trabajo>,
    borrar: Vec<D2D_RECT_F>,
}

fn mover(r: D2D_RECT_F, dx: f32, dy: f32) -> D2D_RECT_F {
    rect(r.left + dx, r.top + dy, r.right + dx, r.bottom + dy)
}

/// [`texto_de_celda`] con la cache de celdas de [`CacheHoja`]: el texto, su
/// formato, su disposicion y su desborde no cambian mientras no cambien la
/// hoja ni el zoom.
#[allow(clippy::too_many_arguments)]
fn texto_cacheado(
    p: &Pintor,
    cache: &mut CacheHoja,
    libro: &Libro,
    hoja: &Hoja,
    rejilla: &Rejilla,
    m: &Mapa,
    c: &pixpin_hojas::Celda,
    caja: Rango,
    combinada: bool,
    trabajos: &mut Vec<Trabajo>,
    borrar: &mut Vec<D2D_RECT_F>,
) {
    // El modo oscuro cambia el color de los textos: tambien es de la llave.
    let llave = (
        libro as *const Libro as usize,
        hoja as *const Hoja as usize,
        hoja.version,
        m.e.to_bits() ^ rejilla.ppp.to_bits().rotate_left(17) ^ ((oscuro() as u64) << 63),
    );
    if cache.llave_celdas != llave {
        cache.llave_celdas = llave;
        cache.celdas.clear();
    }
    let (bx, by) = (m.sx(rejilla, caja.col0), m.sy(rejilla, caja.fila0));
    let k = (caja.fila0, caja.col0);
    if let Some(pr) = cache.celdas.get(&k) {
        for t in &pr.trabajos {
            let giro = t.giro.map(|(g, cx, cy)| (g, cx + bx, cy + by));
            let (x, y) = if t.giro.is_some() {
                (t.x, t.y)
            } else {
                (t.x + bx, t.y + by)
            };
            trabajos.push(Trabajo {
                disposicion: t.disposicion.clone(),
                x,
                y,
                recorte: mover(t.recorte, bx, by),
                color: t.color,
                giro,
                medida: t.medida,
            });
        }
        borrar.extend(pr.borrar.iter().map(|r| mover(*r, bx, by)));
        return;
    }
    let (n_t, n_b) = (trabajos.len(), borrar.len());
    texto_de_celda(
        p, cache, libro, hoja, rejilla, m, c, caja, combinada, trabajos, borrar,
    );
    let pr = Preparada {
        trabajos: trabajos[n_t..]
            .iter()
            .map(|t| Trabajo {
                disposicion: t.disposicion.clone(),
                x: if t.giro.is_some() { t.x } else { t.x - bx },
                y: if t.giro.is_some() { t.y } else { t.y - by },
                recorte: mover(t.recorte, -bx, -by),
                color: t.color,
                giro: t.giro.map(|(g, cx, cy)| (g, cx - bx, cy - by)),
                medida: t.medida,
            })
            .collect(),
        borrar: borrar[n_b..].iter().map(|r| mover(*r, -bx, -by)).collect(),
    };
    if cache.celdas.len() >= 50_000 {
        cache.celdas.clear();
    }
    cache.celdas.insert(k, pr);
}

/// Prepara el texto de una celda (o combinacion `caja`).
#[allow(clippy::too_many_arguments)]
fn texto_de_celda(
    p: &Pintor,
    cache: &mut CacheHoja,
    libro: &Libro,
    hoja: &Hoja,
    rejilla: &Rejilla,
    m: &Mapa,
    c: &pixpin_hojas::Celda,
    caja: Rango,
    combinada: bool,
    trabajos: &mut Vec<Trabajo>,
    borrar: &mut Vec<D2D_RECT_F>,
) {
    let Some(mut cont) = libro.contenido(hoja, c) else {
        return;
    };
    if cont.mostrado.texto.is_empty() {
        return;
    }
    let xf = libro.estilos.xf(c.xf);
    let al = xf.alineacion;
    let mut fuente = libro.estilos.fuente(xf).clone();
    // Lo que ponen la tabla y el formato condicional.
    let (extra, ocultar) = extra_de(libro, hoja, caja.fila0, c);
    if ocultar {
        return;
    }
    if let Some(d) = &extra {
        fuente.negrita = d.negrita.unwrap_or(fuente.negrita);
        fuente.cursiva = d.cursiva.unwrap_or(fuente.cursiva);
        fuente.tachado = d.tachado.unwrap_or(fuente.tachado);
        if let Some(s) = d.subrayado {
            fuente.subrayado = if s {
                Subrayado::Simple
            } else {
                Subrayado::Ninguno
            };
        }
    }
    let zoom = m.e as f32;
    let e = (m.e * rejilla.ppp / 96.0) as f32;
    let tam = fuente.tam * 96.0 / 72.0 * e;
    let (x0, y0, x1, y1) = (
        m.sx(rejilla, caja.col0),
        m.sy(rejilla, caja.fila0),
        m.sx(rejilla, caja.col1 + 1),
        m.sy(rejilla, caja.fila1 + 1),
    );
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let margen = (2.0 * e).round().max(1.0);
    let ancho_util = (x1 - x0 - 1.0 - 2.0 * margen).max(1.0);
    let sangria = al.sangria as f32 * (rejilla.mdw as f32 + 2.0) * zoom;
    let mut color_txt = libro.color_texto(xf, &cont.mostrado);
    if let Some(d) = &extra {
        if let (None, Some(cf)) = (cont.mostrado.color, d.color_fuente) {
            color_txt = cf;
        }
    }
    // Modo oscuro: sin relleno debajo el papel es oscuro y el texto oscuro
    // (el «automatico») se aclara; con relleno propio se queda como lo puso
    // su autor, que ya se lee sobre el.
    if oscuro() {
        // El relleno blanco no cuenta: en oscuro es papel.
        let con_relleno = matches!(libro.estilos.relleno(xf), Relleno::Liso(c) if !casi_blanco(c))
            || extra
                .as_ref()
                .is_some_and(|d| d.relleno.is_some_and(|c| !casi_blanco(c)))
            || libro
                .efecto_condicional(hoja, caja.fila0, c)
                .is_some_and(|ef| ef.escala.is_some());
        if !con_relleno {
            color_txt = legible(color_txt);
            for t in cont.tramos.iter_mut().flatten() {
                t.fuente.color = t.fuente.color.map(legible);
            }
        }
    }
    let ajustar = (al.ajustar
        || matches!(
            al.horizontal,
            Horizontal::Justificar | Horizontal::Distribuido
        )
        || al.vertical == Vertical::Justificar)
        && al.giro == 0;
    let alinear = match cont.horizontal {
        Horizontal::Centro | Horizontal::CentroSeleccion => DWRITE_TEXT_ALIGNMENT_CENTER,
        Horizontal::Derecha => DWRITE_TEXT_ALIGNMENT_TRAILING,
        Horizontal::Justificar | Horizontal::Distribuido => DWRITE_TEXT_ALIGNMENT_JUSTIFIED,
        _ => DWRITE_TEXT_ALIGNMENT_LEADING,
    };
    let mut texto = std::mem::take(&mut cont.mostrado.texto);
    if al.giro == 255 {
        // Apilado: una letra por linea.
        texto = texto
            .chars()
            .flat_map(|ch| [ch, '\n'])
            .collect::<String>()
            .trim_end()
            .to_string();
    }
    let tramos = cont.tramos.as_deref().filter(|_| al.giro == 0);
    let escala_tramos = 96.0 / 72.0 * e;
    let ajuste = if ajustar {
        Some((ancho_util - sangria).max(1.0))
    } else {
        None
    };
    let Some((mut d, mut w, mut h)) = cache.disposicion(
        p.motor,
        &texto,
        &fuente,
        tam,
        ajuste,
        alinear,
        tramos,
        escala_tramos,
    ) else {
        return;
    };
    // Un numero que no cabe: «###» (General recorta decimales antes).
    let almohadillas = !texto.is_empty() && texto.chars().all(|c| c == '#');
    if cont.clase == pixpin_hojas::Clase::Numero
        && !ajustar
        && al.giro == 0
        && (w > ancho_util + 0.5 || almohadillas)
    {
        let mut nuevo = None;
        if xf.formato.es_general {
            if let Valor::Numero(v) = c.valor {
                let por_letra = w / texto.chars().count().max(1) as f32;
                let caben = (ancho_util / por_letra).floor() as usize;
                if caben >= 1 {
                    let t = pixpin_hojas::formato::general_en(v, &libro.regional, caben);
                    if t.chars().count() <= caben && !t.is_empty() {
                        nuevo = Some(t);
                    }
                }
            }
        }
        let t = match nuevo {
            Some(t) => t,
            None => {
                let ancho_almohadilla = (tam * 0.5).max(1.0);
                "#".repeat(((ancho_util / ancho_almohadilla).floor() as usize).max(1))
            }
        };
        texto = t;
        cont.mostrado.relleno = None;
        let Some((d2, w2, h2)) = cache.disposicion(
            p.motor,
            &texto,
            &fuente,
            tam,
            None,
            alinear,
            None,
            escala_tramos,
        ) else {
            return;
        };
        (d, w, h) = (d2, w2, h2);
    }
    // Encoger hasta que quepa.
    if al.encoger && !ajustar && w > ancho_util {
        let t2 = tam * (ancho_util / w).max(0.1);
        if let Some((d2, w2, h2)) = cache.disposicion(
            p.motor,
            &texto,
            &fuente,
            t2,
            None,
            alinear,
            None,
            escala_tramos,
        ) {
            (d, w, h) = (d2, w2, h2);
        }
    }
    // Relleno (`Horizontal::Rellenar`): repetir hasta llenar.
    if cont.horizontal == Horizontal::Rellenar && w > 0.0 && w < ancho_util {
        let n = (ancho_util / w).floor().max(1.0) as usize;
        let t = texto.repeat(n);
        if let Some((d2, w2, h2)) = cache.disposicion(
            p.motor,
            &t,
            &fuente,
            tam,
            None,
            alinear,
            None,
            escala_tramos,
        ) {
            (d, w, h) = (d2, w2, h2);
        }
    }
    // Formato contable (`*`): lo de antes del relleno a la izquierda y el
    // resto a la derecha. Se pinta en dos trozos.
    if let Some(corte) = cont
        .mostrado
        .relleno
        .filter(|&k| k > 0 && k < texto.len() && !ajustar && al.giro == 0)
    {
        let izq = texto[..corte].to_string();
        let der = texto[corte..].to_string();
        let caja_y = |hh: f32| match al.vertical {
            Vertical::Arriba => y0 + margen * 0.5,
            Vertical::Centro | Vertical::Distribuido | Vertical::Justificar => {
                y0 + ((y1 - y0 - 1.0) - hh) / 2.0
            }
            Vertical::Abajo => y1 - 1.0 - hh - (e * 1.0).round(),
        };
        let recorte = rect(x0, y0, x1 - 1.0, y1 - 1.0);
        if let Some((di, _, hi)) = cache.disposicion(
            p.motor,
            &izq,
            &fuente,
            tam,
            None,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            None,
            escala_tramos,
        ) {
            trabajos.push(Trabajo {
                disposicion: di,
                x: x0 + margen,
                y: caja_y(hi),
                recorte,
                color: color_txt,
                giro: None,
                medida: None,
            });
        }
        if let Some((dd, wd, hd)) = cache.disposicion(
            p.motor,
            &der,
            &fuente,
            tam,
            None,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            None,
            escala_tramos,
        ) {
            trabajos.push(Trabajo {
                disposicion: dd,
                x: x1 - 1.0 - margen - wd,
                y: caja_y(hd),
                recorte,
                color: color_txt,
                giro: None,
                medida: None,
            });
        }
        return;
    }
    // Giro.
    if let Some(grados) = al.grados().filter(|g| *g != 0.0) {
        let a = grados.to_radians();
        let (bw, bh) = (
            w * a.cos().abs() + h * a.sin().abs(),
            w * a.sin().abs() + h * a.cos().abs(),
        );
        let cx = match cont.horizontal {
            Horizontal::Izquierda => x0 + margen + bw / 2.0,
            Horizontal::Derecha => x1 - 1.0 - margen - bw / 2.0,
            Horizontal::General if grados > 0.0 => x1 - 1.0 - margen - bw / 2.0,
            Horizontal::General => x0 + margen + bw / 2.0,
            _ => (x0 + x1 - 1.0) / 2.0,
        };
        let cy = match al.vertical {
            Vertical::Arriba => y0 + margen + bh / 2.0,
            Vertical::Centro | Vertical::Distribuido | Vertical::Justificar => {
                (y0 + y1 - 1.0) / 2.0
            }
            Vertical::Abajo => y1 - 1.0 - margen - bh / 2.0,
        };
        trabajos.push(Trabajo {
            disposicion: d,
            x: -w / 2.0,
            y: -h / 2.0,
            recorte: rect(x0, y0, x1 - 1.0, y1 - 1.0),
            color: color_txt,
            giro: Some((grados, cx, cy)),
            medida: None,
        });
        return;
    }
    // Posicion vertical.
    let alto_celda = y1 - y0 - 1.0;
    let y = match al.vertical {
        Vertical::Arriba => y0 + (e * 1.0).round(),
        Vertical::Centro | Vertical::Distribuido | Vertical::Justificar => {
            y0 + ((alto_celda - h) / 2.0).round()
        }
        Vertical::Abajo => y1 - 1.0 - h - (e * 1.0).round().min(((alto_celda - h) / 2.0).max(0.0)),
    };
    // Posicion horizontal y desborde.
    let mut rx0 = x0;
    let mut rx1 = x1 - 1.0;
    let x;
    if ajustar {
        // La disposicion ya alinea dentro de su ancho.
        x = x0
            + margen
            + if alinear == DWRITE_TEXT_ALIGNMENT_LEADING {
                sangria
            } else {
                0.0
            };
    } else {
        let sobra = w - ancho_util + sangria;
        let puede_desbordar = !combinada
            && sobra > 0.0
            && cont.clase == pixpin_hojas::Clase::Texto
            && cont.horizontal != Horizontal::Rellenar;
        let fila = caja.fila0;
        let mut col_izq = caja.col0;
        let mut col_der = caja.col1;
        match cont.horizontal {
            Horizontal::Derecha => {
                if puede_desbordar {
                    let mut falta = sobra;
                    while falta > 0.0 && col_izq > 0 && Rejilla::libre(hoja, fila, col_izq - 1) {
                        col_izq -= 1;
                        falta -= (rejilla.ancho(col_izq) * m.e) as f32;
                    }
                }
                x = x1 - 1.0 - margen - w - sangria;
            }
            Horizontal::Centro | Horizontal::CentroSeleccion => {
                let mut der = x1;
                if cont.horizontal == Horizontal::CentroSeleccion {
                    // Centrado en la seleccion: hasta donde sigan vacias y
                    // con el mismo centrado.
                    while col_der + 1 < pixpin_hojas::rejilla::MAX_COLUMNAS
                        && Rejilla::libre(hoja, fila, col_der + 1)
                        && libro
                            .estilos
                            .xf(hoja.xf_en(fila, col_der + 1))
                            .alineacion
                            .horizontal
                            == Horizontal::CentroSeleccion
                    {
                        col_der += 1;
                    }
                    der = m.sx(rejilla, col_der + 1);
                }
                let centro = (x0 + der - 1.0) / 2.0;
                if puede_desbordar {
                    let mut falta = (w - (der - x0)) / 2.0;
                    while falta > 0.0
                        && col_izq > 0
                        && Rejilla::libre(hoja, fila, col_izq - 1)
                        && Rejilla::libre(hoja, fila, col_der + 1)
                    {
                        col_izq -= 1;
                        col_der += 1;
                        falta -= (rejilla.ancho(col_izq).min(rejilla.ancho(col_der)) * m.e) as f32;
                    }
                }
                x = centro - w / 2.0;
            }
            _ => {
                if puede_desbordar {
                    let mut falta = sobra;
                    while falta > 0.0
                        && col_der + 1 < pixpin_hojas::rejilla::MAX_COLUMNAS
                        && Rejilla::libre(hoja, fila, col_der + 1)
                    {
                        col_der += 1;
                        falta -= (rejilla.ancho(col_der) * m.e) as f32;
                    }
                }
                x = x0 + margen + sangria;
            }
        }
        rx0 = m.sx(rejilla, col_izq);
        rx1 = m.sx(rejilla, col_der + 1) - 1.0;
        if col_izq < caja.col0 || col_der > caja.col1 {
            // La cuadricula que cruza el texto desbordado no se ve.
            let (ya, yb) = (y0, y1 - 1.0);
            let primera = col_izq;
            for k in primera..col_der {
                let bx = m.sx(rejilla, k + 1);
                let tapa = ((x as f64) < bx as f64) && ((x + w) as f64 > (bx - 1.0) as f64);
                if tapa {
                    borrar.push(rect(bx - 1.0, ya, bx, yb));
                }
            }
        }
    }
    trabajos.push(Trabajo {
        disposicion: d,
        x,
        y,
        recorte: rect(rx0, y0, rx1, y1 - 1.0),
        color: color_txt,
        giro: None,
        medida: Some((w, h)),
    });
}

/// Un dibujo (imagen, grafico o forma) en su caja de pantalla.
fn pintar_dibujo(p: &Pintor, cache: &mut CacheHoja, libro: &Libro, d: &Dibujo, r: RectF, e: f32) {
    match &d.contenido {
        ContenidoDibujo::Imagen { ruta, recorte } => match cache.imagen(p.motor, libro, ruta) {
            Some(b) => {
                // SAFETY: GetSize de un bitmap vivo no tiene precondiciones.
                let tam = unsafe { b.GetSize() };
                let [l, t, rr, bb] = *recorte;
                let fuente = if l != 0.0 || t != 0.0 || rr != 0.0 || bb != 0.0 {
                    Some(D2D_RECT_F {
                        left: tam.width * l,
                        top: tam.height * t,
                        right: tam.width * (1.0 - rr),
                        bottom: tam.height * (1.0 - bb),
                    })
                } else {
                    None
                };
                // SAFETY: dentro del fotograma; el bitmap es de este motor.
                unsafe {
                    ctx(p).DrawBitmap(
                        &b,
                        Some(&rect(r.x, r.y, r.x + r.ancho, r.y + r.alto)),
                        1.0,
                        D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC,
                        fuente.as_ref().map(|f| f as *const _),
                        None,
                    )
                };
            }
            None => {
                llenar(
                    p,
                    rect(r.x, r.y, r.x + r.ancho, r.y + r.alto),
                    Rgb {
                        r: 0xF2,
                        g: 0xF2,
                        b: 0xF2,
                    },
                );
                p.trazar(
                    r,
                    1.0,
                    color(Rgb {
                        r: 0xBF,
                        g: 0xBF,
                        b: 0xBF,
                    }),
                );
            }
        },
        ContenidoDibujo::Forma {
            texto,
            relleno,
            linea,
            alinear,
            vertical,
        } => {
            if let Some(c) = relleno {
                llenar(p, rect(r.x, r.y, r.x + r.ancho, r.y + r.alto), *c);
            }
            if let Some(c) = linea {
                p.trazar(r, 1.0 * e.max(1.0), color(*c));
            }
            if !texto.is_empty() {
                let f = Fuente {
                    tam: 11.0,
                    ..libro.estilos.fuente_normal.clone()
                };
                let al = match alinear {
                    1 => DWRITE_TEXT_ALIGNMENT_CENTER,
                    2 => DWRITE_TEXT_ALIGNMENT_TRAILING,
                    _ => DWRITE_TEXT_ALIGNMENT_LEADING,
                };
                let margen = 7.0 * e;
                if let Some((dd, _, hh)) = cache.disposicion(
                    p.motor,
                    texto,
                    &f,
                    11.0 * 96.0 / 72.0 * e,
                    Some((r.ancho - 2.0 * margen).max(1.0)),
                    al,
                    None,
                    1.0,
                ) {
                    let txt =
                        if relleno.is_some_and(|c| (c.r as u32 + c.g as u32 + c.b as u32) < 300) {
                            Rgb::BLANCO
                        } else {
                            Rgb::NEGRO
                        };
                    let y = match vertical {
                        1 => r.y + (r.alto - hh) / 2.0,
                        2 => r.y + r.alto - hh - 4.0 * e,
                        _ => r.y + 4.0 * e,
                    };
                    dibujar_trabajo(
                        p,
                        &Trabajo {
                            disposicion: dd,
                            x: r.x + margen,
                            y,
                            recorte: rect(r.x, r.y, r.x + r.ancho, r.y + r.alto),
                            color: txt,
                            giro: None,
                            medida: None,
                        },
                    );
                }
            }
        }
        ContenidoDibujo::Grafico {
            titulo,
            tipo,
            categorias,
            series,
        } => {
            pintar_grafico(p, cache, libro, r, e, titulo, tipo, categorias, series);
        }
    }
}

/// Un grafico sencillo con los datos que el propio libro guarda en cache:
/// columnas, barras, lineas, areas, dispersion y tarta. No pretende ser el
/// grafico de Excel, sino que se reconozca lo que es.
#[allow(clippy::too_many_arguments)]
fn pintar_grafico(
    p: &Pintor,
    cache: &mut CacheHoja,
    libro: &Libro,
    r: RectF,
    e: f32,
    titulo: &str,
    tipo: &str,
    categorias: &[String],
    series: &[pixpin_hojas::dibujos::Serie],
) {
    let gris = Rgb {
        r: 0x59,
        g: 0x59,
        b: 0x59,
    };
    let gris_claro = Rgb {
        r: 0xD9,
        g: 0xD9,
        b: 0xD9,
    };
    llenar(p, rect(r.x, r.y, r.x + r.ancho, r.y + r.alto), Rgb::BLANCO);
    p.trazar(
        RectF {
            x: r.x + 0.5,
            y: r.y + 0.5,
            ancho: r.ancho - 1.0,
            alto: r.alto - 1.0,
        },
        1.0,
        color(gris_claro),
    );
    p.empujar_recorte(r);
    let normal = libro.estilos.fuente_normal.clone();
    let mut y = r.y + 6.0 * e;
    let titulo = if titulo.is_empty() && series.is_empty() {
        "Gráfico"
    } else {
        titulo
    };
    if !titulo.is_empty() {
        let f = Fuente {
            tam: 14.0,
            ..normal.clone()
        };
        if let Some((d, w, h)) = cache.disposicion(
            p.motor,
            titulo,
            &f,
            14.0 * 96.0 / 72.0 * e,
            None,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            None,
            1.0,
        ) {
            dibujar_trabajo(
                p,
                &Trabajo {
                    disposicion: d,
                    x: r.x + (r.ancho - w) / 2.0,
                    y,
                    recorte: rect(r.x, r.y, r.x + r.ancho, r.y + r.alto),
                    color: gris,
                    giro: None,
                    medida: None,
                },
            );
            y += h + 4.0 * e;
        }
    }
    let pequeno = Fuente { tam: 9.0, ..normal };
    let tam_p = 9.0 * 96.0 / 72.0 * e;
    let es_tarta = tipo.contains("pie") || tipo.contains("doughnut");
    // Leyenda abajo.
    let alto_leyenda = if series.len() > 1 || es_tarta {
        16.0 * e
    } else {
        0.0
    };
    let zona = RectF {
        x: r.x + 8.0 * e,
        y,
        ancho: r.ancho - 16.0 * e,
        alto: (r.y + r.alto - alto_leyenda - 6.0 * e) - y,
    };
    if zona.ancho <= 10.0 || zona.alto <= 10.0 {
        p.soltar_recorte();
        return;
    }
    let colores: Vec<Rgb> = (0..series.len().max(categorias.len()).max(1))
        .map(|i| {
            series
                .get(i)
                .and_then(|s| s.color)
                .unwrap_or(libro.tema.colores[4 + i % 6])
        })
        .collect();
    if es_tarta {
        if let Some(s) = series.first() {
            let total: f64 = s
                .valores
                .iter()
                .filter(|v| v.is_finite() && **v > 0.0)
                .sum();
            if total > 0.0 {
                let radio = (zona.ancho.min(zona.alto) / 2.0 - 2.0).max(1.0);
                let (cx, cy) = (zona.x + zona.ancho / 2.0, zona.y + zona.alto / 2.0);
                let mut a0 = -std::f32::consts::FRAC_PI_2;
                for (i, v) in s.valores.iter().enumerate() {
                    if !v.is_finite() || *v <= 0.0 {
                        continue;
                    }
                    let a1 = a0 + (*v / total) as f32 * std::f32::consts::TAU;
                    let pasos = (((a1 - a0) / 0.05).ceil() as usize).max(2);
                    let mut pts = vec![(cx, cy)];
                    for k in 0..=pasos {
                        let a = a0 + (a1 - a0) * k as f32 / pasos as f32;
                        pts.push((cx + radio * a.cos(), cy + radio * a.sin()));
                    }
                    p.poligono(&pts, color(libro.tema.colores[4 + i % 6]));
                    a0 = a1;
                }
            }
        }
    } else {
        let mut min = 0.0f64;
        let mut max = 0.0f64;
        for s in series {
            for v in s.valores.iter().filter(|v| v.is_finite()) {
                min = min.min(*v);
                max = max.max(*v);
            }
        }
        if max <= min {
            max = min + 1.0;
        }
        let paso = paso_bonito((max - min) / 5.0);
        let min = (min / paso).floor() * paso;
        let max = (max / paso).ceil() * paso;
        // Etiquetas del eje.
        let mut ancho_eje = 0.0f32;
        let mut etiquetas = Vec::new();
        let mut v = min;
        while v <= max + paso * 0.01 {
            let t = pixpin_hojas::formato::general(v, &libro.regional);
            if let Some((d, w, h)) = cache.disposicion(
                p.motor,
                &t,
                &pequeno,
                tam_p,
                None,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                None,
                1.0,
            ) {
                ancho_eje = ancho_eje.max(w);
                etiquetas.push((v, d, w, h));
            }
            v += paso;
        }
        let alto_cat = if categorias.is_empty() {
            0.0
        } else {
            tam_p * 1.6
        };
        let plot = RectF {
            x: zona.x + ancho_eje + 6.0 * e,
            y: zona.y + 4.0 * e,
            ancho: zona.ancho - ancho_eje - 6.0 * e,
            alto: zona.alto - alto_cat - 4.0 * e,
        };
        if plot.ancho > 4.0 && plot.alto > 4.0 {
            let a_y = |v: f64| plot.y + plot.alto - ((v - min) / (max - min)) as f32 * plot.alto;
            for (v, d, w, h) in etiquetas {
                let yy = a_y(v).round();
                llenar(
                    p,
                    rect(plot.x, yy - 0.5, plot.x + plot.ancho, yy + 0.5),
                    gris_claro,
                );
                dibujar_trabajo(
                    p,
                    &Trabajo {
                        disposicion: d,
                        x: plot.x - 4.0 * e - w,
                        y: yy - h / 2.0,
                        recorte: rect(r.x, r.y, r.x + r.ancho, r.y + r.alto),
                        color: gris,
                        giro: None,
                        medida: None,
                    },
                );
            }
            let n = series
                .iter()
                .map(|s| s.valores.len())
                .max()
                .unwrap_or(0)
                .max(categorias.len())
                .max(1);
            let horizontal_barras = tipo == "barHChart";
            let barras = tipo.contains("bar") || tipo.contains("Bar");
            let ancho_cat = plot.ancho / n as f32;
            for (k, s) in series.iter().enumerate() {
                let c = colores[k % colores.len()];
                if barras && !horizontal_barras {
                    let nb = series.len().max(1) as f32;
                    let ancho_barra = ancho_cat * 0.7 / nb;
                    for (i, v) in s.valores.iter().enumerate() {
                        if !v.is_finite() {
                            continue;
                        }
                        let x = plot.x
                            + ancho_cat * i as f32
                            + ancho_cat * 0.15
                            + ancho_barra * k as f32;
                        let (ya, yb) = (a_y(0.0f64.clamp(min, max)), a_y(*v));
                        llenar(p, rect(x, ya.min(yb), x + ancho_barra, ya.max(yb)), c);
                    }
                } else if barras {
                    // Barras horizontales: categorias en vertical.
                    let alto_cat = plot.alto / n as f32;
                    let nb = series.len().max(1) as f32;
                    let alto_barra = alto_cat * 0.7 / nb;
                    for (i, v) in s.valores.iter().enumerate() {
                        if !v.is_finite() {
                            continue;
                        }
                        let yb = plot.y + plot.alto - alto_cat * (i as f32 + 1.0)
                            + alto_cat * 0.15
                            + alto_barra * k as f32;
                        let a_x = |v: f64| plot.x + ((v - min) / (max - min)) as f32 * plot.ancho;
                        let (xa, xb) = (a_x(0.0f64.clamp(min, max)), a_x(*v));
                        llenar(p, rect(xa.min(xb), yb, xa.max(xb), yb + alto_barra), c);
                    }
                } else {
                    let pts: Vec<(f32, f32)> = s
                        .valores
                        .iter()
                        .enumerate()
                        .filter(|(_, v)| v.is_finite())
                        .map(|(i, v)| (plot.x + ancho_cat * (i as f32 + 0.5), a_y(*v)))
                        .collect();
                    if tipo.contains("scatter") {
                        for (x, y) in &pts {
                            p.circulo((*x, *y), 3.0 * e, color(c));
                        }
                    } else if pts.len() >= 2 {
                        if tipo.contains("area") {
                            let mut poli = pts.clone();
                            let base = a_y(0.0f64.clamp(min, max));
                            poli.push((pts[pts.len() - 1].0, base));
                            poli.push((pts[0].0, base));
                            p.poligono(&poli, color(c));
                        } else {
                            p.polilinea(&pts, 2.25 * e, color(c));
                        }
                    }
                }
            }
            // Categorias.
            // Una de cada tantas etiquetas, para que no se pisen.
            let mut max_w = 0.0f32;
            for t in categorias.iter().take(200) {
                if let Some((_, w, _)) = cache.disposicion(
                    p.motor,
                    t,
                    &pequeno,
                    tam_p,
                    None,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                    None,
                    1.0,
                ) {
                    max_w = max_w.max(w);
                }
            }
            let paso_cat = ((max_w + 6.0 * e) / ancho_cat.max(1.0)).ceil().max(1.0) as usize;
            if !tipo.contains("barH") {
                for (i, t) in categorias.iter().enumerate().step_by(paso_cat) {
                    if let Some((d, w, _)) = cache.disposicion(
                        p.motor,
                        t,
                        &pequeno,
                        tam_p,
                        None,
                        DWRITE_TEXT_ALIGNMENT_LEADING,
                        None,
                        1.0,
                    ) {
                        let x = plot.x + ancho_cat * (i as f32 + 0.5) - w / 2.0;
                        dibujar_trabajo(
                            p,
                            &Trabajo {
                                disposicion: d,
                                x,
                                y: plot.y + plot.alto + 3.0 * e,
                                recorte: rect(r.x, r.y, r.x + r.ancho, r.y + r.alto),
                                color: gris,
                                giro: None,
                                medida: None,
                            },
                        );
                    }
                }
            }
        }
    }
    // Leyenda.
    if alto_leyenda > 0.0 {
        let nombres: Vec<&str> = if es_tarta {
            categorias.iter().map(String::as_str).collect()
        } else {
            series.iter().map(|s| s.nombre.as_str()).collect()
        };
        let mut piezas = Vec::new();
        let mut total = 0.0;
        for (i, n) in nombres.iter().enumerate().take(12) {
            if let Some((d, w, h)) = cache.disposicion(
                p.motor,
                n,
                &pequeno,
                tam_p,
                None,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                None,
                1.0,
            ) {
                total += w + 18.0 * e;
                piezas.push((i, d, w, h));
            }
        }
        let mut x = r.x + (r.ancho - total).max(0.0) / 2.0;
        let yl = r.y + r.alto - alto_leyenda - 2.0 * e;
        for (i, d, w, h) in piezas {
            let c = if es_tarta {
                libro.tema.colores[4 + i % 6]
            } else {
                colores[i % colores.len()]
            };
            llenar(
                p,
                rect(
                    x,
                    yl + (alto_leyenda - 7.0 * e) / 2.0,
                    x + 7.0 * e,
                    yl + (alto_leyenda + 7.0 * e) / 2.0,
                ),
                c,
            );
            dibujar_trabajo(
                p,
                &Trabajo {
                    disposicion: d,
                    x: x + 10.0 * e,
                    y: yl + (alto_leyenda - h) / 2.0,
                    recorte: rect(r.x, r.y, r.x + r.ancho, r.y + r.alto),
                    color: gris,
                    giro: None,
                    medida: None,
                },
            );
            x += w + 18.0 * e;
        }
    }
    p.soltar_recorte();
}

fn paso_bonito(bruto: f64) -> f64 {
    if !(bruto.is_finite() && bruto > 0.0) {
        return 1.0;
    }
    let mag = 10f64.powf(bruto.log10().floor());
    let r = bruto / mag;
    let n = if r <= 1.0 {
        1.0
    } else if r <= 2.0 {
        2.0
    } else if r <= 2.5 {
        2.5
    } else if r <= 5.0 {
        5.0
    } else {
        10.0
    };
    n * mag
}

#[allow(clippy::too_many_arguments)]
fn pintar_encabezados(
    p: &Pintor,
    cache: &mut CacheHoja,
    libro: &Libro,
    rejilla: &Rejilla,
    paneles: &[Panel],
    celdas: RectF,
    caja: RectF,
    e: f64,
    seleccion: Option<Rango>,
) {
    let fuente = Fuente {
        negrita: false,
        cursiva: false,
        subrayado: Subrayado::Ninguno,
        tachado: false,
        ..libro.estilos.fuente_normal.clone()
    };
    let tam = (11.0 * 96.0 / 72.0 * e) as f32;
    let (ew, eh) = (celdas.x - caja.x, celdas.y - caja.y);
    // Fondo.
    llenar(
        p,
        rect(caja.x, caja.y, caja.x + caja.ancho, celdas.y),
        ui(ENCABEZADO_FONDO),
    );
    llenar(
        p,
        rect(caja.x, celdas.y, celdas.x, caja.y + caja.alto),
        ui(ENCABEZADO_FONDO),
    );
    let resalte = ui(Rgb {
        r: 0xD2,
        g: 0xD2,
        b: 0xD2,
    });
    let texto_sel = legible(Rgb {
        r: 0x21,
        g: 0x73,
        b: 0x46,
    });
    // `e` trae los ppp de la pantalla (para la letra); las posiciones van
    // con el zoom solo, como las celdas: la rejilla ya esta en px de esa
    // pantalla (con 120 ppp los encabezados se iban un 25 % de las celdas).
    let zoom = e * 96.0 / rejilla.ppp.max(1.0);
    for panel in paneles {
        let m = Mapa::de(panel, celdas, zoom);
        if panel.pantalla[1] == 0.0 && eh > 0.0 {
            // Letras de las columnas.
            p.empujar_recorte(RectF {
                x: m.ox as f32,
                y: caja.y,
                ancho: panel.pantalla[2] as f32,
                alto: eh,
            });
            for c in panel.col0..=panel.col1 {
                let (x0, x1) = (m.sx(rejilla, c), m.sx(rejilla, c + 1));
                if x1 <= x0 {
                    continue;
                }
                let sel = seleccion.is_some_and(|s| c >= s.col0 && c <= s.col1);
                if sel {
                    llenar(p, rect(x0, caja.y, x1, celdas.y), resalte);
                }
                llenar(
                    p,
                    rect(x1 - 1.0, caja.y, x1, celdas.y),
                    ui(ENCABEZADO_LINEA),
                );
                let t = nombre_columna(c);
                if let Some((d, w, h)) = cache.disposicion(
                    p.motor,
                    &t,
                    &fuente,
                    tam,
                    None,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                    None,
                    1.0,
                ) {
                    dibujar_trabajo(
                        p,
                        &Trabajo {
                            disposicion: d,
                            x: ((x0 + x1 - 1.0 - w) / 2.0).round(),
                            y: (caja.y + (eh - h) / 2.0).round(),
                            recorte: rect(x0, caja.y, x1 - 1.0, celdas.y),
                            color: if sel { texto_sel } else { ui(ENCABEZADO_TEXTO) },
                            giro: None,
                            medida: None,
                        },
                    );
                }
            }
            p.soltar_recorte();
        }
        if panel.pantalla[0] == 0.0 && ew > 0.0 {
            // Numeros de las filas.
            p.empujar_recorte(RectF {
                x: caja.x,
                y: m.oy as f32,
                ancho: ew,
                alto: panel.pantalla[3] as f32,
            });
            for f in panel.fila0..=panel.fila1.min(panel.fila0 + 2000) {
                let (y0, y1) = (m.sy(rejilla, f), m.sy(rejilla, f + 1));
                if y1 <= y0 {
                    continue;
                }
                let sel = seleccion.is_some_and(|s| f >= s.fila0 && f <= s.fila1);
                if sel {
                    llenar(p, rect(caja.x, y0, celdas.x, y1), resalte);
                }
                llenar(
                    p,
                    rect(caja.x, y1 - 1.0, celdas.x, y1),
                    ui(ENCABEZADO_LINEA),
                );
                let t = (f + 1).to_string();
                if let Some((d, w, h)) = cache.disposicion(
                    p.motor,
                    &t,
                    &fuente,
                    tam,
                    None,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                    None,
                    1.0,
                ) {
                    dibujar_trabajo(
                        p,
                        &Trabajo {
                            disposicion: d,
                            x: ((caja.x + celdas.x - 1.0 - w) / 2.0).round(),
                            y: ((y0 + y1 - 1.0 - h) / 2.0).round(),
                            recorte: rect(caja.x, y0, celdas.x - 1.0, y1 - 1.0),
                            color: if sel { texto_sel } else { ui(ENCABEZADO_TEXTO) },
                            giro: None,
                            medida: None,
                        },
                    );
                }
            }
            p.soltar_recorte();
        }
    }
    // Bordes de los encabezados y la esquina.
    llenar(
        p,
        rect(caja.x, celdas.y - 1.0, caja.x + caja.ancho, celdas.y),
        ui(ENCABEZADO_LINEA),
    );
    llenar(
        p,
        rect(celdas.x - 1.0, caja.y, celdas.x, caja.y + caja.alto),
        ui(ENCABEZADO_LINEA),
    );
    // El triangulito de la esquina.
    let (cx, cy) = (celdas.x - 4.0 * e as f32, celdas.y - 4.0 * e as f32);
    let l = 9.0 * e as f32;
    p.poligono(
        &[(cx, cy - l), (cx, cy), (cx - l, cy)],
        color(ui(Rgb {
            r: 0xB4,
            g: 0xB4,
            b: 0xB4,
        })),
    );
}

/// Una pestana pintada: su indice de hoja y su caja (para el hit-test).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pestana {
    pub hoja: usize,
    pub caja: RectF,
}

/// La tira de pestanas de las hojas visibles, como la de Excel, en `caja`.
/// `desde`: la primera pestana que se ve (para desplazar la tira). Devuelve
/// donde quedo cada una.
pub fn pintar_pestanas(
    p: &Pintor,
    cache: &mut CacheHoja,
    libro: &Libro,
    activa: usize,
    desde: usize,
    caja: RectF,
    escala: f32,
) -> Vec<Pestana> {
    let fondo = ui(Rgb {
        r: 0xF3,
        g: 0xF3,
        b: 0xF3,
    });
    llenar(
        p,
        rect(caja.x, caja.y, caja.x + caja.ancho, caja.y + caja.alto),
        fondo,
    );
    llenar(
        p,
        rect(caja.x, caja.y, caja.x + caja.ancho, caja.y + 1.0),
        ui(ENCABEZADO_LINEA),
    );
    let f = Fuente {
        tam: 11.0,
        negrita: false,
        ..libro.estilos.fuente_normal.clone()
    };
    let tam = 11.0 * 96.0 / 72.0 * escala;
    let mut x = caja.x + 8.0 * escala;
    let mut v = Vec::new();
    p.empujar_recorte(caja);
    for (i, h) in libro.hojas.iter().enumerate().skip(desde) {
        if h.visibilidad != pixpin_hojas::Visibilidad::Visible {
            continue;
        }
        let es = i == activa;
        let fb = Fuente {
            negrita: es,
            ..f.clone()
        };
        let Some((d, w, hh)) = cache.disposicion(
            p.motor,
            &h.nombre,
            &fb,
            tam,
            None,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            None,
            1.0,
        ) else {
            continue;
        };
        let ancho = w + 24.0 * escala;
        let r = RectF {
            x,
            y: caja.y,
            ancho,
            alto: caja.alto,
        };
        if es {
            llenar(p, rect(r.x, r.y, r.x + r.ancho, r.y + r.alto), papel());
            llenar(
                p,
                rect(
                    r.x + 6.0 * escala,
                    r.y + r.alto - 3.0 * escala,
                    r.x + r.ancho - 6.0 * escala,
                    r.y + r.alto - 1.0 * escala,
                ),
                legible(SELECCION),
            );
        }
        llenar(
            p,
            rect(
                r.x + r.ancho,
                r.y + 6.0 * escala,
                r.x + r.ancho + 1.0,
                r.y + r.alto - 6.0 * escala,
            ),
            ui(ENCABEZADO_LINEA),
        );
        dibujar_trabajo(
            p,
            &Trabajo {
                disposicion: d,
                x: r.x + 12.0 * escala,
                y: r.y + (r.alto - hh) / 2.0,
                recorte: rect(r.x, r.y, r.x + r.ancho, r.y + r.alto),
                color: if es {
                    legible(SELECCION)
                } else {
                    ui(ENCABEZADO_TEXTO)
                },
                giro: None,
                medida: None,
            },
        );
        v.push(Pestana { hoja: i, caja: r });
        x += ancho + 1.0;
        if x > caja.x + caja.ancho {
            break;
        }
    }
    p.soltar_recorte();
    v
}

#[allow(dead_code)]
/// **Minigraficos** (`x14:sparklineGroups`) de las celdas a la vista: linea,
/// columnas o ganancia/perdida, con marcadores. Los valores y el eje salen
/// del libro (los limites de grupo se calculan una vez por grupo).
#[allow(clippy::too_many_arguments)]
fn pintar_minigraficos(
    p: &Pintor,
    libro: &Libro,
    hoja: &Hoja,
    rejilla: &Rejilla,
    m: &Mapa,
    f0: u32,
    f1: u32,
    c0: u32,
    c1: u32,
) {
    use pixpin_hojas::minigraficos::{TipoMini, extremos, puntos_de};
    let minis = &hoja.cola.minigraficos;
    if minis.is_empty() {
        return;
    }
    let k = (m.e * rejilla.ppp / 96.0) as f32;
    let desde = minis.partition_point(|x| x.fila < f0);
    for mi in minis[desde..].iter().take_while(|x| x.fila <= f1) {
        if mi.col < c0
            || mi.col > c1
            || rejilla.alto(mi.fila) <= 0.0
            || rejilla.ancho(mi.col) <= 0.0
        {
            continue;
        }
        let Some(g) = hoja.cola.grupos_mini.get(mi.grupo as usize) else {
            continue;
        };
        let r = hoja.combinada_en(mi.fila, mi.col).unwrap_or(Rango {
            fila0: mi.fila,
            col0: mi.col,
            fila1: mi.fila,
            col1: mi.col,
        });
        let pad = (3.0 * k).round().max(1.0);
        let (bx0, by0) = (m.sx(rejilla, r.col0) + pad, m.sy(rejilla, r.fila0) + pad);
        let (bx1, by1) = (
            m.sx(rejilla, r.col1 + 1) - 1.0 - pad,
            m.sy(rejilla, r.fila1 + 1) - 1.0 - pad,
        );
        let (w, h) = (bx1 - bx0, by1 - by0);
        if w < 4.0 || h < 4.0 {
            continue;
        }
        let valores = libro.valores_mini(hoja, mi);
        let puntos = puntos_de(g, &valores);
        let Some((minimo, maximo)) = extremos(&puntos) else {
            continue;
        };
        let Some((lo, hi)) = libro.eje_mini(hoja, mi, &puntos) else {
            continue;
        };
        let n = puntos.len();
        let y_de = |v: f64| by1 - (((v - lo) / (hi - lo)).clamp(0.0, 1.0) as f32) * h;
        let y_cero = y_de(0.0).clamp(by0, by1);
        // Posicion del primer maximo y minimo (los de «alto» y «bajo»).
        let pos_de = |buscado: f64| puntos.iter().position(|v| *v == Some(buscado));
        let (i_alto, i_bajo) = (pos_de(maximo), pos_de(minimo));
        let color_de_punto = |i: usize, v: f64| -> Rgb {
            if g.alto && Some(i) == i_alto {
                g.c_alto
            } else if g.bajo && Some(i) == i_bajo {
                g.c_bajo
            } else if g.primero && i == 0 {
                g.c_primero
            } else if g.ultimo && i + 1 == n {
                g.c_ultimo
            } else if g.negativos && v < 0.0 {
                g.c_negativo
            } else if matches!(g.tipo, TipoMini::Linea) {
                g.c_marcadores
            } else {
                g.c_serie
            }
        };
        p.empujar_recorte(RectF {
            x: bx0 - pad,
            y: by0 - pad,
            ancho: w + 2.0 * pad,
            alto: h + 2.0 * pad,
        });
        if g.eje_x && lo < 0.0 && hi > 0.0 {
            llenar(
                p,
                rect(bx0, y_cero.round(), bx1, y_cero.round() + 1.0),
                g.c_eje,
            );
        }
        match g.tipo {
            TipoMini::Linea => {
                let x_de = |i: usize| {
                    if n > 1 {
                        bx0 + w * i as f32 / (n - 1) as f32
                    } else {
                        bx0 + w / 2.0
                    }
                };
                let grosor = (g.peso * 96.0 / 72.0 * k).max(1.0);
                // Tramos seguidos; un hueco corta la linea salvo que se pida unir.
                let mut tramo: Vec<(f32, f32)> = Vec::new();
                let unir = g.vacias == pixpin_hojas::minigraficos::Vacias::Unir;
                for (i, v) in puntos.iter().enumerate() {
                    match v {
                        Some(v) => tramo.push((x_de(i), y_de(*v))),
                        None if unir => {}
                        None => {
                            if tramo.len() > 1 {
                                p.polilinea(&tramo, grosor, color(g.c_serie));
                            }
                            tramo.clear();
                        }
                    }
                }
                if tramo.len() > 1 {
                    p.polilinea(&tramo, grosor, color(g.c_serie));
                }
                let lado = (grosor * 2.6).max(3.0);
                for (i, v) in puntos.iter().enumerate() {
                    let Some(v) = *v else { continue };
                    let resaltado = (g.alto && Some(i) == i_alto)
                        || (g.bajo && Some(i) == i_bajo)
                        || (g.primero && i == 0)
                        || (g.ultimo && i + 1 == n)
                        || (g.negativos && v < 0.0);
                    if g.marcadores || resaltado {
                        let (x, y) = (x_de(i), y_de(v));
                        llenar(
                            p,
                            rect(
                                x - lado / 2.0,
                                y - lado / 2.0,
                                x + lado / 2.0,
                                y + lado / 2.0,
                            ),
                            color_de_punto(i, v),
                        );
                    }
                }
            }
            TipoMini::Columna | TipoMini::GananciaPerdida => {
                let hueco = (w / n as f32 * 0.2).max(1.0).min(3.0 * k);
                let ancho = w / n as f32;
                for (i, v) in puntos.iter().enumerate() {
                    let Some(v) = *v else { continue };
                    let x0 = bx0 + ancho * i as f32 + hueco / 2.0;
                    let x1 = (bx0 + ancho * (i + 1) as f32 - hueco / 2.0).max(x0 + 1.0);
                    let c = color_de_punto(i, v);
                    let (ya, yb) = if matches!(g.tipo, TipoMini::GananciaPerdida) {
                        let medio = by0 + h / 2.0;
                        if v > 0.0 {
                            (by0, medio - 1.0)
                        } else if v < 0.0 {
                            (medio + 1.0, by1)
                        } else {
                            continue;
                        }
                    } else {
                        let y = y_de(v);
                        if v == 0.0 {
                            continue;
                        } else {
                            (y.min(y_cero), y.max(y_cero))
                        }
                    };
                    llenar(p, rect(x0, ya, x1, yb.max(ya + 1.0)), c);
                }
            }
        }
        p.soltar_recorte();
    }
}

const COMENTARIO_ROJO: Rgb = Rgb {
    r: 0xE0,
    g: 0x30,
    b: 0x30,
};
const FILTRO_ACTIVO: Rgb = Rgb {
    r: 0x1F,
    g: 0x6F,
    b: 0xB5,
};

/// Un embudo (filtro puesto) de `ancho` px centrado en `(cx, cy)`.
fn embudo(p: &Pintor, cx: f32, cy: f32, ancho: f32, c: Rgb) {
    let (a, h) = (ancho / 2.0, ancho * 0.9);
    let v = [
        (cx - a, cy - h / 2.0),
        (cx + a, cy - h / 2.0),
        (cx + a * 0.18, cy + h * 0.05),
        (cx + a * 0.18, cy + h / 2.0),
        (cx - a * 0.18, cy + h / 2.0 - h * 0.1),
        (cx - a * 0.18, cy + h * 0.05),
    ];
    p.poligono(&v, color(c));
}

/// **Marcas encima de las celdas** (solo para ver): el triangulo rojo de las
/// notas y comentarios en la esquina de arriba a la derecha, y los botones
/// del autofiltro (de la hoja y de las tablas) en la fila de encabezado.
fn pintar_marcas(
    p: &Pintor,
    hoja: &Hoja,
    rejilla: &Rejilla,
    paneles: &[Panel],
    celdas: RectF,
    e: f64,
) {
    let tablas: Vec<&pixpin_hojas::filtros::AutoFiltro> = hoja
        .cola
        .tablas
        .iter()
        .filter(|t| t.encabezado > 0)
        .filter_map(|t| t.filtro.as_ref().map(|f| (t, f)))
        .map(|(_, f)| f)
        .collect();
    let filtros: Vec<&pixpin_hojas::filtros::AutoFiltro> =
        hoja.cola.autofiltro.iter().chain(tablas).collect();
    if filtros.is_empty() && hoja.cola.comentarios.is_empty() {
        return;
    }
    let k = (e * rejilla.ppp / 96.0) as f32;
    p.empujar_recorte(celdas);
    for panel in paneles {
        let m = Mapa::de(panel, celdas, e);
        let recorte = RectF {
            x: m.ox as f32,
            y: m.oy as f32,
            ancho: panel.pantalla[2] as f32,
            alto: panel.pantalla[3] as f32,
        };
        p.empujar_recorte(recorte);
        for f in &filtros {
            let fila = f.rango.fila0;
            if fila < panel.fila0 || fila > panel.fila1 || rejilla.alto(fila) <= 0.0 {
                continue;
            }
            for col in f.columnas_con_boton() {
                if col < panel.col0 || col > panel.col1 || rejilla.ancho(col) <= 0.0 {
                    continue;
                }
                let caja = [
                    m.sx(rejilla, col),
                    m.sy(rejilla, fila),
                    m.sx(rejilla, col + 1),
                    m.sy(rejilla, fila + 1),
                ];
                let Some([bx0, by0, bx1, by1]) = caja_flecha(caja, k) else {
                    continue;
                };
                llenar(p, rect(bx0, by0, bx1, by1), FLECHA_BORDE);
                llenar(
                    p,
                    rect(bx0 + 1.0, by0 + 1.0, bx1 - 1.0, by1 - 1.0),
                    FLECHA_FONDO,
                );
                let (cx, cy) = ((bx0 + bx1) / 2.0, (by0 + by1) / 2.0);
                if f.tiene_filtro(col) {
                    embudo(p, cx, cy, (8.0 * k).round().max(5.0), FILTRO_ACTIVO);
                } else {
                    triangulo_abajo(
                        p,
                        cx,
                        cy + 2.5 * k,
                        (7.0 * k).round().max(3.0),
                        FLECHA_TINTA,
                    );
                }
            }
        }
        let com = &hoja.cola.comentarios;
        let desde = com.partition_point(|c| c.fila < panel.fila0);
        for c in com[desde..].iter().take_while(|c| c.fila <= panel.fila1) {
            if c.col < panel.col0
                || c.col > panel.col1
                || rejilla.alto(c.fila) <= 0.0
                || rejilla.ancho(c.col) <= 0.0
            {
                continue;
            }
            // En una combinacion, la esquina de toda su caja.
            let r = hoja.combinada_en(c.fila, c.col).unwrap_or(Rango {
                fila0: c.fila,
                col0: c.col,
                fila1: c.fila,
                col1: c.col,
            });
            let (x1, y0) = (m.sx(rejilla, r.col1 + 1) - 1.0, m.sy(rejilla, r.fila0));
            let s = (6.0 * k).round().max(4.0);
            if m.sx(rejilla, r.col1 + 1) - m.sx(rejilla, r.col0) < s * 1.5 {
                continue;
            }
            p.poligono(
                &[(x1 - s, y0), (x1, y0), (x1, y0 + s)],
                color(COMENTARIO_ROJO),
            );
        }
        p.soltar_recorte();
    }
    p.soltar_recorte();
}

#[cfg(test)]
mod pruebas_oscuro {
    use super::*;

    #[test]
    fn en_oscuro_el_relleno_de_color_se_queda_y_el_blanco_es_papel() {
        let amarillo = Rgb {
            r: 0xFF,
            g: 0xEB,
            b: 0x9C,
        };
        poner_oscuro(true);
        // Un relleno explicito de color conserva su color.
        assert_eq!(relleno_de(amarillo), amarillo);
        // El blanco liso es papel, como el fondo.
        assert_eq!(relleno_de(Rgb::BLANCO), papel());
        assert_eq!(
            papel(),
            Rgb {
                r: 0x1F,
                g: 0x1F,
                b: 0x22
            }
        );
        // El texto negro se aclara; la rejilla se oscurece.
        assert!(legible(Rgb::NEGRO).r > 200);
        assert!(ui(CUADRICULA).r < 0x60);
        poner_oscuro(false);
        // Caso negativo: en claro no cambia nada.
        assert_eq!(relleno_de(Rgb::BLANCO), Rgb::BLANCO);
        assert_eq!(legible(Rgb::NEGRO), Rgb::NEGRO);
        assert_eq!(ui(CUADRICULA), CUADRICULA);
    }
}
