//! Resolucion del ajuste automatico, separada de donde salgan los candidatos.
//!
//! En S1-B2 los rellenara UI Automation recorriendo el arbol de controles de
//! la ventana bajo el cursor. Aqui la logica se prueba con arboles inventados,
//! que es la unica forma de cubrir los casos raros —elementos de area cero,
//! rectangulos identicos, orden arbitrario— sin depender de que una
//! aplicacion concreta este abierta.

use crate::punto::Punto;
use crate::rect::Rect;

/// Un elemento al que la seleccion podria ajustarse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Candidato {
    pub rect: Rect,
    /// Distancia a la raiz del arbol de controles. La ventana es 0.
    ///
    /// Solo se usa para desempatar cuando dos candidatos tienen exactamente
    /// la misma area, que ocurre cuando un contenedor tiene un unico hijo que
    /// lo ocupa entero.
    pub profundidad: u16,
}

/// El rectangulo mas ajustado que contiene al cursor.
///
/// Devuelve `None` si ningun candidato lo contiene, en vez de inventarse uno:
/// mas vale no ajustar que ajustar a algo que el usuario no esta senalando.
///
/// El resultado **no depende del orden de la lista**. UI Automation no
/// garantiza ninguno, y si dependiera de el el recuadro parpadearia entre
/// elementos al mover el raton un pixel.
pub fn resolver_ajuste(candidatos: &[Candidato], cursor: Punto) -> Option<Rect> {
    candidatos
        .iter()
        .filter(|c| !c.rect.esta_vacio())
        .filter(|c| c.rect.contiene(cursor))
        // El mas pequeno gana; a igualdad de area, el mas profundo del arbol,
        // que es el mas especifico. Ambas claves juntas hacen el criterio
        // total y por tanto independiente del orden de entrada.
        .min_by_key(|c| (c.rect.area(), std::cmp::Reverse(c.profundidad)))
        .map(|c| c.rect)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn c(x: i32, y: i32, ancho: u32, alto: u32, profundidad: u16) -> Candidato {
        Candidato {
            rect: Rect { x, y, ancho, alto },
            profundidad,
        }
    }

    #[test]
    fn sin_candidatos_no_hay_ajuste() {
        assert_eq!(resolver_ajuste(&[], Punto { x: 10, y: 10 }), None);
    }

    #[test]
    fn el_cursor_fuera_de_todo_no_ajusta() {
        // Caso negativo: una implementacion perezosa podria devolver el
        // primer candidato sin comprobar la contencion.
        let candidatos = [c(0, 0, 100, 100, 0)];
        assert_eq!(resolver_ajuste(&candidatos, Punto { x: 500, y: 500 }), None);
    }

    #[test]
    fn gana_el_candidato_mas_ajustado_bajo_el_cursor() {
        // Una ventana con un panel dentro y un boton dentro del panel. El
        // cursor esta sobre el boton: debe ganar el boton, no la ventana.
        let candidatos = [
            c(0, 0, 1000, 800, 0),  // ventana
            c(50, 50, 400, 300, 1), // panel
            c(60, 60, 80, 24, 2),   // boton
        ];
        let r = resolver_ajuste(&candidatos, Punto { x: 100, y: 70 }).unwrap();
        assert_eq!(
            r,
            Rect {
                x: 60,
                y: 60,
                ancho: 80,
                alto: 24
            }
        );
    }

    #[test]
    fn el_orden_de_la_lista_no_cambia_el_resultado() {
        // UI Automation no garantiza orden. Si el resultado dependiera de el,
        // el ajuste parpadearia entre elementos al mover el raton.
        let ventana = c(0, 0, 1000, 800, 0);
        let panel = c(50, 50, 400, 300, 1);
        let boton = c(60, 60, 80, 24, 2);
        let cursor = Punto { x: 100, y: 70 };

        let a = resolver_ajuste(&[ventana, panel, boton], cursor);
        let b = resolver_ajuste(&[boton, ventana, panel], cursor);
        let cc = resolver_ajuste(&[panel, boton, ventana], cursor);
        assert_eq!(a, b);
        assert_eq!(b, cc);
        assert!(a.is_some());
    }

    #[test]
    fn con_areas_iguales_gana_el_mas_profundo() {
        // Dos elementos con el mismo rectangulo: un panel y su unico hijo,
        // que lo ocupa entero. Debe ganar el hijo, que es lo mas especifico.
        let candidatos = [c(0, 0, 100, 100, 1), c(0, 0, 100, 100, 5)];
        let r = resolver_ajuste(&candidatos, Punto { x: 50, y: 50 });
        assert_eq!(
            r,
            Some(Rect {
                x: 0,
                y: 0,
                ancho: 100,
                alto: 100
            })
        );
        // Y el desempate es determinista: repetido con el orden invertido da
        // exactamente lo mismo.
        let invertido = [c(0, 0, 100, 100, 5), c(0, 0, 100, 100, 1)];
        assert_eq!(resolver_ajuste(&invertido, Punto { x: 50, y: 50 }), r);
    }

    #[test]
    fn los_candidatos_vacios_se_ignoran() {
        // UI Automation devuelve elementos de area cero con frecuencia
        // (contenedores plegados). Ajustar a uno de ellos daria una captura
        // vacia sin explicacion.
        let candidatos = [c(10, 10, 0, 0, 9), c(0, 0, 100, 100, 0)];
        let r = resolver_ajuste(&candidatos, Punto { x: 10, y: 10 }).unwrap();
        assert_eq!(
            r,
            Rect {
                x: 0,
                y: 0,
                ancho: 100,
                alto: 100
            }
        );
    }

    #[test]
    fn el_borde_derecho_no_pertenece_al_candidato() {
        // Coherente con la media apertura de Rect::contiene.
        let candidatos = [c(0, 0, 100, 100, 0)];
        assert!(resolver_ajuste(&candidatos, Punto { x: 99, y: 99 }).is_some());
        assert!(resolver_ajuste(&candidatos, Punto { x: 100, y: 50 }).is_none());
    }
}
