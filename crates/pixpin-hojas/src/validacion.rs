//! **Validacion de datos** (`<dataValidations>`): solo para ver. De cada
//! regla se lee a que celdas afecta, de que tipo es y, si es una lista, de
//! donde salen los valores (literales o un rango). No se valida nada al
//! escribir: el visor no edita.
//!
//! Partes traducidas de gridcraft (https://github.com/storytold/gridcraft), MIT OR Apache-2.0, Copyright (c) 2026 ArtCraft Team and the Gridcraft contributors.

use crate::hoja::Rango;
use crate::xml::Nodo;

/// Tope de reglas por hoja y de valores en una lista literal (una hoja
/// danada no se lleva la memoria).
const MAX_REGLAS: usize = 50_000;
const MAX_VALORES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoValidacion {
    Entero,
    Decimal,
    Lista,
    Fecha,
    Hora,
    Longitud,
    Personalizada,
    Cualquiera,
}

/// De donde salen los valores de una lista.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrigenLista {
    /// `"si,no,quiza"`.
    Literal(Vec<String>),
    /// `Hoja2!$A$1:$A$5` o `$A$1:$A$5` (`hoja` es `None` en la misma hoja).
    Rango { hoja: Option<String>, rango: Rango },
    /// Un nombre definido, una formula... algo que no se resuelve aqui.
    Otro(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Validacion {
    pub rangos: Vec<Rango>,
    pub tipo: TipoValidacion,
    /// `between`, `greaterThan`... (`between` si no se dice).
    pub operador: String,
    pub formula1: String,
    pub formula2: Option<String>,
    pub permitir_vacio: bool,
    /// `false` si `showDropDown="1"` (que en Excel significa «sin flecha»).
    pub flecha: bool,
    pub titulo: String,
    pub mensaje: String,
}

impl Validacion {
    pub fn afecta(&self, fila: u32, col: u32) -> bool {
        self.rangos.iter().any(|r| r.contiene(fila, col))
    }

    /// De donde salen los valores, si es una lista.
    pub fn origen_lista(&self) -> Option<OrigenLista> {
        if self.tipo != TipoValidacion::Lista {
            return None;
        }
        origen_de(&self.formula1)
    }

    /// Los limites de un tipo numerico como texto (`1 a 10`, `>= 5`...) para
    /// mostrarlos; `None` si no hay nada que decir.
    pub fn resumen(&self) -> Option<String> {
        let (a, b) = (
            self.formula1.as_str(),
            self.formula2.as_deref().unwrap_or(""),
        );
        if a.is_empty()
            || matches!(
                self.tipo,
                TipoValidacion::Lista | TipoValidacion::Personalizada | TipoValidacion::Cualquiera
            )
        {
            return None;
        }
        Some(match self.operador.as_str() {
            "between" => format!("{a} a {b}"),
            "notBetween" => format!("fuera de {a} a {b}"),
            "equal" => format!("= {a}"),
            "notEqual" => format!("<> {a}"),
            "greaterThan" => format!("> {a}"),
            "lessThan" => format!("< {a}"),
            "greaterThanOrEqual" => format!(">= {a}"),
            "lessThanOrEqual" => format!("<= {a}"),
            _ => a.to_string(),
        })
    }
}

/// Interpreta la formula de una lista.
pub fn origen_de(f: &str) -> Option<OrigenLista> {
    let f = f.trim().trim_start_matches('=').trim();
    if f.is_empty() {
        return None;
    }
    if let Some(interior) = f.strip_prefix('"') {
        // Literal: las comillas dobles internas van duplicadas.
        let interior = interior.strip_suffix('"')?;
        let valores: Vec<String> = interior
            .replace("\"\"", "\"")
            .split(',')
            .map(|s| s.trim().to_string())
            .take(MAX_VALORES)
            .collect();
        return Some(OrigenLista::Literal(valores));
    }
    let (hoja, ref_) = match f.rfind('!') {
        Some(i) => {
            let h = f[..i].trim();
            let h = match h.strip_prefix('\'').and_then(|x| x.strip_suffix('\'')) {
                Some(q) => q.replace("''", "'"),
                None => h.to_string(),
            };
            (Some(h), &f[i + 1..])
        }
        None => (None, f),
    };
    match Rango::leer(ref_) {
        Some(rango) => Some(OrigenLista::Rango { hoja, rango }),
        None => Some(OrigenLista::Otro(f.to_string())),
    }
}

/// Lee todas las reglas de una hoja (las normales y las de la extension x14).
pub fn leer(raiz: &Nodo) -> Vec<Validacion> {
    let mut nodos = Vec::new();
    raiz.todos("dataValidation", &mut nodos);
    let mut v = Vec::new();
    for n in nodos.into_iter().take(MAX_REGLAS) {
        // En x14 el sqref es un hijo (`xm:sqref`); en la normal, un atributo.
        let sq = n
            .attr("sqref")
            .map(str::to_string)
            .or_else(|| n.hijo("sqref").map(|s| s.texto.clone()))
            .unwrap_or_default();
        let rangos: Vec<Rango> = sq.split_whitespace().filter_map(Rango::leer).collect();
        if rangos.is_empty() {
            continue;
        }
        let formula = |nombre: &str| -> Option<String> {
            let f = n.hijo(nombre)?;
            let t = f
                .hijo("f")
                .map(|x| x.texto.as_str())
                .unwrap_or(f.texto.as_str())
                .trim();
            (!t.is_empty()).then(|| t.to_string())
        };
        let tipo = match n.attr("type").unwrap_or("none") {
            "whole" => TipoValidacion::Entero,
            "decimal" => TipoValidacion::Decimal,
            "list" => TipoValidacion::Lista,
            "date" => TipoValidacion::Fecha,
            "time" => TipoValidacion::Hora,
            "textLength" => TipoValidacion::Longitud,
            "custom" => TipoValidacion::Personalizada,
            _ => TipoValidacion::Cualquiera,
        };
        let si = |a: &str| n.attr(a).is_some_and(|x| x == "1" || x == "true");
        v.push(Validacion {
            rangos,
            tipo,
            operador: n.attr("operator").unwrap_or("between").to_string(),
            formula1: formula("formula1").unwrap_or_default(),
            formula2: formula("formula2"),
            permitir_vacio: si("allowBlank"),
            flecha: !si("showDropDown"),
            titulo: n.attr("promptTitle").unwrap_or("").to_string(),
            mensaje: n.attr("prompt").unwrap_or("").to_string(),
        });
    }
    v
}

/// Cuantos valores se ensenan como mucho en una lista desplegable.
pub const MAX_MENU: usize = 500;

/// **Los valores permitidos** de una regla de lista, como texto ya
/// formateado: los literales tal cual, o las celdas del rango (de esta hoja
/// o de otra del libro, que se lee solo hasta la fila que hace falta). Los
/// vacios se saltan. Una formula que no se sabe resolver da una lista vacia.
pub fn valores_lista(
    libro: &crate::libro::Libro,
    actual: &crate::hoja::Hoja,
    v: &Validacion,
) -> Vec<String> {
    match v.origen_lista() {
        Some(OrigenLista::Literal(l)) => l
            .into_iter()
            .filter(|s| !s.is_empty())
            .take(MAX_MENU)
            .collect(),
        Some(OrigenLista::Rango { hoja, rango }) => {
            let otra;
            let h = match hoja
                .as_deref()
                .filter(|n| !n.eq_ignore_ascii_case(&actual.nombre))
            {
                None => actual,
                Some(n) => {
                    let Some(i) = libro
                        .hojas
                        .iter()
                        .position(|h| h.nombre.eq_ignore_ascii_case(n))
                    else {
                        return Vec::new();
                    };
                    let mut h = libro.abrir_hoja(i);
                    h.cargar_hasta_fila(rango.fila1.min(MAX_MENU as u32 * 4));
                    otra = h;
                    &otra
                }
            };
            let mut v = Vec::new();
            'filas: for f in rango.fila0
                ..=rango
                    .fila1
                    .min(rango.fila0.saturating_add(MAX_MENU as u32 * 4))
            {
                for c in rango.col0..=rango.col1.min(rango.col0 + 16) {
                    let Some(celda) = h.celda(f, c) else { continue };
                    let Some(t) = libro.contenido(h, celda).map(|c| c.mostrado.texto) else {
                        continue;
                    };
                    if !t.is_empty() {
                        v.push(t);
                        if v.len() >= MAX_MENU {
                            break 'filas;
                        }
                    }
                }
            }
            v
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::xml::arbol;

    fn reglas(x: &str) -> Vec<Validacion> {
        leer(&arbol(x.as_bytes()))
    }

    #[test]
    fn lista_literal() {
        let v = reglas(
            r#"<worksheet><dataValidations count="1"><dataValidation type="list" allowBlank="1" sqref="B2:B10 D4" promptTitle="Estado" prompt="Elige">
            <formula1>"Abierto,Cerrado, En curso"</formula1></dataValidation></dataValidations></worksheet>"#,
        );
        assert_eq!(v.len(), 1);
        let r = &v[0];
        assert_eq!(r.tipo, TipoValidacion::Lista);
        assert!(r.afecta(5, 1) && r.afecta(3, 3) && !r.afecta(0, 0));
        assert!(r.flecha && r.permitir_vacio);
        assert_eq!(r.titulo, "Estado");
        assert_eq!(
            r.origen_lista(),
            Some(OrigenLista::Literal(vec![
                "Abierto".into(),
                "Cerrado".into(),
                "En curso".into()
            ]))
        );
    }

    #[test]
    fn lista_por_rango_con_y_sin_hoja_y_sin_flecha() {
        let v = reglas(
            r#"<worksheet><dataValidations><dataValidation type="list" sqref="A1"><formula1>Listas!$A$1:$A$5</formula1></dataValidation>
            <dataValidation type="list" sqref="A2" showDropDown="1"><formula1>'Mis datos'!B1:B3</formula1></dataValidation>
            <dataValidation type="list" sqref="A3"><formula1>$C$1:$C$4</formula1></dataValidation></dataValidations></worksheet>"#,
        );
        assert_eq!(v.len(), 3);
        assert_eq!(
            v[0].origen_lista(),
            Some(OrigenLista::Rango {
                hoja: Some("Listas".into()),
                rango: Rango {
                    fila0: 0,
                    col0: 0,
                    fila1: 4,
                    col1: 0
                }
            })
        );
        assert_eq!(
            v[1].origen_lista(),
            Some(OrigenLista::Rango {
                hoja: Some("Mis datos".into()),
                rango: Rango {
                    fila0: 0,
                    col0: 1,
                    fila1: 2,
                    col1: 1
                }
            })
        );
        assert!(!v[1].flecha, "showDropDown=1 oculta la flecha");
        assert_eq!(
            v[2].origen_lista(),
            Some(OrigenLista::Rango {
                hoja: None,
                rango: Rango {
                    fila0: 0,
                    col0: 2,
                    fila1: 3,
                    col1: 2
                }
            })
        );
    }

    #[test]
    fn entero_con_minimo_y_maximo() {
        let v = reglas(
            r#"<worksheet><dataValidations><dataValidation type="whole" operator="between" sqref="C1:C9">
            <formula1>1</formula1><formula2>10</formula2></dataValidation>
            <dataValidation type="whole" operator="greaterThanOrEqual" sqref="E1"><formula1>5</formula1></dataValidation></dataValidations></worksheet>"#,
        );
        assert_eq!(v[0].tipo, TipoValidacion::Entero);
        assert_eq!(v[0].formula1, "1");
        assert_eq!(v[0].formula2.as_deref(), Some("10"));
        assert_eq!(v[0].resumen().as_deref(), Some("1 a 10"));
        assert_eq!(v[0].origen_lista(), None);
        assert_eq!(v[1].resumen().as_deref(), Some(">= 5"));
    }

    #[test]
    fn la_extension_x14_tambien_se_lee() {
        let v = reglas(
            r#"<worksheet><extLst><ext><x14:dataValidations><x14:dataValidation type="list"><x14:formula1><xm:f>Otra!$A$1:$A$3</xm:f></x14:formula1>
            <xm:sqref>F2:F4</xm:sqref></x14:dataValidation></x14:dataValidations></ext></extLst></worksheet>"#,
        );
        assert_eq!(v.len(), 1);
        assert!(v[0].afecta(2, 5));
        assert!(matches!(
            v[0].origen_lista(),
            Some(OrigenLista::Rango { .. })
        ));
    }

    #[test]
    fn casos_raros_y_negativos() {
        // Una hoja sin validaciones.
        assert!(reglas("<worksheet><sheetData/></worksheet>").is_empty());
        // Sin celdas a las que aplicar, o con basura: se descarta sin panico.
        assert!(
            reglas(
                r#"<worksheet><dataValidations><dataValidation type="list"><formula1>"a,b"</formula1></dataValidation>
            <dataValidation type="list" sqref="ZZZZ99999999"><formula1>"a"</formula1></dataValidation></dataValidations></worksheet>"#
            )
            .is_empty()
        );
        // Formulas raras: no se resuelven, pero no rompen.
        assert_eq!(
            origen_de("INDIRECT(\"A\"&B1)"),
            Some(OrigenLista::Otro("INDIRECT(\"A\"&B1)".into()))
        );
        assert_eq!(
            origen_de("MiNombre"),
            Some(OrigenLista::Otro("MiNombre".into()))
        );
        assert_eq!(origen_de("\"sin cerrar"), None);
        assert_eq!(origen_de(""), None);
        assert_eq!(
            origen_de("=Hoja!!!"),
            Some(OrigenLista::Otro("Hoja!!!".into()))
        );
        // Tipo desconocido -> cualquiera, sin lista.
        let v = reglas(r#"<worksheet><dataValidation type="inventado" sqref="A1"/></worksheet>"#);
        assert_eq!(v[0].tipo, TipoValidacion::Cualquiera);
        assert_eq!(v[0].origen_lista(), None);
    }
}
