//! **Una imagen leida para VERLA, no para editarla** (abrir una foto como
//! pin; el usuario, 4-oct-2026: «que las imagenes se abran lo mas rapido
//! posible consumiendo lo minimo»).
//!
//! Una foto de movil son 12 megapixeles: 48 MB en RGBA. El pin la ensena,
//! como mucho, al 80 % de la pantalla. Descodificarla entera para
//! ensenarla a un cuarto de su tamano es pagar la descompresion de todos los
//! pixeles, una copia de 48 MB en memoria y otra igual en la GPU. Aqui se
//! descodifica **a la medida en que se va a ver**, con Windows (WIC):
//!
//! - El escalador de WIC sobre un JPEG no descomprime y luego reduce: le
//!   pide al descodificador la reduccion en la propia transformada (DCT a
//!   1/2, 1/4, 1/8), asi que leer a la mitad cuesta mucho menos que leer
//!   entero.
//! - Se aplica la **orientacion EXIF**: una foto del movil hecha en vertical
//!   se guarda tumbada con una etiqueta que dice como girarla, y sin mirarla
//!   el pin salia de lado.
//! - Sale directamente en RGBA de 8 bits, que es lo que sube la GPU sin mas
//!   conversion.
//!
//! La resolucion completa sigue en el fichero: la pide el pin si se acerca
//! mas alla de lo leido (`cargar` la da entera y girada igual).

use std::path::Path;

use windows::Win32::Graphics::Imaging::{
    GUID_WICPixelFormat32bppRGBA, IWICBitmapFrameDecode, IWICBitmapSource, WICBitmapDitherTypeNone,
    WICBitmapInterpolationModeFant, WICBitmapPaletteTypeCustom, WICBitmapTransformFlipHorizontal,
    WICBitmapTransformFlipVertical, WICBitmapTransformOptions, WICBitmapTransformRotate0,
    WICBitmapTransformRotate90, WICBitmapTransformRotate180, WICBitmapTransformRotate270,
};
use windows::core::Interface;

use crate::imagen::{ErrorCodec, ImagenRgba};

/// Lo leido para ver, y cuanto mide la imagen entera.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vista {
    /// A la medida pedida o algo mayor (nunca mas que la imagen), ya girada.
    pub imagen: ImagenRgba,
    /// La imagen ENTERA, ya girada: es su «100 %», y en esos pixeles van las
    /// anotaciones y el texto reconocido.
    pub ancho_completo: u32,
    pub alto_completo: u32,
}

impl Vista {
    /// Si lo leido es menos que la imagen entera.
    pub fn reducida(&self) -> bool {
        self.imagen.ancho < self.ancho_completo || self.imagen.alto < self.alto_completo
    }
}

/// Si una orientacion EXIF (1 a 8) cambia el ancho por el alto.
pub fn tumbada(orientacion: u16) -> bool {
    (5..=8).contains(&orientacion)
}

/// Las medidas que se VEN de una imagen de `ancho` x `alto` guardada con
/// esa orientacion.
pub fn medidas_giradas(ancho: u32, alto: u32, orientacion: u16) -> (u32, u32) {
    if tumbada(orientacion) {
        (alto, ancho)
    } else {
        (ancho, alto)
    }
}

/// Lo que hay que hacer con los pixeles guardados para verlos derechos,
/// segun la etiqueta EXIF. Una etiqueta desconocida se trata como «tal
/// cual»: es lo que hacen todos los visores.
pub fn transformacion_exif(orientacion: u16) -> WICBitmapTransformOptions {
    let o = |v: i32| WICBitmapTransformOptions(v);
    match orientacion {
        2 => WICBitmapTransformFlipHorizontal,
        3 => WICBitmapTransformRotate180,
        4 => WICBitmapTransformFlipVertical,
        // Traspuesta: girar 90 y voltear en horizontal.
        5 => o(WICBitmapTransformRotate90.0 | WICBitmapTransformFlipHorizontal.0),
        6 => WICBitmapTransformRotate90,
        // Transversa: girar 270 y voltear en horizontal.
        7 => o(WICBitmapTransformRotate270.0 | WICBitmapTransformFlipHorizontal.0),
        8 => WICBitmapTransformRotate270,
        _ => WICBitmapTransformRotate0,
    }
}

/// A que medida leer una imagen de `completo` (ya girada) para verla dentro
/// de `caja`: la mayor que cabe sin deformar y **sin agrandar nunca**. Una
/// caja con un lado a 0 es «sin limite».
pub fn medida_de_lectura(completo: (u32, u32), caja: (u32, u32)) -> (u32, u32) {
    let (w, h) = (completo.0.max(1), completo.1.max(1));
    if caja.0 == 0 || caja.1 == 0 || (w <= caja.0 && h <= caja.1) {
        return (w, h);
    }
    let f = (caja.0 as f64 / w as f64).min(caja.1 as f64 / h as f64);
    (
        ((w as f64 * f).round() as u32).max(1),
        ((h as f64 * f).round() as u32).max(1),
    )
}

/// La etiqueta de orientacion de un fotograma (1 si no tiene o no se lee).
/// `System.Photo.Orientation` es la politica de metadatos de Windows: la
/// misma pregunta vale para JPEG, TIFF y HEIC.
pub(crate) fn orientacion_de(fotograma: &IWICBitmapFrameDecode) -> u16 {
    use windows::Win32::System::Com::StructuredStorage::{PROPVARIANT, PropVariantClear};
    use windows::Win32::System::Variant::VT_UI2;
    // SAFETY: fotograma vivo; `valor` es nuestro, se inicializa vacio (como
    // pide la API) y se libera con `PropVariantClear` sea lo que sea.
    unsafe {
        let Ok(lector) = fotograma.GetMetadataQueryReader() else {
            return 1;
        };
        let mut valor = PROPVARIANT::default();
        let o = match lector
            .GetMetadataByName(windows::core::w!("System.Photo.Orientation"), &mut valor)
        {
            Ok(()) if valor.Anonymous.Anonymous.vt == VT_UI2 => {
                valor.Anonymous.Anonymous.Anonymous.uiVal
            }
            _ => 1,
        };
        let _ = PropVariantClear(&mut valor);
        if (1..=8).contains(&o) { o } else { 1 }
    }
}

/// El filtro con que se reduce una imagen de `entera` a `medida`. Fant
/// (promedia todos los pixeles de origen: lo nitido de una foto sin dientes
/// de sierra) cuando se reduce a menos de la mitad; bilineal, que cuesta un
/// tercio menos en una PNG grande (`medir_lectura`), cuando se reduce poco,
/// que es donde no se nota la diferencia.
pub fn interpolacion_para(
    entera: (u32, u32),
    medida: (u32, u32),
) -> windows::Win32::Graphics::Imaging::WICBitmapInterpolationMode {
    use windows::Win32::Graphics::Imaging::WICBitmapInterpolationModeLinear;
    if medida.0 * 2 < entera.0 || medida.1 * 2 < entera.1 {
        WICBitmapInterpolationModeFant
    } else {
        WICBitmapInterpolationModeLinear
    }
}

/// Encadena al fotograma el escalador (si `medida`, ya girada, es menor que
/// la imagen), la conversion a RGBA de 8 bits y lo que la deja derecha
/// (`orientacion`).
///
/// No hace falta pedir al JPEG que reduzca en su transformada
/// (`IWICBitmapSourceTransform`, 1/2, 1/4, 1/8): el escalador de WIC ya lo
/// hace por dentro. Medido el 4-oct-2026 con una foto de movil 4032x3024 a
/// 864x648: 91 ms con el escalador solo y 92 pidiendolo a mano.
pub(crate) fn fuente_derecha(
    fabrica: &windows::Win32::Graphics::Imaging::IWICImagingFactory,
    fotograma: &IWICBitmapFrameDecode,
    orientacion: u16,
    medida: Option<(u32, u32)>,
) -> windows::core::Result<IWICBitmapSource> {
    use windows::Win32::Graphics::Imaging::WICBitmapCacheOnLoad;
    let mut fuente: IWICBitmapSource = fotograma.cast()?;
    let (mut ancho, mut alto) = (0u32, 0u32);
    // SAFETY: fotograma vivo; salidas locales.
    unsafe { fotograma.GetSize(&mut ancho, &mut alto)? };
    if let Some((vw, vh)) = medida {
        // Se reduce ANTES del giro: lo que se reduce es lo guardado, que
        // esta tumbado si la etiqueta lo dice.
        let (sw, sh) = if tumbada(orientacion) {
            (vh, vw)
        } else {
            (vw, vh)
        };
        if sw < ancho || sh < alto {
            let modo = interpolacion_para((ancho, alto), (sw, sh));
            // SAFETY: fabrica y fuente vivas; medidas no nulas.
            unsafe {
                let escalador = fabrica.CreateBitmapScaler()?;
                escalador.Initialize(&fuente, sw.max(1), sh.max(1), modo)?;
                fuente = escalador.cast()?;
            }
        }
    }
    // SAFETY: fabrica y fuente vivas; sin paleta.
    let convertida: IWICBitmapSource = unsafe {
        let conversor = fabrica.CreateFormatConverter()?;
        conversor.Initialize(
            &fuente,
            &GUID_WICPixelFormat32bppRGBA,
            WICBitmapDitherTypeNone,
            None,
            0.0,
            WICBitmapPaletteTypeCustom,
        )?;
        conversor.cast()?
    };
    if orientacion == 1 {
        return Ok(convertida);
    }
    // El giro, sobre la imagen YA leida en memoria: el que gira pide los
    // pixeles por columnas, y sobre el escalador (que los da por filas) cada
    // columna rehacia la imagen entera. Medido: una foto de movil girada
    // tardaba minutos; asi, lo que tarda copiarla una vez.
    // SAFETY: fabrica y fuente vivas; la transformacion es una constante.
    unsafe {
        let en_memoria = fabrica.CreateBitmapFromSource(&convertida, WICBitmapCacheOnLoad)?;
        let giro = fabrica.CreateBitmapFlipRotator()?;
        giro.Initialize(&en_memoria, transformacion_exif(orientacion))?;
        giro.cast()
    }
}

/// Copia una fuente WIC entera a una `ImagenRgba` (filas compactas).
pub(crate) fn a_imagen(fuente: &IWICBitmapSource) -> windows::core::Result<ImagenRgba> {
    let (mut ancho, mut alto) = (0u32, 0u32);
    // SAFETY: fuente viva; salidas locales.
    unsafe { fuente.GetSize(&mut ancho, &mut alto)? };
    let fila = (ancho as usize)
        .checked_mul(4)
        .filter(|f| *f <= u32::MAX as usize)
        .ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_OUTOFMEMORY))?;
    let total = fila
        .checked_mul(alto as usize)
        .ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_OUTOFMEMORY))?;
    let mut pixeles = vec![0u8; total];
    // SAFETY: el buffer es propio y mide exactamente fila * alto; la fuente
    // da RGBA de 8 bits sin relleno.
    unsafe { fuente.CopyPixels(std::ptr::null(), fila as u32, &mut pixeles)? };
    Ok(ImagenRgba {
        ancho,
        alto,
        pixeles,
    })
}

/// Lee `ruta` para verla dentro de `caja` (pixeles; `(0, 0)` = entera),
/// derecha segun su EXIF. Cualquier formato que lea Windows.
pub fn cargar_para_ver(ruta: &Path, caja: (u32, u32)) -> Result<Vista, ErrorCodec> {
    // Lo que ya cabe y `image` sabe leer (JPEG, PNG, WebP, BMP) va entero por
    // `image`: sin nada que reducir, su JPEG es mas rapido que el de Windows
    // (medido: 25 ms frente a 37-46 con una foto de 879x1280). La cabecera
    // cuesta un milisegundo.
    if crate::wic::camino_de(ruta) == crate::wic::Camino::ImagePrimero
        && let Ok(medidas) = crate::imagen::medidas(ruta)
        && medida_de_lectura(medidas, caja) == medidas
        && let Ok(imagen) = crate::cargar(ruta)
    {
        return Ok(Vista {
            ancho_completo: imagen.ancho,
            alto_completo: imagen.alto,
            imagen,
        });
    }
    leer(ruta, caja).map_err(|e| crate::imagen::error_de_windows(ruta, e))
}

/// Desde cuantos pixeles merece la pena ensenar antes una vista previa: por
/// debajo, leer la buena ya es cuestion de decenas de milisegundos.
pub const PIXELES_PARA_VISTA_PREVIA: u64 = 4_000_000;

/// Si a una imagen de `entera` pixeles le toca vista previa.
pub fn quiere_vista_previa(entera: (u32, u32)) -> bool {
    entera.0 as u64 * entera.1 as u64 >= PIXELES_PARA_VISTA_PREVIA
}

/// **Una primera imagen al momento**, para ensenar el pin ya y afinarlo
/// cuando llegue la buena (`cargar_para_ver`, en otro hilo). Solo para
/// fotos grandes (`quiere_vista_previa`); `Ok(None)` si no hace falta.
///
/// La miniatura que la camara guarda dentro del JPEG (EXIF) si la hay, que
/// sale sin descomprimir nada; si no, la foto leida a un cuarto de la medida
/// a la que se vera, que el JPEG da casi sin hacer la transformada.
pub fn vista_previa(ruta: &Path, caja: (u32, u32)) -> Result<Option<Vista>, ErrorCodec> {
    previa(ruta, caja).map_err(|e| crate::imagen::error_de_windows(ruta, e))
}

/// Si el descodificador sabe dar la imagen a la mitad sin descomprimirla
/// entera (el JPEG si; la PNG dice que si a la pregunta pero devuelve la
/// medida entera).
fn se_lee_reducida(fotograma: &IWICBitmapFrameDecode, entera: (u32, u32)) -> bool {
    use windows::Win32::Graphics::Imaging::IWICBitmapSourceTransform;
    let Ok(t) = fotograma.cast::<IWICBitmapSourceTransform>() else {
        return false;
    };
    let (mut ancho, mut alto) = ((entera.0 / 2).max(1), (entera.1 / 2).max(1));
    // SAFETY: interfaz viva; salidas locales.
    unsafe { t.GetClosestSize(&mut ancho, &mut alto) }.is_ok() && ancho < entera.0
}

fn previa(ruta: &Path, caja: (u32, u32)) -> windows::core::Result<Option<Vista>> {
    use windows::Win32::Graphics::Imaging::WICBitmapCacheOnLoad;
    let _com = crate::wic::Com::iniciar();
    let (fabrica, fotograma) = crate::wic::primer_fotograma(ruta)?;
    let (mut ancho, mut alto) = (0u32, 0u32);
    // SAFETY: fotograma vivo; salidas locales.
    unsafe { fotograma.GetSize(&mut ancho, &mut alto)? };
    if !quiere_vista_previa((ancho, alto)) {
        return Ok(None);
    }
    let orientacion = orientacion_de(&fotograma);
    let (ancho_completo, alto_completo) = medidas_giradas(ancho, alto, orientacion);
    let medida = medida_de_lectura((ancho_completo, alto_completo), caja);
    let cuarto = ((medida.0 / 4).max(1), (medida.1 / 4).max(1));
    // La miniatura EXIF, si la hay y tiene la misma forma que la foto (hay
    // camaras que la guardan recortada a 4:3).
    // SAFETY: fotograma vivo.
    let miniatura = unsafe { fotograma.GetThumbnail() }.ok().filter(|m| {
        let (mut w, mut h) = (0u32, 0u32);
        // SAFETY: fuente viva; salidas locales.
        unsafe { m.GetSize(&mut w, &mut h) }.is_ok()
            && w > 0
            && h > 0
            && ((w as f64 / h as f64) - (ancho as f64 / alto as f64)).abs() < 0.02
    });
    // Sin miniatura, la previa solo sale a cuenta si el formato sabe leerse
    // reducido en la propia descompresion (el JPEG). Una PNG no: su previa
    // costaba lo mismo que la buena (medido: 261 ms frente a 276) y se
    // pagaba dos veces.
    if miniatura.is_none() && !se_lee_reducida(&fotograma, (ancho, alto)) {
        return Ok(None);
    }
    let imagen = match miniatura {
        Some(m) => {
            // La miniatura va tumbada igual que la foto: mismo giro.
            // SAFETY: fabrica y fuente vivas; sin paleta.
            let fuente: IWICBitmapSource = unsafe {
                let conversor = fabrica.CreateFormatConverter()?;
                conversor.Initialize(
                    &m,
                    &GUID_WICPixelFormat32bppRGBA,
                    WICBitmapDitherTypeNone,
                    None,
                    0.0,
                    WICBitmapPaletteTypeCustom,
                )?;
                if orientacion == 1 {
                    conversor.cast()?
                } else {
                    let en_memoria =
                        fabrica.CreateBitmapFromSource(&conversor, WICBitmapCacheOnLoad)?;
                    let giro = fabrica.CreateBitmapFlipRotator()?;
                    giro.Initialize(&en_memoria, transformacion_exif(orientacion))?;
                    giro.cast()?
                }
            };
            a_imagen(&fuente)?
        }
        None => a_imagen(&fuente_derecha(
            &fabrica,
            &fotograma,
            orientacion,
            Some(cuarto),
        )?)?,
    };
    Ok(Some(Vista {
        imagen,
        ancho_completo,
        alto_completo,
    }))
}

fn leer(ruta: &Path, caja: (u32, u32)) -> windows::core::Result<Vista> {
    let _com = crate::wic::Com::iniciar();
    let (fabrica, fotograma) = crate::wic::primer_fotograma(ruta)?;
    let (mut ancho, mut alto) = (0u32, 0u32);
    // SAFETY: fotograma vivo; salidas locales.
    unsafe { fotograma.GetSize(&mut ancho, &mut alto)? };
    let orientacion = orientacion_de(&fotograma);
    let (ancho_completo, alto_completo) = medidas_giradas(ancho, alto, orientacion);
    let medida = medida_de_lectura((ancho_completo, alto_completo), caja);
    let fuente = fuente_derecha(&fabrica, &fotograma, orientacion, Some(medida))?;
    let imagen = a_imagen(&fuente)?;
    Ok(Vista {
        imagen,
        ancho_completo,
        alto_completo,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_orientaciones_5_a_8_cambian_ancho_por_alto() {
        for o in 5..=8 {
            assert_eq!(
                medidas_giradas(4032, 3024, o),
                (3024, 4032),
                "orientacion {o}"
            );
        }
    }

    #[test]
    fn caso_negativo_las_orientaciones_1_a_4_y_las_raras_no_tumban() {
        for o in [0, 1, 2, 3, 4, 9, 255] {
            assert_eq!(
                medidas_giradas(4032, 3024, o),
                (4032, 3024),
                "orientacion {o}"
            );
        }
        assert_eq!(transformacion_exif(0), WICBitmapTransformRotate0);
        assert_eq!(transformacion_exif(42), WICBitmapTransformRotate0);
    }

    #[test]
    fn cada_etiqueta_tiene_su_giro() {
        assert_eq!(transformacion_exif(1), WICBitmapTransformRotate0);
        assert_eq!(transformacion_exif(3), WICBitmapTransformRotate180);
        assert_eq!(transformacion_exif(6), WICBitmapTransformRotate90);
        assert_eq!(transformacion_exif(8), WICBitmapTransformRotate270);
        assert_eq!(transformacion_exif(2), WICBitmapTransformFlipHorizontal);
        assert_eq!(transformacion_exif(4), WICBitmapTransformFlipVertical);
    }

    #[test]
    fn se_lee_a_la_medida_de_la_caja_sin_deformar() {
        // Una foto de movil en una pantalla 1080p al 80 %.
        assert_eq!(medida_de_lectura((4032, 3024), (1536, 864)), (1152, 864));
        assert_eq!(medida_de_lectura((3024, 4032), (1536, 864)), (648, 864));
    }

    #[test]
    fn reducir_a_menos_de_la_mitad_promedia_y_poco_es_bilineal() {
        use windows::Win32::Graphics::Imaging::WICBitmapInterpolationModeLinear;
        assert_eq!(
            interpolacion_para((4032, 3024), (1152, 864)),
            WICBitmapInterpolationModeFant
        );
        assert_eq!(
            interpolacion_para((3840, 2160), (2400, 1350)),
            WICBitmapInterpolationModeLinear
        );
        // Caso negativo: justo la mitad todavia es bilineal.
        assert_eq!(
            interpolacion_para((2000, 1000), (1000, 500)),
            WICBitmapInterpolationModeLinear
        );
    }

    #[test]
    fn caso_negativo_lo_que_ya_cabe_no_se_agranda_ni_se_toca() {
        assert_eq!(medida_de_lectura((800, 600), (1536, 864)), (800, 600));
        // Sin caja: entera.
        assert_eq!(medida_de_lectura((4032, 3024), (0, 0)), (4032, 3024));
        // Medidas nulas no dan una imagen de 0 pixeles.
        assert_eq!(medida_de_lectura((0, 0), (100, 100)), (1, 1));
    }

    fn temporal(nombre: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("pixpin-vista");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(nombre)
    }

    #[test]
    fn una_png_se_lee_reducida_a_la_caja_y_sabe_su_medida_entera() {
        let ruta = temporal("reducida.png");
        let img = ImagenRgba {
            ancho: 400,
            alto: 200,
            pixeles: [200u8, 30, 30, 255].repeat(400 * 200),
        };
        crate::guardar(&img, &ruta, crate::FormatoImagen::Png).unwrap();
        let v = cargar_para_ver(&ruta, (100, 100)).unwrap();
        assert_eq!((v.imagen.ancho, v.imagen.alto), (100, 50));
        assert_eq!((v.ancho_completo, v.alto_completo), (400, 200));
        assert!(v.reducida());
        assert_eq!(
            &v.imagen.pixeles[..4],
            &[200, 30, 30, 255],
            "sigue siendo RGBA"
        );
        // Caso negativo: sin caja, entera y sin reducir.
        let entera = cargar_para_ver(&ruta, (0, 0)).unwrap();
        assert!(!entera.reducida());
        assert_eq!(entera.imagen, img);
    }

    /// Cuanto cuesta cada forma de leer (`PIXPIN_MEDIR_FOTOS`, rutas con
    /// `;`). `cargo test -p pixpin-codec medir_lectura -- --ignored
    /// --nocapture`.
    #[test]
    #[ignore = "mide con fotos de verdad: PIXPIN_MEDIR_FOTOS"]
    fn medir_lectura() {
        let fotos: Vec<std::path::PathBuf> = std::env::var("PIXPIN_MEDIR_FOTOS")
            .unwrap_or_default()
            .split(';')
            .filter(|s| !s.is_empty())
            .map(Into::into)
            .collect();
        let mediana = |mut v: Vec<f64>| {
            v.sort_by(f64::total_cmp);
            v[v.len() / 2]
        };
        let medir = |f: &dyn Fn()| {
            mediana(
                (0..7)
                    .map(|_| {
                        let t = std::time::Instant::now();
                        f();
                        t.elapsed().as_secs_f64() * 1000.0
                    })
                    .collect(),
            )
        };
        for ruta in &fotos {
            let nombre = ruta.file_name().unwrap().to_string_lossy();
            let image = medir(&|| {
                crate::cargar(ruta).unwrap();
            });
            let wic_entera = medir(&|| {
                crate::wic::cargar(ruta).unwrap();
            });
            for caja in [(2400, 1600), (1536, 864)] {
                let a_la_medida = medir(&|| {
                    cargar_para_ver(ruta, caja).unwrap();
                });
                let previa = medir(&|| {
                    vista_previa(ruta, caja).unwrap();
                });
                println!(
                    "{nombre} caja {caja:?}: image entera {image:.1} ms | WIC entera \
                     {wic_entera:.1} | a la medida {a_la_medida:.1} | vista previa {previa:.1}"
                );
            }
        }
    }

    /// Una foto de movil con la etiqueta EXIF de girar 90 (la de
    /// `PIXPIN_FOTO_GIRADA`, guardada tumbada) sale derecha por los tres
    /// caminos: entera con `image`, entera con Windows y a la medida.
    #[test]
    #[ignore = "necesita una foto con orientacion EXIF 6: PIXPIN_FOTO_GIRADA"]
    fn una_foto_girada_sale_derecha_por_todos_los_caminos() {
        let ruta = std::path::PathBuf::from(std::env::var("PIXPIN_FOTO_GIRADA").unwrap());
        let guardada = crate::wic::Com::iniciar();
        let (_, fotograma) = crate::wic::primer_fotograma(&ruta).unwrap();
        let (mut w, mut h) = (0, 0);
        // SAFETY: fotograma vivo; salidas locales.
        unsafe { fotograma.GetSize(&mut w, &mut h).unwrap() };
        assert_eq!(orientacion_de(&fotograma), 6);
        drop(fotograma);
        drop(guardada);
        assert!(w > h, "guardada tumbada");
        assert_eq!(crate::imagen::medidas(&ruta).unwrap(), (h, w));
        let entera = crate::cargar(&ruta).unwrap();
        assert_eq!((entera.ancho, entera.alto), (h, w));
        let por_windows = crate::wic::cargar(&ruta).unwrap();
        assert_eq!((por_windows.ancho, por_windows.alto), (h, w));
        let v = cargar_para_ver(&ruta, (800, 800)).unwrap();
        assert_eq!((v.ancho_completo, v.alto_completo), (h, w));
        assert!(
            v.imagen.alto > v.imagen.ancho,
            "derecha: mas alta que ancha"
        );
    }

    #[test]
    fn solo_las_fotos_grandes_llevan_vista_previa() {
        assert!(quiere_vista_previa((4032, 3024)));
        assert!(quiere_vista_previa((2000, 2000)));
    }

    #[test]
    fn caso_negativo_una_imagen_pequena_no_lleva_vista_previa() {
        assert!(!quiere_vista_previa((1920, 1080)));
        assert!(!quiere_vista_previa((879, 1280)));
        let ruta = temporal("pequena-sin-previa.png");
        let img = ImagenRgba {
            ancho: 64,
            alto: 48,
            pixeles: [1u8, 2, 3, 255].repeat(64 * 48),
        };
        crate::guardar(&img, &ruta, crate::FormatoImagen::Png).unwrap();
        assert!(vista_previa(&ruta, (100, 100)).unwrap().is_none());
    }

    #[test]
    fn caso_negativo_una_png_grande_no_lleva_previa_que_costaria_lo_mismo() {
        let ruta = temporal("grande-sin-previa.png");
        let img = ImagenRgba {
            ancho: 2000,
            alto: 2000,
            pixeles: [40u8, 120, 200, 255].repeat(2000 * 2000),
        };
        crate::guardar(&img, &ruta, crate::FormatoImagen::Png).unwrap();
        assert!(vista_previa(&ruta, (1000, 1000)).unwrap().is_none());
    }

    #[test]
    fn una_foto_grande_sin_miniatura_da_una_previa_a_un_cuarto() {
        let ruta = temporal("grande-con-previa.jpg");
        let img = ImagenRgba {
            ancho: 2400,
            alto: 1800,
            pixeles: [40u8, 120, 200, 255].repeat(2400 * 1800),
        };
        crate::guardar(&img, &ruta, crate::FormatoImagen::Jpg).unwrap();
        let v = vista_previa(&ruta, (1200, 900))
            .unwrap()
            .expect("es grande");
        assert_eq!((v.ancho_completo, v.alto_completo), (2400, 1800));
        assert_eq!((v.imagen.ancho, v.imagen.alto), (300, 225));
    }

    #[test]
    fn un_fichero_que_no_es_imagen_da_error() {
        let ruta = temporal("no-es.png");
        std::fs::write(&ruta, b"hola").unwrap();
        assert!(cargar_para_ver(&ruta, (100, 100)).is_err());
    }
}
