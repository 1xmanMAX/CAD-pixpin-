//! La barra del reproductor: donde cae cada boton y por donde va la pista.
//!
//! Es `guardados/BarraDelReproductor.kt` del movil (:46-106). Geometria
//! pura: aqui no se sabe que suena, ni si suena. Quien pinta pregunta el
//! estado a `pixpin_audio::reloj::Estado` y usa estos rectangulos.
//!
//! Tres decisiones que no son evidentes:
//!
//! - **Los tres botones de transporte van juntos a la izquierda.** Atras,
//!   play y adelante se pulsan en racimo mientras se escucha; repartirlos por
//!   toda la barra obligaria a buscar cada uno.
//! - **La linea de avance es toda la barra, no un cacho.** Es la unica forma
//!   de que pinchar en la mitad de una nota lleve a la mitad, que es lo que
//!   la mano espera; el movil usa un `Slider` de ancho completo por lo mismo.
//! - **Nada se invierte en una barra estrecha.** Los botones se reparten
//!   desde los extremos y el hueco del texto es lo que sobra, que puede ser
//!   cero. En `u32` un ancho negativo pinta fuera de la ventana.

use pixpin_geom::{Punto, Rect};

/// Lo que ocupa la barra, en pixeles logicos.
pub const ALTO: u32 = 46;
/// El lado de cada boton redondo.
pub const BOTON: u32 = 32;
/// El aire a los lados de la barra.
pub const MARGEN: u32 = 10;
/// Entre dos botones del mismo racimo.
pub const HUECO: u32 = 2;
pub const TEXTO_TAM: f32 = 12.0;
/// Lo gruesa que es la linea de avance, pegada al canto de abajo.
pub const PROGRESO_ALTO: u32 = 3;

/// Como queda repartida la barra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Disposicion {
    pub barra: Rect,
    /// Menos diez segundos.
    pub atras: Rect,
    /// El unico boton que cambia de cara: play o pausa.
    pub tocar: Rect,
    /// Mas diez segundos.
    pub adelante: Rect,
    /// La velocidad, que se recorre en ciclo al pulsarla.
    pub velocidad: Rect,
    /// Soltar la pista: la barra desaparece.
    pub cerrar: Rect,
    /// Lo que queda en medio: el titulo y el tiempo. Puede medir cero.
    pub texto: Rect,
    /// La linea de avance, a todo lo ancho y pegada abajo.
    pub progreso: Rect,
}

impl Disposicion {
    pub fn calcular(barra: Rect, escala_por_cien: u32) -> Disposicion {
        let e = |v: u32| v * escala_por_cien / 100;
        let lado = e(BOTON).min(barra.alto);
        let margen = e(MARGEN) as i32;
        let hueco = e(HUECO) as i32;
        let y = barra.y + (barra.alto.saturating_sub(lado) / 2) as i32;
        let redondo = |x: i32| Rect {
            x,
            y,
            ancho: lado,
            alto: lado,
        };

        let atras = redondo(barra.x + margen);
        let tocar = redondo(atras.derecha() + hueco);
        let adelante = redondo(tocar.derecha() + hueco);
        // Los de la derecha se colocan desde el canto hacia dentro: asi el
        // aspa siempre acaba en el margen, mida lo que mida la ventana.
        let cerrar = redondo(barra.derecha() - margen - lado as i32);
        let velocidad = redondo(cerrar.x - hueco - lado as i32);

        let x_texto = adelante.derecha() + margen;
        let ancho_texto = (velocidad.x - margen - x_texto).max(0) as u32;
        Disposicion {
            barra,
            atras,
            tocar,
            adelante,
            velocidad,
            cerrar,
            texto: Rect {
                x: x_texto,
                y: barra.y,
                ancho: ancho_texto,
                alto: barra.alto,
            },
            progreso: Rect {
                x: barra.x,
                y: barra.abajo() - e(PROGRESO_ALTO).min(barra.alto) as i32,
                ancho: barra.ancho,
                alto: e(PROGRESO_ALTO).min(barra.alto),
            },
        }
    }

    /// Lo que se llena de la linea de avance, de 0 a 1.
    pub fn avance(&self, fraccion: f32) -> Rect {
        let f = fraccion.clamp(0.0, 1.0);
        Rect {
            ancho: (self.progreso.ancho as f32 * f) as u32,
            ..self.progreso
        }
    }

    /// A que punto de la pista lleva un clic, de 0 a 1.
    ///
    /// `None` fuera de la barra, y **tambien sobre un boton**: pinchar en
    /// pausa no puede ademas saltar al segundo 12 porque ahi cayo el dedo.
    pub fn punto_de(&self, p: Punto) -> Option<f32> {
        if !self.barra.contiene(p) {
            return None;
        }
        for b in [
            self.atras,
            self.tocar,
            self.adelante,
            self.velocidad,
            self.cerrar,
        ] {
            if b.contiene(p) {
                return None;
            }
        }

        if self.barra.ancho == 0 {
            return None;
        }
        Some(((p.x - self.barra.x) as f32 / self.barra.ancho as f32).clamp(0.0, 1.0))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn barra() -> Rect {
        Rect {
            x: 100,
            y: 500,
            ancho: 400,
            alto: ALTO,
        }
    }

    #[test]
    fn los_tres_del_transporte_van_juntos_a_la_izquierda() {
        let d = Disposicion::calcular(barra(), 100);
        assert_eq!(d.atras.x, barra().x + MARGEN as i32);
        assert!(d.atras.derecha() <= d.tocar.x);
        assert!(d.tocar.derecha() <= d.adelante.x);
        assert_eq!(d.atras.y, d.tocar.y);
    }

    #[test]
    fn el_aspa_acaba_en_el_margen_derecho() {
        let d = Disposicion::calcular(barra(), 100);
        assert_eq!(d.cerrar.derecha(), barra().derecha() - MARGEN as i32);
        assert!(d.velocidad.derecha() <= d.cerrar.x);
    }

    #[test]
    fn el_texto_se_queda_con_lo_que_sobra_y_nunca_con_menos_de_nada() {
        let d = Disposicion::calcular(barra(), 100);
        assert!(d.texto.ancho > 0);
        assert!(d.texto.x >= d.adelante.derecha());
        // Caso negativo: una barra mas estrecha que los cinco botones deja
        // el texto en cero en vez de dar la vuelta al `u32`.
        let estrecha = Disposicion::calcular(
            Rect {
                ancho: 40,
                ..barra()
            },
            100,
        );
        assert_eq!(estrecha.texto.ancho, 0);
    }

    #[test]
    fn pinchar_en_la_mitad_lleva_a_la_mitad() {
        let d = Disposicion::calcular(barra(), 100);
        let medio = Punto {
            x: barra().x + 200,
            y: barra().y + 2,
        };
        assert_eq!(d.punto_de(medio), Some(0.5));
    }

    #[test]
    fn pinchar_un_boton_no_ademas_salta_de_sitio() {
        let d = Disposicion::calcular(barra(), 100);
        let encima = Punto {
            x: d.tocar.x + 2,
            y: d.tocar.y + 2,
        };
        assert_eq!(d.punto_de(encima), None);
        // Y fuera de la barra, tampoco.
        assert_eq!(
            d.punto_de(Punto {
                x: barra().x - 5,
                y: barra().y
            }),
            None
        );
    }

    #[test]
    fn la_linea_de_avance_va_pegada_abajo_y_se_llena_con_la_fraccion() {
        let d = Disposicion::calcular(barra(), 100);
        assert_eq!(d.progreso.abajo(), barra().abajo());
        assert_eq!(d.progreso.ancho, barra().ancho);
        assert_eq!(d.avance(0.0).ancho, 0);
        assert_eq!(d.avance(0.25).ancho, 100);
        assert_eq!(d.avance(1.0).ancho, 400);
        // Un valor fuera de rango no pinta una barra mas larga que la barra.
        assert_eq!(d.avance(5.0).ancho, 400);
        assert_eq!(d.avance(-1.0).ancho, 0);
    }

    #[test]
    fn una_barra_mas_baja_que_un_boton_no_lo_desborda() {
        let baja = Rect {
            alto: 10,
            ..barra()
        };
        let d = Disposicion::calcular(baja, 100);
        assert_eq!(d.tocar.alto, 10);
        assert_eq!(d.progreso.alto, PROGRESO_ALTO);
    }
}
