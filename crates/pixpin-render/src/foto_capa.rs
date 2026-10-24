//! **La foto de una capa de dibujo, para moverla sin repintarla** (13-oct-2026,
//! el usuario: «en los CAD con anotaciones el zoom va lagueado y no es muy
//! rapido»).
//!
//! Pintar lo anotado cuesta lo que cuestan sus trazos: cada realizacion es
//! una llamada de Direct2D, y medido en el visor de planos
//! (`plano_cad::anotar::rendimiento`, i7-10510U + MX250) 378 trazos a la
//! vista eran 7 a 12 ms por fotograma aunque no se teselara nada, mas que
//! el plano entero (2,5 ms). Mientras la camara se mueve la capa no cambia:
//! basta pintarla una vez a un mapa de bits transparente y, en cada
//! fotograma, ponerlo con la camara de ahora (una sola llamada). Al pararse
//! se vuelve a pintar de verdad, nitida.
//!
//! A diferencia de [`crate::CapaEstatica`] (pegada a la pantalla, solo vale
//! con la camara exacta), la foto sabe en que punto del documento y a que
//! aumento se hizo, asi que vale estirada (acercando), encogida (alejando) o
//! corrida (arrastrando). Lo que la foto no tiene (lo que quedaba fuera de
//! la ventana al hacerla) lo pinta quien llama, vivo: ver
//! [`FotoDeCapa::la_tiene`].
//!
//! Que la foto sigue siendo de lo que hay lo dice la `huella` que da quien
//! llama (las versiones de lo dibujado, el papel...): con otra, no vale.
//!
//! Ocupa un mapa del tamano de la ventana (1200 x 800: 3,7 MB), uno solo.

use windows::Win32::Graphics::Direct2D::Common::{
    D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1, ID2D1Bitmap1,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;

use crate::lienzo::{Interpolacion, Pintor, RectF};
use crate::motor::{ErrorRender, MotorRender};

/// Una camara de documento: el punto del documento en la esquina de arriba
/// a la izquierda y los pixeles por unidad del documento.
pub type CamaraDoc = (f32, f32, f32);

/// La foto de una capa (ver el modulo).
pub struct FotoDeCapa {
    bitmap: Option<ID2D1Bitmap1>,
    tamano: (u32, u32),
    camara: CamaraDoc,
    huella: u64,
    /// Si se hizo con todo a su escala (sin nada pendiente de teselar): solo
    /// entonces vale tal cual, en reposo, en vez de volver a pintar.
    nitida: bool,
}

impl FotoDeCapa {
    pub fn nueva() -> Self {
        Self {
            bitmap: None,
            tamano: (0, 0),
            camara: (0.0, 0.0, 1.0),
            huella: 0,
            nitida: false,
        }
    }

    /// Si hay foto de esta `huella` y para una ventana de `tamano`.
    pub fn vale(&self, huella: u64, tamano: (u32, u32)) -> bool {
        self.bitmap.is_some() && self.huella == huella && self.tamano == tamano
    }

    /// Si ademas se hizo nitida con esta misma camara: entonces ponerla es
    /// lo mismo, pixel a pixel, que volver a pintar la capa.
    pub fn vale_tal_cual(&self, huella: u64, tamano: (u32, u32), camara: CamaraDoc) -> bool {
        self.vale(huella, tamano) && self.nitida && self.camara == camara
    }

    /// La camara con que se hizo.
    pub fn camara(&self) -> CamaraDoc {
        self.camara
    }

    /// Lo que cubre, en unidades del documento: `(x0, y0, x1, y1)`.
    pub fn cubre(&self) -> (f32, f32, f32, f32) {
        let (x, y, z) = self.camara;
        let z = z.max(1e-12);
        (
            x,
            y,
            x + self.tamano.0 as f32 / z,
            y + self.tamano.1 as f32 / z,
        )
    }

    /// Si la foto tiene entera una caja del documento `(x0, y0, x1, y1)`: lo
    /// que no, lo tiene que pintar quien llama encima, vivo.
    pub fn la_tiene(&self, caja: (f32, f32, f32, f32)) -> bool {
        let (x0, y0, x1, y1) = self.cubre();
        caja.0 >= x0 && caja.1 >= y0 && caja.2 <= x1 && caja.3 <= y1
    }

    /// **Hace la foto**: borra el mapa (transparente) y pinta en el con
    /// `pintar`, que pone su propia vista (la de `camara`). `nitida` dice si
    /// se pinto todo a su escala. Reusa el mapa si la ventana no cambio.
    pub fn hacer(
        &mut self,
        motor: &MotorRender,
        tamano: (u32, u32),
        camara: CamaraDoc,
        huella: u64,
        pintar: impl FnOnce(&Pintor),
    ) -> Result<(), ErrorRender> {
        if self.bitmap.is_none() || self.tamano != tamano {
            self.bitmap = None;
            let propiedades = D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    // Destino de dibujo: premultiplicado, como los demas.
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
                colorContext: std::mem::ManuallyDrop::new(None),
            };
            // SAFETY: sin datos iniciales, D2D solo reserva; tamano y formato
            // coherentes; el contexto lo mantiene vivo `motor`.
            let b = unsafe {
                motor.contexto().CreateBitmap(
                    D2D_SIZE_U {
                        width: tamano.0.max(1),
                        height: tamano.1.max(1),
                    },
                    None,
                    0,
                    &propiedades,
                )?
            };
            motor.conto(|c| c.bitmaps += 1);
            self.bitmap = Some(b);
            self.tamano = tamano;
        }
        let Some(b) = self.bitmap.clone() else {
            return Ok(());
        };
        // Mientras se pinta no vale (si falla a medias, no se usa).
        self.huella = huella.wrapping_add(1);
        motor.dibujar(&b, |p| {
            p.limpiar_transparente();
            pintar(p);
        })?;
        self.camara = camara;
        self.huella = huella;
        self.nitida = true;
        Ok(())
    }

    /// Apunta que lo pintado en la ultima foto no estaba todo a su escala.
    pub fn no_nitida(&mut self) {
        self.nitida = false;
    }

    /// **Pone la foto** donde cae con la camara `ahora`, en pixeles de la
    /// ventana (quien llama tiene la transformada en la identidad). Con la
    /// misma camara es una copia pixel a pixel; si no, estirada con
    /// interpolacion lineal.
    pub fn poner(&self, p: &Pintor, ahora: CamaraDoc) {
        let Some(b) = &self.bitmap else { return };
        let destino = destino_de(self.camara, ahora, self.tamano);
        let modo = if self.camara == ahora {
            Interpolacion::Vecino
        } else {
            Interpolacion::Lineal
        };
        p.bitmap_con(b, destino, None, modo);
    }

    /// Tira el mapa (otro dispositivo, o nada que pintar).
    pub fn soltar(&mut self) {
        self.bitmap = None;
        self.nitida = false;
    }

    /// Lo que ocupa el mapa ahora, en bytes.
    pub fn bytes(&self) -> usize {
        if self.bitmap.is_some() {
            self.tamano.0 as usize * self.tamano.1 as usize * 4
        } else {
            0
        }
    }
}

impl Default for FotoDeCapa {
    fn default() -> Self {
        Self::nueva()
    }
}

/// Donde cae en la ventana una foto hecha con la camara `foto` y de
/// `tamano` pixeles, vista con la camara `ahora`. Pura, para probarla sin
/// GPU: el pixel `u` de la foto es el punto `x0 + u / z0` del documento, que
/// en la ventana cae en `(x0 + u / z0 - x1) * z1`.
pub fn destino_de(foto: CamaraDoc, ahora: CamaraDoc, tamano: (u32, u32)) -> RectF {
    let (x0, y0, z0) = foto;
    let (x1, y1, z1) = ahora;
    let k = z1 / z0.max(1e-12);
    RectF {
        x: (x0 - x1) * z1,
        y: (y0 - y1) * z1,
        ancho: tamano.0 as f32 * k,
        alto: tamano.1 as f32 * k,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn con_la_misma_camara_cae_entera_en_la_ventana() {
        let r = destino_de((10.0, 20.0, 2.0), (10.0, 20.0, 2.0), (800, 600));
        assert_eq!((r.x, r.y, r.ancho, r.alto), (0.0, 0.0, 800.0, 600.0));
    }

    #[test]
    fn acercando_al_doble_el_mismo_punto_cae_en_el_mismo_pixel() {
        // Un punto del documento: su pixel en la foto y en la ventana.
        let foto = (100.0, 50.0, 0.5);
        let ahora = (180.0, 90.0, 1.0);
        let r = destino_de(foto, ahora, (800, 600));
        let punto = (300.0f32, 200.0f32);
        let en_foto = ((punto.0 - foto.0) * foto.2, (punto.1 - foto.1) * foto.2);
        let por_la_foto = (
            r.x + en_foto.0 * r.ancho / 800.0,
            r.y + en_foto.1 * r.alto / 600.0,
        );
        let directo = ((punto.0 - ahora.0) * ahora.2, (punto.1 - ahora.1) * ahora.2);
        assert!(
            (por_la_foto.0 - directo.0).abs() < 1e-3 && (por_la_foto.1 - directo.1).abs() < 1e-3
        );
        assert!((r.ancho - 1600.0).abs() < 1e-3);
    }

    #[test]
    fn sin_foto_no_vale_nada_y_tiene_lo_que_cubre() {
        let mut f = FotoDeCapa::nueva();
        assert!(!f.vale(0, (0, 0)));
        assert!(!f.vale_tal_cual(0, (0, 0), (0.0, 0.0, 1.0)));
        f.tamano = (100, 50);
        f.camara = (10.0, 10.0, 2.0);
        // Cubre de 10 a 60 en x y de 10 a 35 en y.
        assert!(f.la_tiene((11.0, 11.0, 59.0, 34.0)));
        // Casos negativos: lo que se sale por cualquier lado.
        assert!(!f.la_tiene((9.0, 11.0, 20.0, 20.0)));
        assert!(!f.la_tiene((11.0, 11.0, 61.0, 20.0)));
        assert!(!f.la_tiene((11.0, 11.0, 20.0, 36.0)));
    }
}
