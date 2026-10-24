//! Filtros de imagen de un pin: gris, invertir y brillo.
//!
//! Sirven para leer mejor lo que tienes delante: quitar el color de una
//! captura llena de resaltados, invertir un documento en negativo, o subir
//! el brillo de una foto oscura.
//!
//! **Nunca tocan el original.** Se aplican al construir lo que se ve, no a
//! lo que hay guardado, igual que el giro. Asi «restaurar» es de verdad
//! restaurar y no un intento de deshacer, y una captura filtrada y guardada
//! sigue teniendo sus pixeles de siempre en el almacen.
//!
//! Y se aplican SIEMPRE desde el original, no encima de lo ya filtrado:
//! subir y bajar el brillo diez veces tiene que devolver la imagen exacta
//! con la que se empezo. Encadenando pasadas, cada una redondea y la imagen
//! se degrada hasta quedar irreconocible.

use crate::ImagenRgba;

/// Lo que se le hace a la imagen antes de ensenarla.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Filtros {
    pub gris: bool,
    pub invertido: bool,
    /// Pasos de brillo, cada uno un 10 % del recorrido completo. Negativo
    /// oscurece.
    pub brillo: i32,
}

/// Lo que sube o baja cada paso de brillo: un 10 % de 255.
///
/// Se SUMA y no se multiplica. Multiplicar deja el negro en negro para
/// siempre —cero por lo que sea es cero—, y una captura de un terminal
/// oscuro no se aclararia nunca, que es justo el caso en el que hace falta.
const PASO_BRILLO: i32 = 26;

/// Cuantos pasos de brillo se admiten en cada sentido.
///
/// Diez llevan de negro a blanco: mas alla no queda imagen que ensenar, y
/// dejar seguir pulsando solo daria la impresion de que el ajuste se ha
/// roto.
pub const PASOS_MAXIMOS: i32 = 10;

impl Filtros {
    /// Si no hay nada que hacer. Sirve para ahorrarse la pasada entera.
    pub fn son_neutros(&self) -> bool {
        !self.gris && !self.invertido && self.brillo == 0
    }

    /// Sube o baja el brillo, sin salirse de los topes.
    pub fn con_brillo(self, pasos: i32) -> Filtros {
        Filtros {
            brillo: (self.brillo + pasos).clamp(-PASOS_MAXIMOS, PASOS_MAXIMOS),
            ..self
        }
    }
}

/// El valor de un canal tras aplicar los filtros.
///
/// El orden importa y es este: primero gris, luego invertir, y el brillo al
/// final. Invertir despues del brillo daria lo contrario de lo que se
/// espera —subir el brillo oscureceria— porque la inversion le da la vuelta
/// a lo que acabas de sumar.
fn canal(valor: u8, luma: u8, f: Filtros) -> u8 {
    let mut v = if f.gris { luma as i32 } else { valor as i32 };
    if f.invertido {
        v = 255 - v;
    }
    v += f.brillo * PASO_BRILLO;
    v.clamp(0, 255) as u8
}

/// La luminancia de un pixel, para la escala de grises.
///
/// Con los pesos de la percepcion (Rec. 601) y no con la media de los tres
/// canales: el ojo ve el verde mucho mas claro que el azul, y promediar a
/// partes iguales deja los verdes turbios y los azules demasiado claros.
fn luminancia(r: u8, g: u8, b: u8) -> u8 {
    ((r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000).min(255) as u8
}

/// Aplica los filtros a una copia de la imagen.
///
/// El alfa se deja intacto: filtrar la transparencia convertiria los bordes
/// suaves de un recorte en un halo.
pub fn aplicar(imagen: &ImagenRgba, f: Filtros) -> ImagenRgba {
    let mut fuera = imagen.clone();
    if f.son_neutros() {
        return fuera;
    }
    for p in fuera.pixeles.chunks_exact_mut(4) {
        let luma = luminancia(p[0], p[1], p[2]);
        p[0] = canal(p[0], luma, f);
        p[1] = canal(p[1], luma, f);
        p[2] = canal(p[2], luma, f);
    }
    fuera
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn imagen(pixeles: &[u8]) -> ImagenRgba {
        ImagenRgba {
            ancho: (pixeles.len() / 4) as u32,
            alto: 1,
            pixeles: pixeles.to_vec(),
        }
    }

    #[test]
    fn sin_filtros_la_imagen_sale_igual() {
        let original = imagen(&[10, 20, 30, 40, 200, 100, 50, 255]);
        let salida = aplicar(&original, Filtros::default());
        assert_eq!(salida.pixeles, original.pixeles);
    }

    #[test]
    fn el_gris_usa_los_pesos_del_ojo_y_no_la_media() {
        // Un verde puro se ve MUCHO mas claro que un azul puro. Promediando
        // los tres canales a partes iguales, los dos darian 85 y la imagen
        // saldria turbia.
        let verde = aplicar(
            &imagen(&[0, 255, 0, 255]),
            Filtros {
                gris: true,
                ..Default::default()
            },
        );
        let azul = aplicar(
            &imagen(&[0, 0, 255, 255]),
            Filtros {
                gris: true,
                ..Default::default()
            },
        );
        assert_eq!(verde.pixeles[0], 149);
        assert_eq!(azul.pixeles[0], 29);
        assert!(verde.pixeles[0] > azul.pixeles[0] * 4);
        // Y los tres canales quedan iguales, que es lo que lo hace gris.
        assert_eq!(verde.pixeles[0], verde.pixeles[1]);
        assert_eq!(verde.pixeles[1], verde.pixeles[2]);
    }

    #[test]
    fn invertir_dos_veces_devuelve_el_original() {
        let original = imagen(&[10, 128, 250, 255]);
        let f = Filtros {
            invertido: true,
            ..Default::default()
        };
        let una = aplicar(&original, f);
        assert_eq!(una.pixeles[..3], [245, 127, 5]);
        // Aplicado sobre lo ya invertido, vuelve: es la comprobacion de que
        // la operacion es su propia inversa y no pierde nada por el camino.
        let dos = aplicar(&una, f);
        assert_eq!(dos.pixeles[..3], original.pixeles[..3]);
    }

    #[test]
    fn el_brillo_se_suma_para_que_el_negro_pueda_aclararse() {
        // Multiplicando, cero por lo que sea es cero: una captura de un
        // terminal oscuro no se aclararia NUNCA, que es justo el caso en el
        // que hace falta.
        let negro = aplicar(
            &imagen(&[0, 0, 0, 255]),
            Filtros {
                brillo: 2,
                ..Default::default()
            },
        );
        assert_eq!(negro.pixeles[0], 52);
    }

    #[test]
    fn el_brillo_no_se_sale_ni_da_la_vuelta() {
        // Caso negativo del desbordamiento: 250 mas dos pasos son 302, que
        // en un byte serian 46 — la imagen se volveria oscura de golpe al
        // intentar aclararla.
        let claro = aplicar(
            &imagen(&[250, 250, 250, 255]),
            Filtros {
                brillo: 5,
                ..Default::default()
            },
        );
        assert_eq!(claro.pixeles[..3], [255, 255, 255]);
        let oscuro = aplicar(
            &imagen(&[5, 5, 5, 255]),
            Filtros {
                brillo: -5,
                ..Default::default()
            },
        );
        assert_eq!(oscuro.pixeles[..3], [0, 0, 0]);
    }

    #[test]
    fn subir_y_bajar_el_brillo_deja_los_pasos_donde_estaban() {
        // Los pasos se guardan y la imagen se rehace SIEMPRE del original,
        // asi que ir y volver tiene que dar exactamente el punto de partida.
        // Encadenando pasadas, cada una redondearia y la imagen se
        // degradaria hasta quedar irreconocible.
        let f = Filtros::default().con_brillo(3).con_brillo(-3);
        assert_eq!(f.brillo, 0);
        assert!(f.son_neutros());
        let original = imagen(&[10, 128, 250, 255]);
        assert_eq!(aplicar(&original, f).pixeles, original.pixeles);
    }

    #[test]
    fn los_pasos_de_brillo_tienen_tope() {
        let mut f = Filtros::default();
        for _ in 0..50 {
            f = f.con_brillo(1);
        }
        assert_eq!(f.brillo, PASOS_MAXIMOS);
        for _ in 0..100 {
            f = f.con_brillo(-1);
        }
        assert_eq!(f.brillo, -PASOS_MAXIMOS);
    }

    #[test]
    fn el_brillo_se_aplica_despues_de_invertir() {
        // Al reves, subir el brillo OSCURECERIA: la inversion le daria la
        // vuelta a lo que acabas de sumar, y el boton haria lo contrario de
        // lo que dice.
        let f = Filtros {
            invertido: true,
            brillo: 2,
            ..Default::default()
        };
        // 100 invertido es 155; mas 52 de brillo, 207.
        let salida = aplicar(&imagen(&[100, 100, 100, 255]), f);
        assert_eq!(salida.pixeles[0], 207);
    }

    #[test]
    fn la_transparencia_no_se_toca() {
        // Filtrar el alfa convertiria los bordes suaves de un recorte en un
        // halo.
        let salida = aplicar(
            &imagen(&[10, 20, 30, 128]),
            Filtros {
                gris: true,
                invertido: true,
                brillo: 4,
            },
        );
        assert_eq!(salida.pixeles[3], 128);
    }

    #[test]
    fn una_imagen_vacia_no_revienta() {
        // Caso negativo: sin pixeles no hay nada que recorrer, y la pasada
        // tiene que salir sola en vez de indexar fuera.
        let vacia = ImagenRgba {
            ancho: 0,
            alto: 0,
            pixeles: Vec::new(),
        };
        let salida = aplicar(
            &vacia,
            Filtros {
                gris: true,
                ..Default::default()
            },
        );
        assert!(salida.pixeles.is_empty());
    }
}
