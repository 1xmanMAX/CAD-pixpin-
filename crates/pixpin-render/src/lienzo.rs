//! El pintor seguro: la unica cara de Direct2D que ven las capas sin unsafe.
//!
//! `apps/pixpin` es `forbid(unsafe_code)` y aun asi orquesta todo el dibujo
//! del overlay. Este modulo le da primitivas seguras —rellenar, trazar,
//! bitmap, texto— y encierra el protocolo SetTarget/BeginDraw/EndDraw en
//! una unica funcion con clausura, donde no se puede olvidar ningun paso.

use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_GRADIENT_STOP};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_ALIASED, D2D1_CAP_STYLE_FLAT, D2D1_CAP_STYLE_ROUND, D2D1_DASH_STYLE_DASH,
    D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT, D2D1_ELLIPSE, D2D1_EXTEND_MODE_CLAMP, D2D1_GAMMA_2_2,
    D2D1_INTERPOLATION_MODE, D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC,
    D2D1_INTERPOLATION_MODE_LINEAR, D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR, D2D1_LINE_JOIN_ROUND,
    D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES, D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES,
    D2D1_ROUNDED_RECT, D2D1_STROKE_STYLE_PROPERTIES1, ID2D1Bitmap1, ID2D1PathGeometry1,
    ID2D1RenderTarget, ID2D1SolidColorBrush, ID2D1StrokeStyle,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_ITALIC, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_METRICS, DWRITE_TEXT_RANGE,
    DWRITE_TRIMMING, DWRITE_TRIMMING_GRANULARITY_CHARACTER, DWRITE_WORD_WRAPPING_NO_WRAP,
    IDWriteFactory, IDWriteTextLayout,
};
use windows::core::Interface;
use windows::core::w;
use windows_numerics::Vector2;

use crate::motor::{Color, ErrorRender, MotorRender};

/// Rectangulo en coordenadas de dibujo (pixeles del destino, coma flotante).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectF {
    pub x: f32,
    pub y: f32,
    pub ancho: f32,
    pub alto: f32,
}

impl RectF {
    pub fn desde(r: pixpin_geom_compat::Rect) -> RectF {
        RectF {
            x: r.0 as f32,
            y: r.1 as f32,
            ancho: r.2 as f32,
            alto: r.3 as f32,
        }
    }

    fn a_d2d(self) -> D2D_RECT_F {
        D2D_RECT_F {
            left: self.x,
            top: self.y,
            right: self.x + self.ancho,
            bottom: self.y + self.alto,
        }
    }
}

/// Como se muestrea un bitmap al estirarlo o encogerlo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interpolacion {
    /// Pixeles tal cual: la lupa, o una captura al 100 % o muy ampliada.
    Vecino,
    /// Suave y barata: ampliaciones intermedias.
    Lineal,
    /// Cubica de calidad: reducir sin dientes.
    Cubica,
}

impl Interpolacion {
    fn a_d2d(self) -> D2D1_INTERPOLATION_MODE {
        match self {
            Interpolacion::Vecino => D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
            Interpolacion::Lineal => D2D1_INTERPOLATION_MODE_LINEAR,
            Interpolacion::Cubica => D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC,
        }
    }
}

/// Compatibilidad sin dependencia: (x, y, ancho, alto). `pixpin-render` no
/// depende de `pixpin-geom` a proposito (son de la misma capa L0/L1 en
/// espiritu distinto); quien tiene un Rect de geom lo convierte en tupla.
pub mod pixpin_geom_compat {
    pub type Rect = (i32, i32, u32, u32);
}

impl MotorRender {
    /// Dibuja sobre `destino` con el protocolo completo encerrado: SetTarget,
    /// BeginDraw, la clausura, EndDraw y SetTarget(None). El error de EndDraw
    /// se devuelve; el destino queda siempre desligado.
    pub fn dibujar(
        &self,
        destino: &ID2D1Bitmap1,
        pintar: impl FnOnce(&Pintor),
    ) -> Result<(), ErrorRender> {
        let c = self.contexto();
        self.fotograma.set(self.fotograma.get() + 1);
        // Los contadores son POR fotograma: lo que interesa es «cuantos
        // objetos crea este fotograma», no el acumulado de la sesion.
        self.creados.set(crate::motor::Contadores::default());
        // SAFETY: protocolo documentado de D2D sobre un contexto vivo; el
        // SetTarget(None) final corre tanto en exito como en error de
        // EndDraw, porque va antes del `?`.
        unsafe {
            c.SetTarget(destino);
            c.BeginDraw();
            // El contexto es compartido entre ventanas: un desplazamiento
            // que dejara el fotograma anterior no debe contaminar este.
            c.SetTransform(&windows_numerics::Matrix3x2::identity());
        }
        let pintor = Pintor { motor: self };
        pintar(&pintor);
        // SAFETY: cierra el BeginDraw de arriba; None desliga el destino.
        let fin = unsafe { c.EndDraw(None, None) };
        // SAFETY: desligar siempre, tambien si EndDraw fallo.
        unsafe { c.SetTarget(None) };
        fin?;
        Ok(())
    }
}

/// Primitivas de dibujo validas SOLO dentro de `MotorRender::dibujar`.
pub struct Pintor<'a> {
    pub(crate) motor: &'a MotorRender,
}

/// Estilo de un tramo de texto. Sin nada marcado es el texto normal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EstiloTexto {
    pub negrita: bool,
    pub cursiva: bool,
    /// Monoespaciada (codigo).
    pub mono: bool,
}

/// Un tramo de texto con estilo. Las posiciones van en unidades UTF-16,
/// que es lo que cuenta DirectWrite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tramo {
    pub inicio: u32,
    pub longitud: u32,
    pub estilo: EstiloTexto,
}

/// El texto de `texto_linea` puesto en UNA linea: cada salto (tambien
/// `\r\n`, los de Unicode y los de pagina) y cada tabulador pasan a ser un
/// espacio. Sin esto, «sin partir» de DirectWrite solo deja de partir por el
/// ancho: los saltos del propio texto siguen abriendo lineas, y un resumen
/// que viene de una tabla pegada se salia de su fila y se veia detras de las
/// de abajo. Lo que ya es una linea sale tal cual, sin copiarlo.
pub fn en_una_linea(texto: &str) -> std::borrow::Cow<'_, str> {
    let corta = |c: char| {
        matches!(
            c,
            '\n' | '\r' | '\t' | '\u{b}' | '\u{c}' | '\u{85}' | '\u{2028}' | '\u{2029}'
        )
    };
    if !texto.contains(corta) {
        return std::borrow::Cow::Borrowed(texto);
    }
    // `\r\n` es UN salto: un espacio, no dos.
    std::borrow::Cow::Owned(texto.replace("\r\n", " ").replace(corta, " "))
}

/// Construye la disposicion DirectWrite de un texto: partido a `ancho_max`
/// (o en una sola linea con puntos suspensivos si `una_linea`), con los
/// tramos de estilo aplicados. Es la unica fabrica de disposiciones: la
/// usan el pintor (dentro del fotograma) y el motor (para medir fuera).
pub(crate) fn disposicion_dwrite(
    dwrite: &IDWriteFactory,
    texto: &str,
    tam: f32,
    ancho_max: f32,
    tramos: &[Tramo],
    una_linea: bool,
) -> Option<(IDWriteTextLayout, f32, f32)> {
    // En una linea es en UNA: los saltos del texto tambien abren lineas, y
    // «sin partir» solo quita las que abre el ancho (ver `en_una_linea`).
    let texto = if una_linea {
        en_una_linea(texto)
    } else {
        texto.into()
    };
    let contenido: Vec<u16> = texto.encode_utf16().collect();
    // SAFETY: cadenas constantes terminadas en cero; la disposicion copia
    // el texto y no retiene nada del llamante; los rangos se limitan al
    // texto (DirectWrite recorta los que se pasen).
    unsafe {
        let formato = dwrite
            .CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                tam,
                w!("es-ES"),
            )
            .ok()?;
        let disposicion = dwrite
            .CreateTextLayout(&contenido, &formato, ancho_max, f32::MAX)
            .ok()?;
        if una_linea {
            disposicion
                .SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)
                .ok()?;
            let signo = dwrite.CreateEllipsisTrimmingSign(&formato).ok()?;
            let recorte = DWRITE_TRIMMING {
                granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
                delimiter: 0,
                delimiterCount: 0,
            };
            disposicion.SetTrimming(&recorte, &signo).ok()?;
        }
        for t in tramos {
            let rango = DWRITE_TEXT_RANGE {
                startPosition: t.inicio,
                length: t.longitud,
            };
            if t.estilo.negrita {
                disposicion
                    .SetFontWeight(DWRITE_FONT_WEIGHT_BOLD, rango)
                    .ok()?;
            }
            if t.estilo.cursiva {
                disposicion
                    .SetFontStyle(DWRITE_FONT_STYLE_ITALIC, rango)
                    .ok()?;
            }
            if t.estilo.mono {
                disposicion.SetFontFamilyName(w!("Consolas"), rango).ok()?;
            }
        }
        let mut metricas = DWRITE_TEXT_METRICS::default();
        disposicion.GetMetrics(&mut metricas).ok()?;
        Some((disposicion, metricas.width, metricas.height))
    }
}

/// Una disposicion de texto sin tramos, guardada para el siguiente
/// fotograma: el mismo rotulo se pinta igual mientras no cambie su texto, su
/// tamano, su ancho o su letra, y una disposicion no depende de donde ni de
/// que color.
pub(crate) struct DisposicionCacheada {
    texto: String,
    tam: u32,
    ancho: u32,
    una_linea: bool,
    /// La letra con que se hizo (`None`: la de la interfaz, Segoe UI). Va en
    /// la clave: sin ella, dos textos iguales en dos familias saldrian los
    /// dos con la letra del primero que se pinto.
    letra: Option<ClaveLetra>,
    disposicion: IDWriteTextLayout,
    w: f32,
    h: f32,
    usado: u64,
    /// El halo de este renglon ya pintado en su mapa (`texto_con_halo`,
    /// `halo.rs`), con el grosor, la escala y el color con que se hizo. Vive
    /// y muere con la disposicion.
    halo: Option<(ClaveHalo, crate::halo::MapaDelHalo)>,
    /// El fotograma en que se hizo la disposicion: el mapa del halo solo se
    /// hace para un renglon que ya se pinto antes (ver `halo_cacheado`).
    nacido: u64,
}

/// La letra de una disposicion cacheada, comparable.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ClaveLetra {
    familia: String,
    negrita: bool,
    cursiva: bool,
    interlineado: Option<u32>,
}

impl ClaveLetra {
    fn de(l: &crate::letras::Letra) -> Self {
        ClaveLetra {
            familia: l.familia.to_string(),
            negrita: l.negrita,
            cursiva: l.cursiva,
            interlineado: l.interlineado.map(f32::to_bits),
        }
    }
}

/// Cuantas disposiciones se guardan. Mas que las de un fotograma lleno del
/// universo (600 fichas a la vista con dos lineas cada una), para que un
/// fotograma no eche lo que va a pedir el siguiente.
const MAX_TEXTOS: usize = 2048;

/// `disposicion_dwrite` sin tramos (o `letras::disposicion` si se da una
/// letra), desde la cache del motor.
pub(crate) fn disposicion_cacheada(
    motor: &MotorRender,
    texto: &str,
    tam: f32,
    ancho_max: f32,
    una_linea: bool,
    letra: Option<&crate::letras::Letra>,
) -> Option<(IDWriteTextLayout, f32, f32)> {
    use std::hash::{Hash, Hasher};
    let clave_letra = letra.map(ClaveLetra::de);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    texto.hash(&mut h);
    tam.to_bits().hash(&mut h);
    ancho_max.to_bits().hash(&mut h);
    una_linea.hash(&mut h);
    clave_letra.hash(&mut h);
    let clave = h.finish();
    let ahora = motor.fotograma.get();
    let mut mapa = motor.textos.borrow_mut();
    if let Some(e) = mapa.get_mut(&clave)
        && e.texto == texto
        && e.tam == tam.to_bits()
        && e.ancho == ancho_max.to_bits()
        && e.una_linea == una_linea
        && e.letra == clave_letra
    {
        e.usado = ahora;
        return Some((e.disposicion.clone(), e.w, e.h));
    }
    let (disposicion, w, hh) = match letra {
        Some(l) => crate::letras::disposicion(motor.dwrite(), texto, tam, ancho_max, l)?,
        None => disposicion_dwrite(motor.dwrite(), texto, tam, ancho_max, &[], una_linea)?,
    };
    motor.conto(|c| c.disposiciones += 1);
    if mapa.len() >= MAX_TEXTOS {
        // Primero lo que no se uso ni en este fotograma ni en el anterior;
        // si aun asi no cabe, todo (un fotograma que pide mas que el tope
        // no se puede cachear de todas formas).
        mapa.retain(|_, e| e.usado + 1 >= ahora);
        if mapa.len() >= MAX_TEXTOS {
            mapa.clear();
        }
    }
    mapa.insert(
        clave,
        DisposicionCacheada {
            texto: texto.to_string(),
            tam: tam.to_bits(),
            ancho: ancho_max.to_bits(),
            una_linea,
            letra: clave_letra,
            disposicion: disposicion.clone(),
            w,
            h: hh,
            usado: ahora,
            halo: None,
            nacido: ahora,
        },
    );
    Some((disposicion, w, hh))
}

/// Grosor del halo, nivel de escala (`halo::nivel_de`) y color: lo que hace
/// distinto el mapa de un mismo renglon.
pub(crate) type ClaveHalo = (u32, i32, [u32; 4]);

/// **El mapa del halo de un renglon de `texto_con_halo`**, de la cache del
/// motor: la entrada de la disposicion lo guarda con su clave, y `hacer`
/// solo se llama si no esta o si cambio. `None` si la disposicion no esta en
/// la cache (quien llama la acaba de pedir, asi que no deberia pasar) o
/// `hacer` no pudo.
fn halo_cacheado(
    motor: &MotorRender,
    texto: &str,
    tam: f32,
    letra: &crate::letras::Letra,
    clave_halo: ClaveHalo,
    hacer: impl FnOnce(&IDWriteTextLayout, f32, f32) -> Option<crate::halo::MapaDelHalo>,
) -> Option<crate::halo::MapaDelHalo> {
    use std::hash::{Hash, Hasher};
    let ancho_max = crate::letras::SIN_PARTIR;
    let clave_letra = Some(ClaveLetra::de(letra));
    // La misma clave que `disposicion_cacheada` con `SIN_PARTIR`, sin
    // partir en una linea y con letra: la entrada que acaba de pedir
    // `texto_con_halo`.
    let mut h = std::collections::hash_map::DefaultHasher::new();
    texto.hash(&mut h);
    tam.to_bits().hash(&mut h);
    ancho_max.to_bits().hash(&mut h);
    false.hash(&mut h);
    clave_letra.hash(&mut h);
    let clave = h.finish();
    let (disposicion, w, alto) = {
        let mapa = motor.textos.borrow();
        let e = mapa.get(&clave).filter(|e| {
            e.texto == texto
                && e.tam == tam.to_bits()
                && e.ancho == ancho_max.to_bits()
                && !e.una_linea
                && e.letra == clave_letra
        })?;
        if let Some((c, hecho)) = &e.halo
            && *c == clave_halo
        {
            return Some(hecho.clone());
        }
        // **El numero de la cota que se esta trazando cambia en cada aviso**
        // y su renglon nace y no vuelve: hacerle un mapa (0,5 ms medidos) es
        // mas caro que pintar sus copias una vez. El mapa se hace cuando el
        // renglon vuelve a pintarse en otro fotograma, que es lo que le pasa
        // a una cota quieta.
        if e.nacido == motor.fotograma.get() {
            return None;
        }
        (e.disposicion.clone(), e.w, e.h)
    };
    // Sin el prestamo del mapa: `hacer` habla con DirectWrite y Direct2D.
    let hecho = hacer(&disposicion, w, alto)?;
    if let Some(e) = motor.textos.borrow_mut().get_mut(&clave) {
        e.halo = Some((clave_halo, hecho.clone()));
    }
    Some(hecho)
}

/// Olvida los mapas de halo: son del dispositivo (ver
/// `MotorRender::olvidar_recursos_de_dispositivo`).
pub(crate) fn olvidar_halos(motor: &MotorRender) {
    for e in motor.textos.borrow_mut().values_mut() {
        e.halo = None;
    }
}

/// El lado del brillo pre-pintado. Se estira al radio que haga falta: un
/// degradado suave no pierde nada al ampliarse.
const LADO_BRILLO: u32 = 256;

/// Los pixeles RGBA, sin premultiplicar, de un brillo de color `rgb`: alfa 1 en
/// el centro que cae en linea recta a 0 en el borde, como el
/// `circulo_degradado` de dos paradas al que sustituye.
pub fn pixeles_de_brillo(rgb: [u8; 3], lado: u32) -> Vec<u8> {
    let mut v = vec![0u8; (lado * lado * 4) as usize];
    let r = lado as f32 / 2.0;
    for y in 0..lado {
        for x in 0..lado {
            let (dx, dy) = (x as f32 + 0.5 - r, y as f32 + 0.5 - r);
            let a = (1.0 - (dx * dx + dy * dy).sqrt() / r).clamp(0.0, 1.0);
            let i = ((y * lado + x) * 4) as usize;
            v[i..i + 3].copy_from_slice(&rgb);
            v[i + 3] = (255.0 * a).round() as u8;
        }
    }
    v
}

impl Pintor<'_> {
    fn pincel(&self, color: Color) -> Option<ID2D1SolidColorBrush> {
        self.motor.pincel(color)
    }

    pub fn limpiar(&self, color: Color) {
        // SAFETY: Clear dentro de BeginDraw/EndDraw (lo garantiza `dibujar`).
        unsafe { self.motor.contexto().Clear(Some(&color.a_d2d())) };
    }

    /// Limpia a transparente total: el fondo de una ventana de composicion.
    pub fn limpiar_transparente(&self) {
        self.limpiar(Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        });
    }

    /// Desplaza el origen de todo lo que se pinte a partir de aqui. Lo usa
    /// el pin cuando su ventana es mas pequena que su contenido (recortada
    /// al escritorio): el contenido se dibuja donde le toca y la ventana
    /// ensena la parte visible.
    pub fn desplazar(&self, dx: f32, dy: f32) {
        // SAFETY: SetTransform sobre el contexto vivo, dentro del fotograma.
        unsafe {
            self.motor
                .contexto()
                .SetTransform(&windows_numerics::Matrix3x2::translation(dx, dy));
        }
    }

    /// Ejecuta `pintar` con todo lo que dibuje recortado a `r`. Lo que caiga
    /// fuera no se rasteriza siquiera, asi que sirve para no pagar el relleno
    /// de una zona que luego va a quedar tapada (la sombra bajo la tarjeta).
    pub fn con_recorte(&self, r: RectF, pintar: impl FnOnce(&Pintor)) {
        // SAFETY: Push/Pop emparejados dentro del fotograma; el modo de
        // suavizado por defecto (aliased) es el mas barato y basta para un
        // recorte con bordes rectos.
        unsafe {
            self.motor
                .contexto()
                .PushAxisAlignedClip(&r.a_d2d(), D2D1_ANTIALIAS_MODE_ALIASED);
        }
        pintar(self);
        // SAFETY: cierra el Push de arriba, siempre.
        unsafe { self.motor.contexto().PopAxisAlignedClip() };
    }

    /// Dibuja girado en cuartos de vuelta y volteado alrededor de `centro`,
    /// encima del desplazamiento `base` que ya tuviera la escena. Al salir
    /// deja solo `base`, para que lo de despues no herede el giro.
    ///
    /// Se hace con una transformada y no rotando los pixeles: girar y
    /// volver a girar devuelve la imagen exacta, y una imagen grande no se
    /// copia en memoria cada vez.
    pub fn con_giro(
        &self,
        base: (f32, f32),
        centro: (f32, f32),
        cuartos: u8,
        volteo_h: bool,
        volteo_v: bool,
        pintar: impl FnOnce(&Pintor),
    ) {
        let (cos, sin) = match cuartos % 4 {
            0 => (1.0f32, 0.0f32),
            1 => (0.0, 1.0),
            2 => (-1.0, 0.0),
            _ => (0.0, -1.0),
        };
        let (fx, fy) = (
            if volteo_h { -1.0f32 } else { 1.0 },
            if volteo_v { -1.0f32 } else { 1.0 },
        );
        // Voltear y luego girar, todo alrededor del centro; el
        // desplazamiento base se suma al final.
        let (m11, m12) = (fx * cos, fx * sin);
        let (m21, m22) = (-fy * sin, fy * cos);
        let m = windows_numerics::Matrix3x2 {
            M11: m11,
            M12: m12,
            M21: m21,
            M22: m22,
            M31: centro.0 - (centro.0 * m11 + centro.1 * m21) + base.0,
            M32: centro.1 - (centro.0 * m12 + centro.1 * m22) + base.1,
        };
        // SAFETY: SetTransform sobre el contexto vivo, dentro del fotograma;
        // se restaura siempre justo despues.
        unsafe { self.motor.contexto().SetTransform(&m) };
        pintar(self);
        self.desplazar(base.0, base.1);
    }

    /// Recorte y transformada sueltos, para cuando lo que hay que envolver
    /// es un bloque largo y meterlo en una clausura solo lo haria ilegible.
    /// Van siempre en pareja con `soltar_recorte` y `desplazar`.
    pub fn empujar_recorte(&self, r: RectF) {
        // SAFETY: Push emparejado con el Pop de `soltar_recorte`, dentro del
        // mismo fotograma.
        unsafe {
            self.motor
                .contexto()
                .PushAxisAlignedClip(&r.a_d2d(), D2D1_ANTIALIAS_MODE_ALIASED);
        }
    }

    pub fn soltar_recorte(&self) {
        // SAFETY: cierra el Push de `empujar_recorte`.
        unsafe { self.motor.contexto().PopAxisAlignedClip() };
    }

    /// Mira el contenido de cerca sin mover la ventana: lo amplia por
    /// `escala` y lo corre `desplazamiento`, encima del desplazamiento
    /// `base` de la escena. Es el zoom con la ventana bloqueada.
    pub fn poner_vista(&self, base: (f32, f32), escala: f32, desplazamiento: (f32, f32)) {
        let m = windows_numerics::Matrix3x2 {
            M11: escala,
            M12: 0.0,
            M21: 0.0,
            M22: escala,
            M31: base.0 + desplazamiento.0,
            M32: base.1 + desplazamiento.1,
        };
        // SAFETY: SetTransform sobre el contexto vivo, dentro del fotograma.
        unsafe { self.motor.contexto().SetTransform(&m) };
    }

    /// **Pone en negativo lo ya pintado dentro de `zona`** (redondeada con
    /// `radio`, 0 = esquinas rectas), con la transformada de ahora: cada
    /// canal pasa a `1 - canal`, igual que [`Color::invertido`]. Lo que se
    /// pinte despues encima queda con sus colores.
    ///
    /// Se hace en la GPU, al pintar, y no toca ningun bitmap: es el modo de
    /// mezcla `MASK_INVERT` de Direct2D con una mascara blanca de la forma de
    /// la zona. La mascara se graba en una lista de ordenes con el contexto
    /// aparte del motor (`contexto_de_mascaras`), porque al principal no se le
    /// puede cambiar el destino a mitad de fotograma.
    ///
    /// Pensado para un fondo opaco (una foto, una hoja, la tarjeta del pin):
    /// donde lo de debajo es transparente no hay color que invertir.
    pub fn invertir_colores(&self, zona: RectF, radio: f32) {
        use windows::Win32::Graphics::Direct2D::Common::D2D1_COMPOSITE_MODE_MASK_INVERT;
        use windows::Win32::Graphics::Direct2D::ID2D1Image;
        if !(zona.ancho > 0.0 && zona.alto > 0.0) {
            return;
        }
        let Some(c2) = self.motor.contexto_de_mascaras() else {
            return;
        };
        // SAFETY: protocolo SetTarget/BeginDraw/EndDraw completo sobre el
        // contexto de mascaras, que no tiene nada empujado; el destino se
        // desliga siempre, tambien si algo falla por el camino.
        let mascara = unsafe {
            (|| -> windows::core::Result<ID2D1Image> {
                let pincel = c2.CreateSolidColorBrush(&Color::BLANCO.a_d2d(), None)?;
                let lista = c2.CreateCommandList()?;
                c2.SetTarget(&lista);
                c2.BeginDraw();
                c2.SetTransform(&windows_numerics::Matrix3x2::identity());
                if radio > 0.0 {
                    c2.FillRoundedRectangle(
                        &D2D1_ROUNDED_RECT {
                            rect: zona.a_d2d(),
                            radiusX: radio,
                            radiusY: radio,
                        },
                        &pincel,
                    );
                } else {
                    c2.FillRectangle(&zona.a_d2d(), &pincel);
                }
                let fin = c2.EndDraw(None, None);
                c2.SetTarget(None);
                fin?;
                lista.Close()?;
                lista.cast()
            })()
        };
        let Ok(mascara) = mascara else {
            return;
        };
        // SAFETY: dentro del fotograma del contexto principal; la mascara es
        // del mismo dispositivo.
        unsafe {
            self.motor.contexto().DrawImage(
                &mascara,
                None,
                None,
                D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
                D2D1_COMPOSITE_MODE_MASK_INVERT,
            );
        }
    }

    pub fn rellenar(&self, r: RectF, color: Color) {
        if let Some(p) = self.pincel(color) {
            // SAFETY: dentro del fotograma; pincel y contexto vivos.
            unsafe { self.motor.contexto().FillRectangle(&r.a_d2d(), &p) };
        }
    }

    pub fn rellenar_redondeado(&self, r: RectF, radio: f32, color: Color) {
        if let Some(p) = self.pincel(color) {
            let rr = D2D1_ROUNDED_RECT {
                rect: r.a_d2d(),
                radiusX: radio,
                radiusY: radio,
            };
            // SAFETY: dentro del fotograma; pincel y contexto vivos.
            unsafe { self.motor.contexto().FillRoundedRectangle(&rr, &p) };
        }
    }

    /// Un circulo relleno. Es un rectangulo redondeado con radio = mitad:
    /// Direct2D ya lo hace con antialias y no hace falta geometria nueva.
    pub fn circulo(&self, centro: (f32, f32), radio: f32, color: Color) {
        let r = RectF {
            x: centro.0 - radio,
            y: centro.1 - radio,
            ancho: 2.0 * radio,
            alto: 2.0 * radio,
        };
        self.rellenar_redondeado(r, radio, color);
    }

    /// Solo el borde de un circulo, de `grosor` centrado en el radio.
    pub fn anillo(&self, centro: (f32, f32), radio: f32, grosor: f32, color: Color) {
        if let Some(p) = self.pincel(color) {
            let e = D2D1_ELLIPSE {
                point: Vector2 {
                    X: centro.0,
                    Y: centro.1,
                },
                radiusX: radio,
                radiusY: radio,
            };
            // SAFETY: dentro del fotograma; pincel y contexto vivos.
            unsafe { self.motor.contexto().DrawEllipse(&e, &p, grosor, None) };
        }
    }

    /// Un circulo que va de `dentro` en el centro a `fuera` en el borde: el
    /// brillo de una galaxia o de un planeta.
    ///
    /// Crea el pincel en cada llamada (medido: unos 0,5 ms de CPU). Para
    /// muchos por fotograma, `brillo`, que es lo que usa el universo.
    pub fn circulo_degradado(&self, centro: (f32, f32), radio: f32, dentro: Color, fuera: Color) {
        let paradas = [
            D2D1_GRADIENT_STOP {
                position: 0.0,
                color: dentro.a_d2d(),
            },
            D2D1_GRADIENT_STOP {
                position: 1.0,
                color: fuera.a_d2d(),
            },
        ];
        // La version de `ID2D1RenderTarget`, que es la de dos paradas y
        // gamma: la del contexto pide espacios de color que aqui sobran.
        let destino: &ID2D1RenderTarget = self.motor.contexto();
        // SAFETY: dentro del fotograma; `paradas` vive hasta que la
        // coleccion se crea, y Direct2D la copia.
        let Ok(coleccion) = (unsafe {
            destino.CreateGradientStopCollection(&paradas, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP)
        }) else {
            return;
        };
        let centro_d2d = Vector2 {
            X: centro.0,
            Y: centro.1,
        };
        let props = D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES {
            center: centro_d2d,
            gradientOriginOffset: Vector2 { X: 0.0, Y: 0.0 },
            radiusX: radio,
            radiusY: radio,
        };
        // SAFETY: props y coleccion vivas; el pincel nace dentro del fotograma.
        let Ok(pincel) = (unsafe { destino.CreateRadialGradientBrush(&props, None, &coleccion) })
        else {
            return;
        };
        let e = D2D1_ELLIPSE {
            point: centro_d2d,
            radiusX: radio,
            radiusY: radio,
        };
        // SAFETY: dentro del fotograma; pincel y contexto vivos.
        unsafe { destino.FillEllipse(&e, &pincel) };
    }

    /// Un rectangulo con un degradado lineal de `a` (en el punto `desde`) a
    /// `b` (en `hasta`). Lo pide el timeline (5-oct-2026): los fondos de sus
    /// historias y el velo que oscurece una foto para que el texto se lea.
    /// Como `circulo_degradado`, crea el pincel en cada llamada: son pocos
    /// por fotograma.
    pub fn rect_degradado(
        &self,
        r: RectF,
        desde: (f32, f32),
        hasta: (f32, f32),
        a: Color,
        b: Color,
    ) {
        let paradas = [
            D2D1_GRADIENT_STOP {
                position: 0.0,
                color: a.a_d2d(),
            },
            D2D1_GRADIENT_STOP {
                position: 1.0,
                color: b.a_d2d(),
            },
        ];
        let destino: &ID2D1RenderTarget = self.motor.contexto();
        // SAFETY: dentro del fotograma; `paradas` vive hasta que la
        // coleccion se crea, y Direct2D la copia.
        let Ok(coleccion) = (unsafe {
            destino.CreateGradientStopCollection(&paradas, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP)
        }) else {
            return;
        };
        let props = D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
            startPoint: Vector2 {
                X: desde.0,
                Y: desde.1,
            },
            endPoint: Vector2 {
                X: hasta.0,
                Y: hasta.1,
            },
        };
        // SAFETY: props y coleccion vivas; el pincel nace dentro del fotograma.
        let Ok(pincel) = (unsafe { destino.CreateLinearGradientBrush(&props, None, &coleccion) })
        else {
            return;
        };
        // SAFETY: dentro del fotograma; pincel y contexto vivos.
        unsafe { destino.FillRectangle(&r.a_d2d(), &pincel) };
    }

    /// Lo mismo que `circulo_degradado` de `color` a transparente, pero con
    /// un bitmap pre-pintado por color y estirado al radio. El degradado
    /// creaba una coleccion de paradas y un pincel radial por galaxia y
    /// fotograma: medido, 0,5 ms de CPU cada uno, 11 ms con veinte galaxias.
    /// `opacidad` es la del centro. Si el bitmap no se puede crear, un
    /// circulo liso y tenue: se ve donde esta la galaxia, que es lo que
    /// importa.
    pub fn brillo(&self, centro: (f32, f32), radio: f32, color: Color, opacidad: f32) {
        let a_u8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        let rgb = [a_u8(color.r), a_u8(color.g), a_u8(color.b)];
        let existente = self.motor.brillos.borrow().get(&rgb).cloned();
        let bitmap = existente.or_else(|| {
            let pixeles = pixeles_de_brillo(rgb, LADO_BRILLO);
            let b = self
                .motor
                .bitmap_desde_pixeles_premultiplicado(LADO_BRILLO, LADO_BRILLO, &pixeles)
                .ok()?;
            self.motor.brillos.borrow_mut().insert(rgb, b.clone());
            Some(b)
        });
        let destino = RectF {
            x: centro.0 - radio,
            y: centro.1 - radio,
            ancho: 2.0 * radio,
            alto: 2.0 * radio,
        };
        match bitmap {
            Some(b) => self.bitmap_translucido(&b, destino, None, Interpolacion::Lineal, opacidad),
            None => self.circulo(
                centro,
                radio,
                Color {
                    a: opacidad * 0.3,
                    ..color
                },
            ),
        }
    }

    pub fn trazar(&self, r: RectF, grosor: f32, color: Color) {
        if let Some(p) = self.pincel(color) {
            // SAFETY: dentro del fotograma; pincel y contexto vivos.
            unsafe {
                self.motor
                    .contexto()
                    .DrawRectangle(&r.a_d2d(), &p, grosor, None)
            };
        }
    }

    /// El estilo de trazo discontinuo, compartido por `trazar_discontinuo` y
    /// `polilinea_discontinua`: son la misma raya, sobre un rectangulo o
    /// sobre una geometria cualquiera.
    fn estilo_discontinuo(&self) -> Option<ID2D1StrokeStyle> {
        // Extremos y uniones redondos por lo mismo que `estilo_redondo`: sin
        // ellos, las esquinas de un rectangulo a trazos quedan mordidas. Las
        // rayas de en medio siguen planas, que es como se leen «a trazos».
        let propiedades = D2D1_STROKE_STYLE_PROPERTIES1 {
            startCap: D2D1_CAP_STYLE_ROUND,
            endCap: D2D1_CAP_STYLE_ROUND,
            dashCap: D2D1_CAP_STYLE_FLAT,
            lineJoin: D2D1_LINE_JOIN_ROUND,
            miterLimit: 10.0,
            dashStyle: D2D1_DASH_STYLE_DASH,
            dashOffset: 0.0,
            ..Default::default()
        };
        // SAFETY: la factoria vive en el motor; crear un estilo de trazo no
        // tiene precondiciones. El cast es el upcast StrokeStyle1 ->
        // StrokeStyle, que DrawRectangle/DrawGeometry esperan.
        unsafe {
            self.motor
                .fabrica()
                .CreateStrokeStyle(&propiedades, None)
                .ok()
                .and_then(|e| e.cast::<ID2D1StrokeStyle>().ok())
        }
    }

    pub fn trazar_discontinuo(&self, r: RectF, grosor: f32, color: Color) {
        let estilo = self.estilo_discontinuo();
        if let Some(p) = self.pincel(color) {
            // SAFETY: dentro del fotograma; objetos vivos. StrokeStyle1
            // hereda de StrokeStyle, que es lo que DrawRectangle espera.
            unsafe {
                self.motor
                    .contexto()
                    .DrawRectangle(&r.a_d2d(), &p, grosor, estilo.as_ref())
            };
        }
    }

    pub fn linea(&self, desde: (f32, f32), hasta: (f32, f32), grosor: f32, color: Color) {
        if let Some(p) = self.pincel(color) {
            // SAFETY: dentro del fotograma; pincel y contexto vivos.
            unsafe {
                self.motor.contexto().DrawLine(
                    Vector2 {
                        X: desde.0,
                        Y: desde.1,
                    },
                    Vector2 {
                        X: hasta.0,
                        Y: hasta.1,
                    },
                    &p,
                    grosor,
                    None,
                )
            };
        }
    }

    /// Dibuja un bitmap. `nitido` usa vecino mas cercano (la lupa: pixeles
    /// reales); si no, interpolacion lineal (reescalados suaves).
    pub fn bitmap(&self, b: &ID2D1Bitmap1, destino: RectF, fuente: Option<RectF>, nitido: bool) {
        let modo = if nitido {
            Interpolacion::Vecino
        } else {
            Interpolacion::Lineal
        };
        self.bitmap_con(b, destino, fuente, modo);
    }

    /// Dibuja un bitmap con el modo de interpolacion que se diga (D141). La
    /// transformacion activa cuenta: dentro de la vista del mundo, `destino`
    /// va en coordenadas del mundo.
    pub fn bitmap_con(
        &self,
        b: &ID2D1Bitmap1,
        destino: RectF,
        fuente: Option<RectF>,
        modo: Interpolacion,
    ) {
        self.bitmap_translucido(b, destino, fuente, modo, 1.0);
    }

    /// Como `bitmap_con`, pero atenuado. Lo pide la imagen incrustada en el
    /// lienzo: el panel de propiedades solo le deja tocar la opacidad, asi
    /// que si el dibujo la ignorara, el unico control de una imagen no haria
    /// nada.
    pub fn bitmap_translucido(
        &self,
        b: &ID2D1Bitmap1,
        destino: RectF,
        fuente: Option<RectF>,
        modo: Interpolacion,
        opacidad: f32,
    ) {
        let fuente_d2d = fuente.map(|f| f.a_d2d());
        // SAFETY: dentro del fotograma; bitmap del mismo dispositivo D2D
        // (obligacion del llamante: todos los bitmaps salen de este motor).
        unsafe {
            self.motor.contexto().DrawBitmap(
                b,
                Some(&destino.a_d2d()),
                opacidad.clamp(0.0, 1.0),
                modo.a_d2d(),
                fuente_d2d.as_ref().map(|f| f as *const _),
                None,
            )
        };
    }

    /// **Un bitmap en modo oscuro** (`crate::oscuro`): como [`Pintor::bitmap`]
    /// con interpolacion lineal, pero cada pixel con la luz invertida y el
    /// tono conservado (lo blanco a gris muy oscuro, lo negro a claro). Es un
    /// efecto `ColorMatrix` de Direct2D al pintar: lo hace la GPU en el
    /// mismo fotograma y el bitmap guardado (la cache de hojas de un PDF)
    /// ni se toca ni se duplica. Si el efecto no se puede crear, se pinta
    /// tal cual.
    pub fn bitmap_oscuro(&self, b: &ID2D1Bitmap1, destino: RectF, fuente: Option<RectF>) {
        self.bitmap_con_matriz(b, destino, fuente, &crate::oscuro::matriz_d2d());
    }

    /// **Un bitmap en negativo** (cada canal al reves, el tono tambien): para
    /// pintar una foto debajo de un `invertir_colores` de toda la ventana y
    /// que, tras los dos negativos, salga con sus colores de verdad.
    pub fn bitmap_negativo(&self, b: &ID2D1Bitmap1, destino: RectF, fuente: Option<RectF>) {
        let m = [
            [-1.0, 0.0, 0.0, 0.0],
            [0.0, -1.0, 0.0, 0.0],
            [0.0, 0.0, -1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
            [1.0, 1.0, 1.0, 0.0],
        ];
        self.bitmap_con_matriz(b, destino, fuente, &m);
    }

    /// Un bitmap pasado por una `ColorMatrix` de Direct2D (fila = canal de
    /// entrada R, G, B, A y la de los desplazamientos).
    fn bitmap_con_matriz(
        &self,
        b: &ID2D1Bitmap1,
        destino: RectF,
        fuente: Option<RectF>,
        matriz: &[[f32; 4]; 5],
    ) {
        use windows::Win32::Graphics::Direct2D::Common::D2D1_COMPOSITE_MODE_SOURCE_OVER;
        use windows::Win32::Graphics::Direct2D::{
            CLSID_D2D1ColorMatrix, D2D1_COLORMATRIX_PROP_CLAMP_OUTPUT,
            D2D1_COLORMATRIX_PROP_COLOR_MATRIX, D2D1_PROPERTY_TYPE_BOOL,
            D2D1_PROPERTY_TYPE_MATRIX_5X4,
        };
        let c = self.motor.contexto();
        // SAFETY: GetSize es una consulta del bitmap vivo.
        let tam = unsafe { b.GetSize() };
        let f = fuente.unwrap_or(RectF {
            x: 0.0,
            y: 0.0,
            ancho: tam.width,
            alto: tam.height,
        });
        if !(f.ancho > 0.0 && f.alto > 0.0 && destino.ancho > 0.0 && destino.alto > 0.0) {
            return;
        }
        let datos: Vec<u8> = matriz
            .iter()
            .flatten()
            .flat_map(|x| x.to_ne_bytes())
            .collect();
        // SAFETY: el efecto integrado existe en todo Direct2D 1.1; su entrada
        // es un bitmap vivo del mismo dispositivo; los datos de la matriz
        // tienen los 20 flotantes que pide su tipo.
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
        let Some(img) = img else {
            self.bitmap_con(b, destino, fuente, Interpolacion::Lineal);
            return;
        };
        // La fuente cae en el destino: escala y traslado delante de la
        // transformada de ahora, que se restaura al acabar.
        let (sx, sy) = (destino.ancho / f.ancho, destino.alto / f.alto);
        let (dx, dy) = (destino.x - f.x * sx, destino.y - f.y * sy);
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
                Some(&f.a_d2d()),
                D2D1_INTERPOLATION_MODE_LINEAR,
                D2D1_COMPOSITE_MODE_SOURCE_OVER,
            );
            c.SetTransform(&antes);
        }
    }

    fn disposicion(&self, texto: &str, tam: f32) -> Option<(IDWriteTextLayout, f32, f32)> {
        self.disposicion_ajustada(texto, tam, f32::MAX)
    }

    /// Como `disposicion`, pero partiendo las lineas al llegar a
    /// `ancho_max`. Un ancho infinito da una sola linea.
    fn disposicion_ajustada(
        &self,
        texto: &str,
        tam: f32,
        ancho_max: f32,
    ) -> Option<(IDWriteTextLayout, f32, f32)> {
        disposicion_cacheada(self.motor, texto, tam, ancho_max, false, None)
    }

    /// Un parrafo con tramos de estilo (negrita, cursiva, monoespaciada):
    /// lo que pinta una nota en Markdown.
    #[allow(clippy::too_many_arguments)] // texto, posicion, tamano, ancho, tramos y color
    pub fn parrafo(
        &self,
        texto: &str,
        x: f32,
        y: f32,
        tam: f32,
        ancho_max: f32,
        tramos: &[Tramo],
        color: Color,
    ) {
        self.parrafo_con_enlaces(texto, x, y, tam, ancho_max, tramos, color, &[], color);
    }

    /// Un parrafo como [`Self::parrafo`] con **enlaces**: los trozos
    /// `(inicio, largo)` (unidades UTF-16) van subrayados y en
    /// `color_enlace`, como un enlace de cualquier pagina. Subrayar y tenir
    /// no mueven ninguna letra: se mide y se pulsa con la misma disposicion
    /// que sin ellos (`medir_parrafo`, `cajas_de_trozo` con los mismos
    /// tramos).
    #[allow(clippy::too_many_arguments)] // lo de `parrafo` y los enlaces
    pub fn parrafo_con_enlaces(
        &self,
        texto: &str,
        x: f32,
        y: f32,
        tam: f32,
        ancho_max: f32,
        tramos: &[Tramo],
        color: Color,
        enlaces: &[(u32, u32)],
        color_enlace: Color,
    ) {
        let Some((disposicion, _, _)) =
            disposicion_dwrite(self.motor.dwrite(), texto, tam, ancho_max, tramos, false)
        else {
            return;
        };
        self.motor.conto(|c| c.disposiciones += 1);
        if !enlaces.is_empty()
            && let Some(efecto) = self
                .pincel(color_enlace)
                .and_then(|p| p.cast::<windows::core::IUnknown>().ok())
        {
            for &(inicio, longitud) in enlaces {
                let rango = DWRITE_TEXT_RANGE {
                    startPosition: inicio,
                    length: longitud,
                };
                // SAFETY: disposicion viva y recien hecha para este texto;
                // DirectWrite recorta los rangos que se pasen del final.
                unsafe {
                    let _ = disposicion.SetUnderline(true, rango);
                    let _ = disposicion.SetDrawingEffect(&efecto, rango);
                }
            }
        }
        self.dibujar_disposicion(&disposicion, x, y, color);
    }

    /// Mide un parrafo con estilos, dentro de un fotograma.
    pub fn medir_parrafo(
        &self,
        texto: &str,
        tam: f32,
        ancho_max: f32,
        tramos: &[Tramo],
    ) -> (f32, f32) {
        self.motor.conto(|c| c.disposiciones += 1);
        disposicion_dwrite(self.motor.dwrite(), texto, tam, ancho_max, tramos, false)
            .map(|(_, w, h)| (w, h))
            .unwrap_or((0.0, 0.0))
    }

    /// **Donde cae un trozo de un parrafo**: las cajas (una por renglon que
    /// toque) de las letras `inicio..inicio+largo`, en unidades UTF-16 y con
    /// el origen en la esquina del parrafo. Es lo que marca lo encontrado al
    /// buscar dentro de un documento (D9): se pregunta a la MISMA disposicion
    /// que se pinta (`parrafo` con tramos, `texto_ajustado` sin ellos), asi
    /// la marca no puede caer al lado de la palabra.
    pub fn cajas_de_trozo(
        &self,
        texto: &str,
        tam: f32,
        ancho_max: f32,
        tramos: &[Tramo],
        inicio: u32,
        largo: u32,
    ) -> Vec<RectF> {
        let disposicion = if tramos.is_empty() {
            self.disposicion_ajustada(texto, tam, ancho_max)
        } else {
            disposicion_dwrite(self.motor.dwrite(), texto, tam, ancho_max, tramos, false)
        };
        let Some((disposicion, _, _)) = disposicion else {
            return Vec::new();
        };
        cajas_de_disposicion(&disposicion, inicio, largo)
    }
}

/// Las cajas de las letras `inicio..inicio+largo` de una disposicion, con
/// el origen en su esquina (ver `Pintor::cajas_de_trozo`).
pub(crate) fn cajas_de_disposicion(
    disposicion: &IDWriteTextLayout,
    inicio: u32,
    largo: u32,
) -> Vec<RectF> {
    {
        let mut cuantas = 0u32;
        // SAFETY: la disposicion esta viva; la primera llamada solo pide
        // cuantas cajas hacen falta (falla con «buffer insuficiente», que es
        // lo esperado) y la segunda escribe en un vector de ese tamano.
        unsafe {
            let _ = disposicion.HitTestTextRange(inicio, largo, 0.0, 0.0, None, &mut cuantas);
            if cuantas == 0 {
                return Vec::new();
            }
            let mut metricas = vec![
                windows::Win32::Graphics::DirectWrite::DWRITE_HIT_TEST_METRICS::default();
                cuantas as usize
            ];
            if disposicion
                .HitTestTextRange(inicio, largo, 0.0, 0.0, Some(&mut metricas), &mut cuantas)
                .is_err()
            {
                return Vec::new();
            }
            metricas
                .iter()
                .take(cuantas as usize)
                .map(|m| RectF {
                    x: m.left,
                    y: m.top,
                    ancho: m.width,
                    alto: m.height,
                })
                .collect()
        }
    }
}

impl Pintor<'_> {
    /// Una sola linea que, si no cabe en `ancho_max`, termina en puntos
    /// suspensivos en vez de partirse o salirse: el nombre de una ficha.
    pub fn texto_linea(&self, texto: &str, x: f32, y: f32, tam: f32, ancho_max: f32, color: Color) {
        let Some((disposicion, _, _)) =
            disposicion_cacheada(self.motor, texto, tam, ancho_max, true, None)
        else {
            return;
        };
        self.dibujar_disposicion(&disposicion, x, y, color);
    }

    fn dibujar_disposicion(&self, disposicion: &IDWriteTextLayout, x: f32, y: f32, color: Color) {
        if let Some(p) = self.pincel(color) {
            // SAFETY: dentro del fotograma; objetos vivos.
            unsafe {
                self.motor.contexto().DrawTextLayout(
                    Vector2 { X: x, Y: y },
                    disposicion,
                    &p,
                    Default::default(),
                )
            };
        }
    }

    /// Rellena un poligono cerrado dado por sus vertices.
    ///
    /// Es como se pinta la tinta de un trazo a mano: NO es una linea gruesa,
    /// es una mancha con forma, y por eso puede adelgazar en los extremos.
    /// Recibe pares de `f32` en vez de un tipo propio para no atar este crate
    /// al motor de dibujo, que vive en su misma capa.
    pub fn poligono(&self, vertices: &[(f32, f32)], color: Color) {
        if vertices.len() < 3 {
            return;
        }
        let Some(geometria) = self.geometria(vertices, true) else {
            return;
        };
        if let Some(p) = self.pincel(color) {
            // SAFETY: dentro del fotograma; geometria y pincel vivos.
            unsafe { self.motor.contexto().FillGeometry(&geometria, &p, None) };
        }
    }

    /// Rellena el contorno de un trazo de tinta (ver `crate::tinta`).
    pub fn tinta(&self, contorno: &[(f32, f32)], color: Color) {
        if contorno.len() < 3 {
            return;
        }
        let Some(geometria) = self.geometria_tinta(contorno) else {
            return;
        };
        if let Some(p) = self.pincel(color) {
            // SAFETY: dentro del fotograma; geometria y pincel vivos.
            unsafe { self.motor.contexto().FillGeometry(&geometria, &p, None) };
        }
    }

    pub(crate) fn geometria_tinta(&self, contorno: &[(f32, f32)]) -> Option<ID2D1PathGeometry1> {
        use crate::tinta::{PasoTrayecto, pasos_de_tinta};
        use windows::Win32::Graphics::Direct2D::Common::{
            D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED, D2D1_FILL_MODE_WINDING,
        };
        use windows::Win32::Graphics::Direct2D::D2D1_QUADRATIC_BEZIER_SEGMENT;
        let v = |p: (f32, f32)| Vector2 { X: p.0, Y: p.1 };

        // SAFETY: la geometria se abre, se rellena y se cierra aqui mismo;
        // si algo falla a mitad se descarta sin dibujarla.
        unsafe {
            let geometria = self.motor.fabrica().CreatePathGeometry().ok()?;
            self.motor.conto(|c| c.geometrias += 1);
            let sumidero = geometria.Open().ok()?;
            sumidero.SetFillMode(D2D1_FILL_MODE_WINDING);
            // Las cuadraticas seguidas se mandan en bloque: una llamada COM
            // por tramo seria buena parte del coste de un trazo largo.
            let mut tanda: Vec<D2D1_QUADRATIC_BEZIER_SEGMENT> = Vec::new();
            for paso in pasos_de_tinta(contorno) {
                if !matches!(paso, PasoTrayecto::Cuadratica { .. }) && !tanda.is_empty() {
                    sumidero.AddQuadraticBeziers(&tanda);
                    tanda.clear();
                }
                match paso {
                    PasoTrayecto::Mover(p) => sumidero.BeginFigure(v(p), D2D1_FIGURE_BEGIN_FILLED),
                    PasoTrayecto::Cuadratica { control, fin } => {
                        tanda.push(D2D1_QUADRATIC_BEZIER_SEGMENT {
                            point1: v(control),
                            point2: v(fin),
                        })
                    }
                    PasoTrayecto::Linea(p) => sumidero.AddLine(v(p)),
                    PasoTrayecto::Cerrar => sumidero.EndFigure(D2D1_FIGURE_END_CLOSED),
                }
            }
            sumidero.Close().ok()?;
            Some(geometria)
        }
    }

    /// Como `tinta`, pero reutilizando la teselacion de fotogramas
    /// anteriores. `clave` = (id del elemento, version, indice de la orden).
    /// Si la realizacion no se puede crear (contexto sin D2D 1.1 o
    /// dispositivo raro), se pinta sin cache: se ve igual, cuesta mas.
    pub fn tinta_cacheada(
        &self,
        cache: &mut crate::tinta::CacheTinta,
        clave: (u64, u32, u32),
        contorno: &[(f32, f32)],
        color: Color,
    ) {
        if contorno.len() < 3 {
            return;
        }
        let forma = crate::tinta::huella_de_forma(contorno, 1);
        if cache.reusar_trasladada(clave, forma.0, forma.1, self.motor.fotograma.get()) {
            self.pintar_realizada(cache, clave, color);
            return;
        }
        let Some(geometria) = self.geometria_tinta(contorno) else {
            return;
        };
        if !self.realizar(cache, clave, &geometria, None, forma) {
            // Sin D2D 1.1 o sin poder teselar: se pinta la geometria tal
            // cual. Se ve igual, cuesta mas.
            if let Some(p) = self.pincel(color) {
                // SAFETY: dentro del fotograma; geometria y pincel vivos.
                unsafe { self.motor.contexto().FillGeometry(&geometria, &p, None) };
            }
            return;
        }
        self.pintar_realizada(cache, clave, color);
    }

    /// Como `poligono`, pero guardando la teselacion. Es lo que usan los
    /// rellenos de rough.js: sin esto cada fotograma de paneo creaba y
    /// tiraba una `ID2D1PathGeometry` por pasada.
    pub fn poligono_cacheado(
        &self,
        cache: &mut crate::tinta::CacheTinta,
        clave: (u64, u32, u32),
        vertices: &[(f32, f32)],
        color: Color,
    ) {
        if vertices.len() < 3 {
            return;
        }
        let forma = crate::tinta::huella_de_forma(vertices, 2);
        if cache.reusar_trasladada(clave, forma.0, forma.1, self.motor.fotograma.get()) {
            self.pintar_realizada(cache, clave, color);
            return;
        }
        let Some(geometria) = self.geometria(vertices, true) else {
            return;
        };
        if !self.realizar(cache, clave, &geometria, None, forma) {
            if let Some(p) = self.pincel(color) {
                // SAFETY: dentro del fotograma; geometria y pincel vivos.
                unsafe { self.motor.contexto().FillGeometry(&geometria, &p, None) };
            }
            return;
        }
        self.pintar_realizada(cache, clave, color);
    }

    /// Como `polilinea` / `polilinea_discontinua`, pero guardando la
    /// teselacion del TRAZO (grosor y estilo incluidos). Las rayas son la
    /// primitiva que la guia de Direct2D llama «very expensive»: realizada,
    /// se paga una vez.
    pub fn polilinea_cacheada(
        &self,
        cache: &mut crate::tinta::CacheTinta,
        clave: (u64, u32, u32),
        vertices: &[(f32, f32)],
        grosor: f32,
        discontinua: bool,
        color: Color,
    ) {
        if vertices.len() < 2 {
            return;
        }
        // El grosor y las rayas cambian la realizacion de un trazo: van en
        // la forma, o un cambio de grosor sin mover nada se tomaria por una
        // traslacion de cero.
        let extra = 3 ^ ((grosor.to_bits() as u64) << 8) ^ ((discontinua as u64) << 40);
        let forma = crate::tinta::huella_de_forma(vertices, extra);
        if cache.reusar_trasladada(clave, forma.0, forma.1, self.motor.fotograma.get()) {
            self.pintar_realizada(cache, clave, color);
            return;
        }
        let Some(geometria) = self.geometria(vertices, false) else {
            return;
        };
        let estilo = if discontinua {
            self.estilo_discontinuo()
        } else {
            self.estilo_redondo()
        };
        if !self.realizar(
            cache,
            clave,
            &geometria,
            Some((grosor, estilo.as_ref())),
            forma,
        ) {
            if let Some(p) = self.pincel(color) {
                // SAFETY: dentro del fotograma; geometria, estilo y pincel
                // vivos.
                unsafe {
                    self.motor
                        .contexto()
                        .DrawGeometry(&geometria, &p, grosor, estilo.as_ref())
                };
            }
            return;
        }
        self.pintar_realizada(cache, clave, color);
    }

    /// Tesela `geometria` y la guarda en `clave`. `trazo` a `None` la rellena;
    /// con `Some((grosor, estilo))` realiza el trazo. Devuelve `false` si no
    /// se pudo (contexto sin D2D 1.1, o la GPU rechazo la realizacion): el
    /// llamante pinta entonces la geometria a pelo.
    fn realizar(
        &self,
        cache: &mut crate::tinta::CacheTinta,
        clave: (u64, u32, u32),
        geometria: &ID2D1PathGeometry1,
        trazo: Option<(f32, Option<&ID2D1StrokeStyle>)>,
        forma: (u64, (f32, f32)),
    ) -> bool {
        use windows::Win32::Graphics::Direct2D::{
            D2D1_DEFAULT_FLATTENING_TOLERANCE, ID2D1DeviceContext1,
        };
        let Ok(ctx1) = self.motor.contexto().cast::<ID2D1DeviceContext1>() else {
            return false;
        };
        // La tolerancia se divide por la escala: a 200 % hacen falta el
        // doble de triangulos para que la curva no se vea poligonal.
        let tolerancia = D2D1_DEFAULT_FLATTENING_TOLERANCE / cache.escala.max(0.01);
        // SAFETY: geometria recien creada por el llamante y contexto vivo.
        let hecha = unsafe {
            match trazo {
                None => ctx1.CreateFilledGeometryRealization(geometria, tolerancia),
                Some((grosor, estilo)) => {
                    ctx1.CreateStrokedGeometryRealization(geometria, tolerancia, grosor, estilo)
                }
            }
        };
        let Ok(r) = hecha else {
            return false;
        };
        self.motor.conto(|c| c.realizaciones += 1);
        // Lo que pesa, para el presupuesto de la cache: los segmentos de la
        // geometria, que es de lo que crecen los triangulos realizados. Si
        // Direct2D no lo sabe decir cuenta como uno, que es lo prudente
        // para no echar nada por un numero inventado.
        // SAFETY: consulta de solo lectura sobre la geometria cerrada que
        // acaba de realizarse.
        let peso = unsafe { geometria.GetSegmentCount() }.unwrap_or(1);
        cache.guardar(clave, self.motor.fotograma.get(), r, peso, forma.0, forma.1);
        true
    }

    /// Pinta la realizacion ya cacheada de `clave` SIN mirar la geometria:
    /// para cuando quien llama todavia no la ha convertido a `Vec<(f32,
    /// f32)>` y quiere evitar esa reserva si de todos modos hay un acierto.
    /// Devuelve `false` si no hay nada cacheado -o el contexto no da D2D
    /// 1.1- para que el llamante caiga entonces a la version que la
    /// construye.
    pub fn pintar_realizada(
        &self,
        cache: &mut crate::tinta::CacheTinta,
        clave: (u64, u32, u32),
        color: Color,
    ) -> bool {
        use windows::Win32::Graphics::Direct2D::ID2D1DeviceContext1;
        let Ok(ctx1) = self.motor.contexto().cast::<ID2D1DeviceContext1>() else {
            return false;
        };
        let Some((r, corrida)) = cache.tomar(clave, self.motor.fotograma.get()) else {
            return false;
        };
        if let Some(pincel) = self.pincel(color) {
            if corrida == (0.0, 0.0) {
                // SAFETY: dentro del fotograma; realizacion y pincel vivos.
                unsafe { ctx1.DrawGeometryRealization(&r, &pincel) };
            } else {
                // Lo que se movio desde que se realizo (`reusar_trasladada`):
                // la misma realizacion, corrida en el mundo -antes de la
                // vista- y la vista como estaba despues.
                let mut vista = windows_numerics::Matrix3x2::default();
                // SAFETY: GetTransform/SetTransform sobre el contexto vivo,
                // dentro del fotograma; realizacion y pincel vivos.
                unsafe {
                    ctx1.GetTransform(&mut vista);
                    let corrida =
                        windows_numerics::Matrix3x2::translation(corrida.0, corrida.1) * vista;
                    ctx1.SetTransform(&corrida);
                    ctx1.DrawGeometryRealization(&r, &pincel);
                    ctx1.SetTransform(&vista);
                }
            }
        }
        true
    }

    /// Rellena `marco` dejando sin pintar el poligono `hueco`: es el foco
    /// de D51. Dos figuras cerradas en una misma geometria y la regla de
    /// relleno alternada de Direct2D hacen el agujero sin recortes ni
    /// capas.
    pub fn velo(&self, marco: RectF, hueco: &[(f32, f32)], color: Color) {
        use windows::Win32::Graphics::Direct2D::Common::{
            D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED, D2D1_FILL_MODE_ALTERNATE,
        };
        if hueco.len() < 3 {
            self.rellenar(marco, color);
            return;
        }
        let esquinas = [
            (marco.x, marco.y),
            (marco.x + marco.ancho, marco.y),
            (marco.x + marco.ancho, marco.y + marco.alto),
            (marco.x, marco.y + marco.alto),
        ];
        // SAFETY: igual que `geometria`: crear, rellenar entre Open/Close y
        // descartar si algo falla a mitad, sin usarla nunca a medias.
        let geometria = unsafe {
            let Ok(geometria) = self.motor.fabrica().CreatePathGeometry() else {
                return;
            };
            self.motor.conto(|c| c.geometrias += 1);
            let Ok(sumidero) = geometria.Open() else {
                return;
            };
            sumidero.SetFillMode(D2D1_FILL_MODE_ALTERNATE);
            for figura in [&esquinas[..], hueco] {
                sumidero.BeginFigure(
                    Vector2 {
                        X: figura[0].0,
                        Y: figura[0].1,
                    },
                    D2D1_FIGURE_BEGIN_FILLED,
                );
                let resto: Vec<Vector2> = figura[1..]
                    .iter()
                    .map(|(x, y)| Vector2 { X: *x, Y: *y })
                    .collect();
                sumidero.AddLines(&resto);
                sumidero.EndFigure(D2D1_FIGURE_END_CLOSED);
            }
            if sumidero.Close().is_err() {
                return;
            }
            geometria
        };
        if let Some(p) = self.pincel(color) {
            // SAFETY: dentro del fotograma; geometria y pincel vivos.
            unsafe { self.motor.contexto().FillGeometry(&geometria, &p, None) };
        }
    }

    /// El trazo de las polilineas: extremos y uniones redondos, como pide
    /// Excalidraw a su canvas (`lineCap`/`lineJoin = "round"` en
    /// `renderElement.ts`).
    ///
    /// **Es lo que tapa las esquinas.** Un rectangulo son cuatro lados
    /// sueltos (y a mano, ocho pasadas); con el extremo plano por defecto de
    /// Direct2D cada lado acaba justo en el vertice y en la esquina de fuera
    /// quedaba un cuadradito sin pintar de medio grosor de lado: el usuario
    /// lo veia como «lineas juntas y no un cuadrado».
    fn estilo_redondo(&self) -> Option<ID2D1StrokeStyle> {
        self.estilo_icono(true, true)
    }

    /// **Muchas figuras de un color en una sola geometria**: rellenas
    /// (`cerradas`, con la regla del devanado, asi los triangulos que se
    /// tocan no dejan costuras) o trazadas a `grosor`. Es lo que necesita un
    /// plano al imprimirse: miles de rayas y triangulos, y una orden por
    /// figura lo haria eterno.
    pub fn figuras(&self, figuras: &[Vec<(f32, f32)>], cerradas: bool, grosor: f32, color: Color) {
        use windows::Win32::Graphics::Direct2D::Common::{
            D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_CLOSED,
            D2D1_FIGURE_END_OPEN, D2D1_FILL_MODE_WINDING,
        };
        let minimo = if cerradas { 3 } else { 2 };
        if !figuras.iter().any(|f| f.len() >= minimo) {
            return;
        }
        // SAFETY: como `geometria`: se rellena entre Open y Close y solo se
        // usa cerrada; dentro del fotograma, con el pincel vivo.
        unsafe {
            let Ok(geometria) = self.motor.fabrica().CreatePathGeometry() else {
                return;
            };
            self.motor.conto(|c| c.geometrias += 1);
            let Ok(sumidero) = geometria.Open() else {
                return;
            };
            if cerradas {
                sumidero.SetFillMode(D2D1_FILL_MODE_WINDING);
            }
            for f in figuras.iter().filter(|f| f.len() >= minimo) {
                sumidero.BeginFigure(
                    Vector2 {
                        X: f[0].0,
                        Y: f[0].1,
                    },
                    if cerradas {
                        D2D1_FIGURE_BEGIN_FILLED
                    } else {
                        D2D1_FIGURE_BEGIN_HOLLOW
                    },
                );
                let resto: Vec<Vector2> = f[1..]
                    .iter()
                    .map(|(x, y)| Vector2 { X: *x, Y: *y })
                    .collect();
                sumidero.AddLines(&resto);
                sumidero.EndFigure(if cerradas {
                    D2D1_FIGURE_END_CLOSED
                } else {
                    D2D1_FIGURE_END_OPEN
                });
            }
            if sumidero.Close().is_err() {
                return;
            }
            let Some(p) = self.pincel(color) else { return };
            if cerradas {
                self.motor.contexto().FillGeometry(&geometria, &p, None);
            } else {
                let estilo = self.estilo_redondo();
                self.motor
                    .contexto()
                    .DrawGeometry(&geometria, &p, grosor, estilo.as_ref());
            }
        }
    }

    /// Traza una polilinea abierta de grosor constante.
    pub fn polilinea(&self, vertices: &[(f32, f32)], grosor: f32, color: Color) {
        if vertices.len() < 2 {
            return;
        }
        let Some(geometria) = self.geometria(vertices, false) else {
            return;
        };
        let estilo = self.estilo_redondo();
        if let Some(p) = self.pincel(color) {
            // SAFETY: igual que arriba.
            unsafe {
                self.motor
                    .contexto()
                    .DrawGeometry(&geometria, &p, grosor, estilo.as_ref())
            };
        }
    }

    /// Como `polilinea`, pero a rayas. Es a `polilinea` lo que
    /// `trazar_discontinuo` es a `trazar`: la misma raya sobre una
    /// geometria cualquiera en vez de sobre un rectangulo.
    pub fn polilinea_discontinua(&self, vertices: &[(f32, f32)], grosor: f32, color: Color) {
        if vertices.len() < 2 {
            return;
        }
        let Some(geometria) = self.geometria(vertices, false) else {
            return;
        };
        let estilo = self.estilo_discontinuo();
        if let Some(p) = self.pincel(color) {
            // SAFETY: igual que en `polilinea`; StrokeStyle1 hereda de
            // StrokeStyle, que es lo que DrawGeometry espera.
            unsafe {
                self.motor
                    .contexto()
                    .DrawGeometry(&geometria, &p, grosor, estilo.as_ref())
            };
        }
    }

    /// El marco de lo seleccionado: un rectangulo a rayas alrededor de
    /// `caja`, con una holgura para no pegarse al propio dibujo, girado
    /// `angulo` alrededor del centro de `caja` — el mismo centro que usan
    /// `impacto::toca` y `pintado::ordenes` en `pixpin-motor2d`, para que el
    /// contorno quede pegado a la figura y no recto mientras los tiradores
    /// giran con ella.
    ///
    /// `caja` y `escala` van en las mismas unidades que usa el resto del
    /// lienzo con `poner_vista` puesto: mundo, con `escala` = unidades de
    /// mundo por pixel de pantalla (`1.0 / zoom`). Es lo que hace que el
    /// marco se vea igual de fino a cualquier aumento.
    pub fn marco(&self, caja: (f32, f32, f32, f32), angulo: f32, escala: f32) {
        let (x0, y0, x1, y1) = caja;
        let escala = escala.max(0.01);
        let h = 4.0 * escala;
        let grosor = (1.5 * escala).max(1.0);

        // Sin giro, el rectangulo de siempre: mas barato y sin arrastrar
        // error de coma flotante en las cuatro esquinas.
        if angulo == 0.0 {
            self.trazar_discontinuo(
                RectF {
                    x: x0 - h,
                    y: y0 - h,
                    ancho: (x1 - x0) + 2.0 * h,
                    alto: (y1 - y0) + 2.0 * h,
                },
                grosor,
                Color::ACENTO,
            );
            return;
        }

        // Girado: las cuatro esquinas de la caja con holgura, rotadas
        // alrededor de su propio centro. Misma formula que
        // `Punto2::girar` en pixpin-motor2d, repetida aqui a proposito —
        // este crate no depende de ese, y la geometria es una linea.
        let cx = (x0 + x1) / 2.0;
        let cy = (y0 + y1) / 2.0;
        let (s, c) = angulo.sin_cos();
        let girar = |x: f32, y: f32| -> (f32, f32) {
            let dx = x - cx;
            let dy = y - cy;
            (cx + dx * c - dy * s, cy + dx * s + dy * c)
        };
        let esquinas = [
            girar(x0 - h, y0 - h),
            girar(x1 + h, y0 - h),
            girar(x1 + h, y1 + h),
            girar(x0 - h, y1 + h),
        ];
        let cerrado = [
            esquinas[0],
            esquinas[1],
            esquinas[2],
            esquinas[3],
            esquinas[0],
        ];
        self.polilinea_discontinua(&cerrado, grosor, Color::ACENTO);
    }

    /// La marquesina: el rectangulo de arrastre con el que se elige por
    /// zona, relleno translucido y borde a rayas para distinguirlo de un
    /// elemento de verdad.
    pub fn marquesina(&self, caja: (f32, f32, f32, f32), escala: f32) {
        let (x0, y0, x1, y1) = caja;
        let r = RectF {
            x: x0.min(x1),
            y: y0.min(y1),
            ancho: (x1 - x0).abs(),
            alto: (y1 - y0).abs(),
        };
        self.rellenar(
            r,
            Color {
                a: 0.12,
                ..Color::ACENTO
            },
        );
        self.trazar_discontinuo(r, (1.5 * escala.max(0.01)).max(1.0), Color::ACENTO);
    }

    /// Construye una geometria a partir de los vertices. `cerrada` decide si
    /// el ultimo punto se une con el primero.
    pub(crate) fn geometria(
        &self,
        vertices: &[(f32, f32)],
        cerrada: bool,
    ) -> Option<ID2D1PathGeometry1> {
        use windows::Win32::Graphics::Direct2D::Common::{
            D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_CLOSED,
            D2D1_FIGURE_END_OPEN,
        };

        // SAFETY: la geometria se crea, se rellena entre Open/Close y se
        // devuelve cerrada. Si algo falla a mitad se descarta sin usarla:
        // una geometria sin cerrar reventaria al dibujarla.
        unsafe {
            let geometria = self.motor.fabrica().CreatePathGeometry().ok()?;
            self.motor.conto(|c| c.geometrias += 1);
            let sumidero = geometria.Open().ok()?;
            sumidero.BeginFigure(
                Vector2 {
                    X: vertices[0].0,
                    Y: vertices[0].1,
                },
                if cerrada {
                    D2D1_FIGURE_BEGIN_FILLED
                } else {
                    D2D1_FIGURE_BEGIN_HOLLOW
                },
            );
            let resto: Vec<Vector2> = vertices[1..]
                .iter()
                .map(|(x, y)| Vector2 { X: *x, Y: *y })
                .collect();
            sumidero.AddLines(&resto);
            sumidero.EndFigure(if cerrada {
                D2D1_FIGURE_END_CLOSED
            } else {
                D2D1_FIGURE_END_OPEN
            });
            sumidero.Close().ok()?;
            Some(geometria)
        }
    }

    /// Mide un texto ya ajustado a un ancho: lo que necesita una nota para
    /// saber de que tamano nace (S2-B).
    pub fn medir_texto_ajustado(&self, texto: &str, tam: f32, ancho_max: f32) -> (f32, f32) {
        self.disposicion_ajustada(texto, tam, ancho_max)
            .map(|(_, w, h)| (w, h))
            .unwrap_or((0.0, 0.0))
    }

    /// Dibuja texto partido a un ancho maximo, desde la esquina indicada.
    pub fn texto_ajustado(
        &self,
        texto: &str,
        x: f32,
        y: f32,
        tam: f32,
        ancho_max: f32,
        color: Color,
    ) {
        let Some((disposicion, _, _)) = self.disposicion_ajustada(texto, tam, ancho_max) else {
            return;
        };
        if let Some(p) = self.pincel(color) {
            // SAFETY: dentro del fotograma; objetos vivos.
            unsafe {
                self.motor.contexto().DrawTextLayout(
                    Vector2 { X: x, Y: y },
                    &disposicion,
                    &p,
                    Default::default(),
                )
            };
        }
    }

    /// **Un texto con su letra** (familia, negrita, cursiva, interlineado):
    /// lo que pinta el lienzo de dibujo. `ancho_max` a partir de
    /// `letras::SIN_PARTIR` no parte renglones, que es como van los textos
    /// sueltos de Excalidraw.
    #[allow(clippy::too_many_arguments)] // texto, posicion, tamano, ancho, letra y color
    pub fn texto_con_letra(
        &self,
        texto: &str,
        x: f32,
        y: f32,
        tam: f32,
        ancho_max: f32,
        letra: &crate::letras::Letra,
        color: Color,
    ) {
        let Some((disposicion, _, _)) =
            disposicion_cacheada(self.motor, texto, tam, ancho_max, false, Some(letra))
        else {
            return;
        };
        // La letra de emojis va en color (como `texto_color`); las demas, con
        // el pincel, que es mas barato.
        if letra.familia.contains("Emoji") {
            if let Some(p) = self.pincel(color) {
                // SAFETY: dentro del fotograma; objetos vivos.
                unsafe {
                    self.motor.contexto().DrawTextLayout(
                        Vector2 { X: x, Y: y },
                        &disposicion,
                        &p,
                        D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
                    )
                };
            }
            return;
        }
        self.dibujar_disposicion(&disposicion, x, y, color);
    }

    /// **Un renglon con halo**: el numero de una cota (`dibujarRotulo` de
    /// `Renderer.kt`). Primero el halo y encima las letras.
    ///
    /// El movil traza el contorno de las letras con un pincel de `grosor`
    /// (`Paint.Style.STROKE`, juntas redondas). Direct2D no traza texto sin
    /// sacar antes la geometria de cada glifo (`GetGlyphRunOutline`), que
    /// para un numero de pocas letras es mucho andamio; el mismo halo sale
    /// estampando el renglon en `color_halo` en un anillo de 16 posiciones a
    /// `grosor / 2` (el radio del trazo) y otro de 8 a la mitad, que rellena
    /// los huecos entre copias en las letras finas. Girado, eso eran 24
    /// dibujos de texto sin atlas por numero y fotograma (la mitad del coste
    /// de pintar una cota): desde el segundo fotograma en que se pinta el
    /// mismo renglon, las copias van en un mapa hecho una vez (`halo.rs`).
    #[allow(clippy::too_many_arguments)] // texto, posicion, tamano, letra, dos colores y grosor
    pub fn texto_con_halo(
        &self,
        texto: &str,
        x: f32,
        y: f32,
        tam: f32,
        letra: &crate::letras::Letra,
        color: Color,
        color_halo: Color,
        grosor: f32,
    ) {
        let Some((disposicion, _, _)) = disposicion_cacheada(
            self.motor,
            texto,
            tam,
            crate::letras::SIN_PARTIR,
            false,
            Some(letra),
        ) else {
            return;
        };
        let radio = grosor / 2.0;
        if radio > 0.0
            && color_halo.a > 0.0
            && !self.halo_de_mapa(texto, x, y, tam, letra, color_halo, grosor)
        {
            // Solo si no se pudo hacer el mapa (demasiado grande, o
            // Direct2D no pudo): las 24 copias a pelo, como antes.
            for (n, r) in [(16, radio), (8, radio / 2.0)] {
                for i in 0..n {
                    let a = i as f32 * std::f32::consts::TAU / n as f32;
                    self.dibujar_disposicion(
                        &disposicion,
                        x + r * a.cos(),
                        y + r * a.sin(),
                        color_halo,
                    );
                }
            }
        }
        self.dibujar_disposicion(&disposicion, x, y, color);
    }

    /// El halo de siempre (24 copias) y la letra, para medir contra el nuevo.
    #[cfg(test)]
    // Mismos parametros que el texto con halo real, para medir uno contra otro.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn texto_con_halo_de_copias_para_medir(
        &self,
        texto: &str,
        x: f32,
        y: f32,
        tam: f32,
        letra: &crate::letras::Letra,
        color: Color,
        grosor: f32,
    ) {
        let Some((disposicion, _, _)) = disposicion_cacheada(
            self.motor,
            texto,
            tam,
            crate::letras::SIN_PARTIR,
            false,
            Some(letra),
        ) else {
            return;
        };
        let radio = grosor / 2.0;
        for (n, r) in [(16, radio), (8, radio / 2.0)] {
            for i in 0..n {
                let a = i as f32 * std::f32::consts::TAU / n as f32;
                self.dibujar_disposicion(&disposicion, x + r * a.cos(), y + r * a.sin(), color);
            }
        }
        self.dibujar_disposicion(&disposicion, x, y, color);
    }

    /// **El halo desde su mapa** (`halo.rs`): las 24 copias pintadas una vez
    /// en un mapa a la escala de la vista, guardado con la disposicion, y
    /// aqui un solo `DrawBitmap`. Devuelve `false` si no se pudo, para que
    /// quien llama pinte las copias a pelo.
    #[allow(clippy::too_many_arguments)]
    fn halo_de_mapa(
        &self,
        texto: &str,
        x: f32,
        y: f32,
        tam: f32,
        letra: &crate::letras::Letra,
        color: Color,
        grosor: f32,
    ) -> bool {
        use windows::Win32::Graphics::Direct2D::D2D1_INTERPOLATION_MODE_LINEAR;
        let c = self.motor.contexto();
        let mut vista = windows_numerics::Matrix3x2::default();
        // SAFETY: lectura de la transformada del contexto vivo.
        unsafe { c.GetTransform(&mut vista) };
        // Cuantos pixeles mide una unidad del renglon con esta vista (y el
        // giro, que no cambia la escala).
        let escala = (vista.M11 * vista.M22 - vista.M12 * vista.M21).abs().sqrt();
        let Some(nivel) = crate::halo::nivel_de(escala) else {
            return false;
        };
        let clave = (
            grosor.to_bits(),
            nivel,
            [
                color.r.to_bits(),
                color.g.to_bits(),
                color.b.to_bits(),
                color.a.to_bits(),
            ],
        );
        let hecho = halo_cacheado(self.motor, texto, tam, letra, clave, |d, w, h| {
            crate::halo::hacer_mapa(self.motor, d, w, h, grosor / 2.0, nivel, color)
        });
        let Some(hecho) = hecho else {
            return false;
        };
        let (cx, cy, cw, ch) = hecho.caja;
        let destino = D2D_RECT_F {
            left: x + cx,
            top: y + cy,
            right: x + cx + cw,
            bottom: y + cy + ch,
        };
        // SAFETY: dentro del fotograma; mapa hecho por este motor.
        unsafe {
            c.DrawBitmap(
                &hecho.mapa,
                Some(&destino),
                1.0,
                D2D1_INTERPOLATION_MODE_LINEAR,
                None,
                None,
            )
        };
        true
    }

    /// Lo que ocupa un texto con su letra, con la MISMA disposicion que
    /// pinta `texto_con_letra`.
    pub fn medir_con_letra(
        &self,
        texto: &str,
        tam: f32,
        ancho_max: f32,
        letra: &crate::letras::Letra,
    ) -> (f32, f32) {
        disposicion_cacheada(self.motor, texto, tam, ancho_max, false, Some(letra))
            .map(|(_, w, h)| (w, h))
            .unwrap_or((0.0, 0.0))
    }

    /// Mide el texto sin dibujarlo: para colocar cajas.
    pub fn medir_texto(&self, texto: &str, tam: f32) -> (f32, f32) {
        self.disposicion(texto, tam)
            .map(|(_, w, h)| (w, h))
            .unwrap_or((0.0, 0.0))
    }

    pub fn texto(&self, texto: &str, x: f32, y: f32, tam: f32, color: Color) {
        let Some((disposicion, _, _)) = self.disposicion(texto, tam) else {
            return;
        };
        if let Some(p) = self.pincel(color) {
            // SAFETY: dentro del fotograma; objetos vivos.
            unsafe {
                self.motor.contexto().DrawTextLayout(
                    Vector2 { X: x, Y: y },
                    &disposicion,
                    &p,
                    Default::default(),
                )
            };
        }
    }

    /// Como `texto`, pero dejando que la fuente ponga sus propios colores:
    /// es lo que hace que un emoji salga en color y no como una silueta
    /// negra. Se pide aparte porque el texto normal no lo necesita y la
    /// opcion tiene coste.
    pub fn texto_color(&self, texto: &str, x: f32, y: f32, tam: f32, color: Color) {
        let Some((disposicion, _, _)) = self.disposicion(texto, tam) else {
            return;
        };
        if let Some(p) = self.pincel(color) {
            // SAFETY: dentro del fotograma; objetos vivos.
            unsafe {
                self.motor.contexto().DrawTextLayout(
                    Vector2 { X: x, Y: y },
                    &disposicion,
                    &p,
                    D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
                )
            };
        }
    }

    /// Texto sobre una caja redondeada semitransparente: las etiquetas del
    /// overlay (dimensiones, color de la lupa).
    pub fn texto_con_fondo(
        &self,
        texto: &str,
        x: f32,
        y: f32,
        tam: f32,
        color_texto: Color,
        color_fondo: Color,
    ) {
        let (ancho, alto) = self.medir_texto(texto, tam);
        let relleno = tam * 0.4;
        self.rellenar_redondeado(
            RectF {
                x: x - relleno,
                y: y - relleno * 0.5,
                ancho: ancho + relleno * 2.0,
                alto: alto + relleno,
            },
            tam * 0.25,
            color_fondo,
        );
        self.texto(texto, x, y, tam, color_texto);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_CPU_ACCESS_READ,
        D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE,
        D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING,
        D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
    };
    use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};

    /// Las lineas que DirectWrite reparte en una disposicion. Solo hace
    /// falta la fabrica de DirectWrite: ni GPU ni ventana.
    fn lineas_de(texto: &str, una_linea: bool) -> u32 {
        use windows::Win32::Graphics::DirectWrite::{
            DWRITE_FACTORY_TYPE_SHARED, DWriteCreateFactory, IDWriteFactory,
        };
        // SAFETY: crear la fabrica compartida no toma nada del llamante.
        let dwrite: IDWriteFactory =
            unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }.expect("DirectWrite");
        let (disposicion, _, _) =
            disposicion_dwrite(&dwrite, texto, 13.0, 300.0, &[], una_linea).expect("disposicion");
        let mut m = DWRITE_TEXT_METRICS::default();
        // SAFETY: disposicion viva; solo se leen sus medidas.
        unsafe { disposicion.GetMetrics(&mut m) }.expect("medidas");
        m.lineCount
    }

    /// Una tabla pegada de una hoja de calculo, tal cual llega al resumen
    /// de un proyecto en la lista (la queja: su texto se veia detras de las
    /// filas de abajo).
    const TABLA_PEGADA: &str =
        "DESCRIPCION \tMONTO\r\nALQUILER \t690\r\n\t687.5\r\nCELULAR\t39.95\r\n\t\r\n\t2517.34";

    #[test]
    fn un_texto_de_una_linea_con_saltos_y_tabuladores_sale_en_una_sola_linea() {
        assert_eq!(lineas_de(TABLA_PEGADA, true), 1);
        for salto in [
            "\n", "\r", "\r\n", "\u{2028}", "\u{2029}", "\u{85}", "\u{b}", "\u{c}",
        ] {
            assert_eq!(
                lineas_de(&format!("uno{salto}dos"), true),
                1,
                "salto {salto:?}"
            );
        }
    }

    #[test]
    fn un_parrafo_sigue_partiendose_en_sus_saltos() {
        // Lo contrario no se toca: una burbuja o una nota SI tienen lineas.
        assert!(lineas_de(TABLA_PEGADA, false) > 1);
    }

    #[test]
    fn en_una_linea_cambia_cada_salto_y_tabulador_por_un_espacio() {
        assert_eq!(en_una_linea("a\r\nb\tc\nd\re"), "a b c d e");
        assert_eq!(en_una_linea("x\u{2028}y\u{2029}z"), "x y z");
    }

    #[test]
    fn en_una_linea_deja_igual_y_sin_copiar_lo_que_ya_es_una_linea() {
        let t = "Casa Lima · 3 hojas";
        assert!(matches!(en_una_linea(t), std::borrow::Cow::Borrowed(s) if s == t));
        assert_eq!(en_una_linea(""), "");
    }

    #[test]
    fn cada_interpolacion_va_a_su_modo_de_direct2d() {
        use windows::Win32::Graphics::Direct2D::{
            D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC, D2D1_INTERPOLATION_MODE_LINEAR,
            D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
        };
        assert_eq!(
            Interpolacion::Vecino.a_d2d(),
            D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR
        );
        assert_eq!(
            Interpolacion::Lineal.a_d2d(),
            D2D1_INTERPOLATION_MODE_LINEAR
        );
        assert_eq!(
            Interpolacion::Cubica.a_d2d(),
            D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC
        );
    }

    fn dispositivo() -> (ID3D11Device, ID3D11DeviceContext) {
        let mut d3d = None;
        let mut ctx = None;
        // SAFETY: salidas locales, constantes documentadas.
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                Default::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut d3d),
                None,
                Some(&mut ctx),
            )
            .expect("GPU real");
        }
        (d3d.unwrap(), ctx.unwrap())
    }

    fn textura(d3d: &ID3D11Device, ancho: u32, alto: u32) -> ID3D11Texture2D {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: ancho,
            Height: alto,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32,
            ..Default::default()
        };
        let mut t = None;
        // SAFETY: desc inicializada; t es la salida local.
        unsafe { d3d.CreateTexture2D(&desc, None, Some(&mut t)) }.unwrap();
        t.unwrap()
    }

    fn pixel(
        d3d: &ID3D11Device,
        ctx: &ID3D11DeviceContext,
        t: &ID3D11Texture2D,
        x: u32,
        y: u32,
    ) -> [u8; 4] {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: GetDesc rellena la estructura local.
        unsafe { t.GetDesc(&mut desc) };
        let e_desc = D3D11_TEXTURE2D_DESC {
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
            ..desc
        };
        let mut e = None;
        // SAFETY: desc inicializada; e es la salida local.
        unsafe { d3d.CreateTexture2D(&e_desc, None, Some(&mut e)) }.unwrap();
        let e = e.unwrap();
        // SAFETY: mismo formato/tamano; staging legible tras CopyResource.
        unsafe { ctx.CopyResource(&e, t) };
        let mut mapa = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: staging con lectura; se desmapea justo despues.
        unsafe { ctx.Map(&e, 0, D3D11_MAP_READ, 0, Some(&mut mapa)) }.unwrap();
        // SAFETY: (x, y) dentro de la textura, obligacion del llamante.
        let v = unsafe {
            let base =
                (mapa.pData as *const u8).add(y as usize * mapa.RowPitch as usize + x as usize * 4);
            [*base, *base.add(1), *base.add(2), *base.add(3)]
        };
        // SAFETY: empareja el Map de arriba.
        unsafe { ctx.Unmap(&e, 0) };
        v
    }

    #[test]
    #[ignore = "necesita GPU real; ejecutar con --ignored"]
    fn el_velo_oscurece_fuera_del_hueco_y_deja_el_hueco_intacto() {
        // D51: si la regla de relleno no fuera la alternada, el hueco se
        // pintaria tambien y el foco oscureceria justo lo que se ensena.
        let (d3d, ctx) = dispositivo();
        let motor = MotorRender::nuevo(&d3d).unwrap();
        let destino_tex = textura(&d3d, 128, 128);
        let destino = motor.destino_desde_textura(&destino_tex).unwrap();
        motor
            .dibujar(&destino, |p| {
                p.limpiar(Color {
                    r: 0.0,
                    g: 0.0,
                    b: 1.0,
                    a: 1.0,
                });
                p.velo(
                    RectF {
                        x: 0.0,
                        y: 0.0,
                        ancho: 128.0,
                        alto: 128.0,
                    },
                    &[(32.0, 32.0), (96.0, 32.0), (96.0, 96.0), (32.0, 96.0)],
                    Color {
                        r: 1.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    },
                );
            })
            .unwrap();
        // BGRA: fuera del hueco, rojo opaco; dentro, el azul de la limpieza.
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 8, 8), [0, 0, 255, 255]);
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 64, 64), [255, 0, 0, 255]);
    }

    #[test]
    #[ignore = "necesita GPU real; ejecutar con --ignored"]
    fn el_pintor_seguro_dibuja_sin_una_linea_de_unsafe_en_el_llamante() {
        let (d3d, ctx) = dispositivo();
        let motor = MotorRender::nuevo(&d3d).unwrap();
        let destino_tex = textura(&d3d, 128, 128);
        let destino = motor.destino_desde_textura(&destino_tex).unwrap();

        // Este bloque es exactamente lo que hara apps/pixpin: cero unsafe.
        motor
            .dibujar(&destino, |p| {
                p.limpiar(Color {
                    r: 0.0,
                    g: 0.0,
                    b: 1.0,
                    a: 1.0,
                });
                p.rellenar(
                    RectF {
                        x: 8.0,
                        y: 8.0,
                        ancho: 24.0,
                        alto: 24.0,
                    },
                    Color {
                        r: 1.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    },
                );
                p.rellenar_redondeado(
                    RectF {
                        x: 60.0,
                        y: 60.0,
                        ancho: 40.0,
                        alto: 20.0,
                    },
                    4.0,
                    Color::BLANCO,
                );
                p.trazar_discontinuo(
                    RectF {
                        x: 40.0,
                        y: 8.0,
                        ancho: 30.0,
                        alto: 30.0,
                    },
                    2.0,
                    Color::ACENTO,
                );
                p.linea((0.0, 120.0), (128.0, 120.0), 2.0, Color::BLANCO);
                p.texto("42", 100.0, 8.0, 12.0, Color::BLANCO);
                let (w, h) = p.medir_texto("970x542", 14.0);
                assert!(w > 10.0 && h > 5.0, "medir_texto devolvio ({w}, {h})");
            })
            .expect("el fotograma completo deberia dibujarse");

        // Rojo dentro del rectangulo, azul fuera: la tuberia entera funciona.
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 16, 16), [0, 0, 255, 255]);
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 110, 110), [255, 0, 0, 255]);
        // La caja blanca redondeada dejo su centro blanco.
        assert_eq!(
            pixel(&d3d, &ctx, &destino_tex, 80, 70),
            [255, 255, 255, 255]
        );
    }

    /// Luminancia aproximada de un pixel BGRA, de 0 a 1.
    fn luminancia(p: [u8; 4]) -> f32 {
        (0.114 * p[0] as f32 + 0.587 * p[1] as f32 + 0.299 * p[2] as f32) / 255.0
    }

    #[test]
    #[ignore = "necesita GPU real; ejecutar con --ignored"]
    fn un_circulo_pinta_el_centro_y_no_la_esquina_de_su_caja() {
        let (d3d, ctx) = dispositivo();
        let motor = MotorRender::nuevo(&d3d).unwrap();
        let destino_tex = textura(&d3d, 64, 64);
        let destino = motor.destino_desde_textura(&destino_tex).unwrap();
        let rojo = Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        motor
            .dibujar(&destino, |p| {
                p.limpiar(Color::BLANCO);
                p.circulo((32.0, 32.0), 30.0, rojo);
                p.anillo((32.0, 32.0), 30.0, 2.0, rojo);
            })
            .expect("el fotograma completo deberia dibujarse");
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 32, 32), [0, 0, 255, 255]);
        // Caso negativo: la esquina de la caja del circulo queda fuera de el.
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 1, 1), [255, 255, 255, 255]);
    }

    #[test]
    #[ignore = "necesita GPU real; ejecutar con --ignored"]
    fn el_degradado_es_mas_intenso_en_el_centro_que_cerca_del_borde() {
        let (d3d, ctx) = dispositivo();
        let motor = MotorRender::nuevo(&d3d).unwrap();
        let destino_tex = textura(&d3d, 64, 64);
        let destino = motor.destino_desde_textura(&destino_tex).unwrap();
        motor
            .dibujar(&destino, |p| {
                p.limpiar(Color::NEGRO);
                p.circulo_degradado(
                    (32.0, 32.0),
                    30.0,
                    Color::BLANCO,
                    Color {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 0.0,
                    },
                );
            })
            .expect("el fotograma completo deberia dibujarse");
        let centro = luminancia(pixel(&d3d, &ctx, &destino_tex, 32, 32));
        let borde = luminancia(pixel(&d3d, &ctx, &destino_tex, 32 + 27, 32));
        assert!(centro > borde, "centro {centro} <= borde {borde}");
        assert!(centro > 0.9, "el centro es blanco: {centro}");
        // Caso negativo: fuera del circulo no se pinta nada.
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 1, 1), [0, 0, 0, 255]);
    }

    #[test]
    #[ignore = "necesita GPU real; ejecutar con --ignored"]
    fn un_emoji_con_texto_color_no_sale_monocromo() {
        let (d3d, ctx) = dispositivo();
        let motor = MotorRender::nuevo(&d3d).unwrap();
        let destino_tex = textura(&d3d, 64, 64);
        let destino = motor.destino_desde_textura(&destino_tex).unwrap();
        motor
            .dibujar(&destino, |p| {
                p.limpiar(Color::BLANCO);
                p.texto_color("🟥", 8.0, 8.0, 32.0, Color::NEGRO);
            })
            .expect("el fotograma completo deberia dibujarse");
        let mut rojo = false;
        for y in 0..64 {
            for x in 0..64 {
                let [b, g, r, _] = pixel(&d3d, &ctx, &destino_tex, x, y);
                if r as f32 / 255.0 > 0.5 && (g as f32 / 255.0) < 0.3 && (b as f32 / 255.0) < 0.3 {
                    rojo = true;
                }
            }
        }
        assert!(rojo, "el cuadrado rojo tiene que salir rojo, no negro");
    }

    #[test]
    fn el_negativo_de_un_color_invierte_los_canales_y_deja_el_alfa() {
        let c = Color {
            r: 0.25,
            g: 0.5,
            b: 1.0,
            a: 0.4,
        };
        assert_eq!(
            c.invertido(),
            Color {
                r: 0.75,
                g: 0.5,
                b: 0.0,
                a: 0.4,
            }
        );
        // Dos veces, el de antes: es un interruptor, no un filtro que se
        // acumule.
        assert_eq!(c.invertido().invertido(), c);
        // Caso negativo: el blanco no se queda blanco ni el alfa se toca.
        assert_eq!(Color::BLANCO.invertido(), Color::NEGRO);
        assert_ne!(Color::BLANCO.invertido(), Color::BLANCO);
    }

    #[test]
    #[ignore = "necesita GPU real; ejecutar con --ignored"]
    fn invertir_colores_pone_en_negativo_solo_su_zona_y_lo_de_encima_no() {
        let (d3d, ctx) = dispositivo();
        let motor = MotorRender::nuevo(&d3d).unwrap();
        let destino_tex = textura(&d3d, 128, 128);
        let destino = motor.destino_desde_textura(&destino_tex).unwrap();
        // 0.2/0.6/1.0 son 51/153/255 exactos en 8 bits.
        let fondo = Color {
            r: 0.2,
            g: 0.6,
            b: 1.0,
            a: 1.0,
        };
        motor
            .dibujar(&destino, |p| {
                p.limpiar(fondo);
                // Con una transformada puesta, como el pin con su sombra: la
                // zona va en las coordenadas del pintado, no de la textura.
                p.desplazar(16.0, 16.0);
                p.invertir_colores(
                    RectF {
                        x: 0.0,
                        y: 0.0,
                        ancho: 64.0,
                        alto: 64.0,
                    },
                    16.0,
                );
                // Lo de encima (la tinta) se pinta despues: queda tal cual.
                p.rellenar(
                    RectF {
                        x: 24.0,
                        y: 24.0,
                        ancho: 8.0,
                        alto: 8.0,
                    },
                    Color {
                        r: 1.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    },
                );
            })
            .expect("el fotograma completo deberia dibujarse");
        // BGRA. Dentro de la zona, el negativo exacto: 255-51, 255-153, 0.
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 30, 60), [0, 102, 204, 255]);
        // Lo pintado despues, con su color.
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 44, 44), [0, 0, 255, 255]);
        // Casos negativos: fuera de la zona y en la esquina que el redondeo
        // deja fuera, el color de siempre.
        assert_eq!(
            pixel(&d3d, &ctx, &destino_tex, 100, 100),
            [255, 153, 51, 255]
        );
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 8, 8), [255, 153, 51, 255]);
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 17, 17), [255, 153, 51, 255]);
    }
}
