//! El autofiltro guardado (`<autoFilter>` de la hoja o de una tabla): solo
//! interesa dibujar sus botones en la fila de encabezado y saber en que
//! columnas hay un filtro puesto. Las filas que el filtro oculta ya vienen
//! con `hidden="1"` en el archivo: no se vuelve a filtrar nada aqui.

use crate::hoja::Rango;
use crate::xml::Nodo;

#[derive(Debug, Clone, PartialEq)]
pub struct AutoFiltro {
    /// El rango filtrado; su primera fila es el encabezado con los botones.
    pub rango: Rango,
    /// Columnas (absolutas) sin boton (`hiddenButton` o `showButton="0"`).
    pub sin_boton: Vec<u32>,
    /// Columnas (absolutas) con un filtro puesto.
    pub activas: Vec<u32>,
}

impl AutoFiltro {
    /// Lee un nodo `<autoFilter ref>`; `rango_defecto` es el de la tabla si
    /// el nodo no trae `ref`.
    pub fn leer(n: &Nodo, rango_defecto: Option<Rango>) -> Option<AutoFiltro> {
        let rango = n.attr("ref").and_then(Rango::leer).or(rango_defecto)?;
        let mut sin_boton = Vec::new();
        let mut activas = Vec::new();
        for f in n.hijos_de("filterColumn") {
            let Some(id) = f.attr("colId").and_then(|v| v.parse::<u32>().ok()) else {
                continue;
            };
            // Una columna fuera del rango no existe.
            let col = rango.col0.saturating_add(id);
            if col > rango.col1 {
                continue;
            }
            if f.attr("hiddenButton")
                .is_some_and(|v| v == "1" || v == "true")
                || f.attr("showButton")
                    .is_some_and(|v| v == "0" || v == "false")
            {
                sin_boton.push(col);
            }
            if !f.hijos.is_empty() {
                activas.push(col);
            }
        }
        Some(AutoFiltro {
            rango,
            sin_boton,
            activas,
        })
    }

    /// Las columnas con boton, de izquierda a derecha.
    pub fn columnas_con_boton(&self) -> impl Iterator<Item = u32> + '_ {
        // Tope para un rango absurdo (un autofiltro de 16 384 columnas).
        (self.rango.col0..=self.rango.col1.min(self.rango.col0.saturating_add(2000)))
            .filter(|c| !self.sin_boton.contains(c))
    }

    pub fn tiene_filtro(&self, col: u32) -> bool {
        self.activas.contains(&col)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::xml;

    #[test]
    fn botones_ocultos_y_columnas_filtradas() {
        let x = br#"<x><autoFilter ref="B2:E50"><filterColumn colId="0"><filters><filter val="a"/></filters></filterColumn><filterColumn colId="1" hiddenButton="1"/><filterColumn colId="9"><filters/></filterColumn><filterColumn colId="3" showButton="0"><top10 top="1" val="3"/></filterColumn></autoFilter></x>"#;
        let r = xml::arbol(x);
        let a = AutoFiltro::leer(r.hijo("x").unwrap().hijo("autoFilter").unwrap(), None).unwrap();
        assert_eq!(
            a.rango,
            Rango {
                fila0: 1,
                col0: 1,
                fila1: 49,
                col1: 4
            }
        );
        assert_eq!(a.activas, vec![1, 4]);
        assert_eq!(a.sin_boton, vec![2, 4]);
        assert_eq!(a.columnas_con_boton().collect::<Vec<_>>(), vec![1, 3]);
        assert!(a.tiene_filtro(1) && !a.tiene_filtro(2));
    }

    #[test]
    fn sin_rango_no_hay_filtro_salvo_el_de_la_tabla() {
        let r = xml::arbol(br#"<autoFilter/>"#);
        let n = r.hijo("autoFilter").unwrap();
        assert!(AutoFiltro::leer(n, None).is_none());
        assert!(
            AutoFiltro::leer(
                n,
                Some(Rango {
                    fila0: 0,
                    col0: 0,
                    fila1: 3,
                    col1: 1
                })
            )
            .is_some()
        );
        let r = xml::arbol(br#"<autoFilter ref="no es un rango"/>"#);
        assert!(AutoFiltro::leer(r.hijo("autoFilter").unwrap(), None).is_none());
    }
}
