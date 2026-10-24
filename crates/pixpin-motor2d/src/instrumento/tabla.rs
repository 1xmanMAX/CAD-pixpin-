//! **La tabla de coordenadas**: puntos tecleados, no dibujados a pulso.
//!
//! Puerto de `Tablas.kt` (y del pintado de `Renderer.drawTabla`) de PixPin
//! Android. Las tablas no son elementos: viven en la escena (`tablas`) con un
//! **eje comun para todo el dibujo** (`origenCoordenadas`), se distinguen por
//! su color y se editan desde su tabla. Aqui solo se dice donde caen sus
//! puntos y como se pintan; son los puntos que pone el instrumento de
//! graficar (ver el modulo padre): la tabla aporta los datos, el mismo
//! pintado de ordenes los dibuja en pantalla, en PNG, SVG, PDF y la web.
//!
//! El formato es el del movil, con sus mismos nombres (`id`, `nombre`,
//! `color`, `puntos: [{x, y}]`, `visible`): `TablaDeCoordenadas` de
//! `Tablas.kt`.

use serde::{Deserialize, Serialize};

use crate::elemento::{ColorRgba, EstiloTrazo};
use crate::medida::Escala;
use crate::pintado::Orden;
use crate::vector::Punto2;

/// Una serie de puntos tecleados (`TablaDeCoordenadas`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TablaDeCoordenadas {
    pub id: String,
    #[serde(default)]
    pub nombre: String,
    /// `#rrggbb`, el de sus puntos.
    #[serde(default = "primer_color")]
    pub color: String,
    #[serde(default)]
    pub puntos: Vec<PuntoDeTabla>,
    /// Apagada no se ve, pero no se pierde lo tecleado.
    #[serde(default = "verdadero")]
    pub visible: bool,
}

/// Una fila de la tabla (`PuntoDeTabla`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct PuntoDeTabla {
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
}

fn primer_color() -> String {
    COLORES_DE_TABLA[0].to_string()
}

fn verdadero() -> bool {
    true
}

/// Los colores de las series, repartidos por el circulo cromatico
/// (`COLORES_DE_TABLA`).
pub const COLORES_DE_TABLA: [&str; 6] = [
    "#e03131", "#1971c2", "#2f9e44", "#f08c00", "#9c36b5", "#0c8599",
];

/// El siguiente color libre (`colorLibreDeTabla`).
pub fn color_libre(usados: &[String]) -> &'static str {
    COLORES_DE_TABLA
        .iter()
        .find(|c| !usados.iter().any(|u| u.eq_ignore_ascii_case(c)))
        .copied()
        .unwrap_or(COLORES_DE_TABLA[0])
}

/// Radio del punto y tamano de su numero, en pixeles de pantalla
/// (`TABLA_PUNTO`, `TABLA_LETRA`); medio brazo de la cruz del origen y su
/// trazo (`TABLA_ORIGEN`, `TABLA_TRAZO`); el trazo del eje (`EJE_TRAZO`).
pub const TABLA_PUNTO: f32 = 6.5;
pub const TABLA_LETRA: f32 = 30.0;
pub const TABLA_ORIGEN: f32 = 14.0;
pub const TABLA_TRAZO: f32 = 1.5;
pub const EJE_TRAZO: f32 = 1.0;
/// El halo de las letras, en veces su tamano (`PUNTO_HALO`).
const PUNTO_HALO: f32 = 0.22;
/// Al exportar, los adornos se toman en pixeles de escena y a la mitad
/// (`encogeAlExportar`): en un A4 un punto de pantalla saldria enorme.
const ENCOGE_AL_EXPORTAR: f32 = 0.5;
/// El gris del eje: es de todas las series (`EJE_COLOR`).
pub const EJE_COLOR: ColorRgba = ColorRgba {
    r: 90.0 / 255.0,
    g: 90.0 / 255.0,
    b: 96.0 / 255.0,
    a: 1.0,
};

/// **Donde cae un punto de la tabla en la escena** (`puntoEnEscena`): la y
/// hacia arriba, y en las unidades de la escala si se calibro (si no, un
/// punto es un pixel).
pub fn punto_en_escena(origen: Punto2, p: PuntoDeTabla, escala: Option<&Escala>) -> Punto2 {
    let por_unidad = match escala {
        Some(e) if e.valida() => 1.0 / e.unidades_por_pixel as f64,
        _ => 1.0,
    };
    Punto2::nuevo(
        (origen.x as f64 + p.x * por_unidad) as f32,
        (origen.y as f64 - p.y * por_unidad) as f32,
    )
}

/// Los de una tabla, en el orden en que se teclearon (`puntosEnEscena`).
pub fn puntos_en_escena(
    t: &TablaDeCoordenadas,
    origen: Punto2,
    escala: Option<&Escala>,
) -> Vec<Punto2> {
    t.puntos
        .iter()
        .map(|p| punto_en_escena(origen, *p, escala))
        .collect()
}

/// Los puntos de todas las tablas que se ven: los anclajes del iman
/// (`anclajesDeTablas`). Sin origen no hay ninguno.
pub fn anclajes(
    tablas: &[TablaDeCoordenadas],
    origen: Option<Punto2>,
    escala: Option<&Escala>,
) -> Vec<Punto2> {
    let Some(o) = origen else {
        return Vec::new();
    };
    tablas
        .iter()
        .filter(|t| t.visible)
        .flat_map(|t| puntos_en_escena(t, o, escala))
        .collect()
}

/// Un color `#rrggbb` del fichero; uno roto se pinta del primero de la lista
/// (`parseColor` con su reserva).
pub fn color_de(texto: &str) -> ColorRgba {
    crate::excalidraw::color_desde(Some(&serde_json::Value::String(texto.to_string()))).unwrap_or(
        ColorRgba {
            r: 0xe0 as f32 / 255.0,
            g: 0x31 as f32 / 255.0,
            b: 0x31 as f32 / 255.0,
            a: 1.0,
        },
    )
}

/// **Lo que se pinta de las tablas.** `zoom` es el aumento de la pantalla
/// (los adornos miden lo mismo a cualquier aumento) o `None` al exportar:
/// entonces se toman en pixeles de escena a la mitad y el eje, que es
/// andamio, no sale. `vista` es lo que se ve, para tirar las dos rectas del
/// eje de lado a lado.
pub fn ordenes(
    tablas: &[TablaDeCoordenadas],
    origen: Option<Punto2>,
    escala: Option<&Escala>,
    zoom: Option<f32>,
    vista: Option<(f32, f32, f32, f32)>,
) -> Vec<Orden> {
    let Some(o) = origen else {
        return Vec::new();
    };
    let mut v = Vec::new();
    let (k, encoge) = match zoom {
        Some(z) => (1.0 / z.max(1e-4), 1.0),
        None => (1.0, ENCOGE_AL_EXPORTAR),
    };
    // El eje, una vez y en gris, solo en pantalla: a trazos de lado a lado de
    // lo que se ve y el cero solido encima.
    if zoom.is_some() {
        if let Some((x0, y0, x1, y1)) = vista {
            for puntos in [
                vec![Punto2::nuevo(x0, o.y), Punto2::nuevo(x1, o.y)],
                vec![Punto2::nuevo(o.x, y0), Punto2::nuevo(o.x, y1)],
            ] {
                v.push(Orden::Polilinea {
                    puntos,
                    color: EJE_COLOR,
                    grosor: EJE_TRAZO * k,
                    estilo: EstiloTrazo::Discontinuo,
                });
            }
        }
        let brazo = TABLA_ORIGEN * k;
        for (a, b) in [((-brazo, 0.0), (brazo, 0.0)), ((0.0, -brazo), (0.0, brazo))] {
            v.push(Orden::Polilinea {
                puntos: vec![
                    Punto2::nuevo(o.x + a.0, o.y + a.1),
                    Punto2::nuevo(o.x + b.0, o.y + b.1),
                ],
                color: EJE_COLOR,
                grosor: TABLA_TRAZO * k,
                estilo: EstiloTrazo::Solido,
            });
        }
    }
    let radio = TABLA_PUNTO * k * encoge;
    let tam = TABLA_LETRA * k * encoge;
    for t in tablas.iter().filter(|t| t.visible) {
        let color = color_de(&t.color);
        for (n, p) in puntos_en_escena(t, o, escala).into_iter().enumerate() {
            // Con aro blanco: sobre una foto oscura un punto de color se pierde.
            v.push(Orden::Relleno {
                puntos: circulo(p, radio * 1.55),
                color: ColorRgba::opaco(1.0, 1.0, 1.0),
            });
            v.push(Orden::Relleno {
                puntos: circulo(p, radio),
                color,
            });
            // `1'`, `2'`... por su sitio en la tabla, arriba a la derecha.
            let base = 1.0 * tam;
            v.push(Orden::Rotulo {
                texto: format!("{}'", n + 1),
                x: p.x + radio * 1.8,
                y: p.y - radio * 1.4 - base,
                tam,
                familia: crate::texto::FAMILIA_DEL_SISTEMA.to_string(),
                color,
                halo: crate::contraste::color_de_halo(color),
                grosor_halo: tam * PUNTO_HALO,
                centro: p,
                angulo: 0.0,
            });
        }
    }
    v
}

fn circulo(c: Punto2, r: f32) -> Vec<Punto2> {
    const LADOS: usize = 20;
    (0..LADOS)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / LADOS as f32;
            Punto2::nuevo(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect()
}

/// Lee un numero tecleado, con coma o punto y con signo (`leerCoordenada`).
pub fn leer_coordenada(texto: &str) -> Option<f64> {
    texto
        .trim()
        .replace(',', ".")
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
}

/// **Una tabla copiada de Excel, Sheets, un CSV o un PDF** (`leerTablaPegada`):
/// de cada fila, las dos primeras columnas que sean numeros. La cabecera, una
/// linea en blanco o un total se caen en silencio.
pub fn leer_tabla_pegada(texto: &str) -> Vec<PuntoDeTabla> {
    let lineas: Vec<&str> = texto
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let Some(sep) = separador_de(&lineas) else {
        return Vec::new();
    };
    lineas
        .iter()
        .filter_map(|l| {
            let numeros: Vec<f64> = partir(l, sep)
                .into_iter()
                .filter_map(|c| leer_numero_de_hoja(c.trim().trim_matches('|')))
                .collect();
            (numeros.len() >= 2).then(|| PuntoDeTabla {
                x: numeros[0],
                y: numeros[1],
            })
        })
        .collect()
}

#[derive(Clone, Copy)]
enum Separador {
    Tab,
    PuntoYComa,
    Barra,
    Espacios,
    Coma,
}

fn partir(l: &str, s: Separador) -> Vec<&str> {
    match s {
        Separador::Tab => l.split('\t').collect(),
        Separador::PuntoYComa => l.split(';').collect(),
        Separador::Barra => l.split('|').collect(),
        Separador::Coma => l.split(',').collect(),
        Separador::Espacios => {
            // Dos espacios o mas parten; uno solo es parte del campo.
            let mut v = Vec::new();
            let mut desde = 0;
            let b = l.as_bytes();
            let mut i = 0;
            while i < b.len() {
                if b[i] == b' ' && i + 1 < b.len() && b[i + 1] == b' ' {
                    v.push(&l[desde..i]);
                    while i < b.len() && b[i] == b' ' {
                        i += 1;
                    }
                    desde = i;
                } else {
                    i += 1;
                }
            }
            v.push(&l[desde..]);
            v
        }
    }
}

/// Con que se separan las columnas, del mas seguro al menos (`separadorDe`).
/// La coma va la ultima: en espanol es la decimal.
fn separador_de(lineas: &[&str]) -> Option<Separador> {
    if lineas.iter().any(|l| l.contains('\t')) {
        return Some(Separador::Tab);
    }
    if lineas.iter().any(|l| l.contains(';')) {
        return Some(Separador::PuntoYComa);
    }
    if lineas.iter().any(|l| l.contains('|')) {
        return Some(Separador::Barra);
    }
    let dos_espacios = |l: &str| {
        let b = l.as_bytes();
        (1..b.len().saturating_sub(2)).any(|i| {
            b[i - 1] != b' ' && b[i] == b' ' && b[i + 1] == b' ' && {
                let mut j = i + 1;
                while j < b.len() && b[j] == b' ' {
                    j += 1;
                }
                j < b.len()
            }
        })
    };
    if lineas.iter().any(|l| dos_espacios(l)) {
        return Some(Separador::Espacios);
    }
    if lineas.iter().any(|l| {
        let trozos: Vec<&str> = l.split(',').collect();
        trozos.len() >= 3 || (trozos.len() == 2 && trozos.iter().any(|t| t.contains('.')))
    }) {
        return Some(Separador::Coma);
    }
    None
}

/// Un numero de hoja de calculo venga del pais que venga
/// (`leerNumeroDeHoja`): manda el ultimo signo de puntuacion; tiene que
/// empezar por cifra o signo (una columna de nombres no entra) y se tira la
/// unidad de detras.
pub fn leer_numero_de_hoja(campo: &str) -> Option<f64> {
    let texto = campo.trim().trim_matches(['"', '\'']).trim();
    let primero = texto.chars().next()?;
    if !(primero.is_ascii_digit() || matches!(primero, '-' | '+' | '.' | ',')) {
        return None;
    }
    let limpio: String = texto
        .chars()
        .take_while(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '-' | '+'))
        .collect();
    if !limpio.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    let coma = limpio.rfind(',');
    let punto = limpio.rfind('.');
    let normal = match (coma, punto) {
        (Some(c), Some(p)) if c > p => limpio.replace('.', "").replace(',', "."),
        (Some(_), Some(_)) => limpio.replace(',', ""),
        (Some(_), None) => limpio.replace(',', "."),
        _ => limpio,
    };
    normal.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Un numero escrito para volver a teclearlo, sin ceros de mas (`numero`).
pub fn numero(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    // Los casos de `TablasTest.kt`.

    #[test]
    fn la_y_va_hacia_arriba_y_sin_escala_un_punto_es_un_pixel() {
        let o = Punto2::nuevo(100.0, 100.0);
        let p = punto_en_escena(o, PuntoDeTabla { x: 0.0, y: 10.0 }, None);
        assert_eq!((p.x, p.y), (100.0, 90.0));
    }

    #[test]
    fn con_escala_las_unidades_son_las_del_dibujo() {
        let e = Escala::calibrando(100.0, 2.0, "m", 2).unwrap(); // 0,02 m por px
        let p = punto_en_escena(
            Punto2::nuevo(0.0, 0.0),
            PuntoDeTabla { x: 4.0, y: 0.0 },
            Some(&e),
        );
        assert!((p.x - 200.0).abs() < 1e-3);
    }

    #[test]
    fn sin_origen_no_hay_ni_puntos_ni_anclajes() {
        let t = TablaDeCoordenadas {
            id: "t".into(),
            nombre: String::new(),
            color: "#1971c2".into(),
            puntos: vec![PuntoDeTabla { x: 1.0, y: 1.0 }],
            visible: true,
        };
        assert!(anclajes(std::slice::from_ref(&t), None, None).is_empty());
        assert!(ordenes(&[t], None, None, Some(1.0), None).is_empty());
    }

    #[test]
    fn una_tabla_escondida_no_se_pinta_ni_engancha() {
        let t = TablaDeCoordenadas {
            id: "t".into(),
            nombre: String::new(),
            color: "#1971c2".into(),
            puntos: vec![PuntoDeTabla { x: 1.0, y: 1.0 }],
            visible: false,
        };
        let o = Some(Punto2::nuevo(0.0, 0.0));
        assert!(anclajes(std::slice::from_ref(&t), o, None).is_empty());
        // Exportando, sin eje y sin la tabla escondida: nada.
        assert!(ordenes(&[t], o, None, None, None).is_empty());
    }

    #[test]
    fn el_eje_es_de_la_pantalla_y_los_puntos_salen_tambien_al_exportar() {
        let t = TablaDeCoordenadas {
            id: "t".into(),
            nombre: String::new(),
            color: "#2f9e44".into(),
            puntos: vec![
                PuntoDeTabla { x: 1.0, y: 2.0 },
                PuntoDeTabla { x: 3.0, y: 4.0 },
            ],
            visible: true,
        };
        let o = Some(Punto2::nuevo(0.0, 0.0));
        let fuera = ordenes(std::slice::from_ref(&t), o, None, None, None);
        assert!(!fuera.iter().any(|o| matches!(o, Orden::Polilinea { .. })));
        let rotulos: Vec<String> = fuera
            .iter()
            .filter_map(|o| match o {
                Orden::Rotulo { texto, .. } => Some(texto.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(rotulos, ["1'", "2'"]);
        let dentro = ordenes(&[t], o, None, Some(2.0), Some((-50.0, -50.0, 50.0, 50.0)));
        assert_eq!(
            dentro
                .iter()
                .filter(|o| matches!(o, Orden::Polilinea { .. }))
                .count(),
            4
        );
    }

    #[test]
    fn el_color_libre_no_repite_y_se_acaba_volviendo_al_primero() {
        assert_eq!(color_libre(&[]), "#e03131");
        assert_eq!(color_libre(&["#E03131".into()]), "#1971c2");
        let todos: Vec<String> = COLORES_DE_TABLA.iter().map(|c| c.to_string()).collect();
        assert_eq!(color_libre(&todos), "#e03131");
    }

    #[test]
    fn pegar_de_excel_con_columna_de_nombres_y_cabecera() {
        let t = "Punto\tX\tY\nP1\t1,5\t2\nP2\t-3\t4,25\nTotal\t\t\n";
        assert_eq!(
            leer_tabla_pegada(t),
            vec![
                PuntoDeTabla { x: 1.5, y: 2.0 },
                PuntoDeTabla { x: -3.0, y: 4.25 }
            ]
        );
    }

    #[test]
    fn pegar_csv_ingles_y_europeo_y_columnas_de_pdf() {
        assert_eq!(
            leer_tabla_pegada("1.5,2.5\n3.0,4"),
            vec![
                PuntoDeTabla { x: 1.5, y: 2.5 },
                PuntoDeTabla { x: 3.0, y: 4.0 }
            ]
        );
        assert_eq!(
            leer_tabla_pegada("1,5;2\n"),
            vec![PuntoDeTabla { x: 1.5, y: 2.0 }]
        );
        assert_eq!(
            leer_tabla_pegada("Est. 3   10   20"),
            vec![PuntoDeTabla { x: 10.0, y: 20.0 }]
        );
    }

    #[test]
    fn un_texto_sin_pares_no_trae_nada() {
        // Caso negativo: `12,5` a secas es un numero, no dos columnas.
        assert!(leer_tabla_pegada("12,5\n7").is_empty());
        assert!(leer_tabla_pegada("").is_empty());
        assert!(leer_tabla_pegada("hola que tal").is_empty());
    }

    #[test]
    fn los_numeros_de_hoja_de_cualquier_pais() {
        assert_eq!(leer_numero_de_hoja("1.234,56"), Some(1234.56));
        assert_eq!(leer_numero_de_hoja("1,234.56"), Some(1234.56));
        assert_eq!(leer_numero_de_hoja("12,5 m"), Some(12.5));
        assert_eq!(leer_numero_de_hoja("P1"), None);
        assert_eq!(leer_numero_de_hoja("-"), None);
    }

    #[test]
    fn la_coordenada_tecleada_admite_coma_y_signo() {
        assert_eq!(leer_coordenada(" -4,5 "), Some(-4.5));
        assert_eq!(leer_coordenada("4,"), Some(4.0));
        assert_eq!(leer_coordenada("-"), None);
        assert_eq!(numero(3.0), "3");
        assert_eq!(numero(-0.5), "-0.5");
    }
}
