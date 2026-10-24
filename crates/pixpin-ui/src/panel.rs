//! El panel del overlay de captura con el boton «Seleccionar todo»: un
//! boton, arriba y centrado en el monitor bajo el cursor, mientras todavia
//! no hay seleccion. Pura: donde va y si un punto lo toca.
//!
//! Vive aparte de la barra de resultado porque aparece en el momento
//! contrario (antes de seleccionar, no despues) y no tiene acciones que
//! decidir: es un atajo con forma de boton.

use pixpin_geom::{Punto, Rect};

/// Tamano del boton en px logicos.
const ANCHO_LOGICO: u32 = 168;
const ALTO_LOGICO: u32 = 36;
/// Separacion del borde superior del monitor, en px logicos.
const MARGEN_SUPERIOR_LOGICO: u32 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelTodo {
    pub rect: Rect,
}

impl PanelTodo {
    /// Arriba y centrado en `monitor` (pixeles fisicos), a la escala del
    /// monitor. En un monitor mas estrecho que el boton, pegado al borde.
    pub fn colocar(monitor: Rect, escala_por_cien: u32) -> PanelTodo {
        let e = |v: u32| v * escala_por_cien / 100;
        let ancho = e(ANCHO_LOGICO).min(monitor.ancho.max(1));
        let alto = e(ALTO_LOGICO).min(monitor.alto.max(1));
        let x = monitor.x + (monitor.ancho as i32 - ancho as i32) / 2;
        let y = monitor.y + e(MARGEN_SUPERIOR_LOGICO) as i32;
        PanelTodo {
            rect: Rect {
                x: x.max(monitor.x),
                y: y.min(monitor.abajo() - alto as i32).max(monitor.y),
                ancho,
                alto,
            },
        }
    }

    pub fn contiene(&self, p: Punto) -> bool {
        self.rect.contiene(p)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn monitor() -> Rect {
        Rect {
            x: 100,
            y: 50,
            ancho: 3000,
            alto: 2000,
        }
    }

    #[test]
    fn el_panel_va_arriba_y_centrado_a_la_escala_del_monitor() {
        let p = PanelTodo::colocar(monitor(), 150);
        assert_eq!((p.rect.ancho, p.rect.alto), (252, 54));
        assert_eq!(p.rect.y, 50 + 24);
        // Centrado: mismo margen a izquierda y derecha.
        assert_eq!(p.rect.x - 100, (100 + 3000) - p.rect.derecha());
        assert!(p.contiene(Punto {
            x: p.rect.x + 10,
            y: p.rect.y + 10
        }));
        assert!(!p.contiene(Punto { x: 100, y: 50 }));
    }

    #[test]
    fn en_un_monitor_minusculo_no_se_sale() {
        // Caso negativo: sin los topes, un monitor de 100 px daria un boton
        // de 168 con la mitad fuera.
        let chico = Rect {
            x: 0,
            y: 0,
            ancho: 100,
            alto: 40,
        };
        let p = PanelTodo::colocar(chico, 100);
        assert!(p.rect.x >= 0 && p.rect.derecha() <= 100);
        assert!(p.rect.y >= 0 && p.rect.abajo() <= 40);
    }
}
