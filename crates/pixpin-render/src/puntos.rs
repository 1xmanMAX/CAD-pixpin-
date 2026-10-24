//! Miles de puntos pequenos (las estrellas del universo) en un lote de
//! sprites que se sube UNA vez.
//!
//! Pintarlos como baldosas de bitmap repetidas cuesta una pasada de
//! pantalla entera por capa aunque casi todo sea transparente: medido a
//! 3000 x 2000, 2,5-3 ms de GPU por capa en una grafica integrada. Como
//! geometria teselada (`ID2D1GeometryRealization`) fue peor: 4 ms de CPU y
//! 5 de GPU por capa, por los bordes suavizados de cada cuadradito. Un lote
//! de sprites (`ID2D1SpriteBatch`, Direct2D 1.3, Windows 10) es lo que
//! Direct2D tiene para esto: los rectangulos viven en la GPU, cada fotograma
//! es una llamada con la transformada corrida, y solo se tocan los pixeles
//! de las estrellas.

use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_ALIASED, D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
    D2D1_BITMAP_OPTIONS_NONE, D2D1_BITMAP_PROPERTIES1, D2D1_SPRITE_OPTIONS_NONE, ID2D1Bitmap1,
    ID2D1DeviceContext3, ID2D1SpriteBatch,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::core::Interface;
use windows_numerics::Matrix3x2;

use crate::lienzo::Pintor;
use crate::motor::{ErrorRender, MotorRender};

/// Un punto: su esquina, su lado, su color y su opacidad (0..1).
///
/// El color va por punto y no por lote porque las estrellas del movil son de
/// tres colores (blanco, azul palido y crema calido) y los tres van mezclados
/// en la misma capa: un lote por color seria una llamada mas por fotograma
/// para nada, porque el sprite ya lleva su color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Punto {
    pub x: f32,
    pub y: f32,
    pub lado: f32,
    pub alfa: f32,
    /// `(r, g, b)` de 0 a 1, sin premultiplicar.
    pub rgb: (f32, f32, f32),
}

/// Los puntos ya subidos a la GPU. Son del dispositivo: tras perderlo hay
/// que volver a crearlos.
pub struct Puntos {
    lote: ID2D1SpriteBatch,
    /// Un pixel blanco: cada sprite lo estira a su cuadrado y lo tine con
    /// su opacidad.
    blanco: ID2D1Bitmap1,
    cuantos: u32,
}

/// Los rectangulos y colores de los sprites, sin los puntos que no se ven.
/// El color va premultiplicado, como todo en Direct2D: blanco al `alfa`.
pub fn sprites_de(puntos: &[Punto]) -> (Vec<D2D_RECT_F>, Vec<D2D1_COLOR_F>) {
    puntos
        .iter()
        .filter(|p| p.alfa > 0.0 && p.lado > 0.0)
        .map(|p| {
            let a = p.alfa.clamp(0.0, 1.0);
            (
                D2D_RECT_F {
                    left: p.x,
                    top: p.y,
                    right: p.x + p.lado,
                    bottom: p.y + p.lado,
                },
                D2D1_COLOR_F {
                    r: p.rgb.0.clamp(0.0, 1.0) * a,
                    g: p.rgb.1.clamp(0.0, 1.0) * a,
                    b: p.rgb.2.clamp(0.0, 1.0) * a,
                    a,
                },
            )
        })
        .unzip()
}

impl MotorRender {
    /// Sube `puntos` para pintarlos muchas veces. Falla sin Direct2D 1.3
    /// (anterior a Windows 10): quien llama se queda sin ellos.
    pub fn realizar_puntos(&self, puntos: &[Punto]) -> Result<Puntos, ErrorRender> {
        let ctx3: ID2D1DeviceContext3 = self.contexto().cast()?;
        let (rects, colores) = sprites_de(puntos);
        let propiedades = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_NONE,
            colorContext: std::mem::ManuallyDrop::new(None),
        };
        let pixel = [255u8; 4];
        // SAFETY: el pixel vive durante la llamada y D2D lo copia; los dos
        // vectores tienen `rects.len()` elementos y los pasos son su tamano.
        unsafe {
            let blanco = ctx3.CreateBitmap(
                D2D_SIZE_U {
                    width: 1,
                    height: 1,
                },
                Some(pixel.as_ptr() as *const _),
                4,
                &propiedades,
            )?;
            let lote = ctx3.CreateSpriteBatch()?;
            if !rects.is_empty() {
                lote.AddSprites(
                    rects.len() as u32,
                    rects.as_ptr(),
                    None,
                    Some(colores.as_ptr()),
                    None,
                    std::mem::size_of::<D2D_RECT_F>() as u32,
                    0,
                    std::mem::size_of::<D2D1_COLOR_F>() as u32,
                    0,
                )?;
            }
            Ok(Puntos {
                lote,
                blanco,
                cuantos: rects.len() as u32,
            })
        }
    }
}

impl Pintor<'_> {
    /// Pinta `p` corrido `desplazamiento` pixeles. Deja la transformada y
    /// el suavizado como estaban (los sprites exigen el suavizado apagado).
    pub fn puntos(&self, p: &Puntos, desplazamiento: (f32, f32)) {
        if p.cuantos == 0 {
            return;
        }
        let Ok(ctx3) = self.motor.contexto().cast::<ID2D1DeviceContext3>() else {
            return;
        };
        let mut antes = Matrix3x2::default();
        // SAFETY: dentro del fotograma; se lee el estado para devolverlo
        // igual justo despues del dibujo.
        unsafe {
            ctx3.GetTransform(&mut antes);
            let suavizado = ctx3.GetAntialiasMode();
            ctx3.SetTransform(&Matrix3x2 {
                M31: antes.M31 + desplazamiento.0,
                M32: antes.M32 + desplazamiento.1,
                ..antes
            });
            ctx3.SetAntialiasMode(D2D1_ANTIALIAS_MODE_ALIASED);
            ctx3.DrawSpriteBatch(
                &p.lote,
                0,
                p.cuantos,
                &p.blanco,
                D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
                D2D1_SPRITE_OPTIONS_NONE,
            );
            ctx3.SetAntialiasMode(suavizado);
            ctx3.SetTransform(&antes);
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn cada_punto_visible_es_un_sprite_de_su_color_premultiplicado_por_su_opacidad() {
        let p = |alfa, lado| Punto {
            x: 10.0,
            y: 20.0,
            lado,
            alfa,
            rgb: (1.0, 1.0, 1.0),
        };
        let (rects, colores) = sprites_de(&[p(0.4, 2.0), p(0.0, 1.0), p(0.5, 0.0)]);
        assert_eq!(rects.len(), 1, "los invisibles se quedan fuera");
        assert_eq!(
            (rects[0].left, rects[0].top, rects[0].right, rects[0].bottom),
            (10.0, 20.0, 12.0, 22.0)
        );
        assert_eq!((colores[0].r, colores[0].a), (0.4, 0.4), "premultiplicado");
        // Caso negativo: sin puntos, nada.
        assert!(sprites_de(&[]).0.is_empty());
    }

    #[test]
    fn un_punto_de_color_sale_tenido_y_premultiplicado_y_no_blanco() {
        let (_, colores) = sprites_de(&[Punto {
            x: 0.0,
            y: 0.0,
            lado: 1.0,
            alfa: 0.5,
            rgb: (1.0, 0.8, 0.0),
        }]);
        assert_eq!(
            (colores[0].r, colores[0].g, colores[0].b, colores[0].a),
            (0.5, 0.4, 0.0, 0.5)
        );
    }
}
