//! Las tablas de Excel (`xl/tables/tableN.xml`) y sus estilos: el de la
//! propia tabla si el libro lo define, o uno de los integrados
//! (`TableStyleMedium2`...), que no viajan en el fichero y aqui se
//! reconstruyen con los colores del tema (aproximados a la vista).

use crate::estilos::{Dxf, EstiloLinea, EstiloTabla, Linea, Rgb, Tema, con_matiz};
use crate::hoja::Rango;
use crate::xml::Nodo;

#[derive(Debug, Clone, PartialEq)]
pub struct Tabla {
    pub rango: Rango,
    pub estilo: String,
    /// Filas de encabezado (0 o 1) y de totales (0 o 1).
    pub encabezado: u32,
    pub totales: u32,
    pub bandas_filas: bool,
    pub bandas_columnas: bool,
    pub primera_columna: bool,
    pub ultima_columna: bool,
    /// Los botones de filtro del encabezado (`<autoFilter>` de la tabla).
    pub filtro: Option<crate::filtros::AutoFiltro>,
}

/// Lee `<table>`.
pub fn leer(raiz: &Nodo) -> Option<Tabla> {
    let t = raiz.hijo("table")?;
    let rango = Rango::leer(t.attr("ref")?)?;
    let n = |a: &str, def: u32| {
        t.attr(a)
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(def)
            .min(1)
    };
    let info = t.hijo("tableStyleInfo");
    let si = |a: &str| {
        info.and_then(|i| i.attr(a))
            .is_some_and(|v| v == "1" || v == "true")
    };
    Some(Tabla {
        rango,
        estilo: info.and_then(|i| i.attr("name")).unwrap_or("").to_string(),
        encabezado: n("headerRowCount", 1),
        totales: n("totalsRowCount", 0),
        bandas_filas: si("showRowStripes"),
        bandas_columnas: si("showColumnStripes"),
        primera_columna: si("showFirstColumn"),
        ultima_columna: si("showLastColumn"),
        filtro: t
            .hijo("autoFilter")
            .and_then(|a| crate::filtros::AutoFiltro::leer(a, Some(rango))),
    })
}

fn fino(c: Rgb) -> Option<Linea> {
    Some(Linea {
        estilo: EstiloLinea::Fino,
        color: c,
    })
}

fn doble(c: Rgb) -> Option<Linea> {
    Some(Linea {
        estilo: EstiloLinea::Doble,
        color: c,
    })
}

/// Un estilo integrado por su nombre (`TableStyleLight1`..`21`,
/// `TableStyleMedium1`..`28`, `TableStyleDark1`..`11`). Dentro de cada
/// familia de siete, el primero es el negro/gris y los otros seis van con
/// los acentos 1 a 6 del tema.
pub fn integrado(nombre: &str, tema: &Tema) -> Option<EstiloTabla> {
    let (familia, n) = if let Some(r) = nombre.strip_prefix("TableStyleLight") {
        ("light", r.parse::<u32>().ok()?)
    } else if let Some(r) = nombre.strip_prefix("TableStyleMedium") {
        ("medium", r.parse::<u32>().ok()?)
    } else {
        (
            "dark",
            nombre.strip_prefix("TableStyleDark")?.parse::<u32>().ok()?,
        )
    };
    if n == 0 {
        return None;
    }
    let grupo = (n - 1) / 7;
    let k = (n - 1) % 7;
    let negro = tema.colores[1];
    let blanco = tema.colores[0];
    let c = if k == 0 {
        negro
    } else {
        tema.colores[3 + k as usize]
    };
    // Los «negros» de los estilos claros son grises.
    let claro = |t: f64| {
        if k == 0 {
            con_matiz(
                Rgb {
                    r: 0x80,
                    g: 0x80,
                    b: 0x80,
                },
                t,
            )
        } else {
            con_matiz(c, t)
        }
    };
    let mut e = EstiloTabla {
        alto_banda: 1,
        ..Default::default()
    };
    let negrita = Dxf {
        negrita: Some(true),
        ..Default::default()
    };
    match (familia, grupo) {
        ("light", 0) => {
            let texto = if k == 0 { negro } else { con_matiz(c, -0.25) };
            e.todo = Some(Dxf {
                color_fuente: Some(texto),
                arriba: fino(c),
                abajo: fino(c),
                ..Default::default()
            });
            e.encabezado = Some(Dxf {
                negrita: Some(true),
                abajo: fino(c),
                ..Default::default()
            });
            e.total = Some(Dxf {
                negrita: Some(true),
                arriba: fino(c),
                ..Default::default()
            });
            e.banda_fila = Some(Dxf {
                relleno: Some(claro(0.8)),
                ..Default::default()
            });
            e.banda_columna = e.banda_fila.clone();
            e.primera_columna = Some(negrita.clone());
            e.ultima_columna = Some(negrita);
        }
        ("light", 1) => {
            e.todo = Some(Dxf {
                izquierda: fino(c),
                derecha: fino(c),
                arriba: fino(c),
                abajo: fino(c),
                ..Default::default()
            });
            e.encabezado = Some(Dxf {
                relleno: Some(c),
                color_fuente: Some(blanco),
                negrita: Some(true),
                ..Default::default()
            });
            e.total = Some(Dxf {
                negrita: Some(true),
                arriba: doble(c),
                ..Default::default()
            });
            e.banda_fila = Some(Dxf {
                arriba: fino(c),
                abajo: fino(c),
                ..Default::default()
            });
            e.banda_columna = Some(Dxf {
                izquierda: fino(c),
                derecha: fino(c),
                ..Default::default()
            });
            e.primera_columna = Some(negrita.clone());
            e.ultima_columna = Some(negrita);
        }
        ("light", _) => {
            e.todo = Some(Dxf {
                izquierda: fino(c),
                derecha: fino(c),
                arriba: fino(c),
                abajo: fino(c),
                horizontal: fino(c),
                vertical: fino(c),
                ..Default::default()
            });
            e.encabezado = Some(Dxf {
                negrita: Some(true),
                abajo: fino(c),
                ..Default::default()
            });
            e.total = Some(Dxf {
                negrita: Some(true),
                arriba: doble(c),
                ..Default::default()
            });
            e.banda_fila = Some(Dxf {
                relleno: Some(claro(0.8)),
                ..Default::default()
            });
            e.banda_columna = e.banda_fila.clone();
            e.primera_columna = Some(negrita.clone());
            e.ultima_columna = Some(negrita);
        }
        ("medium", 0) => {
            let linea = claro(0.4);
            e.todo = Some(Dxf {
                izquierda: fino(linea),
                derecha: fino(linea),
                arriba: fino(linea),
                abajo: fino(linea),
                horizontal: fino(linea),
                ..Default::default()
            });
            e.encabezado = Some(Dxf {
                relleno: Some(c),
                color_fuente: Some(blanco),
                negrita: Some(true),
                ..Default::default()
            });
            e.total = Some(Dxf {
                negrita: Some(true),
                arriba: doble(c),
                ..Default::default()
            });
            e.banda_fila = Some(Dxf {
                relleno: Some(claro(0.8)),
                ..Default::default()
            });
            e.banda_columna = e.banda_fila.clone();
            e.primera_columna = Some(negrita.clone());
            e.ultima_columna = Some(negrita);
        }
        ("medium", 1) => {
            let blanca = fino(blanco);
            e.todo = Some(Dxf {
                relleno: Some(claro(0.8)),
                horizontal: blanca,
                vertical: blanca,
                ..Default::default()
            });
            e.encabezado = Some(Dxf {
                relleno: Some(c),
                color_fuente: Some(blanco),
                negrita: Some(true),
                abajo: Some(Linea {
                    estilo: EstiloLinea::Grueso,
                    color: blanco,
                }),
                ..Default::default()
            });
            e.total = Some(Dxf {
                relleno: Some(c),
                color_fuente: Some(blanco),
                negrita: Some(true),
                ..Default::default()
            });
            e.banda_fila = Some(Dxf {
                relleno: Some(claro(0.6)),
                ..Default::default()
            });
            e.banda_columna = e.banda_fila.clone();
            e.primera_columna = Some(Dxf {
                relleno: Some(c),
                color_fuente: Some(blanco),
                negrita: Some(true),
                ..Default::default()
            });
            e.ultima_columna = e.primera_columna.clone();
        }
        ("medium", 2) => {
            e.todo = Some(Dxf {
                arriba: fino(negro),
                abajo: fino(negro),
                horizontal: fino(negro),
                ..Default::default()
            });
            e.encabezado = Some(Dxf {
                relleno: Some(c),
                color_fuente: Some(blanco),
                negrita: Some(true),
                ..Default::default()
            });
            e.total = Some(Dxf {
                negrita: Some(true),
                arriba: doble(negro),
                ..Default::default()
            });
            e.banda_fila = Some(Dxf {
                relleno: Some(claro(0.8)),
                ..Default::default()
            });
            e.banda_columna = e.banda_fila.clone();
            e.primera_columna = Some(negrita.clone());
            e.ultima_columna = Some(negrita);
        }
        ("medium", _) => {
            let linea = claro(0.4);
            e.todo = Some(Dxf {
                relleno: Some(claro(0.8)),
                izquierda: fino(linea),
                derecha: fino(linea),
                arriba: fino(linea),
                abajo: fino(linea),
                horizontal: fino(linea),
                vertical: fino(linea),
                ..Default::default()
            });
            e.encabezado = Some(negrita.clone());
            e.total = Some(Dxf {
                negrita: Some(true),
                arriba: doble(c),
                ..Default::default()
            });
            e.banda_fila = Some(Dxf {
                relleno: Some(claro(0.6)),
                ..Default::default()
            });
            e.banda_columna = e.banda_fila.clone();
            e.primera_columna = Some(negrita.clone());
            e.ultima_columna = Some(negrita);
        }
        _ => {
            // Oscuros.
            let cuerpo = if k == 0 {
                Rgb {
                    r: 0x73,
                    g: 0x73,
                    b: 0x73,
                }
            } else {
                con_matiz(c, -0.25)
            };
            e.todo = Some(Dxf {
                relleno: Some(cuerpo),
                color_fuente: Some(blanco),
                ..Default::default()
            });
            e.encabezado = Some(Dxf {
                relleno: Some(negro),
                color_fuente: Some(blanco),
                negrita: Some(true),
                abajo: Some(Linea {
                    estilo: EstiloLinea::Medio,
                    color: blanco,
                }),
                ..Default::default()
            });
            e.total = Some(Dxf {
                relleno: Some(con_matiz(cuerpo, -0.5)),
                negrita: Some(true),
                arriba: doble(blanco),
                ..Default::default()
            });
            e.banda_fila = Some(Dxf {
                relleno: Some(con_matiz(cuerpo, -0.25)),
                ..Default::default()
            });
            e.banda_columna = e.banda_fila.clone();
            e.primera_columna = Some(Dxf {
                negrita: Some(true),
                relleno: Some(con_matiz(cuerpo, -0.25)),
                ..Default::default()
            });
            e.ultima_columna = e.primera_columna.clone();
        }
    }
    Some(e)
}

/// Pone `el` sobre `d` para la celda (`fila`, `col`) de una zona: los lados
/// de fuera usan los bordes de fuera y los de dentro, las lineas interiores.
#[allow(clippy::too_many_arguments)]
fn aplicar(d: &mut Dxf, el: &Dxf, fila: u32, col: u32, f0: u32, f1: u32, c0: u32, c1: u32) {
    let mut e = el.clone();
    e.arriba = if fila == f0 { el.arriba } else { el.horizontal };
    e.abajo = if fila == f1 { el.abajo } else { el.horizontal };
    e.izquierda = if col == c0 { el.izquierda } else { el.vertical };
    e.derecha = if col == c1 { el.derecha } else { el.vertical };
    e.horizontal = None;
    e.vertical = None;
    d.sobre(&e);
}

/// El formato que la tabla da a una de sus celdas (sin el de la celda).
pub fn formato_en(t: &Tabla, e: &EstiloTabla, fila: u32, col: u32) -> Dxf {
    let r = t.rango;
    let mut d = Dxf::default();
    if !r.contiene(fila, col) {
        return d;
    }
    if let Some(w) = &e.todo {
        aplicar(&mut d, w, fila, col, r.fila0, r.fila1, r.col0, r.col1);
    }
    let ini_datos = r.fila0 + t.encabezado;
    let fin_datos = r.fila1.saturating_sub(t.totales);
    let es_enc = t.encabezado > 0 && fila < ini_datos;
    let es_tot = t.totales > 0 && fila > fin_datos;
    let alto = e.alto_banda.max(1);
    if !es_enc && !es_tot && fila >= ini_datos {
        if t.bandas_columnas {
            let i = (col - r.col0) / alto;
            let (c0, c1) = (
                r.col0 + i * alto,
                (r.col0 + i * alto + alto - 1).min(r.col1),
            );
            let el = if i % 2 == 0 {
                &e.banda_columna
            } else {
                &e.banda_columna2
            };
            if let Some(el) = el {
                aplicar(&mut d, el, fila, col, ini_datos, fin_datos, c0, c1);
            }
        }
        if t.bandas_filas {
            let i = (fila - ini_datos) / alto;
            let (f0, f1) = (
                ini_datos + i * alto,
                (ini_datos + i * alto + alto - 1).min(fin_datos),
            );
            let el = if i % 2 == 0 {
                &e.banda_fila
            } else {
                &e.banda_fila2
            };
            if let Some(el) = el {
                aplicar(&mut d, el, fila, col, f0, f1, r.col0, r.col1);
            }
        }
    }
    if t.primera_columna && col == r.col0 {
        if let Some(el) = &e.primera_columna {
            aplicar(&mut d, el, fila, col, r.fila0, r.fila1, col, col);
        }
    }
    if t.ultima_columna && col == r.col1 {
        if let Some(el) = &e.ultima_columna {
            aplicar(&mut d, el, fila, col, r.fila0, r.fila1, col, col);
        }
    }
    if es_enc {
        if let Some(el) = &e.encabezado {
            aplicar(
                &mut d,
                el,
                fila,
                col,
                r.fila0,
                ini_datos - 1,
                r.col0,
                r.col1,
            );
        }
    }
    if es_tot {
        if let Some(el) = &e.total {
            aplicar(
                &mut d,
                el,
                fila,
                col,
                fin_datos + 1,
                r.fila1,
                r.col0,
                r.col1,
            );
        }
    }
    d
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_tabla_mediana_2_tiene_encabezado_de_color_y_bandas() {
        let tema = Tema::default();
        let e = integrado("TableStyleMedium2", &tema).unwrap();
        let t = Tabla {
            rango: Rango {
                fila0: 0,
                col0: 0,
                fila1: 5,
                col1: 2,
            },
            estilo: "TableStyleMedium2".into(),
            encabezado: 1,
            totales: 0,
            bandas_filas: true,
            bandas_columnas: false,
            primera_columna: false,
            ultima_columna: false,
            filtro: None,
        };
        let enc = formato_en(&t, &e, 0, 1);
        assert_eq!(enc.relleno, Some(tema.colores[4]));
        assert_eq!(enc.negrita, Some(true));
        assert!(formato_en(&t, &e, 1, 1).relleno.is_some());
        assert!(formato_en(&t, &e, 2, 1).relleno.is_none());
        assert_eq!(formato_en(&t, &e, 9, 9), Dxf::default());
        assert!(integrado("Cualquiera", &tema).is_none());
        for n in 1..=28 {
            assert!(integrado(&format!("TableStyleMedium{n}"), &tema).is_some());
        }
    }
}
