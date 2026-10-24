//! Minigraficos (`x14:sparklineGroups`): un graficuito dentro de una celda,
//! de linea, de columnas o de ganancia/perdida, con marcadores de alto,
//! bajo, primero, ultimo y negativos.
//!
//! Aqui solo se lee la definicion y se sacan los valores; el dibujo es de
//! `pixpin_render::hoja_office`. Los limites del eje de un grupo se calculan
//! UNA vez por grupo (y se guardan en el [`crate::Libro`]), no por celda.
//!
//! Referencia: gridcraft (https://github.com/storytold/gridcraft),
//! `crates/chart/src/sparkline.rs`, MIT OR Apache-2.0, Copyright (c) 2026
//! ArtCraft Team and the Gridcraft contributors; el formato, de ECMA-376 y
//! [MS-XLSX] 2.6.

use crate::estilos::{Rgb, Tema, color_de};
use crate::hoja::{Hoja, Rango, Valor};
use crate::xml::Nodo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoMini {
    Linea,
    Columna,
    GananciaPerdida,
}

/// Que hacer con las celdas vacias.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vacias {
    Hueco,
    Cero,
    Unir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimiteEje {
    /// El de cada minigrafico.
    Individual,
    /// El de todo el grupo.
    Grupo,
    /// El valor escrito a mano.
    Manual,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GrupoMini {
    pub tipo: TipoMini,
    pub vacias: Vacias,
    /// Un marcador en cada punto (solo lineas).
    pub marcadores: bool,
    pub alto: bool,
    pub bajo: bool,
    pub primero: bool,
    pub ultimo: bool,
    pub negativos: bool,
    /// Grosor de la linea en puntos.
    pub peso: f32,
    pub min_tipo: LimiteEje,
    pub max_tipo: LimiteEje,
    pub min_manual: f64,
    pub max_manual: f64,
    pub eje_x: bool,
    pub derecha_a_izquierda: bool,
    pub c_serie: Rgb,
    pub c_negativo: Rgb,
    pub c_eje: Rgb,
    pub c_marcadores: Rgb,
    pub c_primero: Rgb,
    pub c_ultimo: Rgb,
    pub c_alto: Rgb,
    pub c_bajo: Rgb,
}

/// De donde salen los datos: una hoja (`None` = la misma) y un rango.
#[derive(Debug, Clone, PartialEq)]
pub struct Origen {
    pub hoja: Option<String>,
    pub rango: Rango,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Minigrafico {
    /// La celda donde se pinta.
    pub fila: u32,
    pub col: u32,
    /// Indice en [`crate::hoja::Cola::grupos_mini`].
    pub grupo: u32,
    pub origen: Origen,
}

/// Tope de minigraficos por hoja y de puntos por minigrafico: un archivo
/// hostil no llena la memoria ni cuelga el pintado.
pub const MAX_MINIGRAFICOS: usize = 200_000;
pub const MAX_PUNTOS: usize = 2_000;

fn si(n: &Nodo, a: &str) -> bool {
    matches!(n.attr(a), Some("1") | Some("true"))
}

fn limite(n: &Nodo, a: &str) -> LimiteEje {
    match n.attr(a) {
        Some("group") => LimiteEje::Grupo,
        Some("custom") => LimiteEje::Manual,
        _ => LimiteEje::Individual,
    }
}

/// `Hoja2!A1:E1`, `'Mi hoja'!A1:E1` o `A1:E1`.
pub fn origen_de(f: &str) -> Option<Origen> {
    let f = f.trim();
    let (hoja, r) = match f.rfind('!') {
        Some(i) => {
            let h = f[..i].trim();
            let h = h
                .strip_prefix('\'')
                .and_then(|x| x.strip_suffix('\''))
                .map(|x| x.replace("''", "'"))
                .unwrap_or_else(|| h.to_string());
            (Some(h), &f[i + 1..])
        }
        None => (None, f),
    };
    Some(Origen {
        hoja,
        rango: Rango::leer(r)?,
    })
}

/// Lee los grupos de minigraficos de la cola de una hoja. Devuelve los
/// grupos y los minigraficos ordenados por celda.
pub fn leer(raiz: &Nodo, tema: &Tema, paleta: &[u32]) -> (Vec<GrupoMini>, Vec<Minigrafico>) {
    let mut grupos = Vec::new();
    let mut minis = Vec::new();
    let mut nodos = Vec::new();
    raiz.todos("sparklineGroup", &mut nodos);
    for g in nodos {
        let col = |nombre: &str, defecto: u32| {
            g.hijo(nombre)
                .and_then(|c| color_de(c, tema, paleta))
                .unwrap_or(Rgb::de_u32(defecto))
        };
        let tipo = match g.attr("type") {
            Some("column") => TipoMini::Columna,
            Some("stacked") => TipoMini::GananciaPerdida,
            _ => TipoMini::Linea,
        };
        let vacias = match g.attr("displayEmptyCellsAs") {
            Some("zero") => Vacias::Cero,
            Some("span") => Vacias::Unir,
            _ => Vacias::Hueco,
        };
        let numero = |a: &str| {
            g.attr(a)
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|v| v.is_finite())
                .unwrap_or(0.0)
        };
        let indice = grupos.len() as u32;
        grupos.push(GrupoMini {
            tipo,
            vacias,
            marcadores: si(g, "markers"),
            alto: si(g, "high"),
            bajo: si(g, "low"),
            primero: si(g, "first"),
            ultimo: si(g, "last"),
            negativos: si(g, "negative"),
            peso: g
                .attr("lineWeight")
                .and_then(|v| v.parse::<f32>().ok())
                .filter(|v| *v > 0.0)
                .unwrap_or(0.75)
                .min(10.0),
            min_tipo: limite(g, "minAxisType"),
            max_tipo: limite(g, "maxAxisType"),
            min_manual: numero("manualMin"),
            max_manual: numero("manualMax"),
            eje_x: si(g, "displayXAxis"),
            derecha_a_izquierda: si(g, "rightToLeft"),
            c_serie: col("colorSeries", 0x376092),
            c_negativo: col("colorNegative", 0xD00000),
            c_eje: col("colorAxis", 0x000000),
            c_marcadores: col("colorMarkers", 0xD00000),
            c_primero: col("colorFirst", 0xD00000),
            c_ultimo: col("colorLast", 0xD00000),
            c_alto: col("colorHigh", 0xD00000),
            c_bajo: col("colorLow", 0xD00000),
        });
        let Some(lista) = g.hijo("sparklines") else {
            continue;
        };
        for s in lista.hijos_de("sparkline") {
            if minis.len() >= MAX_MINIGRAFICOS {
                break;
            }
            let Some(origen) = s.hijo("f").and_then(|f| origen_de(&f.texto)) else {
                continue;
            };
            let Some(destino) = s.hijo("sqref").and_then(|f| Rango::leer(&f.texto)) else {
                continue;
            };
            minis.push(Minigrafico {
                fila: destino.fila0,
                col: destino.col0,
                grupo: indice,
                origen,
            });
        }
    }
    minis.sort_by_key(|m| (m.fila, m.col));
    (grupos, minis)
}

/// Los valores de un rango de una hoja, en orden de lectura (por filas). Las
/// celdas sin numero son `None`. Como mucho [`MAX_PUNTOS`].
pub fn valores_de(hoja: &Hoja, r: Rango) -> Vec<Option<f64>> {
    let mut v = Vec::new();
    for fila in r.fila0..=r.fila1.min(r.fila0.saturating_add(MAX_PUNTOS as u32)) {
        let celdas = hoja
            .filas_entre(fila, fila)
            .first()
            .map(|f| hoja.celdas_de(f))
            .unwrap_or(&[]);
        for col in r.col0..=r.col1.min(r.col0.saturating_add(MAX_PUNTOS as u32)) {
            if v.len() >= MAX_PUNTOS {
                return v;
            }
            let x = celdas
                .binary_search_by_key(&col, |c| c.col)
                .ok()
                .and_then(|i| match celdas[i].valor {
                    Valor::Numero(n) if n.is_finite() => Some(n),
                    _ => None,
                });
            v.push(x);
        }
    }
    v
}

/// Los puntos ya resueltos: las vacias segun el grupo y, a la derecha,
/// al reves. `None` es un hueco.
pub fn puntos_de(g: &GrupoMini, valores: &[Option<f64>]) -> Vec<Option<f64>> {
    let mut p: Vec<Option<f64>> = match g.vacias {
        Vacias::Cero => valores.iter().map(|v| Some(v.unwrap_or(0.0))).collect(),
        _ => valores.to_vec(),
    };
    if g.derecha_a_izquierda {
        p.reverse();
    }
    p
}

/// Minimo y maximo de unos puntos (`None` si no hay ninguno).
pub fn extremos(p: &[Option<f64>]) -> Option<(f64, f64)> {
    let mut it = p.iter().flatten().copied();
    let primero = it.next()?;
    Some(it.fold((primero, primero), |(a, b), x| (a.min(x), b.max(x))))
}

/// El eje vertical (minimo, maximo) de un minigrafico: el propio, el de su
/// grupo (`del_grupo`) o el escrito a mano. Las columnas parten de cero. Si
/// el rango queda plano se abre un poco para no dividir por cero.
pub fn eje_vertical(
    g: &GrupoMini,
    propios: Option<(f64, f64)>,
    del_grupo: Option<(f64, f64)>,
) -> Option<(f64, f64)> {
    let base = |tipo: LimiteEje, manual: f64, es_min: bool| -> Option<f64> {
        match tipo {
            LimiteEje::Manual => Some(manual),
            LimiteEje::Grupo => del_grupo
                .or(propios)
                .map(|e| if es_min { e.0 } else { e.1 }),
            LimiteEje::Individual => propios.map(|e| if es_min { e.0 } else { e.1 }),
        }
    };
    let mut lo = base(g.min_tipo, g.min_manual, true)?;
    let mut hi = base(g.max_tipo, g.max_manual, false)?;
    if g.tipo == TipoMini::Columna {
        // Una columna sin negativos arranca en cero; con ellos, abarca el cero.
        if g.min_tipo != LimiteEje::Manual {
            lo = lo.min(0.0);
        }
        if g.max_tipo != LimiteEje::Manual {
            hi = hi.max(0.0);
        }
    }
    if hi < lo {
        std::mem::swap(&mut lo, &mut hi);
    }
    if hi - lo < 1e-12 {
        lo -= 0.5;
        hi += 0.5;
    }
    Some((lo, hi))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::xml;

    const X: &[u8] = br##"<x xmlns:x14="u" xmlns:xm="m"><extLst><ext><x14:sparklineGroups><x14:sparklineGroup type="column" displayEmptyCellsAs="zero" markers="1" high="1" negative="1" lineWeight="1.5" minAxisType="group" maxAxisType="custom" manualMax="9" displayXAxis="1" rightToLeft="1"><x14:colorSeries rgb="FF00B050"/><x14:colorNegative rgb="FFFF0000"/><x14:colorHigh theme="9"/><x14:sparklines><x14:sparkline><xm:f>Datos!B2:F2</xm:f><xm:sqref>G2</xm:sqref></x14:sparkline><x14:sparkline><xm:f>'Mi hoja'!B1:B5</xm:f><xm:sqref>G1</xm:sqref></x14:sparkline><x14:sparkline><xm:f>nada</xm:f><xm:sqref>G3</xm:sqref></x14:sparkline><x14:sparkline><xm:f>A1:C1</xm:f></x14:sparkline></x14:sparklines></x14:sparklineGroup><x14:sparklineGroup type="stacked"><x14:sparklines><x14:sparkline><xm:f>A1:A3</xm:f><xm:sqref>C1</xm:sqref></x14:sparkline></x14:sparklines></x14:sparklineGroup></x14:sparklineGroups></ext></extLst></x>"##;

    #[test]
    fn lee_grupos_y_minigraficos() {
        let (g, m) = leer(&xml::arbol(X), &Tema::default(), &crate::estilos::PALETA);
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].tipo, TipoMini::Columna);
        assert_eq!(g[0].vacias, Vacias::Cero);
        assert!(
            g[0].marcadores
                && g[0].alto
                && g[0].negativos
                && g[0].eje_x
                && g[0].derecha_a_izquierda
        );
        assert!(!g[0].bajo && !g[0].primero);
        assert_eq!(g[0].peso, 1.5);
        assert_eq!(
            (g[0].min_tipo, g[0].max_tipo, g[0].max_manual),
            (LimiteEje::Grupo, LimiteEje::Manual, 9.0)
        );
        assert_eq!(
            g[0].c_serie,
            Rgb {
                r: 0,
                g: 0xB0,
                b: 0x50
            }
        );
        assert_eq!(g[1].tipo, TipoMini::GananciaPerdida);
        assert_eq!(
            g[1].c_serie,
            Rgb {
                r: 0x37,
                g: 0x60,
                b: 0x92
            },
            "color por defecto"
        );
        // Los dos sin destino o sin origen valido se descartan; sale ordenado por celda.
        assert_eq!(m.len(), 3);
        assert_eq!((m[0].fila, m[0].col, m[0].grupo), (0, 2, 1));
        assert_eq!((m[1].fila, m[1].col), (0, 6));
        assert_eq!(
            m[1].origen,
            Origen {
                hoja: Some("Mi hoja".into()),
                rango: Rango {
                    fila0: 0,
                    col0: 1,
                    fila1: 4,
                    col1: 1
                }
            }
        );
        assert_eq!(m[2].origen.hoja.as_deref(), Some("Datos"));
    }

    #[test]
    fn origenes_raros() {
        assert_eq!(
            origen_de("A1").unwrap().rango,
            Rango {
                fila0: 0,
                col0: 0,
                fila1: 0,
                col1: 0
            }
        );
        assert_eq!(
            origen_de("'O''Neil'!C3:D4").unwrap().hoja.as_deref(),
            Some("O'Neil")
        );
        assert!(origen_de("Hoja1!").is_none());
        assert!(origen_de("").is_none());
        assert!(origen_de("!!!").is_none());
    }

    #[test]
    fn eje_de_columnas_parte_de_cero_y_el_plano_se_abre() {
        let (g, _) = leer(&xml::arbol(X), &Tema::default(), &crate::estilos::PALETA);
        // Columna, minimo del grupo, maximo a mano (9).
        let e = eje_vertical(&g[0], Some((2.0, 5.0)), Some((1.0, 20.0))).unwrap();
        assert_eq!(e, (0.0, 9.0));
        // Sin datos no hay eje.
        assert_eq!(eje_vertical(&g[1], None, None), None);
        // Todos iguales: se abre.
        let mut g1 = g[1].clone();
        g1.tipo = TipoMini::Linea;
        let (lo, hi) = eje_vertical(&g1, Some((4.0, 4.0)), None).unwrap();
        assert!(hi > lo && (lo + hi) / 2.0 == 4.0);
    }

    #[test]
    fn puntos_con_vacias_y_al_reves() {
        let (g, _) = leer(&xml::arbol(X), &Tema::default(), &crate::estilos::PALETA);
        let v = [Some(1.0), None, Some(3.0)];
        // Cero y derecha a izquierda.
        assert_eq!(puntos_de(&g[0], &v), vec![Some(3.0), Some(0.0), Some(1.0)]);
        // Hueco: se queda None.
        assert_eq!(puntos_de(&g[1], &v), v.to_vec());
        assert_eq!(extremos(&v), Some((1.0, 3.0)));
        assert_eq!(extremos(&[None, None]), None);
    }

    #[test]
    fn valores_de_un_rango_de_la_hoja() {
        let mut h = Hoja::vacia_para_pruebas();
        h.poner_numeros_para_pruebas(&[5.0, 6.0, 7.0]);
        let v = valores_de(
            &h,
            Rango {
                fila0: 0,
                col0: 0,
                fila1: 3,
                col1: 0,
            },
        );
        assert_eq!(v, vec![Some(5.0), Some(6.0), Some(7.0), None]);
        let v = valores_de(
            &h,
            Rango {
                fila0: 0,
                col0: 1,
                fila1: 0,
                col1: 2,
            },
        );
        assert_eq!(v, vec![None, None]);
        // Un rango enorme se corta.
        let v = valores_de(
            &h,
            Rango {
                fila0: 0,
                col0: 0,
                fila1: 1_000_000,
                col1: 3,
            },
        );
        assert!(v.len() <= MAX_PUNTOS);
    }
}
