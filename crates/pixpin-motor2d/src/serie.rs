//! **Los numeros de serie**: el circulito con un 1, un 2, un 3.
//!
//! Es la herramienta de anotar una captura paso a paso, y lo unico que tiene
//! de listo es de donde saca el numero: **del mayor que haya en la escena,
//! mas uno**. No es un contador del editor, y eso importa —un contador se
//! desincroniza en cuanto se deshace, se pega o se abre otro documento—. El
//! movil hace exactamente esto (`DrawController.kt:3513-3518`), con la
//! consecuencia visible de que borrar el 2 de tres circulos deja el 4 como
//! siguiente, no el 2: los numeros ya puestos no se renumeran solos.
//!
//! El pintado del circulo y su cifra ya vive en `pintado.rs`; aqui va lo que
//! decide **cual** es el numero y **de que tamano** nace el circulo.

use crate::elemento::{Elemento, Figura};
use crate::vector::Punto2;

/// Cuanto mide el radio del circulo por cada punto de letra
/// (`SERIAL_RADIUS = 0.9`, `DrawController.kt:3459`).
pub const RADIO_POR_LETRA: f32 = 0.9;

/// El numero que le toca al siguiente.
///
/// Mira **solo lo que esta en el dibujo**: un circulo borrado no cuenta, que
/// es lo que permite deshacer el ultimo y que el siguiente vuelva a ser ese
/// mismo numero. Los borrados que quedan por debajo del mayor no cambian
/// nada, y eso tambien es del movil.
pub fn siguiente(elementos: &[Elemento]) -> u32 {
    elementos
        .iter()
        .filter(|e| !e.borrado)
        .filter_map(|e| match e.figura {
            Figura::Serie { numero } => Some(numero),
            _ => None,
        })
        .max()
        .map_or(1, |m| m.saturating_add(1))
}

/// El diametro del circulo para un tamano de letra.
pub fn diametro(tam_letra: f32) -> f32 {
    (tam_letra * RADIO_POR_LETRA * 2.0).max(1.0)
}

/// La caja de un circulo de serie **centrado** en `centro`.
///
/// Centrado y no con la esquina donde se pulsa: un numero de serie se planta
/// senalando algo, y lo que senala es el punto del dedo o del cursor, no la
/// esquina de una caja que el usuario no ve.
pub fn caja(centro: Punto2, tam_letra: f32) -> (f32, f32, f32, f32) {
    let d = diametro(tam_letra);
    (centro.x - d / 2.0, centro.y - d / 2.0, d, d)
}

/// Un numero de serie nuevo, ya con el numero que le toca.
///
/// El `id` lo pone el llamante (la escena es quien reparte identificadores);
/// aqui se deja en cero a proposito para que no haya dos sitios que los
/// inventen.
pub fn nuevo(
    elementos: &[Elemento],
    centro: Punto2,
    tam_letra: f32,
    modelo: &Elemento,
) -> Elemento {
    let (x, y, ancho, alto) = caja(centro, tam_letra);
    Elemento {
        id: 0,
        figura: Figura::Serie {
            numero: siguiente(elementos),
        },
        x,
        y,
        ancho,
        alto,
        // Del estilo activo se heredan el color y el grosor, como cualquier
        // otra figura; la caja no, que la manda el tamano de letra.
        ..modelo.clone()
    }
    .con_caja(x, y, ancho, alto)
}

/// El numero mide el radio por esto (`SERIAL_TEXT_RATIO = 1.25`).
pub const TEXTO_POR_RADIO: f32 = 1.25;

/// La letra del numero: la elegida en el panel (`fontFamily`, en los
/// extras) o, sin elegir, la del sistema en negrita, que es lo que pinta el
/// movil (`paint.reset()` deja la de Android).
pub fn familia_de(e: &Elemento) -> String {
    e.extras
        .familia
        .clone()
        .unwrap_or_else(|| "Segoe UI".to_string())
}

/// **Blanco o negro, lo que se lea sobre `fondo`** (`contrastingTextColor`
/// del movil: luminancia 0,299/0,587/0,114 y el corte en 150 de 255). Con la
/// misma transparencia que el disco.
pub fn tinta_sobre(fondo: crate::elemento::ColorRgba) -> crate::elemento::ColorRgba {
    let luz = 0.299 * fondo.r + 0.587 * fondo.g + 0.114 * fondo.b;
    let v = if luz > 150.0 / 255.0 { 0.0 } else { 1.0 };
    crate::elemento::ColorRgba {
        r: v,
        g: v,
        b: v,
        a: fondo.a,
    }
}
/// Pequeno ayudante: `..modelo.clone()` pisa la caja, asi que se vuelve a
/// poner. Es feo de leer en linea y por eso esta con nombre.
trait ConCaja {
    fn con_caja(self, x: f32, y: f32, ancho: f32, alto: f32) -> Self;
}

impl ConCaja for Elemento {
    fn con_caja(mut self, x: f32, y: f32, ancho: f32, alto: f32) -> Self {
        self.x = x;
        self.y = y;
        self.ancho = ancho;
        self.alto = alto;
        self
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn serie(numero: u32) -> Elemento {
        Elemento {
            figura: Figura::Serie { numero },
            ..Default::default()
        }
    }

    #[test]
    fn el_primero_es_el_uno() {
        assert_eq!(siguiente(&[]), 1);
        assert_eq!(
            siguiente(&[Elemento::default()]),
            1,
            "un dibujo sin circulos empieza por el uno"
        );
    }

    #[test]
    fn borrar_el_dos_de_tres_deja_el_cuatro_como_siguiente() {
        // El caso del movil, y es el que explica por que no hay contador:
        // los numeros ya puestos no se renumeran, asi que el siguiente sale
        // del mayor que se ve, no de cuantos hay.
        let mut es = vec![serie(1), serie(2), serie(3)];
        es[1].borrado = true;
        assert_eq!(siguiente(&es), 4);
    }

    #[test]
    fn deshacer_el_ultimo_devuelve_su_numero() {
        // Caso negativo del anterior: si `siguiente` contara los elementos en
        // vez de mirar el mayor vivo, aqui saldria un 3 y habria dos doses.
        let mut es = vec![serie(1), serie(2)];
        es[1].borrado = true;
        assert_eq!(siguiente(&es), 2);
    }

    #[test]
    fn el_circulo_nace_centrado_en_donde_se_pulsa() {
        // Si naciera con la esquina en el cursor, el numero senalaria un
        // palmo abajo y a la derecha de lo que se queria senalar.
        let (x, y, ancho, alto) = caja(Punto2::nuevo(100.0, 50.0), 20.0);
        assert_eq!(ancho, 36.0, "20 de letra por 0,9 de radio, por dos");
        assert_eq!(alto, 36.0);
        assert_eq!((x + ancho / 2.0, y + alto / 2.0), (100.0, 50.0));
    }

    #[test]
    fn un_tamano_de_letra_absurdo_no_produce_un_circulo_de_area_cero() {
        let (_, _, ancho, alto) = caja(Punto2::nuevo(0.0, 0.0), 0.0);
        assert!(ancho >= 1.0 && alto >= 1.0, "{ancho}x{alto}");
    }

    #[test]
    fn el_nuevo_hereda_el_estilo_pero_no_la_caja_del_modelo() {
        let modelo = Elemento {
            grosor: 7.0,
            x: -999.0,
            ancho: 1.0,
            ..Default::default()
        };
        let e = nuevo(&[serie(5)], Punto2::nuevo(10.0, 10.0), 10.0, &modelo);
        assert_eq!(e.figura, Figura::Serie { numero: 6 });
        assert_eq!(e.grosor, 7.0, "el estilo activo se hereda");
        assert_eq!(e.ancho, 18.0, "la caja la manda el tamano de letra");
        assert_eq!(e.x, 1.0);
        assert_eq!(e.id, 0, "los identificadores los reparte la escena");
    }

    #[test]
    fn sobre_un_disco_oscuro_el_numero_va_en_blanco_y_sobre_uno_claro_en_negro() {
        use crate::elemento::ColorRgba;
        let negro = tinta_sobre(ColorRgba::opaco(0.12, 0.12, 0.12));
        assert_eq!((negro.r, negro.g, negro.b), (1.0, 1.0, 1.0));
        // Caso negativo: sobre amarillo un numero blanco no se leeria.
        let amarillo = tinta_sobre(ColorRgba::opaco(1.0, 0.85, 0.0));
        assert_eq!((amarillo.r, amarillo.g, amarillo.b), (0.0, 0.0, 0.0));
        // Con la transparencia del disco.
        let medio = tinta_sobre(ColorRgba {
            a: 0.5,
            ..ColorRgba::opaco(0.0, 0.0, 0.0)
        });
        assert_eq!(medio.a, 0.5);
    }

    #[test]
    fn el_numero_va_en_la_letra_elegida_y_sin_elegir_en_la_del_sistema() {
        let mut e = serie(1);
        assert_eq!(familia_de(&e), "Segoe UI");
        e.extras.familia = Some("Caveat".into());
        assert_eq!(familia_de(&e), "Caveat");
    }
}
