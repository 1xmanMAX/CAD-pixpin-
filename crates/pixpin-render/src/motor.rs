//! El motor Direct2D: factorias, dispositivo y contexto sobre el D3D11 de
//! la captura.
//!
//! La decision que sostiene esta fase: D2D dibuja sobre el MISMO dispositivo
//! D3D11 que posee la textura capturada. Asi el fondo del overlay es la
//! textura misma envuelta en un bitmap, sin copia y sin bajar a la CPU. Si
//! fueran dispositivos distintos, harian falta texturas compartidas y
//! sincronizacion — complejidad que no compra nada.
//!
//! `pixpin-render` es L1: recibe `&ID3D11Device` y no sabe de donde sale.

use windows::Win32::Graphics::Direct2D::Common::{
    D2D_SIZE_U, D2D1_ALPHA_MODE, D2D1_ALPHA_MODE_IGNORE, D2D1_ALPHA_MODE_PREMULTIPLIED,
    D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_OPTIONS, D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_NONE,
    D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1, D2D1_DEVICE_CONTEXT_OPTIONS_NONE,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1CreateFactory, ID2D1Bitmap1, ID2D1Device,
    ID2D1DeviceContext, ID2D1Factory1, ID2D1SolidColorBrush,
};
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11Texture2D};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWriteCreateFactory, IDWriteFactory,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM,
};
use windows::Win32::Graphics::Dxgi::{IDXGIDevice, IDXGISurface};
use windows::core::Interface;

#[derive(Debug, thiserror::Error)]
pub enum ErrorRender {
    #[error("error de Windows al dibujar: {0}")]
    Windows(#[source] windows::core::Error),
    #[error("la textura no expone una superficie DXGI")]
    SinDxgi,
    #[error("el buffer tiene {tiene} bytes pero {ancho}x{alto} necesita {espera}")]
    TamanoIncoherente {
        ancho: u32,
        alto: u32,
        tiene: usize,
        espera: usize,
    },
}

/// A mano y no con `#[from]`: **todo** error de Windows que sale de pintar o
/// presentar pasa por aqui, y es el unico sitio donde se mira si es que el
/// dispositivo se perdio (`perdida::senalar`). Asi ninguna ventana tiene que
/// acordarse de comprobarlo.
impl From<windows::core::Error> for ErrorRender {
    fn from(e: windows::core::Error) -> Self {
        crate::perdida::senalar(&e);
        ErrorRender::Windows(e)
    }
}

impl ErrorRender {
    /// Si el fallo es que el dispositivo grafico se perdio.
    pub fn es_perdida(&self) -> bool {
        matches!(self, ErrorRender::Windows(e) if crate::perdida::es_perdida(e.code()))
    }
}

/// Validacion pura del buffer RGBA, separada para probarse sin GPU.
pub fn validar_tamano_rgba(ancho: u32, alto: u32, bytes: usize) -> Result<(), ErrorRender> {
    let espera = ancho as usize * alto as usize * 4;
    if ancho == 0 || alto == 0 || bytes != espera {
        return Err(ErrorRender::TamanoIncoherente {
            ancho,
            alto,
            tiene: bytes,
            espera,
        });
    }
    Ok(())
}

/// Color RGBA en coma flotante 0..1, el idioma nativo de Direct2D.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const NEGRO: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const BLANCO: Color = Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    /// El azul de acento de la seleccion.
    pub const ACENTO: Color = Color {
        r: 0.13,
        g: 0.55,
        b: 0.95,
        a: 1.0,
    };

    /// **El color en negativo**: cada canal pasa a `1 - canal` y el alfa se
    /// queda. Es lo que hace `Pintor::invertir_colores` con cada pixel de una
    /// zona; con esto, quien pinta algo que NO se invierte (la tinta de un
    /// lector sobre su papel) sabe sobre que papel queda de verdad.
    pub fn invertido(self) -> Color {
        Color {
            r: 1.0 - self.r,
            g: 1.0 - self.g,
            b: 1.0 - self.b,
            a: self.a,
        }
    }

    /// El velo que oscurece lo no seleccionado.
    pub fn oscurecido() -> Color {
        Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.55,
        }
    }

    pub fn a_d2d(self) -> D2D1_COLOR_F {
        D2D1_COLOR_F {
            r: self.r,
            g: self.g,
            b: self.b,
            a: self.a,
        }
    }
}

/// RGBA recto (lo que da un PNG) a premultiplicado (lo que espera Direct2D
/// para componer con alfa). Pura, para probarla sin GPU.
pub fn premultiplicar(rgba: &[u8]) -> Vec<u8> {
    let mut v = rgba.to_vec();
    for p in v.chunks_exact_mut(4) {
        let a = p[3] as u32;
        if a == 255 {
            continue;
        }
        for c in &mut p[..3] {
            *c = ((*c as u32 * a + 127) / 255) as u8;
        }
    }
    v
}

/// Cuantos objetos de Direct2D/DirectWrite se han creado en el fotograma en
/// curso.
///
/// La guia de rendimiento de Direct2D dice que crear y destruir recursos es
/// caro en hardware; el diagnostico del 2026-09-19 sospechaba que un
/// fotograma de paneo creaba una geometria por cada pasada de rough.js. Sin
/// un numero eso no se puede ni confirmar ni dar por arreglado, asi que el
/// motor los cuenta. Son `Cell<u32>` y un incremento: con la medicion
/// apagada tampoco cuesta nada, no hace falta compilar dos veces.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Contadores {
    /// `CreatePathGeometry` (formas de rough.js, tinta sin cachear, velo).
    pub geometrias: u32,
    /// `CreateFilledGeometryRealization` / `CreateStrokedGeometryRealization`.
    pub realizaciones: u32,
    /// `CreateTextFormat` + `CreateTextLayout` de DirectWrite.
    pub disposiciones: u32,
    /// `CreateSolidColorBrush`.
    pub pinceles: u32,
    /// Bitmaps: subidos, envueltos desde una textura o desde el backbuffer.
    pub bitmaps: u32,
}

impl Contadores {
    /// Cuantos objetos en total. Es el numero que tiene que ser 0 en un
    /// fotograma de paneo sostenido.
    pub fn total(self) -> u32 {
        self.geometrias + self.realizaciones + self.disposiciones + self.pinceles + self.bitmaps
    }
}

pub struct MotorRender {
    fabrica: ID2D1Factory1,
    _dispositivo: ID2D1Device,
    contexto: ID2D1DeviceContext,
    dwrite: IDWriteFactory,
    /// Pinceles ya creados, por color. Crear uno por primitiva costaba casi
    /// tanto como la geometria (diagnostico de E1). Uno por color y no uno
    /// solo con `SetColor`: hay primitivas que piden dos pinceles a la vez y
    /// el segundo pisaria el color del primero.
    ///
    /// Un mapa y no una lista con tope de 32: el universo pide mas de 32
    /// colores por fotograma (cada galaxia el suyo, con dos o tres alfas), y
    /// con la lista echando siempre el mas viejo, NINGUNA busqueda acertaba
    /// y se creaba un pincel por primitiva (medido: `universo::sesion::
    /// medir`).
    pinceles: std::cell::RefCell<std::collections::HashMap<[u32; 4], ID2D1SolidColorBrush>>,
    /// Disposiciones de texto ya construidas (ver `DisposicionCacheada`).
    /// DirectWrite tarda decenas de microsegundos en cada una, y el mismo
    /// rotulo se pedia dos veces por fotograma (medir y pintar): con veinte
    /// galaxias eran 3 ms de CPU solo en rotulos.
    pub(crate) textos:
        std::cell::RefCell<std::collections::HashMap<u64, crate::lienzo::DisposicionCacheada>>,
    /// Cuantos fotogramas se han abierto: la edad de lo cacheado.
    pub(crate) fotograma: std::cell::Cell<u64>,
    /// El brillo de cada color (ver `Pintor::brillo`), por su RGB.
    pub(crate) brillos: std::cell::RefCell<std::collections::HashMap<[u8; 3], ID2D1Bitmap1>>,
    /// Geometrias de los iconos, por la direccion de su trazado estatico.
    /// Un icono se lee y se construye UNA vez; despues cada fotograma solo
    /// cambia la transformada. `None` recuerda un trazado que no se pudo
    /// construir, para no reintentarlo sesenta veces por segundo.
    pub(crate) iconos: std::cell::RefCell<
        std::collections::HashMap<
            usize,
            Option<windows::Win32::Graphics::Direct2D::ID2D1PathGeometry1>,
        >,
    >,
    /// Los cuatro estilos de trazo de los iconos: extremo y union, redondos
    /// o no.
    pub(crate) estilos_icono:
        std::cell::RefCell<[Option<windows::Win32::Graphics::Direct2D::ID2D1StrokeStyle>; 4]>,
    /// Objetos creados en el fotograma en curso (ver `Contadores`).
    pub(crate) creados: std::cell::Cell<Contadores>,
    /// El bitmap del trazo de grafito en curso (ver `grafito::Vivo`). Vive
    /// en el motor y no suelto porque es de su dispositivo: muere con el.
    pub(crate) grafito_vivo: std::cell::RefCell<Option<crate::grafito::Vivo>>,
    /// Un segundo contexto del mismo dispositivo, solo para grabar la
    /// mascara de `Pintor::invertir_colores` en una lista de ordenes. Con el
    /// principal no se puede: cambiarle el destino a mitad de fotograma, con
    /// recortes y capas empujados, lo deja en error. Se crea la primera vez.
    pub(crate) contexto_mascara: std::cell::RefCell<Option<ID2D1DeviceContext>>,
}

impl MotorRender {
    pub fn nuevo(d3d: &ID3D11Device) -> Result<Self, ErrorRender> {
        // SAFETY: crear la factoria no tiene precondiciones; single-threaded
        // porque todo el dibujo ocurre en el hilo de interfaz.
        let fabrica: ID2D1Factory1 =
            unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };
        let dxgi: IDXGIDevice = d3d.cast().map_err(|_| ErrorRender::SinDxgi)?;
        // SAFETY: `dxgi` es una vista valida del dispositivo del llamante; el
        // conteo de referencias COM lo mantiene vivo mientras viva el motor.
        let dispositivo = unsafe { fabrica.CreateDevice(&dxgi)? };
        // SAFETY: el dispositivo D2D se acaba de crear y esta vivo.
        let contexto =
            unsafe { dispositivo.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)? };
        // SAFETY: crear la factoria compartida de DirectWrite no tiene
        // precondiciones.
        let dwrite: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
        Ok(Self {
            fabrica,
            _dispositivo: dispositivo,
            contexto,
            dwrite,
            pinceles: std::cell::RefCell::new(std::collections::HashMap::new()),
            textos: std::cell::RefCell::new(std::collections::HashMap::new()),
            fotograma: std::cell::Cell::new(0),
            brillos: std::cell::RefCell::new(std::collections::HashMap::new()),
            iconos: std::cell::RefCell::new(std::collections::HashMap::new()),
            estilos_icono: std::cell::RefCell::new([None, None, None, None]),
            creados: std::cell::Cell::new(Contadores::default()),
            grafito_vivo: std::cell::RefCell::new(None),
            contexto_mascara: std::cell::RefCell::new(None),
        })
    }

    /// El contexto de las mascaras (ver `contexto_mascara`), creado al
    /// pedirlo por primera vez. `None` si el dispositivo no da otro.
    pub(crate) fn contexto_de_mascaras(&self) -> Option<ID2D1DeviceContext> {
        let mut c = self.contexto_mascara.borrow_mut();
        if c.is_none() {
            // SAFETY: el dispositivo D2D es del motor y esta vivo.
            *c = unsafe {
                self._dispositivo
                    .CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)
            }
            .ok();
        }
        c.clone()
    }

    pub fn fabrica(&self) -> &ID2D1Factory1 {
        &self.fabrica
    }

    /// **Devuelve al sistema la reserva de memoria del dispositivo.**
    ///
    /// Direct2D guarda de reserva lo que ha ido necesitando (texturas de
    /// teselar, de mezclar, de recortar) y el controlador guarda sus propios
    /// buferes; ninguno de los dos lo suelta mientras viva el dispositivo.
    /// En la HD 4000 del equipo minimo la memoria de video ES la del
    /// sistema: lo que el editor retiene de reserva se lo quita a todo lo
    /// demas mientras siga abierto, aunque no este dibujando.
    ///
    /// Es lo que Microsoft pide hacer al dejar de estar en primer plano:
    /// `ID2D1Device::ClearResources`, luego `ClearState` del contexto de
    /// D3D (condicion de `Trim`) y `IDXGIDevice3::Trim`. No toca nada que se
    /// vea ni nada que el editor guarde (las realizaciones, los pinceles y
    /// los bitmaps siguen vivos): lo que se suelta se vuelve a pedir solo si
    /// hace falta. Hay que llamarlo entre fotogramas, nunca con un
    /// `BeginDraw` abierto.
    pub fn devolver_memoria(&self, d3d: &ID3D11Device) {
        use windows::Win32::Graphics::Dxgi::IDXGIDevice3;
        // SAFETY: fuera de cualquier `BeginDraw` (obligacion del llamante,
        // dicha arriba); los dos dispositivos son del motor y siguen vivos.
        unsafe {
            self._dispositivo.ClearResources(0);
            if let Ok(contexto) = d3d.GetImmediateContext() {
                contexto.ClearState();
                contexto.Flush();
            }
            if let Ok(dxgi) = d3d.cast::<IDXGIDevice3>() {
                dxgi.Trim();
            }
        }
    }

    pub fn contexto(&self) -> &ID2D1DeviceContext {
        &self.contexto
    }

    pub fn dwrite(&self) -> &IDWriteFactory {
        &self.dwrite
    }

    /// Objetos de Direct2D/DirectWrite creados desde que empezo el fotograma
    /// en curso (`MotorRender::dibujar` los pone a cero al abrirlo).
    pub fn contadores(&self) -> Contadores {
        self.creados.get()
    }

    /// Suma uno a un contador. `cambio` recibe la estructura y toca el campo
    /// que sea: asi los sitios de creacion no tienen que saber de `Cell`.
    pub(crate) fn conto(&self, cambio: impl FnOnce(&mut Contadores)) {
        let mut c = self.creados.get();
        cambio(&mut c);
        self.creados.set(c);
    }

    /// Tope: un pincel son unos cientos de bytes, y el tope evita que un
    /// degradado pintado a mano llene la memoria de pinceles. Al llegar se
    /// vacia entero: los que sigan haciendo falta vuelven en un fotograma.
    const MAX_PINCELES: usize = 512;

    pub(crate) fn pincel(&self, color: Color) -> Option<ID2D1SolidColorBrush> {
        let clave = [
            color.r.to_bits(),
            color.g.to_bits(),
            color.b.to_bits(),
            color.a.to_bits(),
        ];
        let mut mapa = self.pinceles.borrow_mut();
        if let Some(p) = mapa.get(&clave) {
            return Some(p.clone());
        }
        // SAFETY: crear un pincel sobre el contexto vivo no tiene mas
        // precondiciones.
        let p = unsafe {
            self.contexto
                .CreateSolidColorBrush(&color.a_d2d(), None)
                .ok()?
        };
        self.conto(|c| c.pinceles += 1);
        if mapa.len() >= Self::MAX_PINCELES {
            mapa.clear();
        }
        mapa.insert(clave, p.clone());
        Some(p)
    }

    /// Los recursos cacheados que son del dispositivo (pinceles, brillos, los
    /// mapas del halo de las cotas) se
    /// olvidan: tras perderse el dispositivo ya no valen. Las disposiciones
    /// de texto son de DirectWrite y se quedan.
    pub fn olvidar_recursos_de_dispositivo(&self) {
        self.pinceles.borrow_mut().clear();
        self.brillos.borrow_mut().clear();
        crate::lienzo::olvidar_halos(self);
    }

    /// Cuantas disposiciones de texto hay guardadas: para las pruebas.
    pub fn textos_cacheados(&self) -> usize {
        self.textos.borrow().len()
    }

    /// Mide un texto ajustado a un ancho maximo, **fuera** de un fotograma.
    /// DirectWrite no necesita destino para medir, y quien crea una ventana
    /// necesita el tamano ANTES de tenerla: sin esto, una nota no podria
    /// saber de que tamano nace.
    pub fn medir_texto(&self, texto: &str, tam: f32, ancho_max: f32) -> (f32, f32) {
        self.medir_parrafo(texto, tam, ancho_max, &[])
    }

    /// Como `medir_texto`, con tramos de estilo (negrita, cursiva, mono):
    /// lo que necesita una nota en Markdown para saber de que tamano nace.
    pub fn medir_parrafo(
        &self,
        texto: &str,
        tam: f32,
        ancho_max: f32,
        tramos: &[crate::lienzo::Tramo],
    ) -> (f32, f32) {
        crate::lienzo::disposicion_dwrite(&self.dwrite, texto, tam, ancho_max, tramos, false)
            .map(|(_, w, h)| (w, h))
            .unwrap_or((0.0, 0.0))
    }

    /// Envuelve una textura como bitmap de SOLO LECTURA para dibujarla.
    /// No copia pixeles: el bitmap ES la textura.
    pub fn bitmap_desde_textura(&self, t: &ID3D11Texture2D) -> Result<ID2D1Bitmap1, ErrorRender> {
        self.envolver(t, D2D1_BITMAP_OPTIONS_NONE)
    }

    /// Envuelve una textura como DESTINO de dibujo (`SetTarget`).
    pub fn destino_desde_textura(&self, t: &ID3D11Texture2D) -> Result<ID2D1Bitmap1, ErrorRender> {
        self.envolver(t, D2D1_BITMAP_OPTIONS_TARGET)
    }

    /// Sube pixeles RGBA de CPU como bitmap D2D. Para los pines: la imagen
    /// viene del almacen (PNG en disco), no de una textura de captura. El
    /// alfa se IGNORA: el pin pinta su tarjeta debajo y una captura no trae
    /// alfa util.
    pub fn bitmap_desde_pixeles(
        &self,
        ancho: u32,
        alto: u32,
        rgba: &[u8],
    ) -> Result<ID2D1Bitmap1, ErrorRender> {
        validar_tamano_rgba(ancho, alto, rgba.len())?;
        self.crear_bitmap_rgba(ancho, alto, rgba, D2D1_ALPHA_MODE_IGNORE)
    }

    /// Como `bitmap_desde_pixeles`, pero respetando el alfa (D138): el fondo
    /// del lienzo es blanco, y un PNG transparente subido con el alfa
    /// ignorado salia con fondo negro.
    pub fn bitmap_desde_pixeles_premultiplicado(
        &self,
        ancho: u32,
        alto: u32,
        rgba: &[u8],
    ) -> Result<ID2D1Bitmap1, ErrorRender> {
        validar_tamano_rgba(ancho, alto, rgba.len())?;
        let pre = premultiplicar(rgba);
        self.crear_bitmap_rgba(ancho, alto, &pre, D2D1_ALPHA_MODE_PREMULTIPLIED)
    }

    /// **Un mapa donde pintar y que luego se pinta** (destino y fuente a la
    /// vez, BGRA premultiplicado, sin datos). Es lo que guarda lo de dentro
    /// de una lupa ya pintado: se rehace cuando cambia lo que mira y, entre
    /// tanto, cada fotograma solo lo copia.
    pub fn mapa_de_dibujo(&self, ancho: u32, alto: u32) -> Result<ID2D1Bitmap1, ErrorRender> {
        let propiedades = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
            colorContext: std::mem::ManuallyDrop::new(None),
        };
        // SAFETY: sin datos iniciales (D2D solo reserva) y con el tamano
        // sujeto a uno como minimo; el contexto vive lo que `self`.
        let bitmap = unsafe {
            self.contexto.CreateBitmap(
                D2D_SIZE_U {
                    width: ancho.max(1),
                    height: alto.max(1),
                },
                None,
                0,
                &propiedades,
            )?
        };
        self.conto(|c| c.bitmaps += 1);
        Ok(bitmap)
    }

    /// El lado mayor que admite un bitmap en este dispositivo (D139). La
    /// HD 4000 no sube texturas enormes: lo que pase de aqui se reduce antes.
    pub fn lado_maximo_bitmap(&self) -> u32 {
        // SAFETY: consulta sin precondiciones sobre el contexto vivo.
        unsafe { self.contexto.GetMaximumBitmapSize() }
    }

    fn crear_bitmap_rgba(
        &self,
        ancho: u32,
        alto: u32,
        datos: &[u8],
        alfa: D2D1_ALPHA_MODE,
    ) -> Result<ID2D1Bitmap1, ErrorRender> {
        let propiedades = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_R8G8B8A8_UNORM,
                alphaMode: alfa,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_NONE,
            colorContext: std::mem::ManuallyDrop::new(None),
        };
        // SAFETY: el puntero y el paso describen exactamente `datos` (el
        // llamante ya valido ancho*alto*4), que vive durante la llamada; D2D
        // copia los datos al crear el bitmap.
        let bitmap = unsafe {
            self.contexto.CreateBitmap(
                D2D_SIZE_U {
                    width: ancho,
                    height: alto,
                },
                Some(datos.as_ptr() as *const _),
                ancho * 4,
                &propiedades,
            )?
        };
        self.conto(|c| c.bitmaps += 1);
        Ok(bitmap)
    }

    /// **Una textura que este motor pinta y otro del mismo dispositivo lee.**
    ///
    /// Es la que alimenta a una ventana en vivo de una zona del lienzo: el
    /// editor la pinta cuando cambia lo de dentro y el pin la envuelve una
    /// vez como fuente (`bitmap_desde_textura`) y solo repinta. BGRA,
    /// destino de dibujo y fuente de sombreado; devuelve la textura y su
    /// bitmap de destino.
    pub fn textura_de_destino(
        &self,
        d3d: &ID3D11Device,
        ancho: u32,
        alto: u32,
    ) -> Result<(ID3D11Texture2D, ID2D1Bitmap1), ErrorRender> {
        use windows::Win32::Graphics::Direct3D11::{
            D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_TEXTURE2D_DESC,
            D3D11_USAGE_DEFAULT,
        };
        use windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC;
        let desc = D3D11_TEXTURE2D_DESC {
            Width: ancho.max(1),
            Height: alto.max(1),
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let mut textura = None;
        // SAFETY: descripcion completa de una textura sin datos iniciales; el
        // dispositivo es el del llamante y esta vivo durante la llamada.
        unsafe { d3d.CreateTexture2D(&desc, None, Some(&mut textura))? };
        let textura = textura.ok_or(ErrorRender::SinDxgi)?;
        let destino = self.envolver(&textura, D2D1_BITMAP_OPTIONS_TARGET)?;
        Ok((textura, destino))
    }

    /// Envuelve el backbuffer de un swapchain como destino.
    ///
    /// Los buferes de un swapchain exigen `TARGET | CANNOT_DRAW`: no pueden
    /// usarse como fuente de dibujo, y D2D rechaza con E_INVALIDARG el
    /// intento de envolverlos solo como TARGET — se descubrio ejecutando el
    /// test de la superficie, no leyendo documentacion.
    pub fn destino_backbuffer(&self, t: &ID3D11Texture2D) -> Result<ID2D1Bitmap1, ErrorRender> {
        self.envolver(
            t,
            D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
        )
    }

    fn envolver(
        &self,
        t: &ID3D11Texture2D,
        opciones: D2D1_BITMAP_OPTIONS,
    ) -> Result<ID2D1Bitmap1, ErrorRender> {
        let superficie: IDXGISurface = t.cast().map_err(|_| ErrorRender::SinDxgi)?;
        let propiedades = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                // Todo destino de dibujo va premultiplicado (los swapchain de
                // composicion lo exigen); las fuentes ignoran el alfa porque
                // la captura no trae un canal alfa util y componer contra esa
                // basura mancharia el velo. La comprobacion es por BIT, no
                // por igualdad: el backbuffer lleva TARGET | CANNOT_DRAW.
                alphaMode: if (opciones.0 & D2D1_BITMAP_OPTIONS_TARGET.0) != 0 {
                    D2D1_ALPHA_MODE_PREMULTIPLIED
                } else {
                    D2D1_ALPHA_MODE_IGNORE
                },
            },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: opciones,
            colorContext: std::mem::ManuallyDrop::new(None),
        };
        // SAFETY: la superficie procede de la textura del llamante, viva
        // durante la llamada; D2D retiene su propia referencia al crearla.
        let bitmap = unsafe {
            self.contexto
                .CreateBitmapFromDxgiSurface(&superficie, Some(&propiedades))?
        };
        self.conto(|c| c.bitmaps += 1);
        Ok(bitmap)
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

    /// Un dispositivo D3D11 minimo, solo para el test. No se usa el de
    /// pixpin-capture porque render (L1) no puede depender de capture (L2)
    /// ni siquiera en dev-dependencies: el test de capas tambien las mira.
    fn dispositivo_de_prueba() -> (ID3D11Device, ID3D11DeviceContext) {
        let mut d3d = None;
        let mut ctx = None;
        // SAFETY: punteros de salida locales inicializados a None; constantes
        // documentadas en el resto de parametros.
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
            .expect("el test necesita GPU real");
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
        // SAFETY: desc inicializada; t es la variable de salida local.
        unsafe { d3d.CreateTexture2D(&desc, None, Some(&mut t)) }.unwrap();
        t.unwrap()
    }

    /// Lee un pixel BGRA de una textura, via staging. El llamante garantiza
    /// que (x, y) cae dentro de la textura.
    fn pixel(
        d3d: &ID3D11Device,
        ctx: &ID3D11DeviceContext,
        t: &ID3D11Texture2D,
        x: u32,
        y: u32,
    ) -> [u8; 4] {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: GetDesc solo rellena la estructura local.
        unsafe { t.GetDesc(&mut desc) };
        let escenificada_desc = D3D11_TEXTURE2D_DESC {
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
            ..desc
        };
        let mut e = None;
        // SAFETY: desc inicializada; e es la salida local.
        unsafe { d3d.CreateTexture2D(&escenificada_desc, None, Some(&mut e)) }.unwrap();
        let e = e.unwrap();
        // SAFETY: mismo formato y tamano, usos compatibles con CopyResource.
        unsafe { ctx.CopyResource(&e, t) };
        let mut mapa = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: textura staging con lectura; se desmapea justo despues.
        unsafe { ctx.Map(&e, 0, D3D11_MAP_READ, 0, Some(&mut mapa)) }.unwrap();
        let paso = mapa.RowPitch as usize;
        // SAFETY: pData apunta a alto*paso bytes mientras este mapeada; el
        // offset queda dentro porque (x, y) esta dentro de la textura
        // (obligacion del llamante de este helper de test).
        let v = unsafe {
            let base = (mapa.pData as *const u8).add(y as usize * paso + x as usize * 4);
            [*base, *base.add(1), *base.add(2), *base.add(3)]
        };
        // SAFETY: se desmapea lo que se mapeo, una vez.
        unsafe { ctx.Unmap(&e, 0) };
        v
    }

    #[test]
    #[ignore = "necesita GPU real; ejecutar con --ignored"]
    fn dibujar_un_rectangulo_rojo_deja_pixeles_rojos_en_la_textura() {
        let (d3d, ctx) = dispositivo_de_prueba();
        let motor = MotorRender::nuevo(&d3d).expect("deberia crearse el motor D2D");
        let destino = textura(&d3d, 64, 64);
        let objetivo = motor.destino_desde_textura(&destino).unwrap();

        let c = motor.contexto();
        // SAFETY: el contexto D2D esta vivo (lo mantiene `motor`) y las
        // llamadas siguen el protocolo SetTarget/BeginDraw/EndDraw.
        unsafe {
            c.SetTarget(&objetivo);
            c.BeginDraw();
            c.Clear(Some(
                &Color {
                    r: 0.0,
                    g: 0.0,
                    b: 1.0,
                    a: 1.0,
                }
                .a_d2d(),
            ));
            let pincel = c
                .CreateSolidColorBrush(
                    &Color {
                        r: 1.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }
                    .a_d2d(),
                    None,
                )
                .unwrap();
            c.FillRectangle(
                &windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F {
                    left: 8.0,
                    top: 8.0,
                    right: 32.0,
                    bottom: 32.0,
                },
                &pincel,
            );
            c.EndDraw(None, None).unwrap();
            c.SetTarget(None);
        }

        // Dentro del rectangulo: rojo (BGRA = 0,0,255). Fuera: azul.
        assert_eq!(pixel(&d3d, &ctx, &destino, 16, 16), [0, 0, 255, 255]);
        // Caso negativo: si Clear no funcionara o el rect lo cubriera todo,
        // este pixel tambien seria rojo.
        assert_eq!(pixel(&d3d, &ctx, &destino, 50, 50), [255, 0, 0, 255]);
    }

    #[test]
    #[ignore = "necesita GPU real; ejecutar con --ignored"]
    fn un_bitmap_desde_pixeles_dibuja_esos_pixeles() {
        let (d3d, ctx) = dispositivo_de_prueba();
        let motor = MotorRender::nuevo(&d3d).unwrap();
        // 2x1: rojo, verde (RGBA).
        let bitmap = motor
            .bitmap_desde_pixeles(2, 1, &[255, 0, 0, 255, 0, 255, 0, 255])
            .expect("deberia subir");
        let destino_tex = textura(&d3d, 2, 1);
        let destino = motor.destino_desde_textura(&destino_tex).unwrap();
        motor
            .dibujar(&destino, |p| {
                p.bitmap(
                    &bitmap,
                    crate::lienzo::RectF {
                        x: 0.0,
                        y: 0.0,
                        ancho: 2.0,
                        alto: 1.0,
                    },
                    None,
                    true,
                );
            })
            .unwrap();
        // El destino es BGRA: rojo = [0,0,255,255].
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 0, 0), [0, 0, 255, 255]);
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 1, 0), [0, 255, 0, 255]);
    }

    #[test]
    fn un_buffer_incoherente_no_llega_a_la_gpu() {
        // Caso negativo y ademas corre SIN GPU: la validacion es previa.
        assert!(
            validar_tamano_rgba(2, 2, 7).is_err(),
            "7 bytes no son 2x2x4"
        );
        assert!(validar_tamano_rgba(0, 2, 0).is_err(), "ancho cero no vale");
        assert!(validar_tamano_rgba(2, 2, 16).is_ok());
    }

    #[test]
    fn premultiplicar_multiplica_el_color_por_el_alfa_y_no_toca_lo_opaco() {
        let v = premultiplicar(&[255, 0, 0, 128, 10, 20, 30, 0, 1, 2, 3, 255]);
        assert_eq!(&v[0..4], &[128, 0, 0, 128]);
        assert_eq!(&v[4..8], &[0, 0, 0, 0], "transparente total es negro cero");
        assert_eq!(&v[8..12], &[1, 2, 3, 255], "lo opaco queda igual");
    }

    #[test]
    #[ignore = "necesita GPU real; ejecutar con --ignored"]
    fn un_png_transparente_se_pinta_sobre_el_blanco_y_no_sobre_negro() {
        // D138. Pixel 0: transparente total. Pixel 1: rojo al 50 %.
        let (d3d, ctx) = dispositivo_de_prueba();
        let motor = MotorRender::nuevo(&d3d).unwrap();
        let rgba = [0, 0, 0, 0, 255, 0, 0, 128];
        let con_alfa = motor
            .bitmap_desde_pixeles_premultiplicado(2, 1, &rgba)
            .expect("deberia subir");
        let destino_tex = textura(&d3d, 2, 1);
        let destino = motor.destino_desde_textura(&destino_tex).unwrap();
        let rect = crate::lienzo::RectF {
            x: 0.0,
            y: 0.0,
            ancho: 2.0,
            alto: 1.0,
        };
        motor
            .dibujar(&destino, |p| {
                p.limpiar(Color::BLANCO);
                p.bitmap_con(&con_alfa, rect, None, crate::lienzo::Interpolacion::Vecino);
            })
            .unwrap();
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 0, 0), [255, 255, 255, 255]);
        let [b, g, r, _] = pixel(&d3d, &ctx, &destino_tex, 1, 0);
        assert!(r >= 253, "rojo {r}");
        assert!(
            (125..=130).contains(&g) && (125..=130).contains(&b),
            "{g} {b}"
        );

        // Caso negativo: con el alfa ignorado (lo de antes) sale negro.
        let sin_alfa = motor.bitmap_desde_pixeles(2, 1, &rgba).unwrap();
        motor
            .dibujar(&destino, |p| {
                p.limpiar(Color::BLANCO);
                p.bitmap_con(&sin_alfa, rect, None, crate::lienzo::Interpolacion::Vecino);
            })
            .unwrap();
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 0, 0), [0, 0, 0, 255]);
    }

    #[test]
    #[ignore = "necesita GPU real; ejecutar con --ignored"]
    fn un_bitmap_reducido_se_pinta_estirado_a_su_tamano_logico() {
        // D139: la imagen que no cabia se sube a menos pixeles y se pinta en
        // el rectangulo de su tamano real. 2x2 subido, 4x4 pintado.
        let (d3d, ctx) = dispositivo_de_prueba();
        let motor = MotorRender::nuevo(&d3d).unwrap();
        assert!(
            motor.lado_maximo_bitmap() >= 2048,
            "ninguna GPU con Direct2D baja de 2048"
        );
        let rojo = [255u8, 0, 0, 255].repeat(4);
        let b = motor
            .bitmap_desde_pixeles_premultiplicado(2, 2, &rojo)
            .unwrap();
        let destino_tex = textura(&d3d, 8, 8);
        let destino = motor.destino_desde_textura(&destino_tex).unwrap();
        motor
            .dibujar(&destino, |p| {
                p.limpiar(Color::BLANCO);
                p.bitmap_con(
                    &b,
                    crate::lienzo::RectF {
                        x: 0.0,
                        y: 0.0,
                        ancho: 4.0,
                        alto: 4.0,
                    },
                    None,
                    crate::lienzo::Interpolacion::Vecino,
                );
            })
            .unwrap();
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 3, 3), [0, 0, 255, 255]);
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 4, 4), [255, 255, 255, 255]);
    }

    #[test]
    #[ignore = "necesita GPU real; ejecutar con --ignored"]
    fn un_bitmap_de_solo_lectura_envuelve_la_textura_sin_copiarla() {
        let (d3d, _ctx) = dispositivo_de_prueba();
        let motor = MotorRender::nuevo(&d3d).unwrap();
        let t = textura(&d3d, 32, 32);
        let bitmap = motor.bitmap_desde_textura(&t).expect("deberia envolverse");
        // SAFETY: GetSize es una lectura sin precondiciones sobre un bitmap vivo.
        let tam = unsafe { bitmap.GetSize() };
        assert_eq!((tam.width as u32, tam.height as u32), (32, 32));
    }
}
