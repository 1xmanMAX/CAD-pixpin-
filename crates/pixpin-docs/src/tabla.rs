//! **Las tablas, de verdad**: celdas con su columna, su ancho y sus uniones.
//!
//! El movil pinta las tablas del Word como una `<table>` de HTML
//! (`DocxAHtml.kt`, `tabla`): una celda por `w:tc`, `colspan` con el
//! `w:gridSpan`, la continuacion de una celda unida hacia abajo (`w:vMerge`)
//! como celda vacia, raya de 1 px alrededor de cada celda y aire de 4x8 px
//! dentro. El lector del PC no tiene navegador: aqui se guarda esa misma
//! estructura para que `apps/pixpin/src/visor.rs` coloque cada celda en su
//! sitio, en vez de leer la fila «en una linea» como hasta ahora.
//!
//! **Los anchos**. El `WebView` reparte las columnas por lo que ocupa el
//! texto (`table-layout:auto`). Un Word ya trae el reparto hecho por Word en
//! su rejilla (`w:tblGrid`/`w:gridCol`, en veintavos de punto): se respeta
//! esa proporcion sobre el ancho de texto de la pagina (`w:sectPr`), que es
//! lo que el usuario ve en Word, y encima se aplica la regla del navegador
//! de que **ninguna columna baja de su palabra mas larga** (con otra letra y
//! otra columna, «Cerrado» no debe partirse en «Cerra-do»). Sin rejilla (un
//! libro, una pagina, un Word sin ella) se reparte como el navegador: cada
//! columna a lo que ocupa su texto si cabe, y si no, entre su minimo y su
//! maximo.
//!
//! Aqui no se mide nada: quien llama da los anchos medidos (DirectWrite en
//! el lector) y esto solo reparte. Por eso se prueba sin ventana.

use crate::documento::{Bloque, Clase, SEPARADOR_DE_CELDA, Trozo};

/// Una celda de una fila.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Celda {
    /// Su texto; los parrafos de dentro van separados por `\n`.
    pub trozos: Vec<Trozo>,
    /// Cuantas columnas de la rejilla ocupa (`w:gridSpan`), al menos 1.
    pub columnas: u16,
    /// Es la continuacion de una celda unida hacia abajo (`w:vMerge` sin
    /// `restart`): no trae nada suyo y no lleva raya arriba.
    pub sigue: bool,
    /// Lleva fondo (`w:shd` con color). El lector lo pinta como un velo
    /// suave, que el papel es oscuro y el gris de Word no se leeria.
    pub relleno: bool,
}

impl Celda {
    pub fn llana(trozos: Vec<Trozo>) -> Self {
        Self {
            trozos,
            columnas: 1,
            sigue: false,
            relleno: false,
        }
    }

    pub fn texto(&self) -> String {
        self.trozos.iter().map(|t| t.texto.as_str()).collect()
    }
}

/// Lo que sabe una fila de su tabla.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FilaDeTabla {
    /// Que tabla del documento es (las filas seguidas con el mismo numero
    /// son una tabla; dos tablas pegadas no se mezclan).
    pub tabla: u32,
    /// La rejilla de Word, en veintavos de punto; vacia si no la hay.
    pub rejilla: Vec<u32>,
    /// El ancho de texto de la pagina, en las mismas unidades; 0 si no se
    /// sabe.
    pub pagina: u32,
    pub celdas: Vec<Celda>,
}

/// El ancho de texto de una pagina de Word cuando el documento no lo dice:
/// carta con margenes de una pulgada, lo que Word pone por defecto.
pub const PAGINA_POR_DEFECTO: u32 = 12_240 - 2 * 1_440;

/// Las celdas de una fila. Un Word las trae contadas; un libro o una pagina
/// solo traen el texto con [`SEPARADOR_DE_CELDA`], y se parte por ahi.
pub fn celdas_de(b: &Bloque) -> Vec<Celda> {
    if let Some(f) = &b.fila {
        return f.celdas.clone();
    }
    let mut celdas = vec![Celda::llana(Vec::new())];
    for t in &b.trozos {
        let mut resto = t.texto.as_str();
        loop {
            match resto.split_once(SEPARADOR_DE_CELDA) {
                Some((antes, despues)) => {
                    if !antes.is_empty() {
                        celdas
                            .last_mut()
                            .expect("siempre hay una")
                            .trozos
                            .push(Trozo {
                                texto: antes.to_string(),
                                estilo: t.estilo,
                            });
                    }
                    celdas.push(Celda::llana(Vec::new()));
                    resto = despues;
                }
                None => {
                    if !resto.is_empty() {
                        celdas
                            .last_mut()
                            .expect("siempre hay una")
                            .trozos
                            .push(Trozo {
                                texto: resto.to_string(),
                                estilo: t.estilo,
                            });
                    }
                    break;
                }
            }
        }
    }
    celdas
}

/// Si dos filas son de la misma tabla: el mismo numero en un Word, o dos
/// filas seguidas sin numero (libro, pagina).
pub fn misma_tabla(a: &Bloque, b: &Bloque) -> bool {
    if a.clase != Clase::Fila || b.clase != Clase::Fila {
        return false;
    }
    match (&a.fila, &b.fila) {
        (Some(x), Some(y)) => x.tabla == y.tabla,
        (None, None) => true,
        _ => false,
    }
}

/// Cuantas columnas tiene la rejilla de estas filas: la fila mas ancha,
/// contando lo que ocupa cada celda.
pub fn columnas_de(filas: &[Vec<Celda>]) -> usize {
    filas
        .iter()
        .map(|f| f.iter().map(|c| c.columnas.max(1) as usize).sum::<usize>())
        .max()
        .unwrap_or(0)
        .max(1)
}

/// Los anchos que da la rejilla de Word, en unidades del lector, si la hay
/// y cuadra con las columnas: la proporcion de cada columna sobre el ancho
/// de texto de la pagina, aplicada a la columna del lector.
pub fn anchos_de_word(
    fila: Option<&FilaDeTabla>,
    columnas: usize,
    columna: f32,
) -> Option<Vec<f32>> {
    let f = fila?;
    if f.rejilla.len() != columnas || f.rejilla.iter().all(|w| *w == 0) {
        return None;
    }
    let total: u32 = f.rejilla.iter().sum();
    // Una pagina que no se sabe, o mas estrecha que la tabla dibujada en
    // ella (tabla que se sale del margen en Word): se toma la que haya.
    let pagina = if f.pagina > 0 {
        f.pagina
    } else {
        PAGINA_POR_DEFECTO.max(total)
    };
    Some(
        f.rejilla
            .iter()
            .map(|w| *w as f32 / pagina as f32 * columna)
            .collect(),
    )
}

/// **Reparte el ancho entre las columnas.**
///
/// - `minimo[i]`: lo que ocupa la palabra mas larga de la columna (con su
///   aire): menos que eso parte palabras.
/// - `maximo[i]`: lo que ocupa su parrafo mas largo en una sola linea.
/// - `preferidas`: los anchos de Word, si los hay.
/// - `disponible`: la columna de lectura. `tope`: hasta donde puede
///   desplegarse una tabla ancha (la columna mas su margen derecho, §2.8 de
///   la lista: la tabla se pinta entera y ocupa el margen). Mas alla, se
///   estrecha todo por igual y el texto se envuelve.
pub fn repartir(
    minimo: &[f32],
    maximo: &[f32],
    preferidas: Option<&[f32]>,
    disponible: f32,
    tope: f32,
) -> Vec<f32> {
    let n = minimo.len();
    if n == 0 {
        return Vec::new();
    }
    let maximo: Vec<f32> = (0..n)
        .map(|i| maximo.get(i).copied().unwrap_or(0.0).max(minimo[i]))
        .collect();
    let suma_min: f32 = minimo.iter().sum();
    let mut anchos: Vec<f32> = match preferidas {
        Some(p) if p.len() == n => {
            let mut a: Vec<f32> = p.to_vec();
            // Ninguna por debajo de su palabra: lo que falta se quita de
            // las que tienen holgura, en proporcion a ella.
            let falta: f32 = (0..n).map(|i| (minimo[i] - a[i]).max(0.0)).sum();
            if falta > 0.0 {
                let holgura: f32 = (0..n).map(|i| (a[i] - minimo[i]).max(0.0)).sum();
                let quitar = falta.min(holgura);
                for i in 0..n {
                    if a[i] < minimo[i] {
                        a[i] = minimo[i];
                    } else if holgura > 0.0 {
                        a[i] -= (a[i] - minimo[i]) / holgura * quitar;
                    }
                }
            }
            a
        }
        _ => {
            let suma_max: f32 = maximo.iter().sum();
            if suma_max <= disponible {
                maximo.clone()
            } else if suma_min >= disponible {
                minimo.to_vec()
            } else {
                let k = (disponible - suma_min) / (suma_max - suma_min);
                (0..n)
                    .map(|i| minimo[i] + (maximo[i] - minimo[i]) * k)
                    .collect()
            }
        }
    };
    let total: f32 = anchos.iter().sum();
    if total > tope && total > 0.0 {
        let k = tope / total;
        for a in &mut anchos {
            *a *= k;
        }
    }
    anchos
}

/// La palabra mas larga de un texto (por letras): la que marca el minimo
/// de su columna. Se mide solo esa, no todas.
pub fn palabra_mas_larga(texto: &str) -> &str {
    texto
        .split(|c: char| c.is_whitespace())
        .max_by_key(|p| p.chars().count())
        .unwrap_or("")
}

/// El parrafo mas largo de una celda (por letras): el que marca su maximo.
pub fn parrafo_mas_largo(texto: &str) -> &str {
    texto
        .split('\n')
        .max_by_key(|p| p.chars().count())
        .unwrap_or("")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn fila(textos: &[&str]) -> Bloque {
        let t = textos.join(SEPARADOR_DE_CELDA);
        Bloque::nuevo(Clase::Fila, vec![Trozo::llano(t)])
    }

    #[test]
    fn una_fila_de_un_libro_se_parte_en_sus_celdas_por_el_separador() {
        let c = celdas_de(&fila(&["a", "", "bb"]));
        let t: Vec<String> = c.iter().map(|c| c.texto()).collect();
        assert_eq!(t, vec!["a", "", "bb"], "la celda vacia tambien cuenta");
        assert!(c.iter().all(|c| c.columnas == 1 && !c.sigue));
    }

    #[test]
    fn un_texto_sin_separador_es_una_sola_celda() {
        let c = celdas_de(&fila(&["solo"]));
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].texto(), "solo");
    }

    #[test]
    fn dos_tablas_de_word_pegadas_no_se_mezclan() {
        let mut a = fila(&["a"]);
        let mut b = fila(&["b"]);
        a.fila = Some(FilaDeTabla {
            tabla: 0,
            ..Default::default()
        });
        b.fila = Some(FilaDeTabla {
            tabla: 1,
            ..Default::default()
        });
        assert!(!misma_tabla(&a, &b));
        b.fila.as_mut().unwrap().tabla = 0;
        assert!(misma_tabla(&a, &b));
        let p = Bloque::nuevo(Clase::Parrafo, vec![Trozo::llano("x")]);
        assert!(!misma_tabla(&a, &p), "un parrafo no es una fila");
    }

    #[test]
    fn la_rejilla_cuenta_lo_que_ocupa_cada_celda() {
        let mut ancha = Celda::llana(vec![]);
        ancha.columnas = 2;
        let filas = vec![
            vec![ancha],
            vec![
                Celda::llana(vec![]),
                Celda::llana(vec![]),
                Celda::llana(vec![]),
            ],
        ];
        assert_eq!(columnas_de(&filas), 3);
        assert_eq!(columnas_de(&[]), 1, "nunca cero columnas");
    }

    #[test]
    fn los_anchos_de_word_guardan_la_proporcion_de_la_pagina() {
        let f = FilaDeTabla {
            rejilla: vec![500, 1500],
            pagina: 4000,
            ..Default::default()
        };
        let a = anchos_de_word(Some(&f), 2, 800.0).unwrap();
        assert_eq!(
            a,
            vec![100.0, 300.0],
            "media pagina en Word, media columna aqui"
        );
        // Casos negativos: rejilla que no cuadra con las celdas, o vacia.
        assert!(anchos_de_word(Some(&f), 3, 800.0).is_none());
        assert!(anchos_de_word(None, 2, 800.0).is_none());
        let sin = FilaDeTabla {
            rejilla: vec![0, 0],
            ..Default::default()
        };
        assert!(anchos_de_word(Some(&sin), 2, 800.0).is_none());
    }

    #[test]
    fn con_los_anchos_de_word_ninguna_columna_baja_de_su_palabra_mas_larga() {
        // Como la tabla de la bibliografia: numero, referencia larga, estado.
        let a = repartir(
            &[20.0, 80.0, 70.0],
            &[20.0, 900.0, 70.0],
            Some(&[40.0, 700.0, 50.0]),
            790.0,
            1000.0,
        );
        assert!(a[2] >= 70.0 - 0.01, "«Cerrado» no se parte: {a:?}");
        assert!(
            (a.iter().sum::<f32>() - 790.0).abs() < 0.1,
            "lo que falta sale de la ancha: {a:?}"
        );
        assert!(
            a[1] < 700.0 && a[1] > 650.0,
            "casi todo lo pone la ancha: {a:?}"
        );
        assert!(
            a[0] >= 20.0 && a[0] <= 40.0,
            "la del numero no pasa de lo suyo: {a:?}"
        );
    }

    #[test]
    fn sin_rejilla_una_tabla_estrecha_no_se_estira_a_toda_la_columna() {
        let a = repartir(&[10.0, 10.0], &[50.0, 60.0], None, 800.0, 1000.0);
        assert_eq!(a, vec![50.0, 60.0]);
    }

    #[test]
    fn sin_rejilla_una_tabla_larga_reparte_entre_minimo_y_maximo() {
        let a = repartir(&[100.0, 100.0], &[100.0, 1100.0], None, 600.0, 1000.0);
        assert_eq!(a, vec![100.0, 500.0], "la corta se queda en lo suyo");
    }

    #[test]
    fn una_tabla_mas_ancha_que_el_tope_se_estrecha_hasta_el_tope() {
        let a = repartir(&[400.0, 400.0, 400.0], &[400.0; 3], None, 600.0, 900.0);
        assert!((a.iter().sum::<f32>() - 900.0).abs() < 0.1, "{a:?}");
        // Y una que cabe en el tope pero no en la columna, se despliega.
        let b = repartir(&[350.0, 350.0], &[350.0; 2], None, 600.0, 900.0);
        assert_eq!(b.iter().sum::<f32>(), 700.0);
    }

    #[test]
    fn la_palabra_y_el_parrafo_mas_largos_se_eligen_por_letras() {
        assert_eq!(palabra_mas_larga("un  ejemplo corto"), "ejemplo");
        assert_eq!(palabra_mas_larga(""), "");
        assert_eq!(parrafo_mas_largo("uno\ndos largos\ntres"), "dos largos");
    }
}
