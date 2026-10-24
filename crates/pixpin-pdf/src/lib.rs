//! pixpin-pdf — abrir un PDF y dibujar sus paginas como imagenes.
//!
//! Este crate habla con el sistema operativo o con librerias C. El `unsafe`
//! esta permitido, pero cada bloque lleva su comentario `// SAFETY:`.
//!
//! Se usa `Windows.Data.Pdf`, que viene de serie en Windows 10 y 11. Es la
//! misma decision que ya se tomo con `Windows.Media.Ocr` para el texto y con
//! Media Foundation para el video: pdfium son varios megabytes de binario
//! nativo que compilar, firmar y actualizar, y aqui solo hace falta ver
//! paginas. Lo que trae el sistema las ve igual de bien y no anade un byte al
//! ejecutable.
//!
//! El documento se abre UNA vez y se guarda ([`Documento`]); renderizar es un
//! metodo sobre el, no una funcion suelta que reabra el fichero por pagina.
//! Reabrir cuesta leer y parsear el PDF entero cada vez, y este programa va
//! dirigido a equipos con pocos recursos.
#![deny(clippy::undocumented_unsafe_blocks)]

pub mod aligerar;
pub mod cocido;
pub mod con_anotaciones;
pub mod enlaces;
pub mod escribir;
pub mod letra;
pub mod plano;
pub mod plano_web;
mod png;
pub mod texto;
pub mod union;

use pixpin_codec::imagen::ImagenRgba;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use windows::Data::Pdf::{PdfDocument, PdfPageRenderOptions};
use windows::Graphics::Imaging::{
    BitmapAlphaMode, BitmapDecoder, BitmapEncoder, BitmapInterpolationMode, BitmapPixelFormat,
    BitmapTransform, ColorManagementMode, ExifOrientationMode,
};
use windows::Storage::StorageFile;
use windows::Storage::Streams::InMemoryRandomAccessStream;
use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
use windows::core::HSTRING;
use windows_future::{AsyncStatus, IAsyncAction, IAsyncOperation};

/// Ancho maximo que se acepta al renderizar.
///
/// No es un capricho: la imagen acaba subiendo a una textura de Direct3D 11, y
/// 16384 es el lado maximo que garantiza el nivel de caracteristicas 11_0.
/// Ademas pone un techo a la memoria: una pagina cuadrada a este ancho ya son
/// mil megabytes de RGBA. Sin el limite, un ancho absurdo por un error de
/// calculo se convierte en una reserva gigante y el proceso muere sin
/// explicacion.
pub const ANCHO_MAXIMO: u32 = 16384;

/// Cuantos bytes del principio del fichero se miran para decidir si es un PDF.
///
/// La norma permite basura antes de `%PDF-`, y los lectores de verdad la
/// buscan dentro del primer kilobyte. Se hace igual para no rechazar ficheros
/// que Windows si sabe abrir.
const CABECERA_MIRADA: usize = 1024;

/// `HRESULT_FROM_WIN32(ERROR_WRONG_PASSWORD)`: lo que devuelve
/// `Windows.Data.Pdf` cuando el documento esta cifrado y no se dio la clave.
const HR_CONTRASENA: windows::core::HRESULT = windows::core::HRESULT(0x8007_052Bu32 as i32);

#[derive(Debug, thiserror::Error)]
pub enum ErrorPdf {
    #[error("no existe el fichero {0}")]
    NoExiste(PathBuf),
    /// El fichero existe pero no lleva la marca `%PDF-` al principio. Se
    /// distingue de `Corrupto` porque el arreglo es otro: aqui el usuario
    /// arrastro un .docx o un .zip, no un PDF roto.
    #[error("{0} no es un PDF")]
    NoEsPdf(PathBuf),
    /// Es un PDF, pero pide contrasena. No se ofrece abrirlo a ciegas.
    #[error("{0} esta cifrado y pide contrasena")]
    Cifrado(PathBuf),
    /// Es un PDF por la cabecera, pero Windows no consigue cargarlo.
    #[error("{ruta} es un PDF danado o incompleto")]
    Corrupto {
        ruta: PathBuf,
        #[source]
        fuente: windows::core::Error,
    },
    /// Un PDF de cero paginas es valido para la norma y no sirve para nada
    /// aqui. Se rechaza al abrir para que `paginas()` nunca devuelva cero y
    /// quien llame no tenga que comprobarlo.
    #[error("{0} no tiene ninguna pagina")]
    SinPaginas(PathBuf),
    #[error("no hay pagina {indice}: el documento tiene {paginas}")]
    PaginaFueraDeRango { indice: u32, paginas: u32 },
    #[error("el ancho pedido ({ancho}) tiene que estar entre 1 y {ANCHO_MAXIMO}")]
    AnchoInvalido { ancho: u32 },
    /// El render dijo que si pero devolvio una imagen sin pixeles. No deberia
    /// pasar; si pasa, mejor un error que una imagen negra que el usuario
    /// interpretaria como una pagina en blanco.
    #[error("la pagina {indice} se dibujo vacia")]
    RenderVacio { indice: u32 },
    #[error("el sistema fallo al trabajar con el PDF: {0}")]
    Sistema(#[source] windows::core::Error),
}

impl From<windows::core::Error> for ErrorPdf {
    fn from(e: windows::core::Error) -> Self {
        ErrorPdf::Sistema(e)
    }
}

/// Un PDF abierto, listo para preguntarle paginas.
///
/// Mantiene vivo el documento de WinRT: abrirlo es lo caro (leer y parsear el
/// fichero entero), dibujar una pagina es barato. Un visor que salta de la
/// pagina 3 a la 4 no vuelve a pagar la apertura.
pub struct Documento {
    documento: PdfDocument,
    paginas: u32,
    /// Se guarda para poder decir en los errores DE QUE fichero se habla.
    ruta: PathBuf,
    /// COM del hilo. Va el ultimo a proposito: los campos se destruyen en
    /// orden de declaracion, asi que `documento` se suelta ANTES de que este
    /// guardia llame a `CoUninitialize`. Al reves, el `Release` del
    /// `PdfDocument` correria sobre un apartamento ya cerrado y el proceso
    /// moriria con ACCESS_VIOLATION.
    _com: ComDelHilo,
    /// Marca que hace `Documento` no-`Send`.
    ///
    /// El documento de WinRT si es agil y podria cruzar de hilo, pero
    /// `ComDelHilo` no: `CoUninitialize` tiene que correr en el mismo hilo que
    /// hizo `CoInitializeEx`. Dejar que el `Documento` se mueva seria dejar que
    /// el guardia se deshaga en el hilo equivocado.
    _mismo_hilo: std::marker::PhantomData<*const ()>,
}

impl Documento {
    /// Abre el PDF de `ruta`.
    ///
    /// Distingue los cuatro finales malos: no existe, no es un PDF, esta
    /// cifrado, o esta danado. Un mensaje generico no le dice al usuario si el
    /// arreglo es buscar otro fichero o pedir la contrasena.
    pub fn abrir(ruta: &Path) -> Result<Documento, ErrorPdf> {
        if !ruta.is_file() {
            return Err(ErrorPdf::NoExiste(ruta.to_path_buf()));
        }

        // Se mira la cabecera ANTES de molestar a WinRT. Asi un .docx mal
        // arrastrado se rechaza en microsegundos y con el error exacto, en vez
        // de gastar la carga entera para recibir un HRESULT que no distingue
        // "no es un PDF" de "es un PDF roto".
        if !parece_pdf(&leer_cabecera(ruta)) {
            return Err(ErrorPdf::NoEsPdf(ruta.to_path_buf()));
        }

        let com = ComDelHilo::nuevo();

        // `GetFileFromPathAsync` exige una ruta absoluta y NO traga el prefijo
        // extendido `\\?\` que devuelve `canonicalize`: falla por sintaxis.
        // Por eso se normaliza a mano.
        let absoluta = ruta_para_winrt(ruta)?;
        let fichero: StorageFile = esperar_operacion(&StorageFile::GetFileFromPathAsync(
            &HSTRING::from(&absoluta),
        )?)?;

        let documento = match esperar_operacion(&PdfDocument::LoadFromFileAsync(&fichero)?) {
            Ok(d) => d,
            Err(e) => return Err(clasificar_fallo_de_carga(ruta, e)),
        };

        let paginas = documento.PageCount()?;
        if paginas == 0 {
            return Err(ErrorPdf::SinPaginas(ruta.to_path_buf()));
        }

        Ok(Documento {
            documento,
            paginas,
            ruta: ruta.to_path_buf(),
            _com: com,
            _mismo_hilo: std::marker::PhantomData,
        })
    }

    /// Cuantas paginas tiene. Nunca cero: un PDF sin paginas no se abre.
    pub fn paginas(&self) -> u32 {
        self.paginas
    }

    /// La ruta con la que se abrio.
    pub fn ruta(&self) -> &Path {
        &self.ruta
    }

    /// **Lo que mide el papel de cada pagina**, ancho y alto, sin dibujar
    /// ninguna.
    ///
    /// Lo pide el lector continuo: para colocar las hojas una debajo de otra
    /// tiene que saber cuanto mide cada una ANTES de pintarla, o el
    /// desplazamiento pega saltos segun van llegando. Pedir la medida es
    /// barato (`GetPage` + `Size`, sin rasterizar nada); una pagina cuya
    /// medida no se pueda leer sale como `(0, 0)` y quien coloca le pone una
    /// proporcion corriente.
    ///
    /// **Las paginas giradas** (`/Rotate 90`): `Size` ya da la medida GIRADA,
    /// la misma proporcion que saca [`Documento::renderizar`] (medido; no hay
    /// que mirar `Rotation` y girarla otra vez, que era darle dos vueltas).
    /// Por si un PDF raro no casara, el lector corrige la proporcion de una
    /// hoja en cuanto llega su dibujo.
    pub fn medidas(&self) -> Vec<(f32, f32)> {
        (0..self.paginas)
            .map(|i| {
                let Ok(pagina) = self.documento.GetPage(i) else {
                    return (0.0, 0.0);
                };
                let medida = pagina.Size().map(|s| (s.Width, s.Height));
                let _ = pagina.Close();
                medida.unwrap_or((0.0, 0.0))
            })
            .collect()
    }

    /// Dibuja la pagina `indice` (desde 0) con este ancho en pixeles.
    /// El alto sale de la proporcion de la pagina.
    ///
    /// El ancho que se devuelve es EXACTAMENTE el pedido. Cuesta trabajo
    /// conseguirlo: ver [`ESCALA_MEDIDA`].
    pub fn renderizar(&self, indice: u32, ancho: u32) -> Result<ImagenRgba, ErrorPdf> {
        // Los dos casos se comprueban aqui y no mas abajo porque WinRT
        // responde a ambos con una excepcion, y una excepcion que cruza la
        // frontera de FFI es justo lo que no queremos.
        if ancho == 0 || ancho > ANCHO_MAXIMO {
            return Err(ErrorPdf::AnchoInvalido { ancho });
        }
        if indice >= self.paginas {
            return Err(ErrorPdf::PaginaFueraDeRango {
                indice,
                paginas: self.paginas,
            });
        }

        let pagina = self.documento.GetPage(indice)?;

        let unidades = unidades_para(ancho);
        // El guion bajo de `_flujo` no significa que sobre: significa que no se
        // lee, pero tiene que seguir en pie hasta el final. Si se borra, el
        // descodificador se queda leyendo un flujo cerrado.
        let (mut _flujo, mut descodificador) = dibujar(&pagina, unidades)?;
        let mut natural_ancho = descodificador.PixelWidth()?;
        let mut natural_alto = descodificador.PixelHeight()?;

        if natural_ancho != ancho && natural_ancho > 0 {
            // Salio otra cosa: se aprende la escala de verdad de este Windows y
            // se dibuja UNA segunda vez ya con el numero de unidades bueno.
            // Solo pasa en el primer render del proceso; a partir de ahi la
            // escala esta medida y esta rama no vuelve a entrar.
            ESCALA_MEDIDA.store(
                (natural_ancho as f64 / unidades as f64).to_bits(),
                Ordering::Relaxed,
            );
            let corregidas = unidades_para(ancho);
            if corregidas != unidades {
                let (f2, d2) = dibujar(&pagina, corregidas)?;
                _flujo = f2;
                descodificador = d2;
                natural_ancho = descodificador.PixelWidth()?;
                natural_alto = descodificador.PixelHeight()?;
            }
        }

        if natural_ancho == 0 || natural_alto == 0 {
            let _ = pagina.Close();
            return Err(ErrorPdf::RenderVacio { indice });
        }

        // Ajuste fino. `DestinationWidth` va en unidades enteras, asi que hay
        // anchos en pixeles a los que no se llega por muchas vueltas que se de
        // (con escala 1,4 el 3 es inalcanzable: se salta del 2 al 4). Lo que
        // falta lo hace WIC al descodificar, que es un reescalado nativo de
        // una pasada y no un bucle nuestro. Se pide siempre uno o dos pixeles
        // de bajada, nunca una ampliacion, porque la pagina se dibujo a
        // proposito igual o mas grande que lo pedido.
        let transformacion = BitmapTransform::new()?;
        if natural_ancho != ancho {
            let alto = proporcional(ancho, natural_ancho, natural_alto);
            transformacion.SetScaledWidth(ancho)?;
            transformacion.SetScaledHeight(alto)?;
            // Fant es el remuestreo bueno de WIC. En una bajada de pocos
            // pixeles la diferencia con el vecino mas proximo es justo el
            // borde dentado del texto, que es lo que mas se nota en un PDF.
            transformacion.SetInterpolationMode(BitmapInterpolationMode::Fant)?;
            natural_ancho = ancho;
            natural_alto = alto;
        }

        // Aqui esta la trampa de las caras verdes y los cielos naranjas: WIC
        // entrega BGRA por defecto y nuestras `ImagenRgba` son RGBA. En vez de
        // recibir BGRA y darle la vuelta a cada pixel en un bucle (lo que hace
        // `pixpin-record/src/mp4.rs` cuando no le queda otra), se le PIDE Rgba8
        // al descodificador: la conversion la hace WIC de una pasada y no queda
        // ningun bucle que alguien pueda borrar por error. `Straight` es alfa
        // sin premultiplicar, que es lo que documenta `ImagenRgba`.
        let datos = esperar_operacion(&descodificador.GetPixelDataTransformedAsync(
            BitmapPixelFormat::Rgba8,
            BitmapAlphaMode::Straight,
            &transformacion,
            ExifOrientationMode::IgnoreExifOrientation,
            ColorManagementMode::DoNotColorManage,
        )?)?;
        let pixeles = datos.DetachPixelData()?.to_vec();

        // Soltar la pagina en cuanto se ha copiado su imagen: WinRT se queda
        // con el mapa de bits de la pagina hasta que le venga bien soltarlo, y
        // un visor que pasa paginas seguidas acumula decenas de megabytes.
        let _ = pagina.Close();

        let esperados = (natural_ancho as usize) * (natural_alto as usize) * 4;
        if pixeles.len() < esperados {
            return Err(ErrorPdf::RenderVacio { indice });
        }

        Ok(ImagenRgba {
            ancho: natural_ancho,
            alto: natural_alto,
            pixeles,
        })
    }

    /// **Un trozo de la pagina a la resolucion que se pida**: lo que usa el
    /// lienzo para verse nitido al acercarse (la «lamina» de `PdfDoc` del
    /// movil).
    ///
    /// `trozo` va en fracciones de la pagina, de 0 a 1 (x, y, ancho, alto):
    /// asi quien pide no tiene que saber en que unidades mide el papel de
    /// este PDF. Sale una imagen de mas o menos `ancho` x `alto` pixeles; no
    /// se afina al pixel como en [`Documento::renderizar`] porque quien la
    /// pinta la estira a su caja del mundo, y un pixel de mas no se ve.
    ///
    /// Por que no pintar la pagina entera mas grande: a dieciseis pixeles por
    /// unidad una hoja A4 son veinte mil de ancho, que no caben ni en la
    /// memoria ni en la GPU. Un trozo del tamano de la pantalla, si.
    pub fn renderizar_trozo(
        &self,
        indice: u32,
        trozo: (f32, f32, f32, f32),
        ancho: u32,
        alto: u32,
    ) -> Result<ImagenRgba, ErrorPdf> {
        if ancho == 0 || ancho > ANCHO_MAXIMO || alto == 0 || alto > ANCHO_MAXIMO {
            return Err(ErrorPdf::AnchoInvalido { ancho });
        }
        if indice >= self.paginas {
            return Err(ErrorPdf::PaginaFueraDeRango {
                indice,
                paginas: self.paginas,
            });
        }
        let (fx, fy, fw, fh) = trozo;
        if !(fw > 0.0 && fh > 0.0) {
            return Err(ErrorPdf::RenderVacio { indice });
        }
        let pagina = self.documento.GetPage(indice)?;
        let medida = pagina.Size()?;
        let opciones = PdfPageRenderOptions::new()?;
        // `SourceRect` va en las unidades de `Size` (la pagina ya girada,
        // como sale en `medidas`), no en fracciones.
        opciones.SetSourceRect(windows::Foundation::Rect {
            X: fx * medida.Width,
            Y: fy * medida.Height,
            Width: fw * medida.Width,
            Height: fh * medida.Height,
        })?;
        opciones.SetBitmapEncoderId(BitmapEncoder::BmpEncoderId()?)?;
        // Los dos lados, porque la proporcion la pone el trozo y no la
        // pagina; y en unidades de WinRT, no en pixeles (`unidades_para`).
        let dibujar_trozo = |unidades_w: u32, unidades_h: u32| -> Result<_, ErrorPdf> {
            opciones.SetDestinationWidth(unidades_w)?;
            opciones.SetDestinationHeight(unidades_h)?;
            let flujo = InMemoryRandomAccessStream::new()?;
            esperar_accion(&pagina.RenderWithOptionsToStreamAsync(&flujo, &opciones)?)?;
            flujo.Seek(0)?;
            let d = esperar_operacion(&BitmapDecoder::CreateAsync(&flujo)?)?;
            Ok((flujo, d))
        };
        let unidades = unidades_para(ancho);
        let (mut _flujo, mut descodificador) = dibujar_trozo(unidades, unidades_para(alto))?;
        let natural = descodificador.PixelWidth()?;
        // La escala de la pantalla sin medir todavia en este proceso: se
        // aprende aqui, como en `renderizar`, y se pinta una segunda vez.
        // Sin esto el primer trozo salia un 40 % mas grande en un portatil
        // al 140 %, y gastaba memoria que nadie pidio.
        if natural > 0 && natural.abs_diff(ancho) * 50 > ancho {
            ESCALA_MEDIDA.store(
                (natural as f64 / unidades as f64).to_bits(),
                Ordering::Relaxed,
            );
            let (f2, d2) = dibujar_trozo(unidades_para(ancho), unidades_para(alto))?;
            _flujo = f2;
            descodificador = d2;
        }
        let (w, h) = (descodificador.PixelWidth()?, descodificador.PixelHeight()?);
        if w == 0 || h == 0 {
            let _ = pagina.Close();
            return Err(ErrorPdf::RenderVacio { indice });
        }
        let datos = esperar_operacion(&descodificador.GetPixelDataTransformedAsync(
            BitmapPixelFormat::Rgba8,
            BitmapAlphaMode::Straight,
            &BitmapTransform::new()?,
            ExifOrientationMode::IgnoreExifOrientation,
            ColorManagementMode::DoNotColorManage,
        )?)?;
        let pixeles = datos.DetachPixelData()?.to_vec();
        let _ = pagina.Close();
        if pixeles.len() < (w as usize) * (h as usize) * 4 {
            return Err(ErrorPdf::RenderVacio { indice });
        }
        Ok(ImagenRgba {
            ancho: w,
            alto: h,
            pixeles,
        })
    }
}

/// Cuantos pixeles de verdad salen por cada unidad de `DestinationWidth`.
///
/// Aqui esta la sorpresa gorda de `Windows.Data.Pdf`: **`DestinationWidth` no
/// va en pixeles**. Va en unidades independientes del dispositivo, y WinRT las
/// multiplica por la escala de la pantalla. En un portatil al 140% —el caso
/// corriente hoy— pedir 400 devuelve una imagen de 560 de ancho. Quien se fie
/// del nombre del parametro se encuentra con paginas un 40% mas grandes de lo
/// que reservo, y el fallo no salta hasta que la imagen no cabe donde iba.
///
/// No sirve preguntarle la escala a Win32 (`GetDpiForSystem` y compania):
/// responden segun la conciencia de DPI del proceso, y este crate es una
/// libreria que no sabe ni quiere saber como se declaro el ejecutable que la
/// usa. Asi que la escala **se mide**: el primer render que salga con un ancho
/// distinto del pedido dice cual es, se guarda aqui y ya no se vuelve a pagar.
/// Se guarda para todo el proceso y no por documento porque es una propiedad
/// de la pantalla, no del PDF, y un visor que abre diez ficheros no tiene por
/// que medirla diez veces.
///
/// Se arranca en 1,0 (suponer que no hay escalado) y se corrige sola. Si el
/// usuario cambia el escalado con el programa abierto, el primer render que
/// falle vuelve a medir.
static ESCALA_MEDIDA: AtomicU64 = AtomicU64::new(1.0f64.to_bits());

/// Cuantas unidades hay que pedirle a WinRT para que salgan `ancho` pixeles.
///
/// Se redondea hacia ARRIBA a proposito: mas vale dibujar uno o dos pixeles de
/// mas y bajarlos al descodificar que quedarse corto y tener que ampliar, que
/// es lo que se ve borroso.
fn unidades_para(ancho: u32) -> u32 {
    unidades_con_escala(ancho, f64::from_bits(ESCALA_MEDIDA.load(Ordering::Relaxed)))
}

/// La cuenta de [`unidades_para`], separada de la variable global para poder
/// probarla sin tocarla y sin depender de la pantalla de quien ejecute.
fn unidades_con_escala(ancho: u32, escala: f64) -> u32 {
    // Una escala absurda (cero, negativa, infinita, NaN) solo puede venir de
    // una medida imposible; se ignora y se pide el ancho tal cual, que el
    // render se corrige solo en la siguiente vuelta.
    if !(escala.is_finite() && escala > 0.0) {
        return ancho;
    }
    let unidades = (ancho as f64 / escala).ceil();
    (unidades as u32).clamp(1, ANCHO_MAXIMO)
}

/// El alto que le toca a `ancho` para conservar la proporcion `de_ancho` x
/// `de_alto`. Nunca cero: una imagen de alto cero no es una imagen.
fn proporcional(ancho: u32, de_ancho: u32, de_alto: u32) -> u32 {
    let alto = (ancho as u64 * de_alto as u64).div_ceil(de_ancho.max(1) as u64);
    (alto as u32).max(1)
}

/// Dibuja la pagina a un flujo en memoria y devuelve el descodificador listo.
///
/// El flujo sale junto al descodificador porque **tiene que seguir vivo**
/// mientras se leen los pixeles: el descodificador lee de el a demanda.
fn dibujar(
    pagina: &windows::Data::Pdf::PdfPage,
    unidades: u32,
) -> Result<(InMemoryRandomAccessStream, BitmapDecoder), ErrorPdf> {
    let opciones = PdfPageRenderOptions::new()?;
    // Solo se fija el ancho: dejando el alto en cero, WinRT lo calcula con la
    // proporcion real de la pagina (incluida su rotacion). Fijar los dos
    // deformaria las paginas apaisadas.
    opciones.SetDestinationWidth(unidades)?;
    // El render sale como imagen codificada, no como pixeles sueltos. Se pide
    // BMP y no el PNG de por defecto porque acto seguido se descodifica:
    // comprimir con deflate para descomprimir tres lineas mas abajo es trabajo
    // de CPU tirado, y el equipo suelo tiene cuatro nucleos.
    opciones.SetBitmapEncoderId(BitmapEncoder::BmpEncoderId()?)?;

    let flujo = InMemoryRandomAccessStream::new()?;
    esperar_accion(&pagina.RenderWithOptionsToStreamAsync(&flujo, &opciones)?)?;
    // El render deja el cursor al final y el descodificador lee desde donde
    // este. Sin este rebobinado no encuentra ni la cabecera del BMP.
    flujo.Seek(0)?;

    let descodificador = esperar_operacion(&BitmapDecoder::CreateAsync(&flujo)?)?;
    Ok((flujo, descodificador))
}

impl std::fmt::Debug for Documento {
    /// Se muestran la ruta y las paginas, nada mas. El puntero del objeto de
    /// WinRT no le dice nada a nadie leyendo un mensaje de error, y el `Debug`
    /// derivado lo unico que aportaria seria eso.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Documento")
            .field("ruta", &self.ruta)
            .field("paginas", &self.paginas)
            .finish()
    }
}

/// COM inicializado para este hilo, y soltado al salir **solo si fuimos
/// nosotros quienes lo inicializamos**.
///
/// Es la misma guardia que `pixpin-record/src/mp4.rs`, y por la misma razon:
/// `abrir` es una funcion de libreria y puede llamarla tanto el hilo de la
/// interfaz (ya en apartamento) como uno recien creado por una prueba.
/// `CoInitializeEx` devuelve `RPC_E_CHANGED_MODE` cuando el hilo ya esta en
/// otro apartamento; en ese caso no incrementa la cuenta, y llamar a
/// `CoUninitialize` cerraria el COM de otro.
struct ComDelHilo {
    nuestro: bool,
}

impl ComDelHilo {
    fn nuevo() -> ComDelHilo {
        // SAFETY: `CoInitializeEx` no tiene precondiciones; es la forma normal
        // de entrar en COM. Se guarda si la cuenta subio para deshacerlo con
        // exactitud en `Drop` y no de mas.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        ComDelHilo {
            nuestro: hr != RPC_E_CHANGED_MODE,
        }
    }
}

impl Drop for ComDelHilo {
    fn drop(&mut self) {
        if self.nuestro {
            // SAFETY: empareja exactamente el `CoInitializeEx` de arriba. Este
            // guardia es el ultimo campo de `Documento`, asi que cuando corre
            // ya no queda viva ninguna interfaz creada aqui.
            unsafe { CoUninitialize() };
        }
    }
}

/// Espera a que termine una operacion asincrona de WinRT y recoge su valor.
///
/// Se sondea `Status()` en vez de registrar un `Completed`, igual que en
/// `pixpin-ocr`. Registrar un manejador obligaria a que el hilo bombee mensajes
/// para recibirlo, y desde un apartamento de interfaz eso significa o reentrar
/// el bucle de mensajes o quedarse colgado. `Status()` lo escribe la propia
/// operacion cuando acaba, asi que leerlo funciona en cualquier apartamento. Se
/// duerme un milisegundo entre vueltas para no quemar un nucleo: el equipo
/// suelo solo tiene cuatro.
fn esperar_operacion<T>(operacion: &IAsyncOperation<T>) -> Result<T, windows::core::Error>
where
    T: windows::core::RuntimeType + 'static,
{
    while operacion.Status()? == AsyncStatus::Started {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    operacion.GetResults()
}

/// Lo mismo para las operaciones que no devuelven nada (el render).
fn esperar_accion(accion: &IAsyncAction) -> Result<(), windows::core::Error> {
    while accion.Status()? == AsyncStatus::Started {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    accion.GetResults()
}

/// Los primeros bytes del fichero, o vacio si no se puede leer.
///
/// Devolver vacio en vez de error es deliberado: si el fichero existe pero no
/// se deja leer, `parece_pdf` dira que no y el usuario recibe "no es un PDF",
/// que es lo que va a ver de todas formas cuando intente abrirlo.
fn leer_cabecera(ruta: &Path) -> Vec<u8> {
    use std::io::Read;
    let mut buffer = vec![0u8; CABECERA_MIRADA];
    match std::fs::File::open(ruta) {
        Ok(mut f) => {
            let leidos = f.read(&mut buffer).unwrap_or(0);
            buffer.truncate(leidos);
            buffer
        }
        Err(_) => Vec::new(),
    }
}

/// Si estos bytes iniciales llevan la marca de un PDF.
fn parece_pdf(cabecera: &[u8]) -> bool {
    cabecera.windows(5).any(|v| v == b"%PDF-")
}

/// Convierte el fallo de `LoadFromFileAsync` en un error que dice algo.
///
/// Para cuando se llama a esto ya se sabe que la cabecera dice `%PDF-`, asi que
/// solo quedan dos finales: pide contrasena, o esta roto.
fn clasificar_fallo_de_carga(ruta: &Path, fuente: windows::core::Error) -> ErrorPdf {
    if fuente.code() == HR_CONTRASENA {
        ErrorPdf::Cifrado(ruta.to_path_buf())
    } else {
        ErrorPdf::Corrupto {
            ruta: ruta.to_path_buf(),
            fuente,
        }
    }
}

/// La ruta en la forma que acepta `StorageFile::GetFileFromPathAsync`:
/// absoluta y sin el prefijo extendido `\\?\`.
fn ruta_para_winrt(ruta: &Path) -> Result<String, ErrorPdf> {
    let absoluta = std::path::absolute(ruta).map_err(|_| ErrorPdf::NoExiste(ruta.to_path_buf()))?;
    Ok(sin_prefijo_extendido(&absoluta.to_string_lossy()))
}

/// Quita el `\\?\` (o `\\?\UNC\`) del principio, si lo hay.
///
/// Es la forma que devuelve `std::fs::canonicalize` y la que puede traer una
/// ruta que venga de otra API de Win32. WinRT la rechaza por sintaxis.
fn sin_prefijo_extendido(ruta: &str) -> String {
    if let Some(resto) = ruta.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{resto}")
    } else if let Some(resto) = ruta.strip_prefix(r"\\?\") {
        resto.to_string()
    } else {
        ruta.to_string()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::fs;

    /// Directorio de trabajo propio de cada prueba, vaciado al empezar.
    ///
    /// Se vacia al ENTRAR y no solo al salir porque una prueba que falla se va
    /// por el panico sin limpiar: si no se borrase aqui, la siguiente ejecucion
    /// se encontraria el PDF viejo y podria pasar por razones equivocadas.
    fn temporal(etiqueta: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pixpin-pdf-{etiqueta}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Un objeto de flujo con su `/Length` bien puesto.
    fn flujo(contenido: &str) -> String {
        format!(
            "<< /Length {} >>\nstream\n{contenido}\nendstream",
            contenido.len()
        )
    }

    /// Un PDF valido de tres paginas, escrito byte a byte.
    ///
    /// No hay ningun PDF en el repositorio y no se va a anadir un binario, asi
    /// que la prueba se fabrica el suyo: un PDF minimo es texto plano con una
    /// tabla `xref` de desplazamientos, y esos desplazamientos se calculan
    /// aqui en vez de escribirlos a mano, que es donde siempre se rompen.
    ///
    /// Las paginas 0 y 1 miden LO MISMO (200x100) y solo cambian en el dibujo:
    /// cuadrado negro a la izquierda en la 0, a la derecha en la 1. Si
    /// `GetPage` ignorase el indice, las dos saldrian identicas y comparar solo
    /// medidas no lo delataria. La pagina 2 mide 100x200, la proporcion del
    /// reves, para que un fallo al deducir el alto tampoco pueda esconderse.
    fn pdf_de_tres_paginas() -> Vec<u8> {
        let objetos: Vec<String> = vec![
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R 5 0 R 7 0 R] /Count 3 >>".to_string(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] \
             /Contents 4 0 R /Resources << >> >>"
                .to_string(),
            flujo("0 0 0 rg 10 10 80 80 re f"),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] \
             /Contents 6 0 R /Resources << >> >>"
                .to_string(),
            flujo("0 0 0 rg 110 10 80 80 re f"),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 200] \
             /Contents 8 0 R /Resources << >> >>"
                .to_string(),
            flujo("0 0 0 rg 10 10 80 180 re f"),
        ];

        let mut salida: Vec<u8> = b"%PDF-1.4\n".to_vec();
        let mut desplazamientos = Vec::new();
        for (i, cuerpo) in objetos.iter().enumerate() {
            desplazamientos.push(salida.len());
            salida.extend_from_slice(format!("{} 0 obj\n{cuerpo}\nendobj\n", i + 1).as_bytes());
        }

        let inicio_xref = salida.len();
        let total = objetos.len() + 1;
        salida.extend_from_slice(format!("xref\n0 {total}\n").as_bytes());
        // La entrada cero es la cabeza de la lista de huecos y siempre es esta.
        salida.extend_from_slice(b"0000000000 65535 f \n");
        for d in &desplazamientos {
            // Diez digitos, cinco de generacion y el espacio final: cada linea
            // de la xref mide exactamente veinte bytes o el lector no la sigue.
            salida.extend_from_slice(format!("{d:010} 00000 n \n").as_bytes());
        }
        salida.extend_from_slice(
            format!("trailer\n<< /Size {total} /Root 1 0 R >>\nstartxref\n{inicio_xref}\n%%EOF\n")
                .as_bytes(),
        );
        salida
    }

    /// Escribe el PDF de tres paginas y devuelve su ruta.
    fn pdf_en_disco(etiqueta: &str) -> (PathBuf, PathBuf) {
        let dir = temporal(etiqueta);
        let ruta = dir.join("tres-paginas.pdf");
        fs::write(&ruta, pdf_de_tres_paginas()).unwrap();
        (dir, ruta)
    }

    /// Cuantos colores distintos hay en la imagen, hasta un tope.
    ///
    /// Sirve para cazar el render que falla en silencio: una pagina toda
    /// blanca o toda negra tiene un solo color y para el programa es
    /// indistinguible de una pagina bien dibujada si solo se miran las medidas.
    fn colores_distintos(imagen: &ImagenRgba) -> usize {
        let mut vistos = std::collections::HashSet::new();
        for p in imagen.pixeles.chunks_exact(4) {
            vistos.insert([p[0], p[1], p[2], p[3]]);
            if vistos.len() > 8 {
                break;
            }
        }
        vistos.len()
    }

    /// Cuantos pixeles oscuros hay en la mitad izquierda y en la derecha.
    ///
    /// Es como se comprueba que el dibujo cayo donde dice el PDF y no en
    /// cualquier sitio: las dos paginas de prueba llevan el mismo cuadrado en
    /// lados opuestos.
    fn oscuros_por_mitad(imagen: &ImagenRgba) -> (usize, usize) {
        let (mut izquierda, mut derecha) = (0usize, 0usize);
        for y in 0..imagen.alto {
            for x in 0..imagen.ancho {
                let i = ((y as usize * imagen.ancho as usize) + x as usize) * 4;
                let luz = imagen.pixeles[i] as u32
                    + imagen.pixeles[i + 1] as u32
                    + imagen.pixeles[i + 2] as u32;
                // Bien oscuro: el cuadrado se dibuja negro puro sobre blanco,
                // asi que el umbral no tiene que afinar nada.
                if luz < 150 {
                    if x < imagen.ancho / 2 {
                        izquierda += 1;
                    } else {
                        derecha += 1;
                    }
                }
            }
        }
        (izquierda, derecha)
    }

    #[test]
    fn un_pdf_de_tres_paginas_dice_que_tiene_tres() {
        let (dir, ruta) = pdf_en_disco("cuenta");
        let documento = Documento::abrir(&ruta).unwrap();
        assert_eq!(documento.paginas(), 3);
        assert_eq!(documento.ruta(), ruta);
        drop(documento);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn dos_paginas_del_mismo_tamano_se_dibujan_distintas() {
        // El caso que de verdad prueba que `GetPage` mira el indice: las dos
        // paginas miden igual, asi que comparar medidas no vale de nada y solo
        // los pixeles delatan si se dibujo dos veces la misma.
        let (dir, ruta) = pdf_en_disco("indice");
        let documento = Documento::abrir(&ruta).unwrap();

        let primera = documento.renderizar(0, 200).unwrap();
        let segunda = documento.renderizar(1, 200).unwrap();

        assert_eq!(
            (primera.ancho, primera.alto),
            (segunda.ancho, segunda.alto),
            "las dos paginas miden 200x100 puntos: deben salir del mismo tamano"
        );
        assert_ne!(
            primera.pixeles, segunda.pixeles,
            "la pagina 0 y la 1 tienen el cuadrado en lados opuestos; \
             si salen identicas, GetPage esta ignorando el indice"
        );

        // Y no basta con que sean distintas: el cuadrado tiene que estar en el
        // lado que dice el PDF. Comparar solo los buffers dejaria pasar un
        // render que devuelve ruido diferente cada vez.
        let (izq0, der0) = oscuros_por_mitad(&primera);
        let (izq1, der1) = oscuros_por_mitad(&segunda);
        assert!(
            izq0 > 0 && der0 == 0,
            "la pagina 0 dibuja el cuadrado a la izquierda; salio {izq0} / {der0}"
        );
        assert!(
            der1 > 0 && izq1 == 0,
            "la pagina 1 lo dibuja a la derecha; salio {izq1} / {der1}"
        );

        drop(documento);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn el_ancho_pedido_es_el_que_sale_y_el_alto_sigue_la_proporcion() {
        let (dir, ruta) = pdf_en_disco("proporcion");
        let documento = Documento::abrir(&ruta).unwrap();

        // 200x100 puntos pedida a 400 de ancho: 200 de alto.
        let apaisada = documento.renderizar(0, 400).unwrap();
        assert_eq!(
            apaisada.ancho, 400,
            "el ancho pedido tiene que salir clavado aunque la pantalla \
             tenga escalado; si sale 560 es que `DestinationWidth` se tomo \
             por pixeles y va en unidades de dispositivo"
        );
        assert!(
            apaisada.alto.abs_diff(200) <= 1,
            "una pagina 2:1 a 400 de ancho debe dar unos 200 de alto, dio {}",
            apaisada.alto
        );

        // 100x200 puntos, la proporcion del reves: 800 de alto.
        let vertical = documento.renderizar(2, 400).unwrap();
        assert_eq!(vertical.ancho, 400);
        assert!(
            vertical.alto.abs_diff(800) <= 1,
            "una pagina 1:2 a 400 de ancho debe dar unos 800 de alto, dio {}",
            vertical.alto
        );

        // Y el buffer tiene que cuadrar con las medidas que declara, o quien
        // lo suba a una textura leera fuera de rango.
        assert_eq!(apaisada.pixeles.len(), apaisada.bytes_esperados());
        assert_eq!(vertical.pixeles.len(), vertical.bytes_esperados());

        drop(documento);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn cualquier_ancho_sale_clavado_aunque_no_sea_redondo() {
        // Con escalado de pantalla hay anchos a los que `DestinationWidth` no
        // llega ni por casualidad, porque va en unidades enteras que luego se
        // multiplican: al 140% se salta del 2 al 4 y el 3 no existe. Estos
        // numeros feos comprueban que el ajuste fino de WIC los alcanza igual.
        let (dir, ruta) = pdf_en_disco("anchos-feos");
        let documento = Documento::abrir(&ruta).unwrap();

        for ancho in [1u32, 3, 7, 137, 333, 641] {
            let imagen = documento.renderizar(0, ancho).unwrap();
            assert_eq!(imagen.ancho, ancho, "se pidieron {ancho} de ancho");
            assert_eq!(imagen.pixeles.len(), imagen.bytes_esperados());
            // La proporcion 2:1 de la pagina se mantiene en todos ellos.
            let esperado = ancho.div_ceil(2).max(1);
            assert!(
                imagen.alto.abs_diff(esperado) <= 1,
                "a {ancho} de ancho tocan unos {esperado} de alto, dio {}",
                imagen.alto
            );
        }

        drop(documento);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn las_unidades_que_se_piden_compensan_la_escala_de_la_pantalla() {
        assert_eq!(
            unidades_con_escala(400, 1.0),
            400,
            "sin escalado se pide tal cual"
        );
        // 286 x 1,4 = 400,4: redondear hacia arriba deja la imagen igual o
        // mayor que lo pedido. Con 285 saldrian 399 y habria que AMPLIAR, que
        // es lo que se ve borroso.
        assert_eq!(unidades_con_escala(400, 1.4), 286);
        assert_eq!(unidades_con_escala(400, 1.25), 320);
        assert_eq!(
            unidades_con_escala(1, 1.4),
            1,
            "nunca se piden cero unidades"
        );

        // Casos negativos: una escala imposible no debe dejar el crate
        // inservible, solo hacer que la primera vuelta se quede corta.
        assert_eq!(unidades_con_escala(400, 0.0), 400);
        assert_eq!(unidades_con_escala(400, -2.0), 400);
        assert_eq!(unidades_con_escala(400, f64::NAN), 400);
        assert_eq!(unidades_con_escala(400, f64::INFINITY), 400);
        // Y una escala minuscula no pide un ancho que reviente la memoria.
        assert_eq!(unidades_con_escala(400, 1e-9), ANCHO_MAXIMO);
    }

    #[test]
    fn el_alto_proporcional_nunca_es_cero() {
        assert_eq!(proporcional(400, 200, 100), 200);
        assert_eq!(proporcional(100, 200, 400), 200);
        // Una pagina muy apaisada pedida muy pequena daria cero redondeando
        // hacia abajo, y una imagen de alto cero no se puede ni guardar.
        assert_eq!(proporcional(1, 1000, 10), 1);
        // Caso negativo: un ancho de origen de cero no puede dividir entre
        // cero ni entrar en panico.
        assert!(proporcional(10, 0, 10) >= 1);
    }

    #[test]
    fn la_pagina_dibujada_tiene_mas_de_un_color() {
        // Un render que falla en silencio devuelve la pagina entera blanca o
        // entera negra, con las medidas correctas. Solo mirando los pixeles se
        // distingue de un dibujo de verdad.
        let (dir, ruta) = pdf_en_disco("colores");
        let documento = Documento::abrir(&ruta).unwrap();
        let imagen = documento.renderizar(0, 200).unwrap();

        assert!(
            colores_distintos(&imagen) >= 2,
            "la pagina lleva un cuadrado negro sobre blanco: si sale de un \
             solo color, el render fallo sin decirlo"
        );

        // Y el alfa tiene que estar al maximo: si saliera a cero, la pagina se
        // veria transparente sobre el fondo del visor.
        assert!(
            imagen.es_opaca(),
            "una pagina de PDF se dibuja sobre fondo opaco"
        );

        drop(documento);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn un_fichero_que_no_existe_no_se_abre() {
        // Caso negativo. No toca WinRT: se responde con la ruta que fallo, que
        // es lo unico que el usuario puede usar para arreglarlo.
        let e = Documento::abrir(Path::new("Z:/no/existe/ninguno.pdf")).unwrap_err();
        assert!(matches!(e, ErrorPdf::NoExiste(_)), "salio {e:?}");
        assert!(
            e.to_string().contains("ninguno.pdf"),
            "el error debe decir cual: {e}"
        );
    }

    #[test]
    fn un_fichero_que_no_es_pdf_se_distingue_de_un_pdf_danado() {
        // Caso negativo doble, y el interesante: los dos son "no se puede
        // abrir", pero el arreglo del usuario es distinto. Con un solo error
        // generico no sabria si buscar otro fichero o recuperar este.
        let dir = temporal("no-es-pdf");

        let cualquiera = dir.join("carta.docx");
        fs::write(&cualquiera, b"PK\x03\x04 esto es un zip, no un PDF").unwrap();
        assert!(
            matches!(Documento::abrir(&cualquiera), Err(ErrorPdf::NoEsPdf(_))),
            "un fichero sin la marca %PDF- no es un PDF danado, es otra cosa"
        );

        let roto = dir.join("cortado.pdf");
        fs::write(&roto, b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog").unwrap();
        let e = Documento::abrir(&roto).unwrap_err();
        assert!(
            matches!(e, ErrorPdf::Corrupto { .. }),
            "lleva la marca %PDF- pero esta cortado: es Corrupto, salio {e:?}"
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn una_pagina_fuera_de_rango_da_error_en_vez_de_panico() {
        // Caso negativo. WinRT lanza una excepcion si se le pide una pagina
        // que no hay; se comprueba antes para que no cruce la frontera de FFI.
        let (dir, ruta) = pdf_en_disco("rango");
        let documento = Documento::abrir(&ruta).unwrap();

        let e = documento.renderizar(3, 200).unwrap_err();
        assert!(
            matches!(
                e,
                ErrorPdf::PaginaFueraDeRango {
                    indice: 3,
                    paginas: 3
                }
            ),
            "salio {e:?}"
        );
        // El limite justo: la ultima pagina si existe.
        assert!(documento.renderizar(2, 200).is_ok());
        // Y un indice desmedido tampoco desborda.
        assert!(documento.renderizar(u32::MAX, 200).is_err());

        drop(documento);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn un_ancho_de_cero_o_desmedido_se_rechaza() {
        // Caso negativo. Un ancho de cero le sacaria a WinRT una excepcion, y
        // uno enorme una reserva de memoria que tumba el proceso.
        let (dir, ruta) = pdf_en_disco("ancho");
        let documento = Documento::abrir(&ruta).unwrap();

        assert!(matches!(
            documento.renderizar(0, 0),
            Err(ErrorPdf::AnchoInvalido { ancho: 0 })
        ));
        assert!(matches!(
            documento.renderizar(0, ANCHO_MAXIMO + 1),
            Err(ErrorPdf::AnchoInvalido { .. })
        ));
        // Un pixel de ancho es raro pero legitimo: no se rechaza.
        assert!(documento.renderizar(0, 1).is_ok());

        drop(documento);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn la_marca_pdf_se_reconoce_aunque_venga_precedida_de_basura() {
        // La norma permite bytes antes de `%PDF-` y los lectores de verdad la
        // buscan en el primer kilobyte. Rechazar por no estar en el byte cero
        // dejaria fuera ficheros que Windows si abre.
        assert!(parece_pdf(b"%PDF-1.7\n"));
        assert!(parece_pdf(b"basura por delante\n%PDF-1.4\n"));
        assert!(!parece_pdf(b"PK\x03\x04"));
        assert!(!parece_pdf(b""));
        // Y el corte no se pasa de largo: una marca partida no cuenta.
        assert!(!parece_pdf(b"%PDF"));
    }

    #[test]
    fn el_prefijo_extendido_se_quita_antes_de_dar_la_ruta_a_winrt() {
        // `GetFileFromPathAsync` rechaza por sintaxis las rutas `\\?\`, que es
        // justo la forma que devuelve `canonicalize`. Es un fallo silencioso y
        // desconcertante: el fichero existe y aun asi "no se encuentra".
        assert_eq!(sin_prefijo_extendido(r"\\?\C:\x\y.pdf"), r"C:\x\y.pdf");
        assert_eq!(
            sin_prefijo_extendido(r"\\?\UNC\servidor\comun\y.pdf"),
            r"\\servidor\comun\y.pdf"
        );
        // Una ruta normal se deja tal cual.
        assert_eq!(sin_prefijo_extendido(r"C:\x\y.pdf"), r"C:\x\y.pdf");
        assert_eq!(
            sin_prefijo_extendido(r"\\servidor\comun\y.pdf"),
            r"\\servidor\comun\y.pdf"
        );
    }

    #[test]
    fn un_pdf_que_pide_contrasena_no_se_confunde_con_uno_roto() {
        // El HRESULT se comprueba sin WinRT: `windows::core::Error` se puede
        // construir desde un codigo. Asi la clasificacion queda cubierta aunque
        // aqui no haya un PDF cifrado con el que provocarla de verdad.
        let cifrado = clasificar_fallo_de_carga(
            Path::new("x.pdf"),
            windows::core::Error::from(HR_CONTRASENA),
        );
        assert!(matches!(cifrado, ErrorPdf::Cifrado(_)), "salio {cifrado:?}");

        let roto = clasificar_fallo_de_carga(
            Path::new("x.pdf"),
            windows::core::Error::from(windows::Win32::Foundation::E_FAIL),
        );
        assert!(matches!(roto, ErrorPdf::Corrupto { .. }), "salio {roto:?}");
    }

    /// La proporcion alto/ancho de una medida.
    fn proporcion((w, h): (f32, f32)) -> f32 {
        h / w
    }

    #[test]
    fn las_medidas_de_las_paginas_tienen_la_proporcion_de_lo_que_se_dibuja() {
        // El lector continuo coloca las hojas con estas medidas ANTES de
        // dibujarlas: si no casan con lo que luego sale, el desplazamiento
        // salta al llegar cada hoja.
        let (dir, ruta) = pdf_en_disco("medidas");
        let documento = Documento::abrir(&ruta).unwrap();
        let medidas = documento.medidas();
        assert_eq!(medidas.len(), 3);
        for (i, m) in medidas.iter().enumerate() {
            let imagen = documento.renderizar(i as u32, 200).unwrap();
            let dibujada = imagen.alto as f32 / imagen.ancho as f32;
            assert!(
                (proporcion(*m) - dibujada).abs() < 0.03,
                "pagina {i}: medida {m:?} y dibujo {}x{}",
                imagen.ancho,
                imagen.alto
            );
        }
        assert!(medidas[0].0 > medidas[0].1, "la primera es apaisada");
        assert!(medidas[2].1 > medidas[2].0, "la tercera es vertical");
        drop(documento);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn una_pagina_girada_mide_lo_que_se_ve_y_no_lo_que_dice_su_papel() {
        // Papel apaisado (200x100) girado un cuarto de vuelta: en pantalla
        // es vertical, y eso es lo que el lector tiene que reservarle.
        let objetos = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Rotate 90 \
             /Contents 4 0 R /Resources << >> >>"
                .to_string(),
            flujo("0 0 0 rg 10 10 80 80 re f"),
        ];
        let mut bytes: Vec<u8> = b"%PDF-1.4\n".to_vec();
        let mut desplazamientos = Vec::new();
        for (i, cuerpo) in objetos.iter().enumerate() {
            desplazamientos.push(bytes.len());
            bytes.extend_from_slice(format!("{} 0 obj\n{cuerpo}\nendobj\n", i + 1).as_bytes());
        }
        let inicio = bytes.len();
        let total = objetos.len() + 1;
        bytes.extend_from_slice(format!("xref\n0 {total}\n0000000000 65535 f \n").as_bytes());
        for d in &desplazamientos {
            bytes.extend_from_slice(format!("{d:010} 00000 n \n").as_bytes());
        }
        bytes.extend_from_slice(
            format!("trailer\n<< /Size {total} /Root 1 0 R >>\nstartxref\n{inicio}\n%%EOF\n")
                .as_bytes(),
        );
        let dir = temporal("girada");
        let ruta = dir.join("girada.pdf");
        fs::write(&ruta, bytes).unwrap();
        let documento = Documento::abrir(&ruta).unwrap();
        let m = documento.medidas()[0];
        let imagen = documento.renderizar(0, 100).unwrap();
        let dibujada = imagen.alto as f32 / imagen.ancho as f32;
        assert!((proporcion(m) - dibujada).abs() < 0.05, "medida {m:?}");
        assert!(
            dibujada > 1.0,
            "el dibujo sale girado: {}x{}",
            imagen.ancho,
            imagen.alto
        );
        drop(documento);
        let _ = fs::remove_dir_all(dir);
    }

    /// Cuantos pixeles oscuros tiene una imagen.
    fn oscuros(imagen: &ImagenRgba) -> usize {
        imagen
            .pixeles
            .chunks_exact(4)
            .filter(|p| (p[0] as u32 + p[1] as u32 + p[2] as u32) < 150)
            .count()
    }

    #[test]
    fn un_trozo_de_la_pagina_sale_a_la_resolucion_pedida_y_de_su_sitio() {
        // La pagina 0 (200x100) tiene el cuadrado negro a la IZQUIERDA: la
        // mitad izquierda pedida a 400x400 es casi toda negra y la derecha,
        // blanca. Si `SourceRect` se ignorase, las dos saldrian iguales.
        let (dir, ruta) = pdf_en_disco("trozo");
        let documento = Documento::abrir(&ruta).unwrap();
        let izquierda = documento
            .renderizar_trozo(0, (0.0, 0.0, 0.5, 1.0), 400, 400)
            .unwrap();
        let derecha = documento
            .renderizar_trozo(0, (0.5, 0.0, 0.5, 1.0), 400, 400)
            .unwrap();
        // Cuatro veces mas fino que la pagina entera a 200: es la razon de ser.
        assert!(
            izquierda.ancho >= 380 && izquierda.ancho <= 420,
            "{}",
            izquierda.ancho
        );
        assert!(
            izquierda.alto >= 380 && izquierda.alto <= 420,
            "{}",
            izquierda.alto
        );
        let total = (izquierda.ancho * izquierda.alto) as usize;
        assert!(
            oscuros(&izquierda) > total / 2,
            "la izquierda tiene el cuadrado"
        );
        assert_eq!(oscuros(&derecha), 0, "la derecha esta en blanco");
        // Casos negativos: un trozo vacio o una pagina que no hay.
        assert!(
            documento
                .renderizar_trozo(0, (0.0, 0.0, 0.0, 1.0), 10, 10)
                .is_err()
        );
        assert!(
            documento
                .renderizar_trozo(9, (0.0, 0.0, 1.0, 1.0), 10, 10)
                .is_err()
        );
        assert!(
            documento
                .renderizar_trozo(0, (0.0, 0.0, 1.0, 1.0), 0, 10)
                .is_err()
        );
        drop(documento);
        let _ = fs::remove_dir_all(dir);
    }
}
