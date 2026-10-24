//! **El grano de una tinta, pintado: una tela clavada en el dibujo que el
//! trazo destapa.**
//!
//! La otra mitad del material de tinta (`pixpin_motor2d::tinta::material`, el
//! puerto del `MaterialDeTinta` del movil). Alli se teje el cuadro —puro,
//! 16x16 bytes de cobertura, sin GPU—; aqui se sube a una brocha que se
//! repite y se rellena con ella la silueta del trazo.
//!
//! ## Un solo relleno, no cien rayas
//!
//! Rayar a mano —recortar al contorno y pintar raya por raya— son decenas de
//! ordenes por figura y por fotograma. Con la brocha de mosaico es **una
//! orden**, exactamente la misma que pintar de un color liso: la tela ya
//! lleva dentro las rayas, los puntos o las motas.
//!
//! ## Clavada al dibujo, no a la figura
//!
//! La brocha se ancla al sistema de coordenadas en el que se pinta, o sea al
//! **documento**: la trama esta quieta en el papel y lo que el trazo hace es
//! destaparla, que es lo que hace un rayado en un plano de verdad. Si cada
//! trazo empezara su rayado en la esquina de su propia caja, dos secciones
//! pegadas ensenarian dos tramas descolocadas y mover una se llevaria su
//! plano con ella.

use std::collections::HashMap;

use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_BRUSH_PROPERTIES1, D2D1_BRUSH_PROPERTIES, D2D1_EXTEND_MODE_WRAP,
    D2D1_INTERPOLATION_MODE_LINEAR, ID2D1Bitmap1, ID2D1BitmapBrush1, ID2D1PathGeometry1,
};
use windows_numerics::Matrix3x2;

use crate::motor::Color;

/// Lo que se inclina la brocha del rayado, en grados.
///
/// Repetido a proposito: `pixpin-render` **no depende de `pixpin-motor2d`**
/// —es la frontera de dibujo y no sabe de elementos—, asi que el numero vive
/// aqui y alli (`tinta::material::GRADOS_DEL_GRANO`). Son cuarenta y cinco en
/// los dos sitios; quien cambie uno tiene que cambiar el otro, y lo que lo
/// delataria es la muestra de PNG del grano, donde las rayas dejarian de ir
/// en diagonal.
pub const GRADOS_DEL_GRANO: f32 = 45.0;

/// Cuantas telas se guardan a la vez.
///
/// Cada una son 16x16 pixeles, un kilobyte. Hay una por material **y color**
/// —el color va tenido dentro, que es lo que permite que pintar sea un solo
/// relleno— y una hoja normal no usa mas de un punado; el tope existe solo
/// para que nadie pueda llenar la memoria de video cambiando de color mil
/// veces.
const MAX_TELAS: usize = 96;

/// Cuantas siluetas de trazo se guardan a la vez. Generoso como el de
/// `CacheTinta`: echar lo que el siguiente fotograma va a volver a pedir
/// seria peor que no guardar nada.
const MAX_SILUETAS: usize = 8_000;

/// Las telas ya subidas a la GPU, por material y color.
///
/// Es del dispositivo: tras perderlo hay que vaciarla, igual que
/// `CacheTinta`.
#[derive(Default)]
pub struct CacheGrano {
    telas: HashMap<(u32, u32), ID2D1Bitmap1>,
    /// La silueta de cada trazo, por elemento y version.
    ///
    /// El cuerpo del trazo se pinta con `tinta_cacheada`, que guarda la
    /// geometria **ya teselada** y no hace falta volver a armarla. El grano
    /// no puede usar esa: una realizacion solo se pinta con un pincel de
    /// color, y aqui el pincel es un mosaico. Asi que el grano necesita la
    /// geometria de verdad — y sin guardarla, cada fotograma la arma otra
    /// vez, que en un contorno de mil vertices son mil llamadas COM.
    siluetas: HashMap<(u64, u32), ID2D1PathGeometry1>,
    /// **Los mapas del grafito** (`crate::grafito`): el otro material que no
    /// es un color liso. Vive aqui por lo mismo que las telas: va a los mismos
    /// sitios que la silueta y se pierde con el mismo dispositivo.
    pub grafito: crate::grafito::CacheGrafito,
}

impl CacheGrano {
    pub fn nueva() -> Self {
        Self {
            telas: HashMap::new(),
            siluetas: HashMap::new(),
            grafito: crate::grafito::CacheGrafito::nueva(),
        }
    }

    /// Dispositivo perdido: las telas son del dispositivo viejo y no valen.
    pub fn vaciar(&mut self) {
        self.telas.clear();
        self.siluetas.clear();
        self.grafito.vaciar();
    }

    /// Cuantas siluetas hay guardadas: para las pruebas.
    pub fn cuantas_siluetas(&self) -> usize {
        self.siluetas.len()
    }

    /// Cuantas telas hay tejidas: para las pruebas.
    pub fn cuantas(&self) -> usize {
        self.telas.len()
    }
}

/// El color empaquetado en cuatro bytes, para la llave de la cache.
fn llave_de_color(c: Color) -> u32 {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    (b(c.a) << 24) | (b(c.r) << 16) | (b(c.g) << 8) | b(c.b)
}

impl crate::lienzo::Pintor<'_> {
    /// **Estampa la tela de un material dentro de `contorno`.**
    ///
    /// `contorno` es la silueta del trazo, la misma que rellena
    /// `Pintor::tinta`: la tela se recorta a ella, asi que el grano no se sale
    /// por ningun lado por retorcida que sea la figura —que es lo que permite
    /// rayar una seccion hecha a pulso—.
    ///
    /// `tejido` son `lado * lado` bytes de cobertura
    /// (`pixpin_motor2d::tinta::tejido`), `id_material` distingue una tela de
    /// otra en la cache, `paso` es cada cuantos pixeles se repite el cuadro y
    /// `inclinada` gira la brocha 45 grados.
    ///
    /// No hace nada si el tejido esta vacio o si el contorno no llega a
    /// figura: una tinta lisa no tiene tela que estampar, y estampar sobre
    /// dos puntos no pinta nada.
    #[allow(clippy::too_many_arguments)]
    pub fn grano(
        &self,
        cache: &mut CacheGrano,
        clave: (u64, u32),
        contorno: &[(f32, f32)],
        tejido: &[u8],
        lado: u32,
        id_material: u32,
        color: Color,
        paso: f32,
        inclinada: bool,
    ) {
        if contorno.len() < 3 || lado == 0 || tejido.len() != (lado * lado) as usize {
            return;
        }
        // Una tela que no tapa nada —la lisa, las encendidas— no se sube ni
        // se pinta: seria un relleno entero de nada.
        if tejido.iter().all(|b| *b == 0) {
            return;
        }
        if !(paso.is_finite() && paso > 0.0) {
            return;
        }
        let Some(brocha) = self.brocha_de_grano(cache, tejido, lado, id_material, color) else {
            return;
        };
        // La matriz dice de que tamano sale el cuadro y, en el rayado, de
        // traves: **la brocha se gira, no el dibujo**, asi que las juntas
        // siguen casando.
        let cuanto = paso / lado as f32;
        let mut m = Matrix3x2 {
            M11: cuanto,
            M12: 0.0,
            M21: 0.0,
            M22: cuanto,
            M31: 0.0,
            M32: 0.0,
        };
        if inclinada {
            let radianes = GRADOS_DEL_GRANO.to_radians();
            let (sen, cos) = radianes.sin_cos();
            m = Matrix3x2 {
                M11: cuanto * cos,
                M12: cuanto * sen,
                M21: -cuanto * sen,
                M22: cuanto * cos,
                M31: 0.0,
                M32: 0.0,
            };
        }
        let geometria = match cache.siluetas.get(&clave) {
            Some(g) => g.clone(),
            None => {
                let Some(g) = self.geometria_tinta(contorno) else {
                    return;
                };
                // El tope existe por la misma razon que el de las telas: un
                // documento enorme no puede comerse la memoria. Al pasarse se
                // vacia entero; lo que siga haciendo falta vuelve en un
                // fotograma.
                if cache.siluetas.len() >= MAX_SILUETAS {
                    cache.siluetas.clear();
                }
                // Una version nueva del mismo elemento sustituye a la vieja:
                // la llave lleva la version dentro, asi que la anterior se
                // queda huerfana hasta la proxima limpieza. Se borra aqui
                // para que redibujar un trazo no vaya dejando siluetas.
                cache.siluetas.retain(|(id, _), _| *id != clave.0);
                cache.siluetas.insert(clave, g.clone());
                g
            }
        };
        // SAFETY: dentro del fotograma; brocha y geometria vivas hasta el
        // final de la funcion.
        unsafe {
            brocha.SetTransform(&m);
            self.motor
                .contexto()
                .FillGeometry(&geometria, &brocha, None);
        }
    }

    /// La brocha de mosaico de `(material, color)`, tejiendo la tela si es la
    /// primera vez.
    fn brocha_de_grano(
        &self,
        cache: &mut CacheGrano,
        tejido: &[u8],
        lado: u32,
        id_material: u32,
        color: Color,
    ) -> Option<ID2D1BitmapBrush1> {
        let llave = (id_material, llave_de_color(color));
        let bitmap = match cache.telas.get(&llave) {
            Some(b) => b.clone(),
            None => {
                // El color va **tenido dentro de la tela**: asi pintar el
                // grano es un solo relleno con una brocha, sin una segunda
                // pasada que tina lo estampado.
                let mut rgba = Vec::with_capacity((lado * lado * 4) as usize);
                let canal = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
                let (r, g, b) = (canal(color.r), canal(color.g), canal(color.b));
                let alfa = color.a.clamp(0.0, 1.0);
                for tapa in tejido {
                    rgba.extend_from_slice(&[r, g, b, (*tapa as f32 * alfa).round() as u8]);
                }
                let nueva = self
                    .motor
                    .bitmap_desde_pixeles_premultiplicado(lado, lado, &rgba)
                    .ok()?;
                if cache.telas.len() >= MAX_TELAS {
                    cache.telas.clear();
                }
                cache.telas.insert(llave, nueva.clone());
                nueva
            }
        };
        let propiedades = D2D1_BITMAP_BRUSH_PROPERTIES1 {
            extendModeX: D2D1_EXTEND_MODE_WRAP,
            extendModeY: D2D1_EXTEND_MODE_WRAP,
            // Lineal y no vecino: al alejarse, el cuadro se encoge por debajo
            // del pixel y con el vecino la trama parpadearia al desplazarse.
            interpolationMode: D2D1_INTERPOLATION_MODE_LINEAR,
        };
        // SAFETY: bitmap vivo y contexto vivo; la brocha sale o no sale.
        unsafe {
            self.motor
                .contexto()
                .CreateBitmapBrush(
                    &bitmap,
                    Some(&propiedades),
                    Some(&D2D1_BRUSH_PROPERTIES {
                        opacity: 1.0,
                        transform: Matrix3x2::identity(),
                    }),
                )
                .ok()
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn una_cache_nueva_no_tiene_ninguna_tela() {
        let c = CacheGrano::nueva();
        assert_eq!(c.cuantas(), 0);
    }

    #[test]
    fn vaciar_tira_lo_guardado() {
        let mut c = CacheGrano::nueva();
        c.vaciar();
        assert_eq!(c.cuantas(), 0);
    }

    #[test]
    fn dos_colores_distintos_no_comparten_llave() {
        // Es lo que evita que un rayado rojo salga con la tela del azul: el
        // color va tenido dentro de la tela, asi que forma parte de la llave.
        let rojo = Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let azul = Color {
            r: 0.0,
            g: 0.0,
            b: 1.0,
            a: 1.0,
        };
        assert_ne!(llave_de_color(rojo), llave_de_color(azul));
        // Y el mismo color da la misma llave, o no se reaprovecharia nunca.
        assert_eq!(llave_de_color(rojo), llave_de_color(rojo));
    }

    #[test]
    fn el_color_fuera_de_rango_no_desborda_la_llave() {
        // Caso negativo: un color mal calculado no puede hacer que la llave
        // se salga de sus cuatro bytes.
        let raro = Color {
            r: 5.0,
            g: -3.0,
            b: f32::NAN,
            a: 2.0,
        };
        let l = llave_de_color(raro);
        assert_eq!(l >> 24, 255, "el alfa se acota arriba");
        assert_eq!((l >> 8) & 0xff, 0, "el verde se acota abajo");
    }
}
