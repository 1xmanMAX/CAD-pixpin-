//! **Las imagenes del plano en pantalla** (IMAGEN, IMAGEATTACH): el visor de
//! PixPin no las dibuja, asi que van aqui, con Direct2D, debajo de lo que se
//! pinta encima del plano. Cada fichero se lee una vez; cada mapa de bits se
//! hace una vez por motor de dibujo.

use std::collections::HashMap;
use std::path::PathBuf;

use cad_nucleo::formatos::imagen::ImagenEnPlano;
use cad_nucleo::geo::P;
use pixpin_render::{MotorRender, Pintor, RectF};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

/// Lado mayor con que se sube una imagen a la tarjeta (las mas grandes se
/// reducen: para ver un plano escaneado basta).
const LADO_MAXIMO: u32 = 4096;

#[derive(Default)]
pub struct Imagenes {
    /// Los ficheros ya leidos (`None`: no se pudo).
    leidas: HashMap<PathBuf, Option<pixpin_codec::ImagenRgba>>,
    /// Los mapas de bits, del motor de clave `motor`.
    bitmaps: HashMap<PathBuf, ID2D1Bitmap1>,
    motor: usize,
}

impl Imagenes {
    /// **Pinta las imagenes** que se ven. `a_pantalla` lleva un punto del
    /// dibujo a la ventana. Las giradas (no a escuadra) se quedan en su marco.
    pub fn pintar(
        &mut self,
        p: &Pintor,
        motor: &MotorRender,
        lista: &[ImagenEnPlano],
        a_pantalla: &impl Fn(P) -> (f32, f32),
    ) {
        let clave = motor as *const MotorRender as usize;
        if clave != self.motor {
            self.bitmaps.clear();
            self.motor = clave;
        }
        for im in lista {
            if im.ancho[1].abs() > 1e-9 * im.ancho[0].abs().max(1.0)
                || im.alto[0].abs() > 1e-9 * im.alto[1].abs().max(1.0)
            {
                continue;
            }
            let Some(b) = self.bitmap(motor, &im.ruta) else {
                continue;
            };
            let a = a_pantalla(im.origen);
            let c = a_pantalla([im.origen[0] + im.ancho[0], im.origen[1] + im.alto[1]]);
            let destino = RectF {
                x: a.0.min(c.0),
                y: a.1.min(c.1),
                ancho: (c.0 - a.0).abs(),
                alto: (c.1 - a.1).abs(),
            };
            if destino.ancho < 1.0 || destino.alto < 1.0 {
                continue;
            }
            p.bitmap(&b, destino, None, false);
        }
    }

    fn bitmap(&mut self, motor: &MotorRender, ruta: &PathBuf) -> Option<ID2D1Bitmap1> {
        if let Some(b) = self.bitmaps.get(ruta) {
            return Some(b.clone());
        }
        let img = self
            .leidas
            .entry(ruta.clone())
            .or_insert_with(|| leer(ruta))
            .as_ref()?;
        let b = motor
            .bitmap_desde_pixeles(img.ancho, img.alto, &img.pixeles)
            .ok()?;
        self.bitmaps.insert(ruta.clone(), b.clone());
        Some(b)
    }
}

fn leer(ruta: &std::path::Path) -> Option<pixpin_codec::ImagenRgba> {
    let img = pixpin_codec::imagen::cargar(ruta)
        .map_err(|e| tracing::warn!(?e, ruta = %ruta.display(), "imagen del plano sin leer"))
        .ok()?;
    let lado = img.ancho.max(img.alto);
    if lado <= LADO_MAXIMO {
        return Some(img);
    }
    let k = LADO_MAXIMO as f64 / lado as f64;
    let (w, h) = (
        ((img.ancho as f64 * k).round() as u32).max(1),
        ((img.alto as f64 * k).round() as u32).max(1),
    );
    pixpin_codec::imagen::redimensionar(img, w, h).ok()
}
