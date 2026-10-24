//! Reconstruccion de parrafos a partir de las lineas sueltas que devuelve el
//! OCR.
//!
//! `Windows.Media.Ocr` entrega una lista plana de lineas con su caja, sin
//! ninguna nocion de parrafo ni de columna. Copiar esa lista tal cual produce
//! un texto inservible: se pierden los saltos de parrafo y, en cuanto la
//! captura tiene dos columnas, las lineas se intercalan por altura y las
//! frases quedan cosidas unas dentro de otras.
//!
//! Recuperar esa estructura es geometria pura, asi que vive aqui y no en la
//! capa que habla con Windows: se prueba en milisegundos con disposiciones
//! inventadas —dos columnas, un titulo enorme, una nota al margen— que serian
//! carisimas de reproducir pasando imagenes reales por el OCR.

use crate::rect::Rect;

/// Una linea reconocida por el OCR, con el sitio exacto donde estaba.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineaTexto {
    pub caja: Rect,
    pub texto: String,
}

/// Hueco vertical maximo entre dos lineas del mismo parrafo, medido en
/// fracciones de la altura tipica de linea.
///
/// El interlineado normal deja entre cajas consecutivas mucho menos de media
/// altura de linea, mientras que un salto de parrafo suele valer una linea en
/// blanco entera (>= 1.0). 0.6 cae holgadamente entre ambos, asi que tolera
/// textos de interlineado generoso sin llegar a tragarse un salto de parrafo.
const HUECO_MAXIMO_ENTRE_LINEAS: f32 = 0.60;

/// Solape horizontal minimo entre dos lineas del mismo parrafo, en fracciones
/// del ancho de la mas estrecha.
///
/// Las lineas de un parrafo comparten la misma columna de texto y se solapan
/// casi por completo; incluso la ultima linea, corta, o una vinieta sangrada
/// pasan de sobra de un tercio. Exigir este minimo —en vez de conformarse con
/// que se rocen— evita que un numero de pagina o una nota al margen que apenas
/// toca el borde del bloque acabe pegada al final del parrafo.
const SOLAPE_HORIZONTAL_MINIMO: f32 = 0.30;

/// Agrupa las lineas en parrafos, en orden de lectura.
///
/// Primero separa columnas y luego parte cada columna en parrafos, nunca al
/// reves: una columna se lee entera antes de pasar a la siguiente, que es
/// justo lo que se pierde si se ordena todo por altura.
pub fn agrupar(lineas: Vec<LineaTexto>) -> Vec<Vec<LineaTexto>> {
    if lineas.is_empty() {
        return Vec::new();
    }

    // El umbral se calcula una sola vez sobre el documento entero: dentro de
    // una columna corta puede no haber lineas suficientes para estimar bien la
    // altura tipica.
    let hueco_maximo = (mediana_de_alturas(&lineas) as f32 * HUECO_MAXIMO_ENTRE_LINEAS) as i32;

    let mut grupos: Vec<Vec<LineaTexto>> = Vec::new();
    for mut columna in repartir_en_columnas(lineas) {
        // El OCR no garantiza ningun orden; el desempate por la izquierda hace
        // el criterio total para que el resultado no dependa del de entrada.
        columna.sort_by_key(|l| (l.caja.arriba(), l.caja.izquierda()));

        let mut parrafo: Vec<LineaTexto> = Vec::new();
        for linea in columna {
            let sigue_el_parrafo = match parrafo.last() {
                None => true,
                Some(anterior) => {
                    // Puede salir negativo cuando dos cajas se solapan en
                    // vertical, y entonces es que van juntas de sobra.
                    let hueco = linea.caja.arriba().saturating_sub(anterior.caja.abajo());
                    hueco <= hueco_maximo && se_solapan_en_horizontal(anterior.caja, linea.caja)
                }
            };
            if !sigue_el_parrafo {
                grupos.push(std::mem::take(&mut parrafo));
            }
            parrafo.push(linea);
        }
        if !parrafo.is_empty() {
            grupos.push(parrafo);
        }
    }
    grupos
}

/// Vuelca los grupos a texto plano: las lineas de un parrafo unidas por un
/// espacio y los parrafos separados por una linea en blanco.
///
/// Sin salto final, porque lo normal es pegar esto en otro sitio y una linea
/// vacia de propina se nota.
pub fn a_texto(grupos: &[Vec<LineaTexto>]) -> String {
    grupos
        .iter()
        .map(|parrafo| {
            parrafo
                .iter()
                .map(|l| l.texto.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Reparte las lineas en columnas: bloques cuyos rangos horizontales no se
/// solapan entre si, devueltos de izquierda a derecha.
///
/// El solape se propaga —A con B y B con C dejan a las tres en la misma
/// columna— porque una columna de texto real es irregular: sangrias, lineas
/// cortas y vinietas hacen que dos lineas de la misma columna a veces no se
/// solapen directamente entre ellas.
fn repartir_en_columnas(lineas: Vec<LineaTexto>) -> Vec<Vec<LineaTexto>> {
    let mut ordenadas = lineas;
    ordenadas.sort_by_key(|l| (l.caja.izquierda(), l.caja.derecha()));

    let mut columnas: Vec<Vec<LineaTexto>> = Vec::new();
    // Borde derecho de todo lo acumulado en la columna que se esta llenando.
    let mut borde_derecho = i32::MIN;
    for linea in ordenadas {
        match columnas.last_mut() {
            Some(columna) if linea.caja.izquierda() < borde_derecho => {
                borde_derecho = borde_derecho.max(linea.caja.derecha());
                columna.push(linea);
            }
            _ => {
                borde_derecho = linea.caja.derecha();
                columnas.push(vec![linea]);
            }
        }
    }
    columnas
}

/// Altura tipica de linea.
///
/// Mediana y no media: un titulo o un logotipo miden varias veces lo que el
/// texto corrido y la media se iria detras de ellos, ensanchando el umbral
/// hasta fundir parrafos que estaban separados.
fn mediana_de_alturas(lineas: &[LineaTexto]) -> u32 {
    let mut alturas: Vec<u32> = lineas
        .iter()
        .map(|l| l.caja.alto)
        // Las cajas degeneradas del OCR no dicen nada de la altura tipica y
        // arrastrarian la mediana hacia cero.
        .filter(|alto| *alto > 0)
        .collect();
    if alturas.is_empty() {
        // No hay ninguna escala de la que fiarse: con umbral 1 solo siguen
        // juntas las lineas practicamente pegadas, que es lo prudente.
        return 1;
    }
    alturas.sort_unstable();
    alturas[alturas.len() / 2]
}

fn se_solapan_en_horizontal(a: Rect, b: Rect) -> bool {
    let solape = a
        .derecha()
        .min(b.derecha())
        .saturating_sub(a.izquierda().max(b.izquierda()));
    if solape <= 0 {
        return false;
    }
    let ancho_menor = a.ancho.min(b.ancho);
    if ancho_menor == 0 {
        // Una caja sin ancho no se solapa con nada: no hay fraccion que medir.
        return false;
    }
    solape as f32 >= ancho_menor as f32 * SOLAPE_HORIZONTAL_MINIMO
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn linea(x: i32, y: i32, ancho: u32, alto: u32, texto: &str) -> LineaTexto {
        LineaTexto {
            caja: Rect { x, y, ancho, alto },
            texto: texto.to_string(),
        }
    }

    fn textos(grupos: &[Vec<LineaTexto>]) -> Vec<Vec<&str>> {
        grupos
            .iter()
            .map(|p| p.iter().map(|l| l.texto.as_str()).collect())
            .collect()
    }

    #[test]
    fn tres_lineas_juntas_forman_un_solo_parrafo() {
        let grupos = agrupar(vec![
            linea(0, 0, 200, 18, "una"),
            linea(0, 20, 200, 18, "dos"),
            linea(0, 40, 200, 18, "tres"),
        ]);
        assert_eq!(textos(&grupos), vec![vec!["una", "dos", "tres"]]);
    }

    #[test]
    fn dos_parrafos_separados_por_un_hueco_no_se_juntan() {
        // Interlineado de 2 px dentro del parrafo y una linea en blanco entera
        // antes del siguiente.
        let grupos = agrupar(vec![
            linea(0, 0, 200, 18, "una"),
            linea(0, 20, 200, 18, "dos"),
            linea(0, 80, 200, 18, "otra cosa"),
        ]);
        assert_eq!(textos(&grupos), vec![vec!["una", "dos"], vec!["otra cosa"]]);
    }

    #[test]
    fn dos_columnas_se_leen_enteras_una_despues_de_otra() {
        // Las alturas van intercaladas a proposito: ordenar por y daria
        // "a1 b1 a2 b2 a3 b3", que es exactamente el texto sin sentido que
        // este modulo existe para evitar.
        let grupos = agrupar(vec![
            linea(150, 10, 100, 18, "b1"),
            linea(0, 0, 100, 18, "a1"),
            linea(150, 30, 100, 18, "b2"),
            linea(0, 20, 100, 18, "a2"),
            linea(150, 50, 100, 18, "b3"),
            linea(0, 40, 100, 18, "a3"),
        ]);
        assert_eq!(
            textos(&grupos),
            vec![vec!["a1", "a2", "a3"], vec!["b1", "b2", "b3"]]
        );
    }

    #[test]
    fn una_linea_suelta_a_la_derecha_no_entra_en_el_parrafo_de_la_izquierda() {
        // Caso negativo: la nota al margen esta a la altura del parrafo, pero
        // no comparte su rango horizontal, asi que no puede continuarlo.
        let grupos = agrupar(vec![
            linea(0, 0, 200, 18, "una"),
            linea(0, 20, 200, 18, "dos"),
            linea(400, 20, 80, 18, "nota"),
            linea(0, 40, 200, 18, "tres"),
        ]);
        assert_eq!(
            textos(&grupos),
            vec![vec!["una", "dos", "tres"], vec!["nota"]]
        );
    }

    #[test]
    fn un_titulo_alto_no_arrastra_el_umbral_del_texto_pequeno() {
        // El titulo mide 100 y el cuerpo 20. La media de alturas seria 33 y
        // con ella el hueco de 16 que separa titulo y cuerpo pasaria por
        // interlineado normal, fundiendolos en un solo parrafo. La mediana
        // vale 20 y los mantiene separados.
        let grupos = agrupar(vec![
            linea(0, 0, 300, 100, "Titulo"),
            linea(0, 116, 300, 20, "cuerpo uno"),
            linea(0, 140, 300, 20, "cuerpo dos"),
            linea(0, 164, 300, 20, "cuerpo tres"),
            linea(0, 188, 300, 20, "cuerpo cuatro"),
            linea(0, 212, 300, 20, "cuerpo cinco"),
        ]);
        assert_eq!(grupos.len(), 2);
        assert_eq!(textos(&grupos)[0], vec!["Titulo"]);
        assert_eq!(grupos[1].len(), 5);
    }

    #[test]
    fn una_lista_vacia_devuelve_cero_grupos() {
        // Caso negativo: sin lineas no hay altura tipica que calcular; el
        // umbral no llega a usarse y no hay nada que dividir.
        assert!(agrupar(Vec::new()).is_empty());
    }

    #[test]
    fn una_sola_linea_devuelve_un_grupo_de_una() {
        let grupos = agrupar(vec![linea(0, 0, 100, 18, "sola")]);
        assert_eq!(textos(&grupos), vec![vec!["sola"]]);
    }

    #[test]
    fn las_cajas_de_tamano_cero_no_rompen_nada() {
        // Caso negativo: el OCR puede devolver una caja degenerada. No debe
        // desaparecer texto ni contaminarse la mediana de alturas.
        let grupos = agrupar(vec![
            linea(0, 0, 100, 20, "arriba"),
            linea(10, 30, 0, 0, "fantasma"),
            linea(0, 50, 100, 20, "abajo"),
        ]);
        let plano = a_texto(&grupos);
        assert!(plano.contains("arriba"));
        assert!(plano.contains("fantasma"));
        assert!(plano.contains("abajo"));
        // Sin ancho no se solapa con nadie, asi que se queda sola.
        assert_eq!(grupos.len(), 3);
    }

    #[test]
    fn a_texto_separa_parrafos_con_una_linea_en_blanco_y_no_deja_salto_final() {
        let grupos = agrupar(vec![
            linea(0, 0, 200, 18, "una"),
            linea(0, 20, 200, 18, "dos"),
            linea(0, 80, 200, 18, "tres"),
        ]);
        assert_eq!(a_texto(&grupos), "una dos\n\ntres");
    }

    #[test]
    fn a_texto_sin_grupos_da_una_cadena_vacia() {
        // Caso negativo: nada que unir no debe producir ni un salto de linea.
        assert_eq!(a_texto(&[]), "");
    }

    #[test]
    fn el_resultado_no_depende_del_orden_de_entrada() {
        let esperado = vec![vec!["a1", "a2"], vec!["b1", "b2"]];
        let directo = vec![
            linea(0, 0, 100, 18, "a1"),
            linea(0, 20, 100, 18, "a2"),
            linea(150, 0, 100, 18, "b1"),
            linea(150, 20, 100, 18, "b2"),
        ];
        let revuelto = vec![
            linea(150, 20, 100, 18, "b2"),
            linea(0, 20, 100, 18, "a2"),
            linea(150, 0, 100, 18, "b1"),
            linea(0, 0, 100, 18, "a1"),
        ];
        assert_eq!(textos(&agrupar(directo)), esperado);
        assert_eq!(textos(&agrupar(revuelto)), esperado);
    }
}
