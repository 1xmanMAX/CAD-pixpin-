//! **Lo que el crate `image` no sabe leer, a traves de Windows** (WIC,
//! Windows Imaging Component; el usuario, 3-oct: «que pixpin sea la app
//! predeterminada para fotos de todos los formatos comunes»).
//!
//! `image` va compilado solo con PNG, JPEG, WebP y BMP (cada formato es peso
//! en el .exe). Lo demas lo lee Windows con sus propios descodificadores,
//! que ya estan en el equipo y no pesan nada: GIF, TIFF, ICO y JPEG XR
//! siempre; y con la extension correspondiente de Microsoft Store, HEIC/HEIF
//! (las fotos del iPhone), AVIF, JPEG XL y los RAW de camara.
//!
//! El camino es el de siempre en WIC: fabrica → descodificador por nombre de
//! fichero (WIC mira DENTRO para elegir codec, no la extension) → primer
//! fotograma → conversor a RGBA de 8 bits recto → pixeles. El conversor
//! hace la conversion de color y de profundidad (un HEIC de 10 bits o un
//! RAW de 16 bajan a 8) sin que aqui haya que saber nada del formato.

use std::path::Path;

use windows::Win32::Foundation::{GENERIC_READ, RPC_E_CHANGED_MODE};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, IWICBitmapFrameDecode, IWICImagingFactory,
    WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::core::HSTRING;

use crate::imagen::ImagenRgba;

/// Las extensiones que van a Windows directamente, sin probar antes con
/// `image`: no estan entre sus formatos compilados, asi que probar primero
/// solo seria abrir el fichero dos veces.
pub const SOLO_WINDOWS: [&str; 17] = [
    "heic", "heif", "hif", "avif", "jxl", "gif", "tif", "tiff", "ico", "jxr", "wdp", "dng", "cr2",
    "cr3", "nef", "arw", "orf",
];

/// Por donde se intenta leer una imagen, en orden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Camino {
    /// `image` primero (rapido y sin COM) y, si no puede, Windows.
    ImagePrimero,
    /// Directo a Windows.
    SoloWindows,
}

/// Que camino toca segun la extension (puro, sin tocar disco). Sin extension
/// (las fotos del movil se guardan como `imagenes/<id>`) se va por `image`,
/// que mira dentro, y Windows queda de reserva.
pub fn camino_de(ruta: &Path) -> Camino {
    let ext = ruta
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if SOLO_WINDOWS.contains(&ext.as_str()) {
        Camino::SoloWindows
    } else {
        Camino::ImagePrimero
    }
}

/// Lo que hace falta instalar de Microsoft Store para leer una extension,
/// si es de las que Windows no trae de serie. Es lo que se le dice al
/// usuario cuando su foto no se puede abrir: «falta X», no «error».
pub fn extension_de_la_tienda(ruta: &Path) -> Option<&'static str> {
    let ext = ruta.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "heic" | "heif" | "hif" => "HEIF Image Extensions + HEVC Video Extensions",
        "avif" => "AV1 Video Extension",
        "jxl" => "JPEG XL Image Extension",
        "dng" | "cr2" | "cr3" | "nef" | "arw" | "orf" => "Raw Image Extension",
        _ => return None,
    })
}

/// COM en este hilo mientras dure: lo inicia si no lo estaba y lo cierra
/// al soltarse. Si el hilo ya tenia COM en otro modo (el hilo de una
/// ventana, en STA) se usa el que hay: WIC funciona en los dos.
pub(crate) struct Com(bool);

impl Com {
    pub(crate) fn iniciar() -> Com {
        // SAFETY: sin punteros; cada exito se empareja con su
        // `CoUninitialize` en el `Drop`.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        Com(hr.is_ok() && hr != RPC_E_CHANGED_MODE)
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: empareja el `CoInitializeEx` que salio bien.
            unsafe { CoUninitialize() };
        }
    }
}

pub(crate) fn primer_fotograma(
    ruta: &Path,
) -> windows::core::Result<(IWICImagingFactory, IWICBitmapFrameDecode)> {
    // SAFETY: fabrica de WIC del sistema, en proceso.
    let fabrica: IWICImagingFactory =
        unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)? };
    let nombre = HSTRING::from(ruta.as_os_str());
    // SAFETY: `nombre` vive durante la llamada; sin GUID de fabricante (que
    // WIC elija el codec mirando el contenido); los metadatos, solo si se
    // piden, que es lo barato.
    let descodificador = unsafe {
        fabrica.CreateDecoderFromFilename(
            &nombre,
            None,
            GENERIC_READ,
            WICDecodeMetadataCacheOnDemand,
        )?
    };
    // SAFETY: descodificador vivo; el fotograma 0 existe en toda imagen
    // que WIC acepta (un GIF animado da el primero).
    let fotograma = unsafe { descodificador.GetFrame(0)? };
    Ok((fabrica, fotograma))
}

/// Cuanto mide, leyendo solo la cabecera (WIC no descomprime hasta que se
/// le piden los pixeles). Ya girada segun su EXIF, como la da `cargar`.
pub fn medidas(ruta: &Path) -> windows::core::Result<(u32, u32)> {
    let _com = Com::iniciar();
    let (_, fotograma) = primer_fotograma(ruta)?;
    let (mut ancho, mut alto) = (0u32, 0u32);
    // SAFETY: fotograma vivo; las dos salidas son variables locales.
    unsafe { fotograma.GetSize(&mut ancho, &mut alto)? };
    let orientacion = crate::vista::orientacion_de(&fotograma);
    Ok(crate::vista::medidas_giradas(ancho, alto, orientacion))
}

/// La imagen entera en RGBA recto de 8 bits, como `imagen::cargar`, y
/// derecha segun su orientacion EXIF (una foto del movil hecha en vertical
/// sale en vertical).
pub fn cargar(ruta: &Path) -> windows::core::Result<ImagenRgba> {
    let _com = Com::iniciar();
    let (fabrica, fotograma) = primer_fotograma(ruta)?;
    let orientacion = crate::vista::orientacion_de(&fotograma);
    let fuente = crate::vista::fuente_derecha(&fabrica, &fotograma, orientacion, None)?;
    crate::vista::a_imagen(&fuente)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn heic_avif_raw_y_tiff_van_directos_a_windows() {
        for r in [
            "IMG_0001.HEIC",
            "a.heif",
            "b.avif",
            "c.jxl",
            "d.tif",
            "e.gif",
            "f.ico",
            "g.dng",
            "h.NEF",
        ] {
            assert_eq!(camino_de(Path::new(r)), Camino::SoloWindows, "{r}");
        }
        // Caso negativo: lo que `image` lee bien no paga COM, y lo que no
        // trae extension se mira por dentro antes.
        for r in [
            "a.png",
            "b.JPG",
            "c.jpeg",
            "d.webp",
            "e.bmp",
            "f.jfif",
            "imagenes/0123abcd",
        ] {
            assert_eq!(camino_de(Path::new(r)), Camino::ImagePrimero, "{r}");
        }
    }

    #[test]
    fn a_cada_formato_de_la_tienda_le_toca_su_extension() {
        assert!(extension_de_la_tienda(Path::new("x.heic")).is_some_and(|e| e.contains("HEIF")));
        assert!(extension_de_la_tienda(Path::new("x.HEIF")).is_some_and(|e| e.contains("HEVC")));
        assert!(extension_de_la_tienda(Path::new("x.avif")).is_some_and(|e| e.contains("AV1")));
        assert!(extension_de_la_tienda(Path::new("x.cr2")).is_some_and(|e| e.contains("Raw")));
        // Caso negativo: lo que Windows trae de serie no pide nada.
        for r in ["x.png", "x.tif", "x.gif", "x.jfif", "sin_extension"] {
            assert_eq!(extension_de_la_tienda(Path::new(r)), None, "{r}");
        }
    }

    /// Una imagen de verdad de 3x2, azul, escrita por WIC mismo en el
    /// contenedor que se pida: asi no hacen falta ficheros de prueba.
    fn escribir_con_windows(
        ruta: &Path,
        contenedor: &windows::core::GUID,
    ) -> windows::core::Result<()> {
        use windows::Win32::Foundation::GENERIC_WRITE;
        use windows::Win32::Graphics::Imaging::{
            GUID_WICPixelFormat32bppBGRA, WICBitmapEncoderNoCache,
        };
        let _com = Com::iniciar();
        // SAFETY: protocolo de codificacion de WIC con objetos propios y
        // vivos durante el bloque; el buffer mide 3 * 2 * 4 bytes.
        unsafe {
            let f: IWICImagingFactory =
                CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
            let flujo = f.CreateStream()?;
            flujo.InitializeFromFilename(&HSTRING::from(ruta.as_os_str()), GENERIC_WRITE.0)?;
            let cod = f.CreateEncoder(contenedor, std::ptr::null())?;
            cod.Initialize(&flujo, WICBitmapEncoderNoCache)?;
            let mut marco = None;
            cod.CreateNewFrame(&mut marco, std::ptr::null_mut())?;
            let marco = marco
                .ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_FAIL))?;
            marco.Initialize(None)?;
            marco.SetSize(3, 2)?;
            let mut formato = GUID_WICPixelFormat32bppBGRA;
            marco.SetPixelFormat(&mut formato)?;
            // BGRA: azul puro en todos.
            let px: Vec<u8> = [255u8, 0, 0, 255].repeat(6);
            marco.WritePixels(2, 12, &px)?;
            marco.Commit()?;
            cod.Commit()
        }
    }

    #[test]
    fn windows_lee_un_tiff_que_image_no_sabe_leer() {
        use windows::Win32::Graphics::Imaging::GUID_ContainerFormatTiff;
        let dir = std::env::temp_dir().join("pixpin-codec-wic");
        let _ = std::fs::create_dir_all(&dir);
        let ruta = dir.join("azul.tif");
        escribir_con_windows(&ruta, &GUID_ContainerFormatTiff).unwrap();
        assert_eq!(medidas(&ruta).unwrap(), (3, 2));
        let img = cargar(&ruta).unwrap();
        assert_eq!((img.ancho, img.alto), (3, 2));
        assert_eq!(img.pixeles.len(), img.bytes_esperados());
        // Ya en RGBA: el azul queda en el tercer canal.
        assert_eq!(&img.pixeles[0..4], &[0, 0, 255, 255]);
        // Y por la puerta de siempre, `imagen::cargar`, tambien.
        assert_eq!(crate::imagen::cargar(&ruta).unwrap(), img);
        assert_eq!(crate::imagen::medidas(&ruta).unwrap(), (3, 2));
    }

    /// Los codecs de imagen de ESTE Windows: (nombre, extensiones,
    /// contenedor, si es codificador).
    fn codecs() -> Vec<(String, String, windows::core::GUID, bool)> {
        use windows::Win32::Graphics::Imaging::{
            IWICBitmapCodecInfo, IWICBitmapEncoderInfo, WICComponentEnumerateDefault, WICDecoder,
            WICEncoder,
        };
        use windows::core::{IUnknown, Interface};
        let _com = Com::iniciar();
        let mut v = Vec::new();
        // SAFETY: enumeracion de WIC con objetos propios; los buffers son
        // locales y sus longitudes van con ellos.
        unsafe {
            let f: IWICImagingFactory =
                CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).unwrap();
            let e = f
                .CreateComponentEnumerator(
                    (WICDecoder.0 | WICEncoder.0) as u32,
                    WICComponentEnumerateDefault.0 as u32,
                )
                .unwrap();
            loop {
                let mut uno: [Option<IUnknown>; 1] = [None];
                let mut n = 0u32;
                let _ = e.Next(&mut uno, Some(&mut n));
                let Some(u) = uno[0].take().filter(|_| n == 1) else {
                    break;
                };
                let Ok(info) = u.cast::<IWICBitmapCodecInfo>() else {
                    continue;
                };
                let mut nombre = [0u16; 256];
                let mut exts = [0u16; 1024];
                let (mut a, mut b) = (0u32, 0u32);
                let _ = info.GetFriendlyName(&mut nombre, &mut a);
                let _ = info.GetFileExtensions(&mut exts, &mut b);
                let t = |v: &[u16], n: u32| {
                    String::from_utf16_lossy(&v[..(n as usize).saturating_sub(1)])
                };
                let contenedor = info.GetContainerFormat().unwrap_or_default();
                let codifica = u.cast::<IWICBitmapEncoderInfo>().is_ok();
                v.push((t(&nombre, a), t(&exts, b), contenedor, codifica));
            }
        }
        v
    }

    #[test]
    fn windows_lee_jxr_y_jpeg_xl_si_el_equipo_lo_trae() {
        use windows::Win32::Graphics::Imaging::GUID_ContainerFormatWmp;
        // JPEG XR viene siempre. JPEG XL tiene su codificador en Windows 11
        // 24H2, pero sin la extension de la tienda no arranca (medido el
        // 3-oct: 0x88982F8B): si no se puede escribir, no se prueba.
        let jxl = codecs()
            .into_iter()
            .find(|(_, exts, _, codifica)| *codifica && exts.to_ascii_lowercase().contains(".jxl"))
            .map(|(_, _, c, _)| c);
        let dir = std::env::temp_dir().join("pixpin-codec-wic");
        let _ = std::fs::create_dir_all(&dir);
        let mut casos = vec![("azul.jxr", GUID_ContainerFormatWmp, true)];
        if let Some(c) = jxl {
            casos.push(("azul.jxl", c, false));
        }
        for (nombre, contenedor, siempre) in casos {
            let ruta = dir.join(nombre);
            if let Err(e) = escribir_con_windows(&ruta, &contenedor) {
                assert!(!siempre, "{nombre}: Windows lo trae siempre: {e}");
                println!("{nombre}: este Windows no lo escribe ({e}); no se prueba");
                continue;
            }
            let img = crate::imagen::cargar(&ruta).unwrap_or_else(|e| panic!("{nombre}: {e}"));
            assert_eq!((img.ancho, img.alto), (3, 2), "{nombre}");
            let [r, g, b] = [img.pixeles[0], img.pixeles[1], img.pixeles[2]];
            assert!(
                b > 200 && r < 60 && g < 60,
                "{nombre}: azul, no {r},{g},{b}"
            );
            assert_eq!(crate::imagen::medidas(&ruta).unwrap(), (3, 2), "{nombre}");
        }
    }

    #[test]
    fn un_gif_que_image_no_sabe_leer_sale_por_windows() {
        // El GIF lo escribe el codificador propio (`crate::gif`): `image` no
        // lo lee (no va compilado con GIF) y antes se quedaba en ficha.
        let rojo = ImagenRgba {
            ancho: 4,
            alto: 3,
            pixeles: [220u8, 20, 20, 255].repeat(12),
        };
        let bytes = crate::gif::codificar(&[rojo], crate::gif::OpcionesGif::default()).unwrap();
        let dir = std::env::temp_dir().join("pixpin-codec-wic");
        let _ = std::fs::create_dir_all(&dir);
        let ruta = dir.join("rojo.gif");
        std::fs::write(&ruta, bytes).unwrap();
        assert_eq!(camino_de(&ruta), Camino::SoloWindows);
        let img = crate::imagen::cargar(&ruta).unwrap();
        assert_eq!((img.ancho, img.alto), (4, 3));
        assert!(
            img.pixeles[0] > 180 && img.pixeles[1] < 60,
            "rojo: {:?}",
            &img.pixeles[..4]
        );
        assert_eq!(crate::imagen::medidas(&ruta).unwrap(), (4, 3));
    }

    /// Lo que Windows sabe leer y escribir en ESTE equipo (cambia con lo
    /// instalado de la tienda). Diagnostico, no prueba: `cargo test -p
    /// pixpin-codec inventario -- --ignored --nocapture`.
    #[test]
    #[ignore = "diagnostico: lista los codecs de este equipo"]
    fn inventario_de_codecs() {
        for (nombre, exts, _, _) in codecs() {
            println!("{nombre} | {exts}");
        }
    }

    /// Prueba a leer los ficheros de `PIXPIN_PROBAR_IMAGENES` (separados por
    /// `;`) por la puerta de siempre y dice que pasa con cada uno.
    /// Diagnostico: `cargo test -p pixpin-codec probar_ficheros -- --ignored
    /// --nocapture`.
    #[test]
    #[ignore = "diagnostico: lee ficheros reales de este equipo"]
    fn probar_ficheros_de_la_variable() {
        let lista = std::env::var("PIXPIN_PROBAR_IMAGENES").unwrap_or_default();
        for r in lista.split(';').filter(|r| !r.is_empty()) {
            let r = Path::new(r);
            let t = std::time::Instant::now();
            let m = crate::imagen::medidas(r);
            let tm = t.elapsed();
            let t = std::time::Instant::now();
            let c = crate::imagen::cargar(r).map(|i| (i.ancho, i.alto));
            println!(
                "{} | medidas {m:?} en {tm:?} | cargar {c:?} en {:?}",
                r.display(),
                t.elapsed()
            );
        }
    }

    #[test]
    fn un_fichero_que_no_es_imagen_da_error_y_no_pixeles_vacios() {
        // Caso negativo: un texto con extension de foto no se convierte en
        // una imagen de 0x0 que luego rompe el pin.
        let dir = std::env::temp_dir().join("pixpin-codec-wic");
        let _ = std::fs::create_dir_all(&dir);
        let ruta = dir.join("no-soy-foto.heic");
        std::fs::write(&ruta, b"hola, no soy una foto").unwrap();
        assert!(cargar(&ruta).is_err());
        assert!(medidas(&ruta).is_err());
        // Por la puerta de siempre, el fallo dice que extension instalar
        // (sin HEVC, o con el fichero roto, WIC no distingue mucho: lo
        // importante es que no salga un error de numeros).
        let e = crate::imagen::cargar(&ruta).unwrap_err();
        assert!(e.to_string().contains("no-soy-foto.heic"), "{e}");
        assert!(cargar(Path::new("Z:/no/existe.heic")).is_err());
        // Caso negativo: lo que no existe no manda a la tienda.
        let e = crate::imagen::cargar(Path::new("Z:/no/existe.heic")).unwrap_err();
        assert_eq!(e.extension_que_falta(), None, "{e}");
    }
}
