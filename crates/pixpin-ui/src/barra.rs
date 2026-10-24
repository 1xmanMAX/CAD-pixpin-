//! La barra de resultado: cuatro botones junto a la seleccion confirmada.
//!
//! En S2 esta barra crece hasta ser la entrada al editor y en S3 gana el
//! boton de pinear (la spec lo dice explicitamente), asi que el layout es
//! una lista de acciones, no cuatro rectangulos con nombre.

use pixpin_geom::{Punto, Rect};

/// Medidas base en pixeles logicos (al 100% de escala).
const ALTO_LOGICO: u32 = 40;
const ANCHO_BOTON_LOGICO: u32 = 96;
const HUECO_LOGICO: u32 = 4;
const SEPARACION_LOGICA: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccionBarra {
    Copiar,
    Guardar,
    GuardarComo,
    Descartar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Barra {
    pub origen: Rect,
    escala_por_cien: u32,
}

impl Barra {
    pub const ACCIONES: [AccionBarra; 4] = [
        AccionBarra::Copiar,
        AccionBarra::Guardar,
        AccionBarra::GuardarComo,
        AccionBarra::Descartar,
    ];

    /// Debajo de la seleccion; si no cabe, encima; si tampoco, dentro,
    /// pegada al borde inferior. Siempre integra en el area de trabajo.
    pub fn colocar(seleccion: Rect, area_trabajo: Rect, escala_por_cien: u32) -> Barra {
        let e = |v: u32| v * escala_por_cien / 100;
        let alto = e(ALTO_LOGICO);
        let ancho = e(ANCHO_BOTON_LOGICO) * Self::ACCIONES.len() as u32
            + e(HUECO_LOGICO) * (Self::ACCIONES.len() as u32 - 1);
        let sep = e(SEPARACION_LOGICA) as i32;

        // Alineada al borde derecho de la seleccion, sin salirse por la
        // izquierda del area de trabajo.
        let x = (seleccion.derecha() - ancho as i32).clamp(
            area_trabajo.izquierda(),
            area_trabajo.derecha() - ancho as i32,
        );

        let debajo = seleccion.abajo() + sep;
        let encima = seleccion.arriba() - sep - alto as i32;
        let y = if debajo + (alto as i32) <= area_trabajo.abajo() {
            debajo
        } else if encima >= area_trabajo.arriba() {
            encima
        } else {
            // Dentro de la seleccion, pegada abajo.
            seleccion.abajo() - sep - alto as i32
        };

        Barra {
            origen: Rect { x, y, ancho, alto },
            escala_por_cien,
        }
    }

    pub fn rect_boton(&self, accion: AccionBarra) -> Rect {
        let e = |v: u32| v * self.escala_por_cien / 100;
        let indice = Self::ACCIONES
            .iter()
            .position(|a| *a == accion)
            .expect("toda accion esta en la lista") as u32;
        Rect {
            x: self.origen.x + (indice * (e(ANCHO_BOTON_LOGICO) + e(HUECO_LOGICO))) as i32,
            y: self.origen.y,
            ancho: e(ANCHO_BOTON_LOGICO),
            alto: self.origen.alto,
        }
    }

    pub fn boton_en(&self, p: Punto) -> Option<AccionBarra> {
        Self::ACCIONES
            .into_iter()
            .find(|a| self.rect_boton(*a).contiene(p))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_geom::{Punto, Rect};

    fn area_trabajo() -> Rect {
        Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1040,
        }
    }

    #[test]
    fn la_barra_se_coloca_debajo_de_la_seleccion() {
        let sel = Rect {
            x: 400,
            y: 300,
            ancho: 500,
            alto: 200,
        };
        let b = Barra::colocar(sel, area_trabajo(), 100);
        assert!(b.origen.arriba() > sel.abajo());
        // Alineada al borde derecho de la seleccion, como las herramientas
        // de captura que la gente ya conoce.
        assert_eq!(b.origen.derecha(), sel.derecha());
    }

    #[test]
    fn sin_sitio_debajo_la_barra_sube_encima() {
        let sel = Rect {
            x: 400,
            y: 800,
            ancho: 500,
            alto: 220,
        };
        let b = Barra::colocar(sel, area_trabajo(), 100);
        assert!(b.origen.abajo() < sel.arriba() + 1, "deberia quedar encima");
        assert!(
            area_trabajo().interseccion(b.origen) == Some(b.origen),
            "y dentro del area de trabajo"
        );
    }

    #[test]
    fn con_la_seleccion_a_pantalla_completa_la_barra_queda_dentro() {
        // Caso negativo del layout: ni debajo ni encima hay sitio. Si la
        // implementacion solo probara esas dos, la barra quedaria fuera de
        // pantalla e inalcanzable.
        let sel = Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1040,
        };
        let b = Barra::colocar(sel, area_trabajo(), 100);
        assert_eq!(area_trabajo().interseccion(b.origen), Some(b.origen));
    }

    #[test]
    fn la_barra_escala_con_el_dpi() {
        let sel = Rect {
            x: 100,
            y: 100,
            ancho: 300,
            alto: 200,
        };
        let cien = Barra::colocar(sel, area_trabajo(), 100);
        let doscientos = Barra::colocar(sel, area_trabajo(), 200);
        assert_eq!(doscientos.origen.alto, cien.origen.alto * 2);
        assert_eq!(doscientos.origen.ancho, cien.origen.ancho * 2);
    }

    #[test]
    fn cada_boton_responde_en_su_rectangulo_y_fuera_nadie() {
        let sel = Rect {
            x: 400,
            y: 300,
            ancho: 500,
            alto: 200,
        };
        let b = Barra::colocar(sel, area_trabajo(), 100);
        for accion in Barra::ACCIONES {
            let r = b.rect_boton(accion);
            let centro = Punto {
                x: r.x + (r.ancho / 2) as i32,
                y: r.y + (r.alto / 2) as i32,
            };
            assert_eq!(b.boton_en(centro), Some(accion), "fallo en {accion:?}");
        }
        // Caso negativo: un punto fuera de la barra no responde.
        assert_eq!(b.boton_en(Punto { x: 0, y: 0 }), None);
        // Y el hueco entre dos botones tampoco.
        let primero = b.rect_boton(Barra::ACCIONES[0]);
        assert_eq!(
            b.boton_en(Punto {
                x: primero.derecha() + 1,
                y: primero.y + 4
            }),
            None,
            "el hueco entre botones no debe activar ninguno"
        );
    }
}
